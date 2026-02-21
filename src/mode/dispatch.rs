use std::collections::HashMap;
use std::time::Instant;

use super::options::ModeDefinition;
use super::stack::lookup_hotkey_in_modes;
use super::stack::ModeLookupResult;
use super::stack::ModeStack;
use crate::key::Key;
use crate::manager::ActiveHotkeyPress;
use crate::manager::Callback;
use crate::manager::DeviceHotkeyRegistration;
use crate::manager::DeviceRegistrationId;
use crate::manager::HotkeyCallbacks;
use crate::manager::HotkeyKey;
use crate::manager::HotkeyRegistration;
use crate::manager::PressDispatchState;
use crate::manager::PressOrigin;
use crate::manager::RepeatBehavior;

pub(crate) enum ModeEventDispatch {
    PassThrough,
    Swallowed,
    Handled {
        callbacks: Vec<Callback>,
        passthrough: bool,
    },
}

pub(crate) fn dispatch_mode_key_event(
    hotkey_key: &HotkeyKey,
    value: i32,
    now: Instant,
    definitions: &HashMap<String, ModeDefinition>,
    stack: &mut ModeStack,
    active_presses: &mut HashMap<Key, ActiveHotkeyPress>,
) -> ModeEventDispatch {
    match value {
        1 => dispatch_mode_press(hotkey_key, now, definitions, stack, active_presses),
        0 => dispatch_mode_release(hotkey_key.0, now, definitions, stack, active_presses),
        2 => dispatch_mode_repeat(hotkey_key.0, now, definitions, active_presses),
        _ => ModeEventDispatch::PassThrough,
    }
}

fn dispatch_mode_press(
    hotkey_key: &HotkeyKey,
    now: Instant,
    definitions: &HashMap<String, ModeDefinition>,
    stack: &mut ModeStack,
    active_presses: &mut HashMap<Key, ActiveHotkeyPress>,
) -> ModeEventDispatch {
    if stack.is_empty() {
        return ModeEventDispatch::PassThrough;
    }

    match lookup_hotkey_in_modes(hotkey_key, stack, definitions) {
        ModeLookupResult::Matched {
            mode_name,
            registration,
            oneshot,
        } => {
            let press_dispatch_state = registration.callbacks.min_hold.map_or(
                PressDispatchState::Dispatched,
                |min_hold| {
                    if min_hold.is_zero() {
                        PressDispatchState::Dispatched
                    } else {
                        PressDispatchState::Pending
                    }
                },
            );

            let passthrough = registration.callbacks.passthrough;

            active_presses.insert(
                hotkey_key.0,
                ActiveHotkeyPress {
                    registration_key: hotkey_key.clone(),
                    origin: PressOrigin::Mode(mode_name.clone()),
                    pressed_at: now,
                    press_dispatch_state,
                },
            );

            let mut callbacks = Vec::new();
            if press_dispatch_state == PressDispatchState::Dispatched {
                callbacks.push(registration.callbacks.on_press.clone());
            }

            if oneshot {
                stack.remove_topmost(&mode_name);
            }

            stack.touch_top(now);

            ModeEventDispatch::Handled {
                callbacks,
                passthrough,
            }
        }
        ModeLookupResult::Swallowed => {
            stack.touch_top(now);
            ModeEventDispatch::Swallowed
        }
        ModeLookupResult::PassThrough => ModeEventDispatch::PassThrough,
    }
}

fn dispatch_mode_release(
    key: Key,
    now: Instant,
    definitions: &HashMap<String, ModeDefinition>,
    stack: &mut ModeStack,
    active_presses: &mut HashMap<Key, ActiveHotkeyPress>,
) -> ModeEventDispatch {
    let Some(active) = active_presses.get(&key) else {
        return ModeEventDispatch::PassThrough;
    };
    let PressOrigin::Mode(ref mode_name) = active.origin else {
        return ModeEventDispatch::PassThrough;
    };
    let mode_name = mode_name.clone();

    let active = active_presses.remove(&key).unwrap();

    let Some(registration) = definitions
        .get(&mode_name)
        .and_then(|def| def.bindings.get(&active.registration_key))
    else {
        return ModeEventDispatch::Handled {
            callbacks: Vec::new(),
            passthrough: false,
        };
    };

    let passthrough = registration.callbacks.passthrough;
    let mut callbacks = Vec::new();

    if active.press_dispatch_state == PressDispatchState::Pending {
        if let Some(min_hold) = registration.callbacks.min_hold {
            if now.duration_since(active.pressed_at) >= min_hold {
                callbacks.push(registration.callbacks.on_press.clone());
            }
        }
    }

    if let Some(on_release) = &registration.callbacks.on_release {
        callbacks.push(on_release.clone());
    }

    if !stack.is_empty() {
        stack.touch_top(now);
    }

    ModeEventDispatch::Handled {
        callbacks,
        passthrough,
    }
}

fn dispatch_mode_repeat(
    key: Key,
    now: Instant,
    definitions: &HashMap<String, ModeDefinition>,
    active_presses: &mut HashMap<Key, ActiveHotkeyPress>,
) -> ModeEventDispatch {
    let is_mode_press = active_presses
        .get(&key)
        .is_some_and(|active| matches!(active.origin, PressOrigin::Mode(_)));

    if !is_mode_press {
        return ModeEventDispatch::PassThrough;
    }

    let active = active_presses.get_mut(&key).unwrap();
    let PressOrigin::Mode(ref mode_name) = active.origin else {
        unreachable!();
    };

    let Some(registration) = definitions
        .get(mode_name)
        .and_then(|def| def.bindings.get(&active.registration_key))
    else {
        return ModeEventDispatch::Handled {
            callbacks: Vec::new(),
            passthrough: false,
        };
    };

    let passthrough = registration.callbacks.passthrough;
    let hold_satisfied = registration
        .callbacks
        .min_hold
        .is_none_or(|min_hold| now.duration_since(active.pressed_at) >= min_hold);

    let mut callbacks = Vec::new();
    if registration.callbacks.repeat_behavior == RepeatBehavior::Trigger && hold_satisfied {
        callbacks.push(registration.callbacks.on_press.clone());
        active.press_dispatch_state = PressDispatchState::Dispatched;
    }

    ModeEventDispatch::Handled {
        callbacks,
        passthrough,
    }
}

/// Look up a registration's callbacks for an active press, checking mode definitions
/// when the press originated from a mode, device registrations when the press
/// originated from a device-specific hotkey, or global registrations otherwise.
pub(crate) fn find_callbacks_for_active_press<'a>(
    active: &ActiveHotkeyPress,
    global_registrations: &'a HashMap<HotkeyKey, HotkeyRegistration>,
    mode_definitions: &'a HashMap<String, ModeDefinition>,
    device_registrations: &'a HashMap<DeviceRegistrationId, DeviceHotkeyRegistration>,
) -> Option<&'a HotkeyCallbacks> {
    match &active.origin {
        PressOrigin::Mode(mode_name) => mode_definitions
            .get(mode_name)
            .and_then(|def| def.bindings.get(&active.registration_key))
            .map(|reg| &reg.callbacks),
        PressOrigin::Device(device_reg_id) => device_registrations
            .get(device_reg_id)
            .map(|reg| &reg.callbacks),
        PressOrigin::Global => global_registrations
            .get(&active.registration_key)
            .map(|reg| &reg.callbacks),
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::AtomicUsize;
    use std::sync::atomic::Ordering;
    use std::sync::Arc;
    use std::time::Duration;

    use super::*;
    use crate::key::Key;
    use crate::manager::ActiveHotkeyPress;
    use crate::manager::PressDispatchState;
    use crate::manager::PressOrigin;
    use crate::mode::options::ModeOptions;
    use crate::mode::tests::dispatch_callbacks;
    use crate::mode::tests::make_definition;
    use crate::mode::tests::make_registration;
    use crate::mode::tests::make_registration_with_release;

    #[test]
    fn mode_dispatch_press_fires_callback() {
        let mut stack = ModeStack::default();
        let t0 = Instant::now();
        stack.push("resize".to_string(), t0);

        let counter = Arc::new(AtomicUsize::new(0));
        let key = (Key::H, vec![]);

        let mut definitions = HashMap::new();
        definitions.insert(
            "resize".to_string(),
            make_definition(
                ModeOptions::new(),
                vec![(key.clone(), make_registration(counter.clone()))],
            ),
        );

        let mut active_presses = HashMap::new();
        let dispatch =
            dispatch_mode_key_event(&key, 1, t0, &definitions, &mut stack, &mut active_presses);

        let ModeEventDispatch::Handled { callbacks, .. } = dispatch else {
            panic!("expected Handled");
        };
        dispatch_callbacks(callbacks);
        assert_eq!(counter.load(Ordering::SeqCst), 1);
        assert!(active_presses.contains_key(&Key::H));
    }

    #[test]
    fn mode_dispatch_release_fires_release_callback() {
        let mut stack = ModeStack::default();
        let t0 = Instant::now();
        stack.push("test".to_string(), t0);

        let press_count = Arc::new(AtomicUsize::new(0));
        let release_count = Arc::new(AtomicUsize::new(0));
        let key = (Key::H, vec![]);

        let mut definitions = HashMap::new();
        definitions.insert(
            "test".to_string(),
            make_definition(
                ModeOptions::new(),
                vec![(
                    key.clone(),
                    make_registration_with_release(press_count.clone(), release_count.clone()),
                )],
            ),
        );

        let mut active_presses = HashMap::new();

        let press_dispatch =
            dispatch_mode_key_event(&key, 1, t0, &definitions, &mut stack, &mut active_presses);
        let ModeEventDispatch::Handled { callbacks, .. } = press_dispatch else {
            panic!("expected Handled");
        };
        dispatch_callbacks(callbacks);

        let release_dispatch = dispatch_mode_key_event(
            &key,
            0,
            t0 + Duration::from_millis(10),
            &definitions,
            &mut stack,
            &mut active_presses,
        );
        let ModeEventDispatch::Handled { callbacks, .. } = release_dispatch else {
            panic!("expected Handled");
        };
        dispatch_callbacks(callbacks);

        assert_eq!(press_count.load(Ordering::SeqCst), 1);
        assert_eq!(release_count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn mode_dispatch_oneshot_pops_after_match() {
        let mut stack = ModeStack::default();
        let t0 = Instant::now();
        stack.push("oneshot_mode".to_string(), t0);

        let counter = Arc::new(AtomicUsize::new(0));
        let key = (Key::F, vec![]);

        let mut definitions = HashMap::new();
        definitions.insert(
            "oneshot_mode".to_string(),
            make_definition(
                ModeOptions::new().oneshot(),
                vec![(key.clone(), make_registration(counter.clone()))],
            ),
        );

        let mut active_presses = HashMap::new();
        let dispatch =
            dispatch_mode_key_event(&key, 1, t0, &definitions, &mut stack, &mut active_presses);

        let ModeEventDispatch::Handled { callbacks, .. } = dispatch else {
            panic!("expected Handled");
        };
        dispatch_callbacks(callbacks);
        assert_eq!(counter.load(Ordering::SeqCst), 1);
        assert!(stack.is_empty());
    }

    #[test]
    fn mode_dispatch_oneshot_removes_matched_mode_not_top() {
        let mut stack = ModeStack::default();
        let t0 = Instant::now();

        stack.push("base_oneshot".to_string(), t0);
        stack.push("overlay".to_string(), t0);

        let counter = Arc::new(AtomicUsize::new(0));
        let key = (Key::G, vec![]);

        let mut definitions = HashMap::new();
        definitions.insert(
            "base_oneshot".to_string(),
            make_definition(
                ModeOptions::new().oneshot(),
                vec![(key.clone(), make_registration(counter.clone()))],
            ),
        );
        definitions.insert(
            "overlay".to_string(),
            make_definition(ModeOptions::new(), vec![]),
        );

        let mut active_presses = HashMap::new();
        let dispatch =
            dispatch_mode_key_event(&key, 1, t0, &definitions, &mut stack, &mut active_presses);

        let ModeEventDispatch::Handled { callbacks, .. } = dispatch else {
            panic!("expected Handled");
        };
        dispatch_callbacks(callbacks);
        assert_eq!(counter.load(Ordering::SeqCst), 1);

        assert_eq!(stack.depth(), 1);
        assert_eq!(stack.top(), Some("overlay"));
    }

    #[test]
    fn mode_dispatch_swallow_suppresses_unmatched() {
        let mut stack = ModeStack::default();
        let t0 = Instant::now();
        stack.push("swallow_mode".to_string(), t0);

        let mut definitions = HashMap::new();
        definitions.insert(
            "swallow_mode".to_string(),
            make_definition(ModeOptions::new().swallow(), vec![]),
        );

        let unbound_key = (Key::Z, vec![]);
        let mut active_presses = HashMap::new();
        let dispatch = dispatch_mode_key_event(
            &unbound_key,
            1,
            t0,
            &definitions,
            &mut stack,
            &mut active_presses,
        );

        assert!(matches!(dispatch, ModeEventDispatch::Swallowed));
    }

    #[test]
    fn mode_dispatch_passes_through_when_no_modes_active() {
        let mut stack = ModeStack::default();
        let t0 = Instant::now();
        let key = (Key::A, vec![]);
        let mut active_presses = HashMap::new();

        let dispatch = dispatch_mode_key_event(
            &key,
            1,
            t0,
            &HashMap::new(),
            &mut stack,
            &mut active_presses,
        );

        assert!(matches!(dispatch, ModeEventDispatch::PassThrough));
    }

    #[test]
    fn mode_dispatch_passes_through_for_unmatched_key_without_swallow() {
        let mut stack = ModeStack::default();
        let t0 = Instant::now();
        stack.push("partial_mode".to_string(), t0);

        let mut definitions = HashMap::new();
        definitions.insert(
            "partial_mode".to_string(),
            make_definition(ModeOptions::new(), vec![]),
        );

        let key = (Key::Q, vec![]);
        let mut active_presses = HashMap::new();
        let dispatch =
            dispatch_mode_key_event(&key, 1, t0, &definitions, &mut stack, &mut active_presses);

        assert!(matches!(dispatch, ModeEventDispatch::PassThrough));
    }

    #[test]
    fn find_callbacks_finds_global_press() {
        let counter = Arc::new(AtomicUsize::new(0));
        let key = (Key::A, vec![crate::key::Modifier::Ctrl]);

        let mut global = HashMap::new();
        global.insert(key.clone(), make_registration(counter.clone()));

        let active = ActiveHotkeyPress {
            registration_key: key,
            origin: PressOrigin::Global,
            pressed_at: Instant::now(),
            press_dispatch_state: PressDispatchState::Dispatched,
        };

        let no_modes = HashMap::new();
        let no_devices = HashMap::new();
        let callbacks = find_callbacks_for_active_press(&active, &global, &no_modes, &no_devices);
        assert!(callbacks.is_some());
        (callbacks.unwrap().on_press)();
        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn find_callbacks_finds_mode_press() {
        let counter = Arc::new(AtomicUsize::new(0));
        let key = (Key::H, vec![]);

        let mut definitions = HashMap::new();
        definitions.insert(
            "resize".to_string(),
            make_definition(
                ModeOptions::new(),
                vec![(key.clone(), make_registration(counter.clone()))],
            ),
        );

        let active = ActiveHotkeyPress {
            registration_key: key,
            origin: PressOrigin::Mode("resize".to_string()),
            pressed_at: Instant::now(),
            press_dispatch_state: PressDispatchState::Dispatched,
        };

        let no_global = HashMap::new();
        let no_devices = HashMap::new();
        let callbacks =
            find_callbacks_for_active_press(&active, &no_global, &definitions, &no_devices);
        assert!(callbacks.is_some());
        (callbacks.unwrap().on_press)();
        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }
}
