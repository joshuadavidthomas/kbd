//! Error types for the evdev backend.

/// Errors from the evdev backend.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Failed to create or use the uinput virtual device.
    #[error("uinput virtual device error")]
    Uinput(#[source] std::io::Error),
}
