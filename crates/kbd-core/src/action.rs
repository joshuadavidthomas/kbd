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

use std::fmt;

use crate::key::HotkeySequence;
use crate::key::Key;
use crate::key::Modifier;

/// Layer identifier used by layer-control actions.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LayerName(Box<str>);

impl LayerName {
    #[must_use]
    pub fn new(value: impl Into<Box<str>>) -> Self {
        Self(value.into())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for LayerName {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl From<String> for LayerName {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

impl std::fmt::Display for LayerName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Action executed when a binding matches.
pub enum Action {
    /// Execute user callback code.
    Callback(Box<dyn Fn() + Send + Sync + 'static>),
    /// Emit a single key with optional modifiers.
    EmitKey(Key, Vec<Modifier>),
    /// Emit a sequence of hotkeys.
    EmitSequence(HotkeySequence),
    /// Push a named layer onto the stack.
    PushLayer(LayerName),
    /// Pop the active layer.
    PopLayer,
    /// Toggle a named layer on/off.
    ToggleLayer(LayerName),
    /// Consume the triggering event without further action.
    Swallow,
}

impl<F> From<F> for Action
where
    F: Fn() + Send + Sync + 'static,
{
    fn from(value: F) -> Self {
        Self::Callback(Box::new(value))
    }
}

impl fmt::Debug for Action {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Callback(_) => f.write_str("Action::Callback(..)"),
            Self::EmitKey(key, modifiers) => f
                .debug_tuple("Action::EmitKey")
                .field(key)
                .field(modifiers)
                .finish(),
            Self::EmitSequence(sequence) => f
                .debug_tuple("Action::EmitSequence")
                .field(sequence)
                .finish(),
            Self::PushLayer(layer) => f.debug_tuple("Action::PushLayer").field(layer).finish(),
            Self::PopLayer => f.write_str("Action::PopLayer"),
            Self::ToggleLayer(layer) => f.debug_tuple("Action::ToggleLayer").field(layer).finish(),
            Self::Swallow => f.write_str("Action::Swallow"),
        }
    }
}
