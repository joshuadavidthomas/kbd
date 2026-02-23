//! Input backends — where key events come from.
//!
//! Two backends:
//! - **evdev** — direct `/dev/input/event*` access (universal Linux)
//! - **portal** — XDG `GlobalShortcuts` D-Bus portal (no root needed)
//!
//! Auto-detection: try portal first (when compiled in), fall back to evdev.
//! Explicit selection via `HotkeyManager::builder().backend(Backend::Evdev)`.
//!
//! The backend trait is minimal — it provides device access and capability
//! information. The engine handles all event processing.
//!
//! # Reference
//!
//! Prior art: `archive/v0/src/backend.rs`

pub(crate) mod evdev;

#[cfg(feature = "portal")]
pub(crate) mod portal;

// TODO: Backend capability detection (supports grab, supports sequences, etc.)
// TODO: Auto-detection logic (portal probe → evdev fallback)

/// Backend selection for explicit configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Backend {
    /// Direct evdev device access. Requires `input` group membership.
    Evdev,
    /// XDG `GlobalShortcuts` portal. No special permissions needed.
    #[cfg(feature = "portal")]
    Portal,
}
