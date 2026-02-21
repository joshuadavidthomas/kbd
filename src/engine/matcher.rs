//! Binding matcher — finds which binding (if any) matches a key event.
//!
//! Walks the layer stack top-down, checking bindings in each active layer,
//! then global bindings. Within each layer, speculative patterns (tap-hold,
//! sequences) are checked before immediate patterns (hotkeys).
//!
//! Returns the matched binding's action (or "no match" for forwarding).
//!
//! # Reference
//!
//! Prior art: `archive/v0/src/listener/dispatch.rs` (scattered across
//! `collect_non_modifier_dispatch`, `collect_device_specific_dispatch`,
//! `dispatch_mode_key_event`). This module unifies all matching into one path.

// TODO: match_event() — given a key event + current state, find the matching binding
// TODO: Layer stack traversal with priority
// TODO: Speculative vs immediate pattern ordering
// TODO: Device filter checking (per-binding, not per-dispatch-phase)
