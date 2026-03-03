//! Tap-hold state machine — dual-function key resolution.
//!
//! When a tap-hold key is pressed, enters pending state. Resolves as:
//! - **Tap**: key released before threshold (execute tap action)
//! - **Hold**: threshold exceeded or another key pressed (execute hold action)
//!
//! # Status
//!
//! Not yet implemented.

// TODO: TapHoldState — tracks pending/resolved tap-hold keys
// TODO: on_key_event() — start pending, resolve on interrupt
// TODO: check_timeouts() — resolve on threshold expiry
// TODO: Generate synthetic events for tap resolution
