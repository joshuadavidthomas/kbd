//! Sequence state machine — tracks progress through multi-step patterns.
//!
//! When the first step of a sequence matches, enters pending state with a
//! timeout. Subsequent key events either advance the sequence, complete it,
//! or reset it.
//!

// TODO: SequenceState — tracks all active/pending sequences
// TODO: on_key_event() — advance, complete, or reset pending sequences
// TODO: check_timeouts() — fire standalone hotkeys or reset on timeout
// TODO: BindingGuard overlapping prefixes (key is both standalone and sequence start)
