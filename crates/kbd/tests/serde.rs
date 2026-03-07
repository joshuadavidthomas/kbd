//! Serde round-trip tests for kbd types.
//!
//! These verify that all serde-enabled types serialize and deserialize
//! correctly through JSON. Types with string-based serde (`Key`, `Hotkey`,
//! `HotkeySequence`) use their `Display`/`FromStr` representations.

#![cfg(feature = "serde")]

use kbd::binding::BindingId;
use kbd::binding::BindingOptions;
use kbd::binding::BindingSource;
use kbd::binding::KeyPropagation;
use kbd::binding::OverlayVisibility;
use kbd::device::DeviceFilter;
use kbd::device::DeviceInfo;
use kbd::hotkey::Hotkey;
use kbd::hotkey::HotkeySequence;
use kbd::hotkey::Modifier;
use kbd::key::Key;
use kbd::key_state::KeyTransition;
use kbd::layer::LayerName;
use kbd::layer::LayerOptions;
use kbd::layer::UnmatchedKeys;

fn round_trip<T>(value: &T) -> T
where
    T: serde::Serialize + serde::de::DeserializeOwned + std::fmt::Debug,
{
    let json = serde_json::to_string(value).expect("serialize");
    serde_json::from_str(&json).expect("deserialize")
}

// Key

#[test]
fn key_serializes_as_human_readable_string() {
    let json = serde_json::to_string(&Key::A).unwrap();
    assert_eq!(json, r#""A""#);
}

#[test]
fn key_deserializes_from_human_readable_string() {
    let key: Key = serde_json::from_str(r#""A""#).unwrap();
    assert_eq!(key, Key::A);
}

#[test]
fn key_round_trip() {
    let keys = [
        Key::A,
        Key::ENTER,
        Key::SPACE,
        Key::F12,
        Key::ARROW_UP,
        Key::CONTROL_LEFT,
        Key::ESCAPE,
    ];
    for key in &keys {
        assert_eq!(&round_trip(key), key, "round-trip failed for {key:?}");
    }
}

#[test]
fn key_accepts_aliases() {
    // "Enter" and "Return" both parse to the same key
    let enter: Key = serde_json::from_str(r#""Enter""#).unwrap();
    let ret: Key = serde_json::from_str(r#""Return""#).unwrap();
    assert_eq!(enter, ret);
}

// Modifier

#[test]
fn modifier_round_trip() {
    for modifier in [
        Modifier::Ctrl,
        Modifier::Shift,
        Modifier::Alt,
        Modifier::Super,
    ] {
        assert_eq!(
            round_trip(&modifier),
            modifier,
            "round-trip failed for {modifier:?}"
        );
    }
}

// Hotkey

#[test]
fn hotkey_serializes_as_string() {
    let hotkey = Hotkey::new(Key::A)
        .modifier(Modifier::Ctrl)
        .modifier(Modifier::Shift);
    let json = serde_json::to_string(&hotkey).unwrap();
    assert_eq!(json, r#""Ctrl+Shift+A""#);
}

#[test]
fn hotkey_deserializes_from_string() {
    let hotkey: Hotkey = serde_json::from_str(r#""Ctrl+A""#).unwrap();
    assert_eq!(hotkey.key(), Key::A);
    assert_eq!(hotkey.modifiers(), &[Modifier::Ctrl]);
}

#[test]
fn hotkey_round_trip() {
    let hotkeys = [
        Hotkey::new(Key::A),
        Hotkey::new(Key::S).modifier(Modifier::Ctrl),
        Hotkey::new(Key::F5)
            .modifier(Modifier::Ctrl)
            .modifier(Modifier::Shift),
        Hotkey::new(Key::ESCAPE),
    ];
    for hotkey in &hotkeys {
        assert_eq!(
            &round_trip(hotkey),
            hotkey,
            "round-trip failed for {hotkey}"
        );
    }
}

// HotkeySequence

#[test]
fn hotkey_sequence_serializes_as_string() {
    let seq: HotkeySequence = "Ctrl+K, Ctrl+C".parse().unwrap();
    let json = serde_json::to_string(&seq).unwrap();
    assert_eq!(json, r#""Ctrl+K, Ctrl+C""#);
}

#[test]
fn hotkey_sequence_round_trip() {
    let seq: HotkeySequence = "Ctrl+K, Ctrl+C".parse().unwrap();
    assert_eq!(round_trip(&seq), seq);
}

// LayerName

#[test]
fn layer_name_serializes_as_string() {
    let name = LayerName::new("nav");
    let json = serde_json::to_string(&name).unwrap();
    assert_eq!(json, r#""nav""#);
}

#[test]
fn layer_name_round_trip() {
    let name = LayerName::new("navigation");
    assert_eq!(round_trip(&name), name);
}

// Simple enums

#[test]
fn key_transition_round_trip() {
    for variant in [
        KeyTransition::Press,
        KeyTransition::Release,
        KeyTransition::Repeat,
    ] {
        assert_eq!(round_trip(&variant), variant);
    }
}

#[test]
fn unmatched_keys_round_trip() {
    for variant in [UnmatchedKeys::Fallthrough, UnmatchedKeys::Swallow] {
        assert_eq!(round_trip(&variant), variant);
    }
}

#[test]
fn key_propagation_round_trip() {
    for variant in [KeyPropagation::Stop, KeyPropagation::Continue] {
        assert_eq!(round_trip(&variant), variant);
    }
}

#[test]
fn overlay_visibility_round_trip() {
    for variant in [OverlayVisibility::Visible, OverlayVisibility::Hidden] {
        assert_eq!(round_trip(&variant), variant);
    }
}

// Composite types

#[test]
fn binding_options_round_trip() {
    let opts = BindingOptions::default()
        .with_description("Copy to clipboard")
        .with_source(BindingSource::new("user"))
        .with_propagation(KeyPropagation::Continue)
        .with_overlay_visibility(OverlayVisibility::Hidden);
    assert_eq!(round_trip(&opts), opts);
}

#[test]
fn binding_options_default_round_trip() {
    let opts = BindingOptions::default();
    assert_eq!(round_trip(&opts), opts);
}

#[test]
fn layer_options_round_trip() {
    let opts = LayerOptions::default().with_unmatched(UnmatchedKeys::Swallow);
    assert_eq!(round_trip(&opts), opts);
}

#[test]
fn binding_id_round_trip() {
    let id = BindingId::new();
    assert_eq!(round_trip(&id), id);
}

// DeviceInfo

#[test]
fn device_info_round_trip() {
    let info = DeviceInfo::new("Elgato StreamDeck XL", 0x0fd9, 0x006c);
    assert_eq!(round_trip(&info), info);
}

// DeviceFilter

#[test]
fn device_filter_name_contains_round_trip() {
    let filter = DeviceFilter::name_contains("StreamDeck");
    assert_eq!(round_trip(&filter), filter);
}

#[test]
fn device_filter_usb_id_round_trip() {
    let filter = DeviceFilter::usb(0x0fd9, 0x006c);
    assert_eq!(round_trip(&filter), filter);
}

#[test]
fn binding_options_with_device_round_trip() {
    let opts = BindingOptions::default()
        .with_description("StreamDeck button")
        .with_device(DeviceFilter::name_contains("StreamDeck"));
    assert_eq!(round_trip(&opts), opts);
}

#[test]
fn binding_options_with_usb_device_round_trip() {
    let opts = BindingOptions::default().with_device(DeviceFilter::usb(0x1234, 0x5678));
    assert_eq!(round_trip(&opts), opts);
}
