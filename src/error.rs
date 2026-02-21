//! Error types for the keybound library.
//!
//! Single error enum covering all failure modes. Use `thiserror` for
//! structured variants — no `Error(String)`.
//!
//! # Reference
//!
//! Prior art: `archive/v0/src/error.rs`, `archive/v0/src/hotkey.rs`
//! (ParseHotkeyError). Consolidate into one hierarchy.

// TODO: Error enum with thiserror derives
// TODO: Variants for: parse errors, registration conflicts, backend errors,
//       permission denied, device errors, unsupported feature, manager stopped
// TODO: Absorb ParseHotkeyError into Error (or keep as separate type if
//       FromStr requires it, but make it convertible)

/// Placeholder — see module docs.
#[derive(Debug)]
pub enum Error {}

impl std::fmt::Display for Error {
    fn fmt(&self, _f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Ok(())
    }
}

impl std::error::Error for Error {}
