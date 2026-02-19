use crate::backend::{build_backend, resolve_backend, Backend};
use crate::error::Error;

use evdev::KeyCode;
use std::collections::HashMap;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/// Callback storage type
type Callback = Arc<dyn Fn() + Send + Sync>;

#[derive(Clone, Default)]
pub(crate) enum ReleaseBehavior {
    #[default]
    Disabled,
    SameAsPress,
    Custom(Callback),
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum RepeatBehavior {
    #[default]
    Ignore,
    Trigger,
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum PressDispatchState {
    #[default]
    Pending,
    Dispatched,
}

#[derive(Clone)]
pub(crate) struct HotkeyCallbacks {
    pub(crate) on_press: Callback,
    pub(crate) on_release: Option<Callback>,
    pub(crate) min_hold: Option<Duration>,
    pub(crate) repeat_behavior: RepeatBehavior,
}

#[derive(Clone, Default)]
pub struct HotkeyOptions {
    release_behavior: ReleaseBehavior,
    min_hold: Option<Duration>,
    repeat_behavior: RepeatBehavior,
}

impl HotkeyOptions {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn on_release(mut self) -> Self {
        self.release_behavior = ReleaseBehavior::SameAsPress;
        self
    }

    pub fn on_release_callback<F>(mut self, callback: F) -> Self
    where
        F: Fn() + Send + Sync + 'static,
    {
        self.release_behavior = ReleaseBehavior::Custom(Arc::new(callback));
        self
    }

    pub fn min_hold(mut self, min_hold: Duration) -> Self {
        self.min_hold = Some(min_hold);
        self
    }

    pub fn trigger_on_repeat(mut self, trigger_on_repeat: bool) -> Self {
        self.repeat_behavior = if trigger_on_repeat {
            RepeatBehavior::Trigger
        } else {
            RepeatBehavior::Ignore
        };
        self
    }

    fn build_callbacks<F>(self, callback: F) -> HotkeyCallbacks
    where
        F: Fn() + Send + Sync + 'static,
    {
        let press_callback: Callback = Arc::new(callback);
        let release_callback = match self.release_behavior {
            ReleaseBehavior::Disabled => None,
            ReleaseBehavior::SameAsPress => Some(press_callback.clone()),
            ReleaseBehavior::Custom(callback) => Some(callback),
        };

        HotkeyCallbacks {
            on_press: press_callback,
            on_release: release_callback,
            min_hold: self.min_hold,
            repeat_behavior: self.repeat_behavior,
        }
    }
}

/// Hotkey registration with modifiers
pub(crate) struct HotkeyRegistration {
    pub(crate) callbacks: HotkeyCallbacks,
}

pub(crate) struct ActiveHotkeyPress {
    pub(crate) registration_key: HotkeyKey,
    pub(crate) pressed_at: Instant,
    pub(crate) press_dispatch_state: PressDispatchState,
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

        if requested_backend.is_none() && selected_backend == Backend::Portal {
            return match Self::initialize_with_backend(Backend::Portal) {
                Ok(manager) => Ok(manager),
                Err(error) if should_fallback_from_portal_error(&error) => {
                    Self::initialize_with_backend(Backend::Evdev)
                }
                Err(error) => Err(error),
            };
        }

        Self::initialize_with_backend(selected_backend)
    }

    fn initialize_with_backend(backend: Backend) -> Result<Self, Error> {
        let backend_impl = build_backend(backend)?;

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
            active_backend: backend,
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
        self.register_with_options(key, modifiers, HotkeyOptions::new(), callback)
    }

    pub fn register_with_options<F>(
        &self,
        key: KeyCode,
        modifiers: &[KeyCode],
        options: HotkeyOptions,
        callback: F,
    ) -> Result<Handle, Error>
    where
        F: Fn() + Send + Sync + 'static,
    {
        let hotkey_key = (key, normalize_modifiers(modifiers));
        let callbacks = options.build_callbacks(callback);

        {
            let mut registrations = self.inner.registrations.lock().unwrap();
            registrations.insert(hotkey_key.clone(), HotkeyRegistration { callbacks });
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


fn should_fallback_from_portal_error(error: &Error) -> bool {
    matches!(error, Error::BackendInit(_))
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
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

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

    #[test]
    fn explicit_release_callback_overrides_press_callback_for_release() {
        let press_called = Arc::new(AtomicBool::new(false));
        let release_called = Arc::new(AtomicBool::new(false));

        let press_called_clone = press_called.clone();
        let release_called_clone = release_called.clone();

        let options = HotkeyOptions::new()
            .on_release()
            .on_release_callback(move || {
                release_called_clone.store(true, Ordering::SeqCst);
            });

        let callbacks = options.build_callbacks(move || {
            press_called_clone.store(true, Ordering::SeqCst);
        });

        (callbacks.on_press)();
        callbacks.on_release.as_ref().unwrap()();

        assert!(press_called.load(Ordering::SeqCst));
        assert!(release_called.load(Ordering::SeqCst));
    }

    #[test]
    fn options_can_enable_release_and_repeat() {
        let options = HotkeyOptions::new()
            .on_release()
            .trigger_on_repeat(true)
            .min_hold(Duration::from_millis(50));

        let called = Arc::new(AtomicBool::new(false));
        let called_clone = called.clone();
        let callbacks = options.build_callbacks(move || {
            called_clone.store(true, Ordering::SeqCst);
        });

        assert!(callbacks.on_release.is_some());
        assert!(matches!(callbacks.repeat_behavior, RepeatBehavior::Trigger));
        assert_eq!(callbacks.min_hold, Some(Duration::from_millis(50)));

        (callbacks.on_press)();
        assert!(called.load(Ordering::SeqCst));
    }

    #[test]
    fn fallback_decision_only_accepts_backend_init_error() {
        assert!(should_fallback_from_portal_error(&Error::BackendInit(
            "portal unavailable".to_string(),
        )));
        assert!(!should_fallback_from_portal_error(&Error::NoKeyboardsFound));
        assert!(!should_fallback_from_portal_error(&Error::DeviceAccess(
            "unexpected".to_string(),
        )));
    }

    #[test]
    fn on_release_reuses_press_callback_when_enabled() {
        let count = Arc::new(AtomicUsize::new(0));
        let count_clone = count.clone();

        let callbacks = HotkeyOptions::new().on_release().build_callbacks(move || {
            count_clone.fetch_add(1, Ordering::SeqCst);
        });

        (callbacks.on_press)();
        callbacks.on_release.as_ref().unwrap()();

        assert_eq!(count.load(Ordering::SeqCst), 2);
    }
}
