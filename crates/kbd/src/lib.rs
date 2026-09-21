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
//! Describe your bindings — as strings or programmatically — and the
//! dispatcher tells you when incoming key events match:
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
//! // Register via string parsing
//! dispatcher.register("Ctrl+S", Action::Suppress)?;
//!
//! // Register programmatically — useful for dynamic or computed bindings
//! dispatcher.register(
//!     Hotkey::new(Key::A).modifier(Modifier::Ctrl).modifier(Modifier::Shift),
//!     Action::Suppress,
//! )?;
//!
//! // process() returns Matched, Pending (partial sequence), or NoMatch
//! let result = dispatcher.process(
//!     Hotkey::new(Key::S).modifier(Modifier::Ctrl),
//!     KeyTransition::Press,
//! );
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
//! use kbd::hotkey::{Hotkey, Modifier};
//! use kbd::key::Key;
//! use kbd::key_state::KeyTransition;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let mut dispatcher = Dispatcher::new();
//!
//! // Register via string — parsed into a HotkeySequence
//! dispatcher.register_sequence("Ctrl+K, Ctrl+C", Action::Suppress)?;
//!
//! // Or build the sequence programmatically
//! let steps = vec![
//!     Hotkey::new(Key::K).modifier(Modifier::Ctrl),
//!     Hotkey::new(Key::D).modifier(Modifier::Ctrl),
//! ];
//! dispatcher.register_sequence(steps, Action::Suppress)?;
//!
//! // First step — dispatcher remembers the partial match
//! let r = dispatcher.process(
//!     Hotkey::new(Key::K).modifier(Modifier::Ctrl),
//!     KeyTransition::Press,
//! );
//! assert!(matches!(r, MatchResult::Pending { .. }));
//!
//! // Second step — sequence completes
//! let r = dispatcher.process(
//!     Hotkey::new(Key::C).modifier(Modifier::Ctrl),
//!     KeyTransition::Press,
//! );
//! assert!(matches!(r, MatchResult::Matched { .. }));
//! # Ok(())
//! # }
//! ```
//!
//! # Physical keys
//!
//! Legacy hotkeys and strings match physical positions, not characters. `Key::A` means
//! "the key in the A position on a QWERTY layout" regardless of whether
//! the user's layout is AZERTY, Dvorak, or Colemak. This is the W3C
//! [`KeyboardEvent.code`](https://www.w3.org/TR/uievents-code/) model.
//!
//! Physical keys are layout-independent and predictable — the same binding
//! works on any layout without knowing which one is active.
//!
//! For layout-sensitive shortcuts, [`observation`] provides exact logical
//! character/named-key patterns and observations carrying either or both
//! identities. [`Dispatcher::process_event`](dispatcher::Dispatcher::process_event)
//! resolves both identities together, returning one action. No logical identity
//! is inferred by the legacy physical APIs.
//!
//! Explicit patterns and mixed-domain sequences support string parsing and serde:
//!
//! ```
//! use kbd::action::Action;
//! use kbd::binding::BindingOptions;
//! use kbd::dispatcher::Dispatcher;
//! use kbd::observation::NamedKey;
//! use kbd::sequence::SequenceOptions;
//! let mut dispatcher = Dispatcher::new();
//! dispatcher.register_pattern(r#"Ctrl+logical:"s""#.parse()?, Action::Suppress, BindingOptions::default())?;
//! dispatcher.register_sequence_pattern(
//!     r#"Ctrl+physical:K, logical:",", logical:Enter"#.parse()?,
//!     Action::Suppress,
//!     SequenceOptions::default().with_abort_key(NamedKey::Escape),
//! )?;
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! Quoted logical values are exact character strings; `logical:"Enter"` differs
//! from named `logical:Enter`. Quotes use JSON escapes and protect `+`/`,` delimiters.
//! New [`BindingPattern`](observation::BindingPattern) and
//! [`BindingSequence`](sequence::BindingSequence) values display with explicit
//! domains; legacy physical serialization is unchanged. Sequence candidates consume
//! at most one step per press. [`cancel_pending_sequence`](dispatcher::Dispatcher::cancel_pending_sequence)
//! discards pending sequence state without resetting layers, tap-hold, or held keys.
//!
//! # Feature flags
//!
//! | Flag | Default | Effect |
//! |------|---------|--------|
//! | `serde` | off | Adds `Serialize` and `Deserialize` to key/hotkey types, binding patterns, and mixed sequences |
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
pub mod observation;
pub mod policy;
pub mod sequence;
pub mod tap_hold;
