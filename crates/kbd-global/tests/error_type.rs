#![allow(missing_docs)]
use std::error::Error as StdError;

use kbd::hotkey::Hotkey;
use kbd_global::LayerError;
use kbd_global::ManagerStopped;
use kbd_global::RegisterError;
use kbd_global::ShutdownError;
use kbd_global::StartupError;

#[test]
fn startup_error_display_messages() {
    let cases: Vec<(StartupError, &str)> = vec![
        (StartupError::Device, "input device operation failed"),
        (
            StartupError::UnsupportedFeature,
            "requested feature is unsupported by the selected backend",
        ),
        (
            StartupError::Engine,
            "hotkey engine encountered an internal failure",
        ),
    ];

    for (error, expected_message) in cases {
        assert_eq!(error.to_string(), expected_message);
    }
}

#[test]
fn register_error_display_messages() {
    let cases: Vec<(RegisterError, &str)> = vec![
        (
            RegisterError::AlreadyRegistered,
            "hotkey registration conflicts with an existing binding",
        ),
        (
            RegisterError::UnsupportedFeature,
            "requested feature is unsupported by the selected backend",
        ),
        (
            RegisterError::ManagerStopped(ManagerStopped),
            "hotkey manager is no longer running",
        ),
    ];

    for (error, expected_message) in cases {
        assert_eq!(error.to_string(), expected_message);
    }
}

#[test]
fn layer_error_display_messages() {
    let cases: Vec<(LayerError, &str)> = vec![
        (
            LayerError::AlreadyDefined,
            "a layer with this name is already defined",
        ),
        (
            LayerError::NotDefined,
            "no layer with this name has been defined",
        ),
        (LayerError::EmptyStack, "no active layer to pop"),
        (
            LayerError::ManagerStopped(ManagerStopped),
            "hotkey manager is no longer running",
        ),
    ];

    for (error, expected_message) in cases {
        assert_eq!(error.to_string(), expected_message);
    }
}

#[test]
fn shutdown_error_display_messages() {
    let cases: Vec<(ShutdownError, &str)> = vec![
        (
            ShutdownError::Engine,
            "hotkey engine encountered an internal failure",
        ),
        (
            ShutdownError::ManagerStopped(ManagerStopped),
            "hotkey manager is no longer running",
        ),
    ];

    for (error, expected_message) in cases {
        assert_eq!(error.to_string(), expected_message);
    }
}

#[test]
fn manager_stopped_display_message() {
    assert_eq!(
        ManagerStopped.to_string(),
        "hotkey manager is no longer running"
    );
}

#[test]
fn parse_hotkey_error_converts_into_register_error_with_source() {
    let parse_error = "Ctrl+NotARealKey".parse::<Hotkey>().unwrap_err();
    let error = RegisterError::from(parse_error.clone());

    assert!(matches!(error, RegisterError::Parse(_)));
    assert_eq!(
        error.to_string(),
        "parse error: unknown hotkey token: NotARealKey"
    );

    let source = error.source().expect("parse error should preserve source");
    assert_eq!(source.to_string(), parse_error.to_string());
}
