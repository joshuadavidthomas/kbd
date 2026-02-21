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

#[derive(Debug, Default)]
pub(crate) struct KeyState;
