#![cfg_attr(docsrs, feature(doc_cfg))]

//! Planned procedural macros for `kbd`.
//!
//! The intended scope includes declarative binding registration,
//! compile-time validation of hotkey strings, and composition helpers
//! for larger binding sets.
//!
//! Expected surface area includes macros such as:
//!
//! - `#[derive(Bindings)]`
//! - `#[hotkey(...)]`
//! - `#[flatten]`
//!
//! # Status
//!
//! This crate is currently a scaffold only. No procedural macros are
//! implemented yet.
