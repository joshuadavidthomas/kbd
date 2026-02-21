//! Key state tracking — single source of truth for what's pressed.
//!
//! Modifier state is derived from key state, not tracked separately.
//! "Is Ctrl held?" = "is `KEY_LEFTCTRL` or `KEY_RIGHTCTRL` in the pressed set?"
//!
//! Replaces v0's separate `ModifierTracker` (per-device `HashSet<Modifier>`)
//! and `SharedKeyState` (atomic counters). The engine owns this exclusively —
//! no Arc, no atomics, no shared access.
//!
//! # Reference
//!
//! Prior art: `archive/v0/src/key_state.rs`,
//! `archive/v0/src/listener/device.rs` (`ModifierTracker`)

// TODO: KeyState struct — what's currently pressed
// TODO: Per-device key tracking (for device-specific bindings)
// TODO: active_modifiers() — derived from pressed keys, not parallel state
// TODO: Cleanup on device disconnect (no stuck modifiers)

#[derive(Debug, Default)]
pub(crate) struct KeyState;
