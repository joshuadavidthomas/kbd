//! Binding matcher — finds which binding (if any) matches a key event.
//!
//! Walks the layer stack top-down, checking bindings in each active layer,
//! then global bindings. Within each layer, speculative patterns (tap-hold,
//! sequences) are checked before immediate patterns (hotkeys).
//!
//! Returns the matched binding's action (or "no match" for forwarding).
//!
//! # Reference
//!
//! Prior art: `archive/v0/src/listener/dispatch.rs` (scattered across
//! `collect_non_modifier_dispatch`, `collect_device_specific_dispatch`,
//! `dispatch_mode_key_event`). This module unifies all matching into one path.

use std::collections::HashMap;

use crate::action::Action;
use crate::action::LayerName;
use crate::binding::BindingId;
use crate::binding::Passthrough;
use crate::engine::key_state::KeyTransition;
use crate::engine::LayerStackEntry;
use crate::engine::RegisteredBinding;
use crate::engine::StoredLayer;
use crate::key::Hotkey;
use crate::key::Modifier;
use crate::layer::UnmatchedKeyBehavior;
use crate::Key;

/// Result of attempting to match a key event against registered bindings.
#[derive(Debug)]
pub(crate) enum MatchResult<'a> {
    /// A binding matched. Contains the action and passthrough setting.
    Matched {
        action: &'a Action,
        passthrough: Passthrough,
    },
    /// No binding matched the event.
    NoMatch,
    /// The event was swallowed by a layer with `UnmatchedKeyBehavior::Swallow`.
    Swallowed,
    /// The event was not eligible for matching (modifier-only press, release, repeat).
    Ignored,
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
/// Only key press events trigger matching — release and repeat events
/// are ignored at this phase (press cache for releases comes in Phase 3.3).
pub(crate) fn match_key_event<'a>(
    key: Key,
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
    if Modifier::from_key(key).is_some() {
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
            if matches!(stored_layer.options.unmatched, UnmatchedKeyBehavior::Swallow) {
                return MatchResult::Swallowed;
            }
        }
    }

    // Fall through to global bindings
    if let Some(&id) = binding_ids_by_hotkey.get(candidate) {
        if let Some(binding) = bindings_by_id.get(&id) {
            return MatchResult::Matched {
                action: binding.action(),
                passthrough: binding.passthrough(),
            };
        }
    }

    MatchResult::NoMatch
}

// TODO: Speculative vs immediate pattern ordering
// TODO: Device filter checking (per-binding, not per-dispatch-phase)

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::atomic::AtomicUsize;
    use std::sync::atomic::Ordering;
    use std::sync::Arc;

    use super::match_key_event;
    use super::MatchResult;
    use crate::action::Action;
    use crate::action::LayerName;
    use crate::binding::BindingId;
    use crate::binding::Passthrough;
    use crate::engine::key_state::KeyTransition;
    use crate::engine::LayerStackEntry;
    use crate::engine::RegisteredBinding;
    use crate::engine::StoredLayer;
    use crate::key::Hotkey;
    use crate::key::Modifier;
    use crate::layer::LayerBinding;
    use crate::layer::LayerOptions;
    use crate::layer::UnmatchedKeyBehavior;
    use crate::Key;

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
            self.layers.insert(
                LayerName::from(name),
                StoredLayer { bindings, options },
            );
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
                key,
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
        bindings.add(Key::H, &[], Action::from(move || { gc.fetch_add(1, Ordering::Relaxed); }));

        // Layer binding for H
        let layer_counter = Arc::new(AtomicUsize::new(0));
        let lc = Arc::clone(&layer_counter);
        bindings.add_layer(
            "nav",
            vec![layer_binding(Key::H, &[], Action::from(move || { lc.fetch_add(1, Ordering::Relaxed); }))],
            LayerOptions::default(),
        );
        bindings.push_layer("nav");

        let result = bindings.match_event(Key::H, KeyTransition::Press, &[]);
        if let MatchResult::Matched { action: Action::Callback(cb), .. } = result {
            cb();
        }

        assert_eq!(layer_counter.load(Ordering::Relaxed), 1, "layer binding should fire");
        assert_eq!(global_counter.load(Ordering::Relaxed), 0, "global binding should not fire");
    }

    #[test]
    fn topmost_layer_has_highest_priority() {
        let mut bindings = TestBindings::new();

        let layer1_counter = Arc::new(AtomicUsize::new(0));
        let l1c = Arc::clone(&layer1_counter);
        bindings.add_layer(
            "layer1",
            vec![layer_binding(Key::H, &[], Action::from(move || { l1c.fetch_add(1, Ordering::Relaxed); }))],
            LayerOptions::default(),
        );

        let layer2_counter = Arc::new(AtomicUsize::new(0));
        let l2c = Arc::clone(&layer2_counter);
        bindings.add_layer(
            "layer2",
            vec![layer_binding(Key::H, &[], Action::from(move || { l2c.fetch_add(1, Ordering::Relaxed); }))],
            LayerOptions::default(),
        );

        bindings.push_layer("layer1");
        bindings.push_layer("layer2");

        let result = bindings.match_event(Key::H, KeyTransition::Press, &[]);
        if let MatchResult::Matched { action: Action::Callback(cb), .. } = result {
            cb();
        }

        assert_eq!(layer2_counter.load(Ordering::Relaxed), 1, "topmost layer2 should fire");
        assert_eq!(layer1_counter.load(Ordering::Relaxed), 0, "lower layer1 should not fire");
    }

    #[test]
    fn unmatched_key_falls_through_to_lower_layer() {
        let mut bindings = TestBindings::new();

        let layer1_counter = Arc::new(AtomicUsize::new(0));
        let l1c = Arc::clone(&layer1_counter);
        bindings.add_layer(
            "layer1",
            vec![layer_binding(Key::J, &[], Action::from(move || { l1c.fetch_add(1, Ordering::Relaxed); }))],
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
        if let MatchResult::Matched { action: Action::Callback(cb), .. } = result {
            cb();
        }

        assert_eq!(layer1_counter.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn unmatched_key_falls_through_to_global() {
        let mut bindings = TestBindings::new();

        let global_counter = Arc::new(AtomicUsize::new(0));
        let gc = Arc::clone(&global_counter);
        bindings.add(Key::X, &[], Action::from(move || { gc.fetch_add(1, Ordering::Relaxed); }));

        bindings.add_layer(
            "nav",
            vec![layer_binding(Key::H, &[], Action::Swallow)],
            LayerOptions::default(),
        );
        bindings.push_layer("nav");

        // X is not in nav layer, falls through to global
        let result = bindings.match_event(Key::X, KeyTransition::Press, &[]);
        if let MatchResult::Matched { action: Action::Callback(cb), .. } = result {
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
        bindings.add(Key::X, &[], Action::from(move || { gc.fetch_add(1, Ordering::Relaxed); }));

        bindings.add_layer(
            "modal",
            vec![layer_binding(Key::H, &[], Action::Swallow)],
            LayerOptions {
                unmatched: UnmatchedKeyBehavior::Swallow,
                ..Default::default()
            },
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
        bindings.add(Key::H, &[], Action::from(move || { bc.fetch_add(1, Ordering::Relaxed); }));

        let nav_counter = Arc::new(AtomicUsize::new(0));
        let nc = Arc::clone(&nav_counter);
        bindings.add_layer(
            "nav",
            vec![layer_binding(Key::H, &[], Action::from(move || { nc.fetch_add(1, Ordering::Relaxed); }))],
            LayerOptions::default(),
        );

        // Without layer active, H hits global
        let result = bindings.match_event(Key::H, KeyTransition::Press, &[]);
        if let MatchResult::Matched { action: Action::Callback(cb), .. } = result {
            cb();
        }
        assert_eq!(base_counter.load(Ordering::Relaxed), 1);
        assert_eq!(nav_counter.load(Ordering::Relaxed), 0);

        // With layer active, H hits layer
        bindings.push_layer("nav");
        let result = bindings.match_event(Key::H, KeyTransition::Press, &[]);
        if let MatchResult::Matched { action: Action::Callback(cb), .. } = result {
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
