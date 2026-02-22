use std::time::Duration;
use std::time::Instant;

use crate::action::Action;
use crate::action::LayerName;
use crate::binding::Passthrough;

/// Whether the engine is running in grab mode.
///
/// In grab mode, the engine takes exclusive ownership of input devices
/// and forwards unmatched events through a virtual device. The forwarder
/// is bundled with the enabled state so it's impossible to be in grab
/// mode without a forwarder.
pub(crate) enum GrabState {
    Disabled,
    Enabled {
        forwarder: Box<dyn super::forwarder::ForwardSink>,
    },
}

/// Disposition of a key event after engine processing.
///
/// Returned by `process_key_event` to indicate what happened with the
/// event. Used by tests to verify forwarding and consumption behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum KeyEventDisposition {
    /// Event matched a binding and was consumed (not forwarded).
    MatchedConsumed,
    /// Event matched a binding with passthrough and was forwarded.
    MatchedForwarded,
    /// Event did not match any binding and was forwarded (grab mode).
    UnmatchedForwarded,
    /// Event was not processed (grab mode disabled, or modifier/repeat).
    Ignored,
}

/// Layer stack mutation extracted from a matched action.
///
/// Used to defer layer modifications until after the matcher's borrow
/// on engine state is released.
pub(super) enum LayerEffect {
    None,
    Push(LayerName),
    Pop,
    Toggle(LayerName),
}

impl From<&Action> for LayerEffect {
    fn from(action: &Action) -> Self {
        match action {
            Action::PushLayer(name) => Self::Push(name.clone()),
            Action::PopLayer => Self::Pop,
            Action::ToggleLayer(name) => Self::Toggle(name.clone()),
            Action::Callback(_)
            | Action::EmitKey(..)
            | Action::EmitSequence(..)
            | Action::Swallow => Self::None,
        }
    }
}

/// Intermediate result from Phase 1 (matching) used in Phase 2 (execution).
pub(super) enum MatchOutcome {
    Matched {
        layer_effect: LayerEffect,
        passthrough: Passthrough,
    },
    Swallowed,
    NoMatch,
    Ignored,
}

/// An entry in the layer stack, pairing the layer name with runtime state.
pub(crate) struct LayerStackEntry {
    pub(super) name: LayerName,
    /// Remaining keypress count for oneshot layers. `None` means not oneshot.
    pub(super) oneshot_remaining: Option<usize>,
    /// Timeout configuration and last activity timestamp.
    /// If set, the layer auto-pops when `Instant::now() - last_activity > timeout`.
    pub(super) timeout: Option<LayerTimeout>,
}

pub(super) struct LayerTimeout {
    pub(super) duration: Duration,
    pub(super) last_activity: Instant,
}
