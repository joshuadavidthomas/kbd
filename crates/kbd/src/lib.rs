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
//! use kbd::prelude::*;
//!
//! # fn main() -> Result<(), kbd::error::Error> {
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
//!     &Hotkey::new(Key::S).modifier(Modifier::Ctrl),
//!     KeyTransition::Press,
//! );
//! assert!(matches!(result, MatchResult::Matched { .. }));
//! # Ok(())
//! # }
//! ```
//!
//! # Sequence progress
//!
//! Multi-step sequences can report progress between keystrokes. After each
//! [`Dispatcher::process`] call, drain any recorded sequence steps with
//! [`Dispatcher::drain_sequence_steps`]:
//!
//! ```
//! use kbd::action::Action;
//! use kbd::dispatcher::{Dispatcher, MatchResult};
//! use kbd::hotkey::{Hotkey, Modifier};
//! use kbd::key::Key;
//! use kbd::key_state::KeyTransition;
//! use kbd::sequence::SequenceStepInfo;
//!
//! # fn main() -> Result<(), kbd::error::Error> {
//! let mut dispatcher = Dispatcher::new();
//! let binding_id = dispatcher.register_sequence("Ctrl+K, Ctrl+C", Action::Suppress)?;
//!
//! let pending = dispatcher.process(
//!     &Hotkey::new(Key::K).modifier(Modifier::Ctrl),
//!     KeyTransition::Press,
//! );
//! assert!(matches!(pending, MatchResult::Pending { .. }));
//! assert_eq!(
//!     dispatcher.drain_sequence_steps(),
//!     vec![SequenceStepInfo {
//!         binding_id,
//!         hotkey: Hotkey::new(Key::K).modifier(Modifier::Ctrl),
//!         steps_matched: 1,
//!         steps_remaining: 1,
//!     }]
//! );
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
pub mod dispatcher;
pub mod error;
pub mod hotkey;
pub mod introspection;
pub mod key;
pub mod key_state;
pub mod layer;
pub mod sequence;

/// Convenience re-exports for common types.
///
/// ```
/// use kbd::prelude::*;
///
/// # fn main() -> Result<(), kbd::error::Error> {
/// let mut dispatcher = Dispatcher::new();
/// dispatcher.register(
///     Hotkey::new(Key::S).modifier(Modifier::Ctrl),
///     Action::Suppress,
/// )?;
///
/// let result = dispatcher.process(
///     &Hotkey::new(Key::S).modifier(Modifier::Ctrl),
///     KeyTransition::Press,
/// );
/// assert!(matches!(result, MatchResult::Matched { .. }));
/// # Ok(())
/// # }
/// ```
pub mod prelude {
    pub use crate::action::Action;
    pub use crate::binding::KeyPropagation;
    pub use crate::binding::OverlayVisibility;
    pub use crate::dispatcher::Dispatcher;
    pub use crate::dispatcher::MatchResult;
    pub use crate::hotkey::Hotkey;
    pub use crate::hotkey::HotkeyInput;
    pub use crate::hotkey::HotkeySequence;
    pub use crate::hotkey::Modifier;
    pub use crate::key::Key;
    pub use crate::key_state::KeyTransition;
    pub use crate::layer::Layer;
    pub use crate::layer::LayerName;
    pub use crate::sequence::PendingSequenceInfo;
    pub use crate::sequence::SequenceInput;
    pub use crate::sequence::SequenceOptions;
}
