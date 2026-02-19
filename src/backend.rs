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

    fn register_hotkey(&self, _hotkey: &HotkeyKey) -> Result<(), Error> {
        Ok(())
    }

    fn unregister_hotkey(&self, _hotkey: &HotkeyKey) -> Result<(), Error> {
        Ok(())
    }
}

pub(crate) struct EvdevBackend;

#[cfg(feature = "portal")]
pub(crate) struct PortalBackend {
    registered: Arc<Mutex<std::collections::HashSet<HotkeyKey>>>,
}

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
impl PortalBackend {
    fn new() -> Result<Self, Error> {
        if !probe_portal_support() {
            return Err(Error::BackendInit(
                "XDG GlobalShortcuts portal is unavailable".to_string(),
            ));
        }

        Ok(Self {
            registered: Arc::new(Mutex::new(std::collections::HashSet::new())),
        })
    }
}

#[cfg(feature = "portal")]
impl HotkeyBackend for PortalBackend {
    fn start_listener(
        &self,
        _registrations: Arc<Mutex<HashMap<HotkeyKey, HotkeyRegistration>>>,
        stop_flag: Arc<AtomicBool>,
    ) -> Result<JoinHandle<()>, Error> {
        std::thread::Builder::new()
            .name("portal-listener".to_string())
            .spawn(move || {
                while !stop_flag.load(std::sync::atomic::Ordering::SeqCst) {
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }
            })
            .map_err(|e| Error::ThreadSpawn(e.to_string()))
    }

    fn register_hotkey(&self, hotkey: &HotkeyKey) -> Result<(), Error> {
        if !probe_portal_support() {
            return Err(Error::BackendInit(
                "portal became unavailable while registering hotkey".to_string(),
            ));
        }
        self.registered.lock().unwrap().insert(hotkey.clone());
        Ok(())
    }

    fn unregister_hotkey(&self, hotkey: &HotkeyKey) -> Result<(), Error> {
        self.registered.lock().unwrap().remove(hotkey);
        Ok(())
    }
}

#[cfg(feature = "portal")]
fn resolve_backend_with_probe(
    requested: Option<Backend>,
    portal_available: impl FnOnce() -> bool,
) -> Result<Backend, Error> {
    match requested {
        Some(backend) => Ok(backend),
        None if portal_available() => Ok(Backend::Portal),
        None => Ok(Backend::Evdev),
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
    probe_portal_support_with_runner(|cmd, args| {
        let output = std::process::Command::new(cmd)
            .args(args)
            .output()
            .map_err(|err| err.to_string())?;

        if !output.status.success() {
            return Err(String::from_utf8_lossy(&output.stderr).to_string());
        }

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    })
}

#[cfg(feature = "portal")]
fn probe_portal_support_with_runner(run: impl Fn(&str, &[&str]) -> Result<String, String>) -> bool {
    let has_owner_output = match run(
        "dbus-send",
        &[
            "--session",
            "--print-reply",
            "--dest=org.freedesktop.DBus",
            "/org/freedesktop/DBus",
            "org.freedesktop.DBus.NameHasOwner",
            "string:org.freedesktop.portal.Desktop",
        ],
    ) {
        Ok(output) => output,
        Err(err) => {
            tracing::debug!("portal probe NameHasOwner failed: {err}");
            return false;
        }
    };

    if !has_owner_output.contains("boolean true") {
        return false;
    }

    let interface_output = match run(
        "dbus-send",
        &[
            "--session",
            "--print-reply",
            "--dest=org.freedesktop.portal.Desktop",
            "/org/freedesktop/portal/desktop",
            "org.freedesktop.DBus.Properties.Get",
            "string:org.freedesktop.portal.GlobalShortcuts",
            "string:version",
        ],
    ) {
        Ok(output) => output,
        Err(err) => {
            tracing::debug!("portal probe GlobalShortcuts interface check failed: {err}");
            return false;
        }
    };

    interface_output.contains("variant") && interface_output.contains("uint32")
}

#[cfg(not(feature = "portal"))]
fn probe_portal_support() -> bool {
    false
}

pub(crate) fn build_backend(backend: Backend) -> Result<Box<dyn HotkeyBackend>, Error> {
    match backend {
        Backend::Evdev => Ok(Box::new(EvdevBackend)),
        #[cfg(feature = "portal")]
        Backend::Portal => Ok(Box::new(PortalBackend::new()?)),
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
        assert_eq!(
            resolve_backend_with_probe(None, || false).unwrap(),
            Backend::Evdev
        );
    }

    #[test]
    #[cfg(feature = "portal")]
    fn portal_backend_builds_when_probe_succeeds() {
        let backend = PortalBackend::new();
        if probe_portal_support() {
            assert!(backend.is_ok());
        } else {
            assert!(matches!(backend, Err(Error::BackendInit(_))));
        }
    }

    #[test]
    #[cfg(feature = "portal")]
    fn portal_probe_requires_owner_and_interface() {
        let probe = probe_portal_support_with_runner(|cmd, args| match cmd {
            "dbus-send" if args.iter().any(|a| a.contains("NameHasOwner")) => {
                Ok("boolean true".to_string())
            }
            "dbus-send" if args.iter().any(|a| a.contains("Properties.Get")) => {
                Ok("variant       uint32 1".to_string())
            }
            _ => Err("unexpected command".to_string()),
        });

        assert!(probe);
    }

    #[test]
    #[cfg(feature = "portal")]
    fn portal_probe_fails_without_globalshortcuts_interface() {
        let probe = probe_portal_support_with_runner(|cmd, args| match cmd {
            "dbus-send" if args.iter().any(|a| a.contains("NameHasOwner")) => {
                Ok("boolean true".to_string())
            }
            "dbus-send" if args.iter().any(|a| a.contains("Properties.Get")) => {
                Err("no such interface".to_string())
            }
            _ => Err("unexpected command".to_string()),
        });

        assert!(!probe);
    }
}
