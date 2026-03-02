#![cfg_attr(docsrs, feature(doc_auto_cfg))]

//! Pure-logic keyboard shortcut engine.
//!
//! `kbd` provides the domain types and matching logic that every keyboard
//! shortcut system needs: key types, modifier tracking, binding matching, layer
//! stacks, and sequence resolution. It has zero platform dependencies and can
//! be embedded in any event loop — winit, GPUI, Smithay, a game loop, or a
//! compositor.
//!
//! # Quick Start
//!
//! Register a hotkey, feed key events, and check for matches:
//!
//! ```
//! use kbd::{Action, Hotkey, Key, KeyTransition, MatchResult, Matcher, Modifier};
//!
//! let mut matcher = Matcher::new();
//!
//! // Register Ctrl+S as a global binding
//! let id = matcher.register(
//!     Hotkey::new(Key::S).modifier(Modifier::Ctrl),
//!     Action::Swallow,
//! ).unwrap();
//!
//! // Simulate a key press
//! let result = matcher.process(
//!     &Hotkey::new(Key::S).modifier(Modifier::Ctrl),
//!     KeyTransition::Press,
//! );
//! assert!(matches!(result, MatchResult::Matched { .. }));
//! ```
//!
//! # Feature Flags
//!
//! | Flag | Default | Effect |
//! |------|---------|--------|
//! | `serde` | off | Enables `serde` dependency (serialization support planned) |
//!
//! # See Also
//!
//! - [`kbd-global`](https://docs.rs/kbd-global) — threaded manager with message passing and handles
//! - [`kbd-evdev`](https://docs.rs/kbd-evdev) — Linux evdev key conversion and device monitoring
//! - Bridge crates: [`kbd-crossterm`](https://docs.rs/kbd-crossterm),
//!   [`kbd-egui`](https://docs.rs/kbd-egui), [`kbd-iced`](https://docs.rs/kbd-iced),
//!   [`kbd-tao`](https://docs.rs/kbd-tao), [`kbd-winit`](https://docs.rs/kbd-winit)

/// What happens when a binding matches — callbacks, key emission, layer control.
pub mod action;
/// Binding types — pattern + action + options, device filtering.
pub mod binding;
/// Error types for parsing, conflicts, and layer operations.
pub mod error;
/// Read-only snapshots of matcher state for UI and debugging.
pub mod introspection;
/// Physical key types, modifiers, hotkeys, and string parsing.
pub mod key;
/// Per-device key press/release tracking and modifier derivation.
pub mod key_state;
/// Named binding groups that stack — oneshot, timeout, swallow modes.
pub mod layer;
/// Synchronous matching engine — feed key events, get match results.
pub mod matcher;

// Actions
/// What happens when a binding matches — callbacks, key emission, or layer control.
pub use crate::action::Action;
/// A layer's unique name, used for push/pop/toggle operations.
pub use crate::action::LayerName;
// Bindings
/// Unique identifier for a registered binding.
pub use crate::binding::BindingId;
/// Per-binding behavioral options (passthrough, description, device filter).
pub use crate::binding::BindingOptions;
/// Device filter expression for restricting a binding to specific input devices.
pub use crate::binding::DeviceFilter;
/// Whether a binding appears in hotkey overlays and help screens.
pub use crate::binding::OverlayVisibility;
/// Whether a matched binding consumes or forwards the original key event.
pub use crate::binding::Passthrough;
/// A binding registered with the engine: hotkey + action + options.
pub use crate::binding::RegisteredBinding;
// Errors
/// Error type for registration, layer, and parse operations.
pub use crate::error::Error;
// Introspection
/// Snapshot of an active layer on the stack.
pub use crate::introspection::ActiveLayerInfo;
/// Snapshot of a single binding with its status and metadata.
pub use crate::introspection::BindingInfo;
/// Where a binding lives — global or within a named layer.
pub use crate::introspection::BindingLocation;
/// A pair of bindings in conflict — one shadows the other.
pub use crate::introspection::ConflictInfo;
/// Whether a binding is currently reachable or shadowed by a higher layer.
pub use crate::introspection::ShadowedStatus;
// Keys
/// A key combined with zero or more modifiers (e.g., `Ctrl+C`).
pub use crate::key::Hotkey;
/// An ordered sequence of hotkeys (e.g., `Ctrl+K, Ctrl+C` for chord sequences).
pub use crate::key::HotkeySequence;
/// A physical key on the keyboard.
pub use crate::key::Key;
/// A modifier key (Ctrl, Shift, Alt, Super).
pub use crate::key::Modifier;
/// Error returned when parsing a hotkey string like `"Ctrl+C"` fails.
pub use crate::key::ParseHotkeyError;
// Key state
/// Whether a key was pressed, released, or repeated.
pub use crate::key_state::KeyTransition;
// Layers
/// A named collection of bindings that can be activated and deactivated.
pub use crate::layer::Layer;
/// Per-layer behavioral options (oneshot, timeout, unmatched key behavior).
pub use crate::layer::LayerOptions;
/// Whether unmatched keys in an active layer fall through or are swallowed.
pub use crate::layer::UnmatchedKeyBehavior;
// Matcher
/// Result of attempting to match a key event against registered bindings.
pub use crate::matcher::MatchResult;
/// The synchronous keyboard shortcut matching engine.
pub use crate::matcher::Matcher;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn core_key_types_exist_and_parse() {
        let hotkey: Hotkey = "Ctrl+C".parse().unwrap();
        assert_eq!(hotkey.key(), Key::C);
        assert_eq!(hotkey.modifiers(), &[Modifier::Ctrl]);
    }

    #[test]
    fn core_action_from_closure() {
        let _action: Action = Action::from(|| println!("test"));
    }

    #[test]
    fn core_binding_id_is_unique() {
        let id1 = BindingId::new();
        let id2 = BindingId::new();
        assert_ne!(id1, id2);
    }

    #[test]
    fn core_layer_builder() {
        let layer = Layer::new("test").bind(Key::H, Action::Swallow).swallow();
        assert_eq!(layer.name().as_str(), "test");
        assert_eq!(layer.options().unmatched(), UnmatchedKeyBehavior::Swallow);
    }

    #[test]
    fn core_error_types() {
        let err = Error::AlreadyRegistered;
        let msg = format!("{err}");
        assert!(!msg.is_empty());
    }

    #[test]
    fn core_key_state_tracks_presses() {
        let mut state = key_state::KeyState::default();
        state.apply_device_event(10, Key::A, key_state::KeyTransition::Press);
        assert!(state.is_pressed(Key::A));
        state.apply_device_event(10, Key::A, key_state::KeyTransition::Release);
        assert!(!state.is_pressed(Key::A));
    }

    #[test]
    fn core_matcher_finds_binding() {
        let mut matcher = Matcher::new();
        let hotkey = Hotkey::new(Key::C).modifier(Modifier::Ctrl);
        matcher.register(hotkey.clone(), Action::Swallow).unwrap();

        let result = matcher.process(&hotkey, key_state::KeyTransition::Press);
        assert!(matches!(result, matcher::MatchResult::Matched { .. }));
    }
}
