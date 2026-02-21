mod builder;
mod controller;
pub(crate) mod dispatch;
pub(crate) mod options;
pub(crate) mod registry;
pub(crate) mod stack;

pub use builder::ModeBuilder;
pub use controller::ModeController;
pub use options::ModeOptions;

pub(crate) use dispatch::dispatch_mode_key_event;
pub(crate) use dispatch::find_callbacks_for_active_press;
pub(crate) use dispatch::ModeEventDispatch;
pub(crate) use options::ModeDefinition;
pub(crate) use registry::ModeRegistry;
pub(crate) use stack::pop_timed_out_modes;

#[cfg(test)]
pub(crate) mod tests {
    use std::sync::atomic::AtomicUsize;
    use std::sync::atomic::Ordering;
    use std::sync::Arc;

    use crate::manager::Callback;
    use crate::manager::HotkeyCallbacks;
    use crate::manager::HotkeyKey;
    use crate::manager::HotkeyRegistration;
    use crate::manager::RepeatBehavior;

    use super::options::ModeDefinition;
    use super::options::ModeOptions;

    pub(crate) fn make_registration(counter: Arc<AtomicUsize>) -> HotkeyRegistration {
        HotkeyRegistration {
            callbacks: HotkeyCallbacks {
                on_press: Arc::new(move || {
                    counter.fetch_add(1, Ordering::SeqCst);
                }),
                on_release: None,
                wait_for_release: false,
                min_hold: None,
                repeat_behavior: RepeatBehavior::Ignore,
                passthrough: false,
            },
        }
    }

    pub(crate) fn make_registration_with_release(
        press_counter: Arc<AtomicUsize>,
        release_counter: Arc<AtomicUsize>,
    ) -> HotkeyRegistration {
        let rc = release_counter;
        HotkeyRegistration {
            callbacks: HotkeyCallbacks {
                on_press: Arc::new(move || {
                    press_counter.fetch_add(1, Ordering::SeqCst);
                }),
                on_release: Some(Arc::new(move || {
                    rc.fetch_add(1, Ordering::SeqCst);
                })),
                wait_for_release: true,
                min_hold: None,
                repeat_behavior: RepeatBehavior::Ignore,
                passthrough: false,
            },
        }
    }

    pub(crate) fn make_definition(
        options: ModeOptions,
        bindings: Vec<(HotkeyKey, HotkeyRegistration)>,
    ) -> ModeDefinition {
        ModeDefinition {
            options,
            bindings: bindings.into_iter().collect(),
        }
    }

    pub(crate) fn dispatch_callbacks(callbacks: Vec<Callback>) {
        for cb in callbacks {
            cb();
        }
    }

    // ModeOptions tests live here since they test the type directly

    #[test]
    fn mode_options_default_has_no_special_behavior() {
        let opts = ModeOptions::new();
        assert!(!opts.oneshot);
        assert!(!opts.swallow);
        assert!(opts.timeout.is_none());
    }

    #[test]
    fn mode_options_oneshot_swallow_timeout() {
        use std::time::Duration;

        let opts = ModeOptions::new()
            .oneshot()
            .swallow()
            .timeout(Duration::from_secs(5));

        assert!(opts.oneshot);
        assert!(opts.swallow);
        assert_eq!(opts.timeout, Some(Duration::from_secs(5)));
    }
}
