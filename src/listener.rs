use crate::error::Error;
use crate::manager::{
    normalize_modifiers, ActiveHotkeyPress, HotkeyKey, HotkeyRegistration, PressDispatchState,
    RepeatBehavior,
};

use evdev::{Device, EventSummary, KeyCode};
use std::collections::{HashMap, HashSet};
use std::os::unix::io::AsRawFd;
use std::path::PathBuf;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

type Callback = Arc<dyn Fn() + Send + Sync>;

pub fn spawn_listener_thread(
    keyboard_paths: Vec<PathBuf>,
    registrations: Arc<Mutex<HashMap<HotkeyKey, HotkeyRegistration>>>,
    stop_flag: Arc<AtomicBool>,
) -> Result<JoinHandle<()>, Error> {
    let devices = open_devices(keyboard_paths)?;

    thread::Builder::new()
        .name("evdev-hotkey-listener".into())
        .spawn(move || {
            listener_loop(devices, registrations, stop_flag);
        })
        .map_err(|e| Error::ThreadSpawn(format!("Failed to spawn listener thread: {}", e)))
}

fn open_devices(keyboard_paths: Vec<PathBuf>) -> Result<Vec<Device>, Error> {
    let mut devices: Vec<Device> = Vec::new();
    let mut last_error: Option<String> = None;

    for path in keyboard_paths {
        match Device::open(&path) {
            Ok(device) => {
                let fd = device.as_raw_fd();
                let nonblock_ok = unsafe {
                    let flags = libc::fcntl(fd, libc::F_GETFL);
                    flags != -1 && libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) != -1
                };
                if nonblock_ok {
                    devices.push(device);
                } else {
                    last_error = Some(format!("Failed to set non-blocking mode for {:?}", path));
                    tracing::warn!("Failed to set non-blocking mode for {:?}", path);
                }
            }
            Err(e) => {
                last_error = Some(format!("Failed to open {:?}: {}", path, e));
                tracing::warn!("Failed to open {:?}: {}", path, e);
            }
        }
    }

    if devices.is_empty() {
        return Err(Error::DeviceAccess(last_error.unwrap_or_else(|| {
            "Failed to open any keyboard devices for listening".into()
        })));
    }

    Ok(devices)
}

fn active_modifier_signature(active: &HashSet<KeyCode>) -> Vec<KeyCode> {
    let modifiers: Vec<KeyCode> = active.iter().copied().collect();
    normalize_modifiers(&modifiers)
}

fn invoke_callback(callback: &Callback) -> bool {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        callback();
    }))
    .is_err()
}

fn dispatch_callbacks(callbacks: Vec<Callback>) {
    for callback in callbacks {
        if invoke_callback(&callback) {
            tracing::error!("Hotkey callback panicked; listener continues");
        }
    }
}

fn collect_non_modifier_callbacks(
    key: KeyCode,
    value: i32,
    now: Instant,
    active_modifiers: &HashSet<KeyCode>,
    registrations: &HashMap<HotkeyKey, HotkeyRegistration>,
    active_presses: &mut HashMap<KeyCode, ActiveHotkeyPress>,
) -> Vec<Callback> {
    let mut callbacks = Vec::new();

    match value {
        1 => {
            let modifier_signature = active_modifier_signature(active_modifiers);
            let registration_key = (key, modifier_signature);

            if let Some(registration) = registrations.get(&registration_key) {
                let press_dispatch_state = registration
                    .callbacks
                    .min_hold
                    .map(|min_hold| {
                        if min_hold.is_zero() {
                            PressDispatchState::Dispatched
                        } else {
                            PressDispatchState::Pending
                        }
                    })
                    .unwrap_or(PressDispatchState::Dispatched);

                active_presses.insert(
                    key,
                    ActiveHotkeyPress {
                        registration_key,
                        pressed_at: now,
                        press_dispatch_state,
                    },
                );

                if press_dispatch_state == PressDispatchState::Dispatched {
                    if let Some(callback) = &registration.callbacks.on_press {
                        callbacks.push(callback.clone());
                    }
                }
            }
        }
        0 => {
            if let Some(active) = active_presses.remove(&key) {
                if let Some(registration) = registrations.get(&active.registration_key) {
                    if active.press_dispatch_state == PressDispatchState::Pending {
                        if let Some(min_hold) = registration.callbacks.min_hold {
                            if now.duration_since(active.pressed_at) >= min_hold {
                                if let Some(callback) = &registration.callbacks.on_press {
                                    callbacks.push(callback.clone());
                                }
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
                    let hold_satisfied = registration
                        .callbacks
                        .min_hold
                        .map(|min_hold| now.duration_since(active.pressed_at) >= min_hold)
                        .unwrap_or(true);

                    if registration.callbacks.repeat_behavior == RepeatBehavior::Trigger
                        && hold_satisfied
                    {
                        if let Some(callback) = &registration.callbacks.on_press {
                            callbacks.push(callback.clone());
                            active.press_dispatch_state = PressDispatchState::Dispatched;
                        }
                    }
                }
            }
        }
        _ => {}
    }

    callbacks
}

fn listener_loop(
    mut devices: Vec<Device>,
    registrations: Arc<Mutex<HashMap<HotkeyKey, HotkeyRegistration>>>,
    stop_flag: Arc<AtomicBool>,
) {
    let modifier_keys: HashSet<KeyCode> = [
        KeyCode::KEY_LEFTCTRL,
        KeyCode::KEY_RIGHTCTRL,
        KeyCode::KEY_LEFTMETA,
        KeyCode::KEY_RIGHTMETA,
        KeyCode::KEY_LEFTSHIFT,
        KeyCode::KEY_RIGHTSHIFT,
        KeyCode::KEY_LEFTALT,
        KeyCode::KEY_RIGHTALT,
    ]
    .iter()
    .copied()
    .collect();

    let mut active_modifiers: HashSet<KeyCode> = HashSet::new();
    let mut active_presses: HashMap<KeyCode, ActiveHotkeyPress> = HashMap::new();

    loop {
        if stop_flag.load(Ordering::SeqCst) {
            return;
        }

        for device in &mut devices {
            if let Ok(events) = device.fetch_events() {
                for event in events {
                    if let EventSummary::Key(_, key, value) = event.destructure() {
                        if modifier_keys.contains(&key) {
                            match value {
                                1 => {
                                    active_modifiers.insert(key);
                                }
                                0 => {
                                    active_modifiers.remove(&key);
                                }
                                _ => {}
                            }
                            continue;
                        }

                        let callbacks = {
                            let registrations_guard = registrations.lock().unwrap();
                            collect_non_modifier_callbacks(
                                key,
                                value,
                                Instant::now(),
                                &active_modifiers,
                                &registrations_guard,
                                &mut active_presses,
                            )
                        };

                        dispatch_callbacks(callbacks);
                    }
                }
            }
        }

        thread::sleep(Duration::from_millis(5));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manager::{HotkeyCallbacks, RepeatBehavior};
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

    #[test]
    fn modifier_signature_normalizes_left_and_right() {
        let active: HashSet<KeyCode> = [KeyCode::KEY_RIGHTCTRL, KeyCode::KEY_LEFTSHIFT]
            .iter()
            .copied()
            .collect();

        let signature = active_modifier_signature(&active);
        assert_eq!(
            signature,
            vec![KeyCode::KEY_LEFTCTRL, KeyCode::KEY_LEFTSHIFT]
        );
    }

    #[test]
    fn empty_modifier_signature_is_empty() {
        let active = HashSet::new();
        assert!(active_modifier_signature(&active).is_empty());
    }

    #[test]
    fn invoke_callback_reports_panic_without_propagating() {
        let callback: Callback = Arc::new(|| panic!("boom"));

        assert!(invoke_callback(&callback));
    }

    #[test]
    fn invoke_callback_runs_non_panicking_callback() {
        let called = Arc::new(AtomicBool::new(false));
        let called_clone = called.clone();
        let callback: Callback = Arc::new(move || {
            called_clone.store(true, Ordering::SeqCst);
        });

        assert!(!invoke_callback(&callback));
        assert!(called.load(Ordering::SeqCst));
    }

    #[test]
    fn release_callback_runs_after_press() {
        let press_count = Arc::new(AtomicUsize::new(0));
        let release_count = Arc::new(AtomicUsize::new(0));
        let p = press_count.clone();
        let r = release_count.clone();

        let callbacks = HotkeyCallbacks {
            on_press: Some(Arc::new(move || {
                p.fetch_add(1, Ordering::SeqCst);
            })),
            on_release: Some(Arc::new(move || {
                r.fetch_add(1, Ordering::SeqCst);
            })),
            min_hold: None,
            repeat_behavior: RepeatBehavior::Ignore,
        };

        let mut registrations = HashMap::new();
        registrations.insert(
            (KeyCode::KEY_A, vec![KeyCode::KEY_LEFTCTRL]),
            HotkeyRegistration { callbacks },
        );

        let modifiers: HashSet<KeyCode> = [KeyCode::KEY_LEFTCTRL].into_iter().collect();
        let mut active_presses = HashMap::new();
        let t0 = Instant::now();

        dispatch_callbacks(collect_non_modifier_callbacks(
            KeyCode::KEY_A,
            1,
            t0,
            &modifiers,
            &registrations,
            &mut active_presses,
        ));
        dispatch_callbacks(collect_non_modifier_callbacks(
            KeyCode::KEY_A,
            0,
            t0 + Duration::from_millis(10),
            &modifiers,
            &registrations,
            &mut active_presses,
        ));

        assert_eq!(press_count.load(Ordering::SeqCst), 1);
        assert_eq!(release_count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn min_hold_delays_press_callback_until_release() {
        let press_count = Arc::new(AtomicUsize::new(0));
        let p = press_count.clone();

        let callbacks = HotkeyCallbacks {
            on_press: Some(Arc::new(move || {
                p.fetch_add(1, Ordering::SeqCst);
            })),
            on_release: None,
            min_hold: Some(Duration::from_millis(50)),
            repeat_behavior: RepeatBehavior::Ignore,
        };

        let mut registrations = HashMap::new();
        registrations.insert((KeyCode::KEY_A, vec![]), HotkeyRegistration { callbacks });

        let modifiers = HashSet::new();
        let mut active_presses = HashMap::new();
        let t0 = Instant::now();

        dispatch_callbacks(collect_non_modifier_callbacks(
            KeyCode::KEY_A,
            1,
            t0,
            &modifiers,
            &registrations,
            &mut active_presses,
        ));
        dispatch_callbacks(collect_non_modifier_callbacks(
            KeyCode::KEY_A,
            0,
            t0 + Duration::from_millis(20),
            &modifiers,
            &registrations,
            &mut active_presses,
        ));
        dispatch_callbacks(collect_non_modifier_callbacks(
            KeyCode::KEY_A,
            1,
            t0,
            &modifiers,
            &registrations,
            &mut active_presses,
        ));
        dispatch_callbacks(collect_non_modifier_callbacks(
            KeyCode::KEY_A,
            0,
            t0 + Duration::from_millis(70),
            &modifiers,
            &registrations,
            &mut active_presses,
        ));

        assert_eq!(press_count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn repeat_event_respects_min_hold_threshold() {
        let press_count = Arc::new(AtomicUsize::new(0));
        let p = press_count.clone();

        let callbacks = HotkeyCallbacks {
            on_press: Some(Arc::new(move || {
                p.fetch_add(1, Ordering::SeqCst);
            })),
            on_release: None,
            min_hold: Some(Duration::from_millis(50)),
            repeat_behavior: RepeatBehavior::Trigger,
        };

        let mut registrations = HashMap::new();
        registrations.insert((KeyCode::KEY_A, vec![]), HotkeyRegistration { callbacks });

        let modifiers = HashSet::new();
        let mut active_presses = HashMap::new();
        let now = Instant::now();

        dispatch_callbacks(collect_non_modifier_callbacks(
            KeyCode::KEY_A,
            1,
            now,
            &modifiers,
            &registrations,
            &mut active_presses,
        ));

        dispatch_callbacks(collect_non_modifier_callbacks(
            KeyCode::KEY_A,
            2,
            now + Duration::from_millis(20),
            &modifiers,
            &registrations,
            &mut active_presses,
        ));

        dispatch_callbacks(collect_non_modifier_callbacks(
            KeyCode::KEY_A,
            2,
            now + Duration::from_millis(60),
            &modifiers,
            &registrations,
            &mut active_presses,
        ));

        assert_eq!(press_count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn zero_min_hold_triggers_press_on_key_down_only_once() {
        let press_count = Arc::new(AtomicUsize::new(0));
        let p = press_count.clone();

        let callbacks = HotkeyCallbacks {
            on_press: Some(Arc::new(move || {
                p.fetch_add(1, Ordering::SeqCst);
            })),
            on_release: None,
            min_hold: Some(Duration::ZERO),
            repeat_behavior: RepeatBehavior::Ignore,
        };

        let mut registrations = HashMap::new();
        registrations.insert((KeyCode::KEY_A, vec![]), HotkeyRegistration { callbacks });

        let modifiers = HashSet::new();
        let mut active_presses = HashMap::new();
        let now = Instant::now();

        dispatch_callbacks(collect_non_modifier_callbacks(
            KeyCode::KEY_A,
            1,
            now,
            &modifiers,
            &registrations,
            &mut active_presses,
        ));
        dispatch_callbacks(collect_non_modifier_callbacks(
            KeyCode::KEY_A,
            0,
            now + Duration::from_millis(1),
            &modifiers,
            &registrations,
            &mut active_presses,
        ));

        assert_eq!(press_count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn repeat_after_hold_does_not_double_fire_on_release() {
        let press_count = Arc::new(AtomicUsize::new(0));
        let p = press_count.clone();

        let callbacks = HotkeyCallbacks {
            on_press: Some(Arc::new(move || {
                p.fetch_add(1, Ordering::SeqCst);
            })),
            on_release: None,
            min_hold: Some(Duration::from_millis(50)),
            repeat_behavior: RepeatBehavior::Trigger,
        };

        let mut registrations = HashMap::new();
        registrations.insert((KeyCode::KEY_A, vec![]), HotkeyRegistration { callbacks });

        let modifiers = HashSet::new();
        let mut active_presses = HashMap::new();
        let now = Instant::now();

        dispatch_callbacks(collect_non_modifier_callbacks(
            KeyCode::KEY_A,
            1,
            now,
            &modifiers,
            &registrations,
            &mut active_presses,
        ));
        dispatch_callbacks(collect_non_modifier_callbacks(
            KeyCode::KEY_A,
            2,
            now + Duration::from_millis(60),
            &modifiers,
            &registrations,
            &mut active_presses,
        ));
        dispatch_callbacks(collect_non_modifier_callbacks(
            KeyCode::KEY_A,
            0,
            now + Duration::from_millis(70),
            &modifiers,
            &registrations,
            &mut active_presses,
        ));

        assert_eq!(press_count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn repeat_event_respects_trigger_option() {
        let press_count = Arc::new(AtomicUsize::new(0));
        let p = press_count.clone();

        let callbacks = HotkeyCallbacks {
            on_press: Some(Arc::new(move || {
                p.fetch_add(1, Ordering::SeqCst);
            })),
            on_release: None,
            min_hold: None,
            repeat_behavior: RepeatBehavior::Trigger,
        };

        let mut registrations = HashMap::new();
        registrations.insert((KeyCode::KEY_A, vec![]), HotkeyRegistration { callbacks });

        let modifiers = HashSet::new();
        let mut active_presses = HashMap::new();
        let now = Instant::now();

        dispatch_callbacks(collect_non_modifier_callbacks(
            KeyCode::KEY_A,
            1,
            now,
            &modifiers,
            &registrations,
            &mut active_presses,
        ));
        dispatch_callbacks(collect_non_modifier_callbacks(
            KeyCode::KEY_A,
            2,
            now + Duration::from_millis(1),
            &modifiers,
            &registrations,
            &mut active_presses,
        ));

        assert_eq!(press_count.load(Ordering::SeqCst), 2);
    }
}
