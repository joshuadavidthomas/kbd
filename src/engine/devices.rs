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

use std::os::fd::RawFd;

use crate::engine::key_state::KeyState;

#[derive(Debug, Default)]
pub(crate) struct DeviceManager {
    device_fds: Vec<RawFd>,
}

impl DeviceManager {
    #[must_use]
    pub(crate) fn poll_fds(&self) -> &[RawFd] {
        &self.device_fds
    }

    pub(crate) fn process_polled_events(_: &[libc::pollfd], _: &mut KeyState) {}
}
