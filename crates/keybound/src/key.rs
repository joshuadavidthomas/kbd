//! Re-exports key types from [`kbd_core::key`].
//!
//! All key-related types are defined in `kbd-core`. This module provides
//! a crate-local namespace so internal imports (`use crate::key::*`) and
//! external imports work the same way.

pub use kbd_core::key::*;
