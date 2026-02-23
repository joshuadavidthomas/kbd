//! Error types for the evdev backend.

/// Errors from the evdev backend.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Failed to interact with an input device.
    #[error("input device I/O error")]
    DeviceIo(#[from] std::io::Error),

    /// Failed to create or use the uinput virtual device.
    #[error("uinput virtual device error")]
    Uinput(#[source] std::io::Error),
}
