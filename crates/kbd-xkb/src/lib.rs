#![cfg_attr(docsrs, feature(doc_cfg))]

//! Planned keyboard-layout integration for `kbd`.
//!
//! The intended scope is xkbcommon-backed layout awareness for features
//! that depend on keyboard symbols rather than physical key positions.
//! Examples include:
//!
//! - resolving keycodes to keysyms using the active layout
//! - distinguishing position-based and symbol-based bindings
//! - re-resolving symbol-based bindings when the active layout changes
//!
//! # Status
//!
//! This crate is currently a scaffold only. No functional xkb integration
//! has been implemented yet.
