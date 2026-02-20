use evdev::KeyCode;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// What happens when a dual-function key is tapped (pressed and released quickly).
#[derive(Clone, Debug)]
pub enum TapAction {
    /// Emit a synthetic key press+release visible to other applications.
    Emit(KeyCode),
}

impl TapAction {
    /// Create a tap action that emits a synthetic key event.
    pub fn emit(key: KeyCode) -> Self {
        TapAction::Emit(key)
    }
}

/// What happens when a dual-function key is held past the threshold.
#[derive(Clone, Debug)]
pub enum HoldAction {
    /// Act as a modifier key: synthetically pressed while held, released on key up.
    Modifier(KeyCode),
}

impl HoldAction {
    /// Create a hold action that acts as a modifier key.
    pub fn modifier(key: KeyCode) -> Self {
        HoldAction::Modifier(key)
    }
}

/// Options for tap-hold key behavior.
#[derive(Clone, Debug)]
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
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the duration threshold: held shorter = tap, held longer = hold.
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
pub(crate) struct TapHoldDispatch {
    /// Synthetic key events to emit via uinput: (key, value).
    /// value: 1 = press, 0 = release.
    pub(crate) synthetic_events: Vec<(KeyCode, i32)>,
    /// Whether the current input key event was consumed by tap-hold
    /// (should not be forwarded to applications or processed as a hotkey).
    pub(crate) consumed: bool,
}

impl TapHoldDispatch {
    fn none() -> Self {
        Self {
            synthetic_events: Vec::new(),
            consumed: false,
        }
    }

    fn consumed() -> Self {
        Self {
            synthetic_events: Vec::new(),
            consumed: true,
        }
    }
}

/// Whether a pending tap-hold key has been resolved as "hold".
///
/// The tap case is not represented here because tap is discovered
/// at release time (key released before threshold, no hold resolution).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum HoldResolution {
    /// Still waiting — neither threshold nor interrupting key has triggered hold.
    Pending,
    /// Resolved as hold: modifier is synthetically active.
    Resolved,
}

/// Tracks a currently active (pressed) tap-hold key.
struct ActiveTapHold {
    key: KeyCode,
    pressed_at: Instant,
    hold_resolution: HoldResolution,
    registration: TapHoldRegistration,
}

/// State machine for tap-hold key processing.
///
/// Processes all key events to detect:
/// - Tap: tap-hold key released before threshold
/// - Hold by duration: tap-hold key held past threshold
/// - Hold by interruption: another key pressed while tap-hold key is pending
#[derive(Default)]
pub(crate) struct TapHoldRuntime {
    active: Option<ActiveTapHold>,
}

impl TapHoldRuntime {
    /// Process a key event through the tap-hold state machine.
    ///
    /// Must be called for ALL key events (not just tap-hold keys) so that
    /// interrupting keypresses can trigger early hold resolution.
    pub(crate) fn process_key_event(
        &mut self,
        key: KeyCode,
        value: i32,
        now: Instant,
        registrations: &std::collections::HashMap<KeyCode, TapHoldRegistration>,
    ) -> TapHoldDispatch {
        match value {
            1 => self.on_key_press(key, now, registrations),
            0 => self.on_key_release(key),
            _ => TapHoldDispatch::none(),
        }
    }

    /// Check for threshold-based hold resolution on each tick.
    pub(crate) fn on_tick(&mut self, now: Instant) -> TapHoldDispatch {
        let Some(active) = self.active.as_mut() else {
            return TapHoldDispatch::none();
        };

        if active.hold_resolution == HoldResolution::Resolved {
            return TapHoldDispatch::none();
        }

        if now.duration_since(active.pressed_at) >= active.registration.threshold {
            active.hold_resolution = HoldResolution::Resolved;
            let synthetic_events = hold_press_events(&active.registration.hold_action);
            return TapHoldDispatch {
                synthetic_events,
                consumed: false,
            };
        }

        TapHoldDispatch::none()
    }

    fn on_key_press(
        &mut self,
        key: KeyCode,
        now: Instant,
        registrations: &std::collections::HashMap<KeyCode, TapHoldRegistration>,
    ) -> TapHoldDispatch {
        // Check if this is a tap-hold key being pressed
        if let Some(registration) = registrations.get(&key) {
            // If there's already an active tap-hold, drop the stale one
            self.active = Some(ActiveTapHold {
                key,
                pressed_at: now,
                hold_resolution: HoldResolution::Pending,
                registration: registration.clone(),
            });

            return TapHoldDispatch::consumed();
        }

        // This is NOT a tap-hold key. Check if it interrupts a pending tap-hold.
        let Some(active) = self.active.as_mut() else {
            return TapHoldDispatch::none();
        };

        if active.hold_resolution == HoldResolution::Resolved {
            return TapHoldDispatch::none();
        }

        // Another key pressed while tap-hold is pending → resolve as hold
        active.hold_resolution = HoldResolution::Resolved;
        let synthetic_events = hold_press_events(&active.registration.hold_action);
        TapHoldDispatch {
            synthetic_events,
            consumed: false,
        }
    }

    fn on_key_release(&mut self, key: KeyCode) -> TapHoldDispatch {
        let is_active_key = self.active.as_ref().is_some_and(|a| a.key == key);
        if !is_active_key {
            return TapHoldDispatch::none();
        }

        let active = self.active.take().unwrap();

        match active.hold_resolution {
            HoldResolution::Pending => {
                // Released before threshold → tap
                let synthetic_events = tap_events(&active.registration.tap_action);
                TapHoldDispatch {
                    synthetic_events,
                    consumed: true,
                }
            }
            HoldResolution::Resolved => {
                // Was resolved as hold, now releasing → release the modifier
                let synthetic_events = hold_release_events(&active.registration.hold_action);
                TapHoldDispatch {
                    synthetic_events,
                    consumed: true,
                }
            }
        }
    }
}

fn tap_events(action: &TapAction) -> Vec<(KeyCode, i32)> {
    match action {
        TapAction::Emit(key) => vec![(*key, 1), (*key, 0)],
    }
}

fn hold_press_events(action: &HoldAction) -> Vec<(KeyCode, i32)> {
    match action {
        HoldAction::Modifier(key) => vec![(*key, 1)],
    }
}

fn hold_release_events(action: &HoldAction) -> Vec<(KeyCode, i32)> {
    match action {
        HoldAction::Modifier(key) => vec![(*key, 0)],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn make_registrations(
        entries: Vec<(KeyCode, TapAction, HoldAction, Duration)>,
    ) -> HashMap<KeyCode, TapHoldRegistration> {
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

    fn capslock_as_ctrl_esc(threshold_ms: u64) -> HashMap<KeyCode, TapHoldRegistration> {
        make_registrations(vec![(
            KeyCode::KEY_CAPSLOCK,
            TapAction::emit(KeyCode::KEY_ESC),
            HoldAction::modifier(KeyCode::KEY_LEFTCTRL),
            Duration::from_millis(threshold_ms),
        )])
    }

    #[test]
    fn tap_resolves_on_release_before_threshold() {
        let mut runtime = TapHoldRuntime::default();
        let t0 = Instant::now();
        let regs = capslock_as_ctrl_esc(200);

        // Press CapsLock
        let press_dispatch = runtime.process_key_event(KeyCode::KEY_CAPSLOCK, 1, t0, &regs);
        assert!(press_dispatch.consumed);
        assert!(press_dispatch.synthetic_events.is_empty());

        // Release CapsLock before threshold
        let release_dispatch = runtime.process_key_event(
            KeyCode::KEY_CAPSLOCK,
            0,
            t0 + Duration::from_millis(50),
            &regs,
        );
        assert!(release_dispatch.consumed);
        assert_eq!(
            release_dispatch.synthetic_events,
            vec![(KeyCode::KEY_ESC, 1), (KeyCode::KEY_ESC, 0)]
        );
    }

    #[test]
    fn hold_resolves_on_threshold_expiry() {
        let mut runtime = TapHoldRuntime::default();
        let t0 = Instant::now();
        let regs = capslock_as_ctrl_esc(200);

        // Press CapsLock
        runtime.process_key_event(KeyCode::KEY_CAPSLOCK, 1, t0, &regs);

        // Tick before threshold — no resolution
        let early_tick = runtime.on_tick(t0 + Duration::from_millis(100));
        assert!(early_tick.synthetic_events.is_empty());

        // Tick at threshold — resolves as hold
        let hold_tick = runtime.on_tick(t0 + Duration::from_millis(200));
        assert_eq!(hold_tick.synthetic_events, vec![(KeyCode::KEY_LEFTCTRL, 1)]);

        // Release CapsLock — releases the modifier
        let release = runtime.process_key_event(
            KeyCode::KEY_CAPSLOCK,
            0,
            t0 + Duration::from_millis(500),
            &regs,
        );
        assert!(release.consumed);
        assert_eq!(release.synthetic_events, vec![(KeyCode::KEY_LEFTCTRL, 0)]);
    }

    #[test]
    fn hold_resolves_early_on_interrupting_keypress() {
        let mut runtime = TapHoldRuntime::default();
        let t0 = Instant::now();
        let regs = capslock_as_ctrl_esc(200);

        // Press CapsLock
        let press = runtime.process_key_event(KeyCode::KEY_CAPSLOCK, 1, t0, &regs);
        assert!(press.consumed);

        // Press 'A' while CapsLock is pending — resolves as hold
        let interrupt =
            runtime.process_key_event(KeyCode::KEY_A, 1, t0 + Duration::from_millis(50), &regs);
        assert!(!interrupt.consumed); // 'A' should NOT be consumed
        assert_eq!(interrupt.synthetic_events, vec![(KeyCode::KEY_LEFTCTRL, 1)]);

        // Release CapsLock — releases modifier
        let release = runtime.process_key_event(
            KeyCode::KEY_CAPSLOCK,
            0,
            t0 + Duration::from_millis(100),
            &regs,
        );
        assert!(release.consumed);
        assert_eq!(release.synthetic_events, vec![(KeyCode::KEY_LEFTCTRL, 0)]);
    }

    #[test]
    fn non_tap_hold_key_passes_through() {
        let mut runtime = TapHoldRuntime::default();
        let t0 = Instant::now();
        let regs = capslock_as_ctrl_esc(200);

        // Press a normal key — not consumed, no synthetic events
        let dispatch = runtime.process_key_event(KeyCode::KEY_A, 1, t0, &regs);
        assert!(!dispatch.consumed);
        assert!(dispatch.synthetic_events.is_empty());
    }

    #[test]
    fn second_interrupting_key_does_not_re_resolve() {
        let mut runtime = TapHoldRuntime::default();
        let t0 = Instant::now();
        let regs = capslock_as_ctrl_esc(200);

        runtime.process_key_event(KeyCode::KEY_CAPSLOCK, 1, t0, &regs);

        // First interrupt resolves as hold
        let first =
            runtime.process_key_event(KeyCode::KEY_A, 1, t0 + Duration::from_millis(50), &regs);
        assert_eq!(first.synthetic_events, vec![(KeyCode::KEY_LEFTCTRL, 1)]);

        // Second key press — already resolved, no extra synthetic events
        let second =
            runtime.process_key_event(KeyCode::KEY_B, 1, t0 + Duration::from_millis(60), &regs);
        assert!(second.synthetic_events.is_empty());
        assert!(!second.consumed);
    }

    #[test]
    fn tick_after_hold_resolution_does_not_re_emit() {
        let mut runtime = TapHoldRuntime::default();
        let t0 = Instant::now();
        let regs = capslock_as_ctrl_esc(200);

        runtime.process_key_event(KeyCode::KEY_CAPSLOCK, 1, t0, &regs);
        runtime.on_tick(t0 + Duration::from_millis(200));

        // Subsequent ticks should not produce more events
        let tick = runtime.on_tick(t0 + Duration::from_millis(300));
        assert!(tick.synthetic_events.is_empty());
    }

    #[test]
    fn repeat_events_are_ignored() {
        let mut runtime = TapHoldRuntime::default();
        let t0 = Instant::now();
        let regs = capslock_as_ctrl_esc(200);

        runtime.process_key_event(KeyCode::KEY_CAPSLOCK, 1, t0, &regs);

        // Repeat event (value=2) should be ignored
        let repeat = runtime.process_key_event(
            KeyCode::KEY_CAPSLOCK,
            2,
            t0 + Duration::from_millis(50),
            &regs,
        );
        assert!(!repeat.consumed);
        assert!(repeat.synthetic_events.is_empty());
    }

    #[test]
    fn releasing_non_active_key_is_noop() {
        let mut runtime = TapHoldRuntime::default();
        let t0 = Instant::now();
        let regs = capslock_as_ctrl_esc(200);

        runtime.process_key_event(KeyCode::KEY_CAPSLOCK, 1, t0, &regs);

        // Release a different key — noop
        let release =
            runtime.process_key_event(KeyCode::KEY_A, 0, t0 + Duration::from_millis(50), &regs);
        assert!(!release.consumed);
        assert!(release.synthetic_events.is_empty());
    }

    #[test]
    fn tap_hold_can_be_reused_after_tap() {
        let mut runtime = TapHoldRuntime::default();
        let t0 = Instant::now();
        let regs = capslock_as_ctrl_esc(200);

        // First tap
        runtime.process_key_event(KeyCode::KEY_CAPSLOCK, 1, t0, &regs);
        let release1 = runtime.process_key_event(
            KeyCode::KEY_CAPSLOCK,
            0,
            t0 + Duration::from_millis(50),
            &regs,
        );
        assert_eq!(
            release1.synthetic_events,
            vec![(KeyCode::KEY_ESC, 1), (KeyCode::KEY_ESC, 0)]
        );

        // Second tap
        let t1 = t0 + Duration::from_millis(500);
        runtime.process_key_event(KeyCode::KEY_CAPSLOCK, 1, t1, &regs);
        let release2 = runtime.process_key_event(
            KeyCode::KEY_CAPSLOCK,
            0,
            t1 + Duration::from_millis(50),
            &regs,
        );
        assert_eq!(
            release2.synthetic_events,
            vec![(KeyCode::KEY_ESC, 1), (KeyCode::KEY_ESC, 0)]
        );
    }

    #[test]
    fn tap_hold_can_be_reused_after_hold() {
        let mut runtime = TapHoldRuntime::default();
        let t0 = Instant::now();
        let regs = capslock_as_ctrl_esc(200);

        // First: hold
        runtime.process_key_event(KeyCode::KEY_CAPSLOCK, 1, t0, &regs);
        runtime.on_tick(t0 + Duration::from_millis(200));
        runtime.process_key_event(
            KeyCode::KEY_CAPSLOCK,
            0,
            t0 + Duration::from_millis(500),
            &regs,
        );

        // Second: tap
        let t1 = t0 + Duration::from_secs(1);
        runtime.process_key_event(KeyCode::KEY_CAPSLOCK, 1, t1, &regs);
        let release = runtime.process_key_event(
            KeyCode::KEY_CAPSLOCK,
            0,
            t1 + Duration::from_millis(50),
            &regs,
        );
        assert_eq!(
            release.synthetic_events,
            vec![(KeyCode::KEY_ESC, 1), (KeyCode::KEY_ESC, 0)]
        );
    }

    #[test]
    fn empty_registrations_passes_through_everything() {
        let mut runtime = TapHoldRuntime::default();
        let t0 = Instant::now();
        let regs = HashMap::new();

        let dispatch = runtime.process_key_event(KeyCode::KEY_CAPSLOCK, 1, t0, &regs);
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
