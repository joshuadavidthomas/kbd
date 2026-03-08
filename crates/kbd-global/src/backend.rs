//! Input backends for the global runtime.
//!
//! Backend selection is explicit through
//! [`HotkeyManagerBuilder::backend()`](crate::manager::HotkeyManagerBuilder::backend).
//! At present the runtime supports only direct evdev access.

/// Backend selection for explicit configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Backend {
    /// Direct evdev device access.
    ///
    /// Reads `/dev/input/event*` directly and therefore requires permission to
    /// access Linux input devices.
    Evdev,
}
