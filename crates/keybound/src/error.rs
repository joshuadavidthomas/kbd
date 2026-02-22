//! Error types for the keybound library.
//!
//! Single error enum covering all failure modes.

/// Library-wide error type.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("parse error: {0}")]
    Parse(#[from] crate::key::ParseHotkeyError),
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
