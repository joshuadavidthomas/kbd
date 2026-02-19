use crate::error::Error;
use crate::manager::{normalize_modifiers, HotkeyKey, HotkeyRegistration};

use evdev::{Device, EventSummary, KeyCode};
use std::collections::{HashMap, HashSet};
use std::os::unix::io::AsRawFd;
use std::path::PathBuf;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use std::thread::{self, JoinHandle};
use std::time::Duration;

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

fn invoke_callback(callback: &Arc<dyn Fn() + Send + Sync>) -> bool {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        callback();
    }))
    .is_err()
}

fn listener_loop(
    mut devices: Vec<Device>,
    registrations: Arc<Mutex<HashMap<HotkeyKey, HotkeyRegistration>>>,
    stop_flag: Arc<AtomicBool>,
) {
    // Modifier keys to track
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
    .cloned()
    .collect();

    let mut active_modifiers: HashSet<KeyCode> = HashSet::new();

    loop {
        if stop_flag.load(Ordering::SeqCst) {
            return;
        }

        for device in &mut devices {
            if let Ok(events) = device.fetch_events() {
                for event in events {
                    if let EventSummary::Key(_, key, value) = event.destructure() {
                        // Track modifier state
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

                        // Check for registered hotkeys on key press (value == 1)
                        if value == 1 {
                            // Clone the callback while holding the lock, then release
                            // the lock before invoking it. This prevents deadlocks if
                            // the callback calls register/unregister.
                            let callback = {
                                let modifier_signature =
                                    active_modifier_signature(&active_modifiers);
                                let registrations_guard = registrations.lock().unwrap();
                                registrations_guard
                                    .get(&(key, modifier_signature))
                                    .map(|registration| registration.callback.clone())
                            };

                            if let Some(callback) = callback {
                                if invoke_callback(&callback) {
                                    tracing::error!("Hotkey callback panicked; listener continues");
                                }
                            }
                        }
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
    use std::sync::atomic::{AtomicBool, Ordering};

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
        let callback: Arc<dyn Fn() + Send + Sync> = Arc::new(|| panic!("boom"));

        assert!(invoke_callback(&callback));
    }

    #[test]
    fn invoke_callback_runs_non_panicking_callback() {
        let called = Arc::new(AtomicBool::new(false));
        let called_clone = called.clone();
        let callback: Arc<dyn Fn() + Send + Sync> = Arc::new(move || {
            called_clone.store(true, Ordering::SeqCst);
        });

        assert!(!invoke_callback(&callback));
        assert!(called.load(Ordering::SeqCst));
    }
}
