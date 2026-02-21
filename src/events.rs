//! Async event stream for hotkey notifications.
//!
//! Feature-gated behind `tokio` or `async-std`. Provides a stream of
//! `HotkeyEvent` values for applications that prefer async over callbacks.
//!
//! # Reference
//!
//! Prior art: `archive/v0/src/events.rs`

// TODO: HotkeyEvent enum (Pressed, Released, LayerChanged, SequenceStep)
// TODO: HotkeyEventStream — async Stream impl
// TODO: Integration with engine (engine emits events, stream consumes)
