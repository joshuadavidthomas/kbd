use crate::backend::{build_backend, resolve_backend, Backend};
use crate::error::Error;
use crate::hotkey::{Hotkey, HotkeySequence};

use evdev::KeyCode;
use std::collections::HashMap;
use std::sync::{
    atomic::{AtomicBool, AtomicU64, Ordering},
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
    pub(crate) passthrough: bool,
}

#[derive(Clone, Default)]
pub struct HotkeyOptions {
    release_behavior: ReleaseBehavior,
    min_hold: Option<Duration>,
    repeat_behavior: RepeatBehavior,
    passthrough: bool,
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

    pub fn passthrough(mut self, passthrough: bool) -> Self {
        self.passthrough = passthrough;
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
            passthrough: self.passthrough,
        }
    }
}

/// Hotkey registration with modifiers
#[derive(Clone)]
pub(crate) struct HotkeyRegistration {
    pub(crate) callbacks: HotkeyCallbacks,
}

pub type SequenceId = u64;

#[derive(Clone)]
pub(crate) struct SequenceRegistration {
    pub(crate) steps: Vec<HotkeyKey>,
    pub(crate) callback: Callback,
    pub(crate) timeout: Duration,
    pub(crate) abort_key: KeyCode,
    pub(crate) timeout_fallback: Option<HotkeyKey>,
}

#[derive(Clone)]
pub struct SequenceOptions {
    timeout: Duration,
    abort_key: KeyCode,
    timeout_fallback: Option<Hotkey>,
}

impl Default for SequenceOptions {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(1),
            abort_key: KeyCode::KEY_ESC,
            timeout_fallback: None,
        }
    }
}

impl SequenceOptions {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    pub fn abort_key(mut self, key: KeyCode) -> Self {
        self.abort_key = key;
        self
    }

    pub fn timeout_fallback(mut self, hotkey: Hotkey) -> Self {
        self.timeout_fallback = Some(hotkey);
        self
    }
}

#[derive(Clone, Copy, Default)]
struct ManagerRuntimeOptions {
    grab: bool,
}

pub struct HotkeyManagerBuilder {
    requested_backend: Option<Backend>,
    options: ManagerRuntimeOptions,
}

impl HotkeyManagerBuilder {
    pub fn backend(mut self, backend: Backend) -> Self {
        self.requested_backend = Some(backend);
        self
    }

    pub fn grab(mut self, grab: bool) -> Self {
        self.options.grab = grab;
        self
    }

    pub fn build(self) -> Result<HotkeyManager, Error> {
        HotkeyManager::with_backend_internal(self.requested_backend, self.options)
    }
}

pub(crate) struct ActiveHotkeyPress {
    pub(crate) registration_key: HotkeyKey,
    pub(crate) pressed_at: Instant,
    pub(crate) press_dispatch_state: PressDispatchState,
}

/// Key used to identify hotkey registrations: (target_key, normalized_modifiers)
pub type HotkeyKey = (KeyCode, Vec<KeyCode>);

pub(crate) fn is_modifier_key(key: KeyCode) -> bool {
    matches!(
        key,
        KeyCode::KEY_LEFTCTRL
            | KeyCode::KEY_RIGHTCTRL
            | KeyCode::KEY_LEFTALT
            | KeyCode::KEY_RIGHTALT
            | KeyCode::KEY_LEFTSHIFT
            | KeyCode::KEY_RIGHTSHIFT
            | KeyCode::KEY_LEFTMETA
            | KeyCode::KEY_RIGHTMETA
    )
}

fn validate_hotkey_binding(key: KeyCode, modifiers: &[KeyCode]) -> Result<(), Error> {
    if is_modifier_key(key) {
        return Err(Error::InvalidHotkey(format!(
            "modifier keys cannot be used as the primary hotkey key: {:?}",
            key
        )));
    }

    if let Some(invalid_modifier) = modifiers
        .iter()
        .copied()
        .find(|modifier| !is_modifier_key(*modifier))
    {
        return Err(Error::InvalidHotkey(format!(
            "non-modifier keys cannot be used as modifiers: {:?}",
            invalid_modifier
        )));
    }

    Ok(())
}

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

fn already_registered_error(hotkey_key: &HotkeyKey) -> Error {
    Error::AlreadyRegistered {
        key: hotkey_key.0,
        modifiers: hotkey_key.1.clone(),
    }
}

fn remove_registration_if_matches(
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

/// Handle for unregistering a specific hotkey
#[derive(Clone)]
pub struct Handle {
    key: HotkeyKey,
    registration_marker: Callback,
    manager: Arc<HotkeyManagerInner>,
}

impl std::fmt::Debug for Handle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Handle").field("key", &self.key).finish()
    }
}

impl Handle {
    pub fn unregister(self) -> Result<(), Error> {
        self.manager
            .remove_hotkey(&self.key, &self.registration_marker)
    }
}

#[derive(Clone)]
pub struct SequenceHandle {
    id: SequenceId,
    manager: Arc<HotkeyManagerInner>,
}

impl std::fmt::Debug for SequenceHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SequenceHandle")
            .field("id", &self.id)
            .finish()
    }
}

impl SequenceHandle {
    pub fn unregister(self) -> Result<(), Error> {
        self.manager.remove_sequence(self.id);
        Ok(())
    }
}

/// Inner state shared between HotkeyManager and Handles
struct HotkeyManagerInner {
    registrations: Arc<Mutex<HashMap<HotkeyKey, HotkeyRegistration>>>,
    sequence_registrations: Arc<Mutex<HashMap<SequenceId, SequenceRegistration>>>,
    next_sequence_id: AtomicU64,
    backend_impl: Arc<dyn crate::backend::HotkeyBackend>,
    stop_flag: Arc<AtomicBool>,
    operation_lock: Mutex<()>,
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
        Self::with_backend_internal(None, ManagerRuntimeOptions::default())
    }

    /// Create a manager with an explicit backend.
    pub fn with_backend(backend: Backend) -> Result<Self, Error> {
        Self::with_backend_internal(Some(backend), ManagerRuntimeOptions::default())
    }

    pub fn builder() -> HotkeyManagerBuilder {
        HotkeyManagerBuilder {
            requested_backend: None,
            options: ManagerRuntimeOptions::default(),
        }
    }

    /// Returns the backend selected for this manager instance.
    pub fn active_backend(&self) -> Backend {
        self.active_backend
    }

    /// Resolve which backend would be selected for the current process.
    pub fn detect_backend(requested_backend: Option<Backend>) -> Result<Backend, Error> {
        resolve_backend(requested_backend)
    }

    fn with_backend_internal(
        requested_backend: Option<Backend>,
        options: ManagerRuntimeOptions,
    ) -> Result<Self, Error> {
        let selected_backend = resolve_backend(requested_backend)?;
        validate_runtime_options(selected_backend, options)?;

        if requested_backend.is_none() && selected_backend == Backend::Portal {
            return match Self::initialize_with_backend(Backend::Portal, options) {
                Ok(manager) => Ok(manager),
                Err(error) if should_fallback_from_portal_error(&error) => {
                    #[cfg(feature = "evdev")]
                    {
                        Self::initialize_with_backend(Backend::Evdev, options)
                    }
                    #[cfg(not(feature = "evdev"))]
                    {
                        Err(error)
                    }
                }
                Err(error) => Err(error),
            };
        }

        Self::initialize_with_backend(selected_backend, options)
    }

    fn initialize_with_backend(
        backend: Backend,
        options: ManagerRuntimeOptions,
    ) -> Result<Self, Error> {
        let backend_impl: Arc<dyn crate::backend::HotkeyBackend> =
            build_backend(backend, options.grab)?.into();

        let inner = Arc::new(HotkeyManagerInner {
            registrations: Arc::new(Mutex::new(HashMap::new())),
            sequence_registrations: Arc::new(Mutex::new(HashMap::new())),
            next_sequence_id: AtomicU64::new(1),
            backend_impl: backend_impl.clone(),
            stop_flag: Arc::new(AtomicBool::new(false)),
            operation_lock: Mutex::new(()),
            listener: Mutex::new(None),
        });

        let listener = backend_impl.start_listener(
            inner.registrations.clone(),
            inner.sequence_registrations.clone(),
            inner.stop_flag.clone(),
        )?;

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
        let _operation_guard = self.inner.operation_lock.lock().unwrap();

        if self.inner.stop_flag.load(Ordering::SeqCst) {
            return Err(Error::ManagerStopped);
        }

        validate_hotkey_binding(key, modifiers)?;

        let hotkey_key = (key, normalize_modifiers(modifiers));
        let registration = HotkeyRegistration {
            callbacks: options.build_callbacks(callback),
        };
        let registration_marker = registration.callbacks.on_press.clone();

        {
            let registrations = self.inner.registrations.lock().unwrap();
            if registrations.contains_key(&hotkey_key) {
                return Err(already_registered_error(&hotkey_key));
            }
        }

        self.inner.backend_impl.register_hotkey(&hotkey_key)?;

        {
            let mut registrations = self.inner.registrations.lock().unwrap();
            registrations.insert(hotkey_key.clone(), registration);
        }

        if self.inner.stop_flag.load(Ordering::SeqCst) {
            let unregister_result = self.inner.backend_impl.unregister_hotkey(&hotkey_key);
            let mut registrations = self.inner.registrations.lock().unwrap();
            let _ = remove_registration_if_matches(
                &mut registrations,
                &hotkey_key,
                &registration_marker,
            );
            return match unregister_result {
                Ok(()) => Err(Error::ManagerStopped),
                Err(error) => Err(error),
            };
        }

        Ok(Handle {
            key: hotkey_key,
            registration_marker,
            manager: self.inner.clone(),
        })
    }

    pub fn register_sequence<F>(
        &self,
        sequence: &HotkeySequence,
        options: SequenceOptions,
        callback: F,
    ) -> Result<SequenceHandle, Error>
    where
        F: Fn() + Send + Sync + 'static,
    {
        let _operation_guard = self.inner.operation_lock.lock().unwrap();

        if self.inner.stop_flag.load(Ordering::SeqCst) {
            return Err(Error::ManagerStopped);
        }

        if self.active_backend != Backend::Evdev {
            return Err(Error::UnsupportedFeature(
                "key sequences are only supported by the evdev backend".to_string(),
            ));
        }

        let SequenceOptions {
            timeout,
            abort_key,
            timeout_fallback,
        } = options;

        if timeout.is_zero() {
            return Err(Error::InvalidSequence(
                "sequence timeout must be greater than zero".to_string(),
            ));
        }

        let steps = sequence.steps();
        if steps.len() < 2 {
            return Err(Error::InvalidSequence(
                "sequence must contain at least two steps".to_string(),
            ));
        }

        for step in steps {
            validate_hotkey_binding(step.key(), step.modifiers())?;
        }

        let normalized_steps: Vec<HotkeyKey> = steps
            .iter()
            .map(|step| (step.key(), normalize_modifiers(step.modifiers())))
            .collect();

        let timeout_fallback = timeout_fallback
            .map(|hotkey| {
                validate_hotkey_binding(hotkey.key(), hotkey.modifiers())?;
                Ok((hotkey.key(), normalize_modifiers(hotkey.modifiers())))
            })
            .transpose()?;

        let sequence_id = self.inner.next_sequence_id.fetch_add(1, Ordering::SeqCst);
        let registration = SequenceRegistration {
            steps: normalized_steps,
            callback: Arc::new(callback),
            timeout,
            abort_key,
            timeout_fallback,
        };

        self.inner
            .sequence_registrations
            .lock()
            .unwrap()
            .insert(sequence_id, registration);

        Ok(SequenceHandle {
            id: sequence_id,
            manager: self.inner.clone(),
        })
    }

    pub fn replace<F>(
        &self,
        key: KeyCode,
        modifiers: &[KeyCode],
        callback: F,
    ) -> Result<Handle, Error>
    where
        F: Fn() + Send + Sync + 'static,
    {
        self.replace_with_options(key, modifiers, HotkeyOptions::new(), callback)
    }

    pub fn replace_with_options<F>(
        &self,
        key: KeyCode,
        modifiers: &[KeyCode],
        options: HotkeyOptions,
        callback: F,
    ) -> Result<Handle, Error>
    where
        F: Fn() + Send + Sync + 'static,
    {
        let _operation_guard = self.inner.operation_lock.lock().unwrap();

        if self.inner.stop_flag.load(Ordering::SeqCst) {
            return Err(Error::ManagerStopped);
        }

        validate_hotkey_binding(key, modifiers)?;

        let hotkey_key = (key, normalize_modifiers(modifiers));
        let registration = HotkeyRegistration {
            callbacks: options.build_callbacks(callback),
        };
        let registration_marker = registration.callbacks.on_press.clone();

        let already_registered = self
            .inner
            .registrations
            .lock()
            .unwrap()
            .contains_key(&hotkey_key);

        if !already_registered {
            self.inner.backend_impl.register_hotkey(&hotkey_key)?;
        }

        let previous_registration = {
            let mut registrations = self.inner.registrations.lock().unwrap();
            registrations.insert(hotkey_key.clone(), registration)
        };

        if self.inner.stop_flag.load(Ordering::SeqCst) {
            if already_registered {
                let mut registrations = self.inner.registrations.lock().unwrap();
                if let Some(previous) = previous_registration {
                    registrations.insert(hotkey_key.clone(), previous);
                } else {
                    registrations.remove(&hotkey_key);
                }
            } else {
                let unregister_result = self.inner.backend_impl.unregister_hotkey(&hotkey_key);
                let mut registrations = self.inner.registrations.lock().unwrap();
                let _ = remove_registration_if_matches(
                    &mut registrations,
                    &hotkey_key,
                    &registration_marker,
                );
                return match unregister_result {
                    Ok(()) => Err(Error::ManagerStopped),
                    Err(error) => Err(error),
                };
            }
            return Err(Error::ManagerStopped);
        }

        Ok(Handle {
            key: hotkey_key,
            registration_marker,
            manager: self.inner.clone(),
        })
    }

    pub fn is_registered(&self, key: KeyCode, modifiers: &[KeyCode]) -> bool {
        let hotkey_key = (key, normalize_modifiers(modifiers));
        self.inner
            .registrations
            .lock()
            .unwrap()
            .contains_key(&hotkey_key)
    }

    /// Unregister all hotkeys and stop the listener
    pub fn unregister_all(&self) -> Result<(), Error> {
        let mut first_error = None;

        {
            let _operation_guard = self.inner.operation_lock.lock().unwrap();
            self.inner.stop_flag.store(true, Ordering::SeqCst);

            let registered_keys: Vec<HotkeyKey> = self
                .inner
                .registrations
                .lock()
                .unwrap()
                .keys()
                .cloned()
                .collect();

            for key in &registered_keys {
                if let Err(error) = self.inner.backend_impl.unregister_hotkey(key) {
                    if first_error.is_none() {
                        first_error = Some(error);
                    }
                }
            }

            self.inner.registrations.lock().unwrap().clear();
            self.inner.sequence_registrations.lock().unwrap().clear();
        }

        if let Some(listener) = self.inner.listener.lock().unwrap().take() {
            let listener_thread_id = listener.thread().id();
            let current_thread_id = std::thread::current().id();

            if listener_thread_id != current_thread_id {
                if let Err(err) = listener.join() {
                    if first_error.is_none() {
                        first_error = Some(Error::ThreadSpawn(format!(
                            "Failed to join listener thread: {:?}",
                            err
                        )));
                    }
                }
            }
        }

        if let Some(error) = first_error {
            return Err(error);
        }

        Ok(())
    }
}

fn validate_runtime_options(backend: Backend, options: ManagerRuntimeOptions) -> Result<(), Error> {
    if !options.grab {
        return Ok(());
    }

    if backend != Backend::Evdev {
        return Err(Error::UnsupportedFeature(
            "event grabbing is only supported by the evdev backend".to_string(),
        ));
    }

    #[cfg(not(feature = "grab"))]
    {
        Err(Error::UnsupportedFeature(
            "event grabbing support is not compiled in (enable the `grab` feature)".to_string(),
        ))
    }

    #[cfg(feature = "grab")]
    {
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
    fn remove_hotkey(&self, key: &HotkeyKey, registration_marker: &Callback) -> Result<(), Error> {
        let _operation_guard = self.operation_lock.lock().unwrap();

        let is_current_registration =
            self.registrations
                .lock()
                .unwrap()
                .get(key)
                .is_some_and(|registration| {
                    Arc::ptr_eq(&registration.callbacks.on_press, registration_marker)
                });

        if !is_current_registration {
            return Ok(());
        }

        self.backend_impl.unregister_hotkey(key)?;

        let mut registrations = self.registrations.lock().unwrap();
        let _ = remove_registration_if_matches(&mut registrations, key, registration_marker);

        Ok(())
    }

    fn remove_sequence(&self, sequence_id: SequenceId) {
        let _operation_guard = self.operation_lock.lock().unwrap();
        self.sequence_registrations
            .lock()
            .unwrap()
            .remove(&sequence_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::sync::{mpsc, Barrier, Mutex};
    use std::thread::JoinHandle;

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
    fn passthrough_option_is_stored_in_callbacks() {
        let callbacks = HotkeyOptions::new()
            .passthrough(true)
            .build_callbacks(|| {});
        assert!(callbacks.passthrough);
    }

    #[test]
    fn grab_option_rejects_portal_backend() {
        let err = validate_runtime_options(Backend::Portal, ManagerRuntimeOptions { grab: true })
            .err()
            .unwrap();

        assert!(matches!(err, Error::UnsupportedFeature(_)));
    }

    #[test]
    #[cfg(not(feature = "grab"))]
    fn grab_option_requires_grab_feature_flag() {
        let err = validate_runtime_options(Backend::Evdev, ManagerRuntimeOptions { grab: true })
            .err()
            .unwrap();

        assert!(matches!(err, Error::UnsupportedFeature(_)));
    }

    #[test]
    #[cfg(feature = "grab")]
    fn grab_option_is_allowed_with_evdev_when_feature_enabled() {
        assert!(
            validate_runtime_options(Backend::Evdev, ManagerRuntimeOptions { grab: true },).is_ok()
        );
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

    struct FakeBackend;

    impl crate::backend::HotkeyBackend for FakeBackend {
        fn start_listener(
            &self,
            _registrations: Arc<Mutex<HashMap<HotkeyKey, HotkeyRegistration>>>,
            _sequence_registrations: Arc<Mutex<HashMap<SequenceId, SequenceRegistration>>>,
            _stop_flag: Arc<AtomicBool>,
        ) -> Result<JoinHandle<()>, Error> {
            std::thread::Builder::new()
                .name("fake-listener".to_string())
                .spawn(|| {})
                .map_err(|err| Error::ThreadSpawn(err.to_string()))
        }

        fn register_hotkey(&self, _hotkey: &HotkeyKey) -> Result<(), Error> {
            Ok(())
        }

        fn unregister_hotkey(&self, _hotkey: &HotkeyKey) -> Result<(), Error> {
            Ok(())
        }
    }

    struct UnregisterFailBackend;

    impl crate::backend::HotkeyBackend for UnregisterFailBackend {
        fn start_listener(
            &self,
            _registrations: Arc<Mutex<HashMap<HotkeyKey, HotkeyRegistration>>>,
            _sequence_registrations: Arc<Mutex<HashMap<SequenceId, SequenceRegistration>>>,
            _stop_flag: Arc<AtomicBool>,
        ) -> Result<JoinHandle<()>, Error> {
            std::thread::Builder::new()
                .name("fake-listener".to_string())
                .spawn(|| {})
                .map_err(|err| Error::ThreadSpawn(err.to_string()))
        }

        fn register_hotkey(&self, _hotkey: &HotkeyKey) -> Result<(), Error> {
            Ok(())
        }

        fn unregister_hotkey(&self, _hotkey: &HotkeyKey) -> Result<(), Error> {
            Err(Error::BackendInit("forced unregister failure".to_string()))
        }
    }

    struct BlockingUnregisterFailBackend {
        entered_unregister: Arc<AtomicBool>,
        allow_unregister_finish: Arc<AtomicBool>,
    }

    impl crate::backend::HotkeyBackend for BlockingUnregisterFailBackend {
        fn start_listener(
            &self,
            _registrations: Arc<Mutex<HashMap<HotkeyKey, HotkeyRegistration>>>,
            _sequence_registrations: Arc<Mutex<HashMap<SequenceId, SequenceRegistration>>>,
            _stop_flag: Arc<AtomicBool>,
        ) -> Result<JoinHandle<()>, Error> {
            std::thread::Builder::new()
                .name("fake-listener".to_string())
                .spawn(|| {})
                .map_err(|err| Error::ThreadSpawn(err.to_string()))
        }

        fn register_hotkey(&self, _hotkey: &HotkeyKey) -> Result<(), Error> {
            Ok(())
        }

        fn unregister_hotkey(&self, _hotkey: &HotkeyKey) -> Result<(), Error> {
            self.entered_unregister.store(true, Ordering::SeqCst);
            let deadline = Instant::now() + Duration::from_secs(2);
            while !self.allow_unregister_finish.load(Ordering::SeqCst) {
                if Instant::now() >= deadline {
                    return Err(Error::BackendInit(
                        "timed out waiting to finish unregister".to_string(),
                    ));
                }
                std::thread::yield_now();
            }
            Err(Error::BackendInit("forced unregister failure".to_string()))
        }
    }

    struct BlockingUnregisterSuccessBackend {
        entered_unregister: Arc<AtomicBool>,
        allow_unregister_finish: Arc<AtomicBool>,
        backend_registered: Arc<AtomicBool>,
    }

    impl crate::backend::HotkeyBackend for BlockingUnregisterSuccessBackend {
        fn start_listener(
            &self,
            _registrations: Arc<Mutex<HashMap<HotkeyKey, HotkeyRegistration>>>,
            _sequence_registrations: Arc<Mutex<HashMap<SequenceId, SequenceRegistration>>>,
            _stop_flag: Arc<AtomicBool>,
        ) -> Result<JoinHandle<()>, Error> {
            std::thread::Builder::new()
                .name("fake-listener".to_string())
                .spawn(|| {})
                .map_err(|err| Error::ThreadSpawn(err.to_string()))
        }

        fn register_hotkey(&self, _hotkey: &HotkeyKey) -> Result<(), Error> {
            self.backend_registered.store(true, Ordering::SeqCst);
            Ok(())
        }

        fn unregister_hotkey(&self, _hotkey: &HotkeyKey) -> Result<(), Error> {
            self.entered_unregister.store(true, Ordering::SeqCst);
            let deadline = Instant::now() + Duration::from_secs(2);
            while !self.allow_unregister_finish.load(Ordering::SeqCst) {
                if Instant::now() >= deadline {
                    return Err(Error::BackendInit(
                        "timed out waiting to finish unregister".to_string(),
                    ));
                }
                std::thread::yield_now();
            }
            self.backend_registered.store(false, Ordering::SeqCst);
            Ok(())
        }
    }

    struct CountingUnregisterBackend {
        unregister_calls: Arc<AtomicUsize>,
    }

    impl crate::backend::HotkeyBackend for CountingUnregisterBackend {
        fn start_listener(
            &self,
            _registrations: Arc<Mutex<HashMap<HotkeyKey, HotkeyRegistration>>>,
            _sequence_registrations: Arc<Mutex<HashMap<SequenceId, SequenceRegistration>>>,
            _stop_flag: Arc<AtomicBool>,
        ) -> Result<JoinHandle<()>, Error> {
            std::thread::Builder::new()
                .name("fake-listener".to_string())
                .spawn(|| {})
                .map_err(|err| Error::ThreadSpawn(err.to_string()))
        }

        fn register_hotkey(&self, _hotkey: &HotkeyKey) -> Result<(), Error> {
            Ok(())
        }

        fn unregister_hotkey(&self, _hotkey: &HotkeyKey) -> Result<(), Error> {
            self.unregister_calls.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    }

    struct BlockingRegisterSuccessBackend {
        entered_register: Arc<AtomicBool>,
        allow_register_finish: Arc<AtomicBool>,
    }

    impl crate::backend::HotkeyBackend for BlockingRegisterSuccessBackend {
        fn start_listener(
            &self,
            _registrations: Arc<Mutex<HashMap<HotkeyKey, HotkeyRegistration>>>,
            _sequence_registrations: Arc<Mutex<HashMap<SequenceId, SequenceRegistration>>>,
            _stop_flag: Arc<AtomicBool>,
        ) -> Result<JoinHandle<()>, Error> {
            std::thread::Builder::new()
                .name("fake-listener".to_string())
                .spawn(|| {})
                .map_err(|err| Error::ThreadSpawn(err.to_string()))
        }

        fn register_hotkey(&self, _hotkey: &HotkeyKey) -> Result<(), Error> {
            self.entered_register.store(true, Ordering::SeqCst);
            let deadline = Instant::now() + Duration::from_secs(2);
            while !self.allow_register_finish.load(Ordering::SeqCst) {
                if Instant::now() >= deadline {
                    return Err(Error::BackendInit(
                        "timed out waiting to finish register".to_string(),
                    ));
                }
                std::thread::yield_now();
            }
            Ok(())
        }

        fn unregister_hotkey(&self, _hotkey: &HotkeyKey) -> Result<(), Error> {
            Ok(())
        }
    }

    struct BlockingRegisterThenUnregisterFailBackend {
        entered_register: Arc<AtomicBool>,
        allow_register_finish: Arc<AtomicBool>,
    }

    impl crate::backend::HotkeyBackend for BlockingRegisterThenUnregisterFailBackend {
        fn start_listener(
            &self,
            _registrations: Arc<Mutex<HashMap<HotkeyKey, HotkeyRegistration>>>,
            _sequence_registrations: Arc<Mutex<HashMap<SequenceId, SequenceRegistration>>>,
            _stop_flag: Arc<AtomicBool>,
        ) -> Result<JoinHandle<()>, Error> {
            std::thread::Builder::new()
                .name("fake-listener".to_string())
                .spawn(|| {})
                .map_err(|err| Error::ThreadSpawn(err.to_string()))
        }

        fn register_hotkey(&self, _hotkey: &HotkeyKey) -> Result<(), Error> {
            self.entered_register.store(true, Ordering::SeqCst);
            let deadline = Instant::now() + Duration::from_secs(2);
            while !self.allow_register_finish.load(Ordering::SeqCst) {
                if Instant::now() >= deadline {
                    return Err(Error::BackendInit(
                        "timed out waiting to finish register".to_string(),
                    ));
                }
                std::thread::yield_now();
            }
            Ok(())
        }

        fn unregister_hotkey(&self, _hotkey: &HotkeyKey) -> Result<(), Error> {
            Err(Error::BackendInit("forced unregister failure".to_string()))
        }
    }

    struct FailsSecondRegisterBackend {
        register_calls: Arc<AtomicUsize>,
    }

    impl crate::backend::HotkeyBackend for FailsSecondRegisterBackend {
        fn start_listener(
            &self,
            _registrations: Arc<Mutex<HashMap<HotkeyKey, HotkeyRegistration>>>,
            _sequence_registrations: Arc<Mutex<HashMap<SequenceId, SequenceRegistration>>>,
            _stop_flag: Arc<AtomicBool>,
        ) -> Result<JoinHandle<()>, Error> {
            std::thread::Builder::new()
                .name("fake-listener".to_string())
                .spawn(|| {})
                .map_err(|err| Error::ThreadSpawn(err.to_string()))
        }

        fn register_hotkey(&self, _hotkey: &HotkeyKey) -> Result<(), Error> {
            let call = self.register_calls.fetch_add(1, Ordering::SeqCst) + 1;
            if call > 1 {
                return Err(Error::BackendInit("forced register failure".to_string()));
            }
            Ok(())
        }

        fn unregister_hotkey(&self, _hotkey: &HotkeyKey) -> Result<(), Error> {
            Ok(())
        }
    }

    struct ConcurrentRegisterBackend {
        barrier: Arc<Barrier>,
        register_calls: Arc<AtomicUsize>,
    }

    impl crate::backend::HotkeyBackend for ConcurrentRegisterBackend {
        fn start_listener(
            &self,
            _registrations: Arc<Mutex<HashMap<HotkeyKey, HotkeyRegistration>>>,
            _sequence_registrations: Arc<Mutex<HashMap<SequenceId, SequenceRegistration>>>,
            _stop_flag: Arc<AtomicBool>,
        ) -> Result<JoinHandle<()>, Error> {
            std::thread::Builder::new()
                .name("fake-listener".to_string())
                .spawn(|| {})
                .map_err(|err| Error::ThreadSpawn(err.to_string()))
        }

        fn register_hotkey(&self, _hotkey: &HotkeyKey) -> Result<(), Error> {
            self.register_calls.fetch_add(1, Ordering::SeqCst);
            self.barrier.wait();
            Ok(())
        }

        fn unregister_hotkey(&self, _hotkey: &HotkeyKey) -> Result<(), Error> {
            Ok(())
        }
    }

    fn manager_with_backend(backend_impl: Arc<dyn crate::backend::HotkeyBackend>) -> HotkeyManager {
        HotkeyManager {
            inner: Arc::new(HotkeyManagerInner {
                registrations: Arc::new(Mutex::new(HashMap::new())),
                sequence_registrations: Arc::new(Mutex::new(HashMap::new())),
                next_sequence_id: AtomicU64::new(1),
                backend_impl,
                stop_flag: Arc::new(AtomicBool::new(false)),
                operation_lock: Mutex::new(()),
                listener: Mutex::new(None),
            }),
            active_backend: Backend::Evdev,
        }
    }

    fn manager_with_fake_backend() -> HotkeyManager {
        manager_with_backend(Arc::new(FakeBackend))
    }

    fn registration_with_counter(counter: Arc<AtomicUsize>) -> HotkeyRegistration {
        HotkeyRegistration {
            callbacks: HotkeyCallbacks {
                on_press: Arc::new(move || {
                    counter.fetch_add(1, Ordering::SeqCst);
                }),
                on_release: None,
                min_hold: None,
                repeat_behavior: RepeatBehavior::Ignore,
                passthrough: false,
            },
        }
    }

    fn insert_new_registration(
        registrations: &mut HashMap<HotkeyKey, HotkeyRegistration>,
        hotkey_key: HotkeyKey,
        registration: HotkeyRegistration,
    ) -> Result<(), Error> {
        if registrations.contains_key(&hotkey_key) {
            return Err(already_registered_error(&hotkey_key));
        }

        registrations.insert(hotkey_key, registration);
        Ok(())
    }

    fn wait_until<F>(condition: F, context: &str)
    where
        F: Fn() -> bool,
    {
        let deadline = Instant::now() + Duration::from_secs(2);
        while !condition() {
            if Instant::now() >= deadline {
                panic!("timed out waiting for {context}");
            }
            std::thread::yield_now();
        }
    }

    #[test]
    fn duplicate_registration_returns_error() {
        let mut registrations = HashMap::new();
        let key = (
            KeyCode::KEY_A,
            normalize_modifiers(&[KeyCode::KEY_LEFTCTRL, KeyCode::KEY_RIGHTCTRL]),
        );

        insert_new_registration(
            &mut registrations,
            key.clone(),
            registration_with_counter(Arc::new(AtomicUsize::new(0))),
        )
        .unwrap();

        let err = insert_new_registration(
            &mut registrations,
            (
                KeyCode::KEY_A,
                normalize_modifiers(&[KeyCode::KEY_LEFTCTRL]),
            ),
            registration_with_counter(Arc::new(AtomicUsize::new(0))),
        )
        .err()
        .unwrap();

        assert!(matches!(err, Error::AlreadyRegistered { .. }));
    }

    #[test]
    fn is_registered_tracks_canonicalized_bindings() {
        let manager = manager_with_fake_backend();

        assert!(!manager.is_registered(KeyCode::KEY_D, &[KeyCode::KEY_LEFTCTRL]));

        manager
            .register(KeyCode::KEY_D, &[KeyCode::KEY_RIGHTCTRL], || {})
            .unwrap();

        assert!(manager.is_registered(KeyCode::KEY_D, &[KeyCode::KEY_LEFTCTRL]));
    }

    #[test]
    fn register_rejects_modifier_primary_key() {
        let manager = manager_with_fake_backend();

        let err = manager
            .register(KeyCode::KEY_LEFTCTRL, &[KeyCode::KEY_LEFTSHIFT], || {})
            .err()
            .unwrap();

        assert!(matches!(err, Error::InvalidHotkey(_)));
    }

    #[test]
    fn register_rejects_non_modifier_modifier_keys() {
        let manager = manager_with_fake_backend();

        let err = manager
            .register(KeyCode::KEY_F, &[KeyCode::KEY_A], || {})
            .err()
            .unwrap();

        assert!(matches!(err, Error::InvalidHotkey(_)));
    }

    #[test]
    fn replacement_path_overwrites_existing_registration() {
        let mut registrations = HashMap::new();
        let key = (KeyCode::KEY_B, normalize_modifiers(&[KeyCode::KEY_LEFTALT]));

        let first = Arc::new(AtomicUsize::new(0));
        let second = Arc::new(AtomicUsize::new(0));

        insert_new_registration(
            &mut registrations,
            key.clone(),
            registration_with_counter(first.clone()),
        )
        .unwrap();

        let replaced = registrations.insert(key.clone(), registration_with_counter(second.clone()));

        assert!(replaced.is_some());

        let stored = registrations.get(&key).unwrap();
        (stored.callbacks.on_press)();
        assert_eq!(first.load(Ordering::SeqCst), 0);
        assert_eq!(second.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn public_replace_method_allows_intentional_overwrite() {
        let manager = manager_with_fake_backend();
        let calls = Arc::new(AtomicUsize::new(0));

        manager
            .register(KeyCode::KEY_E, &[KeyCode::KEY_LEFTSHIFT], || {})
            .unwrap();

        let calls_clone = calls.clone();
        manager
            .replace(KeyCode::KEY_E, &[KeyCode::KEY_RIGHTSHIFT], move || {
                calls_clone.fetch_add(1, Ordering::SeqCst);
            })
            .unwrap();

        let key = (
            KeyCode::KEY_E,
            normalize_modifiers(&[KeyCode::KEY_LEFTSHIFT]),
        );
        let registrations = manager.inner.registrations.lock().unwrap();
        let registration = registrations.get(&key).unwrap();
        (registration.callbacks.on_press)();

        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn replace_existing_registration_does_not_reregister_backend() {
        let register_calls = Arc::new(AtomicUsize::new(0));
        let manager = manager_with_backend(Arc::new(FailsSecondRegisterBackend {
            register_calls: register_calls.clone(),
        }));

        manager
            .register(KeyCode::KEY_T, &[KeyCode::KEY_LEFTALT], || {})
            .unwrap();

        manager
            .replace(KeyCode::KEY_T, &[KeyCode::KEY_RIGHTALT], || {})
            .unwrap();

        assert_eq!(register_calls.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn failed_backend_registration_does_not_insert_hotkey() {
        let register_calls = Arc::new(AtomicUsize::new(0));
        let manager = manager_with_backend(Arc::new(FailsSecondRegisterBackend { register_calls }));

        manager
            .register(KeyCode::KEY_U, &[KeyCode::KEY_LEFTALT], || {})
            .unwrap();

        let err = manager
            .register(KeyCode::KEY_I, &[KeyCode::KEY_LEFTALT], || {})
            .err()
            .unwrap();

        assert!(matches!(err, Error::BackendInit(_)));
        assert!(!manager.is_registered(KeyCode::KEY_I, &[KeyCode::KEY_RIGHTALT]));
    }

    #[test]
    fn register_returns_manager_stopped_when_listener_stops_mid_registration() {
        let entered_register = Arc::new(AtomicBool::new(false));
        let allow_register_finish = Arc::new(AtomicBool::new(false));

        let manager = Arc::new(manager_with_backend(Arc::new(
            BlockingRegisterSuccessBackend {
                entered_register: entered_register.clone(),
                allow_register_finish: allow_register_finish.clone(),
            },
        )));

        let manager_register = manager.clone();
        let register_thread = std::thread::spawn(move || {
            manager_register.register(KeyCode::KEY_O, &[KeyCode::KEY_LEFTCTRL], || {})
        });

        wait_until(
            || entered_register.load(Ordering::SeqCst),
            "backend register call to start",
        );

        manager.inner.stop_flag.store(true, Ordering::SeqCst);
        allow_register_finish.store(true, Ordering::SeqCst);

        let err = register_thread.join().unwrap().err().unwrap();
        assert!(matches!(err, Error::ManagerStopped));
        assert!(!manager.is_registered(KeyCode::KEY_O, &[KeyCode::KEY_RIGHTCTRL]));
    }

    #[test]
    fn replace_returns_manager_stopped_when_listener_stops_mid_registration() {
        let entered_register = Arc::new(AtomicBool::new(false));
        let allow_register_finish = Arc::new(AtomicBool::new(false));

        let manager = Arc::new(manager_with_backend(Arc::new(
            BlockingRegisterSuccessBackend {
                entered_register: entered_register.clone(),
                allow_register_finish: allow_register_finish.clone(),
            },
        )));

        let manager_replace = manager.clone();
        let replace_thread = std::thread::spawn(move || {
            manager_replace.replace(KeyCode::KEY_P, &[KeyCode::KEY_LEFTCTRL], || {})
        });

        wait_until(
            || entered_register.load(Ordering::SeqCst),
            "backend replace register call to start",
        );

        manager.inner.stop_flag.store(true, Ordering::SeqCst);
        allow_register_finish.store(true, Ordering::SeqCst);

        let err = replace_thread.join().unwrap().err().unwrap();
        assert!(matches!(err, Error::ManagerStopped));
        assert!(!manager.is_registered(KeyCode::KEY_P, &[KeyCode::KEY_RIGHTCTRL]));
    }

    #[test]
    fn register_stop_rollback_clears_registration_when_backend_unregistration_fails() {
        let entered_register = Arc::new(AtomicBool::new(false));
        let allow_register_finish = Arc::new(AtomicBool::new(false));

        let manager = Arc::new(manager_with_backend(Arc::new(
            BlockingRegisterThenUnregisterFailBackend {
                entered_register: entered_register.clone(),
                allow_register_finish: allow_register_finish.clone(),
            },
        )));

        let manager_register = manager.clone();
        let register_thread = std::thread::spawn(move || {
            manager_register.register(KeyCode::KEY_G, &[KeyCode::KEY_LEFTCTRL], || {})
        });

        wait_until(
            || entered_register.load(Ordering::SeqCst),
            "backend register call to start",
        );

        manager.inner.stop_flag.store(true, Ordering::SeqCst);
        allow_register_finish.store(true, Ordering::SeqCst);

        let err = register_thread.join().unwrap().err().unwrap();
        assert!(matches!(err, Error::BackendInit(_)));
        assert!(!manager.is_registered(KeyCode::KEY_G, &[KeyCode::KEY_RIGHTCTRL]));
    }

    #[test]
    fn replace_stop_rollback_clears_registration_when_backend_unregistration_fails() {
        let entered_register = Arc::new(AtomicBool::new(false));
        let allow_register_finish = Arc::new(AtomicBool::new(false));

        let manager = Arc::new(manager_with_backend(Arc::new(
            BlockingRegisterThenUnregisterFailBackend {
                entered_register: entered_register.clone(),
                allow_register_finish: allow_register_finish.clone(),
            },
        )));

        let manager_replace = manager.clone();
        let replace_thread = std::thread::spawn(move || {
            manager_replace.replace(KeyCode::KEY_H, &[KeyCode::KEY_LEFTCTRL], || {})
        });

        wait_until(
            || entered_register.load(Ordering::SeqCst),
            "backend replace register call to start",
        );

        manager.inner.stop_flag.store(true, Ordering::SeqCst);
        allow_register_finish.store(true, Ordering::SeqCst);

        let err = replace_thread.join().unwrap().err().unwrap();
        assert!(matches!(err, Error::BackendInit(_)));
        assert!(!manager.is_registered(KeyCode::KEY_H, &[KeyCode::KEY_RIGHTCTRL]));
    }

    #[test]
    fn stale_handle_unregister_does_not_remove_replaced_registration() {
        let manager = manager_with_fake_backend();
        let calls = Arc::new(AtomicUsize::new(0));

        let stale_handle = manager
            .register(KeyCode::KEY_T, &[KeyCode::KEY_LEFTALT], || {})
            .unwrap();

        let calls_clone = calls.clone();
        manager
            .replace(KeyCode::KEY_T, &[KeyCode::KEY_RIGHTALT], move || {
                calls_clone.fetch_add(1, Ordering::SeqCst);
            })
            .unwrap();

        stale_handle.unregister().unwrap();

        let key = (KeyCode::KEY_T, normalize_modifiers(&[KeyCode::KEY_LEFTALT]));
        let registrations = manager.inner.registrations.lock().unwrap();
        let registration = registrations.get(&key).unwrap();
        (registration.callbacks.on_press)();

        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn unregister_preserves_registration_on_backend_failure() {
        let manager = manager_with_backend(Arc::new(UnregisterFailBackend));

        let handle = manager
            .register(KeyCode::KEY_Q, &[KeyCode::KEY_LEFTCTRL], || {})
            .unwrap();

        let err = handle.unregister().err().unwrap();
        assert!(matches!(err, Error::BackendInit(_)));
        assert!(manager.is_registered(KeyCode::KEY_Q, &[KeyCode::KEY_RIGHTCTRL]));
    }

    #[test]
    fn failed_unregister_does_not_overwrite_new_replacement() {
        let entered_unregister = Arc::new(AtomicBool::new(false));
        let allow_unregister_finish = Arc::new(AtomicBool::new(false));

        let manager = Arc::new(manager_with_backend(Arc::new(
            BlockingUnregisterFailBackend {
                entered_unregister: entered_unregister.clone(),
                allow_unregister_finish: allow_unregister_finish.clone(),
            },
        )));

        let old_calls = Arc::new(AtomicUsize::new(0));
        let new_calls = Arc::new(AtomicUsize::new(0));

        let old_calls_clone = old_calls.clone();
        let handle = manager
            .register(KeyCode::KEY_W, &[KeyCode::KEY_LEFTCTRL], move || {
                old_calls_clone.fetch_add(1, Ordering::SeqCst);
            })
            .unwrap();

        let unregister_thread = std::thread::spawn(move || handle.unregister());

        wait_until(
            || entered_unregister.load(Ordering::SeqCst),
            "failed unregister backend call to start",
        );

        let manager_replace = manager.clone();
        let new_calls_clone = new_calls.clone();
        let replace_thread = std::thread::spawn(move || {
            manager_replace.replace(KeyCode::KEY_W, &[KeyCode::KEY_RIGHTCTRL], move || {
                new_calls_clone.fetch_add(1, Ordering::SeqCst);
            })
        });

        allow_unregister_finish.store(true, Ordering::SeqCst);

        let err = unregister_thread.join().unwrap().err().unwrap();
        assert!(matches!(err, Error::BackendInit(_)));
        replace_thread.join().unwrap().unwrap();

        let key = (
            KeyCode::KEY_W,
            normalize_modifiers(&[KeyCode::KEY_LEFTCTRL]),
        );
        let registrations = manager.inner.registrations.lock().unwrap();
        let registration = registrations.get(&key).unwrap();
        (registration.callbacks.on_press)();

        assert_eq!(old_calls.load(Ordering::SeqCst), 0);
        assert_eq!(new_calls.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn concurrent_unregister_and_replace_keep_backend_registered() {
        let entered_unregister = Arc::new(AtomicBool::new(false));
        let allow_unregister_finish = Arc::new(AtomicBool::new(false));
        let backend_registered = Arc::new(AtomicBool::new(false));

        let manager = Arc::new(manager_with_backend(Arc::new(
            BlockingUnregisterSuccessBackend {
                entered_unregister: entered_unregister.clone(),
                allow_unregister_finish: allow_unregister_finish.clone(),
                backend_registered: backend_registered.clone(),
            },
        )));

        let handle = manager
            .register(KeyCode::KEY_Y, &[KeyCode::KEY_LEFTCTRL], || {})
            .unwrap();

        let unregister_thread = std::thread::spawn(move || handle.unregister());

        wait_until(
            || entered_unregister.load(Ordering::SeqCst),
            "successful unregister backend call to start",
        );

        let manager_replace = manager.clone();
        let replace_thread = std::thread::spawn(move || {
            manager_replace.replace(KeyCode::KEY_Y, &[KeyCode::KEY_RIGHTCTRL], || {})
        });

        allow_unregister_finish.store(true, Ordering::SeqCst);

        unregister_thread.join().unwrap().unwrap();
        replace_thread.join().unwrap().unwrap();

        assert!(backend_registered.load(Ordering::SeqCst));
        assert!(manager.is_registered(KeyCode::KEY_Y, &[KeyCode::KEY_LEFTCTRL]));
    }

    #[test]
    fn concurrent_duplicate_registration_calls_backend_once() {
        let barrier = Arc::new(Barrier::new(2));
        let register_calls = Arc::new(AtomicUsize::new(0));
        let manager = Arc::new(manager_with_backend(Arc::new(ConcurrentRegisterBackend {
            barrier: barrier.clone(),
            register_calls: register_calls.clone(),
        })));

        let manager_a = manager.clone();
        let first = std::thread::spawn(move || {
            manager_a.register(KeyCode::KEY_W, &[KeyCode::KEY_LEFTCTRL], || {})
        });

        let manager_b = manager.clone();
        let second = std::thread::spawn(move || {
            manager_b.register(KeyCode::KEY_W, &[KeyCode::KEY_RIGHTCTRL], || {})
        });

        wait_until(
            || register_calls.load(Ordering::SeqCst) > 0,
            "first backend register invocation",
        );

        barrier.wait();

        let first_result = first.join().unwrap();
        let second_result = second.join().unwrap();

        let ok_count = usize::from(first_result.is_ok()) + usize::from(second_result.is_ok());
        assert_eq!(ok_count, 1);
        assert!(
            matches!(first_result, Err(Error::AlreadyRegistered { .. }))
                || matches!(second_result, Err(Error::AlreadyRegistered { .. }))
        );
        assert_eq!(register_calls.load(Ordering::SeqCst), 1);
        assert!(manager.is_registered(KeyCode::KEY_W, &[KeyCode::KEY_LEFTCTRL]));
    }

    #[test]
    fn unregister_all_notifies_backend_for_each_registration() {
        let unregister_calls = Arc::new(AtomicUsize::new(0));
        let manager = manager_with_backend(Arc::new(CountingUnregisterBackend {
            unregister_calls: unregister_calls.clone(),
        }));

        manager
            .register(KeyCode::KEY_1, &[KeyCode::KEY_LEFTCTRL], || {})
            .unwrap();
        manager
            .register(KeyCode::KEY_2, &[KeyCode::KEY_LEFTCTRL], || {})
            .unwrap();

        manager.unregister_all().unwrap();

        assert_eq!(unregister_calls.load(Ordering::SeqCst), 2);
        assert!(!manager.is_registered(KeyCode::KEY_1, &[KeyCode::KEY_LEFTCTRL]));
        assert!(!manager.is_registered(KeyCode::KEY_2, &[KeyCode::KEY_LEFTCTRL]));
    }

    #[test]
    fn unregister_all_can_be_called_from_listener_thread() {
        let manager = Arc::new(manager_with_fake_backend());
        let start = Arc::new(Barrier::new(2));
        let start_thread = start.clone();
        let manager_thread = manager.clone();
        let (tx, rx) = mpsc::channel();

        let listener = std::thread::spawn(move || {
            start_thread.wait();
            let result = manager_thread.unregister_all();
            tx.send(result).unwrap();
        });

        *manager.inner.listener.lock().unwrap() = Some(listener);

        start.wait();

        let result = rx
            .recv_timeout(Duration::from_secs(1))
            .expect("listener-thread unregister_all should complete");

        assert!(result.is_ok());
    }

    #[test]
    fn register_returns_manager_stopped_after_unregister_all() {
        let manager = manager_with_fake_backend();
        manager.unregister_all().unwrap();

        let err = manager
            .register(KeyCode::KEY_Z, &[KeyCode::KEY_LEFTCTRL], || {})
            .err()
            .unwrap();
        assert!(matches!(err, Error::ManagerStopped));
    }

    #[test]
    fn replace_returns_manager_stopped_after_unregister_all() {
        let manager = manager_with_fake_backend();
        manager.unregister_all().unwrap();

        let err = manager
            .replace(KeyCode::KEY_Z, &[KeyCode::KEY_LEFTCTRL], || {})
            .err()
            .unwrap();
        assert!(matches!(err, Error::ManagerStopped));
    }

    #[test]
    fn sequence_registration_requires_at_least_two_steps() {
        let manager = manager_with_fake_backend();
        let sequence = HotkeySequence::new(vec![Hotkey::new(KeyCode::KEY_K, vec![])]).unwrap();

        let err = manager
            .register_sequence(&sequence, SequenceOptions::new(), || {})
            .err()
            .unwrap();

        assert!(matches!(err, Error::InvalidSequence(_)));
    }

    #[test]
    fn sequence_registration_rejects_zero_timeout() {
        let manager = manager_with_fake_backend();
        let sequence = HotkeySequence::new(vec![
            Hotkey::new(KeyCode::KEY_K, vec![]),
            Hotkey::new(KeyCode::KEY_C, vec![]),
        ])
        .unwrap();

        let err = manager
            .register_sequence(
                &sequence,
                SequenceOptions::new().timeout(Duration::ZERO),
                || {},
            )
            .err()
            .unwrap();

        assert!(matches!(err, Error::InvalidSequence(_)));
    }

    #[test]
    fn sequence_registration_rejects_invalid_step_bindings() {
        let manager = manager_with_fake_backend();
        let sequence = HotkeySequence::new(vec![
            Hotkey::new(KeyCode::KEY_LEFTCTRL, vec![]),
            Hotkey::new(KeyCode::KEY_C, vec![]),
        ])
        .unwrap();

        let err = manager
            .register_sequence(&sequence, SequenceOptions::new(), || {})
            .err()
            .unwrap();

        assert!(matches!(err, Error::InvalidHotkey(_)));
    }

    #[test]
    fn sequence_handle_unregisters_registered_sequence() {
        let manager = manager_with_fake_backend();
        let sequence = HotkeySequence::new(vec![
            Hotkey::new(KeyCode::KEY_K, vec![]),
            Hotkey::new(KeyCode::KEY_C, vec![]),
        ])
        .unwrap();

        let handle = manager
            .register_sequence(&sequence, SequenceOptions::new(), || {})
            .unwrap();

        assert_eq!(
            manager.inner.sequence_registrations.lock().unwrap().len(),
            1
        );

        handle.unregister().unwrap();

        assert!(manager
            .inner
            .sequence_registrations
            .lock()
            .unwrap()
            .is_empty());
    }

    #[test]
    fn sequence_registration_is_rejected_on_non_evdev_backend() {
        let manager = HotkeyManager {
            inner: Arc::new(HotkeyManagerInner {
                registrations: Arc::new(Mutex::new(HashMap::new())),
                sequence_registrations: Arc::new(Mutex::new(HashMap::new())),
                next_sequence_id: AtomicU64::new(1),
                backend_impl: Arc::new(FakeBackend),
                stop_flag: Arc::new(AtomicBool::new(false)),
                operation_lock: Mutex::new(()),
                listener: Mutex::new(None),
            }),
            active_backend: Backend::Portal,
        };

        let sequence = HotkeySequence::new(vec![
            Hotkey::new(KeyCode::KEY_K, vec![]),
            Hotkey::new(KeyCode::KEY_C, vec![]),
        ])
        .unwrap();

        let err = manager
            .register_sequence(&sequence, SequenceOptions::new(), || {})
            .err()
            .unwrap();

        assert!(matches!(err, Error::UnsupportedFeature(_)));
    }

    #[test]
    fn left_and_right_modifiers_conflict_after_normalization() {
        let mut registrations = HashMap::new();

        let key_left = (
            KeyCode::KEY_C,
            normalize_modifiers(&[KeyCode::KEY_LEFTSHIFT]),
        );
        let key_right = (
            KeyCode::KEY_C,
            normalize_modifiers(&[KeyCode::KEY_RIGHTSHIFT]),
        );

        insert_new_registration(
            &mut registrations,
            key_left,
            registration_with_counter(Arc::new(AtomicUsize::new(0))),
        )
        .unwrap();

        let err = insert_new_registration(
            &mut registrations,
            key_right,
            registration_with_counter(Arc::new(AtomicUsize::new(0))),
        )
        .err()
        .unwrap();

        assert!(matches!(err, Error::AlreadyRegistered { .. }));
    }
}
