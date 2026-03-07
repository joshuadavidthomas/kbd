//! Shared engine types — grab state, key event dispositions, and match outcomes.
//!
//! Internal types used by the engine to track grab mode and classify how
//! each key event was handled.

use std::time::Instant;

use kbd::binding::KeyPropagation;
use kbd::binding::RepeatPolicy;

/// Whether the engine is running in grab mode.
///
/// In grab mode, the engine takes exclusive ownership of input devices
/// and forwards unmatched events through a virtual device. The forwarder
/// is bundled with the enabled state so it's impossible to be in grab
/// mode without a forwarder.
pub(crate) enum GrabState {
    Disabled,
    #[cfg_attr(not(feature = "grab"), allow(dead_code))]
    Enabled {
        forwarder: Box<dyn super::forwarder::ForwardSink>,
    },
}

/// Disposition of a key event after engine processing.
///
/// Returned by `process_key_event` to indicate what happened with the
/// event. Used by tests to verify forwarding and consumption behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum KeyEventOutcome {
    /// Event matched a binding and was consumed (not forwarded).
    MatchedConsumed,
    /// Event matched a binding with propagation and was forwarded.
    MatchedForwarded,
    /// Event did not match any binding and was forwarded (grab mode).
    UnmatchedForwarded,
    /// Event did not match any binding and was not forwarded (non-grab mode).
    Ignored,
}

/// Entry in the press cache, tracking the disposition and repeat policy
/// for a key that was pressed.
///
/// The press cache ensures that release and repeat events use the same
/// disposition as the original press — essential for correctness across
/// layer transitions (keyd's `cache_entry` pattern).
#[derive(Debug, Clone)]
pub(super) struct PressCacheEntry {
    /// The original forwarding disposition from the press event.
    pub(super) outcome: KeyEventOutcome,
    /// Repeat handling info for matched bindings.
    pub(super) repeat_info: Option<RepeatInfo>,
}

/// Repeat handling state for a matched binding in the press cache.
#[derive(Debug, Clone)]
pub(super) struct RepeatInfo {
    /// How repeat events should be handled.
    pub(super) policy: RepeatPolicy,
    /// When the original press occurred (for Custom delay tracking).
    pub(super) press_time: Instant,
    /// When the last repeat action fired (for Custom rate tracking).
    pub(super) last_repeat_fire: Option<Instant>,
}

/// Intermediate result from matching, used for forwarding decisions.
///
/// The `Dispatcher` returns a rich `MatchResult` with six variants, but the
/// engine only cares about three distinctions:
/// - A binding matched and produced an action (with a propagation setting).
/// - The event was consumed without producing a callback (mid-sequence or swallowed).
/// - Nothing matched and the event should be forwarded (in grab mode) or ignored.
pub(super) enum MatchOutcome {
    /// A binding matched. The action has already been executed; only the
    /// propagation setting remains for forwarding decisions.
    Matched { propagation: KeyPropagation },
    /// The event was consumed without firing a callback — either a sequence
    /// is in progress (`Pending`) or a swallow layer suppressed it (`Suppressed`).
    Consumed,
    /// Nothing matched. Forward through the virtual device in grab mode,
    /// or ignore in non-grab mode.
    Unmatched,
}
