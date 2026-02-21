use std::sync::Arc;
use std::time::Duration;

use super::callbacks::Callback;
use super::callbacks::HotkeyCallbacks;
use super::callbacks::PressTimingConfig;
use super::callbacks::ReleaseBehavior;
use super::callbacks::RepeatBehavior;
use crate::device::DeviceFilter;

#[derive(Clone, Default)]
pub struct HotkeyOptions {
    pub(crate) release_behavior: ReleaseBehavior,
    pub(crate) min_hold: Option<Duration>,
    pub(crate) repeat_behavior: RepeatBehavior,
    pub(crate) passthrough: bool,
    pub(crate) debounce: Option<Duration>,
    pub(crate) max_rate: Option<Duration>,
    pub(crate) device_filter: Option<DeviceFilter>,
}

impl HotkeyOptions {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn on_release(mut self) -> Self {
        self.release_behavior = ReleaseBehavior::SameAsPress;
        self
    }

    #[must_use]
    pub fn on_release_callback<F>(mut self, callback: F) -> Self
    where
        F: Fn() + Send + Sync + 'static,
    {
        self.release_behavior = ReleaseBehavior::Custom(Arc::new(callback));
        self
    }

    #[must_use]
    pub fn min_hold(mut self, min_hold: Duration) -> Self {
        self.min_hold = Some(min_hold);
        self
    }

    #[must_use]
    pub fn trigger_on_repeat(mut self) -> Self {
        self.repeat_behavior = RepeatBehavior::Trigger;
        self
    }

    #[must_use]
    pub fn passthrough(mut self) -> Self {
        self.passthrough = true;
        self
    }

    /// Suppress press callback invocations until there has been at least this
    /// much quiet time since the previous press attempt.
    #[must_use]
    pub fn debounce(mut self, duration: Duration) -> Self {
        self.debounce = Some(duration);
        self
    }

    /// Cap press callback invocations to at most one successful dispatch per
    /// interval.
    #[must_use]
    pub fn max_rate(mut self, interval: Duration) -> Self {
        self.max_rate = Some(interval);
        self
    }

    /// Restrict this hotkey to events from devices matching the given filter.
    #[must_use]
    pub fn device(mut self, filter: DeviceFilter) -> Self {
        self.device_filter = Some(filter);
        self
    }

    pub(crate) fn press_timing_config(&self) -> PressTimingConfig {
        PressTimingConfig::new(self.debounce, self.max_rate)
    }

    pub(crate) fn build_callbacks<F>(self, callback: F) -> HotkeyCallbacks
    where
        F: Fn() + Send + Sync + 'static,
    {
        let press_callback: Callback = Arc::new(callback);
        let (release_callback, wait_for_release) = match self.release_behavior {
            ReleaseBehavior::Disabled => (None, false),
            ReleaseBehavior::SameAsPress => (Some(press_callback.clone()), true),
            ReleaseBehavior::Custom(callback) => (Some(callback), true),
        };

        HotkeyCallbacks {
            on_press: press_callback,
            on_release: release_callback,
            wait_for_release,
            min_hold: self.min_hold,
            repeat_behavior: self.repeat_behavior,
            passthrough: self.passthrough,
        }
    }
}

use crate::hotkey::Hotkey;

#[derive(Clone)]
pub struct SequenceOptions {
    pub(crate) timeout: Duration,
    pub(crate) abort_key: crate::key::Key,
    pub(crate) timeout_fallback: Option<Hotkey>,
}

impl Default for SequenceOptions {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(1),
            abort_key: crate::key::Key::Escape,
            timeout_fallback: None,
        }
    }
}

impl SequenceOptions {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    #[must_use]
    pub fn abort_key(mut self, key: crate::key::Key) -> Self {
        self.abort_key = key;
        self
    }

    #[must_use]
    pub fn timeout_fallback(mut self, hotkey: Hotkey) -> Self {
        self.timeout_fallback = Some(hotkey);
        self
    }
}

#[derive(Clone, Copy, Default)]
pub(crate) struct ManagerRuntimeOptions {
    pub(crate) grab: bool,
}

pub struct HotkeyManagerBuilder {
    pub(super) requested_backend: Option<crate::backend::Backend>,
    pub(super) options: ManagerRuntimeOptions,
}

impl HotkeyManagerBuilder {
    #[must_use]
    pub fn backend(mut self, backend: crate::backend::Backend) -> Self {
        self.requested_backend = Some(backend);
        self
    }

    #[must_use]
    pub fn grab(mut self) -> Self {
        self.options.grab = true;
        self
    }

    pub fn build(self) -> Result<super::HotkeyManager, crate::error::Error> {
        super::HotkeyManager::with_backend_internal(self.requested_backend, self.options)
    }
}
