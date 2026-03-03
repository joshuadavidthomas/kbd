//! Input backends — where key events come from.
//!
//! Currently evdev-only: direct `/dev/input/event*` access (universal Linux).
//! XDG `GlobalShortcuts` portal backend is not yet available.
//!
//! Explicit selection via `HotkeyManager::builder().backend(Backend::Evdev)`.
//!
//! The backend trait is minimal — it provides device access and capability
//! information. The engine handles all event processing.
//!

pub(crate) mod evdev;

// TODO: Backend capability detection (supports grab, supports sequences, etc.)
// TODO: Auto-detection logic (portal probe → evdev fallback)

/// Backend selection for explicit configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Backend {
    /// Direct evdev device access. Requires `input` group membership.
    Evdev,
}
