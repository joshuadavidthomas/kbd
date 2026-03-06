#![allow(missing_docs)]
use std::error::Error as StdError;

use kbd::hotkey::Hotkey;
use kbd_global::Error;

#[test]
fn error_display_messages_are_actionable() {
    let cases = [
        (
            Error::AlreadyRegistered,
            "hotkey registration conflicts with an existing binding",
        ),
        (
            Error::BackendInit,
            "failed to initialize the selected backend",
        ),
        (
            Error::BackendUnavailable,
            "selected backend is not available on this system",
        ),
        (
            Error::PermissionDenied,
            "missing permissions to access input devices",
        ),
        (Error::DeviceError, "input device operation failed"),
        (
            Error::UnsupportedFeature,
            "requested feature is unsupported by the selected backend",
        ),
        (Error::ManagerStopped, "hotkey manager is no longer running"),
        (
            Error::EngineError,
            "hotkey engine encountered an internal failure",
        ),
        (
            Error::LayerAlreadyDefined,
            "a layer with this name is already defined",
        ),
    ];

    for (error, expected_message) in cases {
        assert_eq!(error.to_string(), expected_message);
    }
}

#[test]
fn parse_hotkey_error_converts_into_library_error_with_source() {
    // Alphabetic tokens are now treated as modifier aliases, so use a non-alphabetic token
    let parse_error = "Ctrl+@@@".parse::<Hotkey>().unwrap_err();
    let error = Error::from(parse_error.clone());

    assert!(matches!(error, Error::Parse(_)));
    assert_eq!(
        error.to_string(),
        "parse error: unknown hotkey token: @@@"
    );

    let source = error.source().expect("parse error should preserve source");
    assert_eq!(source.to_string(), parse_error.to_string());
}
