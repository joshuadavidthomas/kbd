#![cfg_attr(docsrs, feature(doc_cfg))]

//! Keyboard layout awareness for `kbd`.
//!
//! xkbcommon integration for:
//!
//! - Keycode → keysym resolution based on active XKB layout
//! - `KeyReference` enum: `ByCode` (position-based) vs `BySymbol` (character-based)
//! - Layout change detection and re-resolution of symbol-based bindings
//!
//! # Status
//!
//! Not yet implemented.
