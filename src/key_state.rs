use evdev::KeyCode;
use std::collections::HashSet;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

const TRACKED_KEY_CODES: usize = 768;
const MODIFIER_KEYS: [KeyCode; 8] = [
    KeyCode::KEY_LEFTCTRL,
    KeyCode::KEY_RIGHTCTRL,
    KeyCode::KEY_LEFTALT,
    KeyCode::KEY_RIGHTALT,
    KeyCode::KEY_LEFTSHIFT,
    KeyCode::KEY_RIGHTSHIFT,
    KeyCode::KEY_LEFTMETA,
    KeyCode::KEY_RIGHTMETA,
];

#[derive(Clone)]
pub(crate) struct SharedKeyState {
    pressed_counts: Arc<Vec<AtomicUsize>>,
}

impl Default for SharedKeyState {
    fn default() -> Self {
        Self::new()
    }
}

impl SharedKeyState {
    pub(crate) fn new() -> Self {
        let pressed_counts = (0..TRACKED_KEY_CODES)
            .map(|_| AtomicUsize::new(0))
            .collect();
        Self {
            pressed_counts: Arc::new(pressed_counts),
        }
    }

    pub(crate) fn press(&self, key: KeyCode) {
        if let Some(index) = key_index(key) {
            self.pressed_counts[index].fetch_add(1, Ordering::Relaxed);
        }
    }

    pub(crate) fn release(&self, key: KeyCode) {
        if let Some(index) = key_index(key) {
            decrement_saturating(&self.pressed_counts[index]);
        }
    }

    pub(crate) fn release_keys<I>(&self, keys: I)
    where
        I: IntoIterator<Item = KeyCode>,
    {
        for key in keys {
            self.release(key);
        }
    }

    pub(crate) fn is_pressed(&self, key: KeyCode) -> bool {
        key_index(key).is_some_and(|index| self.pressed_counts[index].load(Ordering::Relaxed) > 0)
    }

    pub(crate) fn active_modifiers(&self) -> HashSet<KeyCode> {
        MODIFIER_KEYS
            .into_iter()
            .filter(|key| self.is_pressed(*key))
            .collect()
    }

    pub(crate) fn clear(&self) {
        for counter in self.pressed_counts.iter() {
            counter.store(0, Ordering::Relaxed);
        }
    }
}

fn key_index(key: KeyCode) -> Option<usize> {
    let index = key.code() as usize;
    (index < TRACKED_KEY_CODES).then_some(index)
}

fn decrement_saturating(counter: &AtomicUsize) {
    let mut current = counter.load(Ordering::Relaxed);
    while current > 0 {
        match counter.compare_exchange_weak(
            current,
            current - 1,
            Ordering::Relaxed,
            Ordering::Relaxed,
        ) {
            Ok(_) => return,
            Err(actual) => current = actual,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn active_modifiers_only_reports_pressed_modifiers() {
        let key_state = SharedKeyState::new();
        key_state.press(KeyCode::KEY_LEFTCTRL);
        key_state.press(KeyCode::KEY_A);

        let active = key_state.active_modifiers();
        assert!(active.contains(&KeyCode::KEY_LEFTCTRL));
        assert!(!active.contains(&KeyCode::KEY_A));
    }

    #[test]
    fn release_saturates_at_zero() {
        let key_state = SharedKeyState::new();
        key_state.release(KeyCode::KEY_B);

        assert!(!key_state.is_pressed(KeyCode::KEY_B));
    }

    #[test]
    fn clear_resets_all_pressed_keys() {
        let key_state = SharedKeyState::new();
        key_state.press(KeyCode::KEY_A);
        key_state.press(KeyCode::KEY_LEFTCTRL);

        key_state.clear();

        assert!(!key_state.is_pressed(KeyCode::KEY_A));
        assert!(key_state.active_modifiers().is_empty());
    }
}
