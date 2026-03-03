//! Key state tracking — single source of truth for what's pressed.
//!
//! Modifier state is derived from key state, not tracked separately.
//! "Is Ctrl held?" = "is `LeftCtrl` or `RightCtrl` in the pressed set?"
//!
//! Device identifiers are plain `i32` values — platform-independent and
//! directly compatible with Unix file descriptors. The engine owns this
//! exclusively — no Arc, no atomics, no shared access.

use std::collections::HashMap;
use std::collections::HashSet;

use crate::hotkey::Modifier;
use crate::key::Key;

/// Whether a key was pressed, released, or repeated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum KeyTransition {
    /// The key was pressed down.
    Press,
    /// The key was released.
    Release,
    /// The key is being held and auto-repeating.
    Repeat,
}

/// Tracks which keys are currently pressed, per device.
///
/// Modifier state is derived from key state, not tracked separately.
/// "Is Ctrl held?" = "is `ControlLeft` or `ControlRight` in the pressed set?"
#[derive(Debug, Default)]
pub struct KeyState {
    pressed_by_device: HashMap<i32, HashSet<Key>>,
}

impl KeyState {
    /// Update key state for a device event.
    ///
    /// Press and repeat events mark the key as pressed; release events
    /// remove it. Device entries are cleaned up when their last key is
    /// released.
    pub fn apply_device_event(&mut self, device_id: i32, key: Key, transition: KeyTransition) {
        match transition {
            KeyTransition::Press | KeyTransition::Repeat => {
                self.pressed_by_device
                    .entry(device_id)
                    .or_default()
                    .insert(key);
            }
            KeyTransition::Release => {
                if let Some(pressed) = self.pressed_by_device.get_mut(&device_id) {
                    pressed.remove(&key);
                    if pressed.is_empty() {
                        self.pressed_by_device.remove(&device_id);
                    }
                }
            }
        }
    }

    /// Remove all key state for a disconnected device.
    pub fn disconnect_device(&mut self, device_id: i32) {
        self.pressed_by_device.remove(&device_id);
    }

    /// Check whether a key is currently pressed on any device.
    #[must_use]
    pub fn is_pressed(&self, key: Key) -> bool {
        self.pressed_by_device
            .values()
            .any(|pressed| pressed.contains(&key))
    }

    /// Returns the set of modifiers currently held, derived from pressed keys.
    ///
    /// Left/right variants are canonicalized: if either `LeftCtrl` or `RightCtrl`
    /// is held, `Modifier::Ctrl` is in the returned set.
    ///
    /// Aggregates across all devices.
    #[must_use]
    pub fn active_modifiers(&self) -> Vec<Modifier> {
        Self::modifiers_from_pressed(|key| self.is_pressed(key))
    }

    /// Check whether a specific key is pressed on a specific device.
    #[must_use]
    pub fn is_pressed_on_device(&self, device_id: i32, key: Key) -> bool {
        self.pressed_by_device
            .get(&device_id)
            .is_some_and(|pressed| pressed.contains(&key))
    }

    /// Returns the set of modifiers currently held on a specific device.
    ///
    /// Same canonicalization as [`active_modifiers`](Self::active_modifiers),
    /// but scoped to a single device.
    #[must_use]
    pub fn active_modifiers_for_device(&self, device_id: i32) -> Vec<Modifier> {
        Self::modifiers_from_pressed(|key| self.is_pressed_on_device(device_id, key))
    }

    /// Shared implementation for deriving active modifiers from a key-pressed predicate.
    fn modifiers_from_pressed(is_pressed: impl Fn(Key) -> bool) -> Vec<Modifier> {
        let mut modifiers = Vec::new();

        for &modifier in &[
            Modifier::Ctrl,
            Modifier::Shift,
            Modifier::Alt,
            Modifier::Super,
        ] {
            let (left, right) = modifier.keys();
            if is_pressed(left) || is_pressed(right) {
                modifiers.push(modifier);
            }
        }

        modifiers
    }
}

#[cfg(test)]
mod tests {
    use super::KeyState;
    use super::KeyTransition;
    use crate::hotkey::Modifier;
    use crate::key::Key;

    #[test]
    fn pressed_keys_are_tracked_per_device() {
        let mut key_state = KeyState::default();

        key_state.apply_device_event(10, Key::A, KeyTransition::Press);
        assert!(key_state.is_pressed(Key::A));

        key_state.apply_device_event(10, Key::A, KeyTransition::Release);
        assert!(!key_state.is_pressed(Key::A));
    }

    #[test]
    fn repeat_events_keep_key_pressed() {
        let mut key_state = KeyState::default();

        key_state.apply_device_event(10, Key::B, KeyTransition::Press);
        key_state.apply_device_event(10, Key::B, KeyTransition::Repeat);
        assert!(key_state.is_pressed(Key::B));

        key_state.apply_device_event(10, Key::B, KeyTransition::Release);
        assert!(!key_state.is_pressed(Key::B));
    }

    #[test]
    fn disconnect_clears_pressed_keys_for_removed_device() {
        let mut key_state = KeyState::default();

        key_state.apply_device_event(10, Key::CONTROL_LEFT, KeyTransition::Press);
        key_state.apply_device_event(11, Key::C, KeyTransition::Press);

        key_state.disconnect_device(10);

        assert!(!key_state.is_pressed(Key::CONTROL_LEFT));
        assert!(key_state.is_pressed(Key::C));
    }

    #[test]
    fn active_modifiers_derived_from_pressed_keys() {
        let mut key_state = KeyState::default();

        assert!(key_state.active_modifiers().is_empty());

        key_state.apply_device_event(10, Key::CONTROL_LEFT, KeyTransition::Press);
        assert_eq!(key_state.active_modifiers(), vec![Modifier::Ctrl]);

        key_state.apply_device_event(10, Key::SHIFT_LEFT, KeyTransition::Press);
        assert_eq!(
            key_state.active_modifiers(),
            vec![Modifier::Ctrl, Modifier::Shift]
        );

        key_state.apply_device_event(10, Key::CONTROL_LEFT, KeyTransition::Release);
        assert_eq!(key_state.active_modifiers(), vec![Modifier::Shift]);
    }

    #[test]
    fn active_modifiers_canonicalizes_left_and_right() {
        let mut key_state = KeyState::default();

        key_state.apply_device_event(10, Key::CONTROL_RIGHT, KeyTransition::Press);
        assert_eq!(key_state.active_modifiers(), vec![Modifier::Ctrl]);

        // Both left and right held still yields one modifier
        key_state.apply_device_event(10, Key::CONTROL_LEFT, KeyTransition::Press);
        assert_eq!(key_state.active_modifiers(), vec![Modifier::Ctrl]);

        // Releasing one side keeps the modifier active
        key_state.apply_device_event(10, Key::CONTROL_RIGHT, KeyTransition::Release);
        assert_eq!(key_state.active_modifiers(), vec![Modifier::Ctrl]);
    }

    #[test]
    fn active_modifiers_spans_devices() {
        let mut key_state = KeyState::default();

        key_state.apply_device_event(10, Key::CONTROL_LEFT, KeyTransition::Press);
        key_state.apply_device_event(11, Key::SHIFT_RIGHT, KeyTransition::Press);

        assert_eq!(
            key_state.active_modifiers(),
            vec![Modifier::Ctrl, Modifier::Shift]
        );
    }

    #[test]
    fn is_pressed_on_device_tracks_per_device() {
        let mut key_state = KeyState::default();

        key_state.apply_device_event(10, Key::A, KeyTransition::Press);
        key_state.apply_device_event(11, Key::B, KeyTransition::Press);

        assert!(key_state.is_pressed_on_device(10, Key::A));
        assert!(!key_state.is_pressed_on_device(10, Key::B));
        assert!(key_state.is_pressed_on_device(11, Key::B));
        assert!(!key_state.is_pressed_on_device(11, Key::A));
    }

    #[test]
    fn is_pressed_on_device_returns_false_for_unknown_device() {
        let key_state = KeyState::default();
        assert!(!key_state.is_pressed_on_device(99, Key::A));
    }

    #[test]
    fn active_modifiers_for_device_isolates_per_device() {
        let mut key_state = KeyState::default();

        key_state.apply_device_event(10, Key::CONTROL_LEFT, KeyTransition::Press);
        key_state.apply_device_event(11, Key::SHIFT_LEFT, KeyTransition::Press);

        assert_eq!(
            key_state.active_modifiers_for_device(10),
            vec![Modifier::Ctrl]
        );
        assert_eq!(
            key_state.active_modifiers_for_device(11),
            vec![Modifier::Shift]
        );
        assert!(key_state.active_modifiers_for_device(99).is_empty());
    }

    #[test]
    fn disconnect_clears_modifiers_for_device() {
        let mut key_state = KeyState::default();

        key_state.apply_device_event(10, Key::CONTROL_LEFT, KeyTransition::Press);
        key_state.apply_device_event(10, Key::SHIFT_LEFT, KeyTransition::Press);
        key_state.apply_device_event(11, Key::ALT_LEFT, KeyTransition::Press);

        key_state.disconnect_device(10);

        // Global modifiers should only reflect device 11
        assert_eq!(key_state.active_modifiers(), vec![Modifier::Alt]);
        // Device 10's modifiers are gone
        assert!(key_state.active_modifiers_for_device(10).is_empty());
        // Device 11's modifiers are intact
        assert_eq!(
            key_state.active_modifiers_for_device(11),
            vec![Modifier::Alt]
        );
    }
}
