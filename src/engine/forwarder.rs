//! uinput virtual device for event forwarding and emission.
//!
//! In grab mode, unmatched key events are re-emitted through a virtual
//! device so they reach applications normally. Also used for `Action::EmitKey`
//! to produce synthetic key events.
//!
//! # Reference
//!
//! Prior art: `archive/v0/src/listener/forwarding.rs`,
//! `reference/keyd/src/vkbd/uinput.c`
//!
//! Note: keyd creates two virtual devices (keyboard + pointer). For now
//! we only need one (keyboard). Pointer device is a future stretch goal.

// TODO: Forwarder — wraps uinput VirtualDevice
// TODO: forward_key() — re-emit an unmatched key event
// TODO: emit_key() — produce a synthetic key event (for remapping/actions)
// TODO: Self-detection prevention (ignore our own virtual device)
