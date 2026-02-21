use std::collections::HashMap;
use std::time::Duration;

use crate::manager::HotkeyKey;
use crate::manager::HotkeyRegistration;

/// Options for a named hotkey mode.
///
/// Modes are groups of hotkeys activated via [`ModeController::push`](crate::ModeController::push).
/// Use options to control auto-deactivation behavior.
#[derive(Clone, Default)]
pub struct ModeOptions {
    pub(crate) oneshot: bool,
    pub(crate) swallow: bool,
    pub(crate) timeout: Option<Duration>,
}

impl ModeOptions {
    /// Create default mode options (persistent, no swallow, no timeout).
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Automatically pop the mode after the first hotkey in the mode fires.
    #[must_use]
    pub fn oneshot(mut self) -> Self {
        self.oneshot = true;
        self
    }

    /// Consume all key events while this mode is active, even keys that don't
    /// match any binding in the mode. Requires grab mode.
    #[must_use]
    pub fn swallow(mut self) -> Self {
        self.swallow = true;
        self
    }

    /// Automatically pop the mode after this duration of inactivity.
    #[must_use]
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }
}

pub(crate) struct ModeDefinition {
    pub(crate) options: ModeOptions,
    pub(crate) bindings: HashMap<HotkeyKey, HotkeyRegistration>,
}
