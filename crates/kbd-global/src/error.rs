//! Error types for the kbd-global runtime.
//!
//! Extends the core error types from `kbd-core` with platform-specific
//! errors for backend initialization, device access, and permissions.

/// Library-wide error type.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("parse error: {0}")]
    Parse(#[from] kbd_core::key::ParseHotkeyError),
    #[error("hotkey registration conflicts with an existing binding")]
    AlreadyRegistered,
    #[error("failed to initialize the selected backend")]
    BackendInit,
    #[error("selected backend is not available on this system")]
    BackendUnavailable,
    #[error("missing permissions to access input devices")]
    PermissionDenied,
    #[error("input device operation failed")]
    DeviceError,
    #[error("requested feature is unsupported by the selected backend")]
    UnsupportedFeature,
    #[error("hotkey manager is no longer running")]
    ManagerStopped,
    #[error("hotkey engine encountered an internal failure")]
    EngineError,
    #[error("a layer with this name is already defined")]
    LayerAlreadyDefined,
    #[error("no layer with this name has been defined")]
    LayerNotDefined,
    #[error("no active layer to pop")]
    EmptyLayerStack,
}

impl From<kbd_core::Error> for Error {
    fn from(error: kbd_core::Error) -> Self {
        match error {
            kbd_core::Error::Parse(e) => Self::Parse(e),
            kbd_core::Error::AlreadyRegistered => Self::AlreadyRegistered,
            kbd_core::Error::LayerAlreadyDefined => Self::LayerAlreadyDefined,
            kbd_core::Error::LayerNotDefined => Self::LayerNotDefined,
            kbd_core::Error::EmptyLayerStack => Self::EmptyLayerStack,
        }
    }
}

impl From<kbd_evdev::error::Error> for Error {
    fn from(error: kbd_evdev::error::Error) -> Self {
        tracing::warn!(%error, "evdev backend error");
        Self::DeviceError
    }
}
