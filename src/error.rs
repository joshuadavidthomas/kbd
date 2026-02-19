use std::fmt;

#[derive(Debug)]
pub enum Error {
    PermissionDenied(String),
    NoKeyboardsFound,
    DeviceAccess(String),
    ThreadSpawn(String),
    BackendUnavailable(&'static str),
    BackendInit(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::PermissionDenied(msg) => write!(f, "Permission denied: {}", msg),
            Error::NoKeyboardsFound => write!(f, "No keyboard devices found"),
            Error::DeviceAccess(s) => write!(f, "Device access error: {}", s),
            Error::ThreadSpawn(s) => write!(f, "Failed to spawn thread: {}", s),
            Error::BackendUnavailable(backend) => {
                write!(f, "Requested backend is not available: {}", backend)
            }
            Error::BackendInit(msg) => write!(f, "Backend initialization failed: {}", msg),
        }
    }
}

impl std::error::Error for Error {}
