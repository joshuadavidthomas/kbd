use std::collections::HashMap;
#[cfg(feature = "portal")]
use std::collections::HashSet;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::sync::Mutex;
use std::thread::JoinHandle;

#[cfg(feature = "portal")]
use evdev::KeyCode;

use crate::device::find_keyboard_devices;
use crate::error::Error;
use crate::key_state::SharedKeyState;
use crate::listener::spawn_listener_thread;
use crate::listener::ListenerConfig;
use crate::listener::ListenerState;
use crate::manager::DeviceHotkeyRegistration;
use crate::manager::DeviceRegistrationId;
use crate::manager::HotkeyKey;
use crate::manager::HotkeyRegistration;
use crate::manager::SequenceId;
use crate::manager::SequenceRegistration;
use crate::mode::ModeRegistry;
use crate::tap_hold::TapHoldRegistration;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Backend {
    Evdev,
    Portal,
}

pub trait HotkeyBackend: Send + Sync {
    fn start_listener(
        &self,
        registrations: Arc<Mutex<HashMap<HotkeyKey, HotkeyRegistration>>>,
        sequence_registrations: Arc<Mutex<HashMap<SequenceId, SequenceRegistration>>>,
        device_registrations: Arc<Mutex<HashMap<DeviceRegistrationId, DeviceHotkeyRegistration>>>,
        tap_hold_registrations: Arc<Mutex<HashMap<evdev::KeyCode, TapHoldRegistration>>>,
        stop_flag: Arc<AtomicBool>,
        key_state: SharedKeyState,
    ) -> Result<JoinHandle<()>, Error>;

    fn register_hotkey(&self, hotkey: &HotkeyKey) -> Result<(), Error>;

    fn unregister_hotkey(&self, hotkey: &HotkeyKey) -> Result<(), Error>;
}

pub(crate) struct EvdevBackend {
    grab: bool,
    mode_registry: ModeRegistry,
}

impl HotkeyBackend for EvdevBackend {
    fn start_listener(
        &self,
        registrations: Arc<Mutex<HashMap<HotkeyKey, HotkeyRegistration>>>,
        sequence_registrations: Arc<Mutex<HashMap<SequenceId, SequenceRegistration>>>,
        device_registrations: Arc<Mutex<HashMap<DeviceRegistrationId, DeviceHotkeyRegistration>>>,
        tap_hold_registrations: Arc<Mutex<HashMap<evdev::KeyCode, TapHoldRegistration>>>,
        stop_flag: Arc<AtomicBool>,
        key_state: SharedKeyState,
    ) -> Result<JoinHandle<()>, Error> {
        let keyboards = find_keyboard_devices()?;
        spawn_listener_thread(
            keyboards,
            ListenerState {
                registrations,
                sequence_registrations,
                device_registrations,
                tap_hold_registrations,
                stop_flag,
                key_state,
                mode_registry: self.mode_registry.clone(),
            },
            ListenerConfig { grab: self.grab },
        )
    }

    fn register_hotkey(&self, _hotkey: &HotkeyKey) -> Result<(), Error> {
        Ok(())
    }

    fn unregister_hotkey(&self, _hotkey: &HotkeyKey) -> Result<(), Error> {
        Ok(())
    }
}

#[cfg(feature = "portal")]
trait PortalClient: Send + Sync {
    fn sync_shortcuts(&self, hotkeys: &[HotkeyKey]) -> Result<(), Error>;
}

#[cfg(feature = "portal")]
struct AshpdPortalClient;

#[cfg(feature = "portal")]
impl PortalClient for AshpdPortalClient {
    fn sync_shortcuts(&self, hotkeys: &[HotkeyKey]) -> Result<(), Error> {
        use ashpd::desktop::global_shortcuts::GlobalShortcuts;
        use ashpd::desktop::global_shortcuts::NewShortcut;

        let shortcuts: Vec<NewShortcut> = hotkeys
            .iter()
            .map(|hotkey| {
                let shortcut_id = shortcut_id(hotkey);
                let trigger = format_portal_trigger(hotkey)?;
                Ok(NewShortcut::new(shortcut_id, format!("Hotkey {trigger}"))
                    .preferred_trigger(Some(trigger.as_str())))
            })
            .collect::<Result<Vec<_>, Error>>()?;

        async_std::task::block_on(async {
            let proxy = GlobalShortcuts::new().await.map_err(|err| {
                Error::BackendInit(format!("Failed creating portal proxy: {err}"))
            })?;

            let session = proxy.create_session().await.map_err(|err| {
                Error::BackendInit(format!("Failed creating portal session: {err}"))
            })?;

            let request = proxy
                .bind_shortcuts(&session, &shortcuts, None)
                .await
                .map_err(|err| {
                    Error::BackendInit(format!("Failed binding portal shortcuts: {err}"))
                })?;

            request
                .response()
                .map_err(|err| Error::BackendInit(format!("Portal bind request failed: {err}")))?;

            Ok(())
        })
    }
}

#[cfg(feature = "portal")]
pub(crate) struct PortalBackend {
    registered: Arc<Mutex<HashSet<HotkeyKey>>>,
    client: Arc<dyn PortalClient>,
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
            registered: Arc::new(Mutex::new(HashSet::new())),
            client: Arc::new(AshpdPortalClient),
        })
    }

    #[cfg(test)]
    fn with_client(client: Arc<dyn PortalClient>) -> Self {
        Self {
            registered: Arc::new(Mutex::new(HashSet::new())),
            client,
        }
    }

    fn sync_registered_shortcuts(&self) -> Result<(), Error> {
        let hotkeys: Vec<HotkeyKey> = self.registered.lock().unwrap().iter().cloned().collect();
        self.client.sync_shortcuts(&hotkeys)
    }
}

#[cfg(feature = "portal")]
impl HotkeyBackend for PortalBackend {
    fn start_listener(
        &self,
        _registrations: Arc<Mutex<HashMap<HotkeyKey, HotkeyRegistration>>>,
        _sequence_registrations: Arc<Mutex<HashMap<SequenceId, SequenceRegistration>>>,
        _device_registrations: Arc<Mutex<HashMap<DeviceRegistrationId, DeviceHotkeyRegistration>>>,
        _tap_hold_registrations: Arc<Mutex<HashMap<evdev::KeyCode, TapHoldRegistration>>>,
        stop_flag: Arc<AtomicBool>,
        _key_state: SharedKeyState,
    ) -> Result<JoinHandle<()>, Error> {
        std::thread::Builder::new()
            .name("portal-listener".to_string())
            .spawn(move || {
                while !stop_flag.load(std::sync::atomic::Ordering::SeqCst) {
                    std::thread::park_timeout(std::time::Duration::from_millis(25));
                }
            })
            .map_err(|e| Error::ThreadSpawn(e.to_string()))
    }

    fn register_hotkey(&self, hotkey: &HotkeyKey) -> Result<(), Error> {
        let inserted = {
            let mut registered = self.registered.lock().unwrap();
            registered.insert(hotkey.clone())
        };

        if let Err(err) = self.sync_registered_shortcuts() {
            if inserted {
                self.registered.lock().unwrap().remove(hotkey);
            }
            return Err(err);
        }

        Ok(())
    }

    fn unregister_hotkey(&self, hotkey: &HotkeyKey) -> Result<(), Error> {
        let removed = {
            let mut registered = self.registered.lock().unwrap();
            registered.remove(hotkey)
        };

        if !removed {
            return Ok(());
        }

        if let Err(err) = self.sync_registered_shortcuts() {
            self.registered.lock().unwrap().insert(hotkey.clone());
            return Err(err);
        }

        Ok(())
    }
}

#[cfg(feature = "portal")]
fn shortcut_id(hotkey: &HotkeyKey) -> String {
    let mut components = Vec::with_capacity(hotkey.1.len() + 1);
    components.extend(hotkey.1.iter().copied().map(key_component));
    components.push(key_component(hotkey.0));
    format!("keybound-{}", components.join("-"))
}

#[cfg(feature = "portal")]
fn key_component(key: KeyCode) -> String {
    format!("{key:?}").to_ascii_lowercase()
}

#[cfg(feature = "portal")]
fn format_portal_trigger(hotkey: &HotkeyKey) -> Result<String, Error> {
    let mut parts = Vec::new();

    for modifier in &hotkey.1 {
        let Some(label) = modifier_label(*modifier) else {
            return Err(Error::BackendInit(format!(
                "Unsupported modifier for portal backend: {modifier:?}"
            )));
        };
        parts.push(label.to_string());
    }

    let Some(key_label) = key_label(hotkey.0) else {
        return Err(Error::BackendInit(format!(
            "Unsupported key for portal backend: {:?}",
            hotkey.0
        )));
    };
    parts.push(key_label.to_string());

    Ok(parts.join("+"))
}

#[cfg(feature = "portal")]
fn modifier_label(key: KeyCode) -> Option<&'static str> {
    match key {
        KeyCode::KEY_LEFTCTRL | KeyCode::KEY_RIGHTCTRL => Some("Ctrl"),
        KeyCode::KEY_LEFTSHIFT | KeyCode::KEY_RIGHTSHIFT => Some("Shift"),
        KeyCode::KEY_LEFTALT | KeyCode::KEY_RIGHTALT => Some("Alt"),
        KeyCode::KEY_LEFTMETA | KeyCode::KEY_RIGHTMETA => Some("Super"),
        _ => None,
    }
}

#[cfg(feature = "portal")]
fn key_label(key: KeyCode) -> Option<&'static str> {
    match key {
        KeyCode::KEY_A => Some("A"),
        KeyCode::KEY_B => Some("B"),
        KeyCode::KEY_C => Some("C"),
        KeyCode::KEY_D => Some("D"),
        KeyCode::KEY_E => Some("E"),
        KeyCode::KEY_F => Some("F"),
        KeyCode::KEY_G => Some("G"),
        KeyCode::KEY_H => Some("H"),
        KeyCode::KEY_I => Some("I"),
        KeyCode::KEY_J => Some("J"),
        KeyCode::KEY_K => Some("K"),
        KeyCode::KEY_L => Some("L"),
        KeyCode::KEY_M => Some("M"),
        KeyCode::KEY_N => Some("N"),
        KeyCode::KEY_O => Some("O"),
        KeyCode::KEY_P => Some("P"),
        KeyCode::KEY_Q => Some("Q"),
        KeyCode::KEY_R => Some("R"),
        KeyCode::KEY_S => Some("S"),
        KeyCode::KEY_T => Some("T"),
        KeyCode::KEY_U => Some("U"),
        KeyCode::KEY_V => Some("V"),
        KeyCode::KEY_W => Some("W"),
        KeyCode::KEY_X => Some("X"),
        KeyCode::KEY_Y => Some("Y"),
        KeyCode::KEY_Z => Some("Z"),
        KeyCode::KEY_0 => Some("0"),
        KeyCode::KEY_1 => Some("1"),
        KeyCode::KEY_2 => Some("2"),
        KeyCode::KEY_3 => Some("3"),
        KeyCode::KEY_4 => Some("4"),
        KeyCode::KEY_5 => Some("5"),
        KeyCode::KEY_6 => Some("6"),
        KeyCode::KEY_7 => Some("7"),
        KeyCode::KEY_8 => Some("8"),
        KeyCode::KEY_9 => Some("9"),
        KeyCode::KEY_ENTER => Some("Enter"),
        KeyCode::KEY_ESC => Some("Escape"),
        KeyCode::KEY_TAB => Some("Tab"),
        KeyCode::KEY_SPACE => Some("Space"),
        KeyCode::KEY_F1 => Some("F1"),
        KeyCode::KEY_F2 => Some("F2"),
        KeyCode::KEY_F3 => Some("F3"),
        KeyCode::KEY_F4 => Some("F4"),
        KeyCode::KEY_F5 => Some("F5"),
        KeyCode::KEY_F6 => Some("F6"),
        KeyCode::KEY_F7 => Some("F7"),
        KeyCode::KEY_F8 => Some("F8"),
        KeyCode::KEY_F9 => Some("F9"),
        KeyCode::KEY_F10 => Some("F10"),
        KeyCode::KEY_F11 => Some("F11"),
        KeyCode::KEY_F12 => Some("F12"),
        _ => None,
    }
}

#[cfg(all(feature = "portal", feature = "evdev"))]
#[allow(clippy::unnecessary_wraps)]
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

#[cfg(all(feature = "portal", not(feature = "evdev")))]
fn resolve_backend_with_probe(
    requested: Option<Backend>,
    portal_available: impl FnOnce() -> bool,
) -> Result<Backend, Error> {
    match requested {
        Some(Backend::Portal) => Ok(Backend::Portal),
        Some(Backend::Evdev) => Err(Error::BackendUnavailable(
            "evdev backend (compile with evdev feature)",
        )),
        None if portal_available() => Ok(Backend::Portal),
        None => Err(Error::BackendUnavailable(
            "no available backend: portal unavailable and evdev feature is disabled",
        )),
    }
}

#[cfg(all(not(feature = "portal"), feature = "evdev"))]
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

#[cfg(all(not(feature = "portal"), not(feature = "evdev")))]
fn resolve_backend_with_probe(
    requested: Option<Backend>,
    _portal_available: impl FnOnce() -> bool,
) -> Result<Backend, Error> {
    match requested {
        Some(Backend::Portal) => Err(Error::BackendUnavailable(
            "portal backend (compile with portal feature)",
        )),
        Some(Backend::Evdev) => Err(Error::BackendUnavailable(
            "evdev backend (compile with evdev feature)",
        )),
        None => Err(Error::BackendUnavailable(
            "no backend is compiled in (enable evdev and/or portal feature)",
        )),
    }
}

pub(crate) fn resolve_backend(requested: Option<Backend>) -> Result<Backend, Error> {
    resolve_backend_with_probe(requested, probe_portal_support)
}

#[cfg(feature = "portal")]
fn probe_portal_support() -> bool {
    probe_portal_support_with_checks(
        probe_portal_name_has_owner,
        probe_global_shortcuts_interface_available,
    )
}

#[cfg(feature = "portal")]
fn probe_portal_support_with_checks(
    owner_check: impl FnOnce() -> Result<bool, String>,
    interface_check: impl FnOnce() -> Result<bool, String>,
) -> bool {
    let has_owner = match owner_check() {
        Ok(has_owner) => has_owner,
        Err(err) => {
            tracing::debug!("portal probe NameHasOwner failed: {err}");
            return false;
        }
    };

    if !has_owner {
        return false;
    }

    match interface_check() {
        Ok(interface_available) => interface_available,
        Err(err) => {
            tracing::debug!("portal probe GlobalShortcuts interface check failed: {err}");
            false
        }
    }
}

#[cfg(feature = "portal")]
fn probe_portal_name_has_owner() -> Result<bool, String> {
    async_std::task::block_on(async {
        let connection = ashpd::zbus::Connection::session()
            .await
            .map_err(|err| format!("Failed opening D-Bus session: {err}"))?;

        let dbus_proxy = ashpd::zbus::Proxy::new(
            &connection,
            "org.freedesktop.DBus",
            "/org/freedesktop/DBus",
            "org.freedesktop.DBus",
        )
        .await
        .map_err(|err| format!("Failed creating D-Bus proxy: {err}"))?;

        dbus_proxy
            .call("NameHasOwner", &("org.freedesktop.portal.Desktop",))
            .await
            .map_err(|err| format!("NameHasOwner failed: {err}"))
    })
}

#[cfg(feature = "portal")]
fn probe_global_shortcuts_interface_available() -> Result<bool, String> {
    async_std::task::block_on(async {
        let connection = ashpd::zbus::Connection::session()
            .await
            .map_err(|err| format!("Failed opening D-Bus session: {err}"))?;

        let properties_proxy = ashpd::zbus::Proxy::new(
            &connection,
            "org.freedesktop.portal.Desktop",
            "/org/freedesktop/portal/desktop",
            "org.freedesktop.DBus.Properties",
        )
        .await
        .map_err(|err| format!("Failed creating Properties proxy: {err}"))?;

        let _: ashpd::zbus::zvariant::OwnedValue = properties_proxy
            .call(
                "Get",
                &("org.freedesktop.portal.GlobalShortcuts", "version"),
            )
            .await
            .map_err(|err| format!("GlobalShortcuts version query failed: {err}"))?;

        Ok(true)
    })
}

#[cfg(not(feature = "portal"))]
fn probe_portal_support() -> bool {
    false
}

#[cfg_attr(not(feature = "evdev"), allow(unused_variables))]
pub(crate) fn build_backend(
    backend: Backend,
    grab: bool,
    mode_registry: ModeRegistry,
) -> Result<Box<dyn HotkeyBackend>, Error> {
    match backend {
        #[cfg(feature = "evdev")]
        Backend::Evdev => Ok(Box::new(EvdevBackend {
            grab,
            mode_registry,
        })),
        #[cfg(not(feature = "evdev"))]
        Backend::Evdev => Err(Error::BackendUnavailable(
            "evdev backend (compile with evdev feature)",
        )),
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
    #[cfg(feature = "evdev")]
    fn explicit_evdev_request_is_respected() {
        assert_eq!(
            resolve_backend_with_probe(Some(Backend::Evdev), || true).unwrap(),
            Backend::Evdev
        );
    }

    #[test]
    #[cfg(not(feature = "evdev"))]
    fn explicit_evdev_request_fails_when_not_compiled() {
        let err = resolve_backend_with_probe(Some(Backend::Evdev), || true).unwrap_err();
        assert!(matches!(err, Error::BackendUnavailable(_)));
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
    #[cfg(feature = "evdev")]
    fn defaults_to_evdev_when_portal_unavailable() {
        assert_eq!(
            resolve_backend_with_probe(None, || false).unwrap(),
            Backend::Evdev
        );
    }

    #[test]
    #[cfg(all(not(feature = "evdev"), feature = "portal"))]
    fn default_selection_fails_when_portal_is_unavailable_and_evdev_is_disabled() {
        let err = resolve_backend_with_probe(None, || false).unwrap_err();
        assert!(matches!(err, Error::BackendUnavailable(_)));
    }

    #[test]
    #[cfg(all(not(feature = "evdev"), not(feature = "portal")))]
    fn default_selection_fails_when_no_backends_are_compiled() {
        let err = resolve_backend_with_probe(None, || false).unwrap_err();
        assert!(matches!(err, Error::BackendUnavailable(_)));
    }

    #[test]
    #[cfg(feature = "portal")]
    fn portal_probe_requires_owner_and_interface() {
        let probe = probe_portal_support_with_checks(|| Ok(true), || Ok(true));
        assert!(probe);
    }

    #[test]
    #[cfg(feature = "portal")]
    fn portal_probe_fails_without_globalshortcuts_interface() {
        let probe = probe_portal_support_with_checks(|| Ok(true), || Err("no interface".into()));
        assert!(!probe);
    }

    #[test]
    #[cfg(feature = "portal")]
    fn portal_trigger_formats_supported_key() {
        let hotkey = (
            KeyCode::KEY_A,
            vec![KeyCode::KEY_LEFTCTRL, KeyCode::KEY_LEFTSHIFT],
        );

        assert_eq!(format_portal_trigger(&hotkey).unwrap(), "Ctrl+Shift+A");
    }

    #[test]
    #[cfg(feature = "portal")]
    fn portal_trigger_rejects_unknown_key() {
        let hotkey = (KeyCode::KEY_VOLUMEUP, vec![KeyCode::KEY_LEFTCTRL]);
        assert!(matches!(
            format_portal_trigger(&hotkey),
            Err(Error::BackendInit(_))
        ));
    }

    #[cfg(feature = "portal")]
    struct FakePortalClient {
        calls: Arc<Mutex<Vec<Vec<HotkeyKey>>>>,
        fail: bool,
    }

    #[cfg(feature = "portal")]
    impl PortalClient for FakePortalClient {
        fn sync_shortcuts(&self, hotkeys: &[HotkeyKey]) -> Result<(), Error> {
            self.calls.lock().unwrap().push(hotkeys.to_vec());
            if self.fail {
                return Err(Error::BackendInit("forced sync failure".to_string()));
            }
            Ok(())
        }
    }

    #[test]
    #[cfg(feature = "portal")]
    fn portal_backend_syncs_registered_shortcuts_on_updates() {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let backend = PortalBackend::with_client(Arc::new(FakePortalClient {
            calls: calls.clone(),
            fail: false,
        }));

        let key_a = (KeyCode::KEY_A, vec![KeyCode::KEY_LEFTCTRL]);
        let key_b = (KeyCode::KEY_B, vec![KeyCode::KEY_LEFTCTRL]);

        backend.register_hotkey(&key_a).unwrap();
        backend.register_hotkey(&key_b).unwrap();
        backend.unregister_hotkey(&key_a).unwrap();

        let calls = calls.lock().unwrap();
        assert_eq!(calls.len(), 3);
        assert_eq!(calls[0], vec![key_a.clone()]);
        assert_eq!(calls[2], vec![key_b.clone()]);
    }

    #[test]
    #[cfg(feature = "portal")]
    fn portal_backend_rolls_back_on_sync_failure() {
        let backend = PortalBackend::with_client(Arc::new(FakePortalClient {
            calls: Arc::new(Mutex::new(Vec::new())),
            fail: true,
        }));

        let key = (KeyCode::KEY_A, vec![KeyCode::KEY_LEFTCTRL]);
        let err = backend.register_hotkey(&key).unwrap_err();

        assert!(matches!(err, Error::BackendInit(_)));
        assert!(backend.registered.lock().unwrap().is_empty());
    }

    #[cfg(feature = "portal")]
    struct FailOnNthSyncClient {
        call_count: Arc<std::sync::atomic::AtomicUsize>,
        fail_on_call: usize,
    }

    #[cfg(feature = "portal")]
    impl PortalClient for FailOnNthSyncClient {
        fn sync_shortcuts(&self, _hotkeys: &[HotkeyKey]) -> Result<(), Error> {
            let call_number = self
                .call_count
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst)
                + 1;
            if call_number == self.fail_on_call {
                return Err(Error::BackendInit("forced sync failure".to_string()));
            }
            Ok(())
        }
    }

    #[test]
    #[cfg(feature = "portal")]
    fn portal_backend_preserves_existing_registration_when_resync_fails() {
        let backend = PortalBackend::with_client(Arc::new(FailOnNthSyncClient {
            call_count: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
            fail_on_call: 2,
        }));

        let key = (KeyCode::KEY_A, vec![KeyCode::KEY_LEFTCTRL]);

        backend.register_hotkey(&key).unwrap();
        let err = backend.register_hotkey(&key).unwrap_err();

        assert!(matches!(err, Error::BackendInit(_)));
        assert!(backend.registered.lock().unwrap().contains(&key));
    }
}
