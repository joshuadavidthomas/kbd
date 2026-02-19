use crate::device::find_keyboard_devices;
use crate::error::Error;
use crate::listener::spawn_listener_thread;
use crate::manager::{HotkeyKey, HotkeyRegistration};

use std::collections::HashMap;
use std::sync::{atomic::AtomicBool, Arc, Mutex};
use std::thread::JoinHandle;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Backend {
    Evdev,
    Portal,
}

pub trait HotkeyBackend: Send + Sync {
    fn start_listener(
        &self,
        registrations: Arc<Mutex<HashMap<HotkeyKey, HotkeyRegistration>>>,
        stop_flag: Arc<AtomicBool>,
    ) -> Result<JoinHandle<()>, Error>;
}

pub(crate) struct EvdevBackend;

impl HotkeyBackend for EvdevBackend {
    fn start_listener(
        &self,
        registrations: Arc<Mutex<HashMap<HotkeyKey, HotkeyRegistration>>>,
        stop_flag: Arc<AtomicBool>,
    ) -> Result<JoinHandle<()>, Error> {
        let keyboards = find_keyboard_devices()?;
        spawn_listener_thread(keyboards, registrations, stop_flag)
    }
}

pub(crate) fn resolve_backend(requested: Option<Backend>) -> Result<Backend, Error> {
    match requested {
        Some(Backend::Portal) => Err(Error::BackendUnavailable(
            "portal backend (compile with portal feature)",
        )),
        Some(Backend::Evdev) | None => Ok(Backend::Evdev),
    }
}

pub(crate) fn build_backend(backend: Backend) -> Result<Box<dyn HotkeyBackend>, Error> {
    match backend {
        Backend::Evdev => Ok(Box::new(EvdevBackend)),
        Backend::Portal => Err(Error::BackendUnavailable(
            "portal backend (compile with portal feature)",
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_to_evdev_when_not_requested() {
        assert_eq!(resolve_backend(None).unwrap(), Backend::Evdev);
    }

    #[test]
    fn portal_request_fails_when_not_compiled() {
        let err = resolve_backend(Some(Backend::Portal)).unwrap_err();
        assert!(matches!(err, Error::BackendUnavailable(_)));
    }
}
