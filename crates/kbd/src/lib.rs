#![cfg_attr(docsrs, feature(doc_cfg))]

//! Pure-logic hotkey engine.
//!
//! `kbd` provides the domain types and synchronous matching logic behind the
//! rest of the workspace: physical keys, modifiers, bindings, layers,
//! sequences, tap-hold behavior, device-aware matching, and introspection.
//! It has no platform dependencies and can be embedded in any event loop.
//!
//! # Quick start
//!
//! Register a hotkey, feed key events, and inspect the match result:
//!
//! ```
//! use kbd::action::Action;
//! use kbd::dispatcher::{Dispatcher, MatchResult};
//! use kbd::key_state::KeyTransition;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let mut dispatcher = Dispatcher::new();
//!
//! dispatcher.register("Ctrl+S", Action::Suppress)?;
//!
//! let result = dispatcher.process("Ctrl+S".parse()?, KeyTransition::Press);
//! assert!(matches!(result, MatchResult::Matched { .. }));
//! # Ok(())
//! # }
//! ```
//!
//! # What the crate covers
//!
//! - hotkey and sequence types in [`hotkey`]
//! - registration and matching in [`dispatcher`]
//! - stackable named layers in [`layer`]
//! - binding metadata and behavior in [`binding`] and [`policy`]
//! - per-device filtering and modifier isolation in [`device`]
//! - read-only snapshots for overlays and debugging in [`introspection`]
//!
//! # Feature flags
//!
//! | Flag | Default | Effect |
//! |------|---------|--------|
//! | `serde` | off | Adds `Serialize` and `Deserialize` to key and hotkey-related types |
//!
//! # See also
//!
//! - [`kbd-global`](https://docs.rs/kbd-global) — threaded Linux runtime built on this crate
//! - [`kbd-evdev`](https://docs.rs/kbd-evdev) — low-level Linux device backend
//! - bridge crates such as [`kbd-crossterm`](https://docs.rs/kbd-crossterm) and [`kbd-winit`](https://docs.rs/kbd-winit)

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
