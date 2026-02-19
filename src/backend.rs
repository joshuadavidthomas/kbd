use crate::device::find_keyboard_devices;
use crate::error::Error;
use crate::listener::spawn_listener_thread;
use crate::manager::{HotkeyKey, HotkeyRegistration};

use std::collections::HashMap;
use std::sync::{atomic::AtomicBool, Arc, Mutex};
#[cfg(feature = "portal")]
use std::sync::atomic::Ordering;
use std::thread::JoinHandle;
#[cfg(feature = "portal")]
use std::time::Duration;

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

#[cfg(feature = "portal")]
pub(crate) struct PortalBackend;

#[cfg(feature = "portal")]
impl HotkeyBackend for PortalBackend {
    fn start_listener(
        &self,
        _registrations: Arc<Mutex<HashMap<HotkeyKey, HotkeyRegistration>>>,
        stop_flag: Arc<AtomicBool>,
    ) -> Result<JoinHandle<()>, Error> {
        if !probe_portal_support() {
            return Err(Error::BackendInit(
                "portal backend is compiled but GlobalShortcuts is unavailable".to_string(),
            ));
        }

        let listener = std::thread::spawn(move || {
            while !stop_flag.load(Ordering::SeqCst) {
                std::thread::sleep(Duration::from_millis(10));
            }
        });

        Ok(listener)
    }
}

#[cfg(feature = "portal")]
fn resolve_backend_with_probe(
    requested: Option<Backend>,
    portal_available: impl FnOnce() -> bool,
) -> Result<Backend, Error> {
    match requested {
        Some(backend) => Ok(backend),
        None => {
            if portal_available() {
                Ok(Backend::Portal)
            } else {
                Ok(Backend::Evdev)
            }
        }
    }
}

#[cfg(not(feature = "portal"))]
fn resolve_backend_with_probe(
    requested: Option<Backend>,
    _portal_available: impl FnOnce() -> bool,
) -> Result<Backend, Error> {
    match requested {
        Some(Backend::Portal) => Err(Error::BackendUnavailable(
            "portal backend (compile with portal feature)",
        )),
        Some(Backend::Evdev) | None => Ok(Backend::Evdev),
    }
}

pub(crate) fn resolve_backend(requested: Option<Backend>) -> Result<Backend, Error> {
    resolve_backend_with_probe(requested, probe_portal_support)
}

#[cfg(feature = "portal")]
fn probe_portal_support() -> bool {
    false
}


#[cfg(not(feature = "portal"))]
fn probe_portal_support() -> bool {
    false
}

pub(crate) fn build_backend(backend: Backend) -> Result<Box<dyn HotkeyBackend>, Error> {
    match backend {
        Backend::Evdev => Ok(Box::new(EvdevBackend)),
        #[cfg(feature = "portal")]
        Backend::Portal => Ok(Box::new(PortalBackend)),
        #[cfg(not(feature = "portal"))]
        Backend::Portal => Err(Error::BackendUnavailable(
            "portal backend (compile with portal feature)",
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_evdev_request_is_respected() {
        assert_eq!(
            resolve_backend_with_probe(Some(Backend::Evdev), || true).unwrap(),
            Backend::Evdev
        );
    }

    #[test]
    #[cfg(not(feature = "portal"))]
    fn portal_request_fails_when_not_compiled() {
        let err = resolve_backend_with_probe(Some(Backend::Portal), || true).unwrap_err();
        assert!(matches!(err, Error::BackendUnavailable(_)));
    }

    #[test]
    #[cfg(feature = "portal")]
    fn explicit_portal_request_is_respected_when_compiled() {
        assert_eq!(
            resolve_backend_with_probe(Some(Backend::Portal), || false).unwrap(),
            Backend::Portal
        );
    }

    #[test]
    #[cfg(feature = "portal")]
    fn defaults_to_portal_when_available() {
        assert_eq!(
            resolve_backend_with_probe(None, || true).unwrap(),
            Backend::Portal
        );
    }

    #[test]
    fn defaults_to_evdev_when_portal_unavailable() {
        assert_eq!(resolve_backend_with_probe(None, || false).unwrap(), Backend::Evdev);
    }
}
