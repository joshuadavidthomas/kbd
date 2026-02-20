#![cfg(feature = "serde")]

use evdev_hotkey::{
    ActionId, ActionMap, Backend, Error, HotkeyBinding, HotkeyConfig, HotkeyManager, ModeBindings,
    SequenceBinding,
};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

fn create_evdev_manager_or_skip() -> Option<HotkeyManager> {
    match HotkeyManager::with_backend(Backend::Evdev) {
        Ok(manager) => Some(manager),
        Err(Error::PermissionDenied(_))
        | Err(Error::NoKeyboardsFound)
        | Err(Error::DeviceAccess(_))
        | Err(Error::BackendUnavailable(_))
        | Err(Error::BackendInit(_)) => {
            println!("Skipping test: environment has no usable evdev backend/input devices");
            None
        }
        Err(err) => panic!("Unexpected manager creation error: {err}"),
    }
}

#[test]
fn deserializes_hotkeys_sequences_and_modes_from_toml() {
    let config: HotkeyConfig = toml::from_str(
        r#"
            hotkeys = [
                { hotkey = "Ctrl+Shift+A", action = "launch-terminal" }
            ]

            sequences = [
                { sequence = "Ctrl+K, Ctrl+C", action = "comment-line" }
            ]

            [modes.resize]
            bindings = [
                { hotkey = "H", action = "resize-left" },
                { hotkey = "L", action = "resize-right" }
            ]
        "#,
    )
    .expect("config should deserialize");

    assert_eq!(config.hotkeys().len(), 1);
    assert_eq!(config.hotkeys()[0].hotkey().to_string(), "Ctrl+Shift+A");
    assert_eq!(config.hotkeys()[0].action().as_str(), "launch-terminal");

    assert_eq!(config.sequences().len(), 1);
    assert_eq!(
        config.sequences()[0].sequence().to_string(),
        "Ctrl+K, Ctrl+C"
    );
    assert_eq!(config.sequences()[0].action().as_str(), "comment-line");

    let resize_mode = config
        .modes()
        .get("resize")
        .expect("resize mode should exist");
    assert_eq!(resize_mode.bindings().len(), 2);
    assert_eq!(resize_mode.bindings()[0].hotkey().to_string(), "H");
    assert_eq!(resize_mode.bindings()[0].action().as_str(), "resize-left");
}

#[test]
fn round_trips_configuration_through_json() {
    let config = HotkeyConfig::new(
        vec![HotkeyBinding::new(
            "Ctrl+Shift+A".parse().unwrap(),
            ActionId::new("launch-terminal").unwrap(),
        )],
        vec![SequenceBinding::new(
            "Ctrl+K, Ctrl+C".parse().unwrap(),
            ActionId::new("comment-line").unwrap(),
        )],
        [(
            "resize".to_string(),
            ModeBindings::new(vec![HotkeyBinding::new(
                "H".parse().unwrap(),
                ActionId::new("resize-left").unwrap(),
            )]),
        )]
        .into_iter()
        .collect(),
    );

    let json = serde_json::to_string(&config).expect("config should serialize");
    let reparsed: HotkeyConfig =
        serde_json::from_str(&json).expect("serialized config should deserialize");

    assert_eq!(reparsed, config);
}

#[test]
fn invalid_configuration_reports_actionable_error_messages() {
    let invalid_hotkey = serde_json::from_str::<HotkeyConfig>(
        r#"{
            "hotkeys": [
                { "hotkey": "Ctrl+NotAKey", "action": "launch-terminal" }
            ]
        }"#,
    )
    .expect_err("invalid hotkey should fail deserialization");

    let hotkey_message = invalid_hotkey.to_string();
    assert!(hotkey_message.contains("invalid hotkey"));
    assert!(hotkey_message.contains("Ctrl+NotAKey"));

    let invalid_action = serde_json::from_str::<HotkeyConfig>(
        r#"{
            "hotkeys": [
                { "hotkey": "Ctrl+A", "action": "   " }
            ]
        }"#,
    )
    .expect_err("empty action id should fail deserialization");

    let action_message = invalid_action.to_string();
    assert!(action_message.contains("action id cannot be empty"));
}

#[test]
fn deserialized_definitions_register_without_manual_conversion() {
    let manager = match create_evdev_manager_or_skip() {
        Some(manager) => manager,
        None => return,
    };

    let config: HotkeyConfig = serde_json::from_str(
        r#"{
            "hotkeys": [
                { "hotkey": "Ctrl+Shift+A", "action": "launch-terminal" }
            ],
            "sequences": [
                { "sequence": "Ctrl+K, Ctrl+C", "action": "comment-line" }
            ],
            "modes": {
                "resize": {
                    "bindings": [
                        { "hotkey": "H", "action": "resize-left" }
                    ]
                }
            }
        }"#,
    )
    .expect("config should deserialize");

    let call_count = Arc::new(AtomicUsize::new(0));
    let mut actions = ActionMap::new();

    for action in ["launch-terminal", "comment-line", "resize-left"] {
        let call_count = call_count.clone();
        actions
            .insert(ActionId::new(action).unwrap(), move || {
                call_count.fetch_add(1, Ordering::SeqCst);
            })
            .expect("action should insert");
    }

    let registered = config
        .register(&manager, &actions)
        .expect("config should register");

    assert_eq!(registered.hotkey_handles().len(), 1);
    assert_eq!(registered.sequence_handles().len(), 1);
    assert_eq!(registered.defined_modes(), &["resize".to_string()]);
}
