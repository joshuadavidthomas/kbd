use crate::error::Error;
use evdev::{Device, KeyCode};
use std::path::PathBuf;

pub fn find_keyboard_devices() -> Result<Vec<PathBuf>, Error> {
    let input_dir = std::fs::read_dir("/dev/input")
        .map_err(|e| Error::DeviceAccess(format!("Cannot open /dev/input: {}", e)))?;

    let mut keyboards = Vec::new();

    for entry in input_dir {
        let entry =
            entry.map_err(|e| Error::DeviceAccess(format!("Failed to read entry: {}", e)))?;
        let path = entry.path();

        let filename = path.file_name().and_then(|n| n.to_str()).unwrap_or("");

        if !filename.starts_with("event") {
            continue;
        }

        match Device::open(&path) {
            Ok(device) => {
                let supported_keys = device.supported_keys();

                if let Some(keys) = supported_keys {
                    if keys.contains(KeyCode::KEY_A)
                        && keys.contains(KeyCode::KEY_Z)
                        && keys.contains(KeyCode::KEY_ENTER)
                    {
                        let name = device.name().unwrap_or("unknown");
                        tracing::info!("Found keyboard: {} at {:?}", name, path);
                        keyboards.push(path);
                    }
                }
            }
            Err(e) => {
                if e.kind() == std::io::ErrorKind::PermissionDenied {
                    // Silently skip devices we can't access
                    tracing::trace!("Permission denied for {:?}", path);
                } else {
                    tracing::trace!("Cannot open {:?}: {}", path, e);
                }
            }
        }
    }

    if keyboards.is_empty() {
        return Err(Error::NoKeyboardsFound);
    }

    Ok(keyboards)
}
