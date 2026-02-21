use std::collections::HashMap;
use std::time::Duration;

use crate::manager::HotkeyKey;
use crate::manager::HotkeyRegistration;

#[derive(Clone, Default)]
pub struct ModeOptions {
    pub(crate) oneshot: bool,
    pub(crate) swallow: bool,
    pub(crate) timeout: Option<Duration>,
}

impl ModeOptions {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn oneshot(mut self) -> Self {
        self.oneshot = true;
        self
    }

    #[must_use]
    pub fn swallow(mut self) -> Self {
        self.swallow = true;
        self
    }

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
