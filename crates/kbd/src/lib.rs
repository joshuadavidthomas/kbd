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
//! | `serde` | off | Adds `Serialize`/`Deserialize` to key and hotkey types |
//!
//! # See Also
//!
//! - [`kbd-global`](https://docs.rs/kbd-global) — threaded manager with message passing and handles
//! - Bridge crates: [`kbd-crossterm`](https://docs.rs/kbd-crossterm),
//!   [`kbd-egui`](https://docs.rs/kbd-egui), [`kbd-iced`](https://docs.rs/kbd-iced),
//!   [`kbd-tao`](https://docs.rs/kbd-tao), [`kbd-winit`](https://docs.rs/kbd-winit)

/// Actions — what happens when a binding matches (callbacks, key emission,
/// layer control).
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
