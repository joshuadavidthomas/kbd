//! Error types for the evdev backend.
//!
//! Today the crate exposes only uinput-related failures. Device discovery and
//! polling are intentionally best-effort: devices can appear, disappear, or be
//! dropped from the poll set without surfacing a rich typed error, and callers
//! observe that through `DeviceManager` behavior and `PollResult` values.

/// Errors from the evdev backend.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Failed to create or use the uinput virtual device.
    #[error("uinput virtual device error")]
    Uinput(#[source] std::io::Error),
}
