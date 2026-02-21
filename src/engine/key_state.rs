//! Key state tracking — single source of truth for what's pressed.
//!
//! Modifier state is derived from key state, not tracked separately.
//! "Is Ctrl held?" = "is `KEY_LEFTCTRL` or `KEY_RIGHTCTRL` in the pressed set?"
//!
//! Replaces v0's separate `ModifierTracker` (per-device `HashSet<Modifier>`)
//! and `SharedKeyState` (atomic counters). The engine owns this exclusively —
//! no Arc, no atomics, no shared access.
//!
//! # Reference
//!
//! Prior art: `archive/v0/src/key_state.rs`,
//! `archive/v0/src/listener/device.rs` (`ModifierTracker`)

use std::collections::HashMap;
use std::collections::HashSet;
use std::os::fd::RawFd;

use crate::Key;

// TODO: active_modifiers() — derived from pressed keys, not parallel state

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum KeyTransition {
    Press,
    Release,
    Repeat,
}

#[derive(Debug, Default)]
pub(crate) struct KeyState {
    pressed_by_device: HashMap<RawFd, HashSet<Key>>,
}

impl KeyState {
    pub(crate) fn apply_device_event(
        &mut self,
        device_fd: RawFd,
        key: Key,
        transition: KeyTransition,
    ) {
        match transition {
            KeyTransition::Press | KeyTransition::Repeat => {
                self.pressed_by_device
                    .entry(device_fd)
                    .or_default()
                    .insert(key);
            }
            KeyTransition::Release => {
                if let Some(pressed) = self.pressed_by_device.get_mut(&device_fd) {
                    pressed.remove(&key);
                    if pressed.is_empty() {
                        self.pressed_by_device.remove(&device_fd);
                    }
                }
            }
        }
    }

    pub(crate) fn disconnect_device(&mut self, device_fd: RawFd) {
        self.pressed_by_device.remove(&device_fd);
    }

    #[must_use]
    pub(crate) fn is_pressed(&self, key: Key) -> bool {
        self.pressed_by_device
            .values()
            .any(|pressed| pressed.contains(&key))
    }
}

#[cfg(test)]
mod tests {
    use super::KeyState;
    use super::KeyTransition;
    use crate::Key;

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

        key_state.apply_device_event(10, Key::LeftCtrl, KeyTransition::Press);
        key_state.apply_device_event(11, Key::C, KeyTransition::Press);

        key_state.disconnect_device(10);

        assert!(!key_state.is_pressed(Key::LeftCtrl));
        assert!(key_state.is_pressed(Key::C));
    }
}
