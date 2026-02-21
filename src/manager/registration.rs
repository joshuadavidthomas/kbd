use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;

use crate::device::DeviceFilter;
use crate::error::Error;
use crate::events::EventHub;
use crate::events::HotkeyEvent;
use crate::hotkey::Hotkey;
use crate::key::Key;
use crate::key::Modifier;

use super::callbacks::Callback;
use super::callbacks::HotkeyCallbacks;
use super::callbacks::PressDispatchState;
use super::callbacks::PressInvocationLimiter;
use super::callbacks::PressTimingConfig;

/// Key used to identify hotkey registrations: (`target_key`, `normalized_modifiers`)
pub(crate) type HotkeyKey = (Key, Vec<Modifier>);

pub(crate) fn normalize_modifiers(modifiers: &[Modifier]) -> Vec<Modifier> {
    let mut normalized: Vec<Modifier> = modifiers.to_vec();
    normalized.sort();
    normalized.dedup();
    normalized
}

/// Hotkey registration with modifiers
#[derive(Clone)]
pub(crate) struct HotkeyRegistration {
    pub(crate) callbacks: HotkeyCallbacks,
}

pub(crate) type DeviceRegistrationId = u64;

#[derive(Clone)]
pub(crate) struct DeviceHotkeyRegistration {
    pub(crate) hotkey_key: HotkeyKey,
    pub(crate) filter: DeviceFilter,
    pub(crate) callbacks: HotkeyCallbacks,
}

pub(crate) type SequenceId = u64;

#[derive(Clone)]
pub(crate) struct SequenceRegistration {
    pub(crate) steps: Vec<HotkeyKey>,
    pub(crate) callback: Callback,
    pub(crate) timeout: Duration,
    pub(crate) abort_key: Key,
    pub(crate) timeout_fallback: Option<HotkeyKey>,
}

/// Where a hotkey press was matched: global registrations, a named mode, or
/// a device-specific registration.
#[derive(Clone, Debug)]
pub(crate) enum PressOrigin {
    Global,
    Mode(String),
    Device(DeviceRegistrationId),
}

pub(crate) struct ActiveHotkeyPress {
    pub(crate) registration_key: HotkeyKey,
    pub(crate) origin: PressOrigin,
    pub(crate) pressed_at: Instant,
    pub(crate) press_dispatch_state: PressDispatchState,
}

pub(crate) fn attach_hotkey_events(
    callbacks: HotkeyCallbacks,
    hotkey_key: &HotkeyKey,
    event_hub: &EventHub,
    press_timing: PressTimingConfig,
) -> HotkeyCallbacks {
    let HotkeyCallbacks {
        on_press,
        on_release,
        wait_for_release,
        min_hold,
        repeat_behavior,
        passthrough,
    } = callbacks;

    let hotkey = Hotkey::new(hotkey_key.0, hotkey_key.1.clone());

    let invocation_limiter = Arc::new(PressInvocationLimiter::new(press_timing));

    let press_event_hub = event_hub.clone();
    let press_hotkey = hotkey.clone();
    let press_limiter = invocation_limiter.clone();
    let wrapped_press: Callback = Arc::new(move || {
        if press_limiter.should_dispatch_now() {
            press_event_hub.emit(&HotkeyEvent::Pressed(press_hotkey.clone()));
            on_press();
        }
    });

    let wrapped_release = match on_release {
        Some(release_callback) => {
            let release_event_hub = event_hub.clone();
            let release_hotkey = hotkey.clone();
            let release_limiter = invocation_limiter.clone();
            Some(Arc::new(move || {
                if release_limiter.should_dispatch_now() {
                    release_event_hub.emit(&HotkeyEvent::Released(release_hotkey.clone()));
                    release_callback();
                }
            }) as Callback)
        }
        None => {
            #[cfg(any(feature = "tokio", feature = "async-std"))]
            {
                let release_event_hub = event_hub.clone();
                let release_hotkey = hotkey.clone();
                Some(Arc::new(move || {
                    release_event_hub.emit(&HotkeyEvent::Released(release_hotkey.clone()));
                }) as Callback)
            }

            #[cfg(not(any(feature = "tokio", feature = "async-std")))]
            {
                None
            }
        }
    };

    HotkeyCallbacks {
        on_press: wrapped_press,
        on_release: wrapped_release,
        wait_for_release,
        min_hold,
        repeat_behavior,
        passthrough,
    }
}

pub(crate) fn already_registered_error(hotkey_key: &HotkeyKey) -> Error {
    Error::AlreadyRegistered {
        key: hotkey_key.0,
        modifiers: hotkey_key.1.clone(),
    }
}

pub(crate) fn remove_registration_if_matches(
    registrations: &mut HashMap<HotkeyKey, HotkeyRegistration>,
    hotkey_key: &HotkeyKey,
    registration_marker: &Callback,
) -> Option<HotkeyRegistration> {
    let should_remove = registrations
        .get(hotkey_key)
        .is_some_and(|current| Arc::ptr_eq(&current.callbacks.on_press, registration_marker));

    if should_remove {
        return registrations.remove(hotkey_key);
    }

    None
}
