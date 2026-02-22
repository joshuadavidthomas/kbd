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
use crate::binding::BindingId;
use crate::binding::Passthrough;
use crate::engine::key_state::KeyTransition;
use crate::engine::RegisteredBinding;
use crate::key::Hotkey;
use crate::key::Modifier;
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
    /// The event was not eligible for matching (modifier-only press, release, repeat).
    Ignored,
}

/// Attempt to find a binding that matches the given key event.
///
/// Only key press events trigger matching — release and repeat events
/// are ignored at this phase (press cache for releases comes in Phase 3).
///
/// Matching checks the trigger key against each registered hotkey, and
/// compares the hotkey's required modifiers against the currently active
/// modifiers. A match requires an exact modifier set match: extra held
/// modifiers cause a miss.
pub(crate) fn match_key_event<'a>(
    key: Key,
    transition: KeyTransition,
    active_modifiers: &[Modifier],
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

    // Build candidate hotkey from the pressed key + active modifiers
    let candidate = Hotkey::new(key, active_modifiers.to_vec());

    if let Some(&id) = binding_ids_by_hotkey.get(&candidate) {
        if let Some(binding) = bindings_by_id.get(&id) {
            return MatchResult::Matched {
                action: binding.action(),
                passthrough: binding.passthrough(),
            };
        }
    }

    MatchResult::NoMatch
}

// TODO: Layer stack traversal with priority
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
    use crate::binding::BindingId;
    use crate::engine::key_state::KeyTransition;
    use crate::engine::RegisteredBinding;
    use crate::key::Hotkey;
    use crate::key::Modifier;
    use crate::Key;

    struct TestBindings {
        bindings_by_id: HashMap<BindingId, RegisteredBinding>,
        binding_ids_by_hotkey: HashMap<Hotkey, BindingId>,
    }

    impl TestBindings {
        fn new() -> Self {
            Self {
                bindings_by_id: HashMap::new(),
                binding_ids_by_hotkey: HashMap::new(),
            }
        }

        fn add(&mut self, key: Key, modifiers: &[Modifier], action: Action) -> BindingId {
            let id = BindingId::new();
            let hotkey = Hotkey::new(key, modifiers.to_vec());
            self.binding_ids_by_hotkey.insert(hotkey.clone(), id);
            self.bindings_by_id
                .insert(id, RegisteredBinding::new(id, hotkey, action));
            id
        }

        fn match_event(
            &self,
            key: Key,
            transition: KeyTransition,
            active_modifiers: &[Modifier],
        ) -> MatchResult<'_> {
            match_key_event(
                key,
                transition,
                active_modifiers,
                &self.binding_ids_by_hotkey,
                &self.bindings_by_id,
            )
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
}
