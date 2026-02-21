use std::sync::Arc;
use std::time::Duration;

use super::callbacks::Callback;
use super::callbacks::HotkeyCallbacks;
use super::callbacks::PressTimingConfig;
use super::callbacks::ReleaseBehavior;
use super::callbacks::RepeatBehavior;
use crate::device::DeviceFilter;

/// Per-hotkey options for press/release behavior, timing, and device filtering.
///
/// Use with [`HotkeyManager::register_with_options`](crate::HotkeyManager::register_with_options).
/// All settings are optional — the default is press-only, no debounce, no
/// device filter.
///
/// # Examples
///
/// ```
/// use std::time::Duration;
/// use keybound::HotkeyOptions;
///
/// let opts = HotkeyOptions::new()
///     .on_release_callback(|| println!("released"))
///     .min_hold(Duration::from_millis(500))
///     .debounce(Duration::from_millis(100));
/// ```
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
    /// Create default options (press-only, no timing constraints).
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Fire the press callback again on key release.
    #[must_use]
    pub fn on_release(mut self) -> Self {
        self.release_behavior = ReleaseBehavior::SameAsPress;
        self
    }

    /// Fire a separate callback on key release.
    #[must_use]
    pub fn on_release_callback<F>(mut self, callback: F) -> Self
    where
        F: Fn() + Send + Sync + 'static,
    {
        self.release_behavior = ReleaseBehavior::Custom(Arc::new(callback));
        self
    }

    /// Require the key to be held for at least this duration before the press
    /// callback fires.
    #[must_use]
    pub fn min_hold(mut self, min_hold: Duration) -> Self {
        self.min_hold = Some(min_hold);
        self
    }

    /// Also fire the press callback on key-repeat events (default: ignore repeats).
    #[must_use]
    pub fn trigger_on_repeat(mut self) -> Self {
        self.repeat_behavior = RepeatBehavior::Trigger;
        self
    }

    /// In grab mode, re-emit this hotkey to other applications after firing
    /// the callback instead of consuming it.
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

/// Options for key sequence registration.
///
/// Controls the timeout between steps, the abort key, and optional fallback
/// behavior when a sequence times out.
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
    /// Create default options (1 second timeout, Escape to abort).
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Maximum time allowed between consecutive steps before the sequence
    /// resets (default: 1 second).
    #[must_use]
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Key that immediately cancels an in-progress sequence (default: Escape).
    #[must_use]
    pub fn abort_key(mut self, key: crate::key::Key) -> Self {
        self.abort_key = key;
        self
    }

    /// When the sequence times out after the first step, dispatch this hotkey
    /// as if it had been pressed normally instead of discarding the input.
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

/// Builder for [`HotkeyManager`](crate::HotkeyManager) with non-default settings.
///
/// Obtain via [`HotkeyManager::builder`](crate::HotkeyManager::builder).
///
/// # Examples
///
/// ```rust,no_run
/// use keybound::HotkeyManager;
///
/// let manager = HotkeyManager::builder()
///     .grab()
///     .build()?;
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub struct HotkeyManagerBuilder {
    pub(super) requested_backend: Option<crate::backend::Backend>,
    pub(super) options: ManagerRuntimeOptions,
}

impl HotkeyManagerBuilder {
    /// Use a specific backend instead of auto-detecting.
    #[must_use]
    pub fn backend(mut self, backend: crate::backend::Backend) -> Self {
        self.requested_backend = Some(backend);
        self
    }

    /// Enable exclusive key capture via `EVIOCGRAB`.
    ///
    /// Requires the `grab` feature and the evdev backend. Non-hotkey events
    /// are re-emitted through a virtual uinput device.
    #[must_use]
    pub fn grab(mut self) -> Self {
        self.options.grab = true;
        self
    }

    /// Build and start the [`HotkeyManager`](crate::HotkeyManager).
    pub fn build(self) -> Result<super::HotkeyManager, crate::error::Error> {
        super::HotkeyManager::with_backend_internal(self.requested_backend, self.options)
    }
}
