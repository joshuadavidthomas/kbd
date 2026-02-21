use std::collections::HashSet;
use std::ffi::CStr;
use std::io;
use std::mem::size_of;
use std::path::PathBuf;

use super::device::DeviceState;
use super::device::ModifierTracker;
use super::io::open_device;
use super::io::remove_device_by_path;
use super::state::ListenerConfig;
use super::state::INOTIFY_BUFFER_SIZE;
use crate::error::Error;
use crate::key_state::SharedKeyState;

pub(crate) struct RawFdGuard(i32);

impl RawFdGuard {
    pub(crate) fn new(fd: i32) -> Self {
        Self(fd)
    }

    pub(crate) fn raw_fd(&self) -> i32 {
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
pub(crate) struct HotplugFsEvent {
    pub(crate) mask: u32,
    pub(crate) device_name: String,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum HotplugPathChange {
    Added(PathBuf),
    Removed(PathBuf),
    Unchanged,
}

pub(crate) fn init_inotify_watcher() -> Result<RawFdGuard, Error> {
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

pub(crate) fn classify_hotplug_change(
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

pub(crate) fn process_hotplug_events(
    inotify_fd: i32,
    devices: &mut Vec<DeviceState>,
    modifier_tracker: &mut ModifierTracker,
    key_state: &SharedKeyState,
    config: ListenerConfig,
) {
    let mut buffer = [0u8; INOTIFY_BUFFER_SIZE];

    loop {
        let bytes_read = unsafe {
            libc::read(
                inotify_fd,
                buffer.as_mut_ptr().cast::<libc::c_void>(),
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

        for event in parse_hotplug_events(&buffer, bytes_read.cast_unsigned()) {
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
                    remove_device_by_path(&path, devices, modifier_tracker, key_state);
                }
                HotplugPathChange::Unchanged => {}
            }
        }
    }
}

pub(crate) fn parse_hotplug_events(buffer: &[u8], bytes_read: usize) -> Vec<HotplugFsEvent> {
    let mut events = Vec::new();
    let mut offset = 0usize;

    while offset + size_of::<libc::inotify_event>() <= bytes_read {
        #[allow(clippy::cast_ptr_alignment)] // using read_unaligned below
        let event_ptr = unsafe { buffer.as_ptr().add(offset).cast::<libc::inotify_event>() };
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
                    .map_or_else(
                        || String::from_utf8_lossy(&name_slice[..cstr_end]).to_string(),
                        std::string::ToString::to_string,
                    )
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
