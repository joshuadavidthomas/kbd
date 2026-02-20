use crate::error::Error;
use crate::permission::get_permission_error_message;
use evdev::{Device, KeyCode};
use std::path::PathBuf;

pub(crate) fn is_keyboard_device(device: &Device) -> bool {
    device
        .supported_keys()
        .is_some_and(|keys| {
            keys.contains(KeyCode::KEY_A)
                && keys.contains(KeyCode::KEY_Z)
                && keys.contains(KeyCode::KEY_ENTER)
        })
}

pub(crate) fn find_keyboard_devices() -> Result<Vec<PathBuf>, Error> {
    let input_dir = std::fs::read_dir("/dev/input")
        .map_err(|e| Error::DeviceAccess(format!("Cannot open /dev/input: {}", e)))?;

    let mut keyboards = Vec::new();
    let mut event_device_count = 0usize;
    let mut permission_denied_count = 0usize;

    for entry in input_dir {
        let entry =
            entry.map_err(|e| Error::DeviceAccess(format!("Failed to read entry: {}", e)))?;
        let path = entry.path();

        let filename = path.file_name().and_then(|n| n.to_str()).unwrap_or("");

        if !filename.starts_with("event") {
            continue;
        }

        event_device_count += 1;

        match Device::open(&path) {
            Ok(device) => {
                if is_keyboard_device(&device) {
                    let name = device.name().unwrap_or("unknown");
                    tracing::info!("Found keyboard: {} at {:?}", name, path);
                    keyboards.push(path);
                }
            }
            Err(e) => {
                if e.kind() == std::io::ErrorKind::PermissionDenied {
                    permission_denied_count += 1;
                    tracing::trace!("Permission denied for {:?}", path);
                } else {
                    tracing::trace!("Cannot open {:?}: {}", path, e);
                }
            }
        }
    }

    if keyboards.is_empty() {
        if event_device_count > 0 && permission_denied_count == event_device_count {
            return Err(Error::PermissionDenied(get_permission_error_message()));
        }
        return Err(Error::NoKeyboardsFound);
    }

    Ok(keyboards)
}
