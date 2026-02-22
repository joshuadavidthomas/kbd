//! The engine — owns all mutable state, runs the event loop.
//!
//! # Architecture
//!
//! The engine runs in a dedicated thread. It owns:
//! - All registered bindings
//! - The layer stack
//! - Key state (what's currently pressed)
//! - Sequence and tap-hold state machines
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
//!     check_timers()          // sequence timeouts, tap-hold thresholds
//! }
//! ```
//!
//! # Modules
//!
//! - [`key_state`] — tracks what's currently pressed, derives modifier state
//! - [`matcher`] — finds matching bindings for a key event
//! - [`sequence`] — sequence pattern state machine
//! - [`tap_hold`] — tap-hold pattern state machine
//! - [`devices`] — device discovery, hotplug, capability detection
//! - [`forwarder`] — uinput virtual device for event forwarding/emission
//!
//! # Reference
//!
//! Prior art: `archive/v0/src/listener.rs` (357-line `listener_loop`),
//! `archive/v0/src/listener/` (dispatch, io, sequence, hotplug, forwarding, state).
//! The engine replaces all of this.

use std::collections::HashMap;
use std::io;
use std::mem::size_of;
use std::os::fd::AsRawFd;
use std::os::fd::FromRawFd;
use std::os::fd::OwnedFd;
use std::os::fd::RawFd;
use std::sync::mpsc;
use std::sync::Arc;
use std::thread;

use crate::action::Action;
use crate::binding::BindingId;
use crate::binding::Passthrough;
use crate::engine::devices::DeviceKeyEvent;
use crate::key::Hotkey;
use crate::Error;
use crate::Key;

pub(crate) mod devices;
pub(crate) mod forwarder;
pub(crate) mod key_state;
pub(crate) mod matcher;
pub(crate) mod sequence;
pub(crate) mod tap_hold;

/// Whether the engine is running in grab mode.
///
/// In grab mode, the engine takes exclusive ownership of input devices
/// and forwards unmatched events through a virtual device. The forwarder
/// is bundled with the enabled state so it's impossible to be in grab
/// mode without a forwarder.
pub(crate) enum GrabState {
    Disabled,
    Enabled {
        forwarder: Box<dyn forwarder::ForwardSink>,
    },
}

/// Disposition of a key event after engine processing.
///
/// Returned by `process_key_event` to indicate what happened with the
/// event. Used by tests to verify forwarding and consumption behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum KeyEventDisposition {
    /// Event matched a binding and was consumed (not forwarded).
    MatchedConsumed,
    /// Event matched a binding with passthrough and was forwarded.
    MatchedForwarded,
    /// Event did not match any binding and was forwarded (grab mode).
    UnmatchedForwarded,
    /// Event was not processed (grab mode disabled, or modifier/repeat).
    Ignored,
}

pub(crate) struct RegisteredBinding {
    id: BindingId,
    hotkey: Hotkey,
    action: Action,
    passthrough: Passthrough,
}

impl RegisteredBinding {
    #[must_use]
    pub(crate) fn new(id: BindingId, hotkey: Hotkey, action: Action) -> Self {
        Self {
            id,
            hotkey,
            action,
            passthrough: Passthrough::default(),
        }
    }

    #[must_use]
    pub(crate) fn with_passthrough(mut self, passthrough: Passthrough) -> Self {
        self.passthrough = passthrough;
        self
    }

    #[must_use]
    pub(crate) const fn id(&self) -> BindingId {
        self.id
    }

    #[must_use]
    pub(crate) fn hotkey(&self) -> &Hotkey {
        &self.hotkey
    }

    #[must_use]
    pub(crate) const fn action(&self) -> &Action {
        &self.action
    }

    #[must_use]
    pub(crate) const fn passthrough(&self) -> Passthrough {
        self.passthrough
    }
}

pub(crate) enum Command {
    Register {
        binding: RegisteredBinding,
        reply: mpsc::Sender<Result<(), Error>>,
    },
    Unregister {
        id: BindingId,
    },
    IsRegistered {
        hotkey: Hotkey,
        reply: mpsc::Sender<bool>,
    },
    Shutdown,
}

#[derive(Clone)]
pub(crate) struct CommandSender {
    command_tx: mpsc::Sender<Command>,
    wake_fd: Arc<WakeFd>,
}

impl CommandSender {
    pub(crate) fn send(&self, command: Command) -> Result<(), Error> {
        self.command_tx
            .send(command)
            .map_err(|_| Error::ManagerStopped)?;
        self.wake_fd.wake().map_err(|_| Error::ManagerStopped)?;
        Ok(())
    }
}

pub(crate) struct EngineRuntime {
    commands: CommandSender,
    join_handle: thread::JoinHandle<Result<(), Error>>,
}

impl EngineRuntime {
    pub(crate) fn spawn(grab_state: GrabState) -> Result<Self, Error> {
        let wake_fd = Arc::new(WakeFd::new()?);
        let (command_tx, command_rx) = mpsc::channel();
        let commands = CommandSender {
            command_tx,
            wake_fd: Arc::clone(&wake_fd),
        };

        let engine = Engine::new(command_rx, wake_fd, grab_state);
        let join_handle = thread::spawn(move || run(engine));

        Ok(Self {
            commands,
            join_handle,
        })
    }

    #[must_use]
    pub(crate) fn commands(&self) -> CommandSender {
        self.commands.clone()
    }

    pub(crate) fn shutdown(self) -> Result<(), Error> {
        let send_result = self.commands.send(Command::Shutdown);
        let join_result = self.join();

        match (send_result, join_result) {
            (Ok(()), Ok(())) => Ok(()),
            (Err(error), Ok(())) | (_, Err(error)) => Err(error),
        }
    }

    pub(crate) fn join(self) -> Result<(), Error> {
        self.join_handle.join().map_err(|_| Error::EngineError)?
    }
}

pub(crate) struct Engine {
    bindings_by_id: HashMap<BindingId, RegisteredBinding>,
    binding_ids_by_hotkey: HashMap<Hotkey, BindingId>,
    devices: devices::DeviceManager,
    key_state: key_state::KeyState,
    grab_state: GrabState,
    command_rx: mpsc::Receiver<Command>,
    wake_fd: Arc<WakeFd>,
}

impl Engine {
    fn new(
        command_rx: mpsc::Receiver<Command>,
        wake_fd: Arc<WakeFd>,
        grab_state: GrabState,
    ) -> Self {
        let device_grab_mode = match &grab_state {
            GrabState::Disabled => devices::DeviceGrabMode::Shared,
            GrabState::Enabled { .. } => devices::DeviceGrabMode::Exclusive,
        };
        Self {
            bindings_by_id: HashMap::new(),
            binding_ids_by_hotkey: HashMap::new(),
            devices: devices::DeviceManager::default_with_grab(device_grab_mode),
            key_state: key_state::KeyState::default(),
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
        // SAFETY: `poll_fds` is a valid mutable buffer of `pollfd` values and
        // `poll_len` matches its length.
        let result = unsafe { libc::poll(poll_fds.as_mut_ptr(), poll_len, -1) };

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

    fn drain_commands(&mut self) -> LoopControl {
        loop {
            match self.command_rx.try_recv() {
                Ok(command) => {
                    if matches!(self.handle_command(command), LoopControl::Shutdown) {
                        return LoopControl::Shutdown;
                    }
                }
                Err(mpsc::TryRecvError::Empty) => return LoopControl::Continue,
                Err(mpsc::TryRecvError::Disconnected) => return LoopControl::Shutdown,
            }
        }
    }

    fn handle_command(&mut self, command: Command) -> LoopControl {
        match command {
            Command::Register { binding, reply } => {
                let register_result = self.register_binding(binding);
                let _ = reply.send(register_result);
                LoopControl::Continue
            }
            Command::Unregister { id } => {
                self.unregister_binding(id);
                LoopControl::Continue
            }
            Command::IsRegistered { hotkey, reply } => {
                let is_registered = self.binding_ids_by_hotkey.contains_key(&hotkey);
                let _ = reply.send(is_registered);
                LoopControl::Continue
            }
            Command::Shutdown => LoopControl::Shutdown,
        }
    }

    fn register_binding(&mut self, binding: RegisteredBinding) -> Result<(), Error> {
        let id = binding.id();
        let hotkey = binding.hotkey().clone();

        if self.bindings_by_id.contains_key(&id) || self.binding_ids_by_hotkey.contains_key(&hotkey)
        {
            return Err(Error::AlreadyRegistered);
        }

        self.binding_ids_by_hotkey.insert(hotkey, id);
        self.bindings_by_id.insert(id, binding);
        Ok(())
    }

    fn unregister_binding(&mut self, id: BindingId) {
        if let Some(binding) = self.bindings_by_id.remove(&id) {
            self.binding_ids_by_hotkey.remove(binding.hotkey());
        }
    }

    fn process_polled_events(&mut self, poll_fds: &[libc::pollfd]) {
        let events = self
            .devices
            .process_polled_events(&poll_fds[1..], &mut self.key_state);

        for event in events {
            let _ = self.process_key_event(event);
        }
    }

    fn process_key_event(&mut self, event: DeviceKeyEvent) -> KeyEventDisposition {
        self.key_state
            .apply_device_event(event.device_fd, event.key, event.transition);

        let active_modifiers = self.key_state.active_modifiers();
        let result = matcher::match_key_event(
            event.key,
            event.transition,
            &active_modifiers,
            &self.binding_ids_by_hotkey,
            &self.bindings_by_id,
        );

        match result {
            matcher::MatchResult::Matched {
                action,
                passthrough,
            } => {
                execute_action(action);
                match passthrough {
                    Passthrough::Enabled => {
                        self.forward_event(event.key, event.transition);
                        KeyEventDisposition::MatchedForwarded
                    }
                    Passthrough::Consume => KeyEventDisposition::MatchedConsumed,
                }
            }
            matcher::MatchResult::NoMatch | matcher::MatchResult::Ignored => {
                if matches!(self.grab_state, GrabState::Enabled { .. }) {
                    self.forward_event(event.key, event.transition);
                    KeyEventDisposition::UnmatchedForwarded
                } else {
                    KeyEventDisposition::Ignored
                }
            }
        }
    }

    fn forward_event(&mut self, key: Key, transition: key_state::KeyTransition) {
        if let GrabState::Enabled { forwarder } = &mut self.grab_state {
            if let Err(error) = forwarder.forward_key(key, transition) {
                tracing::error!(%error, "failed to forward key event through virtual device");
            }
        }
    }
}

/// Execute an action with panic isolation — a panicking callback never
/// kills the engine thread.
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
        Action::EmitKey(..)
        | Action::EmitSequence(..)
        | Action::PushLayer(..)
        | Action::PopLayer
        | Action::ToggleLayer(..)
        | Action::Swallow => {
            // These action types are handled in later phases.
        }
    }
}

pub(crate) fn run(mut engine: Engine) -> Result<(), Error> {
    loop {
        let poll_fds = engine.poll_sources()?;

        if matches!(engine.drain_commands(), LoopControl::Shutdown) {
            return Ok(());
        }

        engine.process_polled_events(&poll_fds);
    }
}

enum LoopControl {
    Continue,
    Shutdown,
}

struct WakeFd {
    fd: OwnedFd,
}

impl WakeFd {
    fn new() -> Result<Self, Error> {
        // SAFETY: Calling libc `eventfd` with constant flags.
        let raw_fd = unsafe { libc::eventfd(0, libc::EFD_CLOEXEC | libc::EFD_NONBLOCK) };
        if raw_fd < 0 {
            return Err(Error::EngineError);
        }

        // SAFETY: `raw_fd` is an owned descriptor returned by `eventfd`.
        let fd = unsafe { OwnedFd::from_raw_fd(raw_fd) };
        Ok(Self { fd })
    }

    fn wake(&self) -> io::Result<()> {
        let increment = 1_u64;

        loop {
            // SAFETY: `increment` points to an initialized `u64` with the exact
            // byte size required by eventfd writes.
            let result = unsafe {
                libc::write(
                    self.fd.as_raw_fd(),
                    (&raw const increment).cast::<libc::c_void>(),
                    size_of::<u64>(),
                )
            };

            if result == 8 {
                return Ok(());
            }

            if result < 0 {
                let error = io::Error::last_os_error();
                if error.kind() == io::ErrorKind::Interrupted {
                    continue;
                }
                if error.kind() == io::ErrorKind::WouldBlock {
                    return Ok(());
                }
                return Err(error);
            }

            return Err(io::Error::new(
                io::ErrorKind::WriteZero,
                "short write to wake eventfd",
            ));
        }
    }

    fn clear(&self) -> io::Result<()> {
        let mut value = 0_u64;

        loop {
            // SAFETY: `value` points to valid writable memory for a single
            // `u64`, which is the required eventfd read size.
            let result = unsafe {
                libc::read(
                    self.fd.as_raw_fd(),
                    (&raw mut value).cast::<libc::c_void>(),
                    size_of::<u64>(),
                )
            };

            if result == 8 {
                continue;
            }

            if result < 0 {
                let error = io::Error::last_os_error();
                if error.kind() == io::ErrorKind::Interrupted {
                    continue;
                }
                if error.kind() == io::ErrorKind::WouldBlock {
                    return Ok(());
                }
                return Err(error);
            }

            if result == 0 {
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "wake eventfd closed while clearing",
                ));
            }

            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "short read from wake eventfd",
            ));
        }
    }

    fn raw_fd(&self) -> RawFd {
        self.fd.as_raw_fd()
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::AtomicUsize;
    use std::sync::atomic::Ordering;
    use std::sync::mpsc;
    use std::sync::Arc;
    use std::time::Duration;

    use super::devices::DeviceKeyEvent;
    use super::key_state::KeyTransition;
    use super::Command;
    use super::Engine;
    use super::EngineRuntime;
    use super::GrabState;
    use super::KeyEventDisposition;
    use super::RegisteredBinding;
    use super::WakeFd;
    use crate::binding::BindingId;
    use crate::binding::Passthrough;
    use crate::key::Hotkey;
    use crate::Action;
    use crate::Error;
    use crate::Key;
    use crate::Modifier;

    #[test]
    fn engine_processes_register_and_unregister_commands() {
        let runtime = EngineRuntime::spawn(GrabState::Disabled).expect("engine should spawn");

        let id = BindingId::new();
        let binding = test_binding(id, Key::A, &[Modifier::Ctrl]);
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
                binding: test_binding(BindingId::new(), Key::B, &[Modifier::Alt]),
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
                binding: test_binding(BindingId::new(), Key::B, &[Modifier::Alt]),
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
        let hotkey = Hotkey::new(Key::C, vec![Modifier::Shift]);

        let (register_reply_tx, register_reply_rx) = mpsc::channel();
        runtime
            .commands()
            .send(Command::Register {
                binding: RegisteredBinding::new(BindingId::new(), hotkey.clone(), Action::Swallow),
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

    fn test_binding(id: BindingId, key: Key, modifiers: &[Modifier]) -> RegisteredBinding {
        let hotkey = Hotkey::new(key, modifiers.to_vec());
        RegisteredBinding::new(id, hotkey, Action::Swallow)
    }

    /// Create a minimal engine for unit testing (no devices, no grab, no event loop).
    fn test_engine() -> Engine {
        test_engine_with_grab(GrabState::Disabled)
    }

    /// Create a test engine with grab mode enabled (using a recording forwarder).
    fn test_engine_with_grab(grab_state: GrabState) -> Engine {
        let wake_fd = Arc::new(WakeFd::new().expect("wake fd should create"));
        let (_tx, rx) = mpsc::channel();
        Engine::new(rx, wake_fd, grab_state)
    }

    /// Create grab state with a recording forwarder for testing.
    /// Returns the GrabState and a handle to inspect forwarded events.
    fn test_grab_state() -> (GrabState, super::forwarder::testing::ForwardedEvents) {
        let (recorder, events) = super::forwarder::testing::RecordingForwarder::new();
        let state = GrabState::Enabled {
            forwarder: Box::new(recorder),
        };
        (state, events)
    }

    fn press_key(engine: &mut Engine, key: Key, device_fd: i32) -> KeyEventDisposition {
        engine.process_key_event(DeviceKeyEvent {
            device_fd,
            key,
            transition: KeyTransition::Press,
        })
    }

    fn release_key(engine: &mut Engine, key: Key, device_fd: i32) -> KeyEventDisposition {
        engine.process_key_event(DeviceKeyEvent {
            device_fd,
            key,
            transition: KeyTransition::Release,
        })
    }

    #[test]
    fn matching_hotkey_fires_callback() {
        let mut engine = test_engine();
        let counter = Arc::new(AtomicUsize::new(0));
        let counter_clone = Arc::clone(&counter);

        let id = BindingId::new();
        let hotkey = Hotkey::new(Key::C, vec![Modifier::Ctrl]);
        let action = Action::from(move || {
            counter_clone.fetch_add(1, Ordering::Relaxed);
        });
        let binding = RegisteredBinding::new(id, hotkey, action);
        engine.register_binding(binding).unwrap();

        // Simulate: press Ctrl, then press C
        press_key(&mut engine, Key::LeftCtrl, 10);
        press_key(&mut engine, Key::C, 10);

        assert_eq!(counter.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn unmatched_event_does_not_fire_any_callback() {
        let mut engine = test_engine();
        let counter = Arc::new(AtomicUsize::new(0));
        let counter_clone = Arc::clone(&counter);

        let id = BindingId::new();
        let hotkey = Hotkey::new(Key::C, vec![Modifier::Ctrl]);
        let action = Action::from(move || {
            counter_clone.fetch_add(1, Ordering::Relaxed);
        });
        let binding = RegisteredBinding::new(id, hotkey, action);
        engine.register_binding(binding).unwrap();

        // Press V instead of C (with Ctrl held)
        press_key(&mut engine, Key::LeftCtrl, 10);
        press_key(&mut engine, Key::V, 10);

        assert_eq!(counter.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn modifier_combination_must_match_exactly() {
        let mut engine = test_engine();
        let counter = Arc::new(AtomicUsize::new(0));
        let counter_clone = Arc::clone(&counter);

        let id = BindingId::new();
        let hotkey = Hotkey::new(Key::C, vec![Modifier::Ctrl]);
        let action = Action::from(move || {
            counter_clone.fetch_add(1, Ordering::Relaxed);
        });
        let binding = RegisteredBinding::new(id, hotkey, action);
        engine.register_binding(binding).unwrap();

        // Press Ctrl+Shift+C — binding only wants Ctrl+C
        press_key(&mut engine, Key::LeftCtrl, 10);
        press_key(&mut engine, Key::LeftShift, 10);
        press_key(&mut engine, Key::C, 10);

        assert_eq!(counter.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn multi_modifier_hotkey_fires_when_all_held() {
        let mut engine = test_engine();
        let counter = Arc::new(AtomicUsize::new(0));
        let counter_clone = Arc::clone(&counter);

        let id = BindingId::new();
        let hotkey = Hotkey::new(Key::A, vec![Modifier::Ctrl, Modifier::Shift]);
        let action = Action::from(move || {
            counter_clone.fetch_add(1, Ordering::Relaxed);
        });
        let binding = RegisteredBinding::new(id, hotkey, action);
        engine.register_binding(binding).unwrap();

        press_key(&mut engine, Key::LeftCtrl, 10);
        press_key(&mut engine, Key::LeftShift, 10);
        press_key(&mut engine, Key::A, 10);

        assert_eq!(counter.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn hotkey_without_modifiers_fires_on_bare_keypress() {
        let mut engine = test_engine();
        let counter = Arc::new(AtomicUsize::new(0));
        let counter_clone = Arc::clone(&counter);

        let id = BindingId::new();
        let hotkey = Hotkey::new(Key::Escape, vec![]);
        let action = Action::from(move || {
            counter_clone.fetch_add(1, Ordering::Relaxed);
        });
        let binding = RegisteredBinding::new(id, hotkey, action);
        engine.register_binding(binding).unwrap();

        press_key(&mut engine, Key::Escape, 10);

        assert_eq!(counter.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn release_does_not_fire_callback() {
        let mut engine = test_engine();
        let counter = Arc::new(AtomicUsize::new(0));
        let counter_clone = Arc::clone(&counter);

        let id = BindingId::new();
        let hotkey = Hotkey::new(Key::C, vec![Modifier::Ctrl]);
        let action = Action::from(move || {
            counter_clone.fetch_add(1, Ordering::Relaxed);
        });
        let binding = RegisteredBinding::new(id, hotkey, action);
        engine.register_binding(binding).unwrap();

        // Press the hotkey so it fires once
        press_key(&mut engine, Key::LeftCtrl, 10);
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
        let hotkey1 = Hotkey::new(Key::P, vec![Modifier::Ctrl]);
        let action1 = Action::from(move || {
            panic!("intentional test panic");
        });
        engine
            .register_binding(RegisteredBinding::new(id1, hotkey1, action1))
            .unwrap();

        // Register a second binding that increments a counter
        let id2 = BindingId::new();
        let hotkey2 = Hotkey::new(Key::Q, vec![Modifier::Ctrl]);
        let action2 = Action::from(move || {
            post_panic_clone.fetch_add(1, Ordering::Relaxed);
        });
        engine
            .register_binding(RegisteredBinding::new(id2, hotkey2, action2))
            .unwrap();

        // Trigger the panicking callback
        press_key(&mut engine, Key::LeftCtrl, 10);
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
        let hotkey = Hotkey::new(Key::C, vec![Modifier::Ctrl]);
        let action = Action::from(move || {
            counter_clone.fetch_add(1, Ordering::Relaxed);
        });
        let binding = RegisteredBinding::new(id, hotkey, action);
        engine.register_binding(binding).unwrap();

        // Use RightCtrl instead of LeftCtrl — should still match
        press_key(&mut engine, Key::RightCtrl, 10);
        press_key(&mut engine, Key::C, 10);

        assert_eq!(counter.load(Ordering::Relaxed), 1);
    }

    // Grab mode tests

    #[test]
    fn grab_mode_forwards_unmatched_key_events() {
        let (grab_state, forwarded) = test_grab_state();
        let mut engine = test_engine_with_grab(grab_state);

        let id = BindingId::new();
        let hotkey = Hotkey::new(Key::C, vec![Modifier::Ctrl]);
        engine
            .register_binding(RegisteredBinding::new(id, hotkey, Action::Swallow))
            .unwrap();

        // Press A with no modifiers — no binding matches, should be forwarded
        let disposition = press_key(&mut engine, Key::A, 10);
        assert_eq!(disposition, KeyEventDisposition::UnmatchedForwarded);

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
        let hotkey = Hotkey::new(Key::C, vec![Modifier::Ctrl]);
        let action = Action::from(move || {
            counter_clone.fetch_add(1, Ordering::Relaxed);
        });
        engine
            .register_binding(RegisteredBinding::new(id, hotkey, action))
            .unwrap();

        // Press Ctrl+C — matches binding, should be consumed
        press_key(&mut engine, Key::LeftCtrl, 10);
        let disposition = press_key(&mut engine, Key::C, 10);

        assert_eq!(disposition, KeyEventDisposition::MatchedConsumed);
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
        let hotkey = Hotkey::new(Key::C, vec![Modifier::Ctrl]);
        let action = Action::from(move || {
            counter_clone.fetch_add(1, Ordering::Relaxed);
        });
        let binding =
            RegisteredBinding::new(id, hotkey, action).with_passthrough(Passthrough::Enabled);
        engine.register_binding(binding).unwrap();

        // Press Ctrl+C with passthrough — should fire AND forward
        press_key(&mut engine, Key::LeftCtrl, 10);
        let disposition = press_key(&mut engine, Key::C, 10);

        assert_eq!(disposition, KeyEventDisposition::MatchedForwarded);
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
        assert_eq!(disposition, KeyEventDisposition::Ignored);
    }

    #[test]
    fn grab_mode_forwards_release_events() {
        let (grab_state, forwarded) = test_grab_state();
        let mut engine = test_engine_with_grab(grab_state);

        // Press and release A — both should be forwarded (no binding matches)
        let press_disposition = press_key(&mut engine, Key::A, 10);
        let release_disposition = release_key(&mut engine, Key::A, 10);

        assert_eq!(press_disposition, KeyEventDisposition::UnmatchedForwarded);
        assert_eq!(release_disposition, KeyEventDisposition::UnmatchedForwarded);

        let events = forwarded.lock().unwrap();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0], (Key::A, KeyTransition::Press));
        assert_eq!(events[1], (Key::A, KeyTransition::Release));
    }

    #[test]
    fn grab_mode_forwards_modifier_presses() {
        let (grab_state, forwarded) = test_grab_state();
        let mut engine = test_engine_with_grab(grab_state);

        // Pressing a modifier key with no bindings should be forwarded
        let disposition = press_key(&mut engine, Key::LeftCtrl, 10);
        assert_eq!(disposition, KeyEventDisposition::UnmatchedForwarded);

        let events = forwarded.lock().unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0], (Key::LeftCtrl, KeyTransition::Press));
    }
}
