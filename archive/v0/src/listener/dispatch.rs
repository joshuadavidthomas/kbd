use std::collections::HashMap;
use std::collections::HashSet;
use std::time::Instant;

use crate::device::DeviceInfo;
use crate::key::Key;
use crate::key::Modifier;
use crate::manager::normalize_modifiers;
use crate::manager::ActiveHotkeyPress;
use crate::manager::Callback;
use crate::manager::DeviceHotkeyRegistration;
use crate::manager::DeviceRegistrationId;
use crate::manager::HotkeyKey;
use crate::manager::HotkeyRegistration;
use crate::manager::PressDispatchState;
use crate::manager::PressOrigin;
use crate::manager::RepeatBehavior;
use crate::mode::find_callbacks_for_active_press;
use crate::mode::ModeDefinition;

// SMELL: why is this here?
pub(crate) fn active_modifier_signature(active: &HashSet<Modifier>) -> Vec<Modifier> {
    normalize_modifiers(&active.iter().copied().collect::<Vec<_>>())
}

// SMELL: what is this, why is this here? why panic catch_unwind, why is it used in the listener.rs
// and here and that's it?
pub(crate) fn invoke_callback(callback: &Callback) -> bool {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        callback();
    }))
    .is_err()
}

pub(crate) fn dispatch_callbacks(callbacks: Vec<Callback>) {
    for callback in callbacks {
        if invoke_callback(&callback) {
            tracing::error!("Hotkey callback panicked; listener continues");
        }
    }
}

pub(crate) fn collect_callbacks_for_synthetic_keys(
    synthetic_keys: &[HotkeyKey],
    registrations: &HashMap<HotkeyKey, HotkeyRegistration>,
) -> Vec<Callback> {
    synthetic_keys
        .iter()
        .filter_map(|key| registrations.get(key))
        .map(|registration| registration.callbacks.on_press.clone())
        .collect()
}

pub(crate) fn collect_due_hold_callbacks(
    now: Instant,
    registrations: &HashMap<HotkeyKey, HotkeyRegistration>,
    mode_definitions: &HashMap<String, ModeDefinition>,
    device_registrations: &HashMap<DeviceRegistrationId, DeviceHotkeyRegistration>,
    active_presses: &mut HashMap<Key, ActiveHotkeyPress>,
) -> Vec<Callback> {
    let mut callbacks = Vec::new();

    for active in active_presses.values_mut() {
        if active.press_dispatch_state != PressDispatchState::Pending {
            continue;
        }

        let Some(hotkey_callbacks) = find_callbacks_for_active_press(
            active,
            registrations,
            mode_definitions,
            device_registrations,
        ) else {
            continue;
        };

        let Some(min_hold) = hotkey_callbacks.min_hold else {
            continue;
        };

        if now.duration_since(active.pressed_at) >= min_hold {
            callbacks.push(hotkey_callbacks.on_press.clone());
            active.press_dispatch_state = PressDispatchState::Dispatched;
        }
    }

    callbacks
}

pub(crate) struct DeviceSpecificDispatch {
    pub(crate) callbacks: Vec<Callback>,
    // SMELL: bool fields
    pub(crate) matched: bool,
    pub(crate) passthrough: bool,
}

pub(crate) fn collect_device_specific_dispatch(
    key: Key,
    value: i32,
    now: Instant,
    device_info: &DeviceInfo,
    device_modifiers: &HashSet<Modifier>,
    device_registrations: &HashMap<DeviceRegistrationId, DeviceHotkeyRegistration>,
    active_presses: &mut HashMap<Key, ActiveHotkeyPress>,
) -> DeviceSpecificDispatch {
    let modifier_signature = active_modifier_signature(device_modifiers);
    let device_hotkey_key = (key, modifier_signature);

    let matching = device_registrations
        .iter()
        .find(|(_, reg)| reg.hotkey_key == device_hotkey_key && reg.filter.matches(device_info));

    let Some((reg_id, registration)) = matching else {
        return DeviceSpecificDispatch {
            callbacks: Vec::new(),
            matched: false,
            passthrough: false,
        };
    };

    let reg_id = *reg_id;
    let mut callbacks = Vec::new();
    let passthrough = registration.callbacks.passthrough;

    match value {
        1 => {
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

            active_presses.insert(
                key,
                ActiveHotkeyPress {
                    registration_key: device_hotkey_key,
                    origin: PressOrigin::Device(reg_id),
                    pressed_at: now,
                    press_dispatch_state,
                },
            );

            if press_dispatch_state == PressDispatchState::Dispatched {
                callbacks.push(registration.callbacks.on_press.clone());
            }
        }
        0 => {
            if let Some(active) = active_presses.remove(&key) {
                if matches!(active.origin, PressOrigin::Device(id) if id == reg_id) {
                    if active.press_dispatch_state == PressDispatchState::Pending {
                        if let Some(min_hold) = registration.callbacks.min_hold {
                            if now.duration_since(active.pressed_at) >= min_hold {
                                callbacks.push(registration.callbacks.on_press.clone());
                            }
                        }
                    }

                    if let Some(callback) = &registration.callbacks.on_release {
                        callbacks.push(callback.clone());
                    }
                }
            }
        }
        2 => {
            if let Some(active) = active_presses.get_mut(&key) {
                if matches!(active.origin, PressOrigin::Device(id) if id == reg_id) {
                    let hold_satisfied = registration
                        .callbacks
                        .min_hold
                        .is_none_or(|min_hold| now.duration_since(active.pressed_at) >= min_hold);

                    if registration.callbacks.repeat_behavior == RepeatBehavior::Trigger
                        && hold_satisfied
                    {
                        callbacks.push(registration.callbacks.on_press.clone());
                        active.press_dispatch_state = PressDispatchState::Dispatched;
                    }
                }
            }
        }
        _ => {}
    }

    DeviceSpecificDispatch {
        callbacks,
        matched: true,
        passthrough,
    }
}

pub(crate) struct NonModifierDispatch {
    pub(crate) callbacks: Vec<Callback>,
    // SMELL: bool fields, also duplcation with above?
    pub(crate) matched_hotkey: bool,
    pub(crate) passthrough: bool,
}

pub(crate) fn should_forward_key_event_in_grab_mode(
    grab_enabled: bool,
    matched_hotkey: bool,
    passthrough: bool,
) -> bool {
    grab_enabled && (!matched_hotkey || passthrough)
}

pub(crate) fn suppress_sequence_followup_key_event(
    suppressed_keys: &mut HashSet<Key>,
    key: Key,
    value: i32,
    suppress_current_key_press: bool,
) -> bool {
    if value == 1 && suppress_current_key_press {
        suppressed_keys.insert(key);
    }

    let suppress_followup = value != 1 && suppressed_keys.contains(&key);
    if value == 0 && suppress_followup {
        suppressed_keys.remove(&key);
    }

    suppress_followup
}

pub(crate) fn collect_non_modifier_dispatch(
    key: Key,
    value: i32,
    now: Instant,
    active_modifiers: &HashSet<Modifier>,
    registrations: &HashMap<HotkeyKey, HotkeyRegistration>,
    active_presses: &mut HashMap<Key, ActiveHotkeyPress>,
    suppress_press: bool,
) -> NonModifierDispatch {
    let mut callbacks = Vec::new();
    let mut matched_hotkey = suppress_press;
    let mut passthrough = false;

    match value {
        1 => {
            if suppress_press {
                return NonModifierDispatch {
                    callbacks,
                    matched_hotkey,
                    passthrough,
                };
            }

            let modifier_signature = active_modifier_signature(active_modifiers);
            let registration_key = (key, modifier_signature);

            if let Some(registration) = registrations.get(&registration_key) {
                matched_hotkey = true;
                passthrough = registration.callbacks.passthrough;

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

                active_presses.insert(
                    key,
                    ActiveHotkeyPress {
                        registration_key,
                        origin: PressOrigin::Global,
                        pressed_at: now,
                        press_dispatch_state,
                    },
                );

                if press_dispatch_state == PressDispatchState::Dispatched {
                    callbacks.push(registration.callbacks.on_press.clone());
                }
            }
        }
        0 => {
            if let Some(active) = active_presses.remove(&key) {
                if let Some(registration) = registrations.get(&active.registration_key) {
                    matched_hotkey = true;
                    passthrough = registration.callbacks.passthrough;

                    if active.press_dispatch_state == PressDispatchState::Pending {
                        if let Some(min_hold) = registration.callbacks.min_hold {
                            if now.duration_since(active.pressed_at) >= min_hold {
                                callbacks.push(registration.callbacks.on_press.clone());
                            }
                        }
                    }

                    if let Some(callback) = &registration.callbacks.on_release {
                        callbacks.push(callback.clone());
                    }
                }
            }
        }
        2 => {
            if let Some(active) = active_presses.get_mut(&key) {
                if let Some(registration) = registrations.get(&active.registration_key) {
                    matched_hotkey = true;
                    passthrough = registration.callbacks.passthrough;

                    let hold_satisfied = registration
                        .callbacks
                        .min_hold
                        .is_none_or(|min_hold| now.duration_since(active.pressed_at) >= min_hold);

                    if registration.callbacks.repeat_behavior == RepeatBehavior::Trigger
                        && hold_satisfied
                    {
                        callbacks.push(registration.callbacks.on_press.clone());
                        active.press_dispatch_state = PressDispatchState::Dispatched;
                    }
                }
            }
        }
        _ => {}
    }

    NonModifierDispatch {
        callbacks,
        matched_hotkey,
        passthrough,
    }
}

// SMELL: wtf is this
#[cfg(test)]
pub(super) fn collect_non_modifier_callbacks(
    key: Key,
    value: i32,
    now: Instant,
    active_modifiers: &HashSet<Modifier>,
    registrations: &HashMap<HotkeyKey, HotkeyRegistration>,
    active_presses: &mut HashMap<Key, ActiveHotkeyPress>,
    suppress_press: bool,
) -> Vec<Callback> {
    collect_non_modifier_dispatch(
        key,
        value,
        now,
        active_modifiers,
        registrations,
        active_presses,
        suppress_press,
    )
    .callbacks
}
