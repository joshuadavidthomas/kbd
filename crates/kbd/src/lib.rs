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
//! dispatcher.register("Ctrl+S", || println!("saved"))?;
//! dispatcher.register("Ctrl+Shift+P", Action::Suppress)?;
//!
//! let result = dispatcher.process("Ctrl+S".parse()?, KeyTransition::Press);
//! assert!(matches!(result, MatchResult::Matched { .. }));
//! # Ok(())
//! # }
//! ```
//!
//! Most integrations revolve around [`dispatcher::Dispatcher`]. Supporting
//! modules cover hotkey parsing, layers, binding metadata and policies,
//! per-device matching, and read-only introspection snapshots.
//!
//! # Layers
//!
//! Layers are named, stackable groups of bindings. When active, a layer's
//! bindings take priority over the layers beneath it. Use them for modes,
//! context-dependent shortcuts, or temporary overrides.
//!
//! ```
//! use kbd::action::Action;
//! use kbd::dispatcher::Dispatcher;
//! use kbd::key::Key;
//! use kbd::layer::Layer;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let mut dispatcher = Dispatcher::new();
//!
//! let layer = Layer::new("vim-normal")
//!     .bind(Key::J, || println!("down"))?
//!     .bind(Key::K, || println!("up"))?;
//!
//! dispatcher.define_layer(layer)?;
//! dispatcher.push_layer("vim-normal")?;
//! # Ok(())
//! # }
//! ```
//!
//! Layers can be oneshot (auto-pop after one match), swallowing (consume
//! unmatched keys), or time-limited. See [`layer`] for the full API.
//!
//! # Sequences
//!
//! Multi-step bindings like `Ctrl+K, Ctrl+C`:
//!
//! ```
//! use kbd::action::Action;
//! use kbd::dispatcher::{Dispatcher, MatchResult};
//! use kbd::key_state::KeyTransition;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let mut dispatcher = Dispatcher::new();
//! dispatcher.register_sequence("Ctrl+K, Ctrl+C", Action::Suppress)?;
//!
//! // First step — dispatcher remembers the partial match
//! let r = dispatcher.process("Ctrl+K".parse()?, KeyTransition::Press);
//! assert!(matches!(r, MatchResult::Pending { .. }));
//!
//! // Second step — sequence completes
//! let r = dispatcher.process("Ctrl+C".parse()?, KeyTransition::Press);
//! assert!(matches!(r, MatchResult::Matched { .. }));
//! # Ok(())
//! # }
//! ```
//!
//! # Physical keys
//!
//! `kbd` matches physical key positions, not characters. `Key::A` means
//! "the key in the A position on a QWERTY layout" regardless of whether
//! the user's layout is AZERTY, Dvorak, or Colemak. This is the W3C
//! [`KeyboardEvent.code`](https://www.w3.org/TR/uievents-code/) model.
//!
//! Physical keys are layout-independent and predictable — the same binding
//! works on any layout without knowing which one is active.
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
