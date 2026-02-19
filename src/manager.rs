use crate::backend::{build_backend, resolve_backend, Backend};
use crate::error::Error;

use evdev::KeyCode;
use std::collections::HashMap;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use std::thread::JoinHandle;

/// Callback storage type
type Callback = Arc<dyn Fn() + Send + Sync>;

/// Hotkey registration with modifiers
pub(crate) struct HotkeyRegistration {
    pub(crate) callback: Callback,
}

/// Key used to identify hotkey registrations: (target_key, normalized_modifiers)
pub type HotkeyKey = (KeyCode, Vec<KeyCode>);

fn canonical_modifier(key: KeyCode) -> KeyCode {
    match key {
        KeyCode::KEY_LEFTCTRL | KeyCode::KEY_RIGHTCTRL => KeyCode::KEY_LEFTCTRL,
        KeyCode::KEY_LEFTALT | KeyCode::KEY_RIGHTALT => KeyCode::KEY_LEFTALT,
        KeyCode::KEY_LEFTSHIFT | KeyCode::KEY_RIGHTSHIFT => KeyCode::KEY_LEFTSHIFT,
        KeyCode::KEY_LEFTMETA | KeyCode::KEY_RIGHTMETA => KeyCode::KEY_LEFTMETA,
        _ => key,
    }
}

pub(crate) fn normalize_modifiers(modifiers: &[KeyCode]) -> Vec<KeyCode> {
    let mut normalized: Vec<KeyCode> = modifiers.iter().copied().map(canonical_modifier).collect();
    normalized.sort();
    normalized.dedup();
    normalized
}

/// Handle for unregistering a specific hotkey
#[derive(Clone)]
pub struct Handle {
    key: HotkeyKey,
    manager: Arc<HotkeyManagerInner>,
}

impl std::fmt::Debug for Handle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Handle").field("key", &self.key).finish()
    }
}

impl Handle {
    pub fn unregister(self) -> Result<(), Error> {
        self.manager.remove_hotkey(&self.key)
    }
}

/// Inner state shared between HotkeyManager and Handles
struct HotkeyManagerInner {
    registrations: Arc<Mutex<HashMap<HotkeyKey, HotkeyRegistration>>>,
    stop_flag: Arc<AtomicBool>,
    listener: Mutex<Option<JoinHandle<()>>>,
}

/// Global hotkey manager for Linux
pub struct HotkeyManager {
    inner: Arc<HotkeyManagerInner>,
    active_backend: Backend,
}

impl HotkeyManager {
    /// Create a new hotkey manager
    pub fn new() -> Result<Self, Error> {
        Self::with_backend_internal(None)
    }

    /// Create a manager with an explicit backend.
    pub fn with_backend(backend: Backend) -> Result<Self, Error> {
        Self::with_backend_internal(Some(backend))
    }

    /// Returns the backend selected for this manager instance.
    pub fn active_backend(&self) -> Backend {
        self.active_backend
    }

    fn with_backend_internal(requested_backend: Option<Backend>) -> Result<Self, Error> {
        let selected_backend = resolve_backend(requested_backend)?;
        let backend_impl = build_backend(selected_backend)?;

        let inner = Arc::new(HotkeyManagerInner {
            registrations: Arc::new(Mutex::new(HashMap::new())),
            stop_flag: Arc::new(AtomicBool::new(false)),
            listener: Mutex::new(None),
        });

        let listener =
            backend_impl.start_listener(inner.registrations.clone(), inner.stop_flag.clone())?;

        *inner.listener.lock().unwrap() = Some(listener);

        Ok(HotkeyManager {
            inner,
            active_backend: selected_backend,
        })
    }

    /// Register a hotkey with a callback
    pub fn register<F>(
        &self,
        key: KeyCode,
        modifiers: &[KeyCode],
        callback: F,
    ) -> Result<Handle, Error>
    where
        F: Fn() + Send + Sync + 'static,
    {
        let callback = Arc::new(callback);
        let hotkey_key = (key, normalize_modifiers(modifiers));

        {
            let mut registrations = self.inner.registrations.lock().unwrap();
            registrations.insert(hotkey_key.clone(), HotkeyRegistration { callback });
        }

        Ok(Handle {
            key: hotkey_key,
            manager: self.inner.clone(),
        })
    }

    /// Unregister all hotkeys and stop the listener
    pub fn unregister_all(&self) -> Result<(), Error> {
        self.inner.registrations.lock().unwrap().clear();
        self.inner.stop_flag.store(true, Ordering::SeqCst);

        if let Some(listener) = self.inner.listener.lock().unwrap().take() {
            listener.join().map_err(|e| {
                Error::ThreadSpawn(format!("Failed to join listener thread: {:?}", e))
            })?;
        }

        Ok(())
    }
}

impl Drop for HotkeyManager {
    fn drop(&mut self) {
        let _ = self.unregister_all();
    }
}

// Private methods for Handle
impl HotkeyManagerInner {
    fn remove_hotkey(&self, key: &HotkeyKey) -> Result<(), Error> {
        self.registrations.lock().unwrap().remove(key);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_left_and_right_variants() {
        let normalized = normalize_modifiers(&[
            KeyCode::KEY_RIGHTCTRL,
            KeyCode::KEY_LEFTCTRL,
            KeyCode::KEY_RIGHTSHIFT,
        ]);

        assert_eq!(
            normalized,
            vec![KeyCode::KEY_LEFTCTRL, KeyCode::KEY_LEFTSHIFT]
        );
    }
}
