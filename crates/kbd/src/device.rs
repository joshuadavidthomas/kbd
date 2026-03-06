//! Input device identification and filtering.
//!
//! [`DeviceInfo`] holds platform-agnostic metadata extracted at device
//! open time. [`DeviceFilter`] restricts bindings to specific devices
//! based on that metadata.

/// Metadata for an input device.
///
/// Platform-agnostic — extracted at device open time and used for
/// [`DeviceFilter`] matching. The device name, USB vendor ID, and USB
/// product ID are the three attributes that identify most devices.
///
/// # Examples
///
/// ```
/// use kbd::device::DeviceInfo;
///
/// let info = DeviceInfo::new("Elgato StreamDeck XL", 0x0fd9, 0x006c);
/// assert_eq!(info.name(), "Elgato StreamDeck XL");
/// assert_eq!(info.vendor(), 0x0fd9);
/// assert_eq!(info.product(), 0x006c);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DeviceInfo {
    name: Box<str>,
    vendor: u16,
    product: u16,
}

impl DeviceInfo {
    /// Create device info from name, vendor ID, and product ID.
    #[must_use]
    pub fn new(name: impl Into<Box<str>>, vendor: u16, product: u16) -> Self {
        Self {
            name: name.into(),
            vendor,
            product,
        }
    }

    /// The device name as reported by the kernel.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// USB vendor ID.
    #[must_use]
    pub const fn vendor(&self) -> u16 {
        self.vendor
    }

    /// USB product ID.
    #[must_use]
    pub const fn product(&self) -> u16 {
        self.product
    }
}

/// Filter for restricting bindings to specific input devices.
///
/// Attach to a binding via [`BindingOptions::with_device`](crate::binding::BindingOptions::with_device)
/// to make it fire only when the key event comes from a matching device.
///
/// When a binding has a device filter, modifier isolation applies:
/// only modifiers held on the same device count toward matching.
/// Global bindings (no device filter) continue to use aggregate
/// modifier state across all devices.
///
/// # Examples
///
/// ```
/// use kbd::device::{DeviceFilter, DeviceInfo};
///
/// // Match by device name substring
/// let by_name = DeviceFilter::name_contains("StreamDeck");
///
/// // Match by USB vendor/product ID
/// let by_id = DeviceFilter::usb(0x0fd9, 0x006c);
///
/// // Check matching
/// let info = DeviceInfo::new("Elgato StreamDeck XL", 0x0fd9, 0x006c);
/// assert!(DeviceFilter::name_contains("StreamDeck").matches(&info));
/// assert!(DeviceFilter::usb(0x0fd9, 0x006c).matches(&info));
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub enum DeviceFilter {
    /// Match devices whose name contains the given substring.
    NameContains(Box<str>),
    /// Match devices by USB vendor and product ID.
    UsbId {
        /// USB vendor ID.
        vendor: u16,
        /// USB product ID.
        product: u16,
    },
}

impl DeviceFilter {
    /// Create a filter that matches devices whose name contains the given pattern.
    #[must_use]
    pub fn name_contains(pattern: impl Into<Box<str>>) -> Self {
        Self::NameContains(pattern.into())
    }

    /// Create a filter that matches devices by USB vendor and product ID.
    #[must_use]
    pub fn usb(vendor: u16, product: u16) -> Self {
        Self::UsbId { vendor, product }
    }

    /// Check whether a device matches this filter.
    #[must_use]
    pub fn matches(&self, info: &DeviceInfo) -> bool {
        match self {
            Self::NameContains(pattern) => info.name().contains(pattern.as_ref()),
            Self::UsbId { vendor, product } => {
                info.vendor() == *vendor && info.product() == *product
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn device_filter_name_contains_matches_substring() {
        let filter = DeviceFilter::name_contains("StreamDeck");
        let info = DeviceInfo::new("Elgato StreamDeck XL", 0x0fd9, 0x006c);
        assert!(filter.matches(&info));
    }

    #[test]
    fn device_filter_name_contains_rejects_non_matching() {
        let filter = DeviceFilter::name_contains("StreamDeck");
        let info = DeviceInfo::new("AT Translated Set 2 keyboard", 0x0001, 0x0001);
        assert!(!filter.matches(&info));
    }

    #[test]
    fn device_filter_usb_id_matches_exact() {
        let filter = DeviceFilter::usb(0x1234, 0x5678);
        let info = DeviceInfo::new("Some Device", 0x1234, 0x5678);
        assert!(filter.matches(&info));
    }

    #[test]
    fn device_filter_usb_id_rejects_wrong_vendor() {
        let filter = DeviceFilter::usb(0x1234, 0x5678);
        let info = DeviceInfo::new("Some Device", 0xFFFF, 0x5678);
        assert!(!filter.matches(&info));
    }

    #[test]
    fn device_filter_usb_id_rejects_wrong_product() {
        let filter = DeviceFilter::usb(0x1234, 0x5678);
        let info = DeviceInfo::new("Some Device", 0x1234, 0xFFFF);
        assert!(!filter.matches(&info));
    }
}
