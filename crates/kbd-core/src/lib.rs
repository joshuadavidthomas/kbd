//! Pure-logic keyboard shortcut engine.
//!
//! `kbd-core` provides the domain types and matching logic that every keyboard
//! shortcut system needs: key types, modifier tracking, binding matching, layer
//! stacks, and sequence resolution. It has zero platform dependencies and can
//! be embedded in any event loop — winit, GPUI, Smithay, a game loop, or a
//! compositor.
//!
//! # What belongs here
//!
//! - `Key`, `Modifier`, `Hotkey`, `HotkeySequence` — the input vocabulary
//! - `Action`, `Binding`, `BindingOptions`, `BindingId` — what to match and do
//! - `Layer`, `LayerOptions` — named binding groups that stack
//! - `Matcher`, `MatchResult`, `KeyState` — the synchronous matching engine
//! - Core error types (parse, conflict, layer)
//!
//! # What does NOT belong here
//!
//! - evdev types or Linux-specific I/O (`kbd-evdev`)
//! - Portal / D-Bus integration (`kbd-portal`)
//! - Keyboard layout / xkbcommon (`kbd-xkb`)
//! - Threaded manager, message passing, handles (`keybound`)

// TODO: Phase 3.7 — move key.rs, action.rs, binding.rs, layer.rs here
// TODO: Phase 3.7 — move engine/matcher.rs, engine/key_state.rs here (pure logic)
// TODO: Phase 3.7 — move core error variants here
// TODO: Phase 3.9 — expose public synchronous Matcher type
