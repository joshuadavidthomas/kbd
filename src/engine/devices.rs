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

// TODO: DeviceManager — tracks active devices and their file descriptors
// TODO: discover_devices() — scan /dev/input/ for keyboards
// TODO: process_hotplug() — handle inotify events for add/remove
// TODO: Device info (name, vendor/product ID) for DeviceFilter matching
// TODO: Cleanup key state on device disconnect (no stuck keys)
