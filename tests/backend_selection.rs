#[cfg(any(not(feature = "portal"), not(feature = "evdev")))]
use evdev_hotkey::Error;
use evdev_hotkey::{Backend, HotkeyManager};

#[test]
#[cfg(not(feature = "portal"))]
fn portal_request_reports_feature_disabled_error() {
    let err = HotkeyManager::detect_backend(Some(Backend::Portal)).unwrap_err();
    assert!(matches!(err, Error::BackendUnavailable(_)));
}

#[test]
#[cfg(not(feature = "portal"))]
fn explicit_portal_manager_request_is_feature_gated() {
    let err = HotkeyManager::with_backend(Backend::Portal).err().unwrap();
    assert!(matches!(err, Error::BackendUnavailable(_)));
}

#[test]
#[cfg(not(feature = "evdev"))]
fn evdev_request_reports_feature_disabled_error() {
    let err = HotkeyManager::detect_backend(Some(Backend::Evdev)).unwrap_err();
    assert!(matches!(err, Error::BackendUnavailable(_)));
}

#[test]
#[cfg(not(feature = "evdev"))]
fn explicit_evdev_manager_request_is_feature_gated() {
    let err = HotkeyManager::with_backend(Backend::Evdev).err().unwrap();
    assert!(matches!(err, Error::BackendUnavailable(_)));
}

#[test]
#[cfg(feature = "portal")]
fn explicit_portal_request_is_respected() {
    let selected = HotkeyManager::detect_backend(Some(Backend::Portal)).unwrap();
    assert_eq!(selected, Backend::Portal);
}

#[test]
#[cfg(all(feature = "portal", feature = "evdev"))]
fn explicit_evdev_request_is_respected() {
    let selected = HotkeyManager::detect_backend(Some(Backend::Evdev)).unwrap();
    assert_eq!(selected, Backend::Evdev);
}

#[test]
#[cfg(all(feature = "portal", feature = "evdev"))]
fn default_backend_detection_returns_a_supported_backend() {
    let selected = HotkeyManager::detect_backend(None).unwrap();
    assert!(matches!(selected, Backend::Portal | Backend::Evdev));
}

#[test]
#[cfg(all(feature = "portal", not(feature = "evdev")))]
fn default_backend_detection_without_evdev_depends_on_portal_availability() {
    match HotkeyManager::detect_backend(None) {
        Ok(selected) => assert_eq!(selected, Backend::Portal),
        Err(err) => assert!(matches!(err, Error::BackendUnavailable(_))),
    }
}
