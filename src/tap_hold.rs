use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;

use crate::key::Key;
use crate::key::Modifier;

/// What happens when a dual-function key is tapped (pressed and released quickly).
#[derive(Clone, Debug)]
pub enum TapAction {
    /// Emit a synthetic key press+release visible to other applications.
    Emit(Key),
}

impl TapAction {
    /// Create a tap action that emits a synthetic key event.
    #[must_use]
    pub fn emit(key: Key) -> Self {
        TapAction::Emit(key)
    }
}

/// What happens when a dual-function key is held past the threshold.
#[derive(Clone, Debug)]
pub enum HoldAction {
    /// Act as a modifier key: synthetically pressed while held, released on key up.
    Modifier(Modifier),
}

impl HoldAction {
    /// Create a hold action that acts as a modifier key.
    #[must_use]
    pub fn modifier(modifier: Modifier) -> Self {
        HoldAction::Modifier(modifier)
    }
}

/// Options for tap-hold key behavior.
#[derive(Clone, Copy, Debug)]
pub struct TapHoldOptions {
    pub(crate) threshold: Duration,
}

impl Default for TapHoldOptions {
    fn default() -> Self {
        Self {
            threshold: Duration::from_millis(200),
        }
    }
}

impl TapHoldOptions {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the duration threshold: held shorter = tap, held longer = hold.
    #[must_use]
    pub fn threshold(mut self, threshold: Duration) -> Self {
        self.threshold = threshold;
        self
    }
}

/// Internal registration for a tap-hold key.
#[derive(Clone)]
pub(crate) struct TapHoldRegistration {
    pub(crate) tap_action: TapAction,
    pub(crate) hold_action: HoldAction,
    pub(crate) threshold: Duration,
    pub(crate) marker: Arc<()>,
}

/// Result of tap-hold processing for a key event.
/// Synthetic events use `evdev::KeyCode` because they are emitted via uinput.
pub(crate) struct TapHoldDispatch {
    pub(crate) synthetic_events: Vec<(evdev::KeyCode, i32)>,
    pub(crate) consumed: bool,
}

impl TapHoldDispatch {
    fn none() -> Self {
        Self {
            synthetic_events: Vec::new(),
            consumed: false,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum HoldResolution {
    Pending,
    Resolved,
}

struct ActiveTapHold {
    pressed_at: Instant,
    hold_resolution: HoldResolution,
    registration: TapHoldRegistration,
}

/// State machine for tap-hold key processing.
///
/// Keyed by `Key` (our abstraction). The listener converts evdev key codes
/// to `Key` before calling into this runtime.
#[derive(Default)]
pub(crate) struct TapHoldRuntime {
    active: HashMap<Key, ActiveTapHold>,
}

impl TapHoldRuntime {
    pub(crate) fn process_key_event(
        &mut self,
        key: Key,
        value: i32,
        now: Instant,
        registrations: &HashMap<Key, TapHoldRegistration>,
    ) -> TapHoldDispatch {
        match value {
            1 => self.on_key_press(key, now, registrations),
            0 => self.on_key_release(key),
            2 => self.on_key_repeat(key),
            _ => TapHoldDispatch::none(),
        }
    }

    pub(crate) fn on_tick(&mut self, now: Instant) -> TapHoldDispatch {
        let mut keys_to_resolve: Vec<(Instant, Key)> = self
            .active
            .iter()
            .filter_map(|(key, active)| {
                (active.hold_resolution == HoldResolution::Pending
                    && now.saturating_duration_since(active.pressed_at)
                        >= active.registration.threshold)
                    .then_some((active.pressed_at, *key))
            })
            .collect();
        keys_to_resolve.sort_by_key(|(pressed_at, key)| (*pressed_at, *key));

        let mut synthetic_events = Vec::new();
        for (_, key) in keys_to_resolve {
            if let Some(active) = self.active.get_mut(&key) {
                active.hold_resolution = HoldResolution::Resolved;
                synthetic_events.extend(hold_press_events(&active.registration.hold_action));
            }
        }

        if synthetic_events.is_empty() {
            return TapHoldDispatch::none();
        }

        TapHoldDispatch {
            synthetic_events,
            consumed: false,
        }
    }

    pub(crate) fn release_all(&mut self) -> Vec<(evdev::KeyCode, i32)> {
        let mut keys_to_release: Vec<(Instant, Key)> = self
            .active
            .iter()
            .filter_map(|(key, active)| {
                (active.hold_resolution == HoldResolution::Resolved)
                    .then_some((active.pressed_at, *key))
            })
            .collect();
        keys_to_release.sort_by_key(|(pressed_at, key)| (*pressed_at, *key));

        let mut synthetic_events = Vec::new();
        for (_, key) in keys_to_release {
            if let Some(active) = self.active.remove(&key) {
                synthetic_events.extend(hold_release_events(&active.registration.hold_action));
            }
        }

        self.active.clear();
        synthetic_events
    }

    fn on_key_press(
        &mut self,
        key: Key,
        now: Instant,
        registrations: &HashMap<Key, TapHoldRegistration>,
    ) -> TapHoldDispatch {
        let mut synthetic_events = self.resolve_pending_holds_for_interrupt(key);

        if let Some(registration) = registrations.get(&key) {
            if let Some(previous_active) = self.active.remove(&key) {
                if previous_active.hold_resolution == HoldResolution::Resolved {
                    synthetic_events.extend(hold_release_events(
                        &previous_active.registration.hold_action,
                    ));
                }
            }

            self.active.insert(
                key,
                ActiveTapHold {
                    pressed_at: now,
                    hold_resolution: HoldResolution::Pending,
                    registration: registration.clone(),
                },
            );

            return TapHoldDispatch {
                synthetic_events,
                consumed: true,
            };
        }

        TapHoldDispatch {
            synthetic_events,
            consumed: false,
        }
    }

    fn on_key_release(&mut self, key: Key) -> TapHoldDispatch {
        let Some(active) = self.active.remove(&key) else {
            return TapHoldDispatch::none();
        };

        match active.hold_resolution {
            HoldResolution::Pending => {
                let synthetic_events = tap_events(&active.registration.tap_action);
                TapHoldDispatch {
                    synthetic_events,
                    consumed: true,
                }
            }
            HoldResolution::Resolved => {
                let synthetic_events = hold_release_events(&active.registration.hold_action);
                TapHoldDispatch {
                    synthetic_events,
                    consumed: true,
                }
            }
        }
    }

    fn on_key_repeat(&self, key: Key) -> TapHoldDispatch {
        if self.active.contains_key(&key) {
            return TapHoldDispatch {
                synthetic_events: Vec::new(),
                consumed: true,
            };
        }

        TapHoldDispatch::none()
    }

    fn resolve_pending_holds_for_interrupt(&mut self, key: Key) -> Vec<(evdev::KeyCode, i32)> {
        let mut keys_to_resolve: Vec<(Instant, Key)> = self
            .active
            .iter()
            .filter_map(|(active_key, active)| {
                (*active_key != key && active.hold_resolution == HoldResolution::Pending)
                    .then_some((active.pressed_at, *active_key))
            })
            .collect();
        keys_to_resolve.sort_by_key(|(pressed_at, key)| (*pressed_at, *key));

        let mut synthetic_events = Vec::new();
        for (_, key_to_resolve) in keys_to_resolve {
            if let Some(active) = self.active.get_mut(&key_to_resolve) {
                active.hold_resolution = HoldResolution::Resolved;
                synthetic_events.extend(hold_press_events(&active.registration.hold_action));
            }
        }

        synthetic_events
    }
}

fn tap_events(action: &TapAction) -> Vec<(evdev::KeyCode, i32)> {
    match action {
        TapAction::Emit(key) => {
            let code = key.to_evdev();
            vec![(code, 1), (code, 0)]
        }
    }
}

fn hold_press_events(action: &HoldAction) -> Vec<(evdev::KeyCode, i32)> {
    match action {
        HoldAction::Modifier(modifier) => vec![(modifier.to_evdev(), 1)],
    }
}

fn hold_release_events(action: &HoldAction) -> Vec<(evdev::KeyCode, i32)> {
    match action {
        HoldAction::Modifier(modifier) => vec![(modifier.to_evdev(), 0)],
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    fn make_registrations(
        entries: Vec<(Key, TapAction, HoldAction, Duration)>,
    ) -> HashMap<Key, TapHoldRegistration> {
        entries
            .into_iter()
            .map(|(key, tap, hold, threshold)| {
                (
                    key,
                    TapHoldRegistration {
                        tap_action: tap,
                        hold_action: hold,
                        threshold,
                        marker: Arc::new(()),
                    },
                )
            })
            .collect()
    }

    fn capslock_as_ctrl_esc(threshold_ms: u64) -> HashMap<Key, TapHoldRegistration> {
        make_registrations(vec![(
            Key::CapsLock,
            TapAction::emit(Key::Escape),
            HoldAction::modifier(Modifier::Ctrl),
            Duration::from_millis(threshold_ms),
        )])
    }

    #[test]
    fn tap_resolves_on_release_before_threshold() {
        let mut runtime = TapHoldRuntime::default();
        let t0 = Instant::now();
        let regs = capslock_as_ctrl_esc(200);

        let press_dispatch = runtime.process_key_event(Key::CapsLock, 1, t0, &regs);
        assert!(press_dispatch.consumed);
        assert!(press_dispatch.synthetic_events.is_empty());

        let release_dispatch =
            runtime.process_key_event(Key::CapsLock, 0, t0 + Duration::from_millis(50), &regs);
        assert!(release_dispatch.consumed);
        let esc = Key::Escape.to_evdev();
        assert_eq!(release_dispatch.synthetic_events, vec![(esc, 1), (esc, 0)]);
    }

    #[test]
    fn hold_resolves_on_threshold_expiry() {
        let mut runtime = TapHoldRuntime::default();
        let t0 = Instant::now();
        let regs = capslock_as_ctrl_esc(200);

        runtime.process_key_event(Key::CapsLock, 1, t0, &regs);

        let early_tick = runtime.on_tick(t0 + Duration::from_millis(100));
        assert!(early_tick.synthetic_events.is_empty());

        let ctrl = Modifier::Ctrl.to_evdev();
        let hold_tick = runtime.on_tick(t0 + Duration::from_millis(200));
        assert_eq!(hold_tick.synthetic_events, vec![(ctrl, 1)]);

        let release =
            runtime.process_key_event(Key::CapsLock, 0, t0 + Duration::from_millis(500), &regs);
        assert!(release.consumed);
        assert_eq!(release.synthetic_events, vec![(ctrl, 0)]);
    }

    #[test]
    fn hold_resolves_early_on_interrupting_keypress() {
        let mut runtime = TapHoldRuntime::default();
        let t0 = Instant::now();
        let regs = capslock_as_ctrl_esc(200);

        let press = runtime.process_key_event(Key::CapsLock, 1, t0, &regs);
        assert!(press.consumed);

        let ctrl = Modifier::Ctrl.to_evdev();
        let interrupt = runtime.process_key_event(Key::A, 1, t0 + Duration::from_millis(50), &regs);
        assert!(!interrupt.consumed);
        assert_eq!(interrupt.synthetic_events, vec![(ctrl, 1)]);

        let release =
            runtime.process_key_event(Key::CapsLock, 0, t0 + Duration::from_millis(100), &regs);
        assert!(release.consumed);
        assert_eq!(release.synthetic_events, vec![(ctrl, 0)]);
    }

    #[test]
    fn non_tap_hold_key_passes_through() {
        let mut runtime = TapHoldRuntime::default();
        let t0 = Instant::now();
        let regs = capslock_as_ctrl_esc(200);

        let dispatch = runtime.process_key_event(Key::A, 1, t0, &regs);
        assert!(!dispatch.consumed);
        assert!(dispatch.synthetic_events.is_empty());
    }

    #[test]
    fn second_interrupting_key_does_not_re_resolve() {
        let mut runtime = TapHoldRuntime::default();
        let t0 = Instant::now();
        let regs = capslock_as_ctrl_esc(200);

        runtime.process_key_event(Key::CapsLock, 1, t0, &regs);

        let ctrl = Modifier::Ctrl.to_evdev();
        let first = runtime.process_key_event(Key::A, 1, t0 + Duration::from_millis(50), &regs);
        assert_eq!(first.synthetic_events, vec![(ctrl, 1)]);

        let second = runtime.process_key_event(Key::B, 1, t0 + Duration::from_millis(60), &regs);
        assert!(second.synthetic_events.is_empty());
        assert!(!second.consumed);
    }

    #[test]
    fn tick_after_hold_resolution_does_not_re_emit() {
        let mut runtime = TapHoldRuntime::default();
        let t0 = Instant::now();
        let regs = capslock_as_ctrl_esc(200);

        runtime.process_key_event(Key::CapsLock, 1, t0, &regs);
        runtime.on_tick(t0 + Duration::from_millis(200));

        let tick = runtime.on_tick(t0 + Duration::from_millis(300));
        assert!(tick.synthetic_events.is_empty());
    }

    #[test]
    fn repeat_events_for_active_tap_hold_key_are_consumed() {
        let mut runtime = TapHoldRuntime::default();
        let t0 = Instant::now();
        let regs = capslock_as_ctrl_esc(200);

        runtime.process_key_event(Key::CapsLock, 1, t0, &regs);

        let repeat =
            runtime.process_key_event(Key::CapsLock, 2, t0 + Duration::from_millis(50), &regs);
        assert!(repeat.consumed);
        assert!(repeat.synthetic_events.is_empty());
    }

    #[test]
    fn repeat_events_for_non_active_key_pass_through() {
        let mut runtime = TapHoldRuntime::default();
        let t0 = Instant::now();
        let regs = capslock_as_ctrl_esc(200);

        runtime.process_key_event(Key::CapsLock, 1, t0, &regs);

        let repeat = runtime.process_key_event(Key::A, 2, t0 + Duration::from_millis(50), &regs);
        assert!(!repeat.consumed);
        assert!(repeat.synthetic_events.is_empty());
    }

    #[test]
    fn releasing_non_active_key_is_noop() {
        let mut runtime = TapHoldRuntime::default();
        let t0 = Instant::now();
        let regs = capslock_as_ctrl_esc(200);

        runtime.process_key_event(Key::CapsLock, 1, t0, &regs);

        let release = runtime.process_key_event(Key::A, 0, t0 + Duration::from_millis(50), &regs);
        assert!(!release.consumed);
        assert!(release.synthetic_events.is_empty());
    }

    #[test]
    fn tap_hold_can_be_reused_after_tap() {
        let mut runtime = TapHoldRuntime::default();
        let t0 = Instant::now();
        let regs = capslock_as_ctrl_esc(200);
        let esc = Key::Escape.to_evdev();

        runtime.process_key_event(Key::CapsLock, 1, t0, &regs);
        let release1 =
            runtime.process_key_event(Key::CapsLock, 0, t0 + Duration::from_millis(50), &regs);
        assert_eq!(release1.synthetic_events, vec![(esc, 1), (esc, 0)]);

        let t1 = t0 + Duration::from_millis(500);
        runtime.process_key_event(Key::CapsLock, 1, t1, &regs);
        let release2 =
            runtime.process_key_event(Key::CapsLock, 0, t1 + Duration::from_millis(50), &regs);
        assert_eq!(release2.synthetic_events, vec![(esc, 1), (esc, 0)]);
    }

    #[test]
    fn tap_hold_can_be_reused_after_hold() {
        let mut runtime = TapHoldRuntime::default();
        let t0 = Instant::now();
        let regs = capslock_as_ctrl_esc(200);

        runtime.process_key_event(Key::CapsLock, 1, t0, &regs);
        runtime.on_tick(t0 + Duration::from_millis(200));
        runtime.process_key_event(Key::CapsLock, 0, t0 + Duration::from_millis(500), &regs);

        let t1 = t0 + Duration::from_secs(1);
        runtime.process_key_event(Key::CapsLock, 1, t1, &regs);
        let release =
            runtime.process_key_event(Key::CapsLock, 0, t1 + Duration::from_millis(50), &regs);
        let esc = Key::Escape.to_evdev();
        assert_eq!(release.synthetic_events, vec![(esc, 1), (esc, 0)]);
    }

    #[test]
    fn multiple_tap_hold_keys_keep_independent_hold_release_state() {
        let mut runtime = TapHoldRuntime::default();
        let t0 = Instant::now();
        let regs = make_registrations(vec![
            (
                Key::CapsLock,
                TapAction::emit(Key::Escape),
                HoldAction::modifier(Modifier::Ctrl),
                Duration::from_millis(200),
            ),
            (
                Key::Tab,
                TapAction::emit(Key::Enter),
                HoldAction::modifier(Modifier::Alt),
                Duration::from_millis(200),
            ),
        ]);

        let ctrl = Modifier::Ctrl.to_evdev();
        let enter = Key::Enter.to_evdev();

        runtime.process_key_event(Key::CapsLock, 1, t0, &regs);

        let hold_tick = runtime.on_tick(t0 + Duration::from_millis(200));
        assert_eq!(hold_tick.synthetic_events, vec![(ctrl, 1)]);

        let tab_press =
            runtime.process_key_event(Key::Tab, 1, t0 + Duration::from_millis(210), &regs);
        assert!(tab_press.consumed);
        assert!(tab_press.synthetic_events.is_empty());

        let caps_release =
            runtime.process_key_event(Key::CapsLock, 0, t0 + Duration::from_millis(220), &regs);
        assert!(caps_release.consumed);
        assert_eq!(caps_release.synthetic_events, vec![(ctrl, 0)]);

        let tab_release =
            runtime.process_key_event(Key::Tab, 0, t0 + Duration::from_millis(240), &regs);
        assert!(tab_release.consumed);
        assert_eq!(tab_release.synthetic_events, vec![(enter, 1), (enter, 0)]);
    }

    #[test]
    fn interrupting_key_resolves_each_pending_tap_hold_key_once() {
        let mut runtime = TapHoldRuntime::default();
        let t0 = Instant::now();
        let regs = make_registrations(vec![
            (
                Key::CapsLock,
                TapAction::emit(Key::Escape),
                HoldAction::modifier(Modifier::Ctrl),
                Duration::from_millis(200),
            ),
            (
                Key::Tab,
                TapAction::emit(Key::Enter),
                HoldAction::modifier(Modifier::Alt),
                Duration::from_millis(200),
            ),
        ]);

        let ctrl = Modifier::Ctrl.to_evdev();
        let alt = Modifier::Alt.to_evdev();

        runtime.process_key_event(Key::CapsLock, 1, t0, &regs);

        let tab_press =
            runtime.process_key_event(Key::Tab, 1, t0 + Duration::from_millis(10), &regs);
        assert!(tab_press.consumed);
        assert_eq!(tab_press.synthetic_events, vec![(ctrl, 1)]);

        let a_press = runtime.process_key_event(Key::A, 1, t0 + Duration::from_millis(20), &regs);
        assert!(!a_press.consumed);
        assert_eq!(a_press.synthetic_events, vec![(alt, 1)]);

        let second_a_press =
            runtime.process_key_event(Key::A, 1, t0 + Duration::from_millis(30), &regs);
        assert!(second_a_press.synthetic_events.is_empty());

        let caps_release =
            runtime.process_key_event(Key::CapsLock, 0, t0 + Duration::from_millis(40), &regs);
        assert_eq!(caps_release.synthetic_events, vec![(ctrl, 0)]);

        let tab_release =
            runtime.process_key_event(Key::Tab, 0, t0 + Duration::from_millis(50), &regs);
        assert_eq!(tab_release.synthetic_events, vec![(alt, 0)]);
    }

    #[test]
    fn non_monotonic_tick_timestamp_does_not_panic_or_resolve_hold() {
        let mut runtime = TapHoldRuntime::default();
        let t0 = Instant::now();
        let regs = capslock_as_ctrl_esc(200);

        runtime.process_key_event(Key::CapsLock, 1, t0, &regs);

        let earlier = t0.checked_sub(Duration::from_millis(1)).unwrap_or(t0);
        let earlier_tick = runtime.on_tick(earlier);
        assert!(earlier_tick.synthetic_events.is_empty());

        let release_dispatch =
            runtime.process_key_event(Key::CapsLock, 0, t0 + Duration::from_millis(50), &regs);
        let esc = Key::Escape.to_evdev();
        assert_eq!(release_dispatch.synthetic_events, vec![(esc, 1), (esc, 0)]);
    }

    #[test]
    fn release_all_emits_release_for_resolved_holds_and_clears_state() {
        let mut runtime = TapHoldRuntime::default();
        let t0 = Instant::now();
        let regs = capslock_as_ctrl_esc(200);

        runtime.process_key_event(Key::CapsLock, 1, t0, &regs);
        runtime.on_tick(t0 + Duration::from_millis(200));

        let ctrl = Modifier::Ctrl.to_evdev();
        let shutdown_releases = runtime.release_all();
        assert_eq!(shutdown_releases, vec![(ctrl, 0)]);

        let release_after_shutdown =
            runtime.process_key_event(Key::CapsLock, 0, t0 + Duration::from_millis(250), &regs);
        assert!(!release_after_shutdown.consumed);
        assert!(release_after_shutdown.synthetic_events.is_empty());
    }

    #[test]
    fn release_all_drops_pending_tap_without_emitting() {
        let mut runtime = TapHoldRuntime::default();
        let t0 = Instant::now();
        let regs = capslock_as_ctrl_esc(200);

        runtime.process_key_event(Key::CapsLock, 1, t0, &regs);

        let shutdown_releases = runtime.release_all();
        assert!(shutdown_releases.is_empty());

        let release_after_shutdown =
            runtime.process_key_event(Key::CapsLock, 0, t0 + Duration::from_millis(50), &regs);
        assert!(!release_after_shutdown.consumed);
        assert!(release_after_shutdown.synthetic_events.is_empty());
    }

    #[test]
    fn empty_registrations_passes_through_everything() {
        let mut runtime = TapHoldRuntime::default();
        let t0 = Instant::now();
        let regs = HashMap::new();

        let dispatch = runtime.process_key_event(Key::CapsLock, 1, t0, &regs);
        assert!(!dispatch.consumed);
        assert!(dispatch.synthetic_events.is_empty());
    }

    #[test]
    fn tick_with_no_active_tap_hold_is_noop() {
        let mut runtime = TapHoldRuntime::default();
        let t0 = Instant::now();

        let tick = runtime.on_tick(t0);
        assert!(tick.synthetic_events.is_empty());
    }
}
