#![cfg_attr(docsrs, feature(doc_cfg))]

//! Pure-logic hotkey engine.
//!
//! `kbd` provides the domain types and matching logic that every hotkey
//! system needs: key types, modifier tracking, binding matching, layer
//! stacks, and sequence resolution. It has zero platform dependencies and can
//! be embedded in any event loop — winit, GPUI, Smithay, a game loop, or a
//! compositor.
//!
//! # Quick Start
//!
//! Register a hotkey, feed key events, and check for matches:
//!
//! ```
//! use kbd::action::Action;
//! use kbd::dispatcher::{Dispatcher, MatchResult};
//! use kbd::key::{Hotkey, Key, Modifier};
//! use kbd::key_state::KeyTransition;
//!
//! let mut dispatcher = Dispatcher::new();
//!
//! // Register Ctrl+S as a global binding
//! let id = dispatcher.register(
//!     Hotkey::new(Key::S).modifier(Modifier::Ctrl),
//!     Action::Suppress,
//! ).unwrap();
//!
//! // Simulate a key press
//! let result = dispatcher.process(
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
//! - Bridge crates: [`kbd-crossterm`](https://docs.rs/kbd-crossterm),
//!   [`kbd-egui`](https://docs.rs/kbd-egui), [`kbd-iced`](https://docs.rs/kbd-iced),
//!   [`kbd-tao`](https://docs.rs/kbd-tao), [`kbd-winit`](https://docs.rs/kbd-winit)

/// What happens when a binding matches — callbacks, key emission, layer control.
pub mod action;
/// Binding types — pattern + action + options, device filtering.
pub mod binding;
/// Synchronous dispatch engine — feed key events, get match results.
pub mod dispatcher;
/// Error types for parsing, conflicts, and layer operations.
pub mod error;
/// Read-only snapshots of dispatcher state for UI and debugging.
pub mod introspection;
/// Physical key types, modifiers, hotkeys, and string parsing.
pub mod key;
/// Per-device key press/release tracking and modifier derivation.
pub mod key_state;
/// Named binding groups that stack — oneshot, timeout, swallow modes.
pub mod layer;

#[cfg(test)]
mod tests {
    use crate::action::Action;
    use crate::binding::BindingId;
    use crate::dispatcher::Dispatcher;
    use crate::error::Error;
    use crate::key::Hotkey;
    use crate::key::Key;
    use crate::key::Modifier;
    use crate::key_state::KeyTransition;
    use crate::layer::Layer;
    use crate::layer::UnmatchedKeys;

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
        let layer = Layer::new("test").bind(Key::H, Action::Suppress).swallow();
        assert_eq!(layer.name().as_str(), "test");
        assert_eq!(layer.options().unmatched(), UnmatchedKeys::Swallow);
    }

    #[test]
    fn core_error_types() {
        let err = Error::AlreadyRegistered;
        let msg = format!("{err}");
        assert!(!msg.is_empty());
    }

    #[test]
    fn core_key_state_tracks_presses() {
        let mut state = crate::key_state::KeyState::default();
        state.apply_device_event(10, Key::A, KeyTransition::Press);
        assert!(state.is_pressed(Key::A));
        state.apply_device_event(10, Key::A, KeyTransition::Release);
        assert!(!state.is_pressed(Key::A));
    }

    #[test]
    fn core_dispatcher_finds_binding() {
        let mut dispatcher = Dispatcher::new();
        let hotkey = Hotkey::new(Key::C).modifier(Modifier::Ctrl);
        dispatcher
            .register(hotkey.clone(), Action::Suppress)
            .unwrap();

        let result = dispatcher.process(&hotkey, KeyTransition::Press);
        assert!(matches!(
            result,
            crate::dispatcher::MatchResult::Matched { .. }
        ));
    }
}
