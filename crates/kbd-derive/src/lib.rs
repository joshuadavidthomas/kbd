#![cfg_attr(docsrs, feature(doc_auto_cfg))]

//! Derive macros for kbd.
//!
//! This crate will provide `#[derive(Bindings)]` for declarative hotkey
//! registration, `#[hotkey(...)]` attributes, `#[flatten]` for composition,
//! and compile-time hotkey string validation.
//!
//! # Status
//!
//! Placeholder proc-macro crate. No macros implemented yet.
//! Implementation planned after Phase 4, when the full action vocabulary
//! (sequences, tap-hold, emit) has settled.
//!
//! See PLAN.md "Future idea: derive macro for declarative bindings" for
//! the full design rationale.
