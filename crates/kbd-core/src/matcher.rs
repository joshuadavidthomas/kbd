//! Binding matcher — finds which binding (if any) matches a key event.
//!
//! Walks the layer stack top-down, checking bindings in each active layer,
//! then global bindings. Within each layer, speculative patterns (tap-hold,
//! sequences) are checked before immediate patterns (hotkeys).
//!
//! Returns the matched binding's action (or "no match" for forwarding).

use std::collections::HashMap;
use std::time::Duration;
use std::time::Instant;

use crate::action::Action;
use crate::action::LayerName;
use crate::binding::BindingId;
use crate::binding::Passthrough;
use crate::binding::RegisteredBinding;
use crate::key::Hotkey;
use crate::key::Modifier;
use crate::key_state::KeyTransition;
use crate::layer::StoredLayer;
use crate::layer::UnmatchedKeyBehavior;

/// Result of attempting to match a key event against registered bindings.
#[derive(Debug)]
pub enum MatchResult<'a> {
    /// A binding matched. Contains the action and passthrough setting.
    Matched {
        action: &'a Action,
        passthrough: Passthrough,
    },
    /// A multi-step sequence is in progress. Consumers can use this for
    /// UI feedback ("waiting for next key…"). Produced by the sequence
    /// state machine (Phase 4).
    Pending {
        steps_matched: usize,
        steps_remaining: usize,
    },
    /// No binding matched the event.
    NoMatch,
    /// The event was swallowed by a layer with `UnmatchedKeyBehavior::Swallow`.
    Swallowed,
    /// The event was not eligible for matching (modifier-only press, release, repeat).
    Ignored,
}

/// An entry in the layer stack, pairing the layer name with runtime state.
pub struct LayerStackEntry {
    pub name: LayerName,
    /// Remaining keypress count for oneshot layers. `None` means not oneshot.
    pub oneshot_remaining: Option<usize>,
    /// Timeout configuration and last activity timestamp.
    /// If set, the layer auto-pops when `Instant::now() - last_activity > timeout`.
    pub timeout: Option<LayerTimeout>,
}

pub struct LayerTimeout {
    pub duration: Duration,
    pub last_activity: Instant,
}

/// Attempt to find a binding that matches the given key event.
///
/// Matching walks the layer stack top-down:
/// 1. For each active layer (most recent first):
///    - Check that layer's bindings against the candidate hotkey
///    - If a binding matches, stop — this layer owns this event
///    - If no binding matches and the layer has `Swallow`, the event is consumed
///    - If no binding matches and the layer has `Fallthrough`, continue to next layer
/// 2. Check global bindings (always-active base layer)
/// 3. If nothing matched, the event is unmatched
///
/// Only key press events trigger matching — release events use the press
/// cache (see engine's `process_key_event`), repeat events are ignored.
#[must_use]
#[allow(clippy::implicit_hasher)]
pub fn match_key_event<'a>(
    transition: KeyTransition,
    candidate: &Hotkey,
    layer_stack: &[LayerStackEntry],
    layers: &'a HashMap<LayerName, StoredLayer>,
    binding_ids_by_hotkey: &HashMap<Hotkey, BindingId>,
    bindings_by_id: &'a HashMap<BindingId, RegisteredBinding>,
) -> MatchResult<'a> {
    // Only match on key press events
    if !matches!(transition, KeyTransition::Press) {
        return MatchResult::Ignored;
    }

    // Skip if the pressed key is itself a modifier — modifier-only presses
    // don't trigger hotkeys (they modify state for subsequent presses)
    if Modifier::from_key(candidate.key()).is_some() {
        return MatchResult::Ignored;
    }

    // Walk layer stack top-down (most recently pushed first)
    for entry in layer_stack.iter().rev() {
        if let Some(stored_layer) = layers.get(&entry.name) {
            // Check this layer's bindings
            for layer_binding in &stored_layer.bindings {
                if layer_binding.hotkey == *candidate {
                    return MatchResult::Matched {
                        action: &layer_binding.action,
                        passthrough: layer_binding.passthrough,
                    };
                }
            }

            // No match in this layer — check swallow behavior
            if matches!(
                stored_layer.options.unmatched(),
                UnmatchedKeyBehavior::Swallow
            ) {
                return MatchResult::Swallowed;
            }
        }
    }

    // Fall through to global bindings
    if let Some(&id) = binding_ids_by_hotkey.get(candidate)
        && let Some(binding) = bindings_by_id.get(&id)
    {
        return MatchResult::Matched {
            action: binding.action(),
            passthrough: binding.passthrough(),
        };
    }

    MatchResult::NoMatch
}

// TODO: Speculative vs immediate pattern ordering
// TODO: Device filter checking (per-binding, not per-dispatch-phase)

/// A synchronous keyboard shortcut matching engine.
///
/// `Matcher` is the embeddable engine. No threads, no channels, no evdev.
/// Consumers drive it from their own event loop — winit, GPUI, Smithay,
/// a game loop, whatever.
///
/// # Example
///
/// ```rust
/// use kbd_core::{Action, Hotkey, Key, Layer, Matcher, Modifier};
/// use kbd_core::key_state::KeyTransition;
/// use kbd_core::matcher::MatchResult;
///
/// let mut matcher = Matcher::new();
/// let hotkey = Hotkey::new(Key::S).modifier(Modifier::Ctrl);
/// matcher.register(hotkey, Action::Swallow).unwrap();
///
/// let candidate = Hotkey::new(Key::S).modifier(Modifier::Ctrl);
/// let result = matcher.process(&candidate, KeyTransition::Press);
/// assert!(matches!(result, MatchResult::Matched { .. }));
/// ```
pub struct Matcher {
    bindings_by_id: HashMap<BindingId, RegisteredBinding>,
    binding_ids_by_hotkey: HashMap<Hotkey, BindingId>,
    layers: HashMap<LayerName, StoredLayer>,
    layer_stack: Vec<LayerStackEntry>,
}

/// Internal reference to a matched binding, used to re-find the action
/// after layer mutations are applied.
enum MatchedBindingRef {
    Global(BindingId),
    Layer { name: LayerName, index: usize },
}

/// Internal match outcome that carries binding refs and layer effects.
enum InternalOutcome {
    Matched {
        binding_ref: MatchedBindingRef,
        layer_effect: LayerEffect,
        passthrough: Passthrough,
    },
    Swallowed,
    NoMatch,
    Ignored,
}

/// Layer stack mutation extracted from a matched action.
enum LayerEffect {
    None,
    Push(LayerName),
    Pop,
    Toggle(LayerName),
}

impl LayerEffect {
    fn from_action(action: &Action) -> Self {
        match action {
            Action::PushLayer(name) => Self::Push(name.clone()),
            Action::PopLayer => Self::Pop,
            Action::ToggleLayer(name) => Self::Toggle(name.clone()),
            Action::Callback(_)
            | Action::EmitKey(..)
            | Action::EmitSequence(..)
            | Action::Swallow => Self::None,
        }
    }
}

impl Matcher {
    /// Create a new empty matcher with no bindings or layers.
    #[must_use]
    pub fn new() -> Self {
        Self {
            bindings_by_id: HashMap::new(),
            binding_ids_by_hotkey: HashMap::new(),
            layers: HashMap::new(),
            layer_stack: Vec::new(),
        }
    }

    /// Register a binding. Returns the assigned [`BindingId`].
    ///
    /// Returns `Error::AlreadyRegistered` if a binding for the same hotkey exists.
    pub fn register(
        &mut self,
        hotkey: impl Into<Hotkey>,
        action: impl Into<Action>,
    ) -> Result<BindingId, crate::Error> {
        let id = BindingId::new();
        let binding = RegisteredBinding::new(id, hotkey.into(), action.into());
        self.register_binding(binding)?;
        Ok(id)
    }

    /// Register a [`RegisteredBinding`] with full options control.
    ///
    /// Returns `Error::AlreadyRegistered` if a binding for the same hotkey exists.
    pub fn register_binding(&mut self, binding: RegisteredBinding) -> Result<(), crate::Error> {
        let id = binding.id();
        let hotkey = binding.hotkey().clone();

        if self.bindings_by_id.contains_key(&id) || self.binding_ids_by_hotkey.contains_key(&hotkey)
        {
            return Err(crate::Error::AlreadyRegistered);
        }

        self.binding_ids_by_hotkey.insert(hotkey, id);
        self.bindings_by_id.insert(id, binding);
        Ok(())
    }

    /// Unregister a binding by its [`BindingId`].
    pub fn unregister(&mut self, id: BindingId) {
        if let Some(binding) = self.bindings_by_id.remove(&id) {
            self.binding_ids_by_hotkey.remove(binding.hotkey());
        }
    }

    /// Check whether a hotkey has a registered global binding.
    #[must_use]
    pub fn is_registered(&self, hotkey: &Hotkey) -> bool {
        self.binding_ids_by_hotkey.contains_key(hotkey)
    }

    /// Define a named layer. The layer is not active until pushed.
    ///
    /// Returns `Error::LayerAlreadyDefined` if a layer with the same name exists.
    pub fn define_layer(&mut self, layer: crate::layer::Layer) -> Result<(), crate::Error> {
        let (name, bindings, options) = layer.into_parts();
        match self.layers.entry(name) {
            std::collections::hash_map::Entry::Occupied(_) => {
                Err(crate::Error::LayerAlreadyDefined)
            }
            std::collections::hash_map::Entry::Vacant(entry) => {
                entry.insert(StoredLayer { bindings, options });
                Ok(())
            }
        }
    }

    /// Push a named layer onto the stack, activating its bindings.
    ///
    /// Returns `Error::LayerNotDefined` if no layer with this name is defined.
    pub fn push_layer(&mut self, name: impl Into<LayerName>) -> Result<(), crate::Error> {
        let name = name.into();
        let stored = self
            .layers
            .get(&name)
            .ok_or(crate::Error::LayerNotDefined)?;
        let oneshot_remaining = stored.options.oneshot();
        let timeout = stored.options.timeout().map(|duration| LayerTimeout {
            duration,
            last_activity: Instant::now(),
        });
        self.layer_stack.push(LayerStackEntry {
            name,
            oneshot_remaining,
            timeout,
        });
        Ok(())
    }

    /// Pop the topmost layer from the stack.
    ///
    /// Returns the popped layer's name, or `Error::EmptyLayerStack` if empty.
    pub fn pop_layer(&mut self) -> Result<LayerName, crate::Error> {
        self.layer_stack
            .pop()
            .map(|entry| entry.name)
            .ok_or(crate::Error::EmptyLayerStack)
    }

    /// Toggle a layer: push if not active, remove if active.
    ///
    /// Returns `Error::LayerNotDefined` if no layer with this name is defined.
    pub fn toggle_layer(&mut self, name: impl Into<LayerName>) -> Result<(), crate::Error> {
        let name = name.into();
        if !self.layers.contains_key(&name) {
            return Err(crate::Error::LayerNotDefined);
        }
        if let Some(pos) = self
            .layer_stack
            .iter()
            .rposition(|entry| entry.name == name)
        {
            self.layer_stack.remove(pos);
        } else {
            self.push_layer(name)?;
        }
        Ok(())
    }

    /// Process a key event and return the match result.
    ///
    /// The caller provides the hotkey (key + currently active modifiers)
    /// and the key transition. The matcher walks the layer stack, finds
    /// the matching binding, and applies layer effects (push/pop/toggle)
    /// internally.
    ///
    /// Only key press events trigger matching — release and repeat events
    /// return `MatchResult::Ignored`. Modifier-only presses also return
    /// `MatchResult::Ignored`.
    pub fn process(&mut self, hotkey: &Hotkey, transition: KeyTransition) -> MatchResult<'_> {
        // Phase 1: Match and extract outcome (temporary borrow of &self)
        let outcome = self.match_extract(hotkey, transition);

        // Phase 2: Apply layer effects (&mut self)
        if let InternalOutcome::Matched {
            ref layer_effect, ..
        } = outcome
        {
            self.apply_layer_effect(layer_effect);
        }

        // Phase 3: Tick oneshot and reset timeouts for actionable events
        if !matches!(outcome, InternalOutcome::Ignored) {
            self.reset_layer_timeouts();
            self.tick_oneshot_layers();
        }

        // Phase 4: Convert to MatchResult by re-borrowing &self
        match outcome {
            InternalOutcome::Matched {
                binding_ref,
                passthrough,
                ..
            } => {
                let action = self.resolve_binding(&binding_ref);
                MatchResult::Matched {
                    action,
                    passthrough,
                }
            }
            InternalOutcome::Swallowed => MatchResult::Swallowed,
            InternalOutcome::NoMatch => MatchResult::NoMatch,
            InternalOutcome::Ignored => MatchResult::Ignored,
        }
    }

    /// Return the nearest layer timeout deadline, if any.
    ///
    /// Returns `None` if no timeout layers are active. The caller can
    /// use this to set poll timeouts in their event loop.
    #[must_use]
    pub fn next_timeout_deadline(&self) -> Option<Duration> {
        let now = Instant::now();
        let mut min_remaining = None;

        for entry in &self.layer_stack {
            if let Some(timeout) = &entry.timeout {
                let elapsed = now.duration_since(timeout.last_activity);
                let remaining = timeout.duration.saturating_sub(elapsed);
                min_remaining = Some(match min_remaining {
                    Some(current) => std::cmp::min(current, remaining),
                    None => remaining,
                });
            }
        }

        min_remaining
    }

    /// Check all active timeout layers and pop any that have expired.
    ///
    /// Call this periodically from your event loop (e.g., on each poll
    /// cycle) to ensure timeout layers auto-pop on schedule.
    pub fn check_timeouts(&mut self) {
        let now = Instant::now();
        self.layer_stack.retain(|entry| {
            if let Some(timeout) = &entry.timeout {
                now.duration_since(timeout.last_activity) < timeout.duration
            } else {
                true
            }
        });
    }

    /// Return a snapshot of all registered bindings with their status.
    #[must_use]
    pub fn list_bindings(&self) -> Vec<crate::introspection::BindingInfo> {
        use crate::introspection::BindingInfo;
        use crate::introspection::BindingLocation;
        use crate::introspection::ShadowedStatus;

        // Build a map of hotkey → claiming layer name for active layers.
        // Walk top-down so the topmost layer claiming a hotkey "wins".
        let mut claimed_by: HashMap<&Hotkey, &LayerName> = HashMap::new();
        for entry in self.layer_stack.iter().rev() {
            if let Some(stored) = self.layers.get(&entry.name) {
                for binding in &stored.bindings {
                    claimed_by.entry(&binding.hotkey).or_insert(&entry.name);
                }
            }
        }

        let mut results = Vec::new();

        // Global bindings
        for binding in self.bindings_by_id.values() {
            let shadowed = if let Some(&layer_name) = claimed_by.get(binding.hotkey()) {
                ShadowedStatus::ShadowedBy(layer_name.clone())
            } else {
                ShadowedStatus::Active
            };

            results.push(BindingInfo {
                hotkey: binding.hotkey().clone(),
                description: binding.options().description().map(Box::from),
                location: BindingLocation::Global,
                shadowed,
                overlay_visibility: binding.options().overlay_visibility(),
            });
        }

        // Layer bindings (all defined layers, active or not)
        for (layer_name, stored) in &self.layers {
            let is_active = self.layer_stack.iter().any(|e| &e.name == layer_name);

            for binding in &stored.bindings {
                let shadowed = if !is_active {
                    ShadowedStatus::Inactive
                } else if let Some(&claiming_layer) = claimed_by.get(&binding.hotkey) {
                    if claiming_layer == layer_name {
                        ShadowedStatus::Active
                    } else {
                        ShadowedStatus::ShadowedBy(claiming_layer.clone())
                    }
                } else {
                    ShadowedStatus::Active
                };

                results.push(BindingInfo {
                    hotkey: binding.hotkey.clone(),
                    description: None,
                    location: BindingLocation::Layer(layer_name.clone()),
                    shadowed,
                    overlay_visibility: crate::binding::OverlayVisibility::Visible,
                });
            }
        }

        results
    }

    /// Query what would fire if this hotkey were pressed now.
    ///
    /// Considers the current layer stack. Returns `None` if nothing
    /// would match (including swallow-layer suppression).
    #[must_use]
    pub fn bindings_for_key(&self, hotkey: &Hotkey) -> Option<crate::introspection::BindingInfo> {
        use crate::introspection::BindingInfo;
        use crate::introspection::BindingLocation;
        use crate::introspection::ShadowedStatus;

        // Modifier-only keys never fire bindings in the real matcher,
        // so they can't match here either.
        if Modifier::from_key(hotkey.key()).is_some() {
            return None;
        }

        // Walk layer stack top-down, same as the matcher
        for entry in self.layer_stack.iter().rev() {
            if let Some(stored) = self.layers.get(&entry.name) {
                for binding in &stored.bindings {
                    if binding.hotkey == *hotkey {
                        return Some(BindingInfo {
                            hotkey: binding.hotkey.clone(),
                            description: None,
                            location: BindingLocation::Layer(entry.name.clone()),
                            shadowed: ShadowedStatus::Active,
                            overlay_visibility: crate::binding::OverlayVisibility::Visible,
                        });
                    }
                }

                // Swallow layers block all unmatched keys from reaching
                // lower layers and globals — matches the real matcher.
                if matches!(stored.options.unmatched(), UnmatchedKeyBehavior::Swallow) {
                    return None;
                }
            }
        }

        // Fall through to global bindings
        if let Some(&id) = self.binding_ids_by_hotkey.get(hotkey)
            && let Some(binding) = self.bindings_by_id.get(&id)
        {
            return Some(BindingInfo {
                hotkey: binding.hotkey().clone(),
                description: binding.options().description().map(Box::from),
                location: BindingLocation::Global,
                shadowed: ShadowedStatus::Active,
                overlay_visibility: binding.options().overlay_visibility(),
            });
        }

        None
    }

    /// Return the current layer stack (bottom to top).
    #[must_use]
    pub fn active_layers(&self) -> Vec<crate::introspection::ActiveLayerInfo> {
        self.layer_stack
            .iter()
            .filter_map(|entry| {
                self.layers
                    .get(&entry.name)
                    .map(|stored| crate::introspection::ActiveLayerInfo {
                        name: entry.name.clone(),
                        description: stored.options.description().map(Box::from),
                        binding_count: stored.bindings.len(),
                    })
            })
            .collect()
    }

    /// Return bindings shadowed by higher-priority layers.
    #[must_use]
    pub fn conflicts(&self) -> Vec<crate::introspection::ConflictInfo> {
        use crate::introspection::BindingLocation;
        use crate::introspection::ConflictInfo;
        use crate::introspection::ShadowedStatus;

        let all_bindings = self.list_bindings();
        let mut conflicts = Vec::new();

        for shadowed in &all_bindings {
            if let ShadowedStatus::ShadowedBy(ref shadowing_layer) = shadowed.shadowed
                && let Some(shadowing) = all_bindings.iter().find(|b| {
                    b.hotkey == shadowed.hotkey
                        && b.location == BindingLocation::Layer(shadowing_layer.clone())
                        && matches!(b.shadowed, ShadowedStatus::Active)
                })
            {
                conflicts.push(ConflictInfo {
                    hotkey: shadowed.hotkey.clone(),
                    shadowed_binding: shadowed.clone(),
                    shadowing_binding: shadowing.clone(),
                });
            }
        }

        conflicts
    }

    // Internal matching logic

    /// Match a hotkey against the layer stack and global bindings.
    /// Returns an `InternalOutcome` that carries binding refs and layer effects.
    fn match_extract(&self, hotkey: &Hotkey, transition: KeyTransition) -> InternalOutcome {
        // Only match on key press events
        if !matches!(transition, KeyTransition::Press) {
            return InternalOutcome::Ignored;
        }

        // Modifier-only presses don't trigger hotkeys
        if Modifier::from_key(hotkey.key()).is_some() {
            return InternalOutcome::Ignored;
        }

        // Walk layer stack top-down
        for entry in self.layer_stack.iter().rev() {
            if let Some(stored) = self.layers.get(&entry.name) {
                for (index, layer_binding) in stored.bindings.iter().enumerate() {
                    if layer_binding.hotkey == *hotkey {
                        return InternalOutcome::Matched {
                            binding_ref: MatchedBindingRef::Layer {
                                name: entry.name.clone(),
                                index,
                            },
                            layer_effect: LayerEffect::from_action(&layer_binding.action),
                            passthrough: layer_binding.passthrough,
                        };
                    }
                }

                if matches!(stored.options.unmatched(), UnmatchedKeyBehavior::Swallow) {
                    return InternalOutcome::Swallowed;
                }
            }
        }

        // Fall through to global bindings
        if let Some(&id) = self.binding_ids_by_hotkey.get(hotkey)
            && self.bindings_by_id.contains_key(&id)
        {
            let action = self.bindings_by_id[&id].action();
            return InternalOutcome::Matched {
                binding_ref: MatchedBindingRef::Global(id),
                layer_effect: LayerEffect::from_action(action),
                passthrough: self.bindings_by_id[&id].passthrough(),
            };
        }

        InternalOutcome::NoMatch
    }

    /// Resolve a binding reference back to its action.
    fn resolve_binding(&self, binding_ref: &MatchedBindingRef) -> &Action {
        match binding_ref {
            MatchedBindingRef::Global(id) => self.bindings_by_id[id].action(),
            MatchedBindingRef::Layer { name, index } => &self.layers[name].bindings[*index].action,
        }
    }

    /// Apply a layer effect extracted from a matched action.
    fn apply_layer_effect(&mut self, effect: &LayerEffect) {
        match effect {
            LayerEffect::None => {}
            LayerEffect::Push(name) => {
                let _ = self.push_layer(name.clone());
            }
            LayerEffect::Pop => {
                let _ = self.pop_layer();
            }
            LayerEffect::Toggle(name) => {
                let _ = self.toggle_layer(name.clone());
            }
        }
    }

    /// Reset timeout clocks on all active timeout layers (activity occurred).
    fn reset_layer_timeouts(&mut self) {
        let now = Instant::now();
        for entry in &mut self.layer_stack {
            if let Some(timeout) = &mut entry.timeout {
                timeout.last_activity = now;
            }
        }
    }

    /// Decrement oneshot counters and pop expired layers.
    fn tick_oneshot_layers(&mut self) {
        let mut pop_index = None;
        for (i, entry) in self.layer_stack.iter_mut().enumerate().rev() {
            if let Some(remaining) = &mut entry.oneshot_remaining {
                *remaining = remaining.saturating_sub(1);
                if *remaining == 0 {
                    pop_index = Some(i);
                }
                break;
            }
        }
        if let Some(index) = pop_index {
            self.layer_stack.remove(index);
        }
    }
}

impl Default for Matcher {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::Arc;
    use std::sync::atomic::AtomicUsize;
    use std::sync::atomic::Ordering;

    use super::LayerStackEntry;
    use super::MatchResult;
    use super::match_key_event;
    use crate::Key;
    use crate::action::Action;
    use crate::action::LayerName;
    use crate::binding::BindingId;
    use crate::binding::Passthrough;
    use crate::binding::RegisteredBinding;
    use crate::key::Hotkey;
    use crate::key::Modifier;
    use crate::key_state::KeyTransition;
    use crate::layer::LayerBinding;
    use crate::layer::LayerOptions;
    use crate::layer::StoredLayer;
    use crate::layer::UnmatchedKeyBehavior;

    struct TestBindings {
        bindings_by_id: HashMap<BindingId, RegisteredBinding>,
        binding_ids_by_hotkey: HashMap<Hotkey, BindingId>,
        layers: HashMap<LayerName, StoredLayer>,
        layer_stack: Vec<LayerStackEntry>,
    }

    impl TestBindings {
        fn new() -> Self {
            Self {
                bindings_by_id: HashMap::new(),
                binding_ids_by_hotkey: HashMap::new(),
                layers: HashMap::new(),
                layer_stack: Vec::new(),
            }
        }

        fn add(&mut self, key: Key, modifiers: &[Modifier], action: Action) -> BindingId {
            let id = BindingId::new();
            let hotkey = Hotkey::with_modifiers(key, modifiers.to_vec());
            self.binding_ids_by_hotkey.insert(hotkey.clone(), id);
            self.bindings_by_id
                .insert(id, RegisteredBinding::new(id, hotkey, action));
            id
        }

        fn add_layer(&mut self, name: &str, bindings: Vec<LayerBinding>, options: LayerOptions) {
            self.layers
                .insert(LayerName::from(name), StoredLayer { bindings, options });
        }

        fn push_layer(&mut self, name: &str) {
            self.layer_stack.push(LayerStackEntry {
                name: LayerName::from(name),
                oneshot_remaining: None,
                timeout: None,
            });
        }

        fn match_event(
            &self,
            key: Key,
            transition: KeyTransition,
            active_modifiers: &[Modifier],
        ) -> MatchResult<'_> {
            let candidate = Hotkey::with_modifiers(key, active_modifiers.to_vec());
            match_key_event(
                transition,
                &candidate,
                &self.layer_stack,
                &self.layers,
                &self.binding_ids_by_hotkey,
                &self.bindings_by_id,
            )
        }
    }

    fn layer_binding(key: Key, modifiers: &[Modifier], action: Action) -> LayerBinding {
        LayerBinding {
            hotkey: Hotkey::with_modifiers(key, modifiers.to_vec()),
            action,
            passthrough: Passthrough::default(),
        }
    }

    #[test]
    fn matches_simple_hotkey_on_press() {
        let mut bindings = TestBindings::new();
        let counter = Arc::new(AtomicUsize::new(0));
        let counter_clone = Arc::clone(&counter);
        bindings.add(
            Key::C,
            &[Modifier::Ctrl],
            Action::from(move || {
                counter_clone.fetch_add(1, Ordering::Relaxed);
            }),
        );

        let result = bindings.match_event(Key::C, KeyTransition::Press, &[Modifier::Ctrl]);
        let MatchResult::Matched {
            action: Action::Callback(cb),
            ..
        } = result
        else {
            panic!("expected Matched(Callback), got {result:?}");
        };
        cb();
        assert_eq!(counter.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn no_match_when_no_bindings_registered() {
        let bindings = TestBindings::new();
        let result = bindings.match_event(Key::A, KeyTransition::Press, &[]);
        assert!(matches!(result, MatchResult::NoMatch));
    }

    #[test]
    fn no_match_when_wrong_key() {
        let mut bindings = TestBindings::new();
        bindings.add(Key::C, &[Modifier::Ctrl], Action::Swallow);

        let result = bindings.match_event(Key::V, KeyTransition::Press, &[Modifier::Ctrl]);
        assert!(matches!(result, MatchResult::NoMatch));
    }

    #[test]
    fn no_match_when_missing_required_modifier() {
        let mut bindings = TestBindings::new();
        bindings.add(Key::C, &[Modifier::Ctrl], Action::Swallow);

        // No modifiers held
        let result = bindings.match_event(Key::C, KeyTransition::Press, &[]);
        assert!(matches!(result, MatchResult::NoMatch));
    }

    #[test]
    fn no_match_when_extra_modifier_held() {
        let mut bindings = TestBindings::new();
        bindings.add(Key::C, &[Modifier::Ctrl], Action::Swallow);

        // Ctrl+Shift held but binding only wants Ctrl
        let result = bindings.match_event(
            Key::C,
            KeyTransition::Press,
            &[Modifier::Ctrl, Modifier::Shift],
        );
        assert!(matches!(result, MatchResult::NoMatch));
    }

    #[test]
    fn matches_multi_modifier_combination() {
        let mut bindings = TestBindings::new();
        bindings.add(Key::A, &[Modifier::Ctrl, Modifier::Shift], Action::Swallow);

        let result = bindings.match_event(
            Key::A,
            KeyTransition::Press,
            &[Modifier::Ctrl, Modifier::Shift],
        );
        assert!(matches!(result, MatchResult::Matched { .. }));
    }

    #[test]
    fn modifier_order_does_not_affect_matching() {
        let mut bindings = TestBindings::new();
        // Register with Shift, Ctrl order
        bindings.add(Key::A, &[Modifier::Shift, Modifier::Ctrl], Action::Swallow);

        // Match with Ctrl, Shift order
        let result = bindings.match_event(
            Key::A,
            KeyTransition::Press,
            &[Modifier::Ctrl, Modifier::Shift],
        );
        assert!(matches!(result, MatchResult::Matched { .. }));
    }

    #[test]
    fn matches_hotkey_with_no_modifiers() {
        let mut bindings = TestBindings::new();
        bindings.add(Key::Escape, &[], Action::Swallow);

        let result = bindings.match_event(Key::Escape, KeyTransition::Press, &[]);
        assert!(matches!(result, MatchResult::Matched { .. }));
    }

    #[test]
    fn ignores_release_events() {
        let mut bindings = TestBindings::new();
        bindings.add(Key::C, &[Modifier::Ctrl], Action::Swallow);

        let result = bindings.match_event(Key::C, KeyTransition::Release, &[Modifier::Ctrl]);
        assert!(matches!(result, MatchResult::Ignored));
    }

    #[test]
    fn ignores_repeat_events() {
        let mut bindings = TestBindings::new();
        bindings.add(Key::C, &[Modifier::Ctrl], Action::Swallow);

        let result = bindings.match_event(Key::C, KeyTransition::Repeat, &[Modifier::Ctrl]);
        assert!(matches!(result, MatchResult::Ignored));
    }

    #[test]
    fn modifier_key_press_does_not_trigger_hotkeys() {
        let mut bindings = TestBindings::new();
        // Even if someone registers LeftCtrl with no modifiers
        bindings.add(Key::LeftCtrl, &[], Action::Swallow);

        let result = bindings.match_event(Key::LeftCtrl, KeyTransition::Press, &[]);
        assert!(matches!(result, MatchResult::Ignored));
    }

    // Layer stack matching tests

    #[test]
    fn layer_binding_matches_when_layer_is_active() {
        let mut bindings = TestBindings::new();
        bindings.add_layer(
            "nav",
            vec![layer_binding(Key::H, &[], Action::Swallow)],
            LayerOptions::default(),
        );
        bindings.push_layer("nav");

        let result = bindings.match_event(Key::H, KeyTransition::Press, &[]);
        assert!(matches!(result, MatchResult::Matched { .. }));
    }

    #[test]
    fn layer_binding_does_not_match_when_layer_is_inactive() {
        let mut bindings = TestBindings::new();
        bindings.add_layer(
            "nav",
            vec![layer_binding(Key::H, &[], Action::Swallow)],
            LayerOptions::default(),
        );
        // Don't push the layer

        let result = bindings.match_event(Key::H, KeyTransition::Press, &[]);
        assert!(matches!(result, MatchResult::NoMatch));
    }

    #[test]
    fn active_layer_takes_priority_over_global_binding() {
        let mut bindings = TestBindings::new();

        // Global binding for H
        let global_counter = Arc::new(AtomicUsize::new(0));
        let gc = Arc::clone(&global_counter);
        bindings.add(
            Key::H,
            &[],
            Action::from(move || {
                gc.fetch_add(1, Ordering::Relaxed);
            }),
        );

        // Layer binding for H
        let layer_counter = Arc::new(AtomicUsize::new(0));
        let lc = Arc::clone(&layer_counter);
        bindings.add_layer(
            "nav",
            vec![layer_binding(
                Key::H,
                &[],
                Action::from(move || {
                    lc.fetch_add(1, Ordering::Relaxed);
                }),
            )],
            LayerOptions::default(),
        );
        bindings.push_layer("nav");

        let result = bindings.match_event(Key::H, KeyTransition::Press, &[]);
        if let MatchResult::Matched {
            action: Action::Callback(cb),
            ..
        } = result
        {
            cb();
        }

        assert_eq!(
            layer_counter.load(Ordering::Relaxed),
            1,
            "layer binding should fire"
        );
        assert_eq!(
            global_counter.load(Ordering::Relaxed),
            0,
            "global binding should not fire"
        );
    }

    #[test]
    fn topmost_layer_has_highest_priority() {
        let mut bindings = TestBindings::new();

        let layer1_counter = Arc::new(AtomicUsize::new(0));
        let l1c = Arc::clone(&layer1_counter);
        bindings.add_layer(
            "layer1",
            vec![layer_binding(
                Key::H,
                &[],
                Action::from(move || {
                    l1c.fetch_add(1, Ordering::Relaxed);
                }),
            )],
            LayerOptions::default(),
        );

        let layer2_counter = Arc::new(AtomicUsize::new(0));
        let l2c = Arc::clone(&layer2_counter);
        bindings.add_layer(
            "layer2",
            vec![layer_binding(
                Key::H,
                &[],
                Action::from(move || {
                    l2c.fetch_add(1, Ordering::Relaxed);
                }),
            )],
            LayerOptions::default(),
        );

        bindings.push_layer("layer1");
        bindings.push_layer("layer2");

        let result = bindings.match_event(Key::H, KeyTransition::Press, &[]);
        if let MatchResult::Matched {
            action: Action::Callback(cb),
            ..
        } = result
        {
            cb();
        }

        assert_eq!(
            layer2_counter.load(Ordering::Relaxed),
            1,
            "topmost layer2 should fire"
        );
        assert_eq!(
            layer1_counter.load(Ordering::Relaxed),
            0,
            "lower layer1 should not fire"
        );
    }

    #[test]
    fn unmatched_key_falls_through_to_lower_layer() {
        let mut bindings = TestBindings::new();

        let layer1_counter = Arc::new(AtomicUsize::new(0));
        let l1c = Arc::clone(&layer1_counter);
        bindings.add_layer(
            "layer1",
            vec![layer_binding(
                Key::J,
                &[],
                Action::from(move || {
                    l1c.fetch_add(1, Ordering::Relaxed);
                }),
            )],
            LayerOptions::default(), // Fallthrough
        );

        bindings.add_layer(
            "layer2",
            vec![layer_binding(Key::H, &[], Action::Swallow)],
            LayerOptions::default(), // Fallthrough
        );

        bindings.push_layer("layer1");
        bindings.push_layer("layer2");

        // J is not in layer2, should fall through to layer1
        let result = bindings.match_event(Key::J, KeyTransition::Press, &[]);
        if let MatchResult::Matched {
            action: Action::Callback(cb),
            ..
        } = result
        {
            cb();
        }

        assert_eq!(layer1_counter.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn unmatched_key_falls_through_to_global() {
        let mut bindings = TestBindings::new();

        let global_counter = Arc::new(AtomicUsize::new(0));
        let gc = Arc::clone(&global_counter);
        bindings.add(
            Key::X,
            &[],
            Action::from(move || {
                gc.fetch_add(1, Ordering::Relaxed);
            }),
        );

        bindings.add_layer(
            "nav",
            vec![layer_binding(Key::H, &[], Action::Swallow)],
            LayerOptions::default(),
        );
        bindings.push_layer("nav");

        // X is not in nav layer, falls through to global
        let result = bindings.match_event(Key::X, KeyTransition::Press, &[]);
        if let MatchResult::Matched {
            action: Action::Callback(cb),
            ..
        } = result
        {
            cb();
        }

        assert_eq!(global_counter.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn swallow_layer_consumes_unmatched_keys() {
        let mut bindings = TestBindings::new();

        // Global binding that should NOT fire
        let global_counter = Arc::new(AtomicUsize::new(0));
        let gc = Arc::clone(&global_counter);
        bindings.add(
            Key::X,
            &[],
            Action::from(move || {
                gc.fetch_add(1, Ordering::Relaxed);
            }),
        );

        bindings.add_layer(
            "modal",
            vec![layer_binding(Key::H, &[], Action::Swallow)],
            LayerOptions::default().with_unmatched(UnmatchedKeyBehavior::Swallow),
        );
        bindings.push_layer("modal");

        // X is not in the swallow layer — should be swallowed, not passed to global
        let result = bindings.match_event(Key::X, KeyTransition::Press, &[]);
        assert!(matches!(result, MatchResult::Swallowed));
        assert_eq!(global_counter.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn same_key_different_layers() {
        let mut bindings = TestBindings::new();

        let base_counter = Arc::new(AtomicUsize::new(0));
        let bc = Arc::clone(&base_counter);
        bindings.add(
            Key::H,
            &[],
            Action::from(move || {
                bc.fetch_add(1, Ordering::Relaxed);
            }),
        );

        let nav_counter = Arc::new(AtomicUsize::new(0));
        let nc = Arc::clone(&nav_counter);
        bindings.add_layer(
            "nav",
            vec![layer_binding(
                Key::H,
                &[],
                Action::from(move || {
                    nc.fetch_add(1, Ordering::Relaxed);
                }),
            )],
            LayerOptions::default(),
        );

        // Without layer active, H hits global
        let result = bindings.match_event(Key::H, KeyTransition::Press, &[]);
        if let MatchResult::Matched {
            action: Action::Callback(cb),
            ..
        } = result
        {
            cb();
        }
        assert_eq!(base_counter.load(Ordering::Relaxed), 1);
        assert_eq!(nav_counter.load(Ordering::Relaxed), 0);

        // With layer active, H hits layer
        bindings.push_layer("nav");
        let result = bindings.match_event(Key::H, KeyTransition::Press, &[]);
        if let MatchResult::Matched {
            action: Action::Callback(cb),
            ..
        } = result
        {
            cb();
        }
        assert_eq!(base_counter.load(Ordering::Relaxed), 1); // unchanged
        assert_eq!(nav_counter.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn layer_binding_with_modifiers() {
        let mut bindings = TestBindings::new();
        bindings.add_layer(
            "nav",
            vec![layer_binding(Key::H, &[Modifier::Ctrl], Action::Swallow)],
            LayerOptions::default(),
        );
        bindings.push_layer("nav");

        // Without Ctrl — no match
        let result = bindings.match_event(Key::H, KeyTransition::Press, &[]);
        assert!(matches!(result, MatchResult::NoMatch));

        // With Ctrl — matches
        let result = bindings.match_event(Key::H, KeyTransition::Press, &[Modifier::Ctrl]);
        assert!(matches!(result, MatchResult::Matched { .. }));
    }
}

#[cfg(test)]
mod matcher_tests {
    use std::sync::Arc;
    use std::sync::atomic::AtomicUsize;
    use std::sync::atomic::Ordering;
    use std::time::Duration;

    use super::*;
    use crate::Key;
    use crate::binding::BindingOptions;
    use crate::binding::OverlayVisibility;
    use crate::introspection::BindingLocation;
    use crate::introspection::ShadowedStatus;
    use crate::key::Modifier;
    use crate::layer::Layer;

    // Registration and basic matching

    #[test]
    fn new_matcher_is_empty() {
        let matcher = Matcher::new();
        assert!(matcher.list_bindings().is_empty());
        assert!(matcher.active_layers().is_empty());
        assert!(matcher.conflicts().is_empty());
    }

    #[test]
    fn register_returns_unique_id() {
        let mut matcher = Matcher::new();
        let id1 = matcher
            .register(Hotkey::new(Key::A), Action::Swallow)
            .unwrap();
        let id2 = matcher
            .register(Hotkey::new(Key::B), Action::Swallow)
            .unwrap();
        assert_ne!(id1, id2);
    }

    #[test]
    fn register_duplicate_hotkey_returns_error() {
        let mut matcher = Matcher::new();
        matcher
            .register(Hotkey::new(Key::A), Action::Swallow)
            .unwrap();
        let result = matcher.register(Hotkey::new(Key::A), Action::Swallow);
        assert!(matches!(result, Err(crate::Error::AlreadyRegistered)));
    }

    #[test]
    fn is_registered_reflects_state() {
        let mut matcher = Matcher::new();
        let hotkey = Hotkey::new(Key::C).modifier(Modifier::Ctrl);
        assert!(!matcher.is_registered(&hotkey));

        matcher.register(hotkey.clone(), Action::Swallow).unwrap();
        assert!(matcher.is_registered(&hotkey));
    }

    #[test]
    fn unregister_removes_binding() {
        let mut matcher = Matcher::new();
        let hotkey = Hotkey::new(Key::A);
        let id = matcher.register(hotkey.clone(), Action::Swallow).unwrap();

        assert!(matcher.is_registered(&hotkey));
        matcher.unregister(id);
        assert!(!matcher.is_registered(&hotkey));
    }

    #[test]
    fn process_matches_registered_hotkey() {
        let mut matcher = Matcher::new();
        let counter = Arc::new(AtomicUsize::new(0));
        let cc = Arc::clone(&counter);

        matcher
            .register(Hotkey::new(Key::C).modifier(Modifier::Ctrl), move || {
                cc.fetch_add(1, Ordering::Relaxed);
            })
            .unwrap();

        let result = matcher.process(
            &Hotkey::new(Key::C).modifier(Modifier::Ctrl),
            KeyTransition::Press,
        );

        assert!(matches!(result, MatchResult::Matched { .. }));
        if let MatchResult::Matched {
            action: Action::Callback(cb),
            ..
        } = result
        {
            cb();
        }
        assert_eq!(counter.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn process_returns_no_match_for_unregistered_hotkey() {
        let mut matcher = Matcher::new();
        matcher
            .register(
                Hotkey::new(Key::C).modifier(Modifier::Ctrl),
                Action::Swallow,
            )
            .unwrap();

        let result = matcher.process(
            &Hotkey::new(Key::V).modifier(Modifier::Ctrl),
            KeyTransition::Press,
        );
        assert!(matches!(result, MatchResult::NoMatch));
    }

    #[test]
    fn process_requires_exact_modifiers() {
        let mut matcher = Matcher::new();
        matcher
            .register(
                Hotkey::new(Key::C).modifier(Modifier::Ctrl),
                Action::Swallow,
            )
            .unwrap();

        // Missing modifier
        let result = matcher.process(&Hotkey::new(Key::C), KeyTransition::Press);
        assert!(matches!(result, MatchResult::NoMatch));

        // Extra modifier
        let result = matcher.process(
            &Hotkey::new(Key::C)
                .modifier(Modifier::Ctrl)
                .modifier(Modifier::Shift),
            KeyTransition::Press,
        );
        assert!(matches!(result, MatchResult::NoMatch));
    }

    #[test]
    fn process_ignores_release_events() {
        let mut matcher = Matcher::new();
        matcher
            .register(Hotkey::new(Key::A), Action::Swallow)
            .unwrap();

        let result = matcher.process(&Hotkey::new(Key::A), KeyTransition::Release);
        assert!(matches!(result, MatchResult::Ignored));
    }

    #[test]
    fn process_ignores_repeat_events() {
        let mut matcher = Matcher::new();
        matcher
            .register(Hotkey::new(Key::A), Action::Swallow)
            .unwrap();

        let result = matcher.process(&Hotkey::new(Key::A), KeyTransition::Repeat);
        assert!(matches!(result, MatchResult::Ignored));
    }

    #[test]
    fn process_ignores_modifier_only_presses() {
        let mut matcher = Matcher::new();
        matcher
            .register(Hotkey::new(Key::LeftCtrl), Action::Swallow)
            .unwrap();

        let result = matcher.process(&Hotkey::new(Key::LeftCtrl), KeyTransition::Press);
        assert!(matches!(result, MatchResult::Ignored));
    }

    #[test]
    fn process_matches_no_modifier_hotkey() {
        let mut matcher = Matcher::new();
        matcher
            .register(Hotkey::new(Key::Escape), Action::Swallow)
            .unwrap();

        let result = matcher.process(&Hotkey::new(Key::Escape), KeyTransition::Press);
        assert!(matches!(result, MatchResult::Matched { .. }));
    }

    // Layer operations

    #[test]
    fn define_and_push_layer_activates_bindings() {
        let mut matcher = Matcher::new();
        let counter = Arc::new(AtomicUsize::new(0));
        let cc = Arc::clone(&counter);

        let layer = Layer::new("nav").bind(
            Key::H,
            Action::from(move || {
                cc.fetch_add(1, Ordering::Relaxed);
            }),
        );
        matcher.define_layer(layer).unwrap();
        matcher.push_layer("nav").unwrap();

        let result = matcher.process(&Hotkey::new(Key::H), KeyTransition::Press);
        if let MatchResult::Matched {
            action: Action::Callback(cb),
            ..
        } = result
        {
            cb();
        }
        assert_eq!(counter.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn pop_layer_deactivates_bindings() {
        let mut matcher = Matcher::new();
        let layer = Layer::new("nav").bind(Key::H, Action::Swallow);
        matcher.define_layer(layer).unwrap();
        matcher.push_layer("nav").unwrap();

        let popped = matcher.pop_layer().unwrap();
        assert_eq!(popped.as_str(), "nav");

        let result = matcher.process(&Hotkey::new(Key::H), KeyTransition::Press);
        assert!(matches!(result, MatchResult::NoMatch));
    }

    #[test]
    fn toggle_layer_pushes_when_not_active() {
        let mut matcher = Matcher::new();
        let layer = Layer::new("nav").bind(Key::H, Action::Swallow);
        matcher.define_layer(layer).unwrap();

        matcher.toggle_layer("nav").unwrap();

        let result = matcher.process(&Hotkey::new(Key::H), KeyTransition::Press);
        assert!(matches!(result, MatchResult::Matched { .. }));
    }

    #[test]
    fn toggle_layer_removes_when_active() {
        let mut matcher = Matcher::new();
        let layer = Layer::new("nav").bind(Key::H, Action::Swallow);
        matcher.define_layer(layer).unwrap();
        matcher.push_layer("nav").unwrap();

        matcher.toggle_layer("nav").unwrap();

        let result = matcher.process(&Hotkey::new(Key::H), KeyTransition::Press);
        assert!(matches!(result, MatchResult::NoMatch));
    }

    #[test]
    fn push_undefined_layer_returns_error() {
        let mut matcher = Matcher::new();
        let result = matcher.push_layer("nonexistent");
        assert!(matches!(result, Err(crate::Error::LayerNotDefined)));
    }

    #[test]
    fn pop_empty_stack_returns_error() {
        let mut matcher = Matcher::new();
        let result = matcher.pop_layer();
        assert!(matches!(result, Err(crate::Error::EmptyLayerStack)));
    }

    #[test]
    fn define_duplicate_layer_returns_error() {
        let mut matcher = Matcher::new();
        matcher
            .define_layer(Layer::new("nav").bind(Key::H, Action::Swallow))
            .unwrap();
        let result = matcher.define_layer(Layer::new("nav").bind(Key::J, Action::Swallow));
        assert!(matches!(result, Err(crate::Error::LayerAlreadyDefined)));
    }

    #[test]
    fn topmost_layer_has_highest_priority() {
        let mut matcher = Matcher::new();
        let layer1_counter = Arc::new(AtomicUsize::new(0));
        let l1c = Arc::clone(&layer1_counter);
        let layer2_counter = Arc::new(AtomicUsize::new(0));
        let l2c = Arc::clone(&layer2_counter);

        matcher
            .define_layer(Layer::new("layer1").bind(
                Key::H,
                Action::from(move || {
                    l1c.fetch_add(1, Ordering::Relaxed);
                }),
            ))
            .unwrap();
        matcher
            .define_layer(Layer::new("layer2").bind(
                Key::H,
                Action::from(move || {
                    l2c.fetch_add(1, Ordering::Relaxed);
                }),
            ))
            .unwrap();
        matcher.push_layer("layer1").unwrap();
        matcher.push_layer("layer2").unwrap();

        let result = matcher.process(&Hotkey::new(Key::H), KeyTransition::Press);
        if let MatchResult::Matched {
            action: Action::Callback(cb),
            ..
        } = result
        {
            cb();
        }
        assert_eq!(layer2_counter.load(Ordering::Relaxed), 1);
        assert_eq!(layer1_counter.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn unmatched_key_falls_through_to_global() {
        let mut matcher = Matcher::new();
        let counter = Arc::new(AtomicUsize::new(0));
        let cc = Arc::clone(&counter);

        matcher
            .register(Hotkey::new(Key::X), move || {
                cc.fetch_add(1, Ordering::Relaxed);
            })
            .unwrap();
        matcher
            .define_layer(Layer::new("nav").bind(Key::H, Action::Swallow))
            .unwrap();
        matcher.push_layer("nav").unwrap();

        let result = matcher.process(&Hotkey::new(Key::X), KeyTransition::Press);
        if let MatchResult::Matched {
            action: Action::Callback(cb),
            ..
        } = result
        {
            cb();
        }
        assert_eq!(counter.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn swallow_layer_consumes_unmatched_keys() {
        let mut matcher = Matcher::new();
        matcher
            .register(Hotkey::new(Key::X), Action::Swallow)
            .unwrap();
        matcher
            .define_layer(Layer::new("modal").bind(Key::H, Action::Swallow).swallow())
            .unwrap();
        matcher.push_layer("modal").unwrap();

        let result = matcher.process(&Hotkey::new(Key::X), KeyTransition::Press);
        assert!(matches!(result, MatchResult::Swallowed));
    }

    // Layer actions applied internally by process()

    #[test]
    fn process_applies_push_layer_action() {
        let mut matcher = Matcher::new();
        let counter = Arc::new(AtomicUsize::new(0));
        let cc = Arc::clone(&counter);

        matcher
            .define_layer(Layer::new("nav").bind(
                Key::H,
                Action::from(move || {
                    cc.fetch_add(1, Ordering::Relaxed);
                }),
            ))
            .unwrap();
        matcher
            .register(
                Hotkey::new(Key::F1),
                Action::PushLayer(LayerName::from("nav")),
            )
            .unwrap();

        // Press F1 → pushes nav layer
        matcher.process(&Hotkey::new(Key::F1), KeyTransition::Press);

        // Now H should match in nav layer
        let result = matcher.process(&Hotkey::new(Key::H), KeyTransition::Press);
        if let MatchResult::Matched {
            action: Action::Callback(cb),
            ..
        } = result
        {
            cb();
        }
        assert_eq!(counter.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn process_applies_pop_layer_action() {
        let mut matcher = Matcher::new();

        let layer = Layer::new("nav")
            .bind(Key::H, Action::Swallow)
            .bind(Key::Escape, Action::PopLayer);
        matcher.define_layer(layer).unwrap();
        matcher.push_layer("nav").unwrap();

        // Escape pops the layer
        matcher.process(&Hotkey::new(Key::Escape), KeyTransition::Press);

        // H should no longer match
        let result = matcher.process(&Hotkey::new(Key::H), KeyTransition::Press);
        assert!(matches!(result, MatchResult::NoMatch));
    }

    #[test]
    fn process_applies_toggle_layer_action() {
        let mut matcher = Matcher::new();

        matcher
            .define_layer(Layer::new("nav").bind(Key::H, Action::Swallow))
            .unwrap();
        matcher
            .register(
                Hotkey::new(Key::F2),
                Action::ToggleLayer(LayerName::from("nav")),
            )
            .unwrap();

        // Toggle on
        matcher.process(&Hotkey::new(Key::F2), KeyTransition::Press);
        let result = matcher.process(&Hotkey::new(Key::H), KeyTransition::Press);
        assert!(matches!(result, MatchResult::Matched { .. }));

        // Toggle off
        matcher.process(&Hotkey::new(Key::F2), KeyTransition::Press);
        let result = matcher.process(&Hotkey::new(Key::H), KeyTransition::Press);
        assert!(matches!(result, MatchResult::NoMatch));
    }

    // Oneshot layers

    #[test]
    fn oneshot_layer_pops_after_n_keypresses() {
        let mut matcher = Matcher::new();
        let counter = Arc::new(AtomicUsize::new(0));
        let cc = Arc::clone(&counter);

        let layer = Layer::new("oneshot")
            .bind(
                Key::H,
                Action::from(move || {
                    cc.fetch_add(1, Ordering::Relaxed);
                }),
            )
            .oneshot(1);
        matcher.define_layer(layer).unwrap();
        matcher.push_layer("oneshot").unwrap();

        // First press → matches and auto-pops
        let result = matcher.process(&Hotkey::new(Key::H), KeyTransition::Press);
        if let MatchResult::Matched {
            action: Action::Callback(cb),
            ..
        } = result
        {
            cb();
        }
        assert_eq!(counter.load(Ordering::Relaxed), 1);

        // Second press → layer gone
        let result = matcher.process(&Hotkey::new(Key::H), KeyTransition::Press);
        assert!(matches!(result, MatchResult::NoMatch));
    }

    // Timeout layers

    #[test]
    fn timeout_layer_pops_after_inactivity() {
        let mut matcher = Matcher::new();
        let layer = Layer::new("timed")
            .bind(Key::H, Action::Swallow)
            .timeout(Duration::from_millis(50));
        matcher.define_layer(layer).unwrap();
        matcher.push_layer("timed").unwrap();

        // H matches while active
        let result = matcher.process(&Hotkey::new(Key::H), KeyTransition::Press);
        assert!(matches!(result, MatchResult::Matched { .. }));

        // Wait for timeout
        std::thread::sleep(Duration::from_millis(80));
        matcher.check_timeouts();

        // H should no longer match
        let result = matcher.process(&Hotkey::new(Key::H), KeyTransition::Press);
        assert!(matches!(result, MatchResult::NoMatch));
    }

    // Introspection

    #[test]
    fn list_bindings_returns_global_bindings() {
        let mut matcher = Matcher::new();
        matcher
            .register_binding(
                RegisteredBinding::new(
                    BindingId::new(),
                    Hotkey::new(Key::C).modifier(Modifier::Ctrl),
                    Action::Swallow,
                )
                .with_options(BindingOptions::default().with_description("Copy")),
            )
            .unwrap();

        let bindings = matcher.list_bindings();
        assert_eq!(bindings.len(), 1);
        assert_eq!(bindings[0].description.as_deref(), Some("Copy"));
        assert_eq!(bindings[0].location, BindingLocation::Global);
        assert_eq!(bindings[0].shadowed, ShadowedStatus::Active);
    }

    #[test]
    fn list_bindings_includes_layer_bindings() {
        let mut matcher = Matcher::new();
        matcher
            .define_layer(
                Layer::new("nav")
                    .bind(Key::H, Action::Swallow)
                    .bind(Key::J, Action::Swallow),
            )
            .unwrap();

        let bindings = matcher.list_bindings();
        let layer_bindings: Vec<_> = bindings
            .iter()
            .filter(|b| matches!(b.location, BindingLocation::Layer(_)))
            .collect();
        assert_eq!(layer_bindings.len(), 2);
    }

    #[test]
    fn list_bindings_detects_shadowed_global() {
        let mut matcher = Matcher::new();
        matcher
            .register(Hotkey::new(Key::H), Action::Swallow)
            .unwrap();
        matcher
            .define_layer(Layer::new("nav").bind(Key::H, Action::Swallow))
            .unwrap();
        matcher.push_layer("nav").unwrap();

        let bindings = matcher.list_bindings();
        let global_h = bindings
            .iter()
            .find(|b| b.hotkey == Hotkey::new(Key::H) && b.location == BindingLocation::Global)
            .expect("should find global H");
        assert_eq!(
            global_h.shadowed,
            ShadowedStatus::ShadowedBy(LayerName::from("nav"))
        );
    }

    #[test]
    fn bindings_for_key_returns_matching_binding() {
        let mut matcher = Matcher::new();
        matcher
            .register_binding(
                RegisteredBinding::new(
                    BindingId::new(),
                    Hotkey::new(Key::C).modifier(Modifier::Ctrl),
                    Action::Swallow,
                )
                .with_options(BindingOptions::default().with_description("Copy")),
            )
            .unwrap();

        let result = matcher.bindings_for_key(&Hotkey::new(Key::C).modifier(Modifier::Ctrl));
        assert!(result.is_some());
        assert_eq!(result.unwrap().description.as_deref(), Some("Copy"));
    }

    #[test]
    fn bindings_for_key_returns_none_when_no_match() {
        let mut matcher = Matcher::new();
        matcher
            .register(
                Hotkey::new(Key::C).modifier(Modifier::Ctrl),
                Action::Swallow,
            )
            .unwrap();

        let result = matcher.bindings_for_key(&Hotkey::new(Key::V).modifier(Modifier::Ctrl));
        assert!(result.is_none());
    }

    #[test]
    fn bindings_for_key_respects_layer_stack() {
        let mut matcher = Matcher::new();
        matcher
            .register(Hotkey::new(Key::H), Action::Swallow)
            .unwrap();
        matcher
            .define_layer(Layer::new("nav").bind(Key::H, Action::Swallow))
            .unwrap();
        matcher.push_layer("nav").unwrap();

        let result = matcher.bindings_for_key(&Hotkey::new(Key::H));
        assert!(result.is_some());
        assert_eq!(
            result.unwrap().location,
            BindingLocation::Layer(LayerName::from("nav"))
        );
    }

    #[test]
    fn bindings_for_key_respects_swallow_layer() {
        let mut matcher = Matcher::new();
        matcher
            .register(Hotkey::new(Key::X), Action::Swallow)
            .unwrap();
        matcher
            .define_layer(Layer::new("modal").bind(Key::H, Action::Swallow).swallow())
            .unwrap();
        matcher.push_layer("modal").unwrap();

        // X not in swallow layer → blocked from reaching global
        let result = matcher.bindings_for_key(&Hotkey::new(Key::X));
        assert!(result.is_none());
    }

    #[test]
    fn bindings_for_key_returns_none_for_modifier_key() {
        let mut matcher = Matcher::new();
        matcher
            .register(Hotkey::new(Key::LeftCtrl), Action::Swallow)
            .unwrap();

        let result = matcher.bindings_for_key(&Hotkey::new(Key::LeftCtrl));
        assert!(result.is_none());
    }

    #[test]
    fn active_layers_reflects_stack() {
        let mut matcher = Matcher::new();
        matcher
            .define_layer(
                Layer::new("layer1")
                    .bind(Key::H, Action::Swallow)
                    .description("First"),
            )
            .unwrap();
        matcher
            .define_layer(
                Layer::new("layer2")
                    .bind(Key::J, Action::Swallow)
                    .bind(Key::K, Action::Swallow)
                    .description("Second"),
            )
            .unwrap();
        matcher.push_layer("layer1").unwrap();
        matcher.push_layer("layer2").unwrap();

        let active = matcher.active_layers();
        assert_eq!(active.len(), 2);
        assert_eq!(active[0].name.as_str(), "layer1");
        assert_eq!(active[0].description.as_deref(), Some("First"));
        assert_eq!(active[0].binding_count, 1);
        assert_eq!(active[1].name.as_str(), "layer2");
        assert_eq!(active[1].description.as_deref(), Some("Second"));
        assert_eq!(active[1].binding_count, 2);
    }

    #[test]
    fn conflicts_detects_layer_shadowing_global() {
        let mut matcher = Matcher::new();
        matcher
            .register(Hotkey::new(Key::H), Action::Swallow)
            .unwrap();
        matcher
            .define_layer(Layer::new("nav").bind(Key::H, Action::Swallow))
            .unwrap();
        matcher.push_layer("nav").unwrap();

        let conflicts = matcher.conflicts();
        assert_eq!(conflicts.len(), 1);
        assert_eq!(conflicts[0].hotkey, Hotkey::new(Key::H));
        assert_eq!(
            conflicts[0].shadowed_binding.location,
            BindingLocation::Global
        );
        assert_eq!(
            conflicts[0].shadowing_binding.location,
            BindingLocation::Layer(LayerName::from("nav"))
        );
    }

    #[test]
    fn conflicts_empty_when_no_overlaps() {
        let mut matcher = Matcher::new();
        matcher
            .register(
                Hotkey::new(Key::C).modifier(Modifier::Ctrl),
                Action::Swallow,
            )
            .unwrap();
        assert!(matcher.conflicts().is_empty());
    }

    // MatchResult::Pending exists

    #[test]
    fn pending_variant_exists() {
        let result: MatchResult<'_> = MatchResult::Pending {
            steps_matched: 1,
            steps_remaining: 2,
        };
        assert!(matches!(
            result,
            MatchResult::Pending {
                steps_matched: 1,
                steps_remaining: 2
            }
        ));
    }

    // Overlay visibility preserved through introspection

    #[test]
    fn list_bindings_preserves_overlay_visibility() {
        let mut matcher = Matcher::new();
        matcher
            .register_binding(
                RegisteredBinding::new(
                    BindingId::new(),
                    Hotkey::new(Key::C).modifier(Modifier::Ctrl),
                    Action::Swallow,
                )
                .with_options(
                    BindingOptions::default().with_overlay_visibility(OverlayVisibility::Hidden),
                ),
            )
            .unwrap();

        let bindings = matcher.list_bindings();
        assert_eq!(bindings[0].overlay_visibility, OverlayVisibility::Hidden);
    }

    // Standalone usage without any engine thread

    #[test]
    fn standalone_matcher_full_workflow() {
        let mut matcher = Matcher::new();

        // Register global bindings
        let counter = Arc::new(AtomicUsize::new(0));
        let cc = Arc::clone(&counter);
        matcher
            .register(Hotkey::new(Key::C).modifier(Modifier::Ctrl), move || {
                cc.fetch_add(1, Ordering::Relaxed);
            })
            .unwrap();

        // Define and push a layer
        matcher
            .define_layer(
                Layer::new("nav")
                    .bind(Key::H, Action::Swallow)
                    .bind(Key::Escape, Action::PopLayer),
            )
            .unwrap();
        matcher.push_layer("nav").unwrap();

        // Layer binding matches
        let result = matcher.process(&Hotkey::new(Key::H), KeyTransition::Press);
        assert!(matches!(result, MatchResult::Matched { .. }));

        // Global binding falls through
        let result = matcher.process(
            &Hotkey::new(Key::C).modifier(Modifier::Ctrl),
            KeyTransition::Press,
        );
        if let MatchResult::Matched {
            action: Action::Callback(cb),
            ..
        } = result
        {
            cb();
        }
        assert_eq!(counter.load(Ordering::Relaxed), 1);

        // Pop layer via action
        matcher.process(&Hotkey::new(Key::Escape), KeyTransition::Press);

        // Layer binding no longer matches
        let result = matcher.process(&Hotkey::new(Key::H), KeyTransition::Press);
        assert!(matches!(result, MatchResult::NoMatch));

        // Introspection works
        assert!(matcher.active_layers().is_empty());
        assert!(!matcher.list_bindings().is_empty());
    }
}
