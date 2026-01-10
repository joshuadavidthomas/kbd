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
    let thread = thread::spawn(move || {
        listener_loop(keyboard_paths, registrations, stop_flag);
    });

    Ok(thread)
}

/// Check if required modifiers are satisfied
fn modifiers_satisfied(required: &[KeyCode], active: &HashSet<KeyCode>) -> bool {
    // Group modifiers by type (left OR right is acceptable)
    let has_ctrl =
        active.contains(&KeyCode::KEY_LEFTCTRL) || active.contains(&KeyCode::KEY_RIGHTCTRL);
    let has_alt = active.contains(&KeyCode::KEY_LEFTALT) || active.contains(&KeyCode::KEY_RIGHTALT);
    let has_shift =
        active.contains(&KeyCode::KEY_LEFTSHIFT) || active.contains(&KeyCode::KEY_RIGHTSHIFT);
    let has_meta =
        active.contains(&KeyCode::KEY_LEFTMETA) || active.contains(&KeyCode::KEY_RIGHTMETA);

    for &modifier in required {
        match modifier {
            KeyCode::KEY_LEFTCTRL | KeyCode::KEY_RIGHTCTRL => {
                if !has_ctrl {
                    return false;
                }
            }
            KeyCode::KEY_LEFTALT | KeyCode::KEY_RIGHTALT => {
                if !has_alt {
                    return false;
                }
            }
            KeyCode::KEY_LEFTSHIFT | KeyCode::KEY_RIGHTSHIFT => {
                if !has_shift {
                    return false;
                }
            }
            KeyCode::KEY_LEFTMETA | KeyCode::KEY_RIGHTMETA => {
                if !has_meta {
                    return false;
                }
            }
            _ => {}
        }
    }

    true
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
                // Set non-blocking
                let fd = device.as_raw_fd();
                unsafe {
                    let flags = libc::fcntl(fd, libc::F_GETFL);
                    if flags != -1 {
                        libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK);
                    }
                }
                devices.push(device);
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
    fn test_modifiers_satisfied() {
        let active: HashSet<KeyCode> = [KeyCode::KEY_LEFTCTRL, KeyCode::KEY_LEFTSHIFT]
            .iter()
            .cloned()
            .collect();

        // Should match - has required modifiers
        assert!(modifiers_satisfied(
            &[KeyCode::KEY_LEFTCTRL, KeyCode::KEY_LEFTSHIFT],
            &active
        ));

        // Should match - accepts right variant when left registered
        assert!(modifiers_satisfied(&[KeyCode::KEY_LEFTCTRL], &active));

        // Should not match - missing alt
        assert!(!modifiers_satisfied(
            &[KeyCode::KEY_LEFTCTRL, KeyCode::KEY_LEFTALT],
            &active
        ));
    }

    #[test]
    fn test_modifiers_satisfied_empty() {
        let active: HashSet<KeyCode> = HashSet::new();

        // Empty required modifiers should always be satisfied
        assert!(modifiers_satisfied(&[], &active));

        // Non-empty required with empty active should not match
        assert!(!modifiers_satisfied(&[KeyCode::KEY_LEFTCTRL], &active));
    }
}
