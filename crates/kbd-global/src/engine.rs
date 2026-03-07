//! The engine — owns all mutable state, runs the event loop.
//!
//! # Architecture
//!
//! The engine runs in a dedicated thread. It owns:
//! - All registered bindings
//! - The layer stack
//! - Key state (what's currently pressed)
//! - The press cache (for correct releases across layer transitions)
//! - Device handles and the uinput forwarder
//!
//! No shared mutable state. The manager communicates via a command channel.
//! An eventfd (or pipe) wakes the engine's `poll()` when commands arrive.
//!
//! # Event loop
//!
//! ```text
//! loop {
//!     poll(device_fds + wake_fd, timeout)
//!     drain_commands()        // process register/unregister/layer ops
//!     process_key_events()    // for each ready device
//!     check_timers()          // layer timeouts
//! }
//! ```
//!
//! # Modules
//!
//! - [`devices`] — device discovery, hotplug, capability detection
//! - [`forwarder`] — uinput virtual device for event forwarding/emission
//! - [`types`] — shared engine types (grab state, dispositions, match outcomes)
//! - [`command`] — command enum and sender for manager→engine communication
//! - [`runtime`] — engine thread lifecycle (spawn, shutdown, join)
//!
//! Key state and matching logic live in `kbd` (`KeyState`, `Dispatcher`).

use std::collections::HashMap;
use std::io;
use std::path::Path;
use std::sync::Arc;
use std::sync::mpsc;
use std::time::Instant;

use kbd::action::Action;
use kbd::binding::KeyPropagation;
use kbd::binding::RepeatPolicy;
use kbd::device::DeviceContext;
use kbd::dispatcher::Dispatcher;
use kbd::dispatcher::MatchResult;
use kbd::hotkey::Hotkey;
use kbd::key::Key;
use kbd::key_state::KeyState;
use kbd::key_state::KeyTransition;

use crate::Error;
use crate::engine::devices::DeviceKeyEvent;

pub(crate) mod command;
pub(crate) mod devices;
pub(crate) mod forwarder;
pub(crate) mod runtime;
pub(crate) mod types;
mod wake;

pub(crate) use self::command::Command;
pub(crate) use self::command::CommandSender;
pub(crate) use self::runtime::EngineRuntime;
pub(crate) use self::types::GrabState;
pub(crate) use self::types::KeyEventOutcome;
use self::types::MatchOutcome;
use self::types::PressCacheEntry;
use self::types::RepeatInfo;
use self::wake::WakeFd;

pub(crate) struct Engine {
    /// The synchronous matching engine — owns bindings, layers, and layer stack.
    dispatcher: Dispatcher,
    /// Press cache: records what happened on key press so the corresponding
    /// release event uses the same disposition. Essential for correct
    /// release behavior across layer transitions — if a key was consumed
    /// on press, its release should also be consumed even if the layer
    /// was popped in between (oneshot, `PopLayer` action, etc.).
    ///
    /// Also stores repeat policy info so the engine can handle OS
    /// auto-repeat events per-binding (Allow, Suppress, or Custom).
    ///
    /// Mirrors the approach used by keyd's `cache_entry` system.
    press_cache: HashMap<Key, PressCacheEntry>,
    devices: devices::DeviceManager,
    key_state: KeyState,
    grab_state: GrabState,
    command_rx: mpsc::Receiver<Command>,
    wake_fd: Arc<WakeFd>,
}

impl Engine {
    fn new_with_input_dir(
        command_rx: mpsc::Receiver<Command>,
        wake_fd: Arc<WakeFd>,
        grab_state: GrabState,
        input_directory: &Path,
    ) -> Self {
        let device_grab_mode = match &grab_state {
            GrabState::Disabled => devices::DeviceGrabMode::Shared,
            GrabState::Enabled { .. } => devices::DeviceGrabMode::Exclusive,
        };
        Self {
            dispatcher: Dispatcher::new(),
            press_cache: HashMap::new(),
            devices: devices::DeviceManager::new(input_directory, device_grab_mode),
            key_state: KeyState::default(),
            grab_state,
            command_rx,
            wake_fd,
        }
    }

    fn poll_sources(&mut self) -> Result<Vec<libc::pollfd>, Error> {
        let device_fds = self.devices.poll_fds();

        let mut poll_fds = Vec::with_capacity(device_fds.len() + 1);
        poll_fds.push(libc::pollfd {
            fd: self.wake_fd.raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        });

        for &fd in device_fds {
            poll_fds.push(libc::pollfd {
                fd,
                events: libc::POLLIN,
                revents: 0,
            });
        }

        let poll_len = libc::nfds_t::try_from(poll_fds.len()).map_err(|_| Error::EngineError)?;
        let poll_timeout_ms = self.next_timer_deadline_ms();
        // SAFETY: `poll_fds` is a valid mutable buffer of `pollfd` values and
        // `poll_len` matches its length.
        let result = unsafe { libc::poll(poll_fds.as_mut_ptr(), poll_len, poll_timeout_ms) };

        if result < 0 {
            let error = io::Error::last_os_error();
            if error.kind() == io::ErrorKind::Interrupted {
                return Ok(poll_fds);
            }
            return Err(Error::EngineError);
        }

        if (poll_fds[0].revents & libc::POLLIN) != 0 {
            self.wake_fd.clear().map_err(|_| Error::EngineError)?;
        }

        Ok(poll_fds)
    }

    /// Returns the poll timeout in milliseconds based on the nearest layer timeout.
    /// Returns -1 (infinite) if no timeouts are pending.
    fn next_timer_deadline_ms(&self) -> i32 {
        match self.dispatcher.next_timeout_deadline() {
            Some(remaining) => {
                let ms = remaining.as_millis();
                // Clamp to i32::MAX, add 1ms to ensure we wake after expiry
                i32::try_from(ms.saturating_add(1)).unwrap_or(i32::MAX)
            }
            None => -1,
        }
    }

    /// Drain all pending commands, returning `true` if shutdown was requested.
    fn drain_commands(&mut self) -> bool {
        loop {
            match self.command_rx.try_recv() {
                Ok(Command::Shutdown) => return true,
                Ok(command) => self.handle_command(command),
                Err(mpsc::TryRecvError::Empty) => return false,
                // Semantically distinct from Shutdown (channel broke vs explicit
                // request) but the correct response is the same: stop the loop.
                #[allow(clippy::match_same_arms)]
                Err(mpsc::TryRecvError::Disconnected) => return true,
            }
        }
    }

    fn handle_command(&mut self, command: Command) {
        match command {
            // Registration
            Command::Register { binding, reply } => {
                let _ = reply.send(
                    self.dispatcher
                        .register_binding(binding)
                        .map_err(Error::from),
                );
            }
            Command::RegisterSequence {
                sequence,
                action,
                options,
                reply,
            } => {
                let result = self
                    .dispatcher
                    .register_sequence_with_options(sequence, action, options)
                    .map_err(Error::from);
                let _ = reply.send(result);
            }
            Command::Unregister { id } => self.dispatcher.unregister(id),

            // Sequences
            Command::PendingSequence { reply } => {
                let _ = reply.send(self.dispatcher.pending_sequence());
            }

            // Layers
            Command::DefineLayer { layer, reply } => {
                let _ = reply.send(self.dispatcher.define_layer(layer).map_err(Error::from));
            }
            Command::PushLayer { name, reply } => {
                let _ = reply.send(self.dispatcher.push_layer(name).map_err(Error::from));
            }
            Command::PopLayer { reply } => {
                let _ = reply.send(self.dispatcher.pop_layer().map_err(Error::from));
            }
            Command::ToggleLayer { name, reply } => {
                let _ = reply.send(self.dispatcher.toggle_layer(name).map_err(Error::from));
            }

            // Queries
            Command::IsRegistered { hotkey, reply } => {
                let _ = reply.send(self.dispatcher.is_registered(&hotkey));
            }
            Command::IsKeyPressed { key, reply } => {
                let _ = reply.send(self.key_state.is_pressed(key));
            }
            Command::ActiveModifiers { reply } => {
                let _ = reply.send(self.key_state.active_modifiers());
            }
            Command::ListBindings { reply } => {
                let _ = reply.send(self.dispatcher.list_bindings());
            }
            Command::BindingsForKey { hotkey, reply } => {
                let _ = reply.send(self.dispatcher.bindings_for_key(&hotkey));
            }
            Command::ActiveLayers { reply } => {
                let _ = reply.send(self.dispatcher.active_layers());
            }
            Command::Conflicts { reply } => {
                let _ = reply.send(self.dispatcher.conflicts());
            }

            // Intercepted by drain_commands
            Command::Shutdown => {}
        }
    }

    fn process_polled_events(&mut self, poll_fds: &[libc::pollfd]) {
        let result = self.devices.process_polled_events(&poll_fds[1..]);

        for fd in result.disconnected_devices {
            self.key_state.disconnect_device(fd);
        }

        for event in result.key_events {
            let _ = self.process_key_event(event);
        }
    }

    fn process_key_event(&mut self, event: DeviceKeyEvent) -> KeyEventOutcome {
        self.key_state
            .apply_device_event(event.device_fd, event.key, event.transition);

        // Release: use cached disposition, remove cache entry.
        if matches!(event.transition, KeyTransition::Release) {
            if let Some(cached) = self.press_cache.remove(&event.key) {
                match cached.outcome {
                    KeyEventOutcome::MatchedForwarded | KeyEventOutcome::UnmatchedForwarded => {
                        self.forward_event(event.key, event.transition);
                    }
                    KeyEventOutcome::MatchedConsumed | KeyEventOutcome::Ignored => {}
                }
                return cached.outcome;
            }
        }

        // Repeat: use cached disposition for forwarding, check repeat
        // policy for action re-execution.
        if matches!(event.transition, KeyTransition::Repeat) {
            return self.handle_repeat_event(event);
        }

        // Press: match through the Dispatcher.
        self.process_press_event(event)
    }

    /// Handle a repeat event using the press cache and repeat policy.
    fn handle_repeat_event(&mut self, event: DeviceKeyEvent) -> KeyEventOutcome {
        let now = Instant::now();

        let Some(cached) = self.press_cache.get(&event.key) else {
            // No cache entry — modifier key or key pressed before cache.
            return KeyEventOutcome::Ignored;
        };

        // Extract what we need from the cache before any mutable borrows.
        let outcome = cached.outcome;
        let should_fire = match &cached.repeat_info {
            None => false,
            Some(info) => match info.policy {
                RepeatPolicy::Suppress => false,
                RepeatPolicy::Allow => true,
                RepeatPolicy::Custom { delay, rate } => {
                    let since_press = now.duration_since(info.press_time);

                    if since_press < delay {
                        false
                    } else if let Some(last_fire) = info.last_repeat_fire {
                        now.duration_since(last_fire) >= rate
                    } else {
                        true
                    }
                }
                // RepeatPolicy is #[non_exhaustive]
                #[allow(clippy::match_same_arms)]
                _ => false,
            },
        };
        // `cached` is no longer used; NLL releases the borrow on
        // `self.press_cache` so the mutable calls below compile.

        if should_fire {
            // Re-execute the action using lookup_action — a read-only
            // query that doesn't trigger debounce, rate limiting, oneshot
            // ticks, or layer effects.
            //
            // Note: lookup_action resolves against the *current* layer
            // stack. If a layer was popped after the original press,
            // the repeat may match a different binding (or none). This
            // is acceptable — the alternative (caching the Action
            // itself) would prevent live-updated bindings from taking
            // effect during a held key.
            let active_modifiers = self.key_state.active_modifiers();
            let candidate = Hotkey::with_modifiers(event.key, active_modifiers);

            let device_modifiers = self.key_state.active_modifiers_for_device(event.device_fd);
            let device_info = self.devices.device_info(event.device_fd);

            let action = if let Some(info) = device_info {
                let ctx = DeviceContext::new(event.device_fd, info)
                    .with_device_modifiers(device_modifiers);
                self.dispatcher.lookup_action(&candidate, Some(&ctx))
            } else {
                self.dispatcher.lookup_action(&candidate, None)
            };

            if let Some(action) = action {
                execute_action(action);

                // Update last repeat fire time for Custom rate tracking.
                // Only update when the action actually fired — if the
                // binding was removed or a layer changed, we shouldn't
                // advance the rate timer.
                if let Some(entry) = self.press_cache.get_mut(&event.key) {
                    if let Some(ref mut info) = entry.repeat_info {
                        info.last_repeat_fire = Some(now);
                    }
                }
            }
        }

        // Forwarding follows the original press disposition
        match outcome {
            KeyEventOutcome::MatchedForwarded | KeyEventOutcome::UnmatchedForwarded => {
                self.forward_event(event.key, event.transition);
            }
            KeyEventOutcome::MatchedConsumed | KeyEventOutcome::Ignored => {}
        }
        outcome
    }

    /// Process a key press event through the Dispatcher.
    fn process_press_event(&mut self, event: DeviceKeyEvent) -> KeyEventOutcome {
        let active_modifiers = self.key_state.active_modifiers();
        let candidate = Hotkey::with_modifiers(event.key, active_modifiers);

        // Build device context for device-specific binding support.
        let device_modifiers = self.key_state.active_modifiers_for_device(event.device_fd);
        let device_info = self.devices.device_info(event.device_fd);

        // Process through the Dispatcher.
        let now = Instant::now();
        let (outcome, repeat_info) = {
            let result = if let Some(info) = device_info {
                let ctx = DeviceContext::new(event.device_fd, info)
                    .with_device_modifiers(device_modifiers);
                self.dispatcher
                    .process_with_device(&candidate, event.transition, &ctx)
            } else {
                self.dispatcher.process(&candidate, event.transition)
            };
            match &result {
                MatchResult::Matched {
                    action,
                    propagation,
                    repeat_policy,
                } => {
                    // Capture press_time before executing the action so
                    // Custom repeat delay is measured from the key event,
                    // not from after callback completion.
                    let repeat_info = Some(RepeatInfo {
                        policy: *repeat_policy,
                        press_time: now,
                        last_repeat_fire: None,
                    });
                    execute_action(action);
                    (
                        MatchOutcome::Matched {
                            propagation: *propagation,
                        },
                        repeat_info,
                    )
                }
                MatchResult::Throttled { propagation } => {
                    // Throttled: action doesn't fire, but key forwarding
                    // still respects the binding's propagation setting.
                    (
                        MatchOutcome::Matched {
                            propagation: *propagation,
                        },
                        None,
                    )
                }
                MatchResult::Pending { .. } | MatchResult::Suppressed => {
                    (MatchOutcome::Consumed, None)
                }
                MatchResult::NoMatch | MatchResult::Ignored => (MatchOutcome::Unmatched, None),
                // MatchResult is #[non_exhaustive]
                #[allow(clippy::match_same_arms)]
                _ => (MatchOutcome::Unmatched, None),
            }
        };

        // Determine event disposition based on match outcome and grab state
        let disposition = match outcome {
            MatchOutcome::Matched { propagation } => match propagation {
                KeyPropagation::Continue
                    if matches!(self.grab_state, GrabState::Enabled { .. }) =>
                {
                    self.forward_event(event.key, event.transition);
                    KeyEventOutcome::MatchedForwarded
                }
                KeyPropagation::Continue | KeyPropagation::Stop => KeyEventOutcome::MatchedConsumed,
                // KeyPropagation is #[non_exhaustive]
                #[allow(clippy::match_same_arms)]
                _ => KeyEventOutcome::MatchedConsumed,
            },
            MatchOutcome::Consumed => KeyEventOutcome::MatchedConsumed,
            MatchOutcome::Unmatched => {
                if matches!(self.grab_state, GrabState::Enabled { .. }) {
                    self.forward_event(event.key, event.transition);
                    KeyEventOutcome::UnmatchedForwarded
                } else {
                    KeyEventOutcome::Ignored
                }
            }
        };

        // Cache the disposition for non-modifier key presses.
        if kbd::hotkey::Modifier::from_key(event.key).is_none() {
            self.press_cache.insert(
                event.key,
                PressCacheEntry {
                    outcome: disposition,
                    repeat_info,
                },
            );
        }

        disposition
    }

    fn forward_event(&mut self, key: Key, transition: KeyTransition) {
        if let GrabState::Enabled { forwarder } = &mut self.grab_state
            && let Err(error) = forwarder.forward_key(key, transition)
        {
            tracing::error!(%error, "failed to forward key event through virtual device");
        }
    }
}

/// Execute a user-facing action with panic isolation — a panicking callback
/// never kills the engine thread.
///
/// Handles `Action::Callback` directly. Layer-control actions (`PushLayer`,
/// `PopLayer`, `ToggleLayer`) are handled by `Dispatcher::process()` internally
/// and never reach this function. `Action::Suppress` is a no-op by design.
///
/// `Action::EmitHotkey` and `Action::EmitSequence` are not yet implemented —
/// they will panic if reached. Key emission support is planned.
fn execute_action(action: &Action) {
    match action {
        Action::Callback(callback) => {
            if let Err(panic) = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                callback();
            })) {
                tracing::error!(
                    panic_info = format!("{panic:?}"),
                    "user callback panicked — panic caught, engine continues"
                );
            }
        }
        Action::EmitHotkey(_) => {
            todo!("Action::EmitHotkey is not yet implemented in the kbd-global runtime")
        }
        Action::EmitSequence(_) => {
            todo!("Action::EmitSequence is not yet implemented in the kbd-global runtime")
        }
        // Layer actions are handled by Dispatcher::process(); Suppress is a no-op by design.
        // Explicit: Action is #[non_exhaustive]; list known variants so new
        // ones don't silently fall through.
        #[allow(clippy::match_same_arms)]
        Action::PushLayer(_) | Action::PopLayer | Action::ToggleLayer(_) | Action::Suppress => {}
        _ => {}
    }
}

pub(crate) fn run(mut engine: Engine) -> Result<(), Error> {
    loop {
        let poll_fds = engine.poll_sources()?;

        if engine.drain_commands() {
            return Ok(());
        }

        engine.process_polled_events(&poll_fds);
        for timeout_result in engine.dispatcher.check_timeouts_with_results() {
            if let MatchResult::Matched { action, .. } = timeout_result {
                execute_action(action);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;
    use std::sync::Arc;
    use std::sync::atomic::AtomicUsize;
    use std::sync::atomic::Ordering;
    use std::sync::mpsc;
    use std::time::Duration;

    use kbd::action::Action;
    use kbd::binding::BindingId;
    use kbd::binding::BindingOptions;
    use kbd::binding::KeyPropagation;
    use kbd::binding::RegisteredBinding;
    use kbd::binding::RepeatPolicy;
    use kbd::hotkey::Hotkey;
    use kbd::hotkey::Modifier;
    use kbd::key::Key;
    use kbd::key_state::KeyTransition;

    use super::Command;
    use super::Engine;
    use super::EngineRuntime;
    use super::GrabState;
    use super::KeyEventOutcome;
    use super::devices;
    use super::devices::DeviceKeyEvent;
    use super::wake::WakeFd;
    use crate::Error;

    #[test]
    fn engine_processes_register_and_unregister_commands() {
        let runtime = EngineRuntime::spawn(GrabState::Disabled).expect("engine should spawn");

        let id = BindingId::new();
        let binding = test_binding(id, Hotkey::new(Key::A).modifier(Modifier::Ctrl));
        let (reply_tx, reply_rx) = mpsc::channel();

        runtime
            .commands()
            .send(Command::Register {
                binding,
                reply: reply_tx,
            })
            .expect("register command should send");

        let register_result = reply_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("register command should receive reply");
        assert!(register_result.is_ok());

        runtime
            .commands()
            .send(Command::Unregister { id })
            .expect("unregister command should send");

        runtime.shutdown().expect("engine should shutdown cleanly");
    }

    #[test]
    fn engine_rejects_duplicate_hotkeys() {
        let runtime = EngineRuntime::spawn(GrabState::Disabled).expect("engine should spawn");

        let (first_reply_tx, first_reply_rx) = mpsc::channel();

        runtime
            .commands()
            .send(Command::Register {
                binding: test_binding(
                    BindingId::new(),
                    Hotkey::new(Key::B).modifier(Modifier::Alt),
                ),
                reply: first_reply_tx,
            })
            .expect("first register command should send");

        let first_result = first_reply_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("first register command should receive reply");
        assert!(first_result.is_ok());

        let (second_reply_tx, second_reply_rx) = mpsc::channel();
        runtime
            .commands()
            .send(Command::Register {
                binding: test_binding(
                    BindingId::new(),
                    Hotkey::new(Key::B).modifier(Modifier::Alt),
                ),
                reply: second_reply_tx,
            })
            .expect("second register command should send");

        let second_result = second_reply_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("second register command should receive reply");
        assert!(matches!(second_result, Err(Error::AlreadyRegistered)));

        runtime.shutdown().expect("engine should shutdown cleanly");
    }

    #[test]
    fn engine_reports_registration_queries() {
        let runtime = EngineRuntime::spawn(GrabState::Disabled).expect("engine should spawn");
        let hotkey = Hotkey::new(Key::C).modifier(Modifier::Shift);

        let (register_reply_tx, register_reply_rx) = mpsc::channel();
        runtime
            .commands()
            .send(Command::Register {
                binding: RegisteredBinding::new(BindingId::new(), hotkey.clone(), Action::Suppress),
                reply: register_reply_tx,
            })
            .expect("register command should send");

        let register_result = register_reply_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("register command should receive reply");
        assert!(register_result.is_ok());

        let (query_reply_tx, query_reply_rx) = mpsc::channel();
        runtime
            .commands()
            .send(Command::IsRegistered {
                hotkey,
                reply: query_reply_tx,
            })
            .expect("query command should send");

        let is_registered = query_reply_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("query command should receive reply");
        assert!(is_registered);

        runtime.shutdown().expect("engine should shutdown cleanly");
    }

    #[test]
    fn engine_shutdown_command_exits_thread() {
        let runtime = EngineRuntime::spawn(GrabState::Disabled).expect("engine should spawn");

        runtime
            .commands()
            .send(Command::Shutdown)
            .expect("shutdown command should send");

        runtime.join().expect("engine thread should join");
    }

    #[test]
    fn command_sender_reports_manager_stopped_after_engine_exit() {
        let runtime = EngineRuntime::spawn(GrabState::Disabled).expect("engine should spawn");
        let commands = runtime.commands();

        commands
            .send(Command::Shutdown)
            .expect("shutdown command should send");
        runtime.join().expect("engine thread should join");

        let send_result = commands.send(Command::Unregister {
            id: BindingId::new(),
        });
        assert!(matches!(send_result, Err(Error::ManagerStopped)));
    }

    fn test_binding(id: BindingId, hotkey: Hotkey) -> RegisteredBinding {
        RegisteredBinding::new(id, hotkey, Action::Suppress)
    }

    /// Create a minimal engine for unit testing (no devices, no grab, no event loop).
    fn test_engine() -> Engine {
        test_engine_with_grab(GrabState::Disabled)
    }

    /// Create a test engine with grab mode enabled (using a recording forwarder).
    fn test_engine_with_grab(grab_state: GrabState) -> Engine {
        let wake_fd = Arc::new(WakeFd::new().expect("wake fd should create"));
        let (_tx, rx) = mpsc::channel();
        Engine::new_with_input_dir(rx, wake_fd, grab_state, Path::new(devices::INPUT_DIRECTORY))
    }

    /// Create grab state with a recording forwarder for testing.
    /// Returns the `GrabState` and a handle to inspect forwarded events.
    fn test_grab_state() -> (GrabState, super::forwarder::testing::ForwardedEvents) {
        let (recorder, events) = super::forwarder::testing::RecordingForwarder::new();
        let state = GrabState::Enabled {
            forwarder: Box::new(recorder),
        };
        (state, events)
    }

    fn press_key(engine: &mut Engine, key: Key, device_fd: i32) -> KeyEventOutcome {
        engine.process_key_event(DeviceKeyEvent {
            device_fd,
            key,
            transition: KeyTransition::Press,
        })
    }

    fn release_key(engine: &mut Engine, key: Key, device_fd: i32) -> KeyEventOutcome {
        engine.process_key_event(DeviceKeyEvent {
            device_fd,
            key,
            transition: KeyTransition::Release,
        })
    }

    fn repeat_key(engine: &mut Engine, key: Key, device_fd: i32) -> KeyEventOutcome {
        engine.process_key_event(DeviceKeyEvent {
            device_fd,
            key,
            transition: KeyTransition::Repeat,
        })
    }

    #[test]
    fn matching_hotkey_fires_callback() {
        let mut engine = test_engine();
        let counter = Arc::new(AtomicUsize::new(0));
        let counter_clone = Arc::clone(&counter);

        let id = BindingId::new();
        let hotkey = Hotkey::new(Key::C).modifier(Modifier::Ctrl);
        let action = Action::from(move || {
            counter_clone.fetch_add(1, Ordering::Relaxed);
        });
        let binding = RegisteredBinding::new(id, hotkey, action);
        engine.dispatcher.register_binding(binding).unwrap();

        // Simulate: press Ctrl, then press C
        press_key(&mut engine, Key::CONTROL_LEFT, 10);
        press_key(&mut engine, Key::C, 10);

        assert_eq!(counter.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn unmatched_event_does_not_fire_any_callback() {
        let mut engine = test_engine();
        let counter = Arc::new(AtomicUsize::new(0));
        let counter_clone = Arc::clone(&counter);

        let id = BindingId::new();
        let hotkey = Hotkey::new(Key::C).modifier(Modifier::Ctrl);
        let action = Action::from(move || {
            counter_clone.fetch_add(1, Ordering::Relaxed);
        });
        let binding = RegisteredBinding::new(id, hotkey, action);
        engine.dispatcher.register_binding(binding).unwrap();

        // Press V instead of C (with Ctrl held)
        press_key(&mut engine, Key::CONTROL_LEFT, 10);
        press_key(&mut engine, Key::V, 10);

        assert_eq!(counter.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn modifier_combination_must_match_exactly() {
        let mut engine = test_engine();
        let counter = Arc::new(AtomicUsize::new(0));
        let counter_clone = Arc::clone(&counter);

        let id = BindingId::new();
        let hotkey = Hotkey::new(Key::C).modifier(Modifier::Ctrl);
        let action = Action::from(move || {
            counter_clone.fetch_add(1, Ordering::Relaxed);
        });
        let binding = RegisteredBinding::new(id, hotkey, action);
        engine.dispatcher.register_binding(binding).unwrap();

        // Press Ctrl+Shift+C — binding only wants Ctrl+C
        press_key(&mut engine, Key::CONTROL_LEFT, 10);
        press_key(&mut engine, Key::SHIFT_LEFT, 10);
        press_key(&mut engine, Key::C, 10);

        assert_eq!(counter.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn multi_modifier_hotkey_fires_when_all_held() {
        let mut engine = test_engine();
        let counter = Arc::new(AtomicUsize::new(0));
        let counter_clone = Arc::clone(&counter);

        let id = BindingId::new();
        let hotkey = Hotkey::new(Key::A)
            .modifier(Modifier::Ctrl)
            .modifier(Modifier::Shift);
        let action = Action::from(move || {
            counter_clone.fetch_add(1, Ordering::Relaxed);
        });
        let binding = RegisteredBinding::new(id, hotkey, action);
        engine.dispatcher.register_binding(binding).unwrap();

        press_key(&mut engine, Key::CONTROL_LEFT, 10);
        press_key(&mut engine, Key::SHIFT_LEFT, 10);
        press_key(&mut engine, Key::A, 10);

        assert_eq!(counter.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn hotkey_without_modifiers_fires_on_bare_keypress() {
        let mut engine = test_engine();
        let counter = Arc::new(AtomicUsize::new(0));
        let counter_clone = Arc::clone(&counter);

        let id = BindingId::new();
        let hotkey = Hotkey::new(Key::ESCAPE);
        let action = Action::from(move || {
            counter_clone.fetch_add(1, Ordering::Relaxed);
        });
        let binding = RegisteredBinding::new(id, hotkey, action);
        engine.dispatcher.register_binding(binding).unwrap();

        press_key(&mut engine, Key::ESCAPE, 10);

        assert_eq!(counter.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn release_does_not_fire_callback() {
        let mut engine = test_engine();
        let counter = Arc::new(AtomicUsize::new(0));
        let counter_clone = Arc::clone(&counter);

        let id = BindingId::new();
        let hotkey = Hotkey::new(Key::C).modifier(Modifier::Ctrl);
        let action = Action::from(move || {
            counter_clone.fetch_add(1, Ordering::Relaxed);
        });
        let binding = RegisteredBinding::new(id, hotkey, action);
        engine.dispatcher.register_binding(binding).unwrap();

        // Press the hotkey so it fires once
        press_key(&mut engine, Key::CONTROL_LEFT, 10);
        press_key(&mut engine, Key::C, 10);
        assert_eq!(counter.load(Ordering::Relaxed), 1);

        // Release should not fire again
        release_key(&mut engine, Key::C, 10);
        assert_eq!(counter.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn panicking_callback_does_not_kill_engine() {
        let mut engine = test_engine();
        let post_panic_counter = Arc::new(AtomicUsize::new(0));
        let post_panic_clone = Arc::clone(&post_panic_counter);

        // Register a binding that panics
        let id1 = BindingId::new();
        let hotkey1 = Hotkey::new(Key::P).modifier(Modifier::Ctrl);
        let action1 = Action::from(move || {
            panic!("intentional test panic");
        });
        engine
            .dispatcher
            .register_binding(RegisteredBinding::new(id1, hotkey1, action1))
            .unwrap();

        // Register a second binding that increments a counter
        let id2 = BindingId::new();
        let hotkey2 = Hotkey::new(Key::Q).modifier(Modifier::Ctrl);
        let action2 = Action::from(move || {
            post_panic_clone.fetch_add(1, Ordering::Relaxed);
        });
        engine
            .dispatcher
            .register_binding(RegisteredBinding::new(id2, hotkey2, action2))
            .unwrap();

        // Trigger the panicking callback
        press_key(&mut engine, Key::CONTROL_LEFT, 10);
        press_key(&mut engine, Key::P, 10);
        // Engine should still be alive

        // Release P, then press Q
        release_key(&mut engine, Key::P, 10);
        press_key(&mut engine, Key::Q, 10);

        assert_eq!(post_panic_counter.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn right_modifier_satisfies_binding() {
        let mut engine = test_engine();
        let counter = Arc::new(AtomicUsize::new(0));
        let counter_clone = Arc::clone(&counter);

        let id = BindingId::new();
        let hotkey = Hotkey::new(Key::C).modifier(Modifier::Ctrl);
        let action = Action::from(move || {
            counter_clone.fetch_add(1, Ordering::Relaxed);
        });
        let binding = RegisteredBinding::new(id, hotkey, action);
        engine.dispatcher.register_binding(binding).unwrap();

        // Use RightCtrl instead of LeftCtrl — should still match
        press_key(&mut engine, Key::CONTROL_RIGHT, 10);
        press_key(&mut engine, Key::C, 10);

        assert_eq!(counter.load(Ordering::Relaxed), 1);
    }

    // Grab mode tests

    #[test]
    fn grab_mode_forwards_unmatched_key_events() {
        let (grab_state, forwarded) = test_grab_state();
        let mut engine = test_engine_with_grab(grab_state);

        let id = BindingId::new();
        let hotkey = Hotkey::new(Key::C).modifier(Modifier::Ctrl);
        engine
            .dispatcher
            .register_binding(RegisteredBinding::new(id, hotkey, Action::Suppress))
            .unwrap();

        // Press A with no modifiers — no binding matches, should be forwarded
        let disposition = press_key(&mut engine, Key::A, 10);
        assert_eq!(disposition, KeyEventOutcome::UnmatchedForwarded);

        // Verify the forwarder received the event
        let events = forwarded.lock().unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0], (Key::A, KeyTransition::Press));
    }

    #[test]
    fn grab_mode_consumes_matched_key_events() {
        let (grab_state, forwarded) = test_grab_state();
        let mut engine = test_engine_with_grab(grab_state);
        let counter = Arc::new(AtomicUsize::new(0));
        let counter_clone = Arc::clone(&counter);

        let id = BindingId::new();
        let hotkey = Hotkey::new(Key::C).modifier(Modifier::Ctrl);
        let action = Action::from(move || {
            counter_clone.fetch_add(1, Ordering::Relaxed);
        });
        engine
            .dispatcher
            .register_binding(RegisteredBinding::new(id, hotkey, action))
            .unwrap();

        // Press Ctrl+C — matches binding, should be consumed
        press_key(&mut engine, Key::CONTROL_LEFT, 10);
        let disposition = press_key(&mut engine, Key::C, 10);

        assert_eq!(disposition, KeyEventOutcome::MatchedConsumed);
        assert_eq!(counter.load(Ordering::Relaxed), 1);

        // Forwarder should NOT have the C press (modifier press is forwarded though)
        let events = forwarded.lock().unwrap();
        let c_events: Vec<_> = events.iter().filter(|(key, _)| *key == Key::C).collect();
        assert!(c_events.is_empty(), "matched key C should not be forwarded");
    }

    #[test]
    fn grab_mode_forwards_matched_event_with_passthrough() {
        let (grab_state, forwarded) = test_grab_state();
        let mut engine = test_engine_with_grab(grab_state);
        let counter = Arc::new(AtomicUsize::new(0));
        let counter_clone = Arc::clone(&counter);

        let id = BindingId::new();
        let hotkey = Hotkey::new(Key::C).modifier(Modifier::Ctrl);
        let action = Action::from(move || {
            counter_clone.fetch_add(1, Ordering::Relaxed);
        });
        let binding =
            RegisteredBinding::new(id, hotkey, action).with_propagation(KeyPropagation::Continue);
        engine.dispatcher.register_binding(binding).unwrap();

        // Press Ctrl+C with passthrough — should fire AND forward
        press_key(&mut engine, Key::CONTROL_LEFT, 10);
        let disposition = press_key(&mut engine, Key::C, 10);

        assert_eq!(disposition, KeyEventOutcome::MatchedForwarded);
        assert_eq!(counter.load(Ordering::Relaxed), 1);

        // Forwarder should have the C press event
        let events = forwarded.lock().unwrap();
        let c_events: Vec<_> = events.iter().filter(|(key, _)| *key == Key::C).collect();
        assert_eq!(
            c_events.len(),
            1,
            "passthrough should forward the matched key"
        );
    }

    #[test]
    fn no_grab_mode_does_not_forward_unmatched_events() {
        let mut engine = test_engine();

        // Press A with no bindings — should be ignored, not forwarded
        let disposition = press_key(&mut engine, Key::A, 10);
        assert_eq!(disposition, KeyEventOutcome::Ignored);
    }

    #[test]
    fn passthrough_without_grab_mode_still_returns_consumed() {
        let mut engine = test_engine();
        let counter = Arc::new(AtomicUsize::new(0));
        let counter_clone = Arc::clone(&counter);

        let id = BindingId::new();
        let hotkey = Hotkey::new(Key::C).modifier(Modifier::Ctrl);
        let action = Action::from(move || {
            counter_clone.fetch_add(1, Ordering::Relaxed);
        });
        let binding =
            RegisteredBinding::new(id, hotkey, action).with_propagation(KeyPropagation::Continue);
        engine.dispatcher.register_binding(binding).unwrap();

        press_key(&mut engine, Key::CONTROL_LEFT, 10);
        let disposition = press_key(&mut engine, Key::C, 10);

        // Without grab mode, passthrough is a no-op — event reaches apps
        // through the normal kernel path. The engine reports MatchedConsumed
        // because it matched and executed the action; no virtual-device
        // forwarding occurred.
        assert_eq!(disposition, KeyEventOutcome::MatchedConsumed);
        assert_eq!(counter.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn grab_mode_forwards_release_events() {
        let (grab_state, forwarded) = test_grab_state();
        let mut engine = test_engine_with_grab(grab_state);

        // Press and release A — both should be forwarded (no binding matches)
        let press_disposition = press_key(&mut engine, Key::A, 10);
        let release_disposition = release_key(&mut engine, Key::A, 10);

        assert_eq!(press_disposition, KeyEventOutcome::UnmatchedForwarded);
        assert_eq!(release_disposition, KeyEventOutcome::UnmatchedForwarded);

        let events = forwarded.lock().unwrap();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0], (Key::A, KeyTransition::Press));
        assert_eq!(events[1], (Key::A, KeyTransition::Release));
    }

    #[test]
    fn is_key_pressed_query_reflects_key_state() {
        let mut engine = test_engine();

        assert!(!engine.key_state.is_pressed(Key::A));

        press_key(&mut engine, Key::A, 10);
        assert!(engine.key_state.is_pressed(Key::A));

        release_key(&mut engine, Key::A, 10);
        assert!(!engine.key_state.is_pressed(Key::A));
    }

    #[test]
    fn active_modifiers_query_reflects_held_modifiers() {
        let mut engine = test_engine();

        assert!(engine.key_state.active_modifiers().is_empty());

        press_key(&mut engine, Key::CONTROL_LEFT, 10);
        assert_eq!(engine.key_state.active_modifiers(), vec![Modifier::Ctrl]);

        press_key(&mut engine, Key::SHIFT_LEFT, 10);
        assert_eq!(
            engine.key_state.active_modifiers(),
            vec![Modifier::Ctrl, Modifier::Shift]
        );

        release_key(&mut engine, Key::CONTROL_LEFT, 10);
        assert_eq!(engine.key_state.active_modifiers(), vec![Modifier::Shift]);
    }

    #[test]
    fn is_key_pressed_command_via_runtime() {
        let runtime = EngineRuntime::spawn(GrabState::Disabled).expect("engine should spawn");

        let (reply_tx, reply_rx) = mpsc::channel();
        runtime
            .commands()
            .send(Command::IsKeyPressed {
                key: Key::A,
                reply: reply_tx,
            })
            .expect("query command should send");

        let is_pressed = reply_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("query command should receive reply");
        assert!(!is_pressed);

        runtime.shutdown().expect("engine should shutdown cleanly");
    }

    #[test]
    fn active_modifiers_command_via_runtime() {
        let runtime = EngineRuntime::spawn(GrabState::Disabled).expect("engine should spawn");

        let (reply_tx, reply_rx) = mpsc::channel();
        runtime
            .commands()
            .send(Command::ActiveModifiers { reply: reply_tx })
            .expect("query command should send");

        let modifiers = reply_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("query command should receive reply");
        assert!(modifiers.is_empty());

        runtime.shutdown().expect("engine should shutdown cleanly");
    }

    #[test]
    fn pending_sequence_command_reports_progress() {
        let mut engine = test_engine();

        let sequence = "Ctrl+K, Ctrl+C"
            .parse::<kbd::hotkey::HotkeySequence>()
            .unwrap();
        let (register_reply_tx, register_reply_rx) = mpsc::channel();
        let () = engine.handle_command(Command::RegisterSequence {
            sequence,
            action: Action::Suppress,
            options: kbd::sequence::SequenceOptions::default(),
            reply: register_reply_tx,
        });
        assert!(register_reply_rx.recv().unwrap().is_ok());

        press_key(&mut engine, Key::CONTROL_LEFT, 10);
        let _ = press_key(&mut engine, Key::K, 10);

        let (reply_tx, reply_rx) = mpsc::channel();
        let () = engine.handle_command(Command::PendingSequence { reply: reply_tx });
        let pending = reply_rx.recv().expect("pending sequence reply");

        let pending = pending.expect("sequence should be pending");
        assert_eq!(pending.steps_matched, 1);
        assert_eq!(pending.steps_remaining, 1);
    }

    #[test]
    fn sequence_timeout_option_applies_to_registration() {
        let mut engine = test_engine();

        let sequence = "Ctrl+K, Ctrl+C"
            .parse::<kbd::hotkey::HotkeySequence>()
            .unwrap();
        let (register_reply_tx, register_reply_rx) = mpsc::channel();
        let () = engine.handle_command(Command::RegisterSequence {
            sequence,
            action: Action::Suppress,
            options: kbd::sequence::SequenceOptions::default()
                .with_timeout(Duration::from_millis(10)),
            reply: register_reply_tx,
        });
        assert!(register_reply_rx.recv().unwrap().is_ok());

        press_key(&mut engine, Key::CONTROL_LEFT, 10);
        let _ = press_key(&mut engine, Key::K, 10);

        std::thread::sleep(Duration::from_millis(20));
        let _ = engine.dispatcher.check_timeouts_with_results();
        assert!(engine.dispatcher.pending_sequence().is_none());
    }

    #[test]
    fn modifier_state_cleaned_on_device_disconnect() {
        let mut engine = test_engine();

        press_key(&mut engine, Key::CONTROL_LEFT, 10);
        press_key(&mut engine, Key::SHIFT_LEFT, 11);

        assert_eq!(
            engine.key_state.active_modifiers(),
            vec![Modifier::Ctrl, Modifier::Shift]
        );

        // Simulate device 10 disconnecting
        engine.key_state.disconnect_device(10);

        // Only modifiers from device 11 should remain
        assert_eq!(engine.key_state.active_modifiers(), vec![Modifier::Shift]);
        assert!(!engine.key_state.is_pressed(Key::CONTROL_LEFT));
    }

    // Layer storage tests

    #[test]
    fn engine_stores_defined_layer() {
        let mut engine = test_engine();
        let layer = kbd::layer::Layer::new("nav")
            .bind(Key::H, Action::Suppress)
            .unwrap()
            .bind(Key::J, Action::Suppress)
            .unwrap();

        let result = engine.dispatcher.define_layer(layer);
        assert!(result.is_ok());

        // Verify via introspection: the layer's bindings are listed
        let bindings = engine.dispatcher.list_bindings();
        let nav_bindings: Vec<_> = bindings
            .iter()
            .filter(|b| {
                b.location
                    == kbd::introspection::BindingLocation::Layer(kbd::layer::LayerName::from(
                        "nav",
                    ))
            })
            .collect();
        assert_eq!(nav_bindings.len(), 2);
    }

    #[test]
    fn engine_stores_layer_bindings() {
        let mut engine = test_engine();
        let layer = kbd::layer::Layer::new("nav")
            .bind(Key::H, Action::Suppress)
            .unwrap()
            .bind(Key::J, Action::Suppress)
            .unwrap()
            .bind(Key::K, Action::Suppress)
            .unwrap();

        engine.dispatcher.define_layer(layer).unwrap();

        // Verify via introspection: three bindings in the nav layer
        let bindings = engine.dispatcher.list_bindings();
        let nav_bindings: Vec<_> = bindings
            .iter()
            .filter(|b| {
                b.location
                    == kbd::introspection::BindingLocation::Layer(kbd::layer::LayerName::from(
                        "nav",
                    ))
            })
            .collect();
        assert_eq!(nav_bindings.len(), 3);
    }

    #[test]
    fn engine_stores_layer_options() {
        let mut engine = test_engine();
        let layer = kbd::layer::Layer::new("oneshot-nav")
            .bind(Key::H, Action::Suppress)
            .unwrap()
            .swallow()
            .oneshot(1)
            .timeout(std::time::Duration::from_secs(5));

        engine.dispatcher.define_layer(layer).unwrap();

        // Verify layer was stored by pushing and checking behavior:
        // the oneshot layer should auto-pop after 1 keypress
        engine
            .dispatcher
            .push_layer(kbd::layer::LayerName::from("oneshot-nav"))
            .unwrap();

        // H should match in the layer
        let result = engine
            .dispatcher
            .process(&Hotkey::new(Key::H), KeyTransition::Press);
        assert!(matches!(
            result,
            kbd::dispatcher::MatchResult::Matched { .. }
        ));

        // After oneshot depth of 1, layer should be gone
        let result = engine
            .dispatcher
            .process(&Hotkey::new(Key::H), KeyTransition::Press);
        assert!(matches!(result, kbd::dispatcher::MatchResult::NoMatch));
    }

    #[test]
    fn engine_stores_empty_layer() {
        let mut engine = test_engine();
        let layer = kbd::layer::Layer::new("empty");

        engine.dispatcher.define_layer(layer).unwrap();

        // Push the empty layer — should succeed and have 0 bindings
        engine
            .dispatcher
            .push_layer(kbd::layer::LayerName::from("empty"))
            .unwrap();
        let active = engine.dispatcher.active_layers();
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].name.as_str(), "empty");
        assert_eq!(active[0].binding_count, 0);
    }

    #[test]
    fn define_layer_via_runtime_command() {
        let runtime = EngineRuntime::spawn(GrabState::Disabled).expect("engine should spawn");

        let layer = kbd::layer::Layer::new("nav")
            .bind(Key::H, Action::Suppress)
            .unwrap();
        let (reply_tx, reply_rx) = mpsc::channel();

        runtime
            .commands()
            .send(Command::DefineLayer {
                layer,
                reply: reply_tx,
            })
            .expect("define layer command should send");

        let result = reply_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("define layer command should receive reply");
        assert!(result.is_ok());

        runtime.shutdown().expect("engine should shutdown cleanly");
    }

    #[test]
    fn define_duplicate_layer_via_runtime_returns_error() {
        let runtime = EngineRuntime::spawn(GrabState::Disabled).expect("engine should spawn");

        // Define first layer — should succeed
        let first_layer = kbd::layer::Layer::new("nav")
            .bind(Key::H, Action::Suppress)
            .unwrap();
        let (reply_tx, reply_rx) = mpsc::channel();
        runtime
            .commands()
            .send(Command::DefineLayer {
                layer: first_layer,
                reply: reply_tx,
            })
            .expect("first define layer should send");
        assert!(
            reply_rx
                .recv_timeout(Duration::from_secs(1))
                .unwrap()
                .is_ok()
        );

        // Define second layer with same name — should fail
        let duplicate_layer = kbd::layer::Layer::new("nav")
            .bind(Key::J, Action::Suppress)
            .unwrap();
        let (dup_reply_tx, dup_reply_rx) = mpsc::channel();
        runtime
            .commands()
            .send(Command::DefineLayer {
                layer: duplicate_layer,
                reply: dup_reply_tx,
            })
            .expect("duplicate define layer should send");
        let result = dup_reply_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("duplicate define layer should receive reply");
        assert!(matches!(result, Err(Error::LayerAlreadyDefined)));

        runtime.shutdown().expect("engine should shutdown cleanly");
    }

    fn define_and_push_layer(engine: &mut Engine, name: &str, bindings: Vec<(Key, Action)>) {
        let mut layer = kbd::layer::Layer::new(name);
        for (key, action) in bindings {
            layer = layer.bind(key, action).unwrap();
        }
        engine.dispatcher.define_layer(layer).unwrap();
        engine
            .dispatcher
            .push_layer(kbd::layer::LayerName::from(name))
            .unwrap();
    }

    #[test]
    fn push_layer_activates_layer_bindings() {
        let mut engine = test_engine();
        let counter = Arc::new(AtomicUsize::new(0));
        let cc = Arc::clone(&counter);

        define_and_push_layer(
            &mut engine,
            "nav",
            vec![(
                Key::H,
                Action::from(move || {
                    cc.fetch_add(1, Ordering::Relaxed);
                }),
            )],
        );

        press_key(&mut engine, Key::H, 10);
        assert_eq!(counter.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn pop_layer_deactivates_layer_bindings() {
        let mut engine = test_engine();
        let counter = Arc::new(AtomicUsize::new(0));
        let cc = Arc::clone(&counter);

        define_and_push_layer(
            &mut engine,
            "nav",
            vec![(
                Key::H,
                Action::from(move || {
                    cc.fetch_add(1, Ordering::Relaxed);
                }),
            )],
        );

        // Pop the layer
        let popped = engine.dispatcher.pop_layer().unwrap();
        assert_eq!(popped.as_str(), "nav");

        // H should no longer match
        press_key(&mut engine, Key::H, 10);
        assert_eq!(counter.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn toggle_layer_pushes_when_not_active() {
        let mut engine = test_engine();
        let counter = Arc::new(AtomicUsize::new(0));
        let cc = Arc::clone(&counter);

        let layer = kbd::layer::Layer::new("nav")
            .bind(
                Key::H,
                Action::from(move || {
                    cc.fetch_add(1, Ordering::Relaxed);
                }),
            )
            .unwrap();
        engine.dispatcher.define_layer(layer).unwrap();

        engine
            .dispatcher
            .toggle_layer(kbd::layer::LayerName::from("nav"))
            .unwrap();

        press_key(&mut engine, Key::H, 10);
        assert_eq!(counter.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn toggle_layer_removes_when_active() {
        let mut engine = test_engine();
        let counter = Arc::new(AtomicUsize::new(0));
        let cc = Arc::clone(&counter);

        define_and_push_layer(
            &mut engine,
            "nav",
            vec![(
                Key::H,
                Action::from(move || {
                    cc.fetch_add(1, Ordering::Relaxed);
                }),
            )],
        );

        // Toggle off
        engine
            .dispatcher
            .toggle_layer(kbd::layer::LayerName::from("nav"))
            .unwrap();

        // H should no longer match
        press_key(&mut engine, Key::H, 10);
        assert_eq!(counter.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn toggle_undefined_layer_returns_error() {
        let mut engine = test_engine();
        let result = engine
            .dispatcher
            .toggle_layer(kbd::layer::LayerName::from("nonexistent"));
        assert!(matches!(result, Err(kbd::error::Error::LayerNotDefined)));
    }

    #[test]
    fn layer_stack_priority_topmost_wins() {
        let mut engine = test_engine();

        let layer1_counter = Arc::new(AtomicUsize::new(0));
        let l1c = Arc::clone(&layer1_counter);
        define_and_push_layer(
            &mut engine,
            "layer1",
            vec![(
                Key::H,
                Action::from(move || {
                    l1c.fetch_add(1, Ordering::Relaxed);
                }),
            )],
        );

        let layer2_counter = Arc::new(AtomicUsize::new(0));
        let l2c = Arc::clone(&layer2_counter);
        define_and_push_layer(
            &mut engine,
            "layer2",
            vec![(
                Key::H,
                Action::from(move || {
                    l2c.fetch_add(1, Ordering::Relaxed);
                }),
            )],
        );

        press_key(&mut engine, Key::H, 10);

        assert_eq!(
            layer2_counter.load(Ordering::Relaxed),
            1,
            "topmost layer should fire"
        );
        assert_eq!(
            layer1_counter.load(Ordering::Relaxed),
            0,
            "lower layer should not fire"
        );
    }

    #[test]
    fn unmatched_key_falls_through_layers_to_global() {
        let mut engine = test_engine();

        let global_counter = Arc::new(AtomicUsize::new(0));
        let gc = Arc::clone(&global_counter);
        engine
            .dispatcher
            .register_binding(RegisteredBinding::new(
                BindingId::new(),
                Hotkey::new(Key::X),
                Action::from(move || {
                    gc.fetch_add(1, Ordering::Relaxed);
                }),
            ))
            .unwrap();

        define_and_push_layer(&mut engine, "nav", vec![(Key::H, Action::Suppress)]);

        // X not in layer, falls through to global
        press_key(&mut engine, Key::X, 10);
        assert_eq!(global_counter.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn swallow_layer_consumes_unmatched_keys() {
        let mut engine = test_engine();

        let global_counter = Arc::new(AtomicUsize::new(0));
        let gc = Arc::clone(&global_counter);
        engine
            .dispatcher
            .register_binding(RegisteredBinding::new(
                BindingId::new(),
                Hotkey::new(Key::X),
                Action::from(move || {
                    gc.fetch_add(1, Ordering::Relaxed);
                }),
            ))
            .unwrap();

        let layer = kbd::layer::Layer::new("modal")
            .bind(Key::H, Action::Suppress)
            .unwrap()
            .swallow();
        engine.dispatcher.define_layer(layer).unwrap();
        engine
            .dispatcher
            .push_layer(kbd::layer::LayerName::from("modal"))
            .unwrap();

        // X not in swallow layer — consumed, global should NOT fire
        let disposition = press_key(&mut engine, Key::X, 10);
        assert_eq!(disposition, KeyEventOutcome::MatchedConsumed);
        assert_eq!(global_counter.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn action_push_layer_activates_layer() {
        let mut engine = test_engine();

        let layer_counter = Arc::new(AtomicUsize::new(0));
        let lc = Arc::clone(&layer_counter);
        let layer = kbd::layer::Layer::new("nav")
            .bind(
                Key::H,
                Action::from(move || {
                    lc.fetch_add(1, Ordering::Relaxed);
                }),
            )
            .unwrap();
        engine.dispatcher.define_layer(layer).unwrap();

        // Register a global binding that pushes the layer
        engine
            .dispatcher
            .register_binding(RegisteredBinding::new(
                BindingId::new(),
                Hotkey::new(Key::F1),
                Action::PushLayer(kbd::layer::LayerName::from("nav")),
            ))
            .unwrap();

        // Press F1 to activate nav layer
        press_key(&mut engine, Key::F1, 10);

        // Now H should fire layer binding
        press_key(&mut engine, Key::H, 10);
        assert_eq!(layer_counter.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn action_pop_layer_deactivates_layer() {
        let mut engine = test_engine();

        let counter = Arc::new(AtomicUsize::new(0));
        let cc = Arc::clone(&counter);
        let layer = kbd::layer::Layer::new("nav")
            .bind(
                Key::H,
                Action::from(move || {
                    cc.fetch_add(1, Ordering::Relaxed);
                }),
            )
            .unwrap()
            .bind(Key::ESCAPE, Action::PopLayer)
            .unwrap();
        engine.dispatcher.define_layer(layer).unwrap();
        engine
            .dispatcher
            .push_layer(kbd::layer::LayerName::from("nav"))
            .unwrap();

        // H fires in nav layer
        press_key(&mut engine, Key::H, 10);
        release_key(&mut engine, Key::H, 10);
        assert_eq!(counter.load(Ordering::Relaxed), 1);

        // Escape pops the layer
        press_key(&mut engine, Key::ESCAPE, 10);
        release_key(&mut engine, Key::ESCAPE, 10);

        // H should no longer match
        press_key(&mut engine, Key::H, 10);
        assert_eq!(counter.load(Ordering::Relaxed), 1); // unchanged
    }

    #[test]
    fn action_toggle_layer_toggles() {
        let mut engine = test_engine();

        let counter = Arc::new(AtomicUsize::new(0));
        let cc = Arc::clone(&counter);
        let layer = kbd::layer::Layer::new("nav")
            .bind(
                Key::H,
                Action::from(move || {
                    cc.fetch_add(1, Ordering::Relaxed);
                }),
            )
            .unwrap();
        engine.dispatcher.define_layer(layer).unwrap();

        // Register toggle binding
        engine
            .dispatcher
            .register_binding(RegisteredBinding::new(
                BindingId::new(),
                Hotkey::new(Key::F2),
                Action::ToggleLayer(kbd::layer::LayerName::from("nav")),
            ))
            .unwrap();

        // Toggle on
        press_key(&mut engine, Key::F2, 10);
        release_key(&mut engine, Key::F2, 10);

        press_key(&mut engine, Key::H, 10);
        release_key(&mut engine, Key::H, 10);
        assert_eq!(counter.load(Ordering::Relaxed), 1);

        // Toggle off
        press_key(&mut engine, Key::F2, 10);
        release_key(&mut engine, Key::F2, 10);

        press_key(&mut engine, Key::H, 10);
        assert_eq!(counter.load(Ordering::Relaxed), 1); // unchanged
    }

    #[test]
    fn same_key_different_action_per_layer() {
        let mut engine = test_engine();

        let global_counter = Arc::new(AtomicUsize::new(0));
        let gc = Arc::clone(&global_counter);
        engine
            .dispatcher
            .register_binding(RegisteredBinding::new(
                BindingId::new(),
                Hotkey::new(Key::H),
                Action::from(move || {
                    gc.fetch_add(1, Ordering::Relaxed);
                }),
            ))
            .unwrap();

        let layer_counter = Arc::new(AtomicUsize::new(0));
        let lc = Arc::clone(&layer_counter);
        define_and_push_layer(
            &mut engine,
            "nav",
            vec![(
                Key::H,
                Action::from(move || {
                    lc.fetch_add(100, Ordering::Relaxed);
                }),
            )],
        );

        press_key(&mut engine, Key::H, 10);
        assert_eq!(global_counter.load(Ordering::Relaxed), 0);
        assert_eq!(layer_counter.load(Ordering::Relaxed), 100);

        // Pop layer, now global should fire
        engine.dispatcher.pop_layer().unwrap();
        release_key(&mut engine, Key::H, 10);
        press_key(&mut engine, Key::H, 10);
        assert_eq!(global_counter.load(Ordering::Relaxed), 1);
        assert_eq!(layer_counter.load(Ordering::Relaxed), 100);
    }

    #[test]
    fn oneshot_layer_auto_pops_after_n_keypresses() {
        let mut engine = test_engine();
        let counter = Arc::new(AtomicUsize::new(0));
        let cc = Arc::clone(&counter);

        let layer = kbd::layer::Layer::new("oneshot")
            .bind(
                Key::H,
                Action::from(move || {
                    cc.fetch_add(1, Ordering::Relaxed);
                }),
            )
            .unwrap()
            .oneshot(1);
        engine.dispatcher.define_layer(layer).unwrap();
        engine
            .dispatcher
            .push_layer(kbd::layer::LayerName::from("oneshot"))
            .unwrap();

        // First keypress — should match and auto-pop
        press_key(&mut engine, Key::H, 10);
        release_key(&mut engine, Key::H, 10);
        assert_eq!(counter.load(Ordering::Relaxed), 1);

        // Second keypress — layer should be gone
        press_key(&mut engine, Key::H, 10);
        assert_eq!(counter.load(Ordering::Relaxed), 1); // unchanged
    }

    #[test]
    fn oneshot_layer_counts_unmatched_keys() {
        let mut engine = test_engine();
        let counter = Arc::new(AtomicUsize::new(0));
        let cc = Arc::clone(&counter);

        let layer = kbd::layer::Layer::new("oneshot")
            .bind(
                Key::H,
                Action::from(move || {
                    cc.fetch_add(1, Ordering::Relaxed);
                }),
            )
            .unwrap()
            .oneshot(1);
        engine.dispatcher.define_layer(layer).unwrap();
        engine
            .dispatcher
            .push_layer(kbd::layer::LayerName::from("oneshot"))
            .unwrap();

        // Press an unmatched key — should count toward oneshot depth and pop
        press_key(&mut engine, Key::X, 10);
        release_key(&mut engine, Key::X, 10);

        // Layer should be gone — H shouldn't match
        press_key(&mut engine, Key::H, 10);
        assert_eq!(counter.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn oneshot_layer_with_depth_two() {
        let mut engine = test_engine();
        let counter = Arc::new(AtomicUsize::new(0));
        let cc = Arc::clone(&counter);

        let layer = kbd::layer::Layer::new("oneshot2")
            .bind(
                Key::H,
                Action::from(move || {
                    cc.fetch_add(1, Ordering::Relaxed);
                }),
            )
            .unwrap()
            .oneshot(2);
        engine.dispatcher.define_layer(layer).unwrap();
        engine
            .dispatcher
            .push_layer(kbd::layer::LayerName::from("oneshot2"))
            .unwrap();

        // First keypress — layer still active
        press_key(&mut engine, Key::H, 10);
        release_key(&mut engine, Key::H, 10);
        assert_eq!(counter.load(Ordering::Relaxed), 1);

        // Second keypress — should match, then auto-pop
        press_key(&mut engine, Key::H, 10);
        release_key(&mut engine, Key::H, 10);
        assert_eq!(counter.load(Ordering::Relaxed), 2);

        // Third keypress — layer gone
        press_key(&mut engine, Key::H, 10);
        assert_eq!(counter.load(Ordering::Relaxed), 2);
    }

    #[test]
    fn push_layer_via_runtime_command() {
        let runtime = EngineRuntime::spawn(GrabState::Disabled).expect("engine should spawn");

        // Define layer
        let layer = kbd::layer::Layer::new("nav")
            .bind(Key::H, Action::Suppress)
            .unwrap();
        let (reply_tx, reply_rx) = mpsc::channel();
        runtime
            .commands()
            .send(Command::DefineLayer {
                layer,
                reply: reply_tx,
            })
            .unwrap();
        reply_rx
            .recv_timeout(Duration::from_secs(1))
            .unwrap()
            .unwrap();

        // Push layer
        let (reply_tx, reply_rx) = mpsc::channel();
        runtime
            .commands()
            .send(Command::PushLayer {
                name: kbd::layer::LayerName::from("nav"),
                reply: reply_tx,
            })
            .unwrap();
        let result = reply_rx.recv_timeout(Duration::from_secs(1)).unwrap();
        assert!(result.is_ok());

        runtime.shutdown().unwrap();
    }

    #[test]
    fn push_undefined_layer_via_runtime_returns_error() {
        let runtime = EngineRuntime::spawn(GrabState::Disabled).expect("engine should spawn");

        let (reply_tx, reply_rx) = mpsc::channel();
        runtime
            .commands()
            .send(Command::PushLayer {
                name: kbd::layer::LayerName::from("nonexistent"),
                reply: reply_tx,
            })
            .unwrap();
        let result = reply_rx.recv_timeout(Duration::from_secs(1)).unwrap();
        assert!(matches!(result, Err(Error::LayerNotDefined)));

        runtime.shutdown().unwrap();
    }

    #[test]
    fn pop_layer_via_runtime_command() {
        let runtime = EngineRuntime::spawn(GrabState::Disabled).expect("engine should spawn");

        // Define and push layer
        let layer = kbd::layer::Layer::new("nav")
            .bind(Key::H, Action::Suppress)
            .unwrap();
        let (reply_tx, reply_rx) = mpsc::channel();
        runtime
            .commands()
            .send(Command::DefineLayer {
                layer,
                reply: reply_tx,
            })
            .unwrap();
        reply_rx
            .recv_timeout(Duration::from_secs(1))
            .unwrap()
            .unwrap();

        let (reply_tx, reply_rx) = mpsc::channel();
        runtime
            .commands()
            .send(Command::PushLayer {
                name: kbd::layer::LayerName::from("nav"),
                reply: reply_tx,
            })
            .unwrap();
        reply_rx
            .recv_timeout(Duration::from_secs(1))
            .unwrap()
            .unwrap();

        // Pop layer
        let (reply_tx, reply_rx) = mpsc::channel();
        runtime
            .commands()
            .send(Command::PopLayer { reply: reply_tx })
            .unwrap();
        let result = reply_rx.recv_timeout(Duration::from_secs(1)).unwrap();
        assert!(result.is_ok());
        assert_eq!(result.unwrap().as_str(), "nav");

        runtime.shutdown().unwrap();
    }

    #[test]
    fn pop_empty_stack_via_runtime_returns_error() {
        let runtime = EngineRuntime::spawn(GrabState::Disabled).expect("engine should spawn");

        let (reply_tx, reply_rx) = mpsc::channel();
        runtime
            .commands()
            .send(Command::PopLayer { reply: reply_tx })
            .unwrap();
        let result = reply_rx.recv_timeout(Duration::from_secs(1)).unwrap();
        assert!(matches!(result, Err(Error::EmptyLayerStack)));

        runtime.shutdown().unwrap();
    }

    #[test]
    fn timeout_layer_auto_pops_after_inactivity() {
        let mut engine = test_engine();
        let counter = Arc::new(AtomicUsize::new(0));
        let cc = Arc::clone(&counter);

        let layer = kbd::layer::Layer::new("timed")
            .bind(
                Key::H,
                Action::from(move || {
                    cc.fetch_add(1, Ordering::Relaxed);
                }),
            )
            .unwrap()
            .timeout(Duration::from_millis(50));
        engine.dispatcher.define_layer(layer).unwrap();
        engine
            .dispatcher
            .push_layer(kbd::layer::LayerName::from("timed"))
            .unwrap();

        // H fires while layer is active
        press_key(&mut engine, Key::H, 10);
        release_key(&mut engine, Key::H, 10);
        assert_eq!(counter.load(Ordering::Relaxed), 1);

        // Wait for timeout to expire
        std::thread::sleep(Duration::from_millis(80));

        // Check timeouts (simulating the engine loop check)
        engine.dispatcher.check_timeouts();

        // Layer should be gone — H no longer matches
        press_key(&mut engine, Key::H, 10);
        assert_eq!(counter.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn timeout_layer_resets_on_activity() {
        let mut engine = test_engine();
        let counter = Arc::new(AtomicUsize::new(0));
        let cc = Arc::clone(&counter);

        let layer = kbd::layer::Layer::new("timed")
            .bind(
                Key::H,
                Action::from(move || {
                    cc.fetch_add(1, Ordering::Relaxed);
                }),
            )
            .unwrap()
            .timeout(Duration::from_millis(100));
        engine.dispatcher.define_layer(layer).unwrap();
        engine
            .dispatcher
            .push_layer(kbd::layer::LayerName::from("timed"))
            .unwrap();

        // Activity within the timeout window
        std::thread::sleep(Duration::from_millis(50));
        press_key(&mut engine, Key::H, 10);
        release_key(&mut engine, Key::H, 10);
        assert_eq!(counter.load(Ordering::Relaxed), 1);

        // Wait a bit more but not enough from last activity
        std::thread::sleep(Duration::from_millis(50));
        engine.dispatcher.check_timeouts();

        // Layer should still be active
        press_key(&mut engine, Key::H, 10);
        release_key(&mut engine, Key::H, 10);
        assert_eq!(counter.load(Ordering::Relaxed), 2);

        // Now wait for full timeout from last activity
        std::thread::sleep(Duration::from_millis(120));
        engine.dispatcher.check_timeouts();

        // Layer should be gone
        press_key(&mut engine, Key::H, 10);
        assert_eq!(counter.load(Ordering::Relaxed), 2);
    }

    #[test]
    fn toggle_layer_via_runtime_command() {
        let runtime = EngineRuntime::spawn(GrabState::Disabled).expect("engine should spawn");

        // Define layer
        let layer = kbd::layer::Layer::new("nav")
            .bind(Key::H, Action::Suppress)
            .unwrap();
        let (reply_tx, reply_rx) = mpsc::channel();
        runtime
            .commands()
            .send(Command::DefineLayer {
                layer,
                reply: reply_tx,
            })
            .unwrap();
        reply_rx
            .recv_timeout(Duration::from_secs(1))
            .unwrap()
            .unwrap();

        // Toggle on
        let (reply_tx, reply_rx) = mpsc::channel();
        runtime
            .commands()
            .send(Command::ToggleLayer {
                name: kbd::layer::LayerName::from("nav"),
                reply: reply_tx,
            })
            .unwrap();
        let result = reply_rx.recv_timeout(Duration::from_secs(1)).unwrap();
        assert!(result.is_ok());

        runtime.shutdown().unwrap();
    }

    // Press cache tests (Section 3.3)

    #[test]
    fn press_cache_release_consumed_when_press_was_consumed() {
        let (grab_state, forwarded) = test_grab_state();
        let mut engine = test_engine_with_grab(grab_state);

        engine
            .dispatcher
            .register_binding(RegisteredBinding::new(
                BindingId::new(),
                Hotkey::new(Key::C).modifier(Modifier::Ctrl),
                Action::Suppress,
            ))
            .unwrap();

        press_key(&mut engine, Key::CONTROL_LEFT, 10);
        press_key(&mut engine, Key::C, 10);

        let disposition = release_key(&mut engine, Key::C, 10);
        assert_eq!(disposition, KeyEventOutcome::MatchedConsumed);

        let events = forwarded.lock().unwrap();
        let c_events: Vec<_> = events.iter().filter(|(key, _)| *key == Key::C).collect();
        assert!(
            c_events.is_empty(),
            "consumed key's release should not be forwarded"
        );
    }

    #[test]
    fn press_cache_release_forwarded_when_press_had_passthrough() {
        let (grab_state, forwarded) = test_grab_state();
        let mut engine = test_engine_with_grab(grab_state);

        engine
            .dispatcher
            .register_binding(
                RegisteredBinding::new(
                    BindingId::new(),
                    Hotkey::new(Key::C).modifier(Modifier::Ctrl),
                    Action::Suppress,
                )
                .with_propagation(KeyPropagation::Continue),
            )
            .unwrap();

        press_key(&mut engine, Key::CONTROL_LEFT, 10);
        let press_disp = press_key(&mut engine, Key::C, 10);
        assert_eq!(press_disp, KeyEventOutcome::MatchedForwarded);

        let release_disp = release_key(&mut engine, Key::C, 10);
        assert_eq!(release_disp, KeyEventOutcome::MatchedForwarded);

        let events = forwarded.lock().unwrap();
        let c_events: Vec<_> = events.iter().filter(|(key, _)| *key == Key::C).collect();
        assert_eq!(
            c_events.len(),
            2,
            "passthrough key should be forwarded on both press and release"
        );
    }

    #[test]
    fn press_cache_release_consumed_when_swallowed() {
        let (grab_state, forwarded) = test_grab_state();
        let mut engine = test_engine_with_grab(grab_state);

        let layer = kbd::layer::Layer::new("modal")
            .bind(Key::H, Action::Suppress)
            .unwrap()
            .swallow();
        engine.dispatcher.define_layer(layer).unwrap();
        engine
            .dispatcher
            .push_layer(kbd::layer::LayerName::from("modal"))
            .unwrap();

        let press_disp = press_key(&mut engine, Key::X, 10);
        assert_eq!(press_disp, KeyEventOutcome::MatchedConsumed);

        let release_disp = release_key(&mut engine, Key::X, 10);
        assert_eq!(release_disp, KeyEventOutcome::MatchedConsumed);

        let events = forwarded.lock().unwrap();
        let x_events: Vec<_> = events.iter().filter(|(key, _)| *key == Key::X).collect();
        assert!(
            x_events.is_empty(),
            "swallowed key should not be forwarded on release"
        );
    }

    #[test]
    fn press_cache_cleared_after_release() {
        let (grab_state, _forwarded) = test_grab_state();
        let mut engine = test_engine_with_grab(grab_state);

        let counter = Arc::new(AtomicUsize::new(0));
        let cc = Arc::clone(&counter);

        engine
            .dispatcher
            .register_binding(RegisteredBinding::new(
                BindingId::new(),
                Hotkey::new(Key::C).modifier(Modifier::Ctrl),
                Action::from(move || {
                    cc.fetch_add(1, Ordering::Relaxed);
                }),
            ))
            .unwrap();

        press_key(&mut engine, Key::CONTROL_LEFT, 10);
        press_key(&mut engine, Key::C, 10);
        let release_disp = release_key(&mut engine, Key::C, 10);
        assert_eq!(release_disp, KeyEventOutcome::MatchedConsumed);

        let second_press_disp = press_key(&mut engine, Key::C, 10);
        assert_eq!(second_press_disp, KeyEventOutcome::MatchedConsumed);
        assert_eq!(counter.load(Ordering::Relaxed), 2);
    }

    #[test]
    fn press_cache_layer_pop_during_press_release_correct() {
        let (grab_state, forwarded) = test_grab_state();
        let mut engine = test_engine_with_grab(grab_state);

        let layer = kbd::layer::Layer::new("nav")
            .bind(Key::H, Action::PopLayer)
            .unwrap()
            .bind(Key::J, Action::Suppress)
            .unwrap();
        engine.dispatcher.define_layer(layer).unwrap();
        engine
            .dispatcher
            .push_layer(kbd::layer::LayerName::from("nav"))
            .unwrap();

        let press_disp = press_key(&mut engine, Key::H, 10);
        assert_eq!(press_disp, KeyEventOutcome::MatchedConsumed);
        assert!(
            engine.dispatcher.active_layers().is_empty(),
            "layer should have been popped"
        );

        let release_disp = release_key(&mut engine, Key::H, 10);
        assert_eq!(release_disp, KeyEventOutcome::MatchedConsumed);

        let events = forwarded.lock().unwrap();
        let h_events: Vec<_> = events.iter().filter(|(key, _)| *key == Key::H).collect();
        assert!(
            h_events.is_empty(),
            "press cache should prevent release forwarding after layer pop"
        );
    }

    #[test]
    fn press_cache_repeat_consumed_when_press_was_consumed() {
        let (grab_state, forwarded) = test_grab_state();
        let mut engine = test_engine_with_grab(grab_state);

        engine
            .dispatcher
            .register_binding(RegisteredBinding::new(
                BindingId::new(),
                Hotkey::new(Key::C).modifier(Modifier::Ctrl),
                Action::Suppress,
            ))
            .unwrap();

        press_key(&mut engine, Key::CONTROL_LEFT, 10);
        press_key(&mut engine, Key::C, 10);

        let disposition = engine.process_key_event(DeviceKeyEvent {
            device_fd: 10,
            key: Key::C,
            transition: KeyTransition::Repeat,
        });
        assert_eq!(disposition, KeyEventOutcome::MatchedConsumed);

        let release_disposition = release_key(&mut engine, Key::C, 10);
        assert_eq!(release_disposition, KeyEventOutcome::MatchedConsumed);

        let events = forwarded.lock().unwrap();
        let c_events: Vec<_> = events.iter().filter(|(key, _)| *key == Key::C).collect();
        assert!(
            c_events.is_empty(),
            "consumed key's repeat and release should not be forwarded"
        );
    }

    #[test]
    fn press_cache_repeat_forwarded_when_press_had_passthrough() {
        let (grab_state, forwarded) = test_grab_state();
        let mut engine = test_engine_with_grab(grab_state);

        engine
            .dispatcher
            .register_binding(
                RegisteredBinding::new(
                    BindingId::new(),
                    Hotkey::new(Key::C).modifier(Modifier::Ctrl),
                    Action::Suppress,
                )
                .with_propagation(KeyPropagation::Continue),
            )
            .unwrap();

        press_key(&mut engine, Key::CONTROL_LEFT, 10);
        press_key(&mut engine, Key::C, 10);

        let disposition = engine.process_key_event(DeviceKeyEvent {
            device_fd: 10,
            key: Key::C,
            transition: KeyTransition::Repeat,
        });
        assert_eq!(disposition, KeyEventOutcome::MatchedForwarded);

        let events = forwarded.lock().unwrap();
        let c_events: Vec<_> = events.iter().filter(|(key, _)| *key == Key::C).collect();
        assert_eq!(
            c_events.len(),
            2,
            "passthrough key should be forwarded on both press and repeat"
        );
    }

    // Introspection tests (Section 3.5)

    #[test]
    fn list_bindings_returns_global_binding() {
        let mut engine = test_engine();

        engine
            .dispatcher
            .register_binding(
                RegisteredBinding::new(
                    BindingId::new(),
                    Hotkey::new(Key::C).modifier(Modifier::Ctrl),
                    Action::Suppress,
                )
                .with_options(kbd::binding::BindingOptions::default().with_description("Copy")),
            )
            .unwrap();

        let bindings = engine.dispatcher.list_bindings();
        assert_eq!(bindings.len(), 1);

        let info = &bindings[0];
        assert_eq!(info.hotkey, Hotkey::new(Key::C).modifier(Modifier::Ctrl));
        assert_eq!(info.description.as_deref(), Some("Copy"));
        assert_eq!(info.location, kbd::introspection::BindingLocation::Global);
        assert_eq!(info.shadowed, kbd::introspection::ShadowedStatus::Active);
    }

    #[test]
    fn list_bindings_includes_layer_bindings() {
        let mut engine = test_engine();

        let layer = kbd::layer::Layer::new("nav")
            .bind(Key::H, Action::Suppress)
            .unwrap()
            .bind(Key::J, Action::Suppress)
            .unwrap();
        engine.dispatcher.define_layer(layer).unwrap();

        let bindings = engine.dispatcher.list_bindings();
        let layer_bindings: Vec<_> = bindings
            .iter()
            .filter(|b| matches!(b.location, kbd::introspection::BindingLocation::Layer(_)))
            .collect();
        assert_eq!(layer_bindings.len(), 2);
    }

    #[test]
    fn list_bindings_inactive_layer_binding_marked_inactive() {
        let mut engine = test_engine();

        let layer = kbd::layer::Layer::new("nav")
            .bind(Key::H, Action::Suppress)
            .unwrap();
        engine.dispatcher.define_layer(layer).unwrap();

        let bindings = engine.dispatcher.list_bindings();
        let nav_binding = bindings
            .iter()
            .find(|b| b.hotkey == Hotkey::new(Key::H))
            .expect("should find H binding");
        assert_eq!(
            nav_binding.shadowed,
            kbd::introspection::ShadowedStatus::Inactive
        );
    }

    #[test]
    fn list_bindings_detects_shadowed_global_binding() {
        let mut engine = test_engine();

        engine
            .dispatcher
            .register_binding(RegisteredBinding::new(
                BindingId::new(),
                Hotkey::new(Key::H),
                Action::Suppress,
            ))
            .unwrap();

        let layer = kbd::layer::Layer::new("nav")
            .bind(Key::H, Action::Suppress)
            .unwrap();
        engine.dispatcher.define_layer(layer).unwrap();
        engine
            .dispatcher
            .push_layer(kbd::layer::LayerName::from("nav"))
            .unwrap();

        let bindings = engine.dispatcher.list_bindings();

        let global_h = bindings
            .iter()
            .find(|b| {
                b.hotkey == Hotkey::new(Key::H)
                    && matches!(b.location, kbd::introspection::BindingLocation::Global)
            })
            .expect("should find global H");
        assert_eq!(
            global_h.shadowed,
            kbd::introspection::ShadowedStatus::ShadowedBy(kbd::layer::LayerName::from("nav"))
        );

        let layer_h = bindings
            .iter()
            .find(|b| {
                b.hotkey == Hotkey::new(Key::H)
                    && matches!(b.location, kbd::introspection::BindingLocation::Layer(_))
            })
            .expect("should find layer H");
        assert_eq!(layer_h.shadowed, kbd::introspection::ShadowedStatus::Active);
    }

    #[test]
    fn list_bindings_higher_layer_shadows_lower_layer() {
        let mut engine = test_engine();

        let layer1 = kbd::layer::Layer::new("layer1")
            .bind(Key::H, Action::Suppress)
            .unwrap();
        engine.dispatcher.define_layer(layer1).unwrap();

        let layer2 = kbd::layer::Layer::new("layer2")
            .bind(Key::H, Action::Suppress)
            .unwrap();
        engine.dispatcher.define_layer(layer2).unwrap();

        engine
            .dispatcher
            .push_layer(kbd::layer::LayerName::from("layer1"))
            .unwrap();
        engine
            .dispatcher
            .push_layer(kbd::layer::LayerName::from("layer2"))
            .unwrap();

        let bindings = engine.dispatcher.list_bindings();

        let layer1_h = bindings
            .iter()
            .find(|b| {
                b.hotkey == Hotkey::new(Key::H)
                    && b.location
                        == kbd::introspection::BindingLocation::Layer(kbd::layer::LayerName::from(
                            "layer1",
                        ))
            })
            .expect("should find layer1 H");
        assert_eq!(
            layer1_h.shadowed,
            kbd::introspection::ShadowedStatus::ShadowedBy(kbd::layer::LayerName::from("layer2"))
        );

        let layer2_h = bindings
            .iter()
            .find(|b| {
                b.hotkey == Hotkey::new(Key::H)
                    && b.location
                        == kbd::introspection::BindingLocation::Layer(kbd::layer::LayerName::from(
                            "layer2",
                        ))
            })
            .expect("should find layer2 H");
        assert_eq!(
            layer2_h.shadowed,
            kbd::introspection::ShadowedStatus::Active
        );
    }

    #[test]
    fn binding_for_key_returns_matching_global_binding() {
        let mut engine = test_engine();

        engine
            .dispatcher
            .register_binding(
                RegisteredBinding::new(
                    BindingId::new(),
                    Hotkey::new(Key::C).modifier(Modifier::Ctrl),
                    Action::Suppress,
                )
                .with_options(kbd::binding::BindingOptions::default().with_description("Copy")),
            )
            .unwrap();

        let result = engine
            .dispatcher
            .bindings_for_key(&Hotkey::new(Key::C).modifier(Modifier::Ctrl));
        assert!(result.is_some());

        let info = result.unwrap();
        assert_eq!(info.hotkey, Hotkey::new(Key::C).modifier(Modifier::Ctrl));
        assert_eq!(info.description.as_deref(), Some("Copy"));
        assert_eq!(info.location, kbd::introspection::BindingLocation::Global);
    }

    #[test]
    fn binding_for_key_returns_none_when_no_match() {
        let mut engine = test_engine();

        engine
            .dispatcher
            .register_binding(RegisteredBinding::new(
                BindingId::new(),
                Hotkey::new(Key::C).modifier(Modifier::Ctrl),
                Action::Suppress,
            ))
            .unwrap();

        let result = engine
            .dispatcher
            .bindings_for_key(&Hotkey::new(Key::V).modifier(Modifier::Ctrl));
        assert!(result.is_none());
    }

    #[test]
    fn binding_for_key_respects_layer_stack() {
        let mut engine = test_engine();

        engine
            .dispatcher
            .register_binding(
                RegisteredBinding::new(BindingId::new(), Hotkey::new(Key::H), Action::Suppress)
                    .with_options(
                        kbd::binding::BindingOptions::default().with_description("Global H"),
                    ),
            )
            .unwrap();

        let layer = kbd::layer::Layer::new("nav")
            .bind(Key::H, Action::Suppress)
            .unwrap();
        engine.dispatcher.define_layer(layer).unwrap();
        engine
            .dispatcher
            .push_layer(kbd::layer::LayerName::from("nav"))
            .unwrap();

        let result = engine.dispatcher.bindings_for_key(&Hotkey::new(Key::H));
        assert!(result.is_some());

        let info = result.unwrap();
        assert_eq!(
            info.location,
            kbd::introspection::BindingLocation::Layer(kbd::layer::LayerName::from("nav"))
        );
    }

    #[test]
    fn active_layers_returns_empty_when_no_layers_pushed() {
        let engine = test_engine();
        let layers = engine.dispatcher.active_layers();
        assert!(layers.is_empty());
    }

    #[test]
    fn active_layers_returns_stack_in_order() {
        let mut engine = test_engine();

        let layer1 = kbd::layer::Layer::new("layer1")
            .bind(Key::H, Action::Suppress)
            .unwrap()
            .description("First layer");
        engine.dispatcher.define_layer(layer1).unwrap();

        let layer2 = kbd::layer::Layer::new("layer2")
            .bind(Key::J, Action::Suppress)
            .unwrap()
            .bind(Key::K, Action::Suppress)
            .unwrap()
            .description("Second layer");
        engine.dispatcher.define_layer(layer2).unwrap();

        engine
            .dispatcher
            .push_layer(kbd::layer::LayerName::from("layer1"))
            .unwrap();
        engine
            .dispatcher
            .push_layer(kbd::layer::LayerName::from("layer2"))
            .unwrap();

        let active = engine.dispatcher.active_layers();
        assert_eq!(active.len(), 2);

        assert_eq!(active[0].name.as_str(), "layer1");
        assert_eq!(active[0].description.as_deref(), Some("First layer"));
        assert_eq!(active[0].binding_count, 1);

        assert_eq!(active[1].name.as_str(), "layer2");
        assert_eq!(active[1].description.as_deref(), Some("Second layer"));
        assert_eq!(active[1].binding_count, 2);
    }

    #[test]
    fn conflicts_returns_empty_when_no_conflicts() {
        let mut engine = test_engine();

        engine
            .dispatcher
            .register_binding(RegisteredBinding::new(
                BindingId::new(),
                Hotkey::new(Key::C).modifier(Modifier::Ctrl),
                Action::Suppress,
            ))
            .unwrap();

        let conflicts = engine.dispatcher.conflicts();
        assert!(conflicts.is_empty());
    }

    #[test]
    fn conflicts_detects_layer_shadowing_global() {
        let mut engine = test_engine();

        engine
            .dispatcher
            .register_binding(RegisteredBinding::new(
                BindingId::new(),
                Hotkey::new(Key::H),
                Action::Suppress,
            ))
            .unwrap();

        let layer = kbd::layer::Layer::new("nav")
            .bind(Key::H, Action::Suppress)
            .unwrap();
        engine.dispatcher.define_layer(layer).unwrap();
        engine
            .dispatcher
            .push_layer(kbd::layer::LayerName::from("nav"))
            .unwrap();

        let conflicts = engine.dispatcher.conflicts();
        assert_eq!(conflicts.len(), 1);

        let conflict = &conflicts[0];
        assert_eq!(conflict.hotkey, Hotkey::new(Key::H));
        assert_eq!(
            conflict.shadowed_binding.location,
            kbd::introspection::BindingLocation::Global
        );
        assert_eq!(
            conflict.shadowing_binding.location,
            kbd::introspection::BindingLocation::Layer(kbd::layer::LayerName::from("nav"))
        );
    }

    #[test]
    fn conflicts_detects_layer_shadowing_lower_layer() {
        let mut engine = test_engine();

        let layer1 = kbd::layer::Layer::new("layer1")
            .bind(Key::H, Action::Suppress)
            .unwrap();
        engine.dispatcher.define_layer(layer1).unwrap();

        let layer2 = kbd::layer::Layer::new("layer2")
            .bind(Key::H, Action::Suppress)
            .unwrap();
        engine.dispatcher.define_layer(layer2).unwrap();

        engine
            .dispatcher
            .push_layer(kbd::layer::LayerName::from("layer1"))
            .unwrap();
        engine
            .dispatcher
            .push_layer(kbd::layer::LayerName::from("layer2"))
            .unwrap();

        let conflicts = engine.dispatcher.conflicts();
        assert_eq!(conflicts.len(), 1);

        let conflict = &conflicts[0];
        assert_eq!(conflict.hotkey, Hotkey::new(Key::H));
        assert_eq!(
            conflict.shadowed_binding.location,
            kbd::introspection::BindingLocation::Layer(kbd::layer::LayerName::from("layer1"))
        );
        assert_eq!(
            conflict.shadowing_binding.location,
            kbd::introspection::BindingLocation::Layer(kbd::layer::LayerName::from("layer2"))
        );
    }

    #[test]
    fn introspection_via_runtime_commands() {
        let runtime = EngineRuntime::spawn(GrabState::Disabled).expect("engine should spawn");

        let (reply_tx, reply_rx) = mpsc::channel();
        runtime
            .commands()
            .send(Command::Register {
                binding: RegisteredBinding::new(
                    BindingId::new(),
                    Hotkey::new(Key::C).modifier(Modifier::Ctrl),
                    Action::Suppress,
                )
                .with_options(kbd::binding::BindingOptions::default().with_description("Copy")),
                reply: reply_tx,
            })
            .unwrap();
        reply_rx
            .recv_timeout(Duration::from_secs(1))
            .unwrap()
            .unwrap();

        let (reply_tx, reply_rx) = mpsc::channel();
        runtime
            .commands()
            .send(Command::ListBindings { reply: reply_tx })
            .unwrap();
        let bindings = reply_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("should receive reply");
        assert_eq!(bindings.len(), 1);
        assert_eq!(bindings[0].description.as_deref(), Some("Copy"));

        let (reply_tx, reply_rx) = mpsc::channel();
        runtime
            .commands()
            .send(Command::BindingsForKey {
                hotkey: Hotkey::new(Key::C).modifier(Modifier::Ctrl),
                reply: reply_tx,
            })
            .unwrap();
        let result = reply_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("should receive reply");
        assert!(result.is_some());

        let (reply_tx, reply_rx) = mpsc::channel();
        runtime
            .commands()
            .send(Command::ActiveLayers { reply: reply_tx })
            .unwrap();
        let layers = reply_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("should receive reply");
        assert!(layers.is_empty());

        let (reply_tx, reply_rx) = mpsc::channel();
        runtime
            .commands()
            .send(Command::Conflicts { reply: reply_tx })
            .unwrap();
        let conflicts = reply_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("should receive reply");
        assert!(conflicts.is_empty());

        runtime.shutdown().unwrap();
    }

    #[test]
    fn list_bindings_overlay_visibility_preserved() {
        let mut engine = test_engine();

        engine
            .dispatcher
            .register_binding(
                RegisteredBinding::new(
                    BindingId::new(),
                    Hotkey::new(Key::C).modifier(Modifier::Ctrl),
                    Action::Suppress,
                )
                .with_options(
                    kbd::binding::BindingOptions::default()
                        .with_overlay_visibility(kbd::binding::OverlayVisibility::Hidden),
                ),
            )
            .unwrap();

        let bindings = engine.dispatcher.list_bindings();
        assert_eq!(bindings.len(), 1);
        assert_eq!(
            bindings[0].overlay_visibility,
            kbd::binding::OverlayVisibility::Hidden
        );
    }

    #[test]
    fn binding_for_key_respects_swallow_layer() {
        let mut engine = test_engine();

        engine
            .dispatcher
            .register_binding(RegisteredBinding::new(
                BindingId::new(),
                Hotkey::new(Key::X),
                Action::Suppress,
            ))
            .unwrap();

        let layer = kbd::layer::Layer::new("modal")
            .bind(Key::H, Action::Suppress)
            .unwrap()
            .swallow();
        engine.dispatcher.define_layer(layer).unwrap();
        engine
            .dispatcher
            .push_layer(kbd::layer::LayerName::from("modal"))
            .unwrap();

        let result = engine.dispatcher.bindings_for_key(&Hotkey::new(Key::X));
        assert!(
            result.is_none(),
            "swallow layer should block fallthrough to global binding"
        );

        let result = engine.dispatcher.bindings_for_key(&Hotkey::new(Key::H));
        assert!(result.is_some());
    }

    #[test]
    fn binding_for_key_returns_none_for_modifier_key() {
        let mut engine = test_engine();

        engine
            .dispatcher
            .register_binding(RegisteredBinding::new(
                BindingId::new(),
                Hotkey::new(Key::CONTROL_LEFT),
                Action::Suppress,
            ))
            .unwrap();

        let result = engine
            .dispatcher
            .bindings_for_key(&Hotkey::new(Key::CONTROL_LEFT));
        assert!(
            result.is_none(),
            "modifier-only key should not match, consistent with real dispatcher"
        );
    }

    // Repeat policy tests

    #[test]
    fn repeat_suppress_does_not_refire_callback() {
        let mut engine = test_engine();
        let counter = Arc::new(AtomicUsize::new(0));
        let cc = Arc::clone(&counter);

        let binding = RegisteredBinding::new(
            BindingId::new(),
            Hotkey::new(Key::A),
            Action::from(move || {
                cc.fetch_add(1, Ordering::Relaxed);
            }),
        );
        engine.dispatcher.register_binding(binding).unwrap();

        press_key(&mut engine, Key::A, 10);
        assert_eq!(counter.load(Ordering::Relaxed), 1);

        repeat_key(&mut engine, Key::A, 10);
        assert_eq!(counter.load(Ordering::Relaxed), 1);

        repeat_key(&mut engine, Key::A, 10);
        assert_eq!(counter.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn repeat_allow_refires_callback() {
        let mut engine = test_engine();
        let counter = Arc::new(AtomicUsize::new(0));
        let cc = Arc::clone(&counter);

        let binding = RegisteredBinding::new(
            BindingId::new(),
            Hotkey::new(Key::A),
            Action::from(move || {
                cc.fetch_add(1, Ordering::Relaxed);
            }),
        )
        .with_options(BindingOptions::default().with_repeat_policy(RepeatPolicy::Allow));
        engine.dispatcher.register_binding(binding).unwrap();

        press_key(&mut engine, Key::A, 10);
        assert_eq!(counter.load(Ordering::Relaxed), 1);

        repeat_key(&mut engine, Key::A, 10);
        assert_eq!(counter.load(Ordering::Relaxed), 2);

        repeat_key(&mut engine, Key::A, 10);
        assert_eq!(counter.load(Ordering::Relaxed), 3);
    }

    #[test]
    fn repeat_custom_respects_initial_delay() {
        let mut engine = test_engine();
        let counter = Arc::new(AtomicUsize::new(0));
        let cc = Arc::clone(&counter);

        let binding = RegisteredBinding::new(
            BindingId::new(),
            Hotkey::new(Key::A),
            Action::from(move || {
                cc.fetch_add(1, Ordering::Relaxed);
            }),
        )
        .with_options(
            BindingOptions::default().with_repeat_policy(RepeatPolicy::Custom {
                delay: Duration::from_millis(50),
                rate: Duration::from_millis(10),
            }),
        );
        engine.dispatcher.register_binding(binding).unwrap();

        press_key(&mut engine, Key::A, 10);
        assert_eq!(counter.load(Ordering::Relaxed), 1);

        repeat_key(&mut engine, Key::A, 10);
        assert_eq!(counter.load(Ordering::Relaxed), 1);

        std::thread::sleep(Duration::from_millis(55));

        repeat_key(&mut engine, Key::A, 10);
        assert_eq!(counter.load(Ordering::Relaxed), 2);
    }

    #[test]
    fn repeat_custom_respects_rate() {
        let mut engine = test_engine();
        let counter = Arc::new(AtomicUsize::new(0));
        let cc = Arc::clone(&counter);

        let binding = RegisteredBinding::new(
            BindingId::new(),
            Hotkey::new(Key::A),
            Action::from(move || {
                cc.fetch_add(1, Ordering::Relaxed);
            }),
        )
        .with_options(
            BindingOptions::default().with_repeat_policy(RepeatPolicy::Custom {
                delay: Duration::from_millis(0),
                rate: Duration::from_millis(50),
            }),
        );
        engine.dispatcher.register_binding(binding).unwrap();

        press_key(&mut engine, Key::A, 10);
        assert_eq!(counter.load(Ordering::Relaxed), 1);

        repeat_key(&mut engine, Key::A, 10);
        assert_eq!(counter.load(Ordering::Relaxed), 2);

        repeat_key(&mut engine, Key::A, 10);
        assert_eq!(counter.load(Ordering::Relaxed), 2);

        std::thread::sleep(Duration::from_millis(55));

        repeat_key(&mut engine, Key::A, 10);
        assert_eq!(counter.load(Ordering::Relaxed), 3);
    }

    // Debounce and rate limit in engine

    #[test]
    fn debounce_suppresses_rapid_repress_through_engine() {
        let mut engine = test_engine();
        let counter = Arc::new(AtomicUsize::new(0));
        let cc = Arc::clone(&counter);

        let binding = RegisteredBinding::new(
            BindingId::new(),
            Hotkey::new(Key::A),
            Action::from(move || {
                cc.fetch_add(1, Ordering::Relaxed);
            }),
        )
        .with_options(BindingOptions::default().with_debounce(Duration::from_millis(100)));
        engine.dispatcher.register_binding(binding).unwrap();

        press_key(&mut engine, Key::A, 10);
        assert_eq!(counter.load(Ordering::Relaxed), 1);
        release_key(&mut engine, Key::A, 10);

        press_key(&mut engine, Key::A, 10);
        assert_eq!(counter.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn rate_limit_caps_invocations_through_engine() {
        let mut engine = test_engine();
        let counter = Arc::new(AtomicUsize::new(0));
        let cc = Arc::clone(&counter);

        let binding = RegisteredBinding::new(
            BindingId::new(),
            Hotkey::new(Key::A),
            Action::from(move || {
                cc.fetch_add(1, Ordering::Relaxed);
            }),
        )
        .with_options(
            BindingOptions::default()
                .with_rate_limit(kbd::binding::RateLimit::new(2, Duration::from_secs(1))),
        );
        engine.dispatcher.register_binding(binding).unwrap();

        for _ in 0..2 {
            press_key(&mut engine, Key::A, 10);
            release_key(&mut engine, Key::A, 10);
        }
        assert_eq!(counter.load(Ordering::Relaxed), 2);

        press_key(&mut engine, Key::A, 10);
        assert_eq!(counter.load(Ordering::Relaxed), 2);
    }

    // Debounce and repeat interaction

    #[test]
    fn debounce_does_not_suppress_repeats() {
        let mut engine = test_engine();
        let counter = Arc::new(AtomicUsize::new(0));
        let cc = Arc::clone(&counter);

        let binding = RegisteredBinding::new(
            BindingId::new(),
            Hotkey::new(Key::A),
            Action::from(move || {
                cc.fetch_add(1, Ordering::Relaxed);
            }),
        )
        .with_options(
            BindingOptions::default()
                .with_debounce(Duration::from_millis(100))
                .with_repeat_policy(RepeatPolicy::Allow),
        );
        engine.dispatcher.register_binding(binding).unwrap();

        press_key(&mut engine, Key::A, 10);
        assert_eq!(counter.load(Ordering::Relaxed), 1);

        repeat_key(&mut engine, Key::A, 10);
        assert_eq!(counter.load(Ordering::Relaxed), 2);
    }
}
