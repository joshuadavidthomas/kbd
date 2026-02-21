//! Async event stream for hotkey notifications.
//!
//! Feature-gated behind `tokio` or `async-std`. Provides a stream of
//! `HotkeyEvent` values for applications that prefer async over callbacks.
//!
//! # Reference
//!
//! Prior art: `archive/v0/src/events.rs`

// TODO: Expand event variants and stream internals as phases progress.

// #[derive(Debug, Clone, PartialEq, Eq)]
// pub enum HotkeyEvent {}

// #[derive(Debug, Clone, Default)]
// pub struct HotkeyEventStream;
