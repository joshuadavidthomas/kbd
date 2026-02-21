use std::collections::HashMap;
use std::collections::HashSet;
use std::io;
use std::os::unix::io::AsRawFd;
use std::path::Path;
use std::path::PathBuf;
use std::thread;
use std::thread::JoinHandle;

use evdev::Device;
use evdev::EventSummary;
use evdev::KeyCode;

use super::device::DeviceState;
use super::device::ModifierTracker;
use super::forwarding::create_key_event_forwarder;
use super::hotplug::init_inotify_watcher;
use super::state::ListenerConfig;
use super::state::ListenerState;
use super::state::POLL_TIMEOUT_MS;
use super::state::VIRTUAL_FORWARDER_DEVICE_NAME;
use crate::device::is_keyboard_device;
use crate::device::DeviceInfo;
use crate::error::Error;
use crate::key_state::SharedKeyState;

pub(crate) fn spawn_listener_thread(
    keyboard_paths: Vec<PathBuf>,
    shared: ListenerState,
    config: ListenerConfig,
) -> Result<JoinHandle<()>, Error> {
    let devices = open_devices(keyboard_paths, config)?;
    let inotify_fd = init_inotify_watcher()?;
    let key_event_forwarder = create_key_event_forwarder(config.grab)?;

    thread::Builder::new()
        .name("keybound-listener".into())
        .spawn(move || {
            super::listener_loop(devices, &inotify_fd, shared, config, key_event_forwarder);
        })
        .map_err(|e| Error::ThreadSpawn(format!("Failed to spawn listener thread: {e}")))
}

pub(crate) fn open_devices(
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

pub(crate) fn should_ignore_device(info: &DeviceInfo, grab: bool) -> bool {
    grab && info.name == VIRTUAL_FORWARDER_DEVICE_NAME
}

pub(crate) fn open_device(path: &Path, grab: bool) -> Result<DeviceState, String> {
    #[allow(unused_mut)]
    let mut device =
        Device::open(path).map_err(|e| format!("Failed to open {}: {e}", path.display()))?;

    if !is_keyboard_device(&device) {
        return Err(format!("Device {} is not a keyboard", path.display()));
    }

    let info = DeviceInfo::from_device(&device);
    if should_ignore_device(&info, grab) {
        return Err(format!(
            "Ignoring internal virtual forwarding device {}",
            path.display()
        ));
    }

    if grab {
        #[cfg(feature = "grab")]
        {
            device.grab().map_err(|e| {
                format!(
                    "Failed to grab {} for exclusive capture: {e}",
                    path.display()
                )
            })?;
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
        pressed_keys: HashSet::new(),
    })
}

fn set_nonblocking(fd: i32, path: &Path) -> Result<(), String> {
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
    if flags == -1 {
        return Err(format!(
            "Failed to get file status flags for {}",
            path.display()
        ));
    }

    if unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } == -1 {
        return Err(format!(
            "Failed to set non-blocking mode for {}",
            path.display()
        ));
    }

    Ok(())
}

pub(crate) fn poll_ready_sources(
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

pub(crate) fn read_key_events(device: &mut Device) -> io::Result<Vec<(KeyCode, i32)>> {
    let mut events = Vec::new();

    for event in device.fetch_events()? {
        if let EventSummary::Key(_, key, value) = event.destructure() {
            events.push((key, value));
        }
    }

    Ok(events)
}

pub(crate) fn should_drop_device(err: &io::Error) -> bool {
    err.raw_os_error() == Some(libc::ENODEV)
        || err.kind() == io::ErrorKind::NotFound
        || err.kind() == io::ErrorKind::UnexpectedEof
}

pub(crate) fn emit_shutdown_tap_hold_releases(
    tap_hold_runtime: &mut crate::tap_hold::TapHoldRuntime,
    key_event_forwarder: &mut Option<Box<dyn super::forwarding::KeyEventForwarder>>,
) {
    for (syn_key, syn_value) in tap_hold_runtime.release_all() {
        if let Some(forwarder) = key_event_forwarder.as_mut() {
            if let Err(err) = forwarder.forward_key_event(syn_key, syn_value) {
                tracing::warn!("Failed emitting tap-hold shutdown synthetic event: {}", err);
            }
        }
    }
}

pub(crate) fn release_pressed_keys(devices: &[DeviceState], key_state: &SharedKeyState) {
    for device in devices {
        key_state.release_keys(device.pressed_keys.iter().copied());
    }
}

pub(crate) fn update_pressed_key_state(
    pressed_keys: &mut HashSet<KeyCode>,
    key_state: &SharedKeyState,
    key: KeyCode,
    value: i32,
) {
    match value {
        1 => {
            if pressed_keys.insert(key) {
                key_state.press(key);
            }
        }
        0 => {
            if pressed_keys.remove(&key) {
                key_state.release(key);
            }
        }
        _ => {}
    }
}

pub(crate) fn remove_device_by_fd(
    fd: i32,
    devices: &mut Vec<DeviceState>,
    modifier_tracker: &mut ModifierTracker,
    key_state: &SharedKeyState,
) {
    if let Some(index) = devices.iter().position(|device| device.fd() == fd) {
        let removed = devices.swap_remove(index);
        key_state.release_keys(removed.pressed_keys.iter().copied());
        modifier_tracker.disconnect(&removed.path);
    }
}

pub(crate) fn remove_device_by_path(
    path: &Path,
    devices: &mut Vec<DeviceState>,
    modifier_tracker: &mut ModifierTracker,
    key_state: &SharedKeyState,
) {
    if let Some(index) = devices.iter().position(|device| device.path == path) {
        let removed = devices.swap_remove(index);
        key_state.release_keys(removed.pressed_keys.iter().copied());
        modifier_tracker.disconnect(&removed.path);
    } else {
        modifier_tracker.disconnect(path);
    }
}
