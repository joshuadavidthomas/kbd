//! Error types for the kbd-global runtime.
//!
//! Errors are scoped to the operation that produces them:
//!
//! - [`StartupError`] — manager construction and engine spawn
//! - [`RegisterError`] — binding registration (hotkeys, sequences, tap-hold)
//! - [`LayerError`] — layer definition and stack operations
//! - [`QueryError`] — hotkey queries that accept parseable input
//! - [`ShutdownError`] — manager and engine shutdown
//! - [`ManagerStopped`] — standalone error for simple queries with no
//!   domain failure modes

/// The manager has been shut down or the engine thread has exited.
///
/// Returned standalone by query methods whose only failure mode is
/// a dead engine, and as a variant inside other scoped error types.
#[derive(Debug, thiserror::Error)]
#[error("hotkey manager is no longer running")]
pub struct ManagerStopped;

/// Error returned when starting the manager or spawning the engine.
///
/// Covers device access, unsupported feature requests (e.g. grab mode
/// without the `grab` feature), and engine thread failures.
#[derive(Debug, thiserror::Error)]
pub enum StartupError {
    /// An input device operation failed (open, read, ioctl, etc.).
    #[error("input device operation failed")]
    Device,
    /// The requested feature is not supported by the selected backend.
    ///
    /// Returned when calling [`HotkeyManagerBuilder::grab()`](crate::HotkeyManagerBuilder::grab)
    /// without the `grab` feature enabled.
    #[error("requested feature is unsupported by the selected backend")]
    UnsupportedFeature,
    /// An internal engine failure occurred (thread panic, fd error, etc.).
    #[error("hotkey engine encountered an internal failure")]
    Engine,
}

impl From<kbd_evdev::error::Error> for StartupError {
    fn from(error: kbd_evdev::error::Error) -> Self {
        tracing::warn!(%error, "evdev backend error");
        Self::Device
    }
}

/// Error returned when registering a binding fails.
///
/// Covers hotkey registration, sequence registration, and tap-hold
/// registration through the manager.
#[derive(Debug, thiserror::Error)]
pub enum RegisterError {
    /// A hotkey string like `"Ctrl+A"` could not be parsed.
    #[error("parse error: {0}")]
    Parse(#[from] kbd::error::ParseHotkeyError),
    /// The hotkey is already bound to another action.
    #[error("hotkey registration conflicts with an existing binding")]
    AlreadyRegistered,
    /// The requested feature is not supported by the selected backend.
    ///
    /// Returned when registering a tap-hold binding without grab mode enabled.
    #[error("requested feature is unsupported by the selected backend")]
    UnsupportedFeature,
    /// The manager has been shut down or the engine thread has exited.
    #[error(transparent)]
    ManagerStopped(#[from] ManagerStopped),
}

impl From<kbd::error::RegisterError> for RegisterError {
    fn from(error: kbd::error::RegisterError) -> Self {
        match error {
            kbd::error::RegisterError::Parse(e) => Self::Parse(e),
            kbd::error::RegisterError::AlreadyRegistered => Self::AlreadyRegistered,
            _ => unreachable!("unknown kbd::error::RegisterError variant"),
        }
    }
}

/// Error returned when a layer operation fails.
///
/// Covers layer definition, push, pop, and toggle operations
/// through the manager.
#[derive(Debug, thiserror::Error)]
pub enum LayerError {
    /// A layer with the given name was already defined.
    #[error("a layer with this name is already defined")]
    AlreadyDefined,
    /// No layer with the given name has been defined.
    #[error("no layer with this name has been defined")]
    NotDefined,
    /// No active layers to pop from the stack.
    #[error("no active layer to pop")]
    EmptyStack,
    /// The manager has been shut down or the engine thread has exited.
    #[error(transparent)]
    ManagerStopped(#[from] ManagerStopped),
}

impl From<kbd::error::LayerError> for LayerError {
    fn from(error: kbd::error::LayerError) -> Self {
        match error {
            kbd::error::LayerError::AlreadyDefined => Self::AlreadyDefined,
            kbd::error::LayerError::NotDefined => Self::NotDefined,
            kbd::error::LayerError::EmptyStack => Self::EmptyStack,
            _ => unreachable!("unknown kbd::error::LayerError variant"),
        }
    }
}

/// Error returned by query methods that accept parseable hotkey input.
///
/// Methods like [`is_registered()`](crate::HotkeyManager::is_registered)
/// and [`bindings_for_key()`](crate::HotkeyManager::bindings_for_key)
/// can fail from string parsing or a dead engine.
#[derive(Debug, thiserror::Error)]
pub enum QueryError {
    /// A hotkey string could not be parsed.
    #[error("parse error: {0}")]
    Parse(#[from] kbd::error::ParseHotkeyError),
    /// The manager has been shut down or the engine thread has exited.
    #[error(transparent)]
    ManagerStopped(#[from] ManagerStopped),
}

/// Error returned when shutting down the manager.
#[derive(Debug, thiserror::Error)]
pub enum ShutdownError {
    /// An internal engine failure occurred (thread panic, mutex poison, etc.).
    #[error("hotkey engine encountered an internal failure")]
    Engine,
    /// The manager has been shut down or the engine thread has exited.
    #[error(transparent)]
    ManagerStopped(#[from] ManagerStopped),
}
