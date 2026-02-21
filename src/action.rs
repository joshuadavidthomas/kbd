//! The [`Action`] enum — what happens when a binding matches.
//!
//! Actions are the output vocabulary of the library. Every place that
//! currently takes a bare `Fn()` closure should accept an `Action` instead,
//! with a `From` impl so closures auto-convert to `Action::Callback`.
//!
//! Variants that are pure data (everything except `Callback`) should be
//! serializable behind the `serde` feature flag.
//!
//! # Variants
//!
//! - `Callback` — run user code (available now)
//! - `EmitKey` — emit a different key through uinput (future, requires grab)
//! - `EmitSequence` — emit a series of keys (future, requires grab)
//! - `PushLayer` / `PopLayer` / `ToggleLayer` — layer stack control
//! - `Swallow` — explicitly consume the key, do nothing
//!
//! # Reference
//!
//! Prior art: `archive/v0/src/tap_hold.rs` (TapAction/HoldAction),
//! `archive/v0/src/mode/controller.rs` (ModeController push/pop).
//! These are the ad-hoc mechanisms this type unifies.

// TODO: Action enum with variants listed above
// TODO: From<F: Fn() + Send + Sync + 'static> for Action (closure convenience)
// TODO: serde support for data variants behind feature flag

/// Placeholder — see module docs.
pub enum Action {}
