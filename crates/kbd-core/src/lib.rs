//! Pure-logic keyboard shortcut engine.
//!
//! `kbd-core` provides the domain types and matching logic that every keyboard
//! shortcut system needs: key types, modifier tracking, binding matching, layer
//! stacks, and sequence resolution. It has zero platform dependencies and can
//! be embedded in any event loop — winit, GPUI, Smithay, a game loop, or a
//! compositor.
//!
//! # What belongs here
//!
//! - `Key`, `Modifier`, `Hotkey`, `HotkeySequence` — the input vocabulary
//! - `Action`, `Binding`, `BindingOptions`, `BindingId` — what to match and do
//! - `Layer`, `LayerOptions` — named binding groups that stack
//! - `Matcher`, `MatchResult`, `KeyState` — the synchronous matching engine
//! - Core error types (parse, conflict, layer)
//!
//! # What does NOT belong here
//!
//! - evdev types or Linux-specific I/O (`kbd-evdev`)
//! - Portal / D-Bus integration (`kbd-portal`)
//! - Keyboard layout / xkbcommon (`kbd-xkb`)
//! - Threaded manager, message passing, handles (`keybound`)

pub mod action;
pub mod binding;
pub mod error;
pub mod introspection;
pub mod key;
pub mod key_state;
pub mod layer;
pub mod matcher;

#[cfg(feature = "winit")]
mod winit;

pub use crate::action::Action;
pub use crate::action::LayerName;
pub use crate::binding::BindingId;
pub use crate::binding::BindingOptions;
pub use crate::binding::DeviceFilter;
pub use crate::binding::OverlayVisibility;
pub use crate::binding::Passthrough;
pub use crate::binding::RegisteredBinding;
pub use crate::error::Error;
pub use crate::introspection::ActiveLayerInfo;
pub use crate::introspection::BindingInfo;
pub use crate::introspection::BindingLocation;
pub use crate::introspection::ConflictInfo;
pub use crate::introspection::ShadowedStatus;
pub use crate::key::Hotkey;
pub use crate::key::HotkeySequence;
pub use crate::key::Key;
pub use crate::key::Modifier;
pub use crate::key::ParseHotkeyError;
pub use crate::key_state::KeyTransition;
pub use crate::layer::Layer;
pub use crate::layer::LayerOptions;
pub use crate::layer::UnmatchedKeyBehavior;
pub use crate::matcher::MatchResult;
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
