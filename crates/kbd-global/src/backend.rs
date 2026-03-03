//! Input backends — where key events come from.
//!
//! Currently evdev-only: direct `/dev/input/event*` access (universal Linux).
//!
//! Explicit selection via `HotkeyManager::builder().backend(Backend::Evdev)`.

/// Backend selection for explicit configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Backend {
    /// Direct evdev device access. Requires `input` group membership.
    Evdev,
}
