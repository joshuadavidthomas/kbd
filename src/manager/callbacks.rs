use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;
use std::time::Instant;

/// Callback storage type
pub(crate) type Callback = Arc<dyn Fn() + Send + Sync>;

#[derive(Clone, Default)]
pub(crate) enum ReleaseBehavior {
    #[default]
    Disabled,
    SameAsPress,
    Custom(Callback),
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum RepeatBehavior {
    #[default]
    Ignore,
    Trigger,
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum PressDispatchState {
    #[default]
    Pending,
    Dispatched,
}

#[derive(Clone)]
pub(crate) struct HotkeyCallbacks {
    pub(crate) on_press: Callback,
    pub(crate) on_release: Option<Callback>,
    pub(crate) wait_for_release: bool,
    pub(crate) min_hold: Option<Duration>,
    pub(crate) repeat_behavior: RepeatBehavior,
    pub(crate) passthrough: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct PressTimingConfig {
    pub(crate) debounce: Option<Duration>,
    pub(crate) max_rate: Option<Duration>,
}

impl PressTimingConfig {
    pub(crate) fn new(debounce: Option<Duration>, max_rate: Option<Duration>) -> Self {
        Self {
            debounce: debounce.filter(|duration| !duration.is_zero()),
            max_rate: max_rate.filter(|duration| !duration.is_zero()),
        }
    }

    pub(crate) fn is_disabled(&self) -> bool {
        self.debounce.is_none() && self.max_rate.is_none()
    }
}

#[derive(Default)]
pub(crate) struct PressInvocationState {
    last_dispatch: Option<Instant>,
}

pub(crate) struct PressInvocationLimiter {
    config: PressTimingConfig,
    state: Mutex<PressInvocationState>,
}

impl PressInvocationLimiter {
    pub(crate) fn new(config: PressTimingConfig) -> Self {
        Self {
            config,
            state: Mutex::new(PressInvocationState::default()),
        }
    }

    pub(crate) fn should_dispatch_now(&self) -> bool {
        self.should_dispatch_at(Instant::now())
    }

    pub(crate) fn should_dispatch_at(&self, now: Instant) -> bool {
        if self.config.is_disabled() {
            return true;
        }

        let mut state = self.state.lock().unwrap();
        let Some(last_dispatch) = state.last_dispatch else {
            state.last_dispatch = Some(now);
            return true;
        };

        if self
            .config
            .debounce
            .is_some_and(|debounce| now.saturating_duration_since(last_dispatch) < debounce)
        {
            return false;
        }

        if self
            .config
            .max_rate
            .is_some_and(|max_rate| now.saturating_duration_since(last_dispatch) < max_rate)
        {
            return false;
        }

        state.last_dispatch = Some(now);
        true
    }
}
