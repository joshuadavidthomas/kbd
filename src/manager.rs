use crate::backend::{build_backend, resolve_backend, Backend};
use crate::device::DeviceFilter;
use crate::error::Error;
use crate::events::{EventHub, HotkeyEvent};
use crate::hotkey::{Hotkey, HotkeySequence};
use crate::key_state::SharedKeyState;
use crate::mode::{ModeBuilder, ModeController, ModeDefinition, ModeOptions, ModeRegistry};
use crate::tap_hold::{HoldAction, TapAction, TapHoldOptions, TapHoldRegistration};

use evdev::KeyCode;
use std::collections::{HashMap, HashSet};
use std::sync::{
    atomic::{AtomicBool, AtomicU64, Ordering},
    Arc, Mutex,
};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/// Callback storage type
pub(crate) type Callback = Arc<dyn Fn() + Send + Sync>;

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
    pub(crate) wait_for_release: bool,
    pub(crate) min_hold: Option<Duration>,
    pub(crate) repeat_behavior: RepeatBehavior,
    pub(crate) passthrough: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct PressTimingConfig {
    pub(crate) debounce: Option<Duration>,
    pub(crate) max_rate: Option<Duration>,
}

impl PressTimingConfig {
    pub(crate) fn new(debounce: Option<Duration>, max_rate: Option<Duration>) -> Self {
        Self {
            debounce: debounce.filter(|duration| !duration.is_zero()),
            max_rate: max_rate.filter(|duration| !duration.is_zero()),
        }
    }

    fn is_disabled(&self) -> bool {
        self.debounce.is_none() && self.max_rate.is_none()
    }
}

#[derive(Default)]
struct PressInvocationState {
    last_dispatch: Option<Instant>,
}

struct PressInvocationLimiter {
    config: PressTimingConfig,
    state: Mutex<PressInvocationState>,
}

impl PressInvocationLimiter {
    fn new(config: PressTimingConfig) -> Self {
        Self {
            config,
            state: Mutex::new(PressInvocationState::default()),
        }
    }

    fn should_dispatch_now(&self) -> bool {
        self.should_dispatch_at(Instant::now())
    }

    fn should_dispatch_at(&self, now: Instant) -> bool {
        if self.config.is_disabled() {
            return true;
        }

        let mut state = self.state.lock().unwrap();
        let Some(last_dispatch) = state.last_dispatch else {
            state.last_dispatch = Some(now);
            return true;
        };

        if self
            .config
            .debounce
            .is_some_and(|debounce| now.saturating_duration_since(last_dispatch) < debounce)
        {
            return false;
        }

        if self
            .config
            .max_rate
            .is_some_and(|max_rate| now.saturating_duration_since(last_dispatch) < max_rate)
        {
            return false;
        }

        state.last_dispatch = Some(now);
        true
    }
}

#[derive(Clone, Default)]
pub struct HotkeyOptions {
    release_behavior: ReleaseBehavior,
    min_hold: Option<Duration>,
    repeat_behavior: RepeatBehavior,
    passthrough: bool,
    debounce: Option<Duration>,
    max_rate: Option<Duration>,
    device_filter: Option<DeviceFilter>,
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

    pub fn trigger_on_repeat(mut self) -> Self {
        self.repeat_behavior = RepeatBehavior::Trigger;
        self
    }

    pub fn passthrough(mut self) -> Self {
        self.passthrough = true;
        self
    }

    /// Suppress press callback invocations until there has been at least this
    /// much quiet time since the previous press attempt.
    pub fn debounce(mut self, duration: Duration) -> Self {
        self.debounce = Some(duration);
        self
    }

    /// Cap press callback invocations to at most one successful dispatch per
    /// interval.
    pub fn max_rate(mut self, interval: Duration) -> Self {
        self.max_rate = Some(interval);
        self
    }

    /// Restrict this hotkey to events from devices matching the given filter.
    pub fn device(mut self, filter: DeviceFilter) -> Self {
        self.device_filter = Some(filter);
        self
    }

    pub(crate) fn press_timing_config(&self) -> PressTimingConfig {
        PressTimingConfig::new(self.debounce, self.max_rate)
    }

    pub(crate) fn build_callbacks<F>(self, callback: F) -> HotkeyCallbacks
    where
        F: Fn() + Send + Sync + 'static,
    {
        let press_callback: Callback = Arc::new(callback);
        let (release_callback, wait_for_release) = match self.release_behavior {
            ReleaseBehavior::Disabled => (None, false),
            ReleaseBehavior::SameAsPress => (Some(press_callback.clone()), true),
            ReleaseBehavior::Custom(callback) => (Some(callback), true),
        };

        HotkeyCallbacks {
            on_press: press_callback,
            on_release: release_callback,
            wait_for_release,
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

    pub fn grab(mut self) -> Self {
        self.options.grab = true;
        self
    }

    pub fn build(self) -> Result<HotkeyManager, Error> {
        HotkeyManager::with_backend_internal(self.requested_backend, self.options)
    }
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

/// Key used to identify hotkey registrations: (target_key, normalized_modifiers)
pub(crate) type HotkeyKey = (KeyCode, Vec<KeyCode>);

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

pub(crate) fn validate_hotkey_binding(key: KeyCode, modifiers: &[KeyCode]) -> Result<(), Error> {
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
            press_event_hub.emit(HotkeyEvent::Pressed(press_hotkey.clone()));
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
                    release_event_hub.emit(HotkeyEvent::Released(release_hotkey.clone()));
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
                    release_event_hub.emit(HotkeyEvent::Released(release_hotkey.clone()));
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

#[derive(Clone)]
enum RegistrationLocation {
    Global(HotkeyKey),
    Device(DeviceRegistrationId),
}

/// Handle for unregistering a specific hotkey
#[derive(Clone)]
pub struct Handle {
    location: RegistrationLocation,
    registration_marker: Callback,
    manager: Arc<HotkeyManagerInner>,
}

impl std::fmt::Debug for Handle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.location {
            RegistrationLocation::Global(key) => {
                f.debug_struct("Handle").field("key", key).finish()
            }
            RegistrationLocation::Device(id) => f
                .debug_struct("Handle")
                .field("device_registration_id", id)
                .finish(),
        }
    }
}

impl Handle {
    pub fn unregister(self) -> Result<(), Error> {
        match &self.location {
            RegistrationLocation::Global(key) => {
                self.manager.remove_hotkey(key, &self.registration_marker)
            }
            RegistrationLocation::Device(id) => {
                self.manager
                    .remove_device_hotkey(*id, &self.registration_marker);
                Ok(())
            }
        }
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

/// Handle for unregistering a tap-hold key binding.
#[derive(Clone)]
pub struct TapHoldHandle {
    key: KeyCode,
    registration_marker: Arc<()>,
    manager: Arc<HotkeyManagerInner>,
}

impl std::fmt::Debug for TapHoldHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TapHoldHandle")
            .field("key", &self.key)
            .finish()
    }
}

impl TapHoldHandle {
    pub fn unregister(self) -> Result<(), Error> {
        self.manager
            .remove_tap_hold(self.key, &self.registration_marker);
        Ok(())
    }
}

/// Inner state shared between HotkeyManager and Handles
struct HotkeyManagerInner {
    registrations: Arc<Mutex<HashMap<HotkeyKey, HotkeyRegistration>>>,
    sequence_registrations: Arc<Mutex<HashMap<SequenceId, SequenceRegistration>>>,
    device_registrations: Arc<Mutex<HashMap<DeviceRegistrationId, DeviceHotkeyRegistration>>>,
    tap_hold_registrations: Arc<Mutex<HashMap<KeyCode, TapHoldRegistration>>>,
    mode_registry: ModeRegistry,
    next_sequence_id: AtomicU64,
    next_device_reg_id: AtomicU64,
    backend_impl: Arc<dyn crate::backend::HotkeyBackend>,
    stop_flag: Arc<AtomicBool>,
    event_hub: EventHub,
    key_state: SharedKeyState,
    grab_enabled: bool,
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

    #[cfg(any(feature = "tokio", feature = "async-std"))]
    pub fn event_stream(&self) -> crate::events::HotkeyEventStream {
        self.inner.event_hub.subscribe()
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
        let event_hub = EventHub::default();
        let mode_registry = ModeRegistry::with_event_hub(event_hub.clone());
        let backend_impl: Arc<dyn crate::backend::HotkeyBackend> =
            build_backend(backend, options.grab, mode_registry.clone())?.into();

        let tap_hold_registrations = Arc::new(Mutex::new(HashMap::new()));
        let key_state = SharedKeyState::new();

        let inner = Arc::new(HotkeyManagerInner {
            registrations: Arc::new(Mutex::new(HashMap::new())),
            sequence_registrations: Arc::new(Mutex::new(HashMap::new())),
            device_registrations: Arc::new(Mutex::new(HashMap::new())),
            tap_hold_registrations: tap_hold_registrations.clone(),
            mode_registry,
            next_sequence_id: AtomicU64::new(1),
            next_device_reg_id: AtomicU64::new(1),
            backend_impl: backend_impl.clone(),
            stop_flag: Arc::new(AtomicBool::new(false)),
            event_hub,
            key_state: key_state.clone(),
            grab_enabled: options.grab,
            operation_lock: Mutex::new(()),
            listener: Mutex::new(None),
        });

        let listener = backend_impl.start_listener(
            inner.registrations.clone(),
            inner.sequence_registrations.clone(),
            inner.device_registrations.clone(),
            inner.tap_hold_registrations.clone(),
            inner.stop_flag.clone(),
            key_state,
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

        let device_filter = options.device_filter.clone();
        let hotkey_key = (key, normalize_modifiers(modifiers));

        if let Some(filter) = device_filter {
            if self.active_backend != Backend::Evdev {
                return Err(Error::UnsupportedFeature(
                    "device-specific hotkeys are only supported by the evdev backend".to_string(),
                ));
            }

            return self.register_device_hotkey(hotkey_key, filter, options, callback);
        }

        let press_timing = options.press_timing_config();
        let callbacks = attach_hotkey_events(
            options.build_callbacks(callback),
            &hotkey_key,
            &self.inner.event_hub,
            press_timing,
        );

        let registration = HotkeyRegistration { callbacks };
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
            location: RegistrationLocation::Global(hotkey_key),
            registration_marker,
            manager: self.inner.clone(),
        })
    }

    fn register_device_hotkey<F>(
        &self,
        hotkey_key: HotkeyKey,
        filter: DeviceFilter,
        options: HotkeyOptions,
        callback: F,
    ) -> Result<Handle, Error>
    where
        F: Fn() + Send + Sync + 'static,
    {
        {
            let device_registrations = self.inner.device_registrations.lock().unwrap();
            let conflict = device_registrations
                .values()
                .any(|existing| existing.hotkey_key == hotkey_key && existing.filter == filter);
            if conflict {
                return Err(already_registered_error(&hotkey_key));
            }
        }

        let press_timing = options.press_timing_config();
        let callbacks = attach_hotkey_events(
            options.build_callbacks(callback),
            &hotkey_key,
            &self.inner.event_hub,
            press_timing,
        );
        let registration_marker = callbacks.on_press.clone();

        let id = self.inner.next_device_reg_id.fetch_add(1, Ordering::SeqCst);

        let registration = DeviceHotkeyRegistration {
            hotkey_key,
            filter,
            callbacks,
        };

        self.inner
            .device_registrations
            .lock()
            .unwrap()
            .insert(id, registration);

        Ok(Handle {
            location: RegistrationLocation::Device(id),
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

        if is_modifier_key(abort_key) {
            return Err(Error::InvalidSequence(format!(
                "modifier keys cannot be used as the abort key: {:?}",
                abort_key
            )));
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
        let sequence_len = normalized_steps.len();
        let callback: Callback = Arc::new(callback);
        let callback_event_hub = self.inner.event_hub.clone();
        let wrapped_callback: Callback = Arc::new(move || {
            callback_event_hub.emit(HotkeyEvent::SequenceStep {
                id: sequence_id,
                step: sequence_len,
                total: sequence_len,
            });
            callback();
        });

        let registration = SequenceRegistration {
            steps: normalized_steps,
            callback: wrapped_callback,
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

    /// Define a named mode with its bindings and options.
    pub fn define_mode<F>(&self, name: &str, options: ModeOptions, build_fn: F) -> Result<(), Error>
    where
        F: FnOnce(&mut ModeBuilder) -> Result<(), Error>,
    {
        let _operation_guard = self.inner.operation_lock.lock().unwrap();

        if self.inner.stop_flag.load(Ordering::SeqCst) {
            return Err(Error::ManagerStopped);
        }

        if self.active_backend != Backend::Evdev {
            return Err(Error::UnsupportedFeature(
                "modes are only supported by the evdev backend".to_string(),
            ));
        }

        {
            let definitions = self.inner.mode_registry.definitions.lock().unwrap();
            if definitions.contains_key(name) {
                return Err(Error::ModeAlreadyDefined(name.to_string()));
            }
        }

        let mut builder = ModeBuilder::new(self.mode_controller());
        build_fn(&mut builder)?;

        let definition = ModeDefinition {
            options,
            bindings: builder.bindings,
        };

        self.inner
            .mode_registry
            .definitions
            .lock()
            .unwrap()
            .insert(name.to_string(), definition);

        Ok(())
    }

    /// Get a mode controller for push/pop operations from callbacks.
    pub fn mode_controller(&self) -> ModeController {
        ModeController::new(self.inner.mode_registry.clone())
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

        let device_filter = options.device_filter.clone();
        let hotkey_key = (key, normalize_modifiers(modifiers));

        if let Some(filter) = device_filter {
            if self.active_backend != Backend::Evdev {
                return Err(Error::UnsupportedFeature(
                    "device-specific hotkeys are only supported by the evdev backend".to_string(),
                ));
            }

            return self.replace_device_hotkey(hotkey_key, filter, options, callback);
        }

        let press_timing = options.press_timing_config();
        let callbacks = attach_hotkey_events(
            options.build_callbacks(callback),
            &hotkey_key,
            &self.inner.event_hub,
            press_timing,
        );

        let registration = HotkeyRegistration { callbacks };
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
            location: RegistrationLocation::Global(hotkey_key),
            registration_marker,
            manager: self.inner.clone(),
        })
    }

    fn replace_device_hotkey<F>(
        &self,
        hotkey_key: HotkeyKey,
        filter: DeviceFilter,
        options: HotkeyOptions,
        callback: F,
    ) -> Result<Handle, Error>
    where
        F: Fn() + Send + Sync + 'static,
    {
        let press_timing = options.press_timing_config();
        let callbacks = attach_hotkey_events(
            options.build_callbacks(callback),
            &hotkey_key,
            &self.inner.event_hub,
            press_timing,
        );
        let registration_marker = callbacks.on_press.clone();

        let mut device_registrations = self.inner.device_registrations.lock().unwrap();

        // Find and replace existing registration with same key+filter
        let existing_id = device_registrations
            .iter()
            .find(|(_, existing)| existing.hotkey_key == hotkey_key && existing.filter == filter)
            .map(|(id, _)| *id);

        let id = if let Some(existing_id) = existing_id {
            device_registrations.insert(
                existing_id,
                DeviceHotkeyRegistration {
                    hotkey_key,
                    filter,
                    callbacks,
                },
            );
            existing_id
        } else {
            let id = self.inner.next_device_reg_id.fetch_add(1, Ordering::SeqCst);
            device_registrations.insert(
                id,
                DeviceHotkeyRegistration {
                    hotkey_key,
                    filter,
                    callbacks,
                },
            );
            id
        };

        Ok(Handle {
            location: RegistrationLocation::Device(id),
            registration_marker,
            manager: self.inner.clone(),
        })
    }

    /// Register a dual-function tap-hold key.
    ///
    /// The key performs one action when tapped (pressed and released quickly)
    /// and a different action when held. Requires event grabbing to be enabled.
    pub fn register_tap_hold(
        &self,
        key: KeyCode,
        tap_action: TapAction,
        hold_action: HoldAction,
        options: TapHoldOptions,
    ) -> Result<TapHoldHandle, Error> {
        let _operation_guard = self.inner.operation_lock.lock().unwrap();

        if self.inner.stop_flag.load(Ordering::SeqCst) {
            return Err(Error::ManagerStopped);
        }

        if !self.inner.grab_enabled {
            return Err(Error::UnsupportedFeature(
                "tap-hold requires event grabbing (use HotkeyManager::builder().grab().build())"
                    .to_string(),
            ));
        }

        if is_modifier_key(key) {
            return Err(Error::InvalidHotkey(format!(
                "modifier keys cannot be used as tap-hold keys: {:?}",
                key
            )));
        }

        {
            let tap_hold_regs = self.inner.tap_hold_registrations.lock().unwrap();
            if tap_hold_regs.contains_key(&key) {
                return Err(Error::AlreadyRegistered {
                    key,
                    modifiers: vec![],
                });
            }
        }

        let registration_marker = Arc::new(());
        let registration = TapHoldRegistration {
            tap_action,
            hold_action,
            threshold: options.threshold,
            marker: registration_marker.clone(),
        };

        self.inner
            .tap_hold_registrations
            .lock()
            .unwrap()
            .insert(key, registration);

        Ok(TapHoldHandle {
            key,
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

    /// Returns whether the given key is currently pressed.
    pub fn is_key_pressed(&self, key: KeyCode) -> bool {
        self.inner.key_state.is_pressed(key)
    }

    /// Returns the set of currently pressed modifier keys.
    pub fn active_modifiers(&self) -> HashSet<KeyCode> {
        self.inner.key_state.active_modifiers()
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
            self.inner.device_registrations.lock().unwrap().clear();
            self.inner.tap_hold_registrations.lock().unwrap().clear();
            self.inner.mode_registry.definitions.lock().unwrap().clear();

            let had_active_mode = {
                let mut mode_stack = self.inner.mode_registry.stack.lock().unwrap();
                let had_active_mode = !mode_stack.is_empty();
                mode_stack.clear();
                had_active_mode
            };

            if had_active_mode {
                self.inner.event_hub.emit(HotkeyEvent::ModeChanged(None));
            }

            self.inner.event_hub.close();
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

        self.inner.key_state.clear();

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

    fn remove_device_hotkey(&self, id: DeviceRegistrationId, registration_marker: &Callback) {
        let _operation_guard = self.operation_lock.lock().unwrap();
        let mut device_registrations = self.device_registrations.lock().unwrap();

        let is_current_registration = device_registrations
            .get(&id)
            .is_some_and(|registration| {
                Arc::ptr_eq(&registration.callbacks.on_press, registration_marker)
            });

        if is_current_registration {
            device_registrations.remove(&id);
        }
    }

    fn remove_tap_hold(&self, key: KeyCode, registration_marker: &Arc<()>) {
        let _operation_guard = self.operation_lock.lock().unwrap();
        let mut tap_hold_registrations = self.tap_hold_registrations.lock().unwrap();

        let is_current_registration = tap_hold_registrations
            .get(&key)
            .is_some_and(|registration| Arc::ptr_eq(&registration.marker, registration_marker));

        if is_current_registration {
            tap_hold_registrations.remove(&key);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(any(feature = "tokio", feature = "async-std"))]
    use crate::events::HotkeyEvent;
    use std::collections::{HashMap, HashSet};
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
            .trigger_on_repeat()
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
    fn release_callback_presence_tracks_user_options() {
        let default_callbacks = HotkeyOptions::new().build_callbacks(|| {});
        assert!(default_callbacks.on_release.is_none());

        let same_as_press_callbacks = HotkeyOptions::new().on_release().build_callbacks(|| {});
        assert!(same_as_press_callbacks.on_release.is_some());

        let custom_release_callbacks = HotkeyOptions::new()
            .on_release_callback(|| {})
            .build_callbacks(|| {});
        assert!(custom_release_callbacks.on_release.is_some());
    }

    #[test]
    fn passthrough_option_is_stored_in_callbacks() {
        let callbacks = HotkeyOptions::new().passthrough().build_callbacks(|| {});
        assert!(callbacks.passthrough);
    }

    #[test]
    fn debounce_limiter_suppresses_rapid_retriggers() {
        let limiter = PressInvocationLimiter::new(PressTimingConfig::new(
            Some(Duration::from_millis(100)),
            None,
        ));
        let start = Instant::now();

        assert!(limiter.should_dispatch_at(start));
        assert!(!limiter.should_dispatch_at(start + Duration::from_millis(50)));
        assert!(limiter.should_dispatch_at(start + Duration::from_millis(130)));
        assert!(!limiter.should_dispatch_at(start + Duration::from_millis(200)));
    }

    #[test]
    fn rate_limit_caps_invocations_to_interval() {
        let limiter = PressInvocationLimiter::new(PressTimingConfig::new(
            None,
            Some(Duration::from_millis(100)),
        ));
        let start = Instant::now();

        assert!(limiter.should_dispatch_at(start));
        assert!(!limiter.should_dispatch_at(start + Duration::from_millis(50)));
        assert!(limiter.should_dispatch_at(start + Duration::from_millis(100)));
        assert!(!limiter.should_dispatch_at(start + Duration::from_millis(150)));
    }

    #[test]
    fn debounce_and_rate_limit_can_be_combined() {
        let limiter = PressInvocationLimiter::new(PressTimingConfig::new(
            Some(Duration::from_millis(100)),
            Some(Duration::from_millis(300)),
        ));
        let start = Instant::now();

        assert!(limiter.should_dispatch_at(start));
        assert!(!limiter.should_dispatch_at(start + Duration::from_millis(50)));
        assert!(!limiter.should_dispatch_at(start + Duration::from_millis(220)));
        assert!(!limiter.should_dispatch_at(start + Duration::from_millis(280)));
        assert!(limiter.should_dispatch_at(start + Duration::from_millis(400)));
    }

    #[test]
    fn hotkey_options_store_timing_configuration() {
        let config = HotkeyOptions::new()
            .debounce(Duration::from_millis(75))
            .max_rate(Duration::from_millis(250))
            .press_timing_config();

        assert_eq!(config.debounce, Some(Duration::from_millis(75)));
        assert_eq!(config.max_rate, Some(Duration::from_millis(250)));
    }

    #[test]
    fn rate_limit_applies_across_press_and_release_callbacks() {
        let invocations = Arc::new(AtomicUsize::new(0));
        let invocations_clone = invocations.clone();

        let options = HotkeyOptions::new()
            .on_release()
            .max_rate(Duration::from_secs(60));
        let press_timing = options.press_timing_config();
        let callbacks = attach_hotkey_events(
            options.build_callbacks(move || {
                invocations_clone.fetch_add(1, Ordering::SeqCst);
            }),
            &(KeyCode::KEY_A, vec![]),
            &crate::events::EventHub::default(),
            press_timing,
        );

        (callbacks.on_press)();
        callbacks.on_release.as_ref().unwrap()();

        assert_eq!(invocations.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn limiter_handles_non_monotonic_timestamps_without_panicking() {
        let limiter = PressInvocationLimiter::new(PressTimingConfig::new(
            Some(Duration::from_millis(100)),
            Some(Duration::from_millis(100)),
        ));
        let start = Instant::now();
        assert!(limiter.should_dispatch_at(start));

        if let Some(earlier) = start.checked_sub(Duration::from_millis(1)) {
            assert!(!limiter.should_dispatch_at(earlier));
        }
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
            _device_registrations: Arc<
                Mutex<HashMap<DeviceRegistrationId, DeviceHotkeyRegistration>>,
            >,
            _tap_hold_registrations: Arc<
                Mutex<HashMap<KeyCode, crate::tap_hold::TapHoldRegistration>>,
            >,
            _stop_flag: Arc<AtomicBool>,
            _key_state: SharedKeyState,
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
            _device_registrations: Arc<
                Mutex<HashMap<DeviceRegistrationId, DeviceHotkeyRegistration>>,
            >,
            _tap_hold_registrations: Arc<
                Mutex<HashMap<KeyCode, crate::tap_hold::TapHoldRegistration>>,
            >,
            _stop_flag: Arc<AtomicBool>,
            _key_state: SharedKeyState,
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
            _device_registrations: Arc<
                Mutex<HashMap<DeviceRegistrationId, DeviceHotkeyRegistration>>,
            >,
            _tap_hold_registrations: Arc<
                Mutex<HashMap<KeyCode, crate::tap_hold::TapHoldRegistration>>,
            >,
            _stop_flag: Arc<AtomicBool>,
            _key_state: SharedKeyState,
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
            _device_registrations: Arc<
                Mutex<HashMap<DeviceRegistrationId, DeviceHotkeyRegistration>>,
            >,
            _tap_hold_registrations: Arc<
                Mutex<HashMap<KeyCode, crate::tap_hold::TapHoldRegistration>>,
            >,
            _stop_flag: Arc<AtomicBool>,
            _key_state: SharedKeyState,
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
            _device_registrations: Arc<
                Mutex<HashMap<DeviceRegistrationId, DeviceHotkeyRegistration>>,
            >,
            _tap_hold_registrations: Arc<
                Mutex<HashMap<KeyCode, crate::tap_hold::TapHoldRegistration>>,
            >,
            _stop_flag: Arc<AtomicBool>,
            _key_state: SharedKeyState,
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
            _device_registrations: Arc<
                Mutex<HashMap<DeviceRegistrationId, DeviceHotkeyRegistration>>,
            >,
            _tap_hold_registrations: Arc<
                Mutex<HashMap<KeyCode, crate::tap_hold::TapHoldRegistration>>,
            >,
            _stop_flag: Arc<AtomicBool>,
            _key_state: SharedKeyState,
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
            _device_registrations: Arc<
                Mutex<HashMap<DeviceRegistrationId, DeviceHotkeyRegistration>>,
            >,
            _tap_hold_registrations: Arc<
                Mutex<HashMap<KeyCode, crate::tap_hold::TapHoldRegistration>>,
            >,
            _stop_flag: Arc<AtomicBool>,
            _key_state: SharedKeyState,
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
            _device_registrations: Arc<
                Mutex<HashMap<DeviceRegistrationId, DeviceHotkeyRegistration>>,
            >,
            _tap_hold_registrations: Arc<
                Mutex<HashMap<KeyCode, crate::tap_hold::TapHoldRegistration>>,
            >,
            _stop_flag: Arc<AtomicBool>,
            _key_state: SharedKeyState,
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
            _device_registrations: Arc<
                Mutex<HashMap<DeviceRegistrationId, DeviceHotkeyRegistration>>,
            >,
            _tap_hold_registrations: Arc<
                Mutex<HashMap<KeyCode, crate::tap_hold::TapHoldRegistration>>,
            >,
            _stop_flag: Arc<AtomicBool>,
            _key_state: SharedKeyState,
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
        manager_with_backend_and_grab(backend_impl, false)
    }

    fn manager_with_backend_and_grab(
        backend_impl: Arc<dyn crate::backend::HotkeyBackend>,
        grab_enabled: bool,
    ) -> HotkeyManager {
        let event_hub = EventHub::default();
        let mode_registry = ModeRegistry::with_event_hub(event_hub.clone());

        HotkeyManager {
            inner: Arc::new(HotkeyManagerInner {
                registrations: Arc::new(Mutex::new(HashMap::new())),
                sequence_registrations: Arc::new(Mutex::new(HashMap::new())),
                device_registrations: Arc::new(Mutex::new(HashMap::new())),
                tap_hold_registrations: Arc::new(Mutex::new(HashMap::new())),
                mode_registry,
                next_sequence_id: AtomicU64::new(1),
                next_device_reg_id: AtomicU64::new(1),
                backend_impl,
                stop_flag: Arc::new(AtomicBool::new(false)),
                event_hub,
                key_state: SharedKeyState::new(),
                grab_enabled,
                operation_lock: Mutex::new(()),
                listener: Mutex::new(None),
            }),
            active_backend: Backend::Evdev,
        }
    }

    fn portal_manager_with_fake_backend() -> HotkeyManager {
        let event_hub = EventHub::default();
        let mode_registry = ModeRegistry::with_event_hub(event_hub.clone());

        HotkeyManager {
            inner: Arc::new(HotkeyManagerInner {
                registrations: Arc::new(Mutex::new(HashMap::new())),
                sequence_registrations: Arc::new(Mutex::new(HashMap::new())),
                device_registrations: Arc::new(Mutex::new(HashMap::new())),
                tap_hold_registrations: Arc::new(Mutex::new(HashMap::new())),
                mode_registry,
                next_sequence_id: AtomicU64::new(1),
                next_device_reg_id: AtomicU64::new(1),
                backend_impl: Arc::new(FakeBackend),
                stop_flag: Arc::new(AtomicBool::new(false)),
                event_hub,
                key_state: SharedKeyState::new(),
                grab_enabled: false,
                operation_lock: Mutex::new(()),
                listener: Mutex::new(None),
            }),
            active_backend: Backend::Portal,
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
                wait_for_release: false,
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

    #[cfg(feature = "async-std")]
    fn block_on_future<F>(future: F) -> F::Output
    where
        F: std::future::Future,
    {
        async_std::task::block_on(future)
    }

    #[cfg(all(feature = "tokio", not(feature = "async-std")))]
    fn block_on_future<F>(future: F) -> F::Output
    where
        F: std::future::Future,
    {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .expect("tokio runtime should build");
        runtime.block_on(future)
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
    fn key_state_query_reports_pressed_then_released_key() {
        let manager = manager_with_fake_backend();

        manager.inner.key_state.press(KeyCode::KEY_A);
        assert!(manager.is_key_pressed(KeyCode::KEY_A));

        manager.inner.key_state.release(KeyCode::KEY_A);
        assert!(!manager.is_key_pressed(KeyCode::KEY_A));
    }

    #[test]
    fn key_state_query_returns_active_modifiers_only() {
        let manager = manager_with_fake_backend();

        manager.inner.key_state.press(KeyCode::KEY_LEFTCTRL);
        manager.inner.key_state.press(KeyCode::KEY_RIGHTSHIFT);
        manager.inner.key_state.press(KeyCode::KEY_A);

        let active_modifiers = manager.active_modifiers();
        let expected: HashSet<KeyCode> = [KeyCode::KEY_LEFTCTRL, KeyCode::KEY_RIGHTSHIFT]
            .into_iter()
            .collect();

        assert_eq!(active_modifiers, expected);
    }

    #[test]
    fn key_state_queries_are_thread_safe_for_concurrent_reads() {
        let manager = Arc::new(manager_with_fake_backend());
        manager.inner.key_state.press(KeyCode::KEY_LEFTCTRL);

        let start = Arc::new(Barrier::new(5));
        let mut threads = Vec::new();

        for _ in 0..4 {
            let manager_clone = manager.clone();
            let start_clone = start.clone();
            threads.push(std::thread::spawn(move || {
                start_clone.wait();
                for _ in 0..1_000 {
                    assert!(manager_clone.is_key_pressed(KeyCode::KEY_LEFTCTRL));
                    assert!(manager_clone
                        .active_modifiers()
                        .contains(&KeyCode::KEY_LEFTCTRL));
                }
            }));
        }

        start.wait();

        for handle in threads {
            handle.join().expect("reader thread should not panic");
        }
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
    fn stale_device_handle_unregister_does_not_remove_replaced_registration() {
        let manager = manager_with_fake_backend();
        let filter = DeviceFilter::name_contains("StreamDeck");
        let calls = Arc::new(AtomicUsize::new(0));

        let stale_handle = manager
            .register_with_options(
                KeyCode::KEY_4,
                &[],
                HotkeyOptions::new().device(filter.clone()),
                || {},
            )
            .unwrap();

        let calls_clone = calls.clone();
        manager
            .replace_with_options(
                KeyCode::KEY_4,
                &[],
                HotkeyOptions::new().device(filter),
                move || {
                    calls_clone.fetch_add(1, Ordering::SeqCst);
                },
            )
            .unwrap();

        stale_handle.unregister().unwrap();

        let device_registrations = manager.inner.device_registrations.lock().unwrap();
        assert_eq!(device_registrations.len(), 1);
        let registration = device_registrations.values().next().unwrap();
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
        let manager = portal_manager_with_fake_backend();

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
    fn sequence_registration_rejects_modifier_as_abort_key() {
        let manager = manager_with_fake_backend();
        let sequence = HotkeySequence::new(vec![
            Hotkey::new(KeyCode::KEY_K, vec![]),
            Hotkey::new(KeyCode::KEY_C, vec![]),
        ])
        .unwrap();

        let err = manager
            .register_sequence(
                &sequence,
                SequenceOptions::new().abort_key(KeyCode::KEY_LEFTCTRL),
                || {},
            )
            .err()
            .unwrap();

        assert!(matches!(err, Error::InvalidSequence(_)));
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

    #[test]
    fn define_mode_stores_bindings() {
        let manager = manager_with_fake_backend();

        manager
            .define_mode("resize", ModeOptions::new(), |m| {
                m.register(KeyCode::KEY_H, &[], || {})?;
                m.register(KeyCode::KEY_J, &[], || {})?;
                Ok(())
            })
            .unwrap();

        let definitions = manager.inner.mode_registry.definitions.lock().unwrap();
        let definition = definitions.get("resize").unwrap();
        assert_eq!(definition.bindings.len(), 2);
    }

    #[test]
    fn define_mode_rejects_duplicate_name() {
        let manager = manager_with_fake_backend();

        manager
            .define_mode("resize", ModeOptions::new(), |_m| Ok(()))
            .unwrap();

        let err = manager
            .define_mode("resize", ModeOptions::new(), |_m| Ok(()))
            .err()
            .unwrap();

        assert!(matches!(err, Error::ModeAlreadyDefined(_)));
    }

    #[test]
    fn define_mode_validates_bindings() {
        let manager = manager_with_fake_backend();

        let err = manager
            .define_mode("bad", ModeOptions::new(), |m| {
                m.register(KeyCode::KEY_LEFTCTRL, &[], || {})?;
                Ok(())
            })
            .err()
            .unwrap();

        assert!(matches!(err, Error::InvalidHotkey(_)));
    }

    #[test]
    fn define_mode_stores_options() {
        let manager = manager_with_fake_backend();

        manager
            .define_mode(
                "launch",
                ModeOptions::new().oneshot().swallow(),
                |_m| Ok(()),
            )
            .unwrap();

        let definitions = manager.inner.mode_registry.definitions.lock().unwrap();
        let definition = definitions.get("launch").unwrap();
        assert!(definition.options.oneshot);
        assert!(definition.options.swallow);
    }

    #[test]
    fn mode_controller_push_and_pop_through_manager() {
        let manager = manager_with_fake_backend();

        manager
            .define_mode("test_mode", ModeOptions::new(), |_m| Ok(()))
            .unwrap();

        let mc = manager.mode_controller();
        assert!(mc.active_mode().is_none());

        mc.push("test_mode");
        assert_eq!(mc.active_mode(), Some("test_mode".to_string()));

        mc.pop();
        assert!(mc.active_mode().is_none());
    }

    #[test]
    fn same_key_in_different_modes_no_conflict() {
        let manager = manager_with_fake_backend();

        manager
            .define_mode("mode_a", ModeOptions::new(), |m| {
                m.register(KeyCode::KEY_F, &[], || {})?;
                Ok(())
            })
            .unwrap();

        manager
            .define_mode("mode_b", ModeOptions::new(), |m| {
                m.register(KeyCode::KEY_F, &[], || {})?;
                Ok(())
            })
            .unwrap();

        // Both modes registered KEY_F without conflict
        let definitions = manager.inner.mode_registry.definitions.lock().unwrap();
        assert!(definitions
            .get("mode_a")
            .unwrap()
            .bindings
            .contains_key(&(KeyCode::KEY_F, vec![])));
        assert!(definitions
            .get("mode_b")
            .unwrap()
            .bindings
            .contains_key(&(KeyCode::KEY_F, vec![])));
    }

    #[test]
    fn mode_builder_provides_mode_controller() {
        let manager = manager_with_fake_backend();

        manager
            .define_mode("test", ModeOptions::new(), |_m| Ok(()))
            .unwrap();

        let result = manager.define_mode("nested", ModeOptions::new(), |m| {
            let mc = m.mode_controller();
            // Controller should be functional
            mc.push("test");
            assert_eq!(mc.active_mode(), Some("test".to_string()));
            mc.pop();
            Ok(())
        });

        assert!(result.is_ok());
    }

    #[test]
    fn define_mode_rejected_after_manager_stopped() {
        let manager = manager_with_fake_backend();
        manager.unregister_all().unwrap();

        let err = manager
            .define_mode("late", ModeOptions::new(), |_m| Ok(()))
            .err()
            .unwrap();

        assert!(matches!(err, Error::ManagerStopped));
    }

    #[test]
    fn define_mode_is_rejected_on_non_evdev_backend() {
        let manager = portal_manager_with_fake_backend();

        let err = manager
            .define_mode("resize", ModeOptions::new(), |_m| Ok(()))
            .err()
            .unwrap();

        assert!(matches!(err, Error::UnsupportedFeature(_)));
    }

    #[test]
    fn device_specific_registration_stores_in_device_registrations() {
        let manager = manager_with_fake_backend();

        let handle = manager
            .register_with_options(
                KeyCode::KEY_1,
                &[],
                HotkeyOptions::new().device(DeviceFilter::name_contains("StreamDeck")),
                || {},
            )
            .unwrap();

        // Should NOT be in global registrations
        assert!(!manager.is_registered(KeyCode::KEY_1, &[]));

        // Should be in device registrations
        assert_eq!(manager.inner.device_registrations.lock().unwrap().len(), 1);

        handle.unregister().unwrap();
        assert!(manager
            .inner
            .device_registrations
            .lock()
            .unwrap()
            .is_empty());
    }

    #[test]
    fn device_specific_duplicate_same_filter_returns_error() {
        let manager = manager_with_fake_backend();
        let filter = DeviceFilter::name_contains("StreamDeck");

        manager
            .register_with_options(
                KeyCode::KEY_1,
                &[],
                HotkeyOptions::new().device(filter.clone()),
                || {},
            )
            .unwrap();

        let err = manager
            .register_with_options(
                KeyCode::KEY_1,
                &[],
                HotkeyOptions::new().device(filter),
                || {},
            )
            .err()
            .unwrap();

        assert!(matches!(err, Error::AlreadyRegistered { .. }));
    }

    #[test]
    fn device_specific_different_filter_no_conflict() {
        let manager = manager_with_fake_backend();

        manager
            .register_with_options(
                KeyCode::KEY_1,
                &[],
                HotkeyOptions::new().device(DeviceFilter::name_contains("StreamDeck")),
                || {},
            )
            .unwrap();

        manager
            .register_with_options(
                KeyCode::KEY_1,
                &[],
                HotkeyOptions::new().device(DeviceFilter::usb(0x1234, 0x5678)),
                || {},
            )
            .unwrap();

        assert_eq!(manager.inner.device_registrations.lock().unwrap().len(), 2);
    }

    #[test]
    fn device_specific_and_global_same_key_no_conflict() {
        let manager = manager_with_fake_backend();

        // Global registration
        manager.register(KeyCode::KEY_1, &[], || {}).unwrap();

        // Device-specific registration with same key
        manager
            .register_with_options(
                KeyCode::KEY_1,
                &[],
                HotkeyOptions::new().device(DeviceFilter::name_contains("StreamDeck")),
                || {},
            )
            .unwrap();

        assert!(manager.is_registered(KeyCode::KEY_1, &[]));
        assert_eq!(manager.inner.device_registrations.lock().unwrap().len(), 1);
    }

    #[test]
    fn device_specific_replace_overwrites_existing() {
        let manager = manager_with_fake_backend();
        let filter = DeviceFilter::name_contains("StreamDeck");
        let count = Arc::new(AtomicUsize::new(0));

        manager
            .register_with_options(
                KeyCode::KEY_1,
                &[],
                HotkeyOptions::new().device(filter.clone()),
                || {},
            )
            .unwrap();

        let count_clone = count.clone();
        manager
            .replace_with_options(
                KeyCode::KEY_1,
                &[],
                HotkeyOptions::new().device(filter),
                move || {
                    count_clone.fetch_add(1, Ordering::SeqCst);
                },
            )
            .unwrap();

        // Should still be one registration
        let device_regs = manager.inner.device_registrations.lock().unwrap();
        assert_eq!(device_regs.len(), 1);
        let (_, reg) = device_regs.iter().next().unwrap();
        (reg.callbacks.on_press)();
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn device_specific_rejected_on_portal_backend() {
        let manager = portal_manager_with_fake_backend();

        let err = manager
            .register_with_options(
                KeyCode::KEY_1,
                &[],
                HotkeyOptions::new().device(DeviceFilter::name_contains("StreamDeck")),
                || {},
            )
            .err()
            .unwrap();

        assert!(matches!(err, Error::UnsupportedFeature(_)));
    }

    #[test]
    fn unregister_all_clears_device_registrations() {
        let manager = manager_with_fake_backend();

        manager
            .register_with_options(
                KeyCode::KEY_1,
                &[],
                HotkeyOptions::new().device(DeviceFilter::name_contains("StreamDeck")),
                || {},
            )
            .unwrap();

        manager.unregister_all().unwrap();

        assert!(manager
            .inner
            .device_registrations
            .lock()
            .unwrap()
            .is_empty());
    }

    #[test]
    fn unregister_all_clears_key_state_queries() {
        let manager = manager_with_fake_backend();
        manager.inner.key_state.press(KeyCode::KEY_A);
        manager.inner.key_state.press(KeyCode::KEY_LEFTCTRL);

        manager.unregister_all().unwrap();

        assert!(!manager.is_key_pressed(KeyCode::KEY_A));
        assert!(manager.active_modifiers().is_empty());
    }

    #[test]
    fn tap_hold_requires_grab_enabled() {
        let manager = manager_with_fake_backend();

        let err = manager
            .register_tap_hold(
                KeyCode::KEY_CAPSLOCK,
                crate::tap_hold::TapAction::emit(KeyCode::KEY_ESC),
                crate::tap_hold::HoldAction::modifier(KeyCode::KEY_LEFTCTRL),
                crate::tap_hold::TapHoldOptions::new(),
            )
            .err()
            .unwrap();

        assert!(matches!(err, Error::UnsupportedFeature(_)));
    }

    #[test]
    fn tap_hold_succeeds_with_grab_enabled() {
        let manager = manager_with_backend_and_grab(Arc::new(FakeBackend), true);

        let handle = manager
            .register_tap_hold(
                KeyCode::KEY_CAPSLOCK,
                crate::tap_hold::TapAction::emit(KeyCode::KEY_ESC),
                crate::tap_hold::HoldAction::modifier(KeyCode::KEY_LEFTCTRL),
                crate::tap_hold::TapHoldOptions::new(),
            )
            .unwrap();

        assert!(manager
            .inner
            .tap_hold_registrations
            .lock()
            .unwrap()
            .contains_key(&KeyCode::KEY_CAPSLOCK));

        handle.unregister().unwrap();

        assert!(!manager
            .inner
            .tap_hold_registrations
            .lock()
            .unwrap()
            .contains_key(&KeyCode::KEY_CAPSLOCK));
    }

    #[test]
    fn tap_hold_rejects_modifier_keys() {
        let manager = manager_with_backend_and_grab(Arc::new(FakeBackend), true);

        let err = manager
            .register_tap_hold(
                KeyCode::KEY_LEFTCTRL,
                crate::tap_hold::TapAction::emit(KeyCode::KEY_ESC),
                crate::tap_hold::HoldAction::modifier(KeyCode::KEY_LEFTCTRL),
                crate::tap_hold::TapHoldOptions::new(),
            )
            .err()
            .unwrap();

        assert!(matches!(err, Error::InvalidHotkey(_)));
    }

    #[test]
    fn tap_hold_rejects_duplicate_key() {
        let manager = manager_with_backend_and_grab(Arc::new(FakeBackend), true);

        manager
            .register_tap_hold(
                KeyCode::KEY_CAPSLOCK,
                crate::tap_hold::TapAction::emit(KeyCode::KEY_ESC),
                crate::tap_hold::HoldAction::modifier(KeyCode::KEY_LEFTCTRL),
                crate::tap_hold::TapHoldOptions::new(),
            )
            .unwrap();

        let err = manager
            .register_tap_hold(
                KeyCode::KEY_CAPSLOCK,
                crate::tap_hold::TapAction::emit(KeyCode::KEY_TAB),
                crate::tap_hold::HoldAction::modifier(KeyCode::KEY_LEFTALT),
                crate::tap_hold::TapHoldOptions::new(),
            )
            .err()
            .unwrap();

        assert!(matches!(err, Error::AlreadyRegistered { .. }));
    }

    #[test]
    fn stale_tap_hold_handle_unregister_does_not_remove_new_registration() {
        let manager = manager_with_backend_and_grab(Arc::new(FakeBackend), true);

        let stale_handle = manager
            .register_tap_hold(
                KeyCode::KEY_CAPSLOCK,
                crate::tap_hold::TapAction::emit(KeyCode::KEY_ESC),
                crate::tap_hold::HoldAction::modifier(KeyCode::KEY_LEFTCTRL),
                crate::tap_hold::TapHoldOptions::new(),
            )
            .unwrap();

        let stale_clone = stale_handle.clone();
        stale_handle.unregister().unwrap();

        manager
            .register_tap_hold(
                KeyCode::KEY_CAPSLOCK,
                crate::tap_hold::TapAction::emit(KeyCode::KEY_TAB),
                crate::tap_hold::HoldAction::modifier(KeyCode::KEY_LEFTALT),
                crate::tap_hold::TapHoldOptions::new(),
            )
            .unwrap();

        stale_clone.unregister().unwrap();

        assert!(manager
            .inner
            .tap_hold_registrations
            .lock()
            .unwrap()
            .contains_key(&KeyCode::KEY_CAPSLOCK));
    }

    #[test]
    fn tap_hold_rejected_when_manager_stopped() {
        let manager = manager_with_backend_and_grab(Arc::new(FakeBackend), true);
        manager.inner.stop_flag.store(true, Ordering::SeqCst);

        let err = manager
            .register_tap_hold(
                KeyCode::KEY_CAPSLOCK,
                crate::tap_hold::TapAction::emit(KeyCode::KEY_ESC),
                crate::tap_hold::HoldAction::modifier(KeyCode::KEY_LEFTCTRL),
                crate::tap_hold::TapHoldOptions::new(),
            )
            .err()
            .unwrap();

        assert!(matches!(err, Error::ManagerStopped));
    }

    #[test]
    fn unregister_all_clears_tap_hold_registrations() {
        let manager = manager_with_backend_and_grab(Arc::new(FakeBackend), true);

        manager
            .register_tap_hold(
                KeyCode::KEY_CAPSLOCK,
                crate::tap_hold::TapAction::emit(KeyCode::KEY_ESC),
                crate::tap_hold::HoldAction::modifier(KeyCode::KEY_LEFTCTRL),
                crate::tap_hold::TapHoldOptions::new(),
            )
            .unwrap();

        manager.unregister_all().unwrap();

        assert!(manager
            .inner
            .tap_hold_registrations
            .lock()
            .unwrap()
            .is_empty());
    }

    #[test]
    #[cfg(any(feature = "tokio", feature = "async-std"))]
    fn event_stream_delivers_hotkey_sequence_and_mode_events() {
        let manager = manager_with_fake_backend();
        let mut stream = manager.event_stream();

        let callback_count = Arc::new(AtomicUsize::new(0));
        let callback_count_clone = callback_count.clone();

        manager
            .register_with_options(
                KeyCode::KEY_A,
                &[KeyCode::KEY_LEFTCTRL],
                HotkeyOptions::new().on_release(),
                move || {
                    callback_count_clone.fetch_add(1, Ordering::SeqCst);
                },
            )
            .unwrap();

        let hotkey_key = (
            KeyCode::KEY_A,
            normalize_modifiers(&[KeyCode::KEY_LEFTCTRL]),
        );

        let registration = manager
            .inner
            .registrations
            .lock()
            .unwrap()
            .get(&hotkey_key)
            .cloned()
            .unwrap();

        (registration.callbacks.on_press)();
        registration.callbacks.on_release.unwrap()();

        let sequence = HotkeySequence::new(vec![
            Hotkey::new(KeyCode::KEY_K, vec![KeyCode::KEY_LEFTCTRL]),
            Hotkey::new(KeyCode::KEY_C, vec![KeyCode::KEY_LEFTCTRL]),
        ])
        .unwrap();

        let sequence_handle = manager
            .register_sequence(&sequence, SequenceOptions::new(), || {})
            .unwrap();

        let sequence_callback = manager
            .inner
            .sequence_registrations
            .lock()
            .unwrap()
            .get(&sequence_handle.id)
            .unwrap()
            .callback
            .clone();

        sequence_callback();

        manager
            .define_mode("resize", ModeOptions::new(), |_mode| Ok(()))
            .unwrap();
        let mode_controller = manager.mode_controller();
        mode_controller.push("resize");
        mode_controller.pop();

        assert_eq!(callback_count.load(Ordering::SeqCst), 2);

        assert_eq!(
            stream.try_next(),
            Some(HotkeyEvent::Pressed(Hotkey::new(
                KeyCode::KEY_A,
                vec![KeyCode::KEY_LEFTCTRL],
            ))),
        );
        assert_eq!(
            stream.try_next(),
            Some(HotkeyEvent::Released(Hotkey::new(
                KeyCode::KEY_A,
                vec![KeyCode::KEY_LEFTCTRL],
            ))),
        );
        assert_eq!(
            stream.try_next(),
            Some(HotkeyEvent::SequenceStep {
                id: sequence_handle.id,
                step: 2,
                total: 2,
            }),
        );
        assert_eq!(
            stream.try_next(),
            Some(HotkeyEvent::ModeChanged(Some("resize".to_string()))),
        );
        assert_eq!(stream.try_next(), Some(HotkeyEvent::ModeChanged(None)));
        assert_eq!(stream.try_next(), None);
    }

    #[test]
    #[cfg(any(feature = "tokio", feature = "async-std"))]
    fn event_stream_emits_release_without_release_callback() {
        let manager = manager_with_fake_backend();
        let mut stream = manager.event_stream();
        let callback_count = Arc::new(AtomicUsize::new(0));

        let callback_count_clone = callback_count.clone();
        manager
            .register(KeyCode::KEY_B, &[KeyCode::KEY_LEFTCTRL], move || {
                callback_count_clone.fetch_add(1, Ordering::SeqCst);
            })
            .unwrap();

        let hotkey_key = (
            KeyCode::KEY_B,
            normalize_modifiers(&[KeyCode::KEY_LEFTCTRL]),
        );

        let registration = manager
            .inner
            .registrations
            .lock()
            .unwrap()
            .get(&hotkey_key)
            .cloned()
            .unwrap();

        (registration.callbacks.on_press)();
        registration.callbacks.on_release.unwrap()();

        assert_eq!(callback_count.load(Ordering::SeqCst), 1);
        assert_eq!(
            stream.try_next(),
            Some(HotkeyEvent::Pressed(Hotkey::new(
                KeyCode::KEY_B,
                vec![KeyCode::KEY_LEFTCTRL],
            ))),
        );
        assert_eq!(
            stream.try_next(),
            Some(HotkeyEvent::Released(Hotkey::new(
                KeyCode::KEY_B,
                vec![KeyCode::KEY_LEFTCTRL],
            ))),
        );
        assert_eq!(stream.try_next(), None);
    }

    #[test]
    #[cfg(any(feature = "tokio", feature = "async-std"))]
    fn event_stream_completes_when_manager_stops() {
        let manager = manager_with_fake_backend();
        let mut stream = manager.event_stream();

        manager.unregister_all().unwrap();

        while stream.try_next().is_some() {}

        assert_eq!(block_on_future(stream.next()), None);
    }

    #[test]
    #[cfg(any(feature = "tokio", feature = "async-std"))]
    fn event_stream_created_after_shutdown_is_closed() {
        let manager = manager_with_fake_backend();
        manager.unregister_all().unwrap();

        let mut stream = manager.event_stream();
        assert_eq!(block_on_future(stream.next()), None);
    }

    #[test]
    #[cfg(any(feature = "tokio", feature = "async-std"))]
    fn mode_changed_events_only_emit_when_active_mode_changes() {
        let manager = manager_with_fake_backend();
        manager
            .define_mode("resize", ModeOptions::new(), |_mode| Ok(()))
            .unwrap();

        let mut stream = manager.event_stream();
        let mode_controller = manager.mode_controller();

        mode_controller.push("resize");
        mode_controller.push("resize");
        mode_controller.pop();
        mode_controller.pop();

        assert_eq!(
            stream.try_next(),
            Some(HotkeyEvent::ModeChanged(Some("resize".to_string()))),
        );
        assert_eq!(stream.try_next(), Some(HotkeyEvent::ModeChanged(None)));
        assert_eq!(stream.try_next(), None);
    }
}
