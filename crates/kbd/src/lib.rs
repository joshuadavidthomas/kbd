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
//! use kbd::hotkey::{Hotkey, Modifier};
//! use kbd::key::Key;
//! use kbd::key_state::KeyTransition;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let mut dispatcher = Dispatcher::new();
//!
//! // Register Ctrl+S as a global binding
//! let id = dispatcher.register(
//!     Hotkey::new(Key::S).modifier(Modifier::Ctrl),
//!     Action::Suppress,
//! )?;
//!
//! // Simulate a key press
//! let result = dispatcher.process(
//!     Hotkey::new(Key::S).modifier(Modifier::Ctrl),
//!     KeyTransition::Press,
//! );
//! assert!(matches!(result, MatchResult::Matched { .. }));
//! # Ok(())
//! # }
//! ```
//!
//! # Feature Flags
//!
//! | Flag | Default | Effect |
//! |------|---------|--------|
//! | `serde` | off | Adds `Serialize`/`Deserialize` to key and hotkey types |
//!
//! # See Also
//!
//! - [`kbd-global`](https://docs.rs/kbd-global) — threaded manager with message passing and handles
//! - Bridge crates: [`kbd-crossterm`](https://docs.rs/kbd-crossterm),
//!   [`kbd-egui`](https://docs.rs/kbd-egui), [`kbd-iced`](https://docs.rs/kbd-iced),
//!   [`kbd-tao`](https://docs.rs/kbd-tao), [`kbd-winit`](https://docs.rs/kbd-winit)

pub mod action;
pub mod binding;
pub mod device;
pub mod dispatcher;
pub mod error;
pub mod hotkey;
pub mod introspection;
pub mod key;
pub mod key_state;
pub mod layer;
pub mod policy;
pub mod sequence;
pub mod tap_hold;
