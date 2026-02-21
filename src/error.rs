//! Error types for the keybound library.
//!
//! Single error enum covering all failure modes.

/// Library-wide error type.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("parse error: {0}")]
    Parse(#[from] crate::key::ParseHotkeyError),
    #[error("hotkey is already registered")]
    AlreadyRegistered,
    #[error("backend initialization failed")]
    BackendInit,
    #[error("requested backend is unavailable")]
    BackendUnavailable,
    #[error("permission denied")]
    PermissionDenied,
    #[error("device error")]
    DeviceError,
    #[error("unsupported feature")]
    UnsupportedFeature,
    #[error("manager has stopped")]
    ManagerStopped,
    #[error("engine error")]
    EngineError,
}
