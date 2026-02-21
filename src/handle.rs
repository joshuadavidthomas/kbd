//! [`Handle`] — RAII guard that keeps a binding alive.
//!
//! When dropped, sends `Command::Unregister` to the engine. No shared
//! state, no locks — just a binding ID and a command sender.
//!
//! One handle type for all binding kinds (replaces v0's `Handle`,
//! `SequenceHandle`, `TapHoldHandle`).
//!
//! # Reference
//!
//! Prior art: `archive/v0/src/manager/handles.rs`

// TODO: Handle struct (BindingId + CommandSender)
// TODO: Drop impl sends Unregister command
// TODO: unregister() method for explicit removal

/// Placeholder — see module docs.
pub struct Handle;
