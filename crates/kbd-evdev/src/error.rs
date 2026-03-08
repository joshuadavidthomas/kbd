//! Error types for the evdev backend.
//!
//! Today the crate exposes only uinput-related failures because device
//! discovery and polling are intentionally best-effort and are surfaced to
//! callers through runtime behavior rather than rich typed errors.

/// Errors from the evdev backend.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Failed to create or use the uinput virtual device.
    #[error("uinput virtual device error")]
    Uinput(#[source] std::io::Error),
}
