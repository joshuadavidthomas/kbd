//! [`HotkeyManager`] — the public API entry point.
//!
//! Thin. Sends commands to the engine, returns handles. Does not own
//! mutable state — the engine owns everything.
//!
//! # Architecture
//!
//! The manager holds a command channel sender and a wake mechanism.
//! Every public method translates to a `Command` sent to the engine.
//! Operations that can fail (register, `define_layer`) use a reply
//! channel to return `Result` synchronously to the caller.
//!
//! ```text
//! HotkeyManager::register()
//!   → sends Command::Register { id, binding, reply_tx }
//!   → engine processes command, sends Result back on reply_tx
//!   → manager returns Handle or Error to caller
//! ```
//!
//! # Reference
//!
//! Prior art: `archive/v0/src/manager.rs` (3000+ lines mixing API with
//! shared-state management). This file should stay small — if it grows
//! past a few hundred lines, something is wrong.

// TODO: HotkeyManager struct (command sender + wake + join handle)
// TODO: HotkeyManager::new() — auto-detect backend, spawn engine
// TODO: HotkeyManager::builder() — explicit configuration
// TODO: register() — simple hotkey with closure (wraps in Action::Callback)
// TODO: register_sequence() — multi-step hotkey
// TODO: register_tap_hold() — dual-function key
// TODO: define_layer() — register a Layer
// TODO: push_layer() / pop_layer() — layer stack control
// TODO: is_key_pressed() / active_modifiers() — state queries
// TODO: Drop impl — send Shutdown command

/// Placeholder — see module docs.
pub struct HotkeyManager;
