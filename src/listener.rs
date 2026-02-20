use crate::device::{is_keyboard_device, DeviceInfo};
use crate::error::Error;
use crate::manager::{
    is_modifier_key, normalize_modifiers, ActiveHotkeyPress, Callback, DeviceHotkeyRegistration,
    DeviceRegistrationId, HotkeyKey, HotkeyRegistration, PressDispatchState, PressOrigin,
    RepeatBehavior, SequenceId, SequenceRegistration,
};
use crate::mode::{
    dispatch_mode_key_event, find_callbacks_for_active_press, pop_timed_out_modes, ModeDefinition,
    ModeEventDispatch, ModeRegistry,
};

#[cfg(feature = "grab")]
use evdev::{uinput::VirtualDevice, AttributeSet, EventType, InputEvent};
use evdev::{Device, EventSummary, KeyCode};
use std::collections::{HashMap, HashSet};
use std::ffi::CStr;
use std::io;
use std::mem::size_of;
use std::os::unix::io::AsRawFd;
use std::path::{Path, PathBuf};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use std::thread::{self, JoinHandle};
use std::time::Instant;

pub(crate) struct ListenerState {
    pub(crate) registrations: Arc<Mutex<HashMap<HotkeyKey, HotkeyRegistration>>>,
    pub(crate) sequence_registrations: Arc<Mutex<HashMap<SequenceId, SequenceRegistration>>>,
    pub(crate) device_registrations:
        Arc<Mutex<HashMap<DeviceRegistrationId, DeviceHotkeyRegistration>>>,
    pub(crate) stop_flag: Arc<AtomicBool>,
    pub(crate) mode_registry: ModeRegistry,
}

const POLL_TIMEOUT_MS: i32 = 25;
const INOTIFY_BUFFER_SIZE: usize = 4096;
#[cfg(feature = "grab")]
const MAX_FORWARDABLE_KEY_CODE: u16 = 767;

#[derive(Clone, Copy, Default)]
pub(crate) struct ListenerConfig {
    pub(crate) grab: bool,
}

trait KeyEventForwarder: Send {
    fn forward_key_event(&mut self, key: KeyCode, value: i32) -> Result<(), Error>;
}

#[cfg(feature = "grab")]
struct UinputForwarder {
    device: VirtualDevice,
}

#[cfg(feature = "grab")]
impl UinputForwarder {
    fn new() -> Result<Self, Error> {
        let mut keys = AttributeSet::<KeyCode>::new();
        for code in 0..=MAX_FORWARDABLE_KEY_CODE {
            keys.insert(KeyCode::new(code));
        }

        let device = VirtualDevice::builder()
            .map_err(|err| Error::DeviceAccess(format!("Failed to open /dev/uinput: {err}")))?
            .name("evdev-hotkey-virtual-keyboard")
            .with_keys(&keys)
            .map_err(|err| Error::DeviceAccess(format!("Failed to configure uinput keys: {err}")))?
            .build()
            .map_err(|err| Error::DeviceAccess(format!("Failed to create uinput device: {err}")))?;

        Ok(Self { device })
    }
}

#[cfg(feature = "grab")]
impl KeyEventForwarder for UinputForwarder {
    fn forward_key_event(&mut self, key: KeyCode, value: i32) -> Result<(), Error> {
        let key_event = InputEvent::new(EventType::KEY.0, key.code(), value);
        self.device.emit(&[key_event]).map_err(|err| {
            Error::DeviceAccess(format!("Failed forwarding key event via uinput: {err}"))
        })
    }
}

fn create_key_event_forwarder(
    grab_enabled: bool,
) -> Result<Option<Box<dyn KeyEventForwarder>>, Error> {
    if !grab_enabled {
        return Ok(None);
    }

    #[cfg(feature = "grab")]
    {
        Ok(Some(Box::new(UinputForwarder::new()?)))
    }

    #[cfg(not(feature = "grab"))]
    {
        Err(Error::UnsupportedFeature(
            "event grabbing support is not compiled in (enable the `grab` feature)".to_string(),
        ))
    }
}

#[derive(Clone)]
struct ActiveSequence {
    id: SequenceId,
    next_step_index: usize,
    deadline: Instant,
}

#[derive(Clone)]
struct PendingStandalone {
    key: HotkeyKey,
    pressed_at: Instant,
    released_at: Option<Instant>,
    deadline: Instant,
    press_dispatched: bool,
}

#[derive(Default)]
struct SequenceRuntime {
    active_sequences: Vec<ActiveSequence>,
    pending_standalone: Option<PendingStandalone>,
}

struct SequenceDispatch {
    callbacks: Vec<Callback>,
    synthetic_keys: Vec<HotkeyKey>,
    suppress_current_key_press: bool,
}

impl SequenceDispatch {
    fn empty() -> Self {
        Self {
            callbacks: Vec::new(),
            synthetic_keys: Vec::new(),
            suppress_current_key_press: false,
        }
    }
}

impl SequenceRuntime {
    fn on_tick(
        &mut self,
        now: Instant,
        registrations: &HashMap<HotkeyKey, HotkeyRegistration>,
        sequence_registrations: &HashMap<SequenceId, SequenceRegistration>,
    ) -> SequenceDispatch {
        let mut callbacks = Vec::new();
        let mut synthetic_keys = Vec::new();

        if let Some(pending) = self.pending_standalone.as_mut() {
            if now >= pending.deadline {
                let mut should_clear_pending = true;

                if let Some(registration) = registrations.get(&pending.key) {
                    let hold_satisfied = registration.callbacks.min_hold.is_none_or(|min_hold| {
                        let held_for = pending.released_at.map_or_else(
                            || now.duration_since(pending.pressed_at),
                            |released_at| released_at.duration_since(pending.pressed_at),
                        );
                        held_for >= min_hold
                    });

                    if !pending.press_dispatched {
                        if hold_satisfied {
                            callbacks.push(registration.callbacks.on_press.clone());
                            pending.press_dispatched = true;
                        } else if pending.released_at.is_none() {
                            if let Some(min_hold) = registration.callbacks.min_hold {
                                pending.deadline = pending.pressed_at + min_hold;
                                should_clear_pending = false;
                            }
                        }
                    }

                    if pending.press_dispatched {
                        if pending.released_at.is_some() {
                            if let Some(on_release) = &registration.callbacks.on_release {
                                callbacks.push(on_release.clone());
                            }
                            should_clear_pending = true;
                        } else {
                            should_clear_pending = registration.callbacks.on_release.is_none();
                        }
                    }
                }

                if should_clear_pending {
                    self.pending_standalone = None;
                }
            }
        }

        let mut retained = Vec::with_capacity(self.active_sequences.len());
        for active in self.active_sequences.drain(..) {
            if now < active.deadline {
                retained.push(active);
                continue;
            }

            if let Some(registration) = sequence_registrations.get(&active.id) {
                if let Some(timeout_fallback) = &registration.timeout_fallback {
                    synthetic_keys.push(timeout_fallback.clone());
                }
            }
        }

        self.active_sequences = retained;

        SequenceDispatch {
            callbacks,
            synthetic_keys,
            suppress_current_key_press: false,
        }
    }

    fn on_key_press(
        &mut self,
        key: HotkeyKey,
        now: Instant,
        registrations: &HashMap<HotkeyKey, HotkeyRegistration>,
        sequence_registrations: &HashMap<SequenceId, SequenceRegistration>,
    ) -> SequenceDispatch {
        if self
            .pending_standalone
            .as_ref()
            .is_some_and(|pending| !pending.press_dispatched)
        {
            self.pending_standalone = None;
        }

        self.active_sequences.retain(|active| {
            sequence_registrations
                .get(&active.id)
                .is_some_and(|registration| registration.abort_key != key.0)
        });

        let mut callbacks = Vec::new();
        let mut retained = Vec::with_capacity(self.active_sequences.len());
        let mut matched_existing_sequence = false;

        for mut active in self.active_sequences.drain(..) {
            let Some(registration) = sequence_registrations.get(&active.id) else {
                continue;
            };

            if registration
                .steps
                .get(active.next_step_index)
                .is_some_and(|expected| *expected == key)
            {
                matched_existing_sequence = true;

                if active.next_step_index + 1 == registration.steps.len() {
                    callbacks.push(registration.callback.clone());
                } else {
                    active.next_step_index += 1;
                    active.deadline = now + registration.timeout;
                    retained.push(active);
                }
            }
        }

        self.active_sequences = retained;

        let mut started_sequences: Vec<ActiveSequence> = Vec::new();
        let mut earliest_deadline = None;
        for (id, registration) in sequence_registrations {
            if registration
                .steps
                .first()
                .is_some_and(|first_step| *first_step == key)
            {
                earliest_deadline = Some(
                    earliest_deadline.map_or(now + registration.timeout, |current: Instant| {
                        current.min(now + registration.timeout)
                    }),
                );

                started_sequences.push(ActiveSequence {
                    id: *id,
                    next_step_index: 1,
                    deadline: now + registration.timeout,
                });
            }
        }

        let mut suppress_current_key_press = matched_existing_sequence;

        if !started_sequences.is_empty() {
            self.active_sequences.extend(started_sequences);

            if registrations.contains_key(&key) {
                suppress_current_key_press = true;
                if let Some(deadline) = earliest_deadline {
                    let can_replace_pending = self
                        .pending_standalone
                        .as_ref()
                        .is_none_or(|pending| !pending.press_dispatched);

                    if can_replace_pending {
                        self.pending_standalone = Some(PendingStandalone {
                            key,
                            pressed_at: now,
                            released_at: None,
                            deadline,
                            press_dispatched: false,
                        });
                    }
                }
            }
        }

        SequenceDispatch {
            callbacks,
            synthetic_keys: Vec::new(),
            suppress_current_key_press,
        }
    }

    fn on_key_release(&mut self, key: KeyCode, now: Instant) {
        if let Some(pending) = self.pending_standalone.as_mut() {
            if pending.key.0 == key && pending.released_at.is_none() {
                pending.released_at = Some(now);
            }
        }
    }
}

struct DeviceState {
    path: PathBuf,
    info: DeviceInfo,
    device: Device,
    active_presses: HashMap<KeyCode, ActiveHotkeyPress>,
}

impl DeviceState {
    fn fd(&self) -> i32 {
        self.device.as_raw_fd()
    }
}

#[derive(Default)]
struct ModifierTracker {
    pressed_modifiers: HashMap<PathBuf, HashSet<KeyCode>>,
}

impl ModifierTracker {
    fn press(&mut self, device_path: &Path, key: KeyCode) {
        self.pressed_modifiers
            .entry(device_path.to_path_buf())
            .or_default()
            .insert(key);
    }

    fn release(&mut self, device_path: &Path, key: KeyCode) {
        if let Some(keys) = self.pressed_modifiers.get_mut(device_path) {
            keys.remove(&key);
            if keys.is_empty() {
                self.pressed_modifiers.remove(device_path);
            }
        }
    }

    fn disconnect(&mut self, device_path: &Path) {
        self.pressed_modifiers.remove(device_path);
    }

    fn active_modifiers(&self) -> HashSet<KeyCode> {
        self.pressed_modifiers
            .values()
            .flat_map(|keys| keys.iter().copied())
            .collect()
    }

    fn device_modifiers(&self, device_path: &Path) -> HashSet<KeyCode> {
        self.pressed_modifiers
            .get(device_path)
            .cloned()
            .unwrap_or_default()
    }
}

struct RawFdGuard(i32);

impl RawFdGuard {
    fn new(fd: i32) -> Self {
        Self(fd)
    }

    fn raw_fd(&self) -> i32 {
        self.0
    }
}

impl Drop for RawFdGuard {
    fn drop(&mut self) {
        if self.0 >= 0 {
            unsafe {
                libc::close(self.0);
            }
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
struct HotplugFsEvent {
    mask: u32,
    device_name: String,
}

#[derive(Debug, PartialEq, Eq)]
enum HotplugPathChange {
    Added(PathBuf),
    Removed(PathBuf),
    Unchanged,
}

pub(crate) fn spawn_listener_thread(
    keyboard_paths: Vec<PathBuf>,
    shared: ListenerState,
    config: ListenerConfig,
) -> Result<JoinHandle<()>, Error> {
    let devices = open_devices(keyboard_paths, config)?;
    let inotify_fd = init_inotify_watcher()?;
    let key_event_forwarder = create_key_event_forwarder(config.grab)?;

    thread::Builder::new()
        .name("evdev-hotkey-listener".into())
        .spawn(move || {
            listener_loop(devices, inotify_fd, shared, config, key_event_forwarder);
        })
        .map_err(|e| Error::ThreadSpawn(format!("Failed to spawn listener thread: {}", e)))
}

fn open_devices(
    keyboard_paths: Vec<PathBuf>,
    config: ListenerConfig,
) -> Result<Vec<DeviceState>, Error> {
    let mut devices: Vec<DeviceState> = Vec::new();
    let mut last_error: Option<String> = None;

    for path in keyboard_paths {
        match open_device(&path, config.grab) {
            Ok(device) => devices.push(device),
            Err(err) => {
                last_error = Some(err);
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

fn open_device(path: &Path, grab: bool) -> Result<DeviceState, String> {
    #[allow(unused_mut)]
    let mut device = Device::open(path).map_err(|e| format!("Failed to open {:?}: {}", path, e))?;

    if !is_keyboard_device(&device) {
        return Err(format!("Device {:?} is not a keyboard", path));
    }

    let info = DeviceInfo::from_device(&device);

    if grab {
        #[cfg(feature = "grab")]
        {
            device
                .grab()
                .map_err(|e| format!("Failed to grab {:?} for exclusive capture: {}", path, e))?;
        }

        #[cfg(not(feature = "grab"))]
        {
            return Err("event grabbing support is not compiled in".to_string());
        }
    }

    set_nonblocking(device.as_raw_fd(), path)?;

    Ok(DeviceState {
        path: path.to_path_buf(),
        info,
        device,
        active_presses: HashMap::new(),
    })
}

fn set_nonblocking(fd: i32, path: &Path) -> Result<(), String> {
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
    if flags == -1 {
        return Err(format!("Failed to get file status flags for {:?}", path));
    }

    if unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } == -1 {
        return Err(format!("Failed to set non-blocking mode for {:?}", path));
    }

    Ok(())
}

fn init_inotify_watcher() -> Result<RawFdGuard, Error> {
    let fd = unsafe { libc::inotify_init1(libc::IN_NONBLOCK | libc::IN_CLOEXEC) };
    if fd < 0 {
        return Err(Error::DeviceAccess(format!(
            "Failed to initialize inotify watcher: {}",
            io::Error::last_os_error()
        )));
    }

    let fd_guard = RawFdGuard::new(fd);
    let input_path = std::ffi::CString::new("/dev/input").unwrap();
    let watch_result = unsafe {
        libc::inotify_add_watch(
            fd_guard.raw_fd(),
            input_path.as_ptr(),
            libc::IN_CREATE
                | libc::IN_DELETE
                | libc::IN_MOVED_FROM
                | libc::IN_MOVED_TO
                | libc::IN_DELETE_SELF
                | libc::IN_MOVE_SELF,
        )
    };

    if watch_result < 0 {
        return Err(Error::DeviceAccess(format!(
            "Failed to watch /dev/input for hotplug events: {}",
            io::Error::last_os_error()
        )));
    }

    Ok(fd_guard)
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

fn collect_callbacks_for_synthetic_keys(
    synthetic_keys: &[HotkeyKey],
    registrations: &HashMap<HotkeyKey, HotkeyRegistration>,
) -> Vec<Callback> {
    synthetic_keys
        .iter()
        .filter_map(|key| registrations.get(key))
        .map(|registration| registration.callbacks.on_press.clone())
        .collect()
}

fn collect_due_hold_callbacks(
    now: Instant,
    registrations: &HashMap<HotkeyKey, HotkeyRegistration>,
    mode_definitions: &HashMap<String, ModeDefinition>,
    device_registrations: &HashMap<DeviceRegistrationId, DeviceHotkeyRegistration>,
    active_presses: &mut HashMap<KeyCode, ActiveHotkeyPress>,
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

struct DeviceSpecificDispatch {
    callbacks: Vec<Callback>,
    matched: bool,
    passthrough: bool,
}

fn collect_device_specific_dispatch(
    key: KeyCode,
    value: i32,
    now: Instant,
    device_info: &DeviceInfo,
    device_modifiers: &HashSet<KeyCode>,
    device_registrations: &HashMap<DeviceRegistrationId, DeviceHotkeyRegistration>,
    active_presses: &mut HashMap<KeyCode, ActiveHotkeyPress>,
) -> DeviceSpecificDispatch {
    let modifier_signature = active_modifier_signature(device_modifiers);
    let device_hotkey_key = (key, modifier_signature);

    // Find matching device registration
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
                    let hold_satisfied = registration.callbacks.min_hold.is_none_or(|min_hold| {
                        now.duration_since(active.pressed_at) >= min_hold
                    });

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

struct NonModifierDispatch {
    callbacks: Vec<Callback>,
    matched_hotkey: bool,
    passthrough: bool,
}

fn should_forward_key_event_in_grab_mode(
    grab_enabled: bool,
    matched_hotkey: bool,
    passthrough: bool,
) -> bool {
    grab_enabled && (!matched_hotkey || passthrough)
}

fn suppress_sequence_followup_key_event(
    suppressed_keys: &mut HashSet<KeyCode>,
    key: KeyCode,
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

fn collect_non_modifier_dispatch(
    key: KeyCode,
    value: i32,
    now: Instant,
    active_modifiers: &HashSet<KeyCode>,
    registrations: &HashMap<HotkeyKey, HotkeyRegistration>,
    active_presses: &mut HashMap<KeyCode, ActiveHotkeyPress>,
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

#[cfg(test)]
fn collect_non_modifier_callbacks(
    key: KeyCode,
    value: i32,
    now: Instant,
    active_modifiers: &HashSet<KeyCode>,
    registrations: &HashMap<HotkeyKey, HotkeyRegistration>,
    active_presses: &mut HashMap<KeyCode, ActiveHotkeyPress>,
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

fn listener_loop(
    mut devices: Vec<DeviceState>,
    inotify_fd: RawFdGuard,
    shared: ListenerState,
    config: ListenerConfig,
    mut key_event_forwarder: Option<Box<dyn KeyEventForwarder>>,
) {
    let ListenerState {
        registrations,
        sequence_registrations,
        device_registrations,
        stop_flag,
        mode_registry,
    } = shared;
    let mut modifier_tracker = ModifierTracker::default();
    let mut sequence_runtime = SequenceRuntime::default();
    let mut suppressed_sequence_keys = HashSet::new();

    loop {
        if stop_flag.load(Ordering::SeqCst) {
            return;
        }

        let (inotify_ready, ready_devices) = match poll_ready_sources(inotify_fd.raw_fd(), &devices)
        {
            Ok(ready) => ready,
            Err(err) => {
                tracing::warn!("Poll error, stopping listener: {}", err);
                stop_flag.store(true, Ordering::SeqCst);
                return;
            }
        };

        let timeout_callbacks = {
            let now = Instant::now();
            let registrations_guard = registrations.lock().unwrap();
            let sequence_guard = sequence_registrations.lock().unwrap();
            let mode_definitions_guard = mode_registry.definitions.lock().unwrap();
            let mut mode_stack_guard = mode_registry.stack.lock().unwrap();

            // Check mode timeouts
            pop_timed_out_modes(&mut mode_stack_guard, &mode_definitions_guard, now);

            let timeout_dispatch =
                sequence_runtime.on_tick(now, &registrations_guard, &sequence_guard);
            let mut callbacks = timeout_dispatch.callbacks;
            callbacks.extend(collect_callbacks_for_synthetic_keys(
                &timeout_dispatch.synthetic_keys,
                &registrations_guard,
            ));

            let device_regs_guard = device_registrations.lock().unwrap();
            for device in &mut devices {
                callbacks.extend(collect_due_hold_callbacks(
                    now,
                    &registrations_guard,
                    &mode_definitions_guard,
                    &device_regs_guard,
                    &mut device.active_presses,
                ));
            }

            callbacks
        };
        dispatch_callbacks(timeout_callbacks);

        if inotify_ready {
            process_hotplug_events(
                inotify_fd.raw_fd(),
                &mut devices,
                &mut modifier_tracker,
                config,
            );
        }

        for (fd, revents) in ready_devices {
            if revents & (libc::POLLERR | libc::POLLHUP | libc::POLLNVAL) != 0 {
                remove_device_by_fd(fd, &mut devices, &mut modifier_tracker);
                continue;
            }

            let Some(device_index) = devices.iter().position(|device| device.fd() == fd) else {
                continue;
            };

            let device_path = devices[device_index].path.clone();

            let key_events = {
                let device = &mut devices[device_index].device;
                match read_key_events(device) {
                    Ok(events) => events,
                    Err(err) if should_drop_device(&err) => {
                        remove_device_by_fd(fd, &mut devices, &mut modifier_tracker);
                        continue;
                    }
                    Err(_) => {
                        continue;
                    }
                }
            };

            for (key, value) in key_events {
                if is_modifier_key(key) {
                    match value {
                        1 => modifier_tracker.press(&device_path, key),
                        0 => modifier_tracker.release(&device_path, key),
                        _ => {}
                    }

                    if config.grab {
                        if let Some(forwarder) = key_event_forwarder.as_mut() {
                            if let Err(err) = forwarder.forward_key_event(key, value) {
                                tracing::warn!("Failed forwarding modifier key event: {}", err);
                            }
                        }
                    }
                    continue;
                }

                let (callbacks, should_forward_event) = {
                    let now = Instant::now();
                    let active_modifiers = modifier_tracker.active_modifiers();
                    let hotkey_key = (key, active_modifier_signature(&active_modifiers));

                    // Mode dispatch takes priority over sequences and global
                    let mode_dispatch = {
                        let mode_definitions_guard = mode_registry.definitions.lock().unwrap();
                        let mut mode_stack_guard = mode_registry.stack.lock().unwrap();
                        dispatch_mode_key_event(
                            &hotkey_key,
                            value,
                            now,
                            &mode_definitions_guard,
                            &mut mode_stack_guard,
                            &mut devices[device_index].active_presses,
                        )
                    };

                    match mode_dispatch {
                        ModeEventDispatch::Swallowed => (Vec::new(), false),
                        ModeEventDispatch::Handled {
                            callbacks: mode_callbacks,
                            passthrough,
                        } => {
                            let should_forward = should_forward_key_event_in_grab_mode(
                                config.grab,
                                true,
                                passthrough,
                            );
                            (mode_callbacks, should_forward)
                        }
                        ModeEventDispatch::PassThrough => {
                            // Device-specific dispatch takes priority
                            let device_modifiers =
                                modifier_tracker.device_modifiers(&device_path);
                            let device_info = devices[device_index].info.clone();
                            let device_regs_guard =
                                device_registrations.lock().unwrap();
                            let device_dispatch = collect_device_specific_dispatch(
                                key,
                                value,
                                now,
                                &device_info,
                                &device_modifiers,
                                &device_regs_guard,
                                &mut devices[device_index].active_presses,
                            );
                            drop(device_regs_guard);

                            if device_dispatch.matched {
                                let should_forward = should_forward_key_event_in_grab_mode(
                                    config.grab,
                                    true,
                                    device_dispatch.passthrough,
                                );
                                (device_dispatch.callbacks, should_forward)
                            } else {
                                // Fall through to sequence and global dispatch
                                let registrations_guard = registrations.lock().unwrap();
                                let sequence_guard =
                                    sequence_registrations.lock().unwrap();

                                let mut sequence_dispatch = SequenceDispatch::empty();
                                if value == 1 {
                                    sequence_dispatch = sequence_runtime.on_key_press(
                                        hotkey_key,
                                        now,
                                        &registrations_guard,
                                        &sequence_guard,
                                    );
                                } else if value == 0 {
                                    sequence_runtime.on_key_release(key, now);
                                }

                                let mut callbacks = sequence_dispatch.callbacks;
                                callbacks.extend(collect_callbacks_for_synthetic_keys(
                                    &sequence_dispatch.synthetic_keys,
                                    &registrations_guard,
                                ));

                                let suppress_followup =
                                    suppress_sequence_followup_key_event(
                                        &mut suppressed_sequence_keys,
                                        key,
                                        value,
                                        sequence_dispatch.suppress_current_key_press,
                                    );

                                let non_modifier_dispatch = if suppress_followup {
                                    NonModifierDispatch {
                                        callbacks: Vec::new(),
                                        matched_hotkey: true,
                                        passthrough: false,
                                    }
                                } else {
                                    collect_non_modifier_dispatch(
                                        key,
                                        value,
                                        now,
                                        &active_modifiers,
                                        &registrations_guard,
                                        &mut devices[device_index].active_presses,
                                        sequence_dispatch.suppress_current_key_press,
                                    )
                                };

                                let should_forward_event =
                                    should_forward_key_event_in_grab_mode(
                                        config.grab,
                                        sequence_dispatch.suppress_current_key_press
                                            || non_modifier_dispatch.matched_hotkey,
                                        non_modifier_dispatch.passthrough,
                                    );

                                callbacks.extend(non_modifier_dispatch.callbacks);

                                (callbacks, should_forward_event)
                            }
                        }
                    }
                };

                dispatch_callbacks(callbacks);

                if should_forward_event {
                    if let Some(forwarder) = key_event_forwarder.as_mut() {
                        if let Err(err) = forwarder.forward_key_event(key, value) {
                            tracing::warn!("Failed forwarding key event: {}", err);
                        }
                    }
                }
            }
        }
    }
}

fn poll_ready_sources(
    inotify_fd: i32,
    devices: &[DeviceState],
) -> io::Result<(bool, Vec<(i32, i16)>)> {
    let mut pollfds = Vec::with_capacity(devices.len() + 1);
    pollfds.push(libc::pollfd {
        fd: inotify_fd,
        events: libc::POLLIN,
        revents: 0,
    });

    for device in devices {
        pollfds.push(libc::pollfd {
            fd: device.fd(),
            events: libc::POLLIN,
            revents: 0,
        });
    }

    let poll_result = unsafe {
        libc::poll(
            pollfds.as_mut_ptr(),
            pollfds.len() as libc::nfds_t,
            POLL_TIMEOUT_MS,
        )
    };

    if poll_result < 0 {
        let err = io::Error::last_os_error();
        if err.kind() == io::ErrorKind::Interrupted {
            return Ok((false, Vec::new()));
        }
        return Err(err);
    }

    let inotify_revents = pollfds[0].revents;
    if inotify_revents & (libc::POLLERR | libc::POLLHUP | libc::POLLNVAL) != 0 {
        return Err(io::Error::other(
            "inotify source became invalid while polling",
        ));
    }

    let inotify_ready = inotify_revents & libc::POLLIN != 0;
    let ready_devices = pollfds
        .iter()
        .skip(1)
        .filter_map(|pollfd| {
            if pollfd.revents == 0 {
                None
            } else {
                Some((pollfd.fd, pollfd.revents))
            }
        })
        .collect();

    Ok((inotify_ready, ready_devices))
}

fn read_key_events(device: &mut Device) -> io::Result<Vec<(KeyCode, i32)>> {
    let mut events = Vec::new();

    for event in device.fetch_events()? {
        if let EventSummary::Key(_, key, value) = event.destructure() {
            events.push((key, value));
        }
    }

    Ok(events)
}

fn should_drop_device(err: &io::Error) -> bool {
    err.raw_os_error() == Some(libc::ENODEV)
        || err.kind() == io::ErrorKind::NotFound
        || err.kind() == io::ErrorKind::UnexpectedEof
}

fn remove_device_by_fd(
    fd: i32,
    devices: &mut Vec<DeviceState>,
    modifier_tracker: &mut ModifierTracker,
) {
    if let Some(index) = devices.iter().position(|device| device.fd() == fd) {
        let removed = devices.swap_remove(index);
        modifier_tracker.disconnect(&removed.path);
    }
}

fn remove_device_by_path(
    path: &Path,
    devices: &mut Vec<DeviceState>,
    modifier_tracker: &mut ModifierTracker,
) {
    if let Some(index) = devices.iter().position(|device| device.path == path) {
        let removed = devices.swap_remove(index);
        modifier_tracker.disconnect(&removed.path);
    } else {
        modifier_tracker.disconnect(path);
    }
}

fn classify_hotplug_change(
    event: &HotplugFsEvent,
    known_paths: &mut HashSet<PathBuf>,
) -> HotplugPathChange {
    if !event.device_name.starts_with("event") {
        return HotplugPathChange::Unchanged;
    }

    let path = PathBuf::from("/dev/input").join(&event.device_name);

    if event.mask & (libc::IN_CREATE | libc::IN_MOVED_TO) != 0 && known_paths.insert(path.clone()) {
        return HotplugPathChange::Added(path);
    }

    if event.mask
        & (libc::IN_DELETE | libc::IN_MOVED_FROM | libc::IN_DELETE_SELF | libc::IN_MOVE_SELF)
        != 0
    {
        known_paths.remove(&path);
        return HotplugPathChange::Removed(path);
    }

    HotplugPathChange::Unchanged
}

fn process_hotplug_events(
    inotify_fd: i32,
    devices: &mut Vec<DeviceState>,
    modifier_tracker: &mut ModifierTracker,
    config: ListenerConfig,
) {
    let mut buffer = [0u8; INOTIFY_BUFFER_SIZE];

    loop {
        let bytes_read = unsafe {
            libc::read(
                inotify_fd,
                buffer.as_mut_ptr() as *mut libc::c_void,
                buffer.len(),
            )
        };

        if bytes_read < 0 {
            let err = io::Error::last_os_error();
            if err.kind() == io::ErrorKind::WouldBlock {
                break;
            }

            tracing::warn!("Failed reading inotify events: {}", err);
            break;
        }

        if bytes_read == 0 {
            break;
        }

        let mut known_paths: HashSet<PathBuf> =
            devices.iter().map(|device| device.path.clone()).collect();

        for event in parse_hotplug_events(&buffer, bytes_read as usize) {
            match classify_hotplug_change(&event, &mut known_paths) {
                HotplugPathChange::Added(path) => match open_device(&path, config.grab) {
                    Ok(device_state) => {
                        devices.push(device_state);
                    }
                    Err(err) => {
                        tracing::debug!("Ignoring hotplugged device {:?}: {}", path, err);
                    }
                },
                HotplugPathChange::Removed(path) => {
                    remove_device_by_path(&path, devices, modifier_tracker);
                }
                HotplugPathChange::Unchanged => {}
            }
        }
    }
}

fn parse_hotplug_events(buffer: &[u8], bytes_read: usize) -> Vec<HotplugFsEvent> {
    let mut events = Vec::new();
    let mut offset = 0usize;

    while offset + size_of::<libc::inotify_event>() <= bytes_read {
        let event_ptr = unsafe { buffer.as_ptr().add(offset) as *const libc::inotify_event };
        let event = unsafe { std::ptr::read_unaligned(event_ptr) };

        let name_start = offset + size_of::<libc::inotify_event>();
        let Some(name_end) = name_start.checked_add(event.len as usize) else {
            break;
        };

        let device_name = if event.len > 0 && name_end <= bytes_read {
            let name_slice = &buffer[name_start..name_end];
            let cstr_end = name_slice
                .iter()
                .position(|byte| *byte == 0)
                .unwrap_or(name_slice.len());

            if cstr_end == 0 {
                String::new()
            } else {
                CStr::from_bytes_with_nul(&name_slice[..=cstr_end.min(name_slice.len() - 1)])
                    .ok()
                    .and_then(|value| value.to_str().ok())
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| String::from_utf8_lossy(&name_slice[..cstr_end]).to_string())
            }
        } else {
            String::new()
        };

        events.push(HotplugFsEvent {
            mask: event.mask,
            device_name,
        });

        let Some(next_offset) =
            offset.checked_add(size_of::<libc::inotify_event>() + event.len as usize)
        else {
            break;
        };
        offset = next_offset;
    }

    events
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::device::DeviceFilter;
    use crate::manager::{HotkeyCallbacks, RepeatBehavior};
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::time::Duration;

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

    fn sequence_registration(
        steps: Vec<HotkeyKey>,
        timeout: Duration,
        abort_key: KeyCode,
        timeout_fallback: Option<HotkeyKey>,
        counter: Arc<AtomicUsize>,
    ) -> SequenceRegistration {
        SequenceRegistration {
            steps,
            callback: Arc::new(move || {
                counter.fetch_add(1, Ordering::SeqCst);
            }),
            timeout,
            abort_key,
            timeout_fallback,
        }
    }

    fn no_release_callbacks(counter: Arc<AtomicUsize>) -> HotkeyCallbacks {
        HotkeyCallbacks {
            on_press: Arc::new(move || {
                counter.fetch_add(1, Ordering::SeqCst);
            }),
            on_release: None,
            min_hold: None,
            repeat_behavior: RepeatBehavior::Ignore,
            passthrough: false,
        }
    }

    #[test]
    fn sequence_completes_within_timeout() {
        let mut runtime = SequenceRuntime::default();
        let t0 = Instant::now();

        let sequence_key_1 = (KeyCode::KEY_K, vec![KeyCode::KEY_LEFTCTRL]);
        let sequence_key_2 = (KeyCode::KEY_C, vec![KeyCode::KEY_LEFTCTRL]);

        let sequence_count = Arc::new(AtomicUsize::new(0));

        let mut sequence_registrations = HashMap::new();
        sequence_registrations.insert(
            1,
            sequence_registration(
                vec![sequence_key_1.clone(), sequence_key_2.clone()],
                Duration::from_millis(50),
                KeyCode::KEY_ESC,
                None,
                sequence_count.clone(),
            ),
        );

        let registrations = HashMap::new();

        runtime.on_key_press(sequence_key_1, t0, &registrations, &sequence_registrations);

        let dispatch = runtime.on_key_press(
            sequence_key_2,
            t0 + Duration::from_millis(20),
            &registrations,
            &sequence_registrations,
        );

        dispatch_callbacks(dispatch.callbacks);
        assert_eq!(sequence_count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn sequence_timeout_clears_pending_state() {
        let mut runtime = SequenceRuntime::default();
        let t0 = Instant::now();

        let sequence_key_1 = (KeyCode::KEY_K, vec![KeyCode::KEY_LEFTCTRL]);
        let sequence_key_2 = (KeyCode::KEY_C, vec![KeyCode::KEY_LEFTCTRL]);

        let sequence_count = Arc::new(AtomicUsize::new(0));
        let mut sequence_registrations = HashMap::new();
        sequence_registrations.insert(
            1,
            sequence_registration(
                vec![sequence_key_1.clone(), sequence_key_2.clone()],
                Duration::from_millis(50),
                KeyCode::KEY_ESC,
                None,
                sequence_count.clone(),
            ),
        );

        let registrations = HashMap::new();

        runtime.on_key_press(sequence_key_1, t0, &registrations, &sequence_registrations);
        runtime.on_tick(
            t0 + Duration::from_millis(60),
            &registrations,
            &sequence_registrations,
        );

        let dispatch = runtime.on_key_press(
            sequence_key_2,
            t0 + Duration::from_millis(65),
            &registrations,
            &sequence_registrations,
        );

        dispatch_callbacks(dispatch.callbacks);
        assert_eq!(sequence_count.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn wrong_key_resets_sequence() {
        let mut runtime = SequenceRuntime::default();
        let t0 = Instant::now();

        let sequence_key_1 = (KeyCode::KEY_K, vec![KeyCode::KEY_LEFTCTRL]);
        let sequence_key_2 = (KeyCode::KEY_C, vec![KeyCode::KEY_LEFTCTRL]);
        let wrong_key = (KeyCode::KEY_X, vec![KeyCode::KEY_LEFTCTRL]);

        let sequence_count = Arc::new(AtomicUsize::new(0));
        let mut sequence_registrations = HashMap::new();
        sequence_registrations.insert(
            1,
            sequence_registration(
                vec![sequence_key_1.clone(), sequence_key_2.clone()],
                Duration::from_millis(100),
                KeyCode::KEY_ESC,
                None,
                sequence_count.clone(),
            ),
        );

        let registrations = HashMap::new();

        runtime.on_key_press(sequence_key_1, t0, &registrations, &sequence_registrations);
        runtime.on_key_press(
            wrong_key,
            t0 + Duration::from_millis(5),
            &registrations,
            &sequence_registrations,
        );

        let dispatch = runtime.on_key_press(
            sequence_key_2,
            t0 + Duration::from_millis(10),
            &registrations,
            &sequence_registrations,
        );

        dispatch_callbacks(dispatch.callbacks);
        assert_eq!(sequence_count.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn standalone_first_step_fires_on_timeout() {
        let mut runtime = SequenceRuntime::default();
        let t0 = Instant::now();

        let sequence_key_1 = (KeyCode::KEY_K, vec![KeyCode::KEY_LEFTCTRL]);
        let sequence_key_2 = (KeyCode::KEY_C, vec![KeyCode::KEY_LEFTCTRL]);

        let standalone_count = Arc::new(AtomicUsize::new(0));
        let sequence_count = Arc::new(AtomicUsize::new(0));

        let mut registrations = HashMap::new();
        registrations.insert(
            sequence_key_1.clone(),
            HotkeyRegistration {
                callbacks: no_release_callbacks(standalone_count.clone()),
            },
        );

        let mut sequence_registrations = HashMap::new();
        sequence_registrations.insert(
            1,
            sequence_registration(
                vec![sequence_key_1.clone(), sequence_key_2],
                Duration::from_millis(50),
                KeyCode::KEY_ESC,
                None,
                sequence_count,
            ),
        );

        let press_dispatch =
            runtime.on_key_press(sequence_key_1, t0, &registrations, &sequence_registrations);
        assert!(press_dispatch.suppress_current_key_press);

        let timeout_dispatch = runtime.on_tick(
            t0 + Duration::from_millis(55),
            &registrations,
            &sequence_registrations,
        );

        dispatch_callbacks(timeout_dispatch.callbacks);

        assert_eq!(standalone_count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn standalone_first_step_timeout_dispatches_release_when_released_before_timeout() {
        let mut runtime = SequenceRuntime::default();
        let t0 = Instant::now();

        let sequence_key_1 = (KeyCode::KEY_K, vec![KeyCode::KEY_LEFTCTRL]);
        let sequence_key_2 = (KeyCode::KEY_C, vec![KeyCode::KEY_LEFTCTRL]);

        let press_count = Arc::new(AtomicUsize::new(0));
        let release_count = Arc::new(AtomicUsize::new(0));
        let press_count_clone = press_count.clone();
        let release_count_clone = release_count.clone();

        let mut registrations = HashMap::new();
        registrations.insert(
            sequence_key_1.clone(),
            HotkeyRegistration {
                callbacks: HotkeyCallbacks {
                    on_press: Arc::new(move || {
                        press_count_clone.fetch_add(1, Ordering::SeqCst);
                    }),
                    on_release: Some(Arc::new(move || {
                        release_count_clone.fetch_add(1, Ordering::SeqCst);
                    })),
                    min_hold: None,
                    repeat_behavior: RepeatBehavior::Ignore,
                    passthrough: false,
                },
            },
        );

        let mut sequence_registrations = HashMap::new();
        sequence_registrations.insert(
            1,
            sequence_registration(
                vec![sequence_key_1.clone(), sequence_key_2],
                Duration::from_millis(50),
                KeyCode::KEY_ESC,
                None,
                Arc::new(AtomicUsize::new(0)),
            ),
        );

        runtime.on_key_press(
            sequence_key_1.clone(),
            t0,
            &registrations,
            &sequence_registrations,
        );
        runtime.on_key_release(sequence_key_1.0, t0 + Duration::from_millis(10));

        let timeout_dispatch = runtime.on_tick(
            t0 + Duration::from_millis(55),
            &registrations,
            &sequence_registrations,
        );
        dispatch_callbacks(timeout_dispatch.callbacks);

        assert_eq!(press_count.load(Ordering::SeqCst), 1);
        assert_eq!(release_count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn standalone_first_step_timeout_dispatches_release_after_timeout_when_key_is_released() {
        let mut runtime = SequenceRuntime::default();
        let t0 = Instant::now();

        let sequence_key_1 = (KeyCode::KEY_K, vec![KeyCode::KEY_LEFTCTRL]);
        let sequence_key_2 = (KeyCode::KEY_C, vec![KeyCode::KEY_LEFTCTRL]);

        let press_count = Arc::new(AtomicUsize::new(0));
        let release_count = Arc::new(AtomicUsize::new(0));
        let press_count_clone = press_count.clone();
        let release_count_clone = release_count.clone();

        let mut registrations = HashMap::new();
        registrations.insert(
            sequence_key_1.clone(),
            HotkeyRegistration {
                callbacks: HotkeyCallbacks {
                    on_press: Arc::new(move || {
                        press_count_clone.fetch_add(1, Ordering::SeqCst);
                    }),
                    on_release: Some(Arc::new(move || {
                        release_count_clone.fetch_add(1, Ordering::SeqCst);
                    })),
                    min_hold: None,
                    repeat_behavior: RepeatBehavior::Ignore,
                    passthrough: false,
                },
            },
        );

        let mut sequence_registrations = HashMap::new();
        sequence_registrations.insert(
            1,
            sequence_registration(
                vec![sequence_key_1.clone(), sequence_key_2],
                Duration::from_millis(50),
                KeyCode::KEY_ESC,
                None,
                Arc::new(AtomicUsize::new(0)),
            ),
        );

        runtime.on_key_press(
            sequence_key_1.clone(),
            t0,
            &registrations,
            &sequence_registrations,
        );

        let timeout_dispatch = runtime.on_tick(
            t0 + Duration::from_millis(55),
            &registrations,
            &sequence_registrations,
        );
        dispatch_callbacks(timeout_dispatch.callbacks);
        assert_eq!(press_count.load(Ordering::SeqCst), 1);
        assert_eq!(release_count.load(Ordering::SeqCst), 0);

        runtime.on_key_release(sequence_key_1.0, t0 + Duration::from_millis(80));

        let release_dispatch = runtime.on_tick(
            t0 + Duration::from_millis(81),
            &registrations,
            &sequence_registrations,
        );
        dispatch_callbacks(release_dispatch.callbacks);

        assert_eq!(press_count.load(Ordering::SeqCst), 1);
        assert_eq!(release_count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn standalone_first_step_timeout_respects_min_hold_when_released_early() {
        let mut runtime = SequenceRuntime::default();
        let t0 = Instant::now();

        let sequence_key_1 = (KeyCode::KEY_K, vec![KeyCode::KEY_LEFTCTRL]);
        let sequence_key_2 = (KeyCode::KEY_C, vec![KeyCode::KEY_LEFTCTRL]);

        let standalone_count = Arc::new(AtomicUsize::new(0));
        let standalone_count_clone = standalone_count.clone();

        let mut registrations = HashMap::new();
        registrations.insert(
            sequence_key_1.clone(),
            HotkeyRegistration {
                callbacks: HotkeyCallbacks {
                    on_press: Arc::new(move || {
                        standalone_count_clone.fetch_add(1, Ordering::SeqCst);
                    }),
                    on_release: None,
                    min_hold: Some(Duration::from_millis(100)),
                    repeat_behavior: RepeatBehavior::Ignore,
                    passthrough: false,
                },
            },
        );

        let mut sequence_registrations = HashMap::new();
        sequence_registrations.insert(
            1,
            sequence_registration(
                vec![sequence_key_1.clone(), sequence_key_2],
                Duration::from_millis(50),
                KeyCode::KEY_ESC,
                None,
                Arc::new(AtomicUsize::new(0)),
            ),
        );

        runtime.on_key_press(
            sequence_key_1.clone(),
            t0,
            &registrations,
            &sequence_registrations,
        );
        runtime.on_key_release(sequence_key_1.0, t0 + Duration::from_millis(30));

        let timeout_dispatch = runtime.on_tick(
            t0 + Duration::from_millis(60),
            &registrations,
            &sequence_registrations,
        );
        dispatch_callbacks(timeout_dispatch.callbacks);

        let later_dispatch = runtime.on_tick(
            t0 + Duration::from_millis(120),
            &registrations,
            &sequence_registrations,
        );
        dispatch_callbacks(later_dispatch.callbacks);

        assert_eq!(standalone_count.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn standalone_first_step_timeout_waits_for_min_hold_when_still_pressed() {
        let mut runtime = SequenceRuntime::default();
        let t0 = Instant::now();

        let sequence_key_1 = (KeyCode::KEY_K, vec![KeyCode::KEY_LEFTCTRL]);
        let sequence_key_2 = (KeyCode::KEY_C, vec![KeyCode::KEY_LEFTCTRL]);

        let standalone_count = Arc::new(AtomicUsize::new(0));
        let standalone_count_clone = standalone_count.clone();

        let mut registrations = HashMap::new();
        registrations.insert(
            sequence_key_1.clone(),
            HotkeyRegistration {
                callbacks: HotkeyCallbacks {
                    on_press: Arc::new(move || {
                        standalone_count_clone.fetch_add(1, Ordering::SeqCst);
                    }),
                    on_release: None,
                    min_hold: Some(Duration::from_millis(100)),
                    repeat_behavior: RepeatBehavior::Ignore,
                    passthrough: false,
                },
            },
        );

        let mut sequence_registrations = HashMap::new();
        sequence_registrations.insert(
            1,
            sequence_registration(
                vec![sequence_key_1.clone(), sequence_key_2],
                Duration::from_millis(50),
                KeyCode::KEY_ESC,
                None,
                Arc::new(AtomicUsize::new(0)),
            ),
        );

        runtime.on_key_press(sequence_key_1, t0, &registrations, &sequence_registrations);

        let first_timeout_dispatch = runtime.on_tick(
            t0 + Duration::from_millis(60),
            &registrations,
            &sequence_registrations,
        );
        dispatch_callbacks(first_timeout_dispatch.callbacks);

        let hold_dispatch = runtime.on_tick(
            t0 + Duration::from_millis(120),
            &registrations,
            &sequence_registrations,
        );
        dispatch_callbacks(hold_dispatch.callbacks);

        assert_eq!(standalone_count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn abort_key_cancels_in_progress_sequences() {
        let mut runtime = SequenceRuntime::default();
        let t0 = Instant::now();

        let sequence_key_1 = (KeyCode::KEY_K, vec![KeyCode::KEY_LEFTCTRL]);
        let sequence_key_2 = (KeyCode::KEY_C, vec![KeyCode::KEY_LEFTCTRL]);

        let sequence_count = Arc::new(AtomicUsize::new(0));
        let mut sequence_registrations = HashMap::new();
        sequence_registrations.insert(
            1,
            sequence_registration(
                vec![sequence_key_1.clone(), sequence_key_2.clone()],
                Duration::from_millis(100),
                KeyCode::KEY_Q,
                None,
                sequence_count.clone(),
            ),
        );

        let registrations = HashMap::new();

        runtime.on_key_press(sequence_key_1, t0, &registrations, &sequence_registrations);
        runtime.on_key_press(
            (KeyCode::KEY_Q, vec![]),
            t0 + Duration::from_millis(10),
            &registrations,
            &sequence_registrations,
        );

        let dispatch = runtime.on_key_press(
            sequence_key_2,
            t0 + Duration::from_millis(20),
            &registrations,
            &sequence_registrations,
        );

        dispatch_callbacks(dispatch.callbacks);
        assert_eq!(sequence_count.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn multiple_sequences_share_prefix_without_interference() {
        let mut runtime = SequenceRuntime::default();
        let t0 = Instant::now();

        let first_step = (KeyCode::KEY_K, vec![KeyCode::KEY_LEFTCTRL]);
        let complete_a = (KeyCode::KEY_C, vec![KeyCode::KEY_LEFTCTRL]);
        let complete_b = (KeyCode::KEY_U, vec![KeyCode::KEY_LEFTCTRL]);

        let sequence_a_count = Arc::new(AtomicUsize::new(0));
        let sequence_b_count = Arc::new(AtomicUsize::new(0));

        let mut sequence_registrations = HashMap::new();
        sequence_registrations.insert(
            1,
            sequence_registration(
                vec![first_step.clone(), complete_a.clone()],
                Duration::from_millis(100),
                KeyCode::KEY_ESC,
                None,
                sequence_a_count.clone(),
            ),
        );
        sequence_registrations.insert(
            2,
            sequence_registration(
                vec![first_step.clone(), complete_b],
                Duration::from_millis(100),
                KeyCode::KEY_ESC,
                None,
                sequence_b_count.clone(),
            ),
        );

        let registrations = HashMap::new();

        runtime.on_key_press(first_step, t0, &registrations, &sequence_registrations);
        let dispatch = runtime.on_key_press(
            complete_a,
            t0 + Duration::from_millis(10),
            &registrations,
            &sequence_registrations,
        );

        dispatch_callbacks(dispatch.callbacks);

        assert_eq!(sequence_a_count.load(Ordering::SeqCst), 1);
        assert_eq!(sequence_b_count.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn timeout_fallback_dispatches_synthetic_hotkey() {
        let mut runtime = SequenceRuntime::default();
        let t0 = Instant::now();

        let sequence_key_1 = (KeyCode::KEY_K, vec![KeyCode::KEY_LEFTCTRL]);
        let sequence_key_2 = (KeyCode::KEY_C, vec![KeyCode::KEY_LEFTCTRL]);
        let fallback_key = (KeyCode::KEY_F, vec![]);

        let fallback_count = Arc::new(AtomicUsize::new(0));

        let mut registrations = HashMap::new();
        registrations.insert(
            fallback_key.clone(),
            HotkeyRegistration {
                callbacks: no_release_callbacks(fallback_count.clone()),
            },
        );

        let mut sequence_registrations = HashMap::new();
        sequence_registrations.insert(
            1,
            sequence_registration(
                vec![sequence_key_1.clone(), sequence_key_2],
                Duration::from_millis(50),
                KeyCode::KEY_ESC,
                Some(fallback_key),
                Arc::new(AtomicUsize::new(0)),
            ),
        );

        runtime.on_key_press(sequence_key_1, t0, &registrations, &sequence_registrations);

        let timeout_dispatch = runtime.on_tick(
            t0 + Duration::from_millis(60),
            &registrations,
            &sequence_registrations,
        );
        let mut callbacks = timeout_dispatch.callbacks;
        callbacks.extend(collect_callbacks_for_synthetic_keys(
            &timeout_dispatch.synthetic_keys,
            &registrations,
        ));

        dispatch_callbacks(callbacks);
        assert_eq!(fallback_count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn release_callback_runs_after_press() {
        let press_count = Arc::new(AtomicUsize::new(0));
        let release_count = Arc::new(AtomicUsize::new(0));
        let p = press_count.clone();
        let r = release_count.clone();

        let callbacks = HotkeyCallbacks {
            on_press: Arc::new(move || {
                p.fetch_add(1, Ordering::SeqCst);
            }),
            on_release: Some(Arc::new(move || {
                r.fetch_add(1, Ordering::SeqCst);
            })),
            min_hold: None,
            repeat_behavior: RepeatBehavior::Ignore,
            passthrough: false,
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
            false,
        ));
        dispatch_callbacks(collect_non_modifier_callbacks(
            KeyCode::KEY_A,
            0,
            t0 + Duration::from_millis(10),
            &modifiers,
            &registrations,
            &mut active_presses,
            false,
        ));

        assert_eq!(press_count.load(Ordering::SeqCst), 1);
        assert_eq!(release_count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn suppressed_sequence_followup_events_are_consumed_until_release() {
        let key = KeyCode::KEY_A;
        let mut suppressed = HashSet::new();

        assert!(!suppress_sequence_followup_key_event(
            &mut suppressed,
            key,
            1,
            true,
        ));
        assert!(suppressed.contains(&key));

        assert!(suppress_sequence_followup_key_event(
            &mut suppressed,
            key,
            2,
            false,
        ));
        assert!(suppressed.contains(&key));

        assert!(suppress_sequence_followup_key_event(
            &mut suppressed,
            key,
            0,
            false,
        ));
        assert!(!suppressed.contains(&key));
    }

    #[test]
    fn grabbed_hotkey_without_passthrough_is_consumed() {
        let callbacks = HotkeyCallbacks {
            on_press: Arc::new(|| {}),
            on_release: None,
            min_hold: None,
            repeat_behavior: RepeatBehavior::Ignore,
            passthrough: false,
        };

        let mut registrations = HashMap::new();
        registrations.insert((KeyCode::KEY_A, vec![]), HotkeyRegistration { callbacks });

        let modifiers = HashSet::new();
        let mut active_presses = HashMap::new();
        let dispatch = collect_non_modifier_dispatch(
            KeyCode::KEY_A,
            1,
            Instant::now(),
            &modifiers,
            &registrations,
            &mut active_presses,
            false,
        );

        assert!(dispatch.matched_hotkey);
        assert!(!dispatch.passthrough);
        assert!(!should_forward_key_event_in_grab_mode(
            true,
            dispatch.matched_hotkey,
            dispatch.passthrough,
        ));
    }

    #[test]
    fn grabbed_hotkey_with_passthrough_is_forwarded() {
        let callbacks = HotkeyCallbacks {
            on_press: Arc::new(|| {}),
            on_release: None,
            min_hold: None,
            repeat_behavior: RepeatBehavior::Ignore,
            passthrough: true,
        };

        let mut registrations = HashMap::new();
        registrations.insert((KeyCode::KEY_A, vec![]), HotkeyRegistration { callbacks });

        let modifiers = HashSet::new();
        let mut active_presses = HashMap::new();
        let dispatch = collect_non_modifier_dispatch(
            KeyCode::KEY_A,
            1,
            Instant::now(),
            &modifiers,
            &registrations,
            &mut active_presses,
            false,
        );

        assert!(dispatch.matched_hotkey);
        assert!(dispatch.passthrough);
        assert!(should_forward_key_event_in_grab_mode(
            true,
            dispatch.matched_hotkey,
            dispatch.passthrough,
        ));
    }

    #[test]
    fn min_hold_delays_press_callback_until_release() {
        let press_count = Arc::new(AtomicUsize::new(0));
        let p = press_count.clone();

        let callbacks = HotkeyCallbacks {
            on_press: Arc::new(move || {
                p.fetch_add(1, Ordering::SeqCst);
            }),
            on_release: None,
            min_hold: Some(Duration::from_millis(50)),
            repeat_behavior: RepeatBehavior::Ignore,
            passthrough: false,
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
            false,
        ));
        dispatch_callbacks(collect_non_modifier_callbacks(
            KeyCode::KEY_A,
            0,
            t0 + Duration::from_millis(20),
            &modifiers,
            &registrations,
            &mut active_presses,
            false,
        ));
        dispatch_callbacks(collect_non_modifier_callbacks(
            KeyCode::KEY_A,
            1,
            t0,
            &modifiers,
            &registrations,
            &mut active_presses,
            false,
        ));
        dispatch_callbacks(collect_non_modifier_callbacks(
            KeyCode::KEY_A,
            0,
            t0 + Duration::from_millis(70),
            &modifiers,
            &registrations,
            &mut active_presses,
            false,
        ));

        assert_eq!(press_count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn min_hold_dispatches_on_tick_before_release() {
        let press_count = Arc::new(AtomicUsize::new(0));
        let p = press_count.clone();

        let callbacks = HotkeyCallbacks {
            on_press: Arc::new(move || {
                p.fetch_add(1, Ordering::SeqCst);
            }),
            on_release: None,
            min_hold: Some(Duration::from_millis(50)),
            repeat_behavior: RepeatBehavior::Ignore,
            passthrough: false,
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
            false,
        ));

        dispatch_callbacks(collect_due_hold_callbacks(
            t0 + Duration::from_millis(60),
            &registrations,
            &HashMap::new(),
            &HashMap::new(),
            &mut active_presses,
        ));

        dispatch_callbacks(collect_non_modifier_callbacks(
            KeyCode::KEY_A,
            0,
            t0 + Duration::from_millis(70),
            &modifiers,
            &registrations,
            &mut active_presses,
            false,
        ));

        assert_eq!(press_count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn repeat_event_respects_min_hold_threshold() {
        let press_count = Arc::new(AtomicUsize::new(0));
        let p = press_count.clone();

        let callbacks = HotkeyCallbacks {
            on_press: Arc::new(move || {
                p.fetch_add(1, Ordering::SeqCst);
            }),
            on_release: None,
            min_hold: Some(Duration::from_millis(50)),
            repeat_behavior: RepeatBehavior::Trigger,
            passthrough: false,
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
            false,
        ));

        dispatch_callbacks(collect_non_modifier_callbacks(
            KeyCode::KEY_A,
            2,
            now + Duration::from_millis(20),
            &modifiers,
            &registrations,
            &mut active_presses,
            false,
        ));

        dispatch_callbacks(collect_non_modifier_callbacks(
            KeyCode::KEY_A,
            2,
            now + Duration::from_millis(60),
            &modifiers,
            &registrations,
            &mut active_presses,
            false,
        ));

        assert_eq!(press_count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn zero_min_hold_triggers_press_on_key_down_only_once() {
        let press_count = Arc::new(AtomicUsize::new(0));
        let p = press_count.clone();

        let callbacks = HotkeyCallbacks {
            on_press: Arc::new(move || {
                p.fetch_add(1, Ordering::SeqCst);
            }),
            on_release: None,
            min_hold: Some(Duration::ZERO),
            repeat_behavior: RepeatBehavior::Ignore,
            passthrough: false,
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
            false,
        ));
        dispatch_callbacks(collect_non_modifier_callbacks(
            KeyCode::KEY_A,
            0,
            now + Duration::from_millis(1),
            &modifiers,
            &registrations,
            &mut active_presses,
            false,
        ));

        assert_eq!(press_count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn repeat_after_hold_does_not_double_fire_on_release() {
        let press_count = Arc::new(AtomicUsize::new(0));
        let p = press_count.clone();

        let callbacks = HotkeyCallbacks {
            on_press: Arc::new(move || {
                p.fetch_add(1, Ordering::SeqCst);
            }),
            on_release: None,
            min_hold: Some(Duration::from_millis(50)),
            repeat_behavior: RepeatBehavior::Trigger,
            passthrough: false,
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
            false,
        ));
        dispatch_callbacks(collect_non_modifier_callbacks(
            KeyCode::KEY_A,
            2,
            now + Duration::from_millis(60),
            &modifiers,
            &registrations,
            &mut active_presses,
            false,
        ));
        dispatch_callbacks(collect_non_modifier_callbacks(
            KeyCode::KEY_A,
            0,
            now + Duration::from_millis(70),
            &modifiers,
            &registrations,
            &mut active_presses,
            false,
        ));

        assert_eq!(press_count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn repeat_event_respects_trigger_option() {
        let press_count = Arc::new(AtomicUsize::new(0));
        let p = press_count.clone();

        let callbacks = HotkeyCallbacks {
            on_press: Arc::new(move || {
                p.fetch_add(1, Ordering::SeqCst);
            }),
            on_release: None,
            min_hold: None,
            repeat_behavior: RepeatBehavior::Trigger,
            passthrough: false,
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
            false,
        ));
        dispatch_callbacks(collect_non_modifier_callbacks(
            KeyCode::KEY_A,
            2,
            now + Duration::from_millis(1),
            &modifiers,
            &registrations,
            &mut active_presses,
            false,
        ));

        assert_eq!(press_count.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn repeat_event_is_ignored_when_not_enabled() {
        let press_count = Arc::new(AtomicUsize::new(0));
        let p = press_count.clone();

        let callbacks = HotkeyCallbacks {
            on_press: Arc::new(move || {
                p.fetch_add(1, Ordering::SeqCst);
            }),
            on_release: None,
            min_hold: None,
            repeat_behavior: RepeatBehavior::Ignore,
            passthrough: false,
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
            false,
        ));
        dispatch_callbacks(collect_non_modifier_callbacks(
            KeyCode::KEY_A,
            2,
            now + Duration::from_millis(1),
            &modifiers,
            &registrations,
            &mut active_presses,
            false,
        ));

        assert_eq!(press_count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn burst_repeat_events_dispatch_without_dropping_callbacks() {
        let press_count = Arc::new(AtomicUsize::new(0));
        let p = press_count.clone();

        let callbacks = HotkeyCallbacks {
            on_press: Arc::new(move || {
                p.fetch_add(1, Ordering::SeqCst);
            }),
            on_release: None,
            min_hold: None,
            repeat_behavior: RepeatBehavior::Trigger,
            passthrough: false,
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
            false,
        ));

        for offset in 1..=64 {
            dispatch_callbacks(collect_non_modifier_callbacks(
                KeyCode::KEY_A,
                2,
                now + Duration::from_millis(offset),
                &modifiers,
                &registrations,
                &mut active_presses,
                false,
            ));
        }

        assert_eq!(press_count.load(Ordering::SeqCst), 65);
    }

    #[test]
    fn modifier_tracker_cleans_state_on_disconnect() {
        let mut tracker = ModifierTracker::default();
        let device_a = PathBuf::from("/dev/input/event100");
        let device_b = PathBuf::from("/dev/input/event101");

        tracker.press(&device_a, KeyCode::KEY_LEFTCTRL);
        tracker.press(&device_b, KeyCode::KEY_LEFTSHIFT);
        tracker.disconnect(&device_a);

        let active = tracker.active_modifiers();
        assert!(!active.contains(&KeyCode::KEY_LEFTCTRL));
        assert!(active.contains(&KeyCode::KEY_LEFTSHIFT));
    }

    #[test]
    fn parse_hotplug_events_extracts_create_and_delete_entries() {
        let mut bytes = Vec::new();

        push_inotify_event(&mut bytes, libc::IN_CREATE, "event42");
        push_inotify_event(&mut bytes, libc::IN_DELETE, "event7");

        let parsed = parse_hotplug_events(&bytes, bytes.len());
        assert_eq!(
            parsed,
            vec![
                HotplugFsEvent {
                    mask: libc::IN_CREATE,
                    device_name: "event42".to_string(),
                },
                HotplugFsEvent {
                    mask: libc::IN_DELETE,
                    device_name: "event7".to_string(),
                },
            ]
        );
    }

    #[test]
    fn parse_hotplug_events_handles_misaligned_buffers() {
        let mut payload = Vec::new();
        push_inotify_event(&mut payload, libc::IN_CREATE, "event9");

        let mut prefixed = vec![0u8];
        prefixed.extend_from_slice(&payload);

        let parsed = parse_hotplug_events(&prefixed[1..], payload.len());
        assert_eq!(
            parsed,
            vec![HotplugFsEvent {
                mask: libc::IN_CREATE,
                device_name: "event9".to_string(),
            }]
        );
    }

    #[test]
    fn classify_hotplug_change_detects_add_and_remove() {
        let mut known_paths = HashSet::new();
        let add = HotplugFsEvent {
            mask: libc::IN_CREATE,
            device_name: "event44".to_string(),
        };

        let added = classify_hotplug_change(&add, &mut known_paths);
        assert_eq!(
            added,
            HotplugPathChange::Added(PathBuf::from("/dev/input/event44"))
        );

        let remove = HotplugFsEvent {
            mask: libc::IN_DELETE,
            device_name: "event44".to_string(),
        };

        let removed = classify_hotplug_change(&remove, &mut known_paths);
        assert_eq!(
            removed,
            HotplugPathChange::Removed(PathBuf::from("/dev/input/event44"))
        );
    }

    #[test]
    fn poll_ready_sources_reports_inotify_fd_errors() {
        let mut fds = [0; 2];
        let pipe_result = unsafe { libc::pipe2(fds.as_mut_ptr(), libc::O_CLOEXEC) };
        assert_eq!(pipe_result, 0);

        let read_fd = RawFdGuard::new(fds[0]);
        let write_fd = RawFdGuard::new(fds[1]);
        drop(write_fd);

        let err = poll_ready_sources(read_fd.raw_fd(), &[]).err().unwrap();
        assert_eq!(err.kind(), io::ErrorKind::Other);
    }

    #[test]
    fn listener_sets_stop_flag_on_poll_failure() {
        let mut fds = [0; 2];
        let pipe_result = unsafe { libc::pipe2(fds.as_mut_ptr(), libc::O_CLOEXEC) };
        assert_eq!(pipe_result, 0);

        let read_fd = RawFdGuard::new(fds[0]);
        let write_fd = RawFdGuard::new(fds[1]);
        drop(write_fd);

        let registrations = Arc::new(Mutex::new(HashMap::new()));
        let sequence_registrations = Arc::new(Mutex::new(HashMap::new()));
        let stop_flag = Arc::new(AtomicBool::new(false));

        listener_loop(
            Vec::new(),
            read_fd,
            ListenerState {
                registrations,
                sequence_registrations,
                device_registrations: Arc::new(Mutex::new(HashMap::new())),
                stop_flag: stop_flag.clone(),
                mode_registry: ModeRegistry::new(),
            },
            ListenerConfig::default(),
            None,
        );

        assert!(stop_flag.load(Ordering::SeqCst));
    }

    #[test]
    fn poll_wakes_quickly_when_fd_becomes_readable() {
        let mut fds = [0; 2];
        let pipe_result = unsafe { libc::pipe2(fds.as_mut_ptr(), libc::O_CLOEXEC) };
        assert_eq!(pipe_result, 0);

        let read_fd = RawFdGuard::new(fds[0]);
        let write_fd = RawFdGuard::new(fds[1]);

        let writer = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(5));
            let one = [1u8];
            unsafe {
                libc::write(write_fd.raw_fd(), one.as_ptr() as *const libc::c_void, 1);
            }
        });

        let start = Instant::now();
        let mut pollfds = [libc::pollfd {
            fd: read_fd.raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        }];
        let result = unsafe { libc::poll(pollfds.as_mut_ptr(), 1, 1_000) };

        writer.join().unwrap();

        assert_eq!(result, 1);
        assert!(start.elapsed() < Duration::from_millis(200));
    }

    #[test]
    fn polling_loop_shutdown_is_bounded_by_timeout() {
        let mut fds = [0; 2];
        let pipe_result = unsafe { libc::pipe2(fds.as_mut_ptr(), libc::O_CLOEXEC) };
        assert_eq!(pipe_result, 0);

        let read_fd = RawFdGuard::new(fds[0]);
        let _write_fd = RawFdGuard::new(fds[1]);

        let stop = Arc::new(AtomicBool::new(false));
        let stop_clone = stop.clone();
        let start = Instant::now();

        let join_handle = std::thread::spawn(move || {
            while !stop_clone.load(Ordering::SeqCst) {
                let _ = unsafe {
                    let mut pollfds = [libc::pollfd {
                        fd: read_fd.raw_fd(),
                        events: libc::POLLIN,
                        revents: 0,
                    }];
                    libc::poll(pollfds.as_mut_ptr(), 1, POLL_TIMEOUT_MS)
                };
            }
        });

        std::thread::sleep(Duration::from_millis(10));
        stop.store(true, Ordering::SeqCst);
        join_handle.join().unwrap();

        assert!(start.elapsed() < Duration::from_millis(200));
    }

    fn push_inotify_event(buffer: &mut Vec<u8>, mask: u32, name: &str) {
        let mut name_bytes = name.as_bytes().to_vec();
        name_bytes.push(0);
        while !name_bytes.len().is_multiple_of(4) {
            name_bytes.push(0);
        }

        let event = libc::inotify_event {
            wd: 1,
            mask,
            cookie: 0,
            len: name_bytes.len() as u32,
        };

        let event_bytes = unsafe {
            std::slice::from_raw_parts(
                &event as *const libc::inotify_event as *const u8,
                size_of::<libc::inotify_event>(),
            )
        };

        buffer.extend_from_slice(event_bytes);
        buffer.extend_from_slice(&name_bytes);
    }

    fn test_device_info(name: &str, vendor: u16, product: u16) -> DeviceInfo {
        DeviceInfo {
            name: name.to_string(),
            vendor,
            product,
        }
    }

    fn device_registration(
        id: DeviceRegistrationId,
        hotkey_key: HotkeyKey,
        filter: DeviceFilter,
        counter: Arc<AtomicUsize>,
    ) -> (DeviceRegistrationId, DeviceHotkeyRegistration) {
        (
            id,
            DeviceHotkeyRegistration {
                hotkey_key,
                filter,
                callbacks: no_release_callbacks(counter),
            },
        )
    }

    #[test]
    fn device_specific_hotkey_fires_on_matching_device() {
        let count = Arc::new(AtomicUsize::new(0));
        let key = (KeyCode::KEY_1, vec![]);
        let filter = DeviceFilter::name_contains("StreamDeck");
        let info = test_device_info("Elgato StreamDeck XL", 0x0fd9, 0x006c);

        let device_regs: HashMap<DeviceRegistrationId, DeviceHotkeyRegistration> =
            [device_registration(1, key, filter, count.clone())]
                .into_iter()
                .collect();

        let modifiers = HashSet::new();
        let mut active_presses = HashMap::new();
        let now = Instant::now();

        let dispatch = collect_device_specific_dispatch(
            KeyCode::KEY_1,
            1,
            now,
            &info,
            &modifiers,
            &device_regs,
            &mut active_presses,
        );

        assert!(dispatch.matched);
        dispatch_callbacks(dispatch.callbacks);
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn device_specific_hotkey_does_not_fire_on_wrong_device() {
        let count = Arc::new(AtomicUsize::new(0));
        let key = (KeyCode::KEY_1, vec![]);
        let filter = DeviceFilter::name_contains("StreamDeck");
        let info = test_device_info("AT Translated Set 2 keyboard", 0x0001, 0x0001);

        let device_regs: HashMap<DeviceRegistrationId, DeviceHotkeyRegistration> =
            [device_registration(1, key, filter, count.clone())]
                .into_iter()
                .collect();

        let modifiers = HashSet::new();
        let mut active_presses = HashMap::new();
        let now = Instant::now();

        let dispatch = collect_device_specific_dispatch(
            KeyCode::KEY_1,
            1,
            now,
            &info,
            &modifiers,
            &device_regs,
            &mut active_presses,
        );

        assert!(!dispatch.matched);
        assert_eq!(count.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn device_specific_hotkey_uses_per_device_modifiers() {
        let count = Arc::new(AtomicUsize::new(0));
        // Register Ctrl+A on StreamDeck
        let key = (KeyCode::KEY_A, vec![KeyCode::KEY_LEFTCTRL]);
        let filter = DeviceFilter::name_contains("StreamDeck");
        let info = test_device_info("StreamDeck", 0x0fd9, 0x006c);

        let device_regs: HashMap<DeviceRegistrationId, DeviceHotkeyRegistration> =
            [device_registration(1, key, filter, count.clone())]
                .into_iter()
                .collect();

        // Device does NOT have Ctrl pressed (another device does)
        let device_modifiers: HashSet<KeyCode> = HashSet::new();
        let mut active_presses = HashMap::new();
        let now = Instant::now();

        let dispatch = collect_device_specific_dispatch(
            KeyCode::KEY_A,
            1,
            now,
            &info,
            &device_modifiers,
            &device_regs,
            &mut active_presses,
        );

        // Should NOT match because device doesn't have Ctrl
        assert!(!dispatch.matched);
        assert_eq!(count.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn device_specific_hotkey_matches_with_correct_per_device_modifiers() {
        let count = Arc::new(AtomicUsize::new(0));
        let key = (KeyCode::KEY_A, vec![KeyCode::KEY_LEFTCTRL]);
        let filter = DeviceFilter::name_contains("StreamDeck");
        let info = test_device_info("StreamDeck", 0x0fd9, 0x006c);

        let device_regs: HashMap<DeviceRegistrationId, DeviceHotkeyRegistration> =
            [device_registration(1, key, filter, count.clone())]
                .into_iter()
                .collect();

        // Device HAS Ctrl pressed
        let device_modifiers: HashSet<KeyCode> =
            [KeyCode::KEY_LEFTCTRL].into_iter().collect();
        let mut active_presses = HashMap::new();
        let now = Instant::now();

        let dispatch = collect_device_specific_dispatch(
            KeyCode::KEY_A,
            1,
            now,
            &info,
            &device_modifiers,
            &device_regs,
            &mut active_presses,
        );

        assert!(dispatch.matched);
        dispatch_callbacks(dispatch.callbacks);
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn global_hotkey_uses_aggregate_modifiers_unchanged() {
        // Global hotkey should still use aggregate modifiers
        let press_count = Arc::new(AtomicUsize::new(0));
        let p = press_count.clone();

        let callbacks = HotkeyCallbacks {
            on_press: Arc::new(move || {
                p.fetch_add(1, Ordering::SeqCst);
            }),
            on_release: None,
            min_hold: None,
            repeat_behavior: RepeatBehavior::Ignore,
            passthrough: false,
        };

        let mut registrations = HashMap::new();
        registrations.insert(
            (KeyCode::KEY_A, vec![KeyCode::KEY_LEFTCTRL]),
            HotkeyRegistration { callbacks },
        );

        // Aggregate modifiers include Ctrl (from any device)
        let aggregate: HashSet<KeyCode> = [KeyCode::KEY_LEFTCTRL].into_iter().collect();
        let mut active_presses = HashMap::new();
        let now = Instant::now();

        dispatch_callbacks(collect_non_modifier_callbacks(
            KeyCode::KEY_A,
            1,
            now,
            &aggregate,
            &registrations,
            &mut active_presses,
            false,
        ));

        assert_eq!(press_count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn modifier_tracker_returns_per_device_modifiers() {
        let mut tracker = ModifierTracker::default();
        let device_a = PathBuf::from("/dev/input/event100");
        let device_b = PathBuf::from("/dev/input/event101");

        tracker.press(&device_a, KeyCode::KEY_LEFTCTRL);
        tracker.press(&device_b, KeyCode::KEY_LEFTSHIFT);

        let a_mods = tracker.device_modifiers(&device_a);
        assert!(a_mods.contains(&KeyCode::KEY_LEFTCTRL));
        assert!(!a_mods.contains(&KeyCode::KEY_LEFTSHIFT));

        let b_mods = tracker.device_modifiers(&device_b);
        assert!(!b_mods.contains(&KeyCode::KEY_LEFTCTRL));
        assert!(b_mods.contains(&KeyCode::KEY_LEFTSHIFT));

        // Aggregate has both
        let agg = tracker.active_modifiers();
        assert!(agg.contains(&KeyCode::KEY_LEFTCTRL));
        assert!(agg.contains(&KeyCode::KEY_LEFTSHIFT));
    }

    #[test]
    fn modifier_tracker_returns_empty_for_unknown_device() {
        let tracker = ModifierTracker::default();
        let unknown = PathBuf::from("/dev/input/event999");
        assert!(tracker.device_modifiers(&unknown).is_empty());
    }

    #[test]
    fn device_specific_usb_id_filter_matches() {
        let count = Arc::new(AtomicUsize::new(0));
        let key = (KeyCode::KEY_F1, vec![]);
        let filter = DeviceFilter::usb(0x1234, 0x5678);
        let info = test_device_info("Custom Macro Pad", 0x1234, 0x5678);

        let device_regs: HashMap<DeviceRegistrationId, DeviceHotkeyRegistration> =
            [device_registration(1, key, filter, count.clone())]
                .into_iter()
                .collect();

        let modifiers = HashSet::new();
        let mut active_presses = HashMap::new();
        let now = Instant::now();

        let dispatch = collect_device_specific_dispatch(
            KeyCode::KEY_F1,
            1,
            now,
            &info,
            &modifiers,
            &device_regs,
            &mut active_presses,
        );

        assert!(dispatch.matched);
        dispatch_callbacks(dispatch.callbacks);
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn device_specific_release_fires_after_press() {
        let press_count = Arc::new(AtomicUsize::new(0));
        let release_count = Arc::new(AtomicUsize::new(0));
        let pc = press_count.clone();
        let rc = release_count.clone();

        let filter = DeviceFilter::name_contains("StreamDeck");
        let info = test_device_info("StreamDeck", 0x0fd9, 0x006c);
        let key = (KeyCode::KEY_1, vec![]);

        let device_regs: HashMap<DeviceRegistrationId, DeviceHotkeyRegistration> = [(
            1,
            DeviceHotkeyRegistration {
                hotkey_key: key,
                filter,
                callbacks: HotkeyCallbacks {
                    on_press: Arc::new(move || {
                        pc.fetch_add(1, Ordering::SeqCst);
                    }),
                    on_release: Some(Arc::new(move || {
                        rc.fetch_add(1, Ordering::SeqCst);
                    })),
                    min_hold: None,
                    repeat_behavior: RepeatBehavior::Ignore,
                    passthrough: false,
                },
            },
        )]
        .into_iter()
        .collect();

        let modifiers = HashSet::new();
        let mut active_presses = HashMap::new();
        let now = Instant::now();

        // Press
        let press_dispatch = collect_device_specific_dispatch(
            KeyCode::KEY_1,
            1,
            now,
            &info,
            &modifiers,
            &device_regs,
            &mut active_presses,
        );
        dispatch_callbacks(press_dispatch.callbacks);

        // Release
        let release_dispatch = collect_device_specific_dispatch(
            KeyCode::KEY_1,
            0,
            now + Duration::from_millis(10),
            &info,
            &modifiers,
            &device_regs,
            &mut active_presses,
        );
        dispatch_callbacks(release_dispatch.callbacks);

        assert_eq!(press_count.load(Ordering::SeqCst), 1);
        assert_eq!(release_count.load(Ordering::SeqCst), 1);
    }
}
