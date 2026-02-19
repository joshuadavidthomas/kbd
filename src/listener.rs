use crate::error::Error;
use crate::manager::{HotkeyKey, HotkeyRegistration};

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
    thread::Builder::new()
        .name("evdev-hotkey-listener".into())
        .spawn(move || {
            listener_loop(keyboard_paths, registrations, stop_flag);
        })
        .map_err(|e| Error::ThreadSpawn(format!("Failed to spawn listener thread: {}", e)))
}

/// Check if active modifiers exactly match the required modifiers.
///
/// Left and right variants are treated as equivalent (e.g., either LEFT_CTRL or
/// RIGHT_CTRL satisfies a KEY_LEFTCTRL requirement), but extra active modifiers
/// that aren't required will cause a non-match.
fn modifiers_satisfied(required: &[KeyCode], active: &HashSet<KeyCode>) -> bool {
    let has_ctrl =
        active.contains(&KeyCode::KEY_LEFTCTRL) || active.contains(&KeyCode::KEY_RIGHTCTRL);
    let has_alt =
        active.contains(&KeyCode::KEY_LEFTALT) || active.contains(&KeyCode::KEY_RIGHTALT);
    let has_shift =
        active.contains(&KeyCode::KEY_LEFTSHIFT) || active.contains(&KeyCode::KEY_RIGHTSHIFT);
    let has_meta =
        active.contains(&KeyCode::KEY_LEFTMETA) || active.contains(&KeyCode::KEY_RIGHTMETA);

    let requires_ctrl = required
        .iter()
        .any(|k| matches!(k, &KeyCode::KEY_LEFTCTRL | &KeyCode::KEY_RIGHTCTRL));
    let requires_alt = required
        .iter()
        .any(|k| matches!(k, &KeyCode::KEY_LEFTALT | &KeyCode::KEY_RIGHTALT));
    let requires_shift = required
        .iter()
        .any(|k| matches!(k, &KeyCode::KEY_LEFTSHIFT | &KeyCode::KEY_RIGHTSHIFT));
    let requires_meta = required
        .iter()
        .any(|k| matches!(k, &KeyCode::KEY_LEFTMETA | &KeyCode::KEY_RIGHTMETA));

    has_ctrl == requires_ctrl
        && has_alt == requires_alt
        && has_shift == requires_shift
        && has_meta == requires_meta
}

fn listener_loop(
    keyboard_paths: Vec<PathBuf>,
    registrations: Arc<Mutex<HashMap<HotkeyKey, HotkeyRegistration>>>,
    stop_flag: Arc<AtomicBool>,
) {
    // Open devices
    let mut devices: Vec<Device> = Vec::new();

    for path in keyboard_paths {
        match Device::open(&path) {
            Ok(device) => {
                let fd = device.as_raw_fd();
                let nonblock_ok = unsafe {
                    let flags = libc::fcntl(fd, libc::F_GETFL);
                    flags != -1
                        && libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) != -1
                };
                if nonblock_ok {
                    devices.push(device);
                } else {
                    tracing::warn!("Failed to set non-blocking mode for {:?}", path);
                }
            }
            Err(e) => {
                tracing::warn!("Failed to open {:?}: {}", path, e);
            }
        }
    }

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
                            let registrations_guard = registrations.lock().unwrap();

                            // Find matching registration by checking all entries with this target key
                            for ((target_key, required_modifiers), registration) in
                                registrations_guard.iter()
                            {
                                if *target_key == key {
                                    // Check if active modifiers satisfy the required modifiers
                                    if modifiers_satisfied(required_modifiers, &active_modifiers) {
                                        (registration.callback)();
                                        break; // Only trigger first matching hotkey
                                    }
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

    #[test]
    fn test_modifiers_exact_match() {
        let active: HashSet<KeyCode> = [KeyCode::KEY_LEFTCTRL, KeyCode::KEY_LEFTSHIFT]
            .iter()
            .cloned()
            .collect();

        // Exact match
        assert!(modifiers_satisfied(
            &[KeyCode::KEY_LEFTCTRL, KeyCode::KEY_LEFTSHIFT],
            &active
        ));

        // Missing required modifier (alt not active)
        assert!(!modifiers_satisfied(
            &[KeyCode::KEY_LEFTCTRL, KeyCode::KEY_LEFTALT],
            &active
        ));

        // Extra active modifier (shift active but not required) — must NOT match
        assert!(!modifiers_satisfied(&[KeyCode::KEY_LEFTCTRL], &active));
    }

    #[test]
    fn test_modifiers_right_variant_satisfies_left() {
        let active: HashSet<KeyCode> = [KeyCode::KEY_RIGHTCTRL].iter().cloned().collect();

        // Right variant satisfies left requirement
        assert!(modifiers_satisfied(&[KeyCode::KEY_LEFTCTRL], &active));
    }

    #[test]
    fn test_modifiers_empty() {
        let active: HashSet<KeyCode> = HashSet::new();

        // No modifiers required, none active
        assert!(modifiers_satisfied(&[], &active));

        // Modifier required but none active
        assert!(!modifiers_satisfied(&[KeyCode::KEY_LEFTCTRL], &active));
    }

    #[test]
    fn test_modifiers_none_required_but_active() {
        let active: HashSet<KeyCode> = [KeyCode::KEY_LEFTCTRL].iter().cloned().collect();

        // No modifiers required but ctrl is active — must NOT match
        assert!(!modifiers_satisfied(&[], &active));
    }
}
