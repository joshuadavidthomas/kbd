pub(crate) mod device;
pub(crate) mod dispatch;
pub(crate) mod forwarding;
pub(crate) mod hotplug;
pub(crate) mod io;
pub(crate) mod sequence;
pub(crate) mod state;

use std::collections::HashSet;
use std::sync::atomic::Ordering;
use std::time::Instant;

use device::DeviceState;
use device::ModifierTracker;
use dispatch::active_modifier_signature;
use dispatch::collect_callbacks_for_synthetic_keys;
use dispatch::collect_device_specific_dispatch;
use dispatch::collect_due_hold_callbacks;
use dispatch::collect_non_modifier_dispatch;
use dispatch::dispatch_callbacks;
use dispatch::should_forward_key_event_in_grab_mode;
use dispatch::suppress_sequence_followup_key_event;
use dispatch::NonModifierDispatch;
use forwarding::KeyEventForwarder;
use hotplug::process_hotplug_events;
use hotplug::RawFdGuard;
use io::emit_shutdown_tap_hold_releases;
use io::poll_ready_sources;
use io::read_key_events;
use io::release_pressed_keys;
use io::remove_device_by_fd;
use io::should_drop_device;
pub(crate) use io::spawn_listener_thread;
use io::update_pressed_key_state;
use sequence::SequenceDispatch;
use sequence::SequenceRuntime;
pub(crate) use state::ListenerConfig;
pub(crate) use state::ListenerState;

use crate::events::HotkeyEvent;
use crate::key::Key;
use crate::key::Modifier;
use crate::mode::dispatch_mode_key_event;
use crate::mode::pop_timed_out_modes;
use crate::mode::ModeEventDispatch;

#[allow(clippy::too_many_lines)]
fn listener_loop(
    mut devices: Vec<DeviceState>,
    inotify_fd: &RawFdGuard,
    shared: ListenerState,
    config: ListenerConfig,
    mut key_event_forwarder: Option<Box<dyn KeyEventForwarder>>,
) {
    let ListenerState {
        registrations,
        sequence_registrations,
        device_registrations,
        tap_hold_registrations,
        stop_flag,
        key_state,
        mode_registry,
    } = shared;
    let mut modifier_tracker = ModifierTracker::default();
    let mut sequence_runtime = SequenceRuntime::default();
    let mut tap_hold_runtime = crate::tap_hold::TapHoldRuntime::default();
    let mut suppressed_sequence_keys: HashSet<Key> = HashSet::new();

    loop {
        if stop_flag.load(Ordering::SeqCst) {
            emit_shutdown_tap_hold_releases(&mut tap_hold_runtime, &mut key_event_forwarder);
            release_pressed_keys(&devices, &key_state);
            return;
        }

        let (inotify_ready, ready_devices) = match poll_ready_sources(inotify_fd.raw_fd(), &devices)
        {
            Ok(ready) => ready,
            Err(err) => {
                tracing::warn!("Poll error, stopping listener: {}", err);
                stop_flag.store(true, Ordering::SeqCst);
                emit_shutdown_tap_hold_releases(&mut tap_hold_runtime, &mut key_event_forwarder);
                release_pressed_keys(&devices, &key_state);
                return;
            }
        };

        let (timeout_callbacks, tap_hold_tick_synthetics, timeout_mode_change_event) = {
            let now = Instant::now();
            let registrations_guard = registrations.lock().unwrap();
            let sequence_guard = sequence_registrations.lock().unwrap();
            let mode_definitions_guard = mode_registry.definitions.lock().unwrap();
            let mut mode_stack_guard = mode_registry.stack.lock().unwrap();

            let timed_out_modes =
                pop_timed_out_modes(&mut mode_stack_guard, &mode_definitions_guard, now);
            let timeout_mode_change_event = if timed_out_modes.is_empty() {
                None
            } else {
                Some(mode_stack_guard.top().map(str::to_string))
            };

            let tap_hold_tick = tap_hold_runtime.on_tick(now);

            let timeout_dispatch =
                sequence_runtime.on_tick(now, &registrations_guard, &sequence_guard);
            let mut callbacks = timeout_dispatch.callbacks;
            callbacks.extend(collect_callbacks_for_synthetic_keys(
                &timeout_dispatch.synthetic_keys,
                &registrations_guard,
            ));

            let device_regs_guard = device_registrations.lock().unwrap();
            for device in &mut devices {
                callbacks.extend(collect_due_hold_callbacks(
                    now,
                    &registrations_guard,
                    &mode_definitions_guard,
                    &device_regs_guard,
                    &mut device.active_presses,
                ));
            }

            (
                callbacks,
                tap_hold_tick.synthetic_events,
                timeout_mode_change_event,
            )
        };

        if let Some(mode_name) = timeout_mode_change_event {
            mode_registry
                .event_hub
                .emit(&HotkeyEvent::ModeChanged(mode_name));
        }

        dispatch_callbacks(timeout_callbacks);

        for (syn_key, syn_value) in tap_hold_tick_synthetics {
            if let Some(forwarder) = key_event_forwarder.as_mut() {
                if let Err(err) = forwarder.forward_key_event(syn_key, syn_value) {
                    tracing::warn!("Failed emitting tap-hold synthetic event: {}", err);
                }
            }
        }

        if inotify_ready {
            process_hotplug_events(
                inotify_fd.raw_fd(),
                &mut devices,
                &mut modifier_tracker,
                &key_state,
                config,
            );
        }

        for (fd, revents) in ready_devices {
            if revents & (libc::POLLERR | libc::POLLHUP | libc::POLLNVAL) != 0 {
                remove_device_by_fd(fd, &mut devices, &mut modifier_tracker, &key_state);
                continue;
            }

            let Some(device_index) = devices.iter().position(|device| device.fd() == fd) else {
                continue;
            };

            let device_path = devices[device_index].path.clone();

            let key_events = {
                let device = &mut devices[device_index].device;
                match read_key_events(device) {
                    Ok(events) => events,
                    Err(err) if should_drop_device(&err) => {
                        remove_device_by_fd(fd, &mut devices, &mut modifier_tracker, &key_state);
                        continue;
                    }
                    Err(_) => {
                        continue;
                    }
                }
            };

            for (key, value) in key_events {
                update_pressed_key_state(
                    &mut devices[device_index].pressed_keys,
                    &key_state,
                    key,
                    value,
                );

                if let Some(modifier) = Modifier::from_evdev(key) {
                    match value {
                        1 => modifier_tracker.press(&device_path, modifier),
                        0 => modifier_tracker.release(&device_path, modifier),
                        _ => {}
                    }

                    if config.grab {
                        if let Some(forwarder) = key_event_forwarder.as_mut() {
                            if let Err(err) = forwarder.forward_key_event(key, value) {
                                tracing::warn!("Failed forwarding modifier key event: {}", err);
                            }
                        }
                    }
                    continue;
                }

                let Some(abstract_key) = Key::from_evdev(key) else {
                    if config.grab {
                        if let Some(forwarder) = key_event_forwarder.as_mut() {
                            if let Err(err) = forwarder.forward_key_event(key, value) {
                                tracing::warn!("Failed forwarding unknown key event: {}", err);
                            }
                        }
                    }
                    continue;
                };

                let tap_hold_dispatch = {
                    let tap_hold_regs_guard = tap_hold_registrations.lock().unwrap();
                    tap_hold_runtime.process_key_event(
                        abstract_key,
                        value,
                        Instant::now(),
                        &tap_hold_regs_guard,
                    )
                };

                for (syn_key, syn_value) in &tap_hold_dispatch.synthetic_events {
                    if let Some(forwarder) = key_event_forwarder.as_mut() {
                        if let Err(err) = forwarder.forward_key_event(*syn_key, *syn_value) {
                            tracing::warn!("Failed emitting tap-hold synthetic event: {}", err);
                        }
                    }
                }

                if tap_hold_dispatch.consumed {
                    continue;
                }

                let (callbacks, should_forward_event, sequence_step_events, mode_change_event) = {
                    let now = Instant::now();
                    let active_modifiers = modifier_tracker.active_modifiers();
                    let hotkey_key = (abstract_key, active_modifier_signature(&active_modifiers));

                    let (mode_dispatch, mode_change_event) = {
                        let mode_definitions_guard = mode_registry.definitions.lock().unwrap();
                        let mut mode_stack_guard = mode_registry.stack.lock().unwrap();
                        let active_mode_before = mode_stack_guard.top().map(str::to_string);
                        let mode_dispatch = dispatch_mode_key_event(
                            &hotkey_key,
                            value,
                            now,
                            &mode_definitions_guard,
                            &mut mode_stack_guard,
                            &mut devices[device_index].active_presses,
                        );
                        let active_mode_after = mode_stack_guard.top().map(str::to_string);
                        let mode_change_event =
                            (active_mode_before != active_mode_after).then_some(active_mode_after);
                        (mode_dispatch, mode_change_event)
                    };

                    match mode_dispatch {
                        ModeEventDispatch::Swallowed => {
                            (Vec::new(), false, Vec::new(), mode_change_event)
                        }
                        ModeEventDispatch::Handled {
                            callbacks: mode_callbacks,
                            passthrough,
                        } => {
                            let should_forward = should_forward_key_event_in_grab_mode(
                                config.grab,
                                true,
                                passthrough,
                            );
                            (
                                mode_callbacks,
                                should_forward,
                                Vec::new(),
                                mode_change_event,
                            )
                        }
                        ModeEventDispatch::PassThrough => {
                            let device_modifiers = modifier_tracker.device_modifiers(&device_path);
                            let device_info = devices[device_index].info.clone();
                            let device_regs_guard = device_registrations.lock().unwrap();
                            let device_dispatch = collect_device_specific_dispatch(
                                abstract_key,
                                value,
                                now,
                                &device_info,
                                &device_modifiers,
                                &device_regs_guard,
                                &mut devices[device_index].active_presses,
                            );
                            drop(device_regs_guard);

                            if device_dispatch.matched {
                                let should_forward = should_forward_key_event_in_grab_mode(
                                    config.grab,
                                    true,
                                    device_dispatch.passthrough,
                                );
                                (
                                    device_dispatch.callbacks,
                                    should_forward,
                                    Vec::new(),
                                    mode_change_event,
                                )
                            } else {
                                let registrations_guard = registrations.lock().unwrap();
                                let sequence_guard = sequence_registrations.lock().unwrap();

                                let mut sequence_dispatch = SequenceDispatch::empty();
                                let mut sequence_release_callbacks = Vec::new();
                                if value == 1 {
                                    sequence_dispatch = sequence_runtime.on_key_press(
                                        hotkey_key,
                                        now,
                                        &registrations_guard,
                                        &sequence_guard,
                                    );
                                } else if value == 0 {
                                    sequence_release_callbacks =
                                        sequence_runtime.on_key_release(abstract_key, now);
                                }

                                let mut callbacks = sequence_dispatch.callbacks;
                                callbacks.extend(sequence_release_callbacks);
                                callbacks.extend(collect_callbacks_for_synthetic_keys(
                                    &sequence_dispatch.synthetic_keys,
                                    &registrations_guard,
                                ));
                                let sequence_step_events = sequence_dispatch.step_events;

                                let suppress_followup = suppress_sequence_followup_key_event(
                                    &mut suppressed_sequence_keys,
                                    abstract_key,
                                    value,
                                    sequence_dispatch.suppress_current_key_press,
                                );

                                let non_modifier_dispatch = if suppress_followup {
                                    NonModifierDispatch {
                                        callbacks: Vec::new(),
                                        matched_hotkey: true,
                                        passthrough: false,
                                    }
                                } else {
                                    collect_non_modifier_dispatch(
                                        abstract_key,
                                        value,
                                        now,
                                        &active_modifiers,
                                        &registrations_guard,
                                        &mut devices[device_index].active_presses,
                                        sequence_dispatch.suppress_current_key_press,
                                    )
                                };

                                let should_forward_event = should_forward_key_event_in_grab_mode(
                                    config.grab,
                                    sequence_dispatch.suppress_current_key_press
                                        || non_modifier_dispatch.matched_hotkey,
                                    non_modifier_dispatch.passthrough,
                                );

                                callbacks.extend(non_modifier_dispatch.callbacks);

                                (
                                    callbacks,
                                    should_forward_event,
                                    sequence_step_events,
                                    mode_change_event,
                                )
                            }
                        }
                    }
                };

                for event in sequence_step_events {
                    mode_registry.event_hub.emit(&event);
                }

                if let Some(mode_name) = mode_change_event {
                    mode_registry
                        .event_hub
                        .emit(&HotkeyEvent::ModeChanged(mode_name));
                }

                dispatch_callbacks(callbacks);

                if should_forward_event {
                    if let Some(forwarder) = key_event_forwarder.as_mut() {
                        if let Err(err) = forwarder.forward_key_event(key, value) {
                            tracing::warn!("Failed forwarding key event: {}", err);
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::collections::HashSet;
    use std::path::PathBuf;
    use std::sync::atomic::AtomicBool;
    use std::sync::atomic::AtomicUsize;
    use std::sync::atomic::Ordering;
    use std::sync::Arc;
    use std::sync::Mutex;
    use std::time::Duration;
    use std::time::Instant;

    use super::device::ModifierTracker;
    use super::dispatch::active_modifier_signature;
    use super::dispatch::collect_callbacks_for_synthetic_keys;
    use super::dispatch::collect_device_specific_dispatch;
    use super::dispatch::collect_due_hold_callbacks;
    use super::dispatch::collect_non_modifier_callbacks;
    use super::dispatch::collect_non_modifier_dispatch;
    use super::dispatch::dispatch_callbacks;
    use super::dispatch::invoke_callback;
    use super::dispatch::should_forward_key_event_in_grab_mode;
    use super::dispatch::suppress_sequence_followup_key_event;
    use super::hotplug::classify_hotplug_change;
    use super::hotplug::parse_hotplug_events;
    use super::hotplug::HotplugFsEvent;
    use super::hotplug::HotplugPathChange;
    use super::hotplug::RawFdGuard;
    use super::io::poll_ready_sources;
    use super::io::should_ignore_device;
    use super::io::update_pressed_key_state;
    use super::listener_loop;
    use super::sequence::SequenceRuntime;
    use super::state::ListenerConfig;
    use super::state::ListenerState;
    use super::state::POLL_TIMEOUT_MS;
    use super::state::VIRTUAL_FORWARDER_DEVICE_NAME;
    use crate::device::DeviceFilter;
    use crate::device::DeviceInfo;
    use crate::events::HotkeyEvent;
    use crate::key::Key;
    use crate::key::Modifier;
    use crate::key_state::SharedKeyState;
    use crate::manager::Callback;
    use crate::manager::DeviceHotkeyRegistration;
    use crate::manager::DeviceRegistrationId;
    use crate::manager::HotkeyCallbacks;
    use crate::manager::HotkeyKey;
    use crate::manager::HotkeyRegistration;
    use crate::manager::RepeatBehavior;
    use crate::manager::SequenceRegistration;
    use crate::mode::ModeRegistry;

    #[test]
    fn modifier_signature_normalizes_left_and_right() {
        let active: HashSet<Modifier> = [Modifier::Ctrl, Modifier::Shift].iter().copied().collect();

        let signature = active_modifier_signature(&active);
        assert_eq!(signature, vec![Modifier::Ctrl, Modifier::Shift]);
    }

    #[test]
    fn empty_modifier_signature_is_empty() {
        let active = HashSet::new();
        assert!(active_modifier_signature(&active).is_empty());
    }

    #[test]
    fn invoke_callback_reports_panic_without_propagating() {
        let callback: Callback = Arc::new(|| panic!("boom"));

        assert!(invoke_callback(&callback));
    }

    #[test]
    fn invoke_callback_runs_non_panicking_callback() {
        let called = Arc::new(AtomicBool::new(false));
        let called_clone = called.clone();
        let callback: Callback = Arc::new(move || {
            called_clone.store(true, Ordering::SeqCst);
        });

        assert!(!invoke_callback(&callback));
        assert!(called.load(Ordering::SeqCst));
    }

    fn sequence_registration(
        steps: Vec<HotkeyKey>,
        timeout: Duration,
        abort_key: Key,
        timeout_fallback: Option<HotkeyKey>,
        counter: Arc<AtomicUsize>,
    ) -> SequenceRegistration {
        SequenceRegistration {
            steps,
            callback: Arc::new(move || {
                counter.fetch_add(1, Ordering::SeqCst);
            }),
            timeout,
            abort_key,
            timeout_fallback,
        }
    }

    fn no_release_callbacks(counter: Arc<AtomicUsize>) -> HotkeyCallbacks {
        HotkeyCallbacks {
            on_press: Arc::new(move || {
                counter.fetch_add(1, Ordering::SeqCst);
            }),
            on_release: None,
            wait_for_release: false,
            min_hold: None,
            repeat_behavior: RepeatBehavior::Ignore,
            passthrough: false,
        }
    }

    #[test]
    fn sequence_completes_within_timeout() {
        let mut runtime = SequenceRuntime::default();
        let t0 = Instant::now();

        let sequence_key_1 = (Key::K, vec![Modifier::Ctrl]);
        let sequence_key_2 = (Key::C, vec![Modifier::Ctrl]);

        let sequence_count = Arc::new(AtomicUsize::new(0));

        let mut sequence_registrations = HashMap::new();
        sequence_registrations.insert(
            1,
            sequence_registration(
                vec![sequence_key_1.clone(), sequence_key_2.clone()],
                Duration::from_millis(50),
                Key::Escape,
                None,
                sequence_count.clone(),
            ),
        );

        let registrations = HashMap::new();

        runtime.on_key_press(sequence_key_1, t0, &registrations, &sequence_registrations);

        let dispatch = runtime.on_key_press(
            sequence_key_2,
            t0 + Duration::from_millis(20),
            &registrations,
            &sequence_registrations,
        );

        dispatch_callbacks(dispatch.callbacks);
        assert_eq!(sequence_count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn sequence_start_emits_step_event() {
        let mut runtime = SequenceRuntime::default();
        let t0 = Instant::now();

        let first_step = (Key::K, vec![Modifier::Ctrl]);
        let second_step = (Key::C, vec![Modifier::Ctrl]);

        let mut sequence_registrations = HashMap::new();
        sequence_registrations.insert(
            7,
            sequence_registration(
                vec![first_step.clone(), second_step],
                Duration::from_millis(100),
                Key::Escape,
                None,
                Arc::new(AtomicUsize::new(0)),
            ),
        );

        let registrations = HashMap::new();
        let dispatch =
            runtime.on_key_press(first_step, t0, &registrations, &sequence_registrations);

        assert_eq!(
            dispatch.step_events,
            vec![HotkeyEvent::SequenceStep {
                id: 7,
                step: 1,
                total: 2,
            }],
        );
    }

    #[test]
    fn sequence_progress_emits_intermediate_step_event() {
        let mut runtime = SequenceRuntime::default();
        let t0 = Instant::now();

        let first_step = (Key::K, vec![Modifier::Ctrl]);
        let second_step = (Key::C, vec![Modifier::Ctrl]);
        let third_step = (Key::D, vec![Modifier::Ctrl]);

        let mut sequence_registrations = HashMap::new();
        sequence_registrations.insert(
            11,
            sequence_registration(
                vec![first_step.clone(), second_step.clone(), third_step],
                Duration::from_millis(100),
                Key::Escape,
                None,
                Arc::new(AtomicUsize::new(0)),
            ),
        );

        let registrations = HashMap::new();
        runtime.on_key_press(first_step, t0, &registrations, &sequence_registrations);
        let dispatch = runtime.on_key_press(
            second_step,
            t0 + Duration::from_millis(10),
            &registrations,
            &sequence_registrations,
        );

        assert_eq!(
            dispatch.step_events,
            vec![HotkeyEvent::SequenceStep {
                id: 11,
                step: 2,
                total: 3,
            }],
        );
    }

    #[test]
    fn sequence_timeout_clears_pending_state() {
        let mut runtime = SequenceRuntime::default();
        let t0 = Instant::now();

        let sequence_key_1 = (Key::K, vec![Modifier::Ctrl]);
        let sequence_key_2 = (Key::C, vec![Modifier::Ctrl]);

        let sequence_count = Arc::new(AtomicUsize::new(0));
        let mut sequence_registrations = HashMap::new();
        sequence_registrations.insert(
            1,
            sequence_registration(
                vec![sequence_key_1.clone(), sequence_key_2.clone()],
                Duration::from_millis(50),
                Key::Escape,
                None,
                sequence_count.clone(),
            ),
        );

        let registrations = HashMap::new();

        runtime.on_key_press(sequence_key_1, t0, &registrations, &sequence_registrations);
        runtime.on_tick(
            t0 + Duration::from_millis(60),
            &registrations,
            &sequence_registrations,
        );

        let dispatch = runtime.on_key_press(
            sequence_key_2,
            t0 + Duration::from_millis(65),
            &registrations,
            &sequence_registrations,
        );

        dispatch_callbacks(dispatch.callbacks);
        assert_eq!(sequence_count.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn wrong_key_resets_sequence() {
        let mut runtime = SequenceRuntime::default();
        let t0 = Instant::now();

        let sequence_key_1 = (Key::K, vec![Modifier::Ctrl]);
        let sequence_key_2 = (Key::C, vec![Modifier::Ctrl]);
        let wrong_key = (Key::X, vec![Modifier::Ctrl]);

        let sequence_count = Arc::new(AtomicUsize::new(0));
        let mut sequence_registrations = HashMap::new();
        sequence_registrations.insert(
            1,
            sequence_registration(
                vec![sequence_key_1.clone(), sequence_key_2.clone()],
                Duration::from_millis(100),
                Key::Escape,
                None,
                sequence_count.clone(),
            ),
        );

        let registrations = HashMap::new();

        runtime.on_key_press(sequence_key_1, t0, &registrations, &sequence_registrations);
        runtime.on_key_press(
            wrong_key,
            t0 + Duration::from_millis(5),
            &registrations,
            &sequence_registrations,
        );

        let dispatch = runtime.on_key_press(
            sequence_key_2,
            t0 + Duration::from_millis(10),
            &registrations,
            &sequence_registrations,
        );

        dispatch_callbacks(dispatch.callbacks);
        assert_eq!(sequence_count.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn standalone_first_step_fires_on_timeout() {
        let mut runtime = SequenceRuntime::default();
        let t0 = Instant::now();

        let sequence_key_1 = (Key::K, vec![Modifier::Ctrl]);
        let sequence_key_2 = (Key::C, vec![Modifier::Ctrl]);

        let standalone_count = Arc::new(AtomicUsize::new(0));
        let sequence_count = Arc::new(AtomicUsize::new(0));

        let mut registrations = HashMap::new();
        registrations.insert(
            sequence_key_1.clone(),
            HotkeyRegistration {
                callbacks: no_release_callbacks(standalone_count.clone()),
            },
        );

        let mut sequence_registrations = HashMap::new();
        sequence_registrations.insert(
            1,
            sequence_registration(
                vec![sequence_key_1.clone(), sequence_key_2],
                Duration::from_millis(50),
                Key::Escape,
                None,
                sequence_count,
            ),
        );

        let press_dispatch =
            runtime.on_key_press(sequence_key_1, t0, &registrations, &sequence_registrations);
        assert!(press_dispatch.suppress_current_key_press);

        let timeout_dispatch = runtime.on_tick(
            t0 + Duration::from_millis(55),
            &registrations,
            &sequence_registrations,
        );

        dispatch_callbacks(timeout_dispatch.callbacks);

        assert_eq!(standalone_count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn standalone_first_step_timeout_dispatches_release_when_released_before_timeout() {
        let mut runtime = SequenceRuntime::default();
        let t0 = Instant::now();

        let sequence_key_1 = (Key::K, vec![Modifier::Ctrl]);
        let sequence_key_2 = (Key::C, vec![Modifier::Ctrl]);

        let press_count = Arc::new(AtomicUsize::new(0));
        let release_count = Arc::new(AtomicUsize::new(0));
        let press_count_clone = press_count.clone();
        let release_count_clone = release_count.clone();

        let mut registrations = HashMap::new();
        registrations.insert(
            sequence_key_1.clone(),
            HotkeyRegistration {
                callbacks: HotkeyCallbacks {
                    on_press: Arc::new(move || {
                        press_count_clone.fetch_add(1, Ordering::SeqCst);
                    }),
                    on_release: Some(Arc::new(move || {
                        release_count_clone.fetch_add(1, Ordering::SeqCst);
                    })),
                    wait_for_release: true,
                    min_hold: None,
                    repeat_behavior: RepeatBehavior::Ignore,
                    passthrough: false,
                },
            },
        );

        let mut sequence_registrations = HashMap::new();
        sequence_registrations.insert(
            1,
            sequence_registration(
                vec![sequence_key_1.clone(), sequence_key_2],
                Duration::from_millis(50),
                Key::Escape,
                None,
                Arc::new(AtomicUsize::new(0)),
            ),
        );

        runtime.on_key_press(
            sequence_key_1.clone(),
            t0,
            &registrations,
            &sequence_registrations,
        );
        runtime.on_key_release(sequence_key_1.0, t0 + Duration::from_millis(10));

        let timeout_dispatch = runtime.on_tick(
            t0 + Duration::from_millis(55),
            &registrations,
            &sequence_registrations,
        );
        dispatch_callbacks(timeout_dispatch.callbacks);

        assert_eq!(press_count.load(Ordering::SeqCst), 1);
        assert_eq!(release_count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn standalone_first_step_timeout_dispatches_release_after_timeout_when_key_is_released() {
        let mut runtime = SequenceRuntime::default();
        let t0 = Instant::now();

        let sequence_key_1 = (Key::K, vec![Modifier::Ctrl]);
        let sequence_key_2 = (Key::C, vec![Modifier::Ctrl]);

        let press_count = Arc::new(AtomicUsize::new(0));
        let release_count = Arc::new(AtomicUsize::new(0));
        let press_count_clone = press_count.clone();
        let release_count_clone = release_count.clone();

        let mut registrations = HashMap::new();
        registrations.insert(
            sequence_key_1.clone(),
            HotkeyRegistration {
                callbacks: HotkeyCallbacks {
                    on_press: Arc::new(move || {
                        press_count_clone.fetch_add(1, Ordering::SeqCst);
                    }),
                    on_release: Some(Arc::new(move || {
                        release_count_clone.fetch_add(1, Ordering::SeqCst);
                    })),
                    wait_for_release: true,
                    min_hold: None,
                    repeat_behavior: RepeatBehavior::Ignore,
                    passthrough: false,
                },
            },
        );

        let mut sequence_registrations = HashMap::new();
        sequence_registrations.insert(
            1,
            sequence_registration(
                vec![sequence_key_1.clone(), sequence_key_2],
                Duration::from_millis(50),
                Key::Escape,
                None,
                Arc::new(AtomicUsize::new(0)),
            ),
        );

        runtime.on_key_press(
            sequence_key_1.clone(),
            t0,
            &registrations,
            &sequence_registrations,
        );

        let timeout_dispatch = runtime.on_tick(
            t0 + Duration::from_millis(55),
            &registrations,
            &sequence_registrations,
        );
        dispatch_callbacks(timeout_dispatch.callbacks);
        assert_eq!(press_count.load(Ordering::SeqCst), 1);
        assert_eq!(release_count.load(Ordering::SeqCst), 0);

        runtime.on_key_release(sequence_key_1.0, t0 + Duration::from_millis(80));

        let release_dispatch = runtime.on_tick(
            t0 + Duration::from_millis(81),
            &registrations,
            &sequence_registrations,
        );
        dispatch_callbacks(release_dispatch.callbacks);

        assert_eq!(press_count.load(Ordering::SeqCst), 1);
        assert_eq!(release_count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn standalone_timeout_does_not_wait_for_internal_release_observer() {
        let mut runtime = SequenceRuntime::default();
        let t0 = Instant::now();

        let sequence_key_1 = (Key::K, vec![Modifier::Ctrl]);
        let sequence_key_2 = (Key::C, vec![Modifier::Ctrl]);

        let press_count = Arc::new(AtomicUsize::new(0));
        let press_count_clone = press_count.clone();

        let mut registrations = HashMap::new();
        registrations.insert(
            sequence_key_1.clone(),
            HotkeyRegistration {
                callbacks: HotkeyCallbacks {
                    on_press: Arc::new(move || {
                        press_count_clone.fetch_add(1, Ordering::SeqCst);
                    }),
                    on_release: Some(Arc::new(|| {})),
                    wait_for_release: false,
                    min_hold: None,
                    repeat_behavior: RepeatBehavior::Ignore,
                    passthrough: false,
                },
            },
        );

        let mut sequence_registrations = HashMap::new();
        sequence_registrations.insert(
            1,
            sequence_registration(
                vec![sequence_key_1.clone(), sequence_key_2],
                Duration::from_millis(50),
                Key::Escape,
                None,
                Arc::new(AtomicUsize::new(0)),
            ),
        );

        runtime.on_key_press(sequence_key_1, t0, &registrations, &sequence_registrations);

        let timeout_dispatch = runtime.on_tick(
            t0 + Duration::from_millis(55),
            &registrations,
            &sequence_registrations,
        );
        dispatch_callbacks(timeout_dispatch.callbacks);

        assert_eq!(press_count.load(Ordering::SeqCst), 1);
        assert!(runtime.pending_standalone.is_none());
    }

    #[test]
    fn internal_release_observer_dispatches_on_key_release_after_timeout() {
        let mut runtime = SequenceRuntime::default();
        let t0 = Instant::now();

        let sequence_key_1 = (Key::K, vec![Modifier::Ctrl]);
        let sequence_key_2 = (Key::C, vec![Modifier::Ctrl]);

        let press_count = Arc::new(AtomicUsize::new(0));
        let release_count = Arc::new(AtomicUsize::new(0));
        let press_count_clone = press_count.clone();
        let release_count_clone = release_count.clone();

        let mut registrations = HashMap::new();
        registrations.insert(
            sequence_key_1.clone(),
            HotkeyRegistration {
                callbacks: HotkeyCallbacks {
                    on_press: Arc::new(move || {
                        press_count_clone.fetch_add(1, Ordering::SeqCst);
                    }),
                    on_release: Some(Arc::new(move || {
                        release_count_clone.fetch_add(1, Ordering::SeqCst);
                    })),
                    wait_for_release: false,
                    min_hold: None,
                    repeat_behavior: RepeatBehavior::Ignore,
                    passthrough: false,
                },
            },
        );

        let mut sequence_registrations = HashMap::new();
        sequence_registrations.insert(
            1,
            sequence_registration(
                vec![sequence_key_1.clone(), sequence_key_2],
                Duration::from_millis(50),
                Key::Escape,
                None,
                Arc::new(AtomicUsize::new(0)),
            ),
        );

        runtime.on_key_press(
            sequence_key_1.clone(),
            t0,
            &registrations,
            &sequence_registrations,
        );

        let timeout_dispatch = runtime.on_tick(
            t0 + Duration::from_millis(55),
            &registrations,
            &sequence_registrations,
        );
        dispatch_callbacks(timeout_dispatch.callbacks);

        assert_eq!(press_count.load(Ordering::SeqCst), 1);
        assert_eq!(release_count.load(Ordering::SeqCst), 0);
        assert!(runtime.pending_standalone.is_none());

        let release_callbacks =
            runtime.on_key_release(sequence_key_1.0, t0 + Duration::from_millis(70));
        dispatch_callbacks(release_callbacks);

        assert_eq!(release_count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn standalone_first_step_timeout_respects_min_hold_when_released_early() {
        let mut runtime = SequenceRuntime::default();
        let t0 = Instant::now();

        let sequence_key_1 = (Key::K, vec![Modifier::Ctrl]);
        let sequence_key_2 = (Key::C, vec![Modifier::Ctrl]);

        let standalone_count = Arc::new(AtomicUsize::new(0));
        let standalone_count_clone = standalone_count.clone();

        let mut registrations = HashMap::new();
        registrations.insert(
            sequence_key_1.clone(),
            HotkeyRegistration {
                callbacks: HotkeyCallbacks {
                    on_press: Arc::new(move || {
                        standalone_count_clone.fetch_add(1, Ordering::SeqCst);
                    }),
                    on_release: None,
                    wait_for_release: false,
                    min_hold: Some(Duration::from_millis(100)),
                    repeat_behavior: RepeatBehavior::Ignore,
                    passthrough: false,
                },
            },
        );

        let mut sequence_registrations = HashMap::new();
        sequence_registrations.insert(
            1,
            sequence_registration(
                vec![sequence_key_1.clone(), sequence_key_2],
                Duration::from_millis(50),
                Key::Escape,
                None,
                Arc::new(AtomicUsize::new(0)),
            ),
        );

        runtime.on_key_press(
            sequence_key_1.clone(),
            t0,
            &registrations,
            &sequence_registrations,
        );
        runtime.on_key_release(sequence_key_1.0, t0 + Duration::from_millis(30));

        let timeout_dispatch = runtime.on_tick(
            t0 + Duration::from_millis(60),
            &registrations,
            &sequence_registrations,
        );
        dispatch_callbacks(timeout_dispatch.callbacks);

        let later_dispatch = runtime.on_tick(
            t0 + Duration::from_millis(120),
            &registrations,
            &sequence_registrations,
        );
        dispatch_callbacks(later_dispatch.callbacks);

        assert_eq!(standalone_count.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn standalone_first_step_timeout_waits_for_min_hold_when_still_pressed() {
        let mut runtime = SequenceRuntime::default();
        let t0 = Instant::now();

        let sequence_key_1 = (Key::K, vec![Modifier::Ctrl]);
        let sequence_key_2 = (Key::C, vec![Modifier::Ctrl]);

        let standalone_count = Arc::new(AtomicUsize::new(0));
        let standalone_count_clone = standalone_count.clone();

        let mut registrations = HashMap::new();
        registrations.insert(
            sequence_key_1.clone(),
            HotkeyRegistration {
                callbacks: HotkeyCallbacks {
                    on_press: Arc::new(move || {
                        standalone_count_clone.fetch_add(1, Ordering::SeqCst);
                    }),
                    on_release: None,
                    wait_for_release: false,
                    min_hold: Some(Duration::from_millis(100)),
                    repeat_behavior: RepeatBehavior::Ignore,
                    passthrough: false,
                },
            },
        );

        let mut sequence_registrations = HashMap::new();
        sequence_registrations.insert(
            1,
            sequence_registration(
                vec![sequence_key_1.clone(), sequence_key_2],
                Duration::from_millis(50),
                Key::Escape,
                None,
                Arc::new(AtomicUsize::new(0)),
            ),
        );

        runtime.on_key_press(sequence_key_1, t0, &registrations, &sequence_registrations);

        let first_timeout_dispatch = runtime.on_tick(
            t0 + Duration::from_millis(60),
            &registrations,
            &sequence_registrations,
        );
        dispatch_callbacks(first_timeout_dispatch.callbacks);

        let hold_dispatch = runtime.on_tick(
            t0 + Duration::from_millis(120),
            &registrations,
            &sequence_registrations,
        );
        dispatch_callbacks(hold_dispatch.callbacks);

        assert_eq!(standalone_count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn abort_key_cancels_in_progress_sequences() {
        let mut runtime = SequenceRuntime::default();
        let t0 = Instant::now();

        let sequence_key_1 = (Key::K, vec![Modifier::Ctrl]);
        let sequence_key_2 = (Key::C, vec![Modifier::Ctrl]);

        let sequence_count = Arc::new(AtomicUsize::new(0));
        let mut sequence_registrations = HashMap::new();
        sequence_registrations.insert(
            1,
            sequence_registration(
                vec![sequence_key_1.clone(), sequence_key_2.clone()],
                Duration::from_millis(100),
                Key::Q,
                None,
                sequence_count.clone(),
            ),
        );

        let registrations = HashMap::new();

        runtime.on_key_press(sequence_key_1, t0, &registrations, &sequence_registrations);
        runtime.on_key_press(
            (Key::Q, vec![]),
            t0 + Duration::from_millis(10),
            &registrations,
            &sequence_registrations,
        );

        let dispatch = runtime.on_key_press(
            sequence_key_2,
            t0 + Duration::from_millis(20),
            &registrations,
            &sequence_registrations,
        );

        dispatch_callbacks(dispatch.callbacks);
        assert_eq!(sequence_count.load(Ordering::SeqCst), 0);
    }

    #[test]
    #[allow(clippy::similar_names)]
    fn multiple_sequences_share_prefix_without_interference() {
        let mut runtime = SequenceRuntime::default();
        let t0 = Instant::now();

        let first_step = (Key::K, vec![Modifier::Ctrl]);
        let complete_a = (Key::C, vec![Modifier::Ctrl]);
        let complete_b = (Key::U, vec![Modifier::Ctrl]);

        let sequence_a_count = Arc::new(AtomicUsize::new(0));
        let sequence_b_count = Arc::new(AtomicUsize::new(0));

        let mut sequence_registrations = HashMap::new();
        sequence_registrations.insert(
            1,
            sequence_registration(
                vec![first_step.clone(), complete_a.clone()],
                Duration::from_millis(100),
                Key::Escape,
                None,
                sequence_a_count.clone(),
            ),
        );
        sequence_registrations.insert(
            2,
            sequence_registration(
                vec![first_step.clone(), complete_b],
                Duration::from_millis(100),
                Key::Escape,
                None,
                sequence_b_count.clone(),
            ),
        );

        let registrations = HashMap::new();

        runtime.on_key_press(first_step, t0, &registrations, &sequence_registrations);
        let dispatch = runtime.on_key_press(
            complete_a,
            t0 + Duration::from_millis(10),
            &registrations,
            &sequence_registrations,
        );

        dispatch_callbacks(dispatch.callbacks);

        assert_eq!(sequence_a_count.load(Ordering::SeqCst), 1);
        assert_eq!(sequence_b_count.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn timeout_fallback_dispatches_synthetic_hotkey() {
        let mut runtime = SequenceRuntime::default();
        let t0 = Instant::now();

        let sequence_key_1 = (Key::K, vec![Modifier::Ctrl]);
        let sequence_key_2 = (Key::C, vec![Modifier::Ctrl]);
        let fallback_key = (Key::F, vec![]);

        let fallback_count = Arc::new(AtomicUsize::new(0));

        let mut registrations = HashMap::new();
        registrations.insert(
            fallback_key.clone(),
            HotkeyRegistration {
                callbacks: no_release_callbacks(fallback_count.clone()),
            },
        );

        let mut sequence_registrations = HashMap::new();
        sequence_registrations.insert(
            1,
            sequence_registration(
                vec![sequence_key_1.clone(), sequence_key_2],
                Duration::from_millis(50),
                Key::Escape,
                Some(fallback_key),
                Arc::new(AtomicUsize::new(0)),
            ),
        );

        runtime.on_key_press(sequence_key_1, t0, &registrations, &sequence_registrations);

        let timeout_dispatch = runtime.on_tick(
            t0 + Duration::from_millis(60),
            &registrations,
            &sequence_registrations,
        );
        let mut callbacks = timeout_dispatch.callbacks;
        callbacks.extend(collect_callbacks_for_synthetic_keys(
            &timeout_dispatch.synthetic_keys,
            &registrations,
        ));

        dispatch_callbacks(callbacks);
        assert_eq!(fallback_count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn release_callback_runs_after_press() {
        let press_count = Arc::new(AtomicUsize::new(0));
        let release_count = Arc::new(AtomicUsize::new(0));
        let p = press_count.clone();
        let r = release_count.clone();

        let callbacks = HotkeyCallbacks {
            on_press: Arc::new(move || {
                p.fetch_add(1, Ordering::SeqCst);
            }),
            on_release: Some(Arc::new(move || {
                r.fetch_add(1, Ordering::SeqCst);
            })),
            wait_for_release: true,
            min_hold: None,
            repeat_behavior: RepeatBehavior::Ignore,
            passthrough: false,
        };

        let mut registrations = HashMap::new();
        registrations.insert(
            (Key::A, vec![Modifier::Ctrl]),
            HotkeyRegistration { callbacks },
        );

        let modifiers: HashSet<Modifier> = [Modifier::Ctrl].into_iter().collect();
        let mut active_presses = HashMap::new();
        let t0 = Instant::now();

        dispatch_callbacks(collect_non_modifier_callbacks(
            Key::A,
            1,
            t0,
            &modifiers,
            &registrations,
            &mut active_presses,
            false,
        ));
        dispatch_callbacks(collect_non_modifier_callbacks(
            Key::A,
            0,
            t0 + Duration::from_millis(10),
            &modifiers,
            &registrations,
            &mut active_presses,
            false,
        ));

        assert_eq!(press_count.load(Ordering::SeqCst), 1);
        assert_eq!(release_count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn suppressed_sequence_followup_events_are_consumed_until_release() {
        let key = Key::A;
        let mut suppressed = HashSet::new();

        assert!(!suppress_sequence_followup_key_event(
            &mut suppressed,
            key,
            1,
            true,
        ));
        assert!(suppressed.contains(&key));

        assert!(suppress_sequence_followup_key_event(
            &mut suppressed,
            key,
            2,
            false,
        ));
        assert!(suppressed.contains(&key));

        assert!(suppress_sequence_followup_key_event(
            &mut suppressed,
            key,
            0,
            false,
        ));
        assert!(!suppressed.contains(&key));
    }

    #[test]
    fn grabbed_hotkey_without_passthrough_is_consumed() {
        let callbacks = HotkeyCallbacks {
            on_press: Arc::new(|| {}),
            on_release: None,
            wait_for_release: false,
            min_hold: None,
            repeat_behavior: RepeatBehavior::Ignore,
            passthrough: false,
        };

        let mut registrations = HashMap::new();
        registrations.insert((Key::A, vec![]), HotkeyRegistration { callbacks });

        let modifiers = HashSet::new();
        let mut active_presses = HashMap::new();
        let dispatch = collect_non_modifier_dispatch(
            Key::A,
            1,
            Instant::now(),
            &modifiers,
            &registrations,
            &mut active_presses,
            false,
        );

        assert!(dispatch.matched_hotkey);
        assert!(!dispatch.passthrough);
        assert!(!should_forward_key_event_in_grab_mode(
            true,
            dispatch.matched_hotkey,
            dispatch.passthrough,
        ));
    }

    #[test]
    fn grabbed_hotkey_with_passthrough_is_forwarded() {
        let callbacks = HotkeyCallbacks {
            on_press: Arc::new(|| {}),
            on_release: None,
            wait_for_release: false,
            min_hold: None,
            repeat_behavior: RepeatBehavior::Ignore,
            passthrough: true,
        };

        let mut registrations = HashMap::new();
        registrations.insert((Key::A, vec![]), HotkeyRegistration { callbacks });

        let modifiers = HashSet::new();
        let mut active_presses = HashMap::new();
        let dispatch = collect_non_modifier_dispatch(
            Key::A,
            1,
            Instant::now(),
            &modifiers,
            &registrations,
            &mut active_presses,
            false,
        );

        assert!(dispatch.matched_hotkey);
        assert!(dispatch.passthrough);
        assert!(should_forward_key_event_in_grab_mode(
            true,
            dispatch.matched_hotkey,
            dispatch.passthrough,
        ));
    }

    #[test]
    fn min_hold_delays_press_callback_until_release() {
        let press_count = Arc::new(AtomicUsize::new(0));
        let p = press_count.clone();

        let callbacks = HotkeyCallbacks {
            on_press: Arc::new(move || {
                p.fetch_add(1, Ordering::SeqCst);
            }),
            on_release: None,
            wait_for_release: false,
            min_hold: Some(Duration::from_millis(50)),
            repeat_behavior: RepeatBehavior::Ignore,
            passthrough: false,
        };

        let mut registrations = HashMap::new();
        registrations.insert((Key::A, vec![]), HotkeyRegistration { callbacks });

        let modifiers = HashSet::new();
        let mut active_presses = HashMap::new();
        let t0 = Instant::now();

        dispatch_callbacks(collect_non_modifier_callbacks(
            Key::A,
            1,
            t0,
            &modifiers,
            &registrations,
            &mut active_presses,
            false,
        ));
        dispatch_callbacks(collect_non_modifier_callbacks(
            Key::A,
            0,
            t0 + Duration::from_millis(20),
            &modifiers,
            &registrations,
            &mut active_presses,
            false,
        ));
        dispatch_callbacks(collect_non_modifier_callbacks(
            Key::A,
            1,
            t0,
            &modifiers,
            &registrations,
            &mut active_presses,
            false,
        ));
        dispatch_callbacks(collect_non_modifier_callbacks(
            Key::A,
            0,
            t0 + Duration::from_millis(70),
            &modifiers,
            &registrations,
            &mut active_presses,
            false,
        ));

        assert_eq!(press_count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn min_hold_dispatches_on_tick_before_release() {
        let press_count = Arc::new(AtomicUsize::new(0));
        let p = press_count.clone();

        let callbacks = HotkeyCallbacks {
            on_press: Arc::new(move || {
                p.fetch_add(1, Ordering::SeqCst);
            }),
            on_release: None,
            wait_for_release: false,
            min_hold: Some(Duration::from_millis(50)),
            repeat_behavior: RepeatBehavior::Ignore,
            passthrough: false,
        };

        let mut registrations = HashMap::new();
        registrations.insert((Key::A, vec![]), HotkeyRegistration { callbacks });

        let modifiers = HashSet::new();
        let mut active_presses = HashMap::new();
        let t0 = Instant::now();

        dispatch_callbacks(collect_non_modifier_callbacks(
            Key::A,
            1,
            t0,
            &modifiers,
            &registrations,
            &mut active_presses,
            false,
        ));

        dispatch_callbacks(collect_due_hold_callbacks(
            t0 + Duration::from_millis(60),
            &registrations,
            &HashMap::new(),
            &HashMap::new(),
            &mut active_presses,
        ));

        dispatch_callbacks(collect_non_modifier_callbacks(
            Key::A,
            0,
            t0 + Duration::from_millis(70),
            &modifiers,
            &registrations,
            &mut active_presses,
            false,
        ));

        assert_eq!(press_count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn repeat_event_respects_min_hold_threshold() {
        let press_count = Arc::new(AtomicUsize::new(0));
        let p = press_count.clone();

        let callbacks = HotkeyCallbacks {
            on_press: Arc::new(move || {
                p.fetch_add(1, Ordering::SeqCst);
            }),
            on_release: None,
            wait_for_release: false,
            min_hold: Some(Duration::from_millis(50)),
            repeat_behavior: RepeatBehavior::Trigger,
            passthrough: false,
        };

        let mut registrations = HashMap::new();
        registrations.insert((Key::A, vec![]), HotkeyRegistration { callbacks });

        let modifiers = HashSet::new();
        let mut active_presses = HashMap::new();
        let now = Instant::now();

        dispatch_callbacks(collect_non_modifier_callbacks(
            Key::A,
            1,
            now,
            &modifiers,
            &registrations,
            &mut active_presses,
            false,
        ));

        dispatch_callbacks(collect_non_modifier_callbacks(
            Key::A,
            2,
            now + Duration::from_millis(20),
            &modifiers,
            &registrations,
            &mut active_presses,
            false,
        ));

        dispatch_callbacks(collect_non_modifier_callbacks(
            Key::A,
            2,
            now + Duration::from_millis(60),
            &modifiers,
            &registrations,
            &mut active_presses,
            false,
        ));

        assert_eq!(press_count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn zero_min_hold_triggers_press_on_key_down_only_once() {
        let press_count = Arc::new(AtomicUsize::new(0));
        let p = press_count.clone();

        let callbacks = HotkeyCallbacks {
            on_press: Arc::new(move || {
                p.fetch_add(1, Ordering::SeqCst);
            }),
            on_release: None,
            wait_for_release: false,
            min_hold: Some(Duration::ZERO),
            repeat_behavior: RepeatBehavior::Ignore,
            passthrough: false,
        };

        let mut registrations = HashMap::new();
        registrations.insert((Key::A, vec![]), HotkeyRegistration { callbacks });

        let modifiers = HashSet::new();
        let mut active_presses = HashMap::new();
        let now = Instant::now();

        dispatch_callbacks(collect_non_modifier_callbacks(
            Key::A,
            1,
            now,
            &modifiers,
            &registrations,
            &mut active_presses,
            false,
        ));
        dispatch_callbacks(collect_non_modifier_callbacks(
            Key::A,
            0,
            now + Duration::from_millis(1),
            &modifiers,
            &registrations,
            &mut active_presses,
            false,
        ));

        assert_eq!(press_count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn repeat_after_hold_does_not_double_fire_on_release() {
        let press_count = Arc::new(AtomicUsize::new(0));
        let p = press_count.clone();

        let callbacks = HotkeyCallbacks {
            on_press: Arc::new(move || {
                p.fetch_add(1, Ordering::SeqCst);
            }),
            on_release: None,
            wait_for_release: false,
            min_hold: Some(Duration::from_millis(50)),
            repeat_behavior: RepeatBehavior::Trigger,
            passthrough: false,
        };

        let mut registrations = HashMap::new();
        registrations.insert((Key::A, vec![]), HotkeyRegistration { callbacks });

        let modifiers = HashSet::new();
        let mut active_presses = HashMap::new();
        let now = Instant::now();

        dispatch_callbacks(collect_non_modifier_callbacks(
            Key::A,
            1,
            now,
            &modifiers,
            &registrations,
            &mut active_presses,
            false,
        ));
        dispatch_callbacks(collect_non_modifier_callbacks(
            Key::A,
            2,
            now + Duration::from_millis(60),
            &modifiers,
            &registrations,
            &mut active_presses,
            false,
        ));
        dispatch_callbacks(collect_non_modifier_callbacks(
            Key::A,
            0,
            now + Duration::from_millis(70),
            &modifiers,
            &registrations,
            &mut active_presses,
            false,
        ));

        assert_eq!(press_count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn repeat_event_respects_trigger_option() {
        let press_count = Arc::new(AtomicUsize::new(0));
        let p = press_count.clone();

        let callbacks = HotkeyCallbacks {
            on_press: Arc::new(move || {
                p.fetch_add(1, Ordering::SeqCst);
            }),
            on_release: None,
            wait_for_release: false,
            min_hold: None,
            repeat_behavior: RepeatBehavior::Trigger,
            passthrough: false,
        };

        let mut registrations = HashMap::new();
        registrations.insert((Key::A, vec![]), HotkeyRegistration { callbacks });

        let modifiers = HashSet::new();
        let mut active_presses = HashMap::new();
        let now = Instant::now();

        dispatch_callbacks(collect_non_modifier_callbacks(
            Key::A,
            1,
            now,
            &modifiers,
            &registrations,
            &mut active_presses,
            false,
        ));
        dispatch_callbacks(collect_non_modifier_callbacks(
            Key::A,
            2,
            now + Duration::from_millis(1),
            &modifiers,
            &registrations,
            &mut active_presses,
            false,
        ));

        assert_eq!(press_count.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn repeat_event_is_ignored_when_not_enabled() {
        let press_count = Arc::new(AtomicUsize::new(0));
        let p = press_count.clone();

        let callbacks = HotkeyCallbacks {
            on_press: Arc::new(move || {
                p.fetch_add(1, Ordering::SeqCst);
            }),
            on_release: None,
            wait_for_release: false,
            min_hold: None,
            repeat_behavior: RepeatBehavior::Ignore,
            passthrough: false,
        };

        let mut registrations = HashMap::new();
        registrations.insert((Key::A, vec![]), HotkeyRegistration { callbacks });

        let modifiers = HashSet::new();
        let mut active_presses = HashMap::new();
        let now = Instant::now();

        dispatch_callbacks(collect_non_modifier_callbacks(
            Key::A,
            1,
            now,
            &modifiers,
            &registrations,
            &mut active_presses,
            false,
        ));
        dispatch_callbacks(collect_non_modifier_callbacks(
            Key::A,
            2,
            now + Duration::from_millis(1),
            &modifiers,
            &registrations,
            &mut active_presses,
            false,
        ));

        assert_eq!(press_count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn burst_repeat_events_dispatch_without_dropping_callbacks() {
        let press_count = Arc::new(AtomicUsize::new(0));
        let p = press_count.clone();

        let callbacks = HotkeyCallbacks {
            on_press: Arc::new(move || {
                p.fetch_add(1, Ordering::SeqCst);
            }),
            on_release: None,
            wait_for_release: false,
            min_hold: None,
            repeat_behavior: RepeatBehavior::Trigger,
            passthrough: false,
        };

        let mut registrations = HashMap::new();
        registrations.insert((Key::A, vec![]), HotkeyRegistration { callbacks });

        let modifiers = HashSet::new();
        let mut active_presses = HashMap::new();
        let now = Instant::now();

        dispatch_callbacks(collect_non_modifier_callbacks(
            Key::A,
            1,
            now,
            &modifiers,
            &registrations,
            &mut active_presses,
            false,
        ));

        for offset in 1..=64 {
            dispatch_callbacks(collect_non_modifier_callbacks(
                Key::A,
                2,
                now + Duration::from_millis(offset),
                &modifiers,
                &registrations,
                &mut active_presses,
                false,
            ));
        }

        assert_eq!(press_count.load(Ordering::SeqCst), 65);
    }

    #[test]
    fn modifier_tracker_cleans_state_on_disconnect() {
        let mut tracker = ModifierTracker::default();
        let device_a = PathBuf::from("/dev/input/event100");
        let device_b = PathBuf::from("/dev/input/event101");

        tracker.press(&device_a, Modifier::Ctrl);
        tracker.press(&device_b, Modifier::Shift);
        tracker.disconnect(&device_a);

        let active = tracker.active_modifiers();
        assert!(!active.contains(&Modifier::Ctrl));
        assert!(active.contains(&Modifier::Shift));
    }

    #[test]
    fn parse_hotplug_events_extracts_create_and_delete_entries() {
        let mut bytes = Vec::new();

        push_inotify_event(&mut bytes, libc::IN_CREATE, "event42");
        push_inotify_event(&mut bytes, libc::IN_DELETE, "event7");

        let parsed = parse_hotplug_events(&bytes, bytes.len());
        assert_eq!(
            parsed,
            vec![
                HotplugFsEvent {
                    mask: libc::IN_CREATE,
                    device_name: "event42".to_string(),
                },
                HotplugFsEvent {
                    mask: libc::IN_DELETE,
                    device_name: "event7".to_string(),
                },
            ]
        );
    }

    #[test]
    fn parse_hotplug_events_handles_misaligned_buffers() {
        let mut payload = Vec::new();
        push_inotify_event(&mut payload, libc::IN_CREATE, "event9");

        let mut prefixed = vec![0u8];
        prefixed.extend_from_slice(&payload);

        let parsed = parse_hotplug_events(&prefixed[1..], payload.len());
        assert_eq!(
            parsed,
            vec![HotplugFsEvent {
                mask: libc::IN_CREATE,
                device_name: "event9".to_string(),
            }]
        );
    }

    #[test]
    fn classify_hotplug_change_detects_add_and_remove() {
        let mut known_paths = HashSet::new();
        let add = HotplugFsEvent {
            mask: libc::IN_CREATE,
            device_name: "event44".to_string(),
        };

        let added = classify_hotplug_change(&add, &mut known_paths);
        assert_eq!(
            added,
            HotplugPathChange::Added(PathBuf::from("/dev/input/event44"))
        );

        let remove = HotplugFsEvent {
            mask: libc::IN_DELETE,
            device_name: "event44".to_string(),
        };

        let removed = classify_hotplug_change(&remove, &mut known_paths);
        assert_eq!(
            removed,
            HotplugPathChange::Removed(PathBuf::from("/dev/input/event44"))
        );
    }

    #[test]
    fn poll_ready_sources_reports_inotify_fd_errors() {
        let mut fds = [0; 2];
        let pipe_result = unsafe { libc::pipe2(fds.as_mut_ptr(), libc::O_CLOEXEC) };
        assert_eq!(pipe_result, 0);

        let read_fd = RawFdGuard::new(fds[0]);
        let write_fd = RawFdGuard::new(fds[1]);
        drop(write_fd);

        let err = poll_ready_sources(read_fd.raw_fd(), &[]).err().unwrap();
        assert_eq!(err.kind(), std::io::ErrorKind::Other);
    }

    #[test]
    fn listener_sets_stop_flag_on_poll_failure() {
        let mut fds = [0; 2];
        let pipe_result = unsafe { libc::pipe2(fds.as_mut_ptr(), libc::O_CLOEXEC) };
        assert_eq!(pipe_result, 0);

        let read_fd = RawFdGuard::new(fds[0]);
        let write_fd = RawFdGuard::new(fds[1]);
        drop(write_fd);

        let registrations = Arc::new(Mutex::new(HashMap::new()));
        let sequence_registrations = Arc::new(Mutex::new(HashMap::new()));
        let stop_flag = Arc::new(AtomicBool::new(false));

        listener_loop(
            Vec::new(),
            &read_fd,
            ListenerState {
                registrations,
                sequence_registrations,
                device_registrations: Arc::new(Mutex::new(HashMap::new())),
                tap_hold_registrations: Arc::new(Mutex::new(HashMap::new())),
                stop_flag: stop_flag.clone(),
                key_state: SharedKeyState::new(),
                mode_registry: ModeRegistry::new(),
            },
            ListenerConfig::default(),
            None,
        );

        assert!(stop_flag.load(Ordering::SeqCst));
    }

    #[test]
    fn poll_wakes_quickly_when_fd_becomes_readable() {
        let mut fds = [0; 2];
        let pipe_result = unsafe { libc::pipe2(fds.as_mut_ptr(), libc::O_CLOEXEC) };
        assert_eq!(pipe_result, 0);

        let read_fd = RawFdGuard::new(fds[0]);
        let write_fd = RawFdGuard::new(fds[1]);

        let writer = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(5));
            let one = [1u8];
            unsafe {
                libc::write(write_fd.raw_fd(), one.as_ptr().cast::<libc::c_void>(), 1);
            }
        });

        let start = Instant::now();
        let mut pollfds = [libc::pollfd {
            fd: read_fd.raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        }];
        let result = unsafe { libc::poll(pollfds.as_mut_ptr(), 1, 1_000) };

        writer.join().unwrap();

        assert_eq!(result, 1);
        assert!(start.elapsed() < Duration::from_millis(200));
    }

    #[test]
    fn polling_loop_shutdown_is_bounded_by_timeout() {
        let mut fds = [0; 2];
        let pipe_result = unsafe { libc::pipe2(fds.as_mut_ptr(), libc::O_CLOEXEC) };
        assert_eq!(pipe_result, 0);

        let read_fd = RawFdGuard::new(fds[0]);
        let _write_fd = RawFdGuard::new(fds[1]);

        let stop = Arc::new(AtomicBool::new(false));
        let stop_clone = stop.clone();
        let start = Instant::now();

        let join_handle = std::thread::spawn(move || {
            while !stop_clone.load(Ordering::SeqCst) {
                let _ = unsafe {
                    let mut pollfds = [libc::pollfd {
                        fd: read_fd.raw_fd(),
                        events: libc::POLLIN,
                        revents: 0,
                    }];
                    libc::poll(pollfds.as_mut_ptr(), 1, POLL_TIMEOUT_MS)
                };
            }
        });

        std::thread::sleep(Duration::from_millis(10));
        stop.store(true, Ordering::SeqCst);
        join_handle.join().unwrap();

        assert!(start.elapsed() < Duration::from_millis(200));
    }

    fn push_inotify_event(buffer: &mut Vec<u8>, mask: u32, name: &str) {
        let mut name_bytes = name.as_bytes().to_vec();
        name_bytes.push(0);
        while !name_bytes.len().is_multiple_of(4) {
            name_bytes.push(0);
        }

        let event = libc::inotify_event {
            wd: 1,
            mask,
            cookie: 0,
            #[allow(clippy::cast_possible_truncation)]
            len: name_bytes.len() as u32,
        };

        let event_bytes = unsafe {
            std::slice::from_raw_parts(
                (&raw const event).cast::<u8>(),
                size_of::<libc::inotify_event>(),
            )
        };

        buffer.extend_from_slice(event_bytes);
        buffer.extend_from_slice(&name_bytes);
    }

    fn test_device_info(name: &str, vendor: u16, product: u16) -> DeviceInfo {
        DeviceInfo {
            name: name.to_string(),
            vendor,
            product,
        }
    }

    #[test]
    fn internal_virtual_forwarder_device_is_ignored_when_grab_enabled() {
        let info = test_device_info(VIRTUAL_FORWARDER_DEVICE_NAME, 0, 0);
        assert!(should_ignore_device(&info, true));
    }

    #[test]
    fn internal_virtual_forwarder_device_is_not_ignored_without_grab() {
        let info = test_device_info(VIRTUAL_FORWARDER_DEVICE_NAME, 0, 0);
        assert!(!should_ignore_device(&info, false));
    }

    fn device_registration(
        id: DeviceRegistrationId,
        hotkey_key: HotkeyKey,
        filter: DeviceFilter,
        counter: Arc<AtomicUsize>,
    ) -> (DeviceRegistrationId, DeviceHotkeyRegistration) {
        (
            id,
            DeviceHotkeyRegistration {
                hotkey_key,
                filter,
                callbacks: no_release_callbacks(counter),
            },
        )
    }

    #[test]
    fn device_specific_hotkey_fires_on_matching_device() {
        let count = Arc::new(AtomicUsize::new(0));
        let key = (Key::Num1, vec![]);
        let filter = DeviceFilter::name_contains("StreamDeck");
        let info = test_device_info("Elgato StreamDeck XL", 0x0fd9, 0x006c);

        let device_regs: HashMap<DeviceRegistrationId, DeviceHotkeyRegistration> =
            [device_registration(1, key, filter, count.clone())]
                .into_iter()
                .collect();

        let modifiers = HashSet::new();
        let mut active_presses = HashMap::new();
        let now = Instant::now();

        let dispatch = collect_device_specific_dispatch(
            Key::Num1,
            1,
            now,
            &info,
            &modifiers,
            &device_regs,
            &mut active_presses,
        );

        assert!(dispatch.matched);
        dispatch_callbacks(dispatch.callbacks);
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn device_specific_hotkey_does_not_fire_on_wrong_device() {
        let count = Arc::new(AtomicUsize::new(0));
        let key = (Key::Num1, vec![]);
        let filter = DeviceFilter::name_contains("StreamDeck");
        let info = test_device_info("AT Translated Set 2 keyboard", 0x0001, 0x0001);

        let device_regs: HashMap<DeviceRegistrationId, DeviceHotkeyRegistration> =
            [device_registration(1, key, filter, count.clone())]
                .into_iter()
                .collect();

        let modifiers = HashSet::new();
        let mut active_presses = HashMap::new();
        let now = Instant::now();

        let dispatch = collect_device_specific_dispatch(
            Key::Num1,
            1,
            now,
            &info,
            &modifiers,
            &device_regs,
            &mut active_presses,
        );

        assert!(!dispatch.matched);
        assert_eq!(count.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn device_specific_hotkey_uses_per_device_modifiers() {
        let count = Arc::new(AtomicUsize::new(0));
        // Register Ctrl+A on StreamDeck
        let key = (Key::A, vec![Modifier::Ctrl]);
        let filter = DeviceFilter::name_contains("StreamDeck");
        let info = test_device_info("StreamDeck", 0x0fd9, 0x006c);

        let device_regs: HashMap<DeviceRegistrationId, DeviceHotkeyRegistration> =
            [device_registration(1, key, filter, count.clone())]
                .into_iter()
                .collect();

        // Device does NOT have Ctrl pressed (another device does)
        let device_modifiers: HashSet<Modifier> = HashSet::new();
        let mut active_presses = HashMap::new();
        let now = Instant::now();

        let dispatch = collect_device_specific_dispatch(
            Key::A,
            1,
            now,
            &info,
            &device_modifiers,
            &device_regs,
            &mut active_presses,
        );

        // Should NOT match because device doesn't have Ctrl
        assert!(!dispatch.matched);
        assert_eq!(count.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn device_specific_hotkey_matches_with_correct_per_device_modifiers() {
        let count = Arc::new(AtomicUsize::new(0));
        let key = (Key::A, vec![Modifier::Ctrl]);
        let filter = DeviceFilter::name_contains("StreamDeck");
        let info = test_device_info("StreamDeck", 0x0fd9, 0x006c);

        let device_regs: HashMap<DeviceRegistrationId, DeviceHotkeyRegistration> =
            [device_registration(1, key, filter, count.clone())]
                .into_iter()
                .collect();

        // Device HAS Ctrl pressed
        let device_modifiers: HashSet<Modifier> = [Modifier::Ctrl].into_iter().collect();
        let mut active_presses = HashMap::new();
        let now = Instant::now();

        let dispatch = collect_device_specific_dispatch(
            Key::A,
            1,
            now,
            &info,
            &device_modifiers,
            &device_regs,
            &mut active_presses,
        );

        assert!(dispatch.matched);
        dispatch_callbacks(dispatch.callbacks);
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn global_hotkey_uses_aggregate_modifiers_unchanged() {
        // Global hotkey should still use aggregate modifiers
        let press_count = Arc::new(AtomicUsize::new(0));
        let p = press_count.clone();

        let callbacks = HotkeyCallbacks {
            on_press: Arc::new(move || {
                p.fetch_add(1, Ordering::SeqCst);
            }),
            on_release: None,
            wait_for_release: false,
            min_hold: None,
            repeat_behavior: RepeatBehavior::Ignore,
            passthrough: false,
        };

        let mut registrations = HashMap::new();
        registrations.insert(
            (Key::A, vec![Modifier::Ctrl]),
            HotkeyRegistration { callbacks },
        );

        // Aggregate modifiers include Ctrl (from any device)
        let aggregate: HashSet<Modifier> = [Modifier::Ctrl].into_iter().collect();
        let mut active_presses = HashMap::new();
        let now = Instant::now();

        dispatch_callbacks(collect_non_modifier_callbacks(
            Key::A,
            1,
            now,
            &aggregate,
            &registrations,
            &mut active_presses,
            false,
        ));

        assert_eq!(press_count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn modifier_tracker_returns_per_device_modifiers() {
        let mut tracker = ModifierTracker::default();
        let device_a = PathBuf::from("/dev/input/event100");
        let device_b = PathBuf::from("/dev/input/event101");

        tracker.press(&device_a, Modifier::Ctrl);
        tracker.press(&device_b, Modifier::Shift);

        let a_mods = tracker.device_modifiers(&device_a);
        assert!(a_mods.contains(&Modifier::Ctrl));
        assert!(!a_mods.contains(&Modifier::Shift));

        let b_mods = tracker.device_modifiers(&device_b);
        assert!(!b_mods.contains(&Modifier::Ctrl));
        assert!(b_mods.contains(&Modifier::Shift));

        // Aggregate has both
        let agg = tracker.active_modifiers();
        assert!(agg.contains(&Modifier::Ctrl));
        assert!(agg.contains(&Modifier::Shift));
    }

    #[test]
    fn modifier_tracker_returns_empty_for_unknown_device() {
        let tracker = ModifierTracker::default();
        let unknown = PathBuf::from("/dev/input/event999");
        assert!(tracker.device_modifiers(&unknown).is_empty());
    }

    #[test]
    fn update_pressed_key_state_tracks_press_and_release() {
        let key_state = SharedKeyState::new();
        let mut pressed_keys = HashSet::new();

        update_pressed_key_state(&mut pressed_keys, &key_state, Key::A.to_evdev(), 1);
        assert!(key_state.is_pressed(Key::A.to_evdev()));

        update_pressed_key_state(&mut pressed_keys, &key_state, Key::A.to_evdev(), 0);
        assert!(!key_state.is_pressed(Key::A.to_evdev()));
    }

    #[test]
    fn key_state_modifier_query_matches_modifier_tracker_state() {
        let key_state = SharedKeyState::new();
        let mut pressed_keys = HashSet::new();
        let mut tracker = ModifierTracker::default();
        let device = PathBuf::from("/dev/input/event200");

        tracker.press(&device, Modifier::Ctrl);
        update_pressed_key_state(&mut pressed_keys, &key_state, Modifier::Ctrl.to_evdev(), 1);
        let key_state_modifiers: HashSet<Modifier> = key_state
            .active_modifiers()
            .into_iter()
            .filter_map(Modifier::from_evdev)
            .collect();
        assert_eq!(key_state_modifiers, tracker.active_modifiers());

        tracker.release(&device, Modifier::Ctrl);
        update_pressed_key_state(&mut pressed_keys, &key_state, Modifier::Ctrl.to_evdev(), 0);
        let key_state_modifiers: HashSet<Modifier> = key_state
            .active_modifiers()
            .into_iter()
            .filter_map(Modifier::from_evdev)
            .collect();
        assert_eq!(key_state_modifiers, tracker.active_modifiers());
    }

    #[test]
    fn device_specific_usb_id_filter_matches() {
        let count = Arc::new(AtomicUsize::new(0));
        let key = (Key::F1, vec![]);
        let filter = DeviceFilter::usb(0x1234, 0x5678);
        let info = test_device_info("Custom Macro Pad", 0x1234, 0x5678);

        let device_regs: HashMap<DeviceRegistrationId, DeviceHotkeyRegistration> =
            [device_registration(1, key, filter, count.clone())]
                .into_iter()
                .collect();

        let modifiers = HashSet::new();
        let mut active_presses = HashMap::new();
        let now = Instant::now();

        let dispatch = collect_device_specific_dispatch(
            Key::F1,
            1,
            now,
            &info,
            &modifiers,
            &device_regs,
            &mut active_presses,
        );

        assert!(dispatch.matched);
        dispatch_callbacks(dispatch.callbacks);
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn device_specific_release_fires_after_press() {
        let press_count = Arc::new(AtomicUsize::new(0));
        let release_count = Arc::new(AtomicUsize::new(0));
        let pc = press_count.clone();
        let rc = release_count.clone();

        let filter = DeviceFilter::name_contains("StreamDeck");
        let info = test_device_info("StreamDeck", 0x0fd9, 0x006c);
        let key = (Key::Num1, vec![]);

        let device_regs: HashMap<DeviceRegistrationId, DeviceHotkeyRegistration> = [(
            1,
            DeviceHotkeyRegistration {
                hotkey_key: key,
                filter,
                callbacks: HotkeyCallbacks {
                    on_press: Arc::new(move || {
                        pc.fetch_add(1, Ordering::SeqCst);
                    }),
                    on_release: Some(Arc::new(move || {
                        rc.fetch_add(1, Ordering::SeqCst);
                    })),
                    wait_for_release: true,
                    min_hold: None,
                    repeat_behavior: RepeatBehavior::Ignore,
                    passthrough: false,
                },
            },
        )]
        .into_iter()
        .collect();

        let modifiers = HashSet::new();
        let mut active_presses = HashMap::new();
        let now = Instant::now();

        // Press
        let press_dispatch = collect_device_specific_dispatch(
            Key::Num1,
            1,
            now,
            &info,
            &modifiers,
            &device_regs,
            &mut active_presses,
        );
        dispatch_callbacks(press_dispatch.callbacks);

        // Release
        let release_dispatch = collect_device_specific_dispatch(
            Key::Num1,
            0,
            now + Duration::from_millis(10),
            &info,
            &modifiers,
            &device_regs,
            &mut active_presses,
        );
        dispatch_callbacks(release_dispatch.callbacks);

        assert_eq!(press_count.load(Ordering::SeqCst), 1);
        assert_eq!(release_count.load(Ordering::SeqCst), 1);
    }
}
