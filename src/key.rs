//! Key types: [`Key`], [`Modifier`], [`Hotkey`], [`HotkeySequence`].
//!
//! Single source of truth for all key-related logic: the key enum, modifier
//! convenience type, hotkey combinations, string parsing (`FromStr`),
//! display formatting, and evdev conversions (`From`/`Into`).
//!
//! # Design notes
//!
//! A key is a key. Ctrl is a key. A is a key. The distinction between
//! "modifier" and "key" is about role in a combination, not about the key
//! itself. `Modifier` exists as a convenience type for the four common
//! modifiers (Ctrl, Shift, Alt, Super) with left/right canonicalization.
//! Internally everything resolves to key codes.
//!
//! `Key` and `Modifier` share behavior (parsing, display, evdev conversion).
//! Eliminate duplication via shared trait, macro, or deriving Modifier from Key.
//!
//! # Reference
//!
//! Prior art: `archive/v0/src/key.rs`, `archive/v0/src/hotkey.rs`

// TODO: Key enum — complete set of keys including modifiers
// TODO: Modifier enum — Ctrl, Shift, Alt, Super with left/right canonicalization
// TODO: Hotkey — trigger Key + ModifierSet, FromStr/Display ("Ctrl+Shift+A")
// TODO: HotkeySequence — Vec<Hotkey>, FromStr/Display ("Ctrl+K, Ctrl+C")
// TODO: From<KeyCode>/Into<KeyCode> for evdev conversion (not ad-hoc methods)
// TODO: Single source of truth for modifier key mappings

/// Placeholder — see TODO items above.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Key {}

/// Placeholder — see TODO items above.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Modifier {}

/// Placeholder — see TODO items above.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Hotkey;

/// Placeholder — see TODO items above.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct HotkeySequence;
