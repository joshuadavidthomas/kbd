//! Error types for the kbd-global runtime.
//!
//! Extends the core error types from `kbd` with platform-specific
//! errors for backend initialization, device access, and permissions.

/// Library-wide error type.
///
/// Covers hotkey parsing, registration conflicts, backend initialization,
/// device access, permissions, and layer operations.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// A hotkey string like `"Ctrl+A"` could not be parsed.
    #[error("parse error: {0}")]
    Parse(#[from] kbd::error::ParseHotkeyError),
    /// The hotkey is already bound to another action.
    #[error("hotkey registration conflicts with an existing binding")]
    AlreadyRegistered,
    /// The selected backend could not be initialized.
    #[error("failed to initialize the selected backend")]
    BackendInit,
    /// The selected backend is not available on this system.
    #[error("selected backend is not available on this system")]
    BackendUnavailable,
    /// The current user lacks permissions to access input devices.
    ///
    /// On Linux, add your user to the `input` group:
    /// ```bash
    /// sudo usermod -aG input $USER
    /// ```
    #[error("missing permissions to access input devices")]
    PermissionDenied,
    /// An input device operation failed (open, read, ioctl, etc.).
    #[error("input device operation failed")]
    DeviceError,
    /// The requested feature is not supported by the selected backend.
    ///
    /// Returned when calling [`HotkeyManagerBuilder::grab()`](crate::HotkeyManagerBuilder::grab)
    /// without the `grab` feature enabled.
    #[error("requested feature is unsupported by the selected backend")]
    UnsupportedFeature,
    /// The manager has been shut down or the engine thread has exited.
    #[error("hotkey manager is no longer running")]
    ManagerStopped,
    /// An internal engine failure occurred (thread panic, fd error, etc.).
    #[error("hotkey engine encountered an internal failure")]
    EngineError,
    /// A layer with the given name was already defined.
    #[error("a layer with this name is already defined")]
    LayerAlreadyDefined,
    /// No layer with the given name has been defined.
    #[error("no layer with this name has been defined")]
    LayerNotDefined,
    /// No active layers to pop from the stack.
    #[error("no active layer to pop")]
    EmptyLayerStack,
}

impl From<kbd::error::Error> for Error {
    fn from(error: kbd::error::Error) -> Self {
        match error {
            kbd::error::Error::Parse(e) => Self::Parse(e),
            kbd::error::Error::AlreadyRegistered => Self::AlreadyRegistered,
            kbd::error::Error::LayerAlreadyDefined => Self::LayerAlreadyDefined,
            kbd::error::Error::LayerNotDefined => Self::LayerNotDefined,
            kbd::error::Error::EmptyLayerStack => Self::EmptyLayerStack,
            _ => Self::EngineError,
        }
    }
}

impl From<kbd_evdev::error::Error> for Error {
    fn from(error: kbd_evdev::error::Error) -> Self {
        tracing::warn!(%error, "evdev backend error");
        Self::DeviceError
    }
}
