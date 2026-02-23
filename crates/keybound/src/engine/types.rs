#[cfg(feature = "evdev")]
use kbd_core::binding::Passthrough;

/// Whether the engine is running in grab mode.
///
/// In grab mode, the engine takes exclusive ownership of input devices
/// and forwards unmatched events through a virtual device. The forwarder
/// is bundled with the enabled state so it's impossible to be in grab
/// mode without a forwarder.
pub(crate) enum GrabState {
    Disabled,
    #[cfg(feature = "evdev")]
    #[cfg_attr(not(feature = "grab"), allow(dead_code))]
    Enabled {
        forwarder: Box<dyn super::forwarder::ForwardSink>,
    },
}

/// Disposition of a key event after engine processing.
///
/// Returned by `process_key_event` to indicate what happened with the
/// event. Used by tests to verify forwarding and consumption behavior.
#[cfg(feature = "evdev")]
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

/// Intermediate result from matching, used for forwarding decisions.
///
/// Layer effects are handled by the `Matcher` — the engine only needs
/// the match/no-match outcome and passthrough setting.
#[cfg(feature = "evdev")]
pub(super) enum MatchOutcome {
    Matched { passthrough: Passthrough },
    Swallowed,
    NoMatch,
    Ignored,
}
