use std::path::PathBuf;

use evdev::Device;
use evdev::KeyCode;

use crate::error::Error;

/// Filter for restricting hotkeys to specific input devices.
///
/// Pass to [`HotkeyOptions::device`](crate::HotkeyOptions::device) to bind a
/// hotkey to a particular keyboard. Only supported on the evdev backend.
///
/// # Examples
///
/// ```
/// use keybound::DeviceFilter;
///
/// // Match by device name substring
/// let by_name = DeviceFilter::name_contains("StreamDeck");
///
/// // Match by USB vendor/product ID
/// let by_id = DeviceFilter::usb(0x0fd9, 0x006c);
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DeviceFilter {
    /// Match devices whose name contains the given substring.
    NameContains(String),
    /// Match devices by USB vendor and product ID.
    UsbId { vendor: u16, product: u16 },
}

impl DeviceFilter {
    /// Create a filter that matches devices whose name contains the given pattern.
    pub fn name_contains(pattern: impl Into<String>) -> Self {
        DeviceFilter::NameContains(pattern.into())
    }

    /// Create a filter that matches devices by USB vendor and product ID.
    #[must_use]
    pub fn usb(vendor: u16, product: u16) -> Self {
        DeviceFilter::UsbId { vendor, product }
    }

    pub(crate) fn matches(&self, info: &DeviceInfo) -> bool {
        match self {
            DeviceFilter::NameContains(pattern) => info.name.contains(pattern.as_str()),
            DeviceFilter::UsbId { vendor, product } => {
                info.vendor == *vendor && info.product == *product
            }
        }
    }
}

/// Internal device identification extracted at open time.
#[derive(Clone, Debug)]
pub(crate) struct DeviceInfo {
    pub(crate) name: String,
    pub(crate) vendor: u16,
    pub(crate) product: u16,
}

impl DeviceInfo {
    pub(crate) fn from_device(device: &Device) -> Self {
        let input_id = device.input_id();
        Self {
            name: device.name().unwrap_or("").to_string(),
            vendor: input_id.vendor(),
            product: input_id.product(),
        }
    }
}

pub(crate) fn is_keyboard_device(device: &Device) -> bool {
    device.supported_keys().is_some_and(|keys| {
        keys.contains(KeyCode::KEY_A)
            && keys.contains(KeyCode::KEY_Z)
            && keys.contains(KeyCode::KEY_ENTER)
    })
}

pub(crate) fn find_keyboard_devices() -> Result<Vec<PathBuf>, Error> {
    let input_dir = std::fs::read_dir("/dev/input")
        .map_err(|e| Error::DeviceAccess(format!("Cannot open /dev/input: {e}")))?;

    let mut keyboards = Vec::new();
    let mut event_device_count = 0usize;
    let mut permission_denied_count = 0usize;

    for entry in input_dir {
        let entry = entry.map_err(|e| Error::DeviceAccess(format!("Failed to read entry: {e}")))?;
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
            return Err(Error::PermissionDenied(permission_error_message()));
        }
        return Err(Error::NoKeyboardsFound);
    }

    Ok(keyboards)
}

fn permission_error_message() -> String {
    let username = std::env::var("USER").unwrap_or_else(|_| "<username>".into());

    format!(
        "The evdev backend requires access to /dev/input/event* devices. If access is denied, try:\n\
         sudo usermod -aG input {username}\n\
         Then log out and log back in for changes to take effect."
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_device_info(name: &str, vendor: u16, product: u16) -> DeviceInfo {
        DeviceInfo {
            name: name.to_string(),
            vendor,
            product,
        }
    }

    #[test]
    fn name_contains_matches_substring() {
        let filter = DeviceFilter::name_contains("StreamDeck");
        let info = test_device_info("Elgato StreamDeck XL", 0x0fd9, 0x006c);
        assert!(filter.matches(&info));
    }

    #[test]
    fn name_contains_rejects_non_matching() {
        let filter = DeviceFilter::name_contains("StreamDeck");
        let info = test_device_info("AT Translated Set 2 keyboard", 0x0001, 0x0001);
        assert!(!filter.matches(&info));
    }

    #[test]
    fn name_contains_is_case_sensitive() {
        let filter = DeviceFilter::name_contains("streamdeck");
        let info = test_device_info("StreamDeck", 0x0001, 0x0001);
        assert!(!filter.matches(&info));
    }

    #[test]
    fn usb_id_matches_exact_vendor_product() {
        let filter = DeviceFilter::usb(0x1234, 0x5678);
        let info = test_device_info("Some Device", 0x1234, 0x5678);
        assert!(filter.matches(&info));
    }

    #[test]
    fn usb_id_rejects_wrong_vendor() {
        let filter = DeviceFilter::usb(0x1234, 0x5678);
        let info = test_device_info("Some Device", 0xFFFF, 0x5678);
        assert!(!filter.matches(&info));
    }

    #[test]
    fn usb_id_rejects_wrong_product() {
        let filter = DeviceFilter::usb(0x1234, 0x5678);
        let info = test_device_info("Some Device", 0x1234, 0xFFFF);
        assert!(!filter.matches(&info));
    }
}
