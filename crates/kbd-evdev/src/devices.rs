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
use kbd_core::Key;
use kbd_core::key_state::KeyTransition;

use crate::KeyCodeExt;
use crate::forwarder::VIRTUAL_DEVICE_NAME;

pub const INPUT_DIRECTORY: &str = "/dev/input";
const HOTPLUG_BUFFER_SIZE: usize = 4096;

/// Whether devices should be grabbed for exclusive access.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceGrabMode {
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
    device: Device,
}

/// A key event from a specific device.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeviceKeyEvent {
    pub device_fd: RawFd,
    pub key: Key,
    pub transition: KeyTransition,
}

/// Result of processing polled events.
///
/// Separates key events from device disconnections so the caller can
/// update its own key state tracking without `DeviceManager` needing
/// access to `KeyState`.
#[derive(Debug)]
pub struct PollResult {
    /// Key events from devices that had data ready.
    pub key_events: Vec<DeviceKeyEvent>,
    /// File descriptors of devices that were removed during this poll
    /// (due to hotplug removal or device errors).
    pub disconnected_devices: Vec<RawFd>,
}

#[derive(Debug)]
pub struct DeviceManager {
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
    #[must_use]
    pub fn new(input_dir: &Path, grab_mode: DeviceGrabMode) -> Self {
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
        let discover_result =
            discover_devices_in_dir_with(&self.input_dir, DiscoveryOutcome::probe);

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

        let open_result = ManagedDevice::open(path, self.grab_mode);
        let Some(device) = open_result.ok().flatten() else {
            return;
        };

        let fd = device.device.as_raw_fd();
        self.devices.insert(fd, device);
        self.rebuild_poll_fds();
    }

    fn remove_device_fd(&mut self, fd: RawFd) -> bool {
        if self.devices.remove(&fd).is_some() {
            self.rebuild_poll_fds();
            true
        } else {
            false
        }
    }

    fn remove_device_path(&mut self, path: &Path) -> Option<RawFd> {
        let fd = self
            .devices
            .iter()
            .find_map(|(&fd, device)| (device.path == path).then_some(fd))?;

        if self.remove_device_fd(fd) {
            Some(fd)
        } else {
            None
        }
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

    fn process_hotplug_events(&mut self, disconnected: &mut Vec<RawFd>) {
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
                match event.classify_change(&mut known_paths, &self.input_dir) {
                    HotplugPathChange::Added(path) => {
                        self.add_device_path(&path);
                    }
                    HotplugPathChange::Removed(path) => {
                        if let Some(fd) = self.remove_device_path(&path) {
                            disconnected.push(fd);
                        }
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
        collected_events: &mut Vec<DeviceKeyEvent>,
        disconnected: &mut Vec<RawFd>,
    ) {
        if (revents & (libc::POLLERR | libc::POLLHUP | libc::POLLNVAL)) != 0 {
            if self.remove_device_fd(fd) {
                disconnected.push(fd);
            }
            return;
        }

        if (revents & libc::POLLIN) == 0 {
            return;
        }

        let Some(device) = self.devices.get_mut(&fd) else {
            return;
        };

        match device.device.read_key_events() {
            Ok(events) => {
                for event in events {
                    collected_events.push(DeviceKeyEvent {
                        device_fd: fd,
                        key: event.key,
                        transition: event.transition,
                    });
                }
            }
            Err(error) if should_drop_device(&error) => {
                if self.remove_device_fd(fd) {
                    disconnected.push(fd);
                }
            }
            Err(_) => {}
        }
    }

    #[must_use]
    pub fn poll_fds(&self) -> &[RawFd] {
        &self.poll_fds
    }

    /// Process all ready file descriptors from a completed poll.
    ///
    /// Returns key events and a list of device fds that were disconnected,
    /// so the caller can update its own key state tracking.
    pub fn process_polled_events(&mut self, polled_fds: &[libc::pollfd]) -> PollResult {
        let mut key_events = Vec::new();
        let mut disconnected_devices = Vec::new();

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
                self.process_hotplug_events(&mut disconnected_devices);
            } else {
                self.process_device_fd(fd, revents, &mut key_events, &mut disconnected_devices);
            }
        }

        PollResult {
            key_events,
            disconnected_devices,
        }
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

impl DiscoveryOutcome {
    fn probe(path: &Path) -> Self {
        let Ok(device) = Device::open(path) else {
            return Self::Skip;
        };

        if device.is_virtual_forwarder() {
            return Self::Skip;
        }

        if device.is_keyboard() {
            Self::Keyboard
        } else {
            Self::NotKeyboard
        }
    }
}

impl ManagedDevice {
    fn open(path: &Path, grab_mode: DeviceGrabMode) -> io::Result<Option<Self>> {
        let mut device = Device::open(path)?;

        if !device.is_keyboard() {
            return Ok(None);
        }

        if device.is_virtual_forwarder() {
            return Ok(None);
        }

        if matches!(grab_mode, DeviceGrabMode::Exclusive) {
            device.grab()?;
        }

        device.set_nonblocking(true)?;

        Ok(Some(Self {
            path: path.to_path_buf(),
            device,
        }))
    }
}

trait DeviceExt {
    /// Returns `true` if this device is our own virtual forwarder.
    ///
    /// Used to prevent feedback loops: the forwarder creates a virtual keyboard
    /// device, and without this check we'd discover and grab our own output device.
    fn is_virtual_forwarder(&self) -> bool;

    /// Returns `true` if this device looks like a keyboard (supports A-Z + Enter).
    fn is_keyboard(&self) -> bool;

    /// Reads pending events and converts them to domain key events.
    fn read_key_events(&mut self) -> io::Result<Vec<ObservedKeyEvent>>;
}

impl DeviceExt for Device {
    fn is_virtual_forwarder(&self) -> bool {
        self.name().is_some_and(|name| name == VIRTUAL_DEVICE_NAME)
    }

    fn is_keyboard(&self) -> bool {
        self.supported_keys().is_some_and(|supported_keys| {
            supported_keys.contains(KeyCode::KEY_A)
                && supported_keys.contains(KeyCode::KEY_Z)
                && supported_keys.contains(KeyCode::KEY_ENTER)
        })
    }

    fn read_key_events(&mut self) -> io::Result<Vec<ObservedKeyEvent>> {
        let mut events = Vec::new();

        for event in self.fetch_events()? {
            if let Some(observed) = ObservedKeyEvent::from_input_event(event) {
                events.push(observed);
            }
        }

        Ok(events)
    }
}

pub(crate) fn discover_devices_in_dir_with<F>(
    input_dir: &Path,
    mut classify: F,
) -> io::Result<Vec<PathBuf>>
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
struct ObservedKeyEvent {
    key: Key,
    transition: KeyTransition,
}

impl ObservedKeyEvent {
    fn from_input_event(event: InputEvent) -> Option<Self> {
        match event.destructure() {
            EventSummary::Key(_, key_code, value) => {
                let transition = key_transition(value)?;
                let key = key_code.to_key();
                if key == Key::Unknown {
                    return None;
                }
                Some(Self { key, transition })
            }
            _ => None,
        }
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

impl HotplugFsEvent {
    pub fn classify_change(
        &self,
        known_paths: &mut HashSet<PathBuf>,
        input_dir: &Path,
    ) -> HotplugPathChange {
        if !self.device_name.starts_with("event") {
            return HotplugPathChange::Unchanged;
        }

        let path = input_dir.join(&self.device_name);

        if self.mask & (libc::IN_CREATE | libc::IN_MOVED_TO) != 0
            && known_paths.insert(path.clone())
        {
            return HotplugPathChange::Added(path);
        }

        if self.mask
            & (libc::IN_DELETE | libc::IN_MOVED_FROM | libc::IN_DELETE_SELF | libc::IN_MOVE_SELF)
            != 0
        {
            known_paths.remove(&path);
            return HotplugPathChange::Removed(path);
        }

        HotplugPathChange::Unchanged
    }
}

#[must_use]
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
    let name_end = name_bytes
        .iter()
        .position(|&byte| byte == 0)
        .unwrap_or(name_bytes.len());

    String::from_utf8_lossy(&name_bytes[..name_end]).into_owned()
}

#[cfg(test)]
mod tests {
    use std::path::Path;
    use std::time::SystemTime;
    use std::time::UNIX_EPOCH;

    use evdev::EventType;
    use evdev::InputEvent;
    use evdev::KeyCode;
    use kbd_core::Key;
    use kbd_core::key_state::KeyTransition;

    use super::DiscoveryOutcome;
    use super::HotplugFsEvent;
    use super::HotplugPathChange;
    use super::ObservedKeyEvent;
    use super::discover_devices_in_dir_with;
    use super::parse_hotplug_events;
    use crate::forwarder::VIRTUAL_DEVICE_NAME;

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
        let added = add_event.classify_change(&mut known_paths, input_dir);
        assert_eq!(added, HotplugPathChange::Added(input_dir.join("event7")));

        let remove_event = HotplugFsEvent {
            mask: libc::IN_DELETE,
            device_name: "event7".into(),
        };
        let removed = remove_event.classify_change(&mut known_paths, input_dir);
        assert_eq!(
            removed,
            HotplugPathChange::Removed(input_dir.join("event7"))
        );

        let ignored = HotplugFsEvent {
            mask: libc::IN_CREATE,
            device_name: "js0".into(),
        }
        .classify_change(&mut known_paths, input_dir);
        assert_eq!(ignored, HotplugPathChange::Unchanged);
    }

    #[test]
    fn virtual_forwarder_name_is_detected() {
        let is_forwarder = |name: &str| name == VIRTUAL_DEVICE_NAME;

        assert!(is_forwarder(VIRTUAL_DEVICE_NAME));
        assert!(!is_forwarder("AT Translated Set 2 keyboard"));
        assert!(!is_forwarder("Logitech USB Keyboard"));
        assert!(!is_forwarder(""));
    }

    #[test]
    fn key_input_events_are_converted_to_domain_keys() {
        let press = ObservedKeyEvent::from_input_event(InputEvent::new(
            EventType::KEY.0,
            KeyCode::KEY_C.0,
            1,
        ));
        assert_eq!(
            press,
            Some(ObservedKeyEvent {
                key: Key::C,
                transition: KeyTransition::Press,
            })
        );

        let release = ObservedKeyEvent::from_input_event(InputEvent::new(
            EventType::KEY.0,
            KeyCode::KEY_C.0,
            0,
        ));
        assert_eq!(
            release,
            Some(ObservedKeyEvent {
                key: Key::C,
                transition: KeyTransition::Release,
            })
        );

        let repeat = ObservedKeyEvent::from_input_event(InputEvent::new(
            EventType::KEY.0,
            KeyCode::KEY_C.0,
            2,
        ));
        assert_eq!(
            repeat,
            Some(ObservedKeyEvent {
                key: Key::C,
                transition: KeyTransition::Repeat,
            })
        );

        let ignored = ObservedKeyEvent::from_input_event(InputEvent::new(
            EventType::KEY.0,
            KeyCode::new(1023).0,
            1,
        ));
        assert_eq!(ignored, None);
    }

    fn unique_test_dir() -> std::path::PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos();

        std::env::temp_dir().join(format!(
            "kbd-discovery-test-{}-{}",
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
