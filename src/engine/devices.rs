//! Device discovery, hotplug, and capability detection.
//!
//! Manages the set of active input devices. Uses inotify to watch
//! `/dev/input/` for device add/remove events. Probes new devices for
//! keyboard capabilities before adding them to the poll set.
//!
//! # Reference
//!
//! Prior art: `archive/v0/src/listener/io.rs`,
//! `archive/v0/src/listener/hotplug.rs`,
//! `archive/v0/src/device.rs`

use std::collections::HashMap;
use std::collections::HashSet;
use std::ffi::CStr;
use std::ffi::CString;
use std::io;
use std::mem::size_of;
use std::os::fd::AsRawFd;
use std::os::fd::FromRawFd;
use std::os::fd::OwnedFd;
use std::os::fd::RawFd;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;
use std::path::PathBuf;

use evdev::Device;
use evdev::EventSummary;
use evdev::InputEvent;
use evdev::KeyCode;

use crate::engine::key_state::KeyState;
use crate::engine::key_state::KeyTransition;
use crate::Key;

const INPUT_DIRECTORY: &str = "/dev/input";
const HOTPLUG_BUFFER_SIZE: usize = 4096;

/// Whether devices should be grabbed for exclusive access.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DeviceGrabMode {
    /// Normal mode — listen passively, events reach other applications.
    Shared,
    /// Grab mode — exclusive access, events only reach us.
    Exclusive,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct HotplugFsEvent {
    pub(crate) mask: u32,
    pub(crate) device_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum HotplugPathChange {
    Added(PathBuf),
    Removed(PathBuf),
    Unchanged,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DiscoveryOutcome {
    Keyboard,
    NotKeyboard,
    Skip,
}

#[derive(Debug)]
struct ManagedDevice {
    path: PathBuf,
    info: DeviceInfo,
    device: Device,
}

#[derive(Debug)]
struct DeviceInfo {
    name: String,
    vendor: u16,
    product: u16,
}

impl DeviceInfo {
    fn from_device(device: &Device) -> Self {
        let input_id = device.input_id();
        Self {
            name: device.name().unwrap_or_default().to_string(),
            vendor: input_id.vendor(),
            product: input_id.product(),
        }
    }
}

#[derive(Debug)]
pub(crate) struct DeviceManager {
    input_dir: PathBuf,
    grab_mode: DeviceGrabMode,
    inotify_fd: Option<OwnedFd>,
    devices: HashMap<RawFd, ManagedDevice>,
    poll_fds: Vec<RawFd>,
}

impl Default for DeviceManager {
    fn default() -> Self {
        Self::new(Path::new(INPUT_DIRECTORY), DeviceGrabMode::Shared)
    }
}

impl DeviceManager {
    pub(crate) fn default_with_grab(grab_mode: DeviceGrabMode) -> Self {
        Self::new(Path::new(INPUT_DIRECTORY), grab_mode)
    }

    pub(crate) fn new(input_dir: &Path, grab_mode: DeviceGrabMode) -> Self {
        let mut manager = Self {
            input_dir: input_dir.to_path_buf(),
            grab_mode,
            inotify_fd: initialize_inotify(input_dir).ok(),
            devices: HashMap::new(),
            poll_fds: Vec::new(),
        };

        manager.discover_existing_devices();
        manager.rebuild_poll_fds();
        manager
    }

    fn discover_existing_devices(&mut self) {
        let discover_result = discover_devices_in_dir_with(&self.input_dir, probe_keyboard_device);

        if let Ok(paths) = discover_result {
            for path in paths {
                self.add_device_path(&path);
            }
        }
    }

    fn add_device_path(&mut self, path: &Path) {
        if self.devices.values().any(|device| device.path == path) {
            return;
        }

        let open_result = open_keyboard_device(path, self.grab_mode);
        let Some(device) = open_result.ok().flatten() else {
            return;
        };

        let fd = device.device.as_raw_fd();
        self.devices.insert(fd, device);
        self.rebuild_poll_fds();
    }

    fn remove_device_fd(&mut self, fd: RawFd, key_state: &mut KeyState) {
        if self.devices.remove(&fd).is_some() {
            key_state.disconnect_device(fd);
            self.rebuild_poll_fds();
        }
    }

    fn remove_device_path(&mut self, path: &Path, key_state: &mut KeyState) {
        let Some(fd) = self
            .devices
            .iter()
            .find_map(|(&fd, device)| (device.path == path).then_some(fd))
        else {
            return;
        };

        self.remove_device_fd(fd, key_state);
    }

    fn rebuild_poll_fds(&mut self) {
        self.poll_fds.clear();

        if let Some(inotify_fd) = self.inotify_fd.as_ref() {
            self.poll_fds.push(inotify_fd.as_raw_fd());
        }

        let mut device_fds: Vec<_> = self.devices.keys().copied().collect();
        device_fds.sort_unstable();
        self.poll_fds.extend(device_fds);
    }

    fn process_hotplug_events(&mut self, key_state: &mut KeyState) {
        let Some(inotify_fd) = self.inotify_fd.as_ref().map(AsRawFd::as_raw_fd) else {
            return;
        };

        let mut buffer = [0_u8; HOTPLUG_BUFFER_SIZE];
        let mut known_paths: HashSet<PathBuf> = self
            .devices
            .values()
            .map(|device| device.path.clone())
            .collect();

        loop {
            // SAFETY: `buffer` is valid writable memory and `inotify_fd`
            // references an open inotify descriptor.
            let read_result = unsafe {
                libc::read(
                    inotify_fd,
                    (&raw mut buffer).cast::<libc::c_void>(),
                    buffer.len(),
                )
            };

            if read_result < 0 {
                let error = io::Error::last_os_error();
                if error.kind() == io::ErrorKind::Interrupted {
                    continue;
                }
                if error.kind() == io::ErrorKind::WouldBlock {
                    break;
                }
                break;
            }

            if read_result == 0 {
                break;
            }

            let bytes_read = usize::try_from(read_result).unwrap_or(0);
            for event in parse_hotplug_events(&buffer, bytes_read) {
                match classify_hotplug_change(&event, &mut known_paths, &self.input_dir) {
                    HotplugPathChange::Added(path) => {
                        self.add_device_path(&path);
                    }
                    HotplugPathChange::Removed(path) => {
                        self.remove_device_path(&path, key_state);
                    }
                    HotplugPathChange::Unchanged => {}
                }
            }
        }
    }

    fn process_device_fd(
        &mut self,
        fd: RawFd,
        revents: i16,
        key_state: &mut KeyState,
        collected_events: &mut Vec<DeviceKeyEvent>,
    ) {
        if (revents & (libc::POLLERR | libc::POLLHUP | libc::POLLNVAL)) != 0 {
            self.remove_device_fd(fd, key_state);
            return;
        }

        if (revents & libc::POLLIN) == 0 {
            return;
        }

        let Some(device) = self.devices.get_mut(&fd) else {
            return;
        };

        match read_key_events(&mut device.device) {
            Ok(events) => {
                tracing::trace!(
                    device_name = %device.info.name,
                    vendor = device.info.vendor,
                    product = device.info.product,
                    event_count = events.len(),
                    "processed device events"
                );

                for event in events {
                    collected_events.push(DeviceKeyEvent {
                        device_fd: fd,
                        key: event.key,
                        transition: event.transition,
                    });
                }
            }
            Err(error) if should_drop_device(&error) => {
                self.remove_device_fd(fd, key_state);
            }
            Err(_) => {}
        }
    }

    #[must_use]
    pub(crate) fn poll_fds(&self) -> &[RawFd] {
        &self.poll_fds
    }

    pub(crate) fn process_polled_events(
        &mut self,
        polled_fds: &[libc::pollfd],
        key_state: &mut KeyState,
    ) -> Vec<DeviceKeyEvent> {
        let mut collected_events = Vec::new();

        let ready_fds: Vec<_> = polled_fds
            .iter()
            .filter(|pollfd| pollfd.revents != 0)
            .map(|pollfd| (pollfd.fd, pollfd.revents))
            .collect();

        for (fd, revents) in ready_fds {
            if self
                .inotify_fd
                .as_ref()
                .is_some_and(|inotify_fd| inotify_fd.as_raw_fd() == fd)
            {
                self.process_hotplug_events(key_state);
            } else {
                self.process_device_fd(fd, revents, key_state, &mut collected_events);
            }
        }

        collected_events
    }
}

fn initialize_inotify(input_dir: &Path) -> io::Result<OwnedFd> {
    // SAFETY: Calls libc with constant flags.
    let raw_fd = unsafe { libc::inotify_init1(libc::IN_NONBLOCK | libc::IN_CLOEXEC) };
    if raw_fd < 0 {
        return Err(io::Error::last_os_error());
    }

    // SAFETY: `raw_fd` is an owned descriptor returned by `inotify_init1`.
    let fd = unsafe { OwnedFd::from_raw_fd(raw_fd) };
    let path_cstr = CString::new(input_dir.as_os_str().as_bytes()).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "input directory contained interior NUL byte",
        )
    })?;

    // SAFETY: `fd` is a valid inotify descriptor and `path_cstr` points to a
    // valid NUL-terminated string.
    let watch_result = unsafe {
        libc::inotify_add_watch(
            fd.as_raw_fd(),
            path_cstr.as_ptr(),
            libc::IN_CREATE
                | libc::IN_DELETE
                | libc::IN_MOVED_FROM
                | libc::IN_MOVED_TO
                | libc::IN_DELETE_SELF
                | libc::IN_MOVE_SELF,
        )
    };

    if watch_result < 0 {
        return Err(io::Error::last_os_error());
    }

    Ok(fd)
}

fn probe_keyboard_device(path: &Path) -> DiscoveryOutcome {
    let Ok(device) = Device::open(path) else {
        return DiscoveryOutcome::Skip;
    };

    if is_virtual_forwarder(&device) {
        return DiscoveryOutcome::Skip;
    }

    if is_keyboard_device(&device) {
        DiscoveryOutcome::Keyboard
    } else {
        DiscoveryOutcome::NotKeyboard
    }
}

fn open_keyboard_device(
    path: &Path,
    grab_mode: DeviceGrabMode,
) -> io::Result<Option<ManagedDevice>> {
    let mut device = Device::open(path)?;

    if !is_keyboard_device(&device) {
        return Ok(None);
    }

    if is_virtual_forwarder(&device) {
        return Ok(None);
    }

    if matches!(grab_mode, DeviceGrabMode::Exclusive) {
        device.grab()?;
    }

    device.set_nonblocking(true)?;

    Ok(Some(ManagedDevice {
        path: path.to_path_buf(),
        info: DeviceInfo::from_device(&device),
        device,
    }))
}

/// Returns `true` if this device is our own virtual forwarder.
///
/// Used to prevent feedback loops: the forwarder creates a virtual keyboard
/// device, and without this check we'd discover and grab our own output device.
fn is_virtual_forwarder(device: &Device) -> bool {
    device.name().is_some_and(is_virtual_forwarder_name)
}

/// Returns `true` if this device name matches our virtual forwarder name.
fn is_virtual_forwarder_name(name: &str) -> bool {
    name == crate::engine::forwarder::VIRTUAL_DEVICE_NAME
}

fn is_keyboard_device(device: &Device) -> bool {
    supports_keyboard_keys(device.supported_keys())
}

fn supports_keyboard_keys(keys: Option<&evdev::AttributeSetRef<KeyCode>>) -> bool {
    keys.is_some_and(|supported_keys| {
        supported_keys.contains(KeyCode::KEY_A)
            && supported_keys.contains(KeyCode::KEY_Z)
            && supported_keys.contains(KeyCode::KEY_ENTER)
    })
}

fn discover_devices_in_dir_with<F>(input_dir: &Path, mut classify: F) -> io::Result<Vec<PathBuf>>
where
    F: FnMut(&Path) -> DiscoveryOutcome,
{
    let mut device_paths = Vec::new();

    for entry_result in std::fs::read_dir(input_dir)? {
        let Ok(entry) = entry_result else {
            continue;
        };

        let path = entry.path();
        let Some(name) = path.file_name().and_then(|candidate| candidate.to_str()) else {
            continue;
        };

        if !name.starts_with("event") {
            continue;
        }

        if matches!(classify(&path), DiscoveryOutcome::Keyboard) {
            device_paths.push(path);
        }
    }

    device_paths.sort_unstable();
    Ok(device_paths)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct DeviceKeyEvent {
    pub(crate) device_fd: RawFd,
    pub(crate) key: Key,
    pub(crate) transition: KeyTransition,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ObservedKeyEvent {
    key: Key,
    transition: KeyTransition,
}

fn read_key_events(device: &mut Device) -> io::Result<Vec<ObservedKeyEvent>> {
    let mut events = Vec::new();

    for event in device.fetch_events()? {
        if let Some(observed) = convert_input_event(event) {
            events.push(observed);
        }
    }

    Ok(events)
}

fn convert_input_event(event: InputEvent) -> Option<ObservedKeyEvent> {
    match event.destructure() {
        EventSummary::Key(_, key_code, value) => {
            let transition = key_transition(value)?;
            let key = Key::from(key_code);
            if key == Key::Unknown {
                return None;
            }
            Some(ObservedKeyEvent { key, transition })
        }
        _ => None,
    }
}

fn key_transition(value: i32) -> Option<KeyTransition> {
    match value {
        1 => Some(KeyTransition::Press),
        0 => Some(KeyTransition::Release),
        2 => Some(KeyTransition::Repeat),
        _ => None,
    }
}

fn should_drop_device(error: &io::Error) -> bool {
    error.raw_os_error() == Some(libc::ENODEV)
        || error.kind() == io::ErrorKind::NotFound
        || error.kind() == io::ErrorKind::UnexpectedEof
}

pub(crate) fn classify_hotplug_change(
    event: &HotplugFsEvent,
    known_paths: &mut HashSet<PathBuf>,
    input_dir: &Path,
) -> HotplugPathChange {
    if !event.device_name.starts_with("event") {
        return HotplugPathChange::Unchanged;
    }

    let path = input_dir.join(&event.device_name);

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

pub(crate) fn parse_hotplug_events(buffer: &[u8], bytes_read: usize) -> Vec<HotplugFsEvent> {
    let mut events = Vec::new();
    let mut offset = 0_usize;

    while offset + size_of::<libc::inotify_event>() <= bytes_read {
        #[allow(clippy::cast_ptr_alignment)]
        let event_ptr = buffer[offset..].as_ptr().cast::<libc::inotify_event>();
        // SAFETY: `event_ptr` points into `buffer`. We only read when enough
        // bytes are available, and use unaligned reads to handle kernel-packed
        // event boundaries.
        let event = unsafe { std::ptr::read_unaligned(event_ptr) };

        let name_start = offset + size_of::<libc::inotify_event>();
        let Some(name_end) = name_start.checked_add(event.len as usize) else {
            break;
        };

        let device_name = if event.len > 0 && name_end <= bytes_read {
            parse_hotplug_name(&buffer[name_start..name_end])
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

fn parse_hotplug_name(name_bytes: &[u8]) -> String {
    let cstr_end = name_bytes
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(name_bytes.len());

    if cstr_end == 0 {
        return String::new();
    }

    CStr::from_bytes_with_nul(&name_bytes[..=cstr_end.min(name_bytes.len() - 1)])
        .ok()
        .and_then(|value| value.to_str().ok())
        .map_or_else(
            || String::from_utf8_lossy(&name_bytes[..cstr_end]).to_string(),
            std::string::ToString::to_string,
        )
}

#[cfg(test)]
mod tests {
    use std::path::Path;
    use std::time::SystemTime;
    use std::time::UNIX_EPOCH;

    use evdev::EventType;
    use evdev::InputEvent;
    use evdev::KeyCode;

    use super::classify_hotplug_change;
    use super::convert_input_event;
    use super::discover_devices_in_dir_with;
    use super::parse_hotplug_events;
    use super::DiscoveryOutcome;
    use super::HotplugFsEvent;
    use super::HotplugPathChange;
    use super::ObservedKeyEvent;
    use crate::engine::key_state::KeyTransition;
    use crate::Key;

    #[test]
    fn discover_event_devices_ignores_non_event_entries_and_non_keyboards() {
        let temp = unique_test_dir();

        std::fs::create_dir_all(&temp).expect("temp dir should be created");
        std::fs::File::create(temp.join("event0"))
            .expect("event0 should be created for discovery test");
        std::fs::File::create(temp.join("event1"))
            .expect("event1 should be created for discovery test");
        std::fs::File::create(temp.join("mouse0"))
            .expect("mouse0 should be created for discovery test");

        let keyboards = discover_devices_in_dir_with(&temp, |path| {
            match path.file_name().and_then(|name| name.to_str()) {
                Some("event0") => DiscoveryOutcome::Keyboard,
                Some("event1") => DiscoveryOutcome::NotKeyboard,
                _ => DiscoveryOutcome::Skip,
            }
        })
        .expect("discovery should succeed for temp dir");

        assert_eq!(keyboards, vec![temp.join("event0")]);

        std::fs::remove_dir_all(temp).expect("temp dir should be removed");
    }

    #[test]
    fn parse_hotplug_events_extracts_device_names() {
        let mut buffer = Vec::new();

        append_inotify_event(&mut buffer, libc::IN_CREATE, "event3");
        append_inotify_event(&mut buffer, libc::IN_DELETE, "mouse0");

        let events = parse_hotplug_events(&buffer, buffer.len());
        assert_eq!(
            events,
            vec![
                HotplugFsEvent {
                    mask: libc::IN_CREATE,
                    device_name: "event3".into(),
                },
                HotplugFsEvent {
                    mask: libc::IN_DELETE,
                    device_name: "mouse0".into(),
                },
            ]
        );
    }

    #[test]
    fn classify_hotplug_change_distinguishes_add_remove_and_ignore() {
        let mut known_paths = std::collections::HashSet::new();
        let input_dir = Path::new("/dev/input");

        let add_event = HotplugFsEvent {
            mask: libc::IN_CREATE,
            device_name: "event7".into(),
        };
        let added = classify_hotplug_change(&add_event, &mut known_paths, input_dir);
        assert_eq!(added, HotplugPathChange::Added(input_dir.join("event7")));

        let remove_event = HotplugFsEvent {
            mask: libc::IN_DELETE,
            device_name: "event7".into(),
        };
        let removed = classify_hotplug_change(&remove_event, &mut known_paths, input_dir);
        assert_eq!(
            removed,
            HotplugPathChange::Removed(input_dir.join("event7"))
        );

        let ignored = classify_hotplug_change(
            &HotplugFsEvent {
                mask: libc::IN_CREATE,
                device_name: "js0".into(),
            },
            &mut known_paths,
            input_dir,
        );
        assert_eq!(ignored, HotplugPathChange::Unchanged);
    }

    #[test]
    fn virtual_forwarder_name_is_detected() {
        use super::is_virtual_forwarder_name;
        use crate::engine::forwarder::VIRTUAL_DEVICE_NAME;

        assert!(is_virtual_forwarder_name(VIRTUAL_DEVICE_NAME));
        assert!(!is_virtual_forwarder_name("AT Translated Set 2 keyboard"));
        assert!(!is_virtual_forwarder_name("Logitech USB Keyboard"));
        assert!(!is_virtual_forwarder_name(""));
    }

    #[test]
    fn key_input_events_are_converted_to_domain_keys() {
        let press = convert_input_event(InputEvent::new(EventType::KEY.0, KeyCode::KEY_C.0, 1));
        assert_eq!(
            press,
            Some(ObservedKeyEvent {
                key: Key::C,
                transition: KeyTransition::Press,
            })
        );

        let release = convert_input_event(InputEvent::new(EventType::KEY.0, KeyCode::KEY_C.0, 0));
        assert_eq!(
            release,
            Some(ObservedKeyEvent {
                key: Key::C,
                transition: KeyTransition::Release,
            })
        );

        let repeat = convert_input_event(InputEvent::new(EventType::KEY.0, KeyCode::KEY_C.0, 2));
        assert_eq!(
            repeat,
            Some(ObservedKeyEvent {
                key: Key::C,
                transition: KeyTransition::Repeat,
            })
        );

        let ignored =
            convert_input_event(InputEvent::new(EventType::KEY.0, KeyCode::new(1023).0, 1));
        assert_eq!(ignored, None);
    }

    fn unique_test_dir() -> std::path::PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos();

        std::env::temp_dir().join(format!(
            "keybound-discovery-test-{}-{}",
            std::process::id(),
            nanos
        ))
    }

    fn append_inotify_event(buffer: &mut Vec<u8>, mask: u32, name: &str) {
        let mut name_bytes = name.as_bytes().to_vec();
        name_bytes.push(0);

        let event = libc::inotify_event {
            wd: 1,
            mask,
            cookie: 0,
            len: u32::try_from(name_bytes.len()).expect("name should fit in u32"),
        };

        // SAFETY: We are serializing a POD C struct to a byte buffer.
        let event_bytes = unsafe {
            std::slice::from_raw_parts(
                (&raw const event).cast::<u8>(),
                std::mem::size_of::<libc::inotify_event>(),
            )
        };

        buffer.extend_from_slice(event_bytes);
        buffer.extend_from_slice(&name_bytes);
    }
}
