#![cfg_attr(docsrs, feature(doc_auto_cfg))]

//! Keyboard layout awareness for kbd.
//!
//! This crate will provide xkbcommon integration:
//!
//! - Keycode → keysym resolution based on active XKB layout
//! - `KeyReference` enum: `ByCode` (position-based) vs `BySymbol` (character-based)
//! - Layout change detection and re-resolution of symbol-based bindings
//!
//! # Status
//!
//! Placeholder crate. No dependencies or implementation yet.
//! Full implementation is planned for Phase 4.9.
