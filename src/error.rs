use std::fmt;

use crate::key::Key;
use crate::key::Modifier;

/// Errors returned by [`HotkeyManager`](crate::HotkeyManager) operations.
#[derive(Debug)]
pub enum Error {
    /// The user lacks permission to access input devices.
    ///
    /// Typically means the user is not in the `input` group. The contained
    /// string includes instructions for granting access.
    PermissionDenied(String),
    /// No keyboard devices were found under `/dev/input/`.
    NoKeyboardsFound,
    /// An individual input device could not be opened or read.
    DeviceAccess(String),
    /// The listener thread failed to spawn.
    ThreadSpawn(String),
    /// The requested backend is not compiled in.
    ///
    /// Enable the corresponding feature (`evdev` or `portal`) in `Cargo.toml`.
    BackendUnavailable(&'static str),
    /// Backend initialization failed at runtime (e.g. portal session refused).
    BackendInit(String),
    /// The [`HotkeyManager`](crate::HotkeyManager) has been stopped via
    /// [`unregister_all`](crate::HotkeyManager::unregister_all).
    ManagerStopped,
    /// A hotkey with the same key + modifier combination is already registered.
    AlreadyRegistered { key: Key, modifiers: Vec<Modifier> },
    /// The operation requires a feature that isn't available with the current
    /// backend or build configuration (e.g. grab mode on the portal backend).
    UnsupportedFeature(String),
    /// A key sequence definition is invalid (e.g. fewer than two steps).
    InvalidSequence(String),
    /// A hotkey string could not be parsed.
    InvalidHotkey(String),
    /// A mode with this name has already been defined.
    ModeAlreadyDefined(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::PermissionDenied(msg) => write!(f, "Permission denied: {msg}"),
            Error::NoKeyboardsFound => write!(f, "No keyboard devices found"),
            Error::DeviceAccess(s) => write!(f, "Device access error: {s}"),
            Error::ThreadSpawn(s) => write!(f, "Failed to spawn thread: {s}"),
            Error::BackendUnavailable(backend) => {
                write!(f, "Requested backend is not available: {backend}")
            }
            Error::BackendInit(msg) => write!(f, "Backend initialization failed: {msg}"),
            Error::ManagerStopped => write!(f, "Hotkey manager has been stopped"),
            Error::AlreadyRegistered { key, modifiers } => write!(
                f,
                "Hotkey is already registered: key={key:?}, modifiers={modifiers:?}"
            ),
            Error::UnsupportedFeature(message) => write!(f, "Unsupported feature: {message}"),
            Error::InvalidSequence(message) => write!(f, "Invalid sequence: {message}"),
            Error::InvalidHotkey(message) => write!(f, "Invalid hotkey: {message}"),
            Error::ModeAlreadyDefined(name) => {
                write!(f, "Mode is already defined: {name}")
            }
        }
    }
}

impl std::error::Error for Error {}
