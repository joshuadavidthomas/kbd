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
//! - [`types`] — shared engine types (grab state, dispositions, layer stack entries)
//! - [`command`] — command enum and sender for manager→engine communication
//! - [`runtime`] — engine thread lifecycle (spawn, shutdown, join)
//! - [`wake`] — eventfd-based wake mechanism and loop control
//!
//! # Reference
//!
//! Prior art: `archive/v0/src/listener.rs` (357-line `listener_loop`),
//! `archive/v0/src/listener/` (dispatch, io, sequence, hotplug, forwarding, state).
//! The engine replaces all of this.

use std::collections::HashMap;
use std::io;
use std::path::Path;
use std::sync::Arc;
use std::sync::mpsc;
use std::time::Instant;

use kbd_core::matcher::LayerStackEntry;
use kbd_core::matcher::LayerTimeout;
use kbd_core::matcher::MatchResult;
use kbd_core::matcher::match_key_event;

use self::key_state::KeyState;
use self::key_state::KeyTransition;
use crate::Error;
use crate::Key;
use crate::Modifier;
use crate::action::Action;
use crate::action::LayerName;
use crate::binding::BindingId;
use crate::binding::Passthrough;
use crate::binding::RegisteredBinding;
use crate::engine::devices::DeviceKeyEvent;
use crate::introspection::ActiveLayerInfo;
use crate::introspection::BindingInfo;
use crate::introspection::BindingLocation;
use crate::introspection::ConflictInfo;
use crate::introspection::ShadowedStatus;
use crate::key::Hotkey;
use crate::layer::Layer;
use crate::layer::StoredLayer;

pub(crate) mod command;
pub(crate) mod devices;
pub(crate) mod forwarder;
pub(crate) mod key_state;
pub(crate) mod runtime;
pub(crate) mod sequence;
pub(crate) mod tap_hold;
pub(crate) mod types;
mod wake;

pub(crate) use self::command::Command;
pub(crate) use self::command::CommandSender;
pub(crate) use self::runtime::EngineRuntime;
pub(crate) use self::types::GrabState;
pub(crate) use self::types::KeyEventDisposition;
use self::types::LayerEffect;
use self::types::MatchOutcome;
use self::wake::LoopControl;
use self::wake::WakeFd;

pub(crate) struct Engine {
    bindings_by_id: HashMap<BindingId, RegisteredBinding>,
    binding_ids_by_hotkey: HashMap<Hotkey, BindingId>,
    layers: HashMap<LayerName, StoredLayer>,
    layer_stack: Vec<LayerStackEntry>,
    /// Press cache: records what happened on key press so the corresponding
    /// release event uses the same disposition. Essential for correct
    /// release behavior across layer transitions — if a key was consumed
    /// on press, its release should also be consumed even if the layer
    /// was popped in between (oneshot, `PopLayer` action, etc.).
    ///
    /// Reference: keyd's `cache_entry` system in `reference/keyd/src/keyboard.c`.
    press_cache: HashMap<Key, KeyEventDisposition>,
    devices: devices::DeviceManager,
    key_state: KeyState,
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
            layers: HashMap::new(),
            layer_stack: Vec::new(),
            press_cache: HashMap::new(),
            devices: devices::DeviceManager::new(
                Path::new(devices::INPUT_DIRECTORY),
                device_grab_mode,
            ),
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
        let now = Instant::now();
        let mut min_remaining = None;

        for entry in &self.layer_stack {
            if let Some(timeout) = &entry.timeout {
                let elapsed = now.duration_since(timeout.last_activity);
                let remaining = timeout.duration.saturating_sub(elapsed);
                min_remaining = Some(match min_remaining {
                    Some(current) => std::cmp::min(current, remaining),
                    None => remaining,
                });
            }
        }

        match min_remaining {
            Some(remaining) => {
                let ms = remaining.as_millis();
                // Clamp to i32::MAX, add 1ms to ensure we wake after expiry
                i32::try_from(ms.saturating_add(1)).unwrap_or(i32::MAX)
            }
            None => -1,
        }
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
            Command::DefineLayer { layer, reply } => {
                let result = self.define_layer(layer);
                let _ = reply.send(result);
                LoopControl::Continue
            }
            Command::PushLayer { name, reply } => {
                let result = self.push_layer(name);
                let _ = reply.send(result);
                LoopControl::Continue
            }
            Command::PopLayer { reply } => {
                let result = self.pop_layer();
                let _ = reply.send(result);
                LoopControl::Continue
            }
            Command::ToggleLayer { name, reply } => {
                let result = self.toggle_layer(name);
                let _ = reply.send(result);
                LoopControl::Continue
            }
            Command::IsRegistered { hotkey, reply } => {
                let is_registered = self.binding_ids_by_hotkey.contains_key(&hotkey);
                let _ = reply.send(is_registered);
                LoopControl::Continue
            }
            Command::IsKeyPressed { key, reply } => {
                let _ = reply.send(self.key_state.is_pressed(key));
                LoopControl::Continue
            }
            Command::ActiveModifiers { reply } => {
                let _ = reply.send(self.key_state.active_modifiers());
                LoopControl::Continue
            }
            Command::ListBindings { reply } => {
                let _ = reply.send(self.list_bindings());
                LoopControl::Continue
            }
            Command::BindingsForKey { hotkey, reply } => {
                let _ = reply.send(self.binding_for_key(&hotkey));
                LoopControl::Continue
            }
            Command::ActiveLayers { reply } => {
                let _ = reply.send(self.active_layers());
                LoopControl::Continue
            }
            Command::Conflicts { reply } => {
                let _ = reply.send(self.conflicts());
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

    fn define_layer(&mut self, layer: Layer) -> Result<(), Error> {
        let (name, bindings, options) = layer.into_parts();

        match self.layers.entry(name) {
            std::collections::hash_map::Entry::Occupied(_) => Err(Error::LayerAlreadyDefined),
            std::collections::hash_map::Entry::Vacant(entry) => {
                entry.insert(StoredLayer { bindings, options });
                Ok(())
            }
        }
    }

    fn push_layer(&mut self, name: LayerName) -> Result<(), Error> {
        let stored = self.layers.get(&name).ok_or(Error::LayerNotDefined)?;
        let oneshot_remaining = stored.options.oneshot();
        let timeout = stored.options.timeout().map(|duration| LayerTimeout {
            duration,
            last_activity: Instant::now(),
        });
        self.layer_stack.push(LayerStackEntry {
            name,
            oneshot_remaining,
            timeout,
        });
        Ok(())
    }

    fn pop_layer(&mut self) -> Result<LayerName, Error> {
        self.layer_stack
            .pop()
            .map(|entry| entry.name)
            .ok_or(Error::EmptyLayerStack)
    }

    fn toggle_layer(&mut self, name: LayerName) -> Result<(), Error> {
        if !self.layers.contains_key(&name) {
            return Err(Error::LayerNotDefined);
        }
        // Remove the topmost (most recently pushed) occurrence, not the bottommost.
        if let Some(pos) = self
            .layer_stack
            .iter()
            .rposition(|entry| entry.name == name)
        {
            self.layer_stack.remove(pos);
        } else {
            self.push_layer(name)?;
        }
        Ok(())
    }

    fn list_bindings(&self) -> Vec<BindingInfo> {
        // Build a map of hotkey → claiming layer name for active layers.
        // Walk top-down so the topmost layer claiming a hotkey "wins".
        let mut claimed_by: HashMap<&Hotkey, &LayerName> = HashMap::new();
        for entry in self.layer_stack.iter().rev() {
            if let Some(stored) = self.layers.get(&entry.name) {
                for binding in &stored.bindings {
                    claimed_by.entry(&binding.hotkey).or_insert(&entry.name);
                }
            }
        }

        let mut results = Vec::new();

        // Global bindings
        for binding in self.bindings_by_id.values() {
            let shadowed = if let Some(&layer_name) = claimed_by.get(binding.hotkey()) {
                ShadowedStatus::ShadowedBy(layer_name.clone())
            } else {
                ShadowedStatus::Active
            };

            results.push(BindingInfo {
                hotkey: binding.hotkey().clone(),
                description: binding.options().description().map(Box::from),
                location: BindingLocation::Global,
                shadowed,
                overlay_visibility: binding.options().overlay_visibility(),
            });
        }

        // Layer bindings (all defined layers, active or not)
        for (layer_name, stored) in &self.layers {
            let is_active = self.layer_stack.iter().any(|e| &e.name == layer_name);

            for binding in &stored.bindings {
                let shadowed = if !is_active {
                    ShadowedStatus::Inactive
                } else if let Some(&claiming_layer) = claimed_by.get(&binding.hotkey) {
                    if claiming_layer == layer_name {
                        ShadowedStatus::Active
                    } else {
                        ShadowedStatus::ShadowedBy(claiming_layer.clone())
                    }
                } else {
                    ShadowedStatus::Active
                };

                results.push(BindingInfo {
                    hotkey: binding.hotkey.clone(),
                    description: None,
                    location: BindingLocation::Layer(layer_name.clone()),
                    shadowed,
                    overlay_visibility: crate::binding::OverlayVisibility::Visible,
                });
            }
        }

        results
    }

    fn binding_for_key(&self, hotkey: &Hotkey) -> Option<BindingInfo> {
        // Modifier-only keys never fire bindings in the real matcher,
        // so they can't match here either.
        if Modifier::from_key(hotkey.key()).is_some() {
            return None;
        }

        // Walk layer stack top-down, same as the matcher
        for entry in self.layer_stack.iter().rev() {
            if let Some(stored) = self.layers.get(&entry.name) {
                for binding in &stored.bindings {
                    if binding.hotkey == *hotkey {
                        return Some(BindingInfo {
                            hotkey: binding.hotkey.clone(),
                            description: None,
                            location: BindingLocation::Layer(entry.name.clone()),
                            shadowed: ShadowedStatus::Active,
                            overlay_visibility: crate::binding::OverlayVisibility::Visible,
                        });
                    }
                }

                // Swallow layers block all unmatched keys from reaching
                // lower layers and globals — matches the real matcher.
                if matches!(
                    stored.options.unmatched(),
                    crate::layer::UnmatchedKeyBehavior::Swallow
                ) {
                    return None;
                }
            }
        }

        // Fall through to global bindings
        if let Some(&id) = self.binding_ids_by_hotkey.get(hotkey)
            && let Some(binding) = self.bindings_by_id.get(&id)
        {
            return Some(BindingInfo {
                hotkey: binding.hotkey().clone(),
                description: binding.options().description().map(Box::from),
                location: BindingLocation::Global,
                shadowed: ShadowedStatus::Active,
                overlay_visibility: binding.options().overlay_visibility(),
            });
        }

        None
    }

    fn active_layers(&self) -> Vec<ActiveLayerInfo> {
        self.layer_stack
            .iter()
            .filter_map(|entry| {
                self.layers.get(&entry.name).map(|stored| ActiveLayerInfo {
                    name: entry.name.clone(),
                    description: stored.options.description().map(Box::from),
                    binding_count: stored.bindings.len(),
                })
            })
            .collect()
    }

    fn conflicts(&self) -> Vec<ConflictInfo> {
        let all_bindings = self.list_bindings();
        let mut conflicts = Vec::new();

        // Find all shadowed bindings and pair with their shadowing binding
        for shadowed in &all_bindings {
            if let ShadowedStatus::ShadowedBy(ref shadowing_layer) = shadowed.shadowed {
                // Find the binding that's doing the shadowing
                if let Some(shadowing) = all_bindings.iter().find(|b| {
                    b.hotkey == shadowed.hotkey
                        && b.location == BindingLocation::Layer(shadowing_layer.clone())
                        && matches!(b.shadowed, ShadowedStatus::Active)
                }) {
                    conflicts.push(ConflictInfo {
                        hotkey: shadowed.hotkey.clone(),
                        shadowed_binding: shadowed.clone(),
                        shadowing_binding: shadowing.clone(),
                    });
                }
            }
        }

        conflicts
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

        // Press cache: on release, use the cached disposition from the
        // corresponding press instead of re-matching. This ensures correct
        // release behavior across layer transitions — if a key was consumed
        // on press, its release is consumed even if the layer was popped.
        if matches!(event.transition, KeyTransition::Release)
            && let Some(cached) = self.press_cache.remove(&event.key)
        {
            match cached {
                KeyEventDisposition::MatchedForwarded | KeyEventDisposition::UnmatchedForwarded => {
                    self.forward_event(event.key, event.transition);
                }
                KeyEventDisposition::MatchedConsumed | KeyEventDisposition::Ignored => {}
            }
            return cached;
        }
        // No cache entry (modifier release, or key pressed before cache existed).
        // Fall through to normal processing.

        let active_modifiers = self.key_state.active_modifiers();
        let candidate = Hotkey::with_modifiers(event.key, active_modifiers);

        // Phase 1: Match against layer stack + global bindings.
        // The MatchResult borrows self.layers and self.bindings_by_id,
        // so we extract what we need and drop the borrow before Phase 2.
        let outcome = {
            let result = match_key_event(
                event.transition,
                &candidate,
                &self.layer_stack,
                &self.layers,
                &self.binding_ids_by_hotkey,
                &self.bindings_by_id,
            );

            match result {
                MatchResult::Matched {
                    action,
                    passthrough,
                } => {
                    // Execute non-mutating parts (callbacks) while borrow is held
                    execute_action(action);
                    // Extract layer effect for Phase 2
                    let layer_effect = LayerEffect::from(action);
                    MatchOutcome::Matched {
                        layer_effect,
                        passthrough,
                    }
                }
                MatchResult::Swallowed => MatchOutcome::Swallowed,
                MatchResult::NoMatch => MatchOutcome::NoMatch,
                MatchResult::Ignored => MatchOutcome::Ignored,
            }
        };
        // result dropped — self.layers borrow released

        let was_actionable = !matches!(outcome, MatchOutcome::Ignored);

        // Phase 2: Apply layer mutations and determine event disposition
        let disposition = match outcome {
            MatchOutcome::Matched {
                layer_effect,
                passthrough,
            } => {
                self.apply_layer_effect(layer_effect);
                match passthrough {
                    Passthrough::Enabled
                        if matches!(self.grab_state, GrabState::Enabled { .. }) =>
                    {
                        self.forward_event(event.key, event.transition);
                        KeyEventDisposition::MatchedForwarded
                    }
                    Passthrough::Enabled | Passthrough::Consume => {
                        KeyEventDisposition::MatchedConsumed
                    }
                }
            }
            MatchOutcome::Swallowed => KeyEventDisposition::MatchedConsumed,
            MatchOutcome::NoMatch | MatchOutcome::Ignored => {
                if matches!(self.grab_state, GrabState::Enabled { .. }) {
                    self.forward_event(event.key, event.transition);
                    KeyEventDisposition::UnmatchedForwarded
                } else {
                    KeyEventDisposition::Ignored
                }
            }
        };

        // Cache the disposition for non-modifier key presses so the
        // corresponding release uses the same disposition.
        if matches!(event.transition, KeyTransition::Press)
            && Modifier::from_key(event.key).is_none()
        {
            self.press_cache.insert(event.key, disposition);
        }

        // Phase 3: Tick oneshot counters and reset timeout clocks for non-modifier key presses
        if matches!(event.transition, KeyTransition::Press)
            && Modifier::from_key(event.key).is_none()
            && was_actionable
        {
            self.reset_layer_timeouts();
            self.tick_oneshot_layers();
        }

        disposition
    }

    /// Reset timeout clocks on all active timeout layers (activity occurred).
    fn reset_layer_timeouts(&mut self) {
        let now = Instant::now();
        for entry in &mut self.layer_stack {
            if let Some(timeout) = &mut entry.timeout {
                timeout.last_activity = now;
            }
        }
    }

    /// Check all active timeout layers and pop any that have expired.
    fn check_layer_timeouts(&mut self) {
        let now = Instant::now();
        self.layer_stack.retain(|entry| {
            if let Some(timeout) = &entry.timeout {
                now.duration_since(timeout.last_activity) < timeout.duration
            } else {
                true
            }
        });
    }

    /// Decrement oneshot counters on the layer stack and pop expired layers.
    fn tick_oneshot_layers(&mut self) {
        // Walk top-down, decrement the first oneshot layer found, pop if exhausted
        let mut pop_index = None;
        for (i, entry) in self.layer_stack.iter_mut().enumerate().rev() {
            if let Some(remaining) = &mut entry.oneshot_remaining {
                *remaining = remaining.saturating_sub(1);
                if *remaining == 0 {
                    pop_index = Some(i);
                }
                break;
            }
        }
        if let Some(index) = pop_index {
            self.layer_stack.remove(index);
        }
    }

    fn apply_layer_effect(&mut self, effect: LayerEffect) {
        match effect {
            LayerEffect::None => {}
            LayerEffect::Push(name) => {
                if let Err(error) = self.push_layer(name) {
                    tracing::error!(%error, "failed to push layer from action");
                }
            }
            LayerEffect::Pop => {
                if let Err(error) = self.pop_layer() {
                    tracing::error!(%error, "failed to pop layer from action");
                }
            }
            LayerEffect::Toggle(name) => {
                if let Err(error) = self.toggle_layer(name) {
                    tracing::error!(%error, "failed to toggle layer from action");
                }
            }
        }
    }

    fn forward_event(&mut self, key: Key, transition: KeyTransition) {
        if let GrabState::Enabled { forwarder } = &mut self.grab_state
            && let Err(error) = forwarder.forward_key(key, transition)
        {
            tracing::error!(%error, "failed to forward key event through virtual device");
        }
    }
}

/// Execute a callback action with panic isolation — a panicking callback
/// never kills the engine thread.
///
/// Only handles `Action::Callback`. Layer-control actions are handled by
/// `Engine::apply_layer_effect` which has access to engine state.
fn execute_action(action: &Action) {
    if let Action::Callback(callback) = action
        && let Err(panic) = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            callback();
        }))
    {
        tracing::error!(
            panic_info = format!("{panic:?}"),
            "user callback panicked — panic caught, engine continues"
        );
    }
}

pub(crate) fn run(mut engine: Engine) -> Result<(), Error> {
    loop {
        let poll_fds = engine.poll_sources()?;

        if matches!(engine.drain_commands(), LoopControl::Shutdown) {
            return Ok(());
        }

        engine.process_polled_events(&poll_fds);
        engine.check_layer_timeouts();
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::AtomicUsize;
    use std::sync::atomic::Ordering;
    use std::sync::mpsc;
    use std::time::Duration;

    use super::Command;
    use super::Engine;
    use super::EngineRuntime;
    use super::GrabState;
    use super::KeyEventDisposition;
    use super::devices::DeviceKeyEvent;
    use super::key_state::KeyTransition;
    use super::wake::WakeFd;
    use crate::Action;
    use crate::Error;
    use crate::Key;
    use crate::Modifier;
    use crate::binding::BindingId;
    use crate::binding::Passthrough;
    use crate::binding::RegisteredBinding;
    use crate::key::Hotkey;

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

    fn test_binding(id: BindingId, hotkey: Hotkey) -> RegisteredBinding {
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
    /// Returns the `GrabState` and a handle to inspect forwarded events.
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
        let hotkey = Hotkey::new(Key::C).modifier(Modifier::Ctrl);
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
        let hotkey = Hotkey::new(Key::C).modifier(Modifier::Ctrl);
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
        let hotkey = Hotkey::new(Key::C).modifier(Modifier::Ctrl);
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
        let hotkey = Hotkey::new(Key::A)
            .modifier(Modifier::Ctrl)
            .modifier(Modifier::Shift);
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
        let hotkey = Hotkey::new(Key::Escape);
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
        let hotkey = Hotkey::new(Key::C).modifier(Modifier::Ctrl);
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
        let hotkey1 = Hotkey::new(Key::P).modifier(Modifier::Ctrl);
        let action1 = Action::from(move || {
            panic!("intentional test panic");
        });
        engine
            .register_binding(RegisteredBinding::new(id1, hotkey1, action1))
            .unwrap();

        // Register a second binding that increments a counter
        let id2 = BindingId::new();
        let hotkey2 = Hotkey::new(Key::Q).modifier(Modifier::Ctrl);
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
        let hotkey = Hotkey::new(Key::C).modifier(Modifier::Ctrl);
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
        let hotkey = Hotkey::new(Key::C).modifier(Modifier::Ctrl);
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
        let hotkey = Hotkey::new(Key::C).modifier(Modifier::Ctrl);
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
        let hotkey = Hotkey::new(Key::C).modifier(Modifier::Ctrl);
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
            RegisteredBinding::new(id, hotkey, action).with_passthrough(Passthrough::Enabled);
        engine.register_binding(binding).unwrap();

        press_key(&mut engine, Key::LeftCtrl, 10);
        let disposition = press_key(&mut engine, Key::C, 10);

        // Without grab mode, passthrough is a no-op — event reaches apps
        // through the normal kernel path. The engine reports MatchedConsumed
        // because it matched and executed the action; no virtual-device
        // forwarding occurred.
        assert_eq!(disposition, KeyEventDisposition::MatchedConsumed);
        assert_eq!(counter.load(Ordering::Relaxed), 1);
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

        press_key(&mut engine, Key::LeftCtrl, 10);
        assert_eq!(engine.key_state.active_modifiers(), vec![Modifier::Ctrl]);

        press_key(&mut engine, Key::LeftShift, 10);
        assert_eq!(
            engine.key_state.active_modifiers(),
            vec![Modifier::Ctrl, Modifier::Shift]
        );

        release_key(&mut engine, Key::LeftCtrl, 10);
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
    fn modifier_state_cleaned_on_device_disconnect() {
        let mut engine = test_engine();

        press_key(&mut engine, Key::LeftCtrl, 10);
        press_key(&mut engine, Key::LeftShift, 11);

        assert_eq!(
            engine.key_state.active_modifiers(),
            vec![Modifier::Ctrl, Modifier::Shift]
        );

        // Simulate device 10 disconnecting
        engine.key_state.disconnect_device(10);

        // Only modifiers from device 11 should remain
        assert_eq!(engine.key_state.active_modifiers(), vec![Modifier::Shift]);
        assert!(!engine.key_state.is_pressed(Key::LeftCtrl));
    }

    // Layer storage tests

    #[test]
    fn engine_stores_defined_layer() {
        let mut engine = test_engine();
        let layer = crate::Layer::new("nav")
            .bind(Key::H, Action::Swallow)
            .bind(Key::J, Action::Swallow);

        let result = engine.define_layer(layer);
        assert!(result.is_ok());
        assert!(
            engine
                .layers
                .contains_key(&crate::action::LayerName::from("nav"))
        );
    }

    #[test]
    fn engine_stores_layer_bindings() {
        let mut engine = test_engine();
        let layer = crate::Layer::new("nav")
            .bind(Key::H, Action::Swallow)
            .bind(Key::J, Action::Swallow)
            .bind(Key::K, Action::Swallow);

        engine.define_layer(layer).unwrap();

        let stored = engine
            .layers
            .get(&crate::action::LayerName::from("nav"))
            .expect("layer should be stored");
        assert_eq!(stored.bindings.len(), 3);
    }

    #[test]
    fn engine_stores_layer_options() {
        let mut engine = test_engine();
        let layer = crate::Layer::new("oneshot-nav")
            .bind(Key::H, Action::Swallow)
            .swallow()
            .oneshot(1)
            .timeout(std::time::Duration::from_secs(5));

        engine.define_layer(layer).unwrap();

        let stored = engine
            .layers
            .get(&crate::action::LayerName::from("oneshot-nav"))
            .expect("layer should be stored");
        assert_eq!(stored.options.oneshot(), Some(1));
        assert_eq!(
            stored.options.unmatched(),
            crate::layer::UnmatchedKeyBehavior::Swallow
        );
        assert_eq!(
            stored.options.timeout(),
            Some(std::time::Duration::from_secs(5))
        );
    }

    #[test]
    fn engine_stores_empty_layer() {
        let mut engine = test_engine();
        let layer = crate::Layer::new("empty");

        engine.define_layer(layer).unwrap();

        let stored = engine
            .layers
            .get(&crate::action::LayerName::from("empty"))
            .expect("layer should be stored");
        assert_eq!(stored.bindings.len(), 0);
    }

    #[test]
    fn define_layer_via_runtime_command() {
        let runtime = EngineRuntime::spawn(GrabState::Disabled).expect("engine should spawn");

        let layer = crate::Layer::new("nav").bind(Key::H, Action::Swallow);
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
        let first_layer = crate::Layer::new("nav").bind(Key::H, Action::Swallow);
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
        let duplicate_layer = crate::Layer::new("nav").bind(Key::J, Action::Swallow);
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
        let mut layer = crate::Layer::new(name);
        for (key, action) in bindings {
            layer = layer.bind(key, action);
        }
        engine.define_layer(layer).unwrap();
        engine
            .push_layer(crate::action::LayerName::from(name))
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
        let popped = engine.pop_layer().unwrap();
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

        let layer = crate::Layer::new("nav").bind(
            Key::H,
            Action::from(move || {
                cc.fetch_add(1, Ordering::Relaxed);
            }),
        );
        engine.define_layer(layer).unwrap();

        engine
            .toggle_layer(crate::action::LayerName::from("nav"))
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
            .toggle_layer(crate::action::LayerName::from("nav"))
            .unwrap();

        // H should no longer match
        press_key(&mut engine, Key::H, 10);
        assert_eq!(counter.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn toggle_undefined_layer_returns_error() {
        let mut engine = test_engine();
        let result = engine.toggle_layer(crate::action::LayerName::from("nonexistent"));
        assert!(matches!(result, Err(Error::LayerNotDefined)));
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
            .register_binding(RegisteredBinding::new(
                BindingId::new(),
                Hotkey::new(Key::X),
                Action::from(move || {
                    gc.fetch_add(1, Ordering::Relaxed);
                }),
            ))
            .unwrap();

        define_and_push_layer(&mut engine, "nav", vec![(Key::H, Action::Swallow)]);

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
            .register_binding(RegisteredBinding::new(
                BindingId::new(),
                Hotkey::new(Key::X),
                Action::from(move || {
                    gc.fetch_add(1, Ordering::Relaxed);
                }),
            ))
            .unwrap();

        let layer = crate::Layer::new("modal")
            .bind(Key::H, Action::Swallow)
            .swallow();
        engine.define_layer(layer).unwrap();
        engine
            .push_layer(crate::action::LayerName::from("modal"))
            .unwrap();

        // X not in swallow layer — consumed, global should NOT fire
        let disposition = press_key(&mut engine, Key::X, 10);
        assert_eq!(disposition, KeyEventDisposition::MatchedConsumed);
        assert_eq!(global_counter.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn action_push_layer_activates_layer() {
        let mut engine = test_engine();

        let layer_counter = Arc::new(AtomicUsize::new(0));
        let lc = Arc::clone(&layer_counter);
        let layer = crate::Layer::new("nav").bind(
            Key::H,
            Action::from(move || {
                lc.fetch_add(1, Ordering::Relaxed);
            }),
        );
        engine.define_layer(layer).unwrap();

        // Register a global binding that pushes the layer
        engine
            .register_binding(RegisteredBinding::new(
                BindingId::new(),
                Hotkey::new(Key::F1),
                Action::PushLayer(crate::action::LayerName::from("nav")),
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
        let layer = crate::Layer::new("nav")
            .bind(
                Key::H,
                Action::from(move || {
                    cc.fetch_add(1, Ordering::Relaxed);
                }),
            )
            .bind(Key::Escape, Action::PopLayer);
        engine.define_layer(layer).unwrap();
        engine
            .push_layer(crate::action::LayerName::from("nav"))
            .unwrap();

        // H fires in nav layer
        press_key(&mut engine, Key::H, 10);
        release_key(&mut engine, Key::H, 10);
        assert_eq!(counter.load(Ordering::Relaxed), 1);

        // Escape pops the layer
        press_key(&mut engine, Key::Escape, 10);
        release_key(&mut engine, Key::Escape, 10);

        // H should no longer match
        press_key(&mut engine, Key::H, 10);
        assert_eq!(counter.load(Ordering::Relaxed), 1); // unchanged
    }

    #[test]
    fn action_toggle_layer_toggles() {
        let mut engine = test_engine();

        let counter = Arc::new(AtomicUsize::new(0));
        let cc = Arc::clone(&counter);
        let layer = crate::Layer::new("nav").bind(
            Key::H,
            Action::from(move || {
                cc.fetch_add(1, Ordering::Relaxed);
            }),
        );
        engine.define_layer(layer).unwrap();

        // Register toggle binding
        engine
            .register_binding(RegisteredBinding::new(
                BindingId::new(),
                Hotkey::new(Key::F2),
                Action::ToggleLayer(crate::action::LayerName::from("nav")),
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
        engine.pop_layer().unwrap();
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

        let layer = crate::Layer::new("oneshot")
            .bind(
                Key::H,
                Action::from(move || {
                    cc.fetch_add(1, Ordering::Relaxed);
                }),
            )
            .oneshot(1);
        engine.define_layer(layer).unwrap();
        engine
            .push_layer(crate::action::LayerName::from("oneshot"))
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

        let layer = crate::Layer::new("oneshot")
            .bind(
                Key::H,
                Action::from(move || {
                    cc.fetch_add(1, Ordering::Relaxed);
                }),
            )
            .oneshot(1);
        engine.define_layer(layer).unwrap();
        engine
            .push_layer(crate::action::LayerName::from("oneshot"))
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

        let layer = crate::Layer::new("oneshot2")
            .bind(
                Key::H,
                Action::from(move || {
                    cc.fetch_add(1, Ordering::Relaxed);
                }),
            )
            .oneshot(2);
        engine.define_layer(layer).unwrap();
        engine
            .push_layer(crate::action::LayerName::from("oneshot2"))
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
        let layer = crate::Layer::new("nav").bind(Key::H, Action::Swallow);
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
                name: crate::action::LayerName::from("nav"),
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
                name: crate::action::LayerName::from("nonexistent"),
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
        let layer = crate::Layer::new("nav").bind(Key::H, Action::Swallow);
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
                name: crate::action::LayerName::from("nav"),
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

        let layer = crate::Layer::new("timed")
            .bind(
                Key::H,
                Action::from(move || {
                    cc.fetch_add(1, Ordering::Relaxed);
                }),
            )
            .timeout(Duration::from_millis(50));
        engine.define_layer(layer).unwrap();
        engine
            .push_layer(crate::action::LayerName::from("timed"))
            .unwrap();

        // H fires while layer is active
        press_key(&mut engine, Key::H, 10);
        release_key(&mut engine, Key::H, 10);
        assert_eq!(counter.load(Ordering::Relaxed), 1);

        // Wait for timeout to expire
        std::thread::sleep(Duration::from_millis(80));

        // Check timeouts (simulating the engine loop check)
        engine.check_layer_timeouts();

        // Layer should be gone — H no longer matches
        press_key(&mut engine, Key::H, 10);
        assert_eq!(counter.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn timeout_layer_resets_on_activity() {
        let mut engine = test_engine();
        let counter = Arc::new(AtomicUsize::new(0));
        let cc = Arc::clone(&counter);

        let layer = crate::Layer::new("timed")
            .bind(
                Key::H,
                Action::from(move || {
                    cc.fetch_add(1, Ordering::Relaxed);
                }),
            )
            .timeout(Duration::from_millis(100));
        engine.define_layer(layer).unwrap();
        engine
            .push_layer(crate::action::LayerName::from("timed"))
            .unwrap();

        // Activity within the timeout window
        std::thread::sleep(Duration::from_millis(50));
        press_key(&mut engine, Key::H, 10);
        release_key(&mut engine, Key::H, 10);
        assert_eq!(counter.load(Ordering::Relaxed), 1);

        // Wait a bit more but not enough from last activity
        std::thread::sleep(Duration::from_millis(50));
        engine.check_layer_timeouts();

        // Layer should still be active
        press_key(&mut engine, Key::H, 10);
        release_key(&mut engine, Key::H, 10);
        assert_eq!(counter.load(Ordering::Relaxed), 2);

        // Now wait for full timeout from last activity
        std::thread::sleep(Duration::from_millis(120));
        engine.check_layer_timeouts();

        // Layer should be gone
        press_key(&mut engine, Key::H, 10);
        assert_eq!(counter.load(Ordering::Relaxed), 2);
    }

    #[test]
    fn toggle_layer_via_runtime_command() {
        let runtime = EngineRuntime::spawn(GrabState::Disabled).expect("engine should spawn");

        // Define layer
        let layer = crate::Layer::new("nav").bind(Key::H, Action::Swallow);
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
                name: crate::action::LayerName::from("nav"),
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
        // In grab mode, if a press matched a binding and was consumed,
        // the release should also be consumed (not forwarded).
        let (grab_state, forwarded) = test_grab_state();
        let mut engine = test_engine_with_grab(grab_state);

        engine
            .register_binding(RegisteredBinding::new(
                BindingId::new(),
                Hotkey::new(Key::C).modifier(Modifier::Ctrl),
                Action::Swallow,
            ))
            .unwrap();

        // Press Ctrl, then press C (matched, consumed)
        press_key(&mut engine, Key::LeftCtrl, 10);
        press_key(&mut engine, Key::C, 10);

        // Release C — should be consumed, NOT forwarded
        let disposition = release_key(&mut engine, Key::C, 10);
        assert_eq!(disposition, KeyEventDisposition::MatchedConsumed);

        // Verify no C events were forwarded
        let events = forwarded.lock().unwrap();
        let c_events: Vec<_> = events.iter().filter(|(key, _)| *key == Key::C).collect();
        assert!(
            c_events.is_empty(),
            "consumed key's release should not be forwarded"
        );
    }

    #[test]
    fn press_cache_release_forwarded_when_press_had_passthrough() {
        // In grab mode, if a press matched with passthrough, the release
        // should also be forwarded (with MatchedForwarded disposition).
        let (grab_state, forwarded) = test_grab_state();
        let mut engine = test_engine_with_grab(grab_state);

        engine
            .register_binding(
                RegisteredBinding::new(
                    BindingId::new(),
                    Hotkey::new(Key::C).modifier(Modifier::Ctrl),
                    Action::Swallow,
                )
                .with_passthrough(Passthrough::Enabled),
            )
            .unwrap();

        // Press Ctrl+C with passthrough — matched, forwarded
        press_key(&mut engine, Key::LeftCtrl, 10);
        let press_disp = press_key(&mut engine, Key::C, 10);
        assert_eq!(press_disp, KeyEventDisposition::MatchedForwarded);

        // Release C — should also be forwarded as MatchedForwarded
        let release_disp = release_key(&mut engine, Key::C, 10);
        assert_eq!(release_disp, KeyEventDisposition::MatchedForwarded);

        // Verify C was forwarded on both press and release
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
        // In grab mode, if a press was swallowed by a layer, the release
        // should also be consumed (not forwarded).
        let (grab_state, forwarded) = test_grab_state();
        let mut engine = test_engine_with_grab(grab_state);

        let layer = crate::Layer::new("modal")
            .bind(Key::H, Action::Swallow)
            .swallow();
        engine.define_layer(layer).unwrap();
        engine
            .push_layer(crate::action::LayerName::from("modal"))
            .unwrap();

        // Press X — unmatched but swallowed by the modal layer
        let press_disp = press_key(&mut engine, Key::X, 10);
        assert_eq!(press_disp, KeyEventDisposition::MatchedConsumed);

        // Release X — should also be consumed
        let release_disp = release_key(&mut engine, Key::X, 10);
        assert_eq!(release_disp, KeyEventDisposition::MatchedConsumed);

        // Verify X was NOT forwarded
        let events = forwarded.lock().unwrap();
        let x_events: Vec<_> = events.iter().filter(|(key, _)| *key == Key::X).collect();
        assert!(
            x_events.is_empty(),
            "swallowed key should not be forwarded on release"
        );
    }

    #[test]
    fn press_cache_cleared_after_release() {
        // After releasing a cached key, the second press+release cycle
        // should match normally, not use stale cache data.
        let (grab_state, _forwarded) = test_grab_state();
        let mut engine = test_engine_with_grab(grab_state);

        let counter = Arc::new(AtomicUsize::new(0));
        let cc = Arc::clone(&counter);

        engine
            .register_binding(RegisteredBinding::new(
                BindingId::new(),
                Hotkey::new(Key::C).modifier(Modifier::Ctrl),
                Action::from(move || {
                    cc.fetch_add(1, Ordering::Relaxed);
                }),
            ))
            .unwrap();

        // First press+release cycle
        press_key(&mut engine, Key::LeftCtrl, 10);
        press_key(&mut engine, Key::C, 10);
        let release_disp = release_key(&mut engine, Key::C, 10);
        assert_eq!(release_disp, KeyEventDisposition::MatchedConsumed);

        // Second press should match normally (cache was cleared on release)
        let second_press_disp = press_key(&mut engine, Key::C, 10);
        assert_eq!(second_press_disp, KeyEventDisposition::MatchedConsumed);
        assert_eq!(counter.load(Ordering::Relaxed), 2);
    }

    #[test]
    fn press_cache_layer_pop_during_press_release_correct() {
        // Press H in a layer that has H → PopLayer. The press matches,
        // pops the layer, and is consumed. The release should still be
        // consumed via press cache even though the layer is now gone.
        let (grab_state, forwarded) = test_grab_state();
        let mut engine = test_engine_with_grab(grab_state);

        let layer = crate::Layer::new("nav")
            .bind(Key::H, Action::PopLayer)
            .bind(Key::J, Action::Swallow);
        engine.define_layer(layer).unwrap();
        engine
            .push_layer(crate::action::LayerName::from("nav"))
            .unwrap();

        // Press H — matches PopLayer, layer is popped, event consumed
        let press_disp = press_key(&mut engine, Key::H, 10);
        assert_eq!(press_disp, KeyEventDisposition::MatchedConsumed);
        assert!(
            engine.layer_stack.is_empty(),
            "layer should have been popped"
        );

        // Release H — should be consumed via cache
        let release_disp = release_key(&mut engine, Key::H, 10);
        assert_eq!(release_disp, KeyEventDisposition::MatchedConsumed);

        // Verify H was not forwarded
        let events = forwarded.lock().unwrap();
        let h_events: Vec<_> = events.iter().filter(|(key, _)| *key == Key::H).collect();
        assert!(
            h_events.is_empty(),
            "press cache should prevent release forwarding after layer pop"
        );
    }

    // Introspection tests (Section 3.5)

    #[test]
    fn list_bindings_returns_global_binding() {
        let mut engine = test_engine();

        engine
            .register_binding(
                RegisteredBinding::new(
                    BindingId::new(),
                    Hotkey::new(Key::C).modifier(Modifier::Ctrl),
                    Action::Swallow,
                )
                .with_options(crate::binding::BindingOptions::default().with_description("Copy")),
            )
            .unwrap();

        let bindings = engine.list_bindings();
        assert_eq!(bindings.len(), 1);

        let info = &bindings[0];
        assert_eq!(info.hotkey, Hotkey::new(Key::C).modifier(Modifier::Ctrl));
        assert_eq!(info.description.as_deref(), Some("Copy"));
        assert_eq!(info.location, crate::introspection::BindingLocation::Global);
        assert_eq!(info.shadowed, crate::introspection::ShadowedStatus::Active);
    }

    #[test]
    fn list_bindings_includes_layer_bindings() {
        let mut engine = test_engine();

        let layer = crate::Layer::new("nav")
            .bind(Key::H, Action::Swallow)
            .bind(Key::J, Action::Swallow);
        engine.define_layer(layer).unwrap();

        let bindings = engine.list_bindings();
        let layer_bindings: Vec<_> = bindings
            .iter()
            .filter(|b| matches!(b.location, crate::introspection::BindingLocation::Layer(_)))
            .collect();
        assert_eq!(layer_bindings.len(), 2);
    }

    #[test]
    fn list_bindings_inactive_layer_binding_marked_inactive() {
        let mut engine = test_engine();

        let layer = crate::Layer::new("nav").bind(Key::H, Action::Swallow);
        engine.define_layer(layer).unwrap();
        // Don't push the layer

        let bindings = engine.list_bindings();
        let nav_binding = bindings
            .iter()
            .find(|b| b.hotkey == Hotkey::new(Key::H))
            .expect("should find H binding");
        assert_eq!(
            nav_binding.shadowed,
            crate::introspection::ShadowedStatus::Inactive
        );
    }

    #[test]
    fn list_bindings_detects_shadowed_global_binding() {
        let mut engine = test_engine();

        // Global binding for H
        engine
            .register_binding(RegisteredBinding::new(
                BindingId::new(),
                Hotkey::new(Key::H),
                Action::Swallow,
            ))
            .unwrap();

        // Layer binding for H
        let layer = crate::Layer::new("nav").bind(Key::H, Action::Swallow);
        engine.define_layer(layer).unwrap();
        engine
            .push_layer(crate::action::LayerName::from("nav"))
            .unwrap();

        let bindings = engine.list_bindings();

        // Global H should be shadowed by nav layer
        let global_h = bindings
            .iter()
            .find(|b| {
                b.hotkey == Hotkey::new(Key::H)
                    && matches!(b.location, crate::introspection::BindingLocation::Global)
            })
            .expect("should find global H");
        assert_eq!(
            global_h.shadowed,
            crate::introspection::ShadowedStatus::ShadowedBy(crate::action::LayerName::from("nav"))
        );

        // Layer H should be active
        let layer_h = bindings
            .iter()
            .find(|b| {
                b.hotkey == Hotkey::new(Key::H)
                    && matches!(b.location, crate::introspection::BindingLocation::Layer(_))
            })
            .expect("should find layer H");
        assert_eq!(
            layer_h.shadowed,
            crate::introspection::ShadowedStatus::Active
        );
    }

    #[test]
    fn list_bindings_higher_layer_shadows_lower_layer() {
        let mut engine = test_engine();

        let layer1 = crate::Layer::new("layer1").bind(Key::H, Action::Swallow);
        engine.define_layer(layer1).unwrap();

        let layer2 = crate::Layer::new("layer2").bind(Key::H, Action::Swallow);
        engine.define_layer(layer2).unwrap();

        engine
            .push_layer(crate::action::LayerName::from("layer1"))
            .unwrap();
        engine
            .push_layer(crate::action::LayerName::from("layer2"))
            .unwrap();

        let bindings = engine.list_bindings();

        let layer1_h = bindings
            .iter()
            .find(|b| {
                b.hotkey == Hotkey::new(Key::H)
                    && b.location
                        == crate::introspection::BindingLocation::Layer(
                            crate::action::LayerName::from("layer1"),
                        )
            })
            .expect("should find layer1 H");
        assert_eq!(
            layer1_h.shadowed,
            crate::introspection::ShadowedStatus::ShadowedBy(crate::action::LayerName::from(
                "layer2"
            ))
        );

        let layer2_h = bindings
            .iter()
            .find(|b| {
                b.hotkey == Hotkey::new(Key::H)
                    && b.location
                        == crate::introspection::BindingLocation::Layer(
                            crate::action::LayerName::from("layer2"),
                        )
            })
            .expect("should find layer2 H");
        assert_eq!(
            layer2_h.shadowed,
            crate::introspection::ShadowedStatus::Active
        );
    }

    #[test]
    fn binding_for_key_returns_matching_global_binding() {
        let mut engine = test_engine();

        engine
            .register_binding(
                RegisteredBinding::new(
                    BindingId::new(),
                    Hotkey::new(Key::C).modifier(Modifier::Ctrl),
                    Action::Swallow,
                )
                .with_options(crate::binding::BindingOptions::default().with_description("Copy")),
            )
            .unwrap();

        let result = engine.binding_for_key(&Hotkey::new(Key::C).modifier(Modifier::Ctrl));
        assert!(result.is_some());

        let info = result.unwrap();
        assert_eq!(info.hotkey, Hotkey::new(Key::C).modifier(Modifier::Ctrl));
        assert_eq!(info.description.as_deref(), Some("Copy"));
        assert_eq!(info.location, crate::introspection::BindingLocation::Global);
    }

    #[test]
    fn binding_for_key_returns_none_when_no_match() {
        let mut engine = test_engine();

        engine
            .register_binding(RegisteredBinding::new(
                BindingId::new(),
                Hotkey::new(Key::C).modifier(Modifier::Ctrl),
                Action::Swallow,
            ))
            .unwrap();

        let result = engine.binding_for_key(&Hotkey::new(Key::V).modifier(Modifier::Ctrl));
        assert!(result.is_none());
    }

    #[test]
    fn binding_for_key_respects_layer_stack() {
        let mut engine = test_engine();

        // Global binding for H
        engine
            .register_binding(
                RegisteredBinding::new(BindingId::new(), Hotkey::new(Key::H), Action::Swallow)
                    .with_options(
                        crate::binding::BindingOptions::default().with_description("Global H"),
                    ),
            )
            .unwrap();

        // Layer binding for H
        let layer = crate::Layer::new("nav").bind(Key::H, Action::Swallow);
        engine.define_layer(layer).unwrap();
        engine
            .push_layer(crate::action::LayerName::from("nav"))
            .unwrap();

        let result = engine.binding_for_key(&Hotkey::new(Key::H));
        assert!(result.is_some());

        let info = result.unwrap();
        assert_eq!(
            info.location,
            crate::introspection::BindingLocation::Layer(crate::action::LayerName::from("nav"))
        );
    }

    #[test]
    fn active_layers_returns_empty_when_no_layers_pushed() {
        let engine = test_engine();
        let layers = engine.active_layers();
        assert!(layers.is_empty());
    }

    #[test]
    fn active_layers_returns_stack_in_order() {
        let mut engine = test_engine();

        let layer1 = crate::Layer::new("layer1")
            .bind(Key::H, Action::Swallow)
            .description("First layer");
        engine.define_layer(layer1).unwrap();

        let layer2 = crate::Layer::new("layer2")
            .bind(Key::J, Action::Swallow)
            .bind(Key::K, Action::Swallow)
            .description("Second layer");
        engine.define_layer(layer2).unwrap();

        engine
            .push_layer(crate::action::LayerName::from("layer1"))
            .unwrap();
        engine
            .push_layer(crate::action::LayerName::from("layer2"))
            .unwrap();

        let active = engine.active_layers();
        assert_eq!(active.len(), 2);

        // Bottom to top order
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
            .register_binding(RegisteredBinding::new(
                BindingId::new(),
                Hotkey::new(Key::C).modifier(Modifier::Ctrl),
                Action::Swallow,
            ))
            .unwrap();

        let conflicts = engine.conflicts();
        assert!(conflicts.is_empty());
    }

    #[test]
    fn conflicts_detects_layer_shadowing_global() {
        let mut engine = test_engine();

        // Global binding for H
        engine
            .register_binding(RegisteredBinding::new(
                BindingId::new(),
                Hotkey::new(Key::H),
                Action::Swallow,
            ))
            .unwrap();

        // Layer binding for H
        let layer = crate::Layer::new("nav").bind(Key::H, Action::Swallow);
        engine.define_layer(layer).unwrap();
        engine
            .push_layer(crate::action::LayerName::from("nav"))
            .unwrap();

        let conflicts = engine.conflicts();
        assert_eq!(conflicts.len(), 1);

        let conflict = &conflicts[0];
        assert_eq!(conflict.hotkey, Hotkey::new(Key::H));
        assert_eq!(
            conflict.shadowed_binding.location,
            crate::introspection::BindingLocation::Global
        );
        assert_eq!(
            conflict.shadowing_binding.location,
            crate::introspection::BindingLocation::Layer(crate::action::LayerName::from("nav"))
        );
    }

    #[test]
    fn conflicts_detects_layer_shadowing_lower_layer() {
        let mut engine = test_engine();

        let layer1 = crate::Layer::new("layer1").bind(Key::H, Action::Swallow);
        engine.define_layer(layer1).unwrap();

        let layer2 = crate::Layer::new("layer2").bind(Key::H, Action::Swallow);
        engine.define_layer(layer2).unwrap();

        engine
            .push_layer(crate::action::LayerName::from("layer1"))
            .unwrap();
        engine
            .push_layer(crate::action::LayerName::from("layer2"))
            .unwrap();

        let conflicts = engine.conflicts();
        assert_eq!(conflicts.len(), 1);

        let conflict = &conflicts[0];
        assert_eq!(conflict.hotkey, Hotkey::new(Key::H));
        assert_eq!(
            conflict.shadowed_binding.location,
            crate::introspection::BindingLocation::Layer(crate::action::LayerName::from("layer1"))
        );
        assert_eq!(
            conflict.shadowing_binding.location,
            crate::introspection::BindingLocation::Layer(crate::action::LayerName::from("layer2"))
        );
    }

    #[test]
    fn introspection_via_runtime_commands() {
        let runtime = EngineRuntime::spawn(GrabState::Disabled).expect("engine should spawn");

        // Register a global binding
        let (reply_tx, reply_rx) = mpsc::channel();
        runtime
            .commands()
            .send(Command::Register {
                binding: RegisteredBinding::new(
                    BindingId::new(),
                    Hotkey::new(Key::C).modifier(Modifier::Ctrl),
                    Action::Swallow,
                )
                .with_options(crate::binding::BindingOptions::default().with_description("Copy")),
                reply: reply_tx,
            })
            .unwrap();
        reply_rx
            .recv_timeout(Duration::from_secs(1))
            .unwrap()
            .unwrap();

        // list_bindings
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

        // bindings_for_key
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

        // active_layers (empty)
        let (reply_tx, reply_rx) = mpsc::channel();
        runtime
            .commands()
            .send(Command::ActiveLayers { reply: reply_tx })
            .unwrap();
        let layers = reply_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("should receive reply");
        assert!(layers.is_empty());

        // conflicts (empty)
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
            .register_binding(
                RegisteredBinding::new(
                    BindingId::new(),
                    Hotkey::new(Key::C).modifier(Modifier::Ctrl),
                    Action::Swallow,
                )
                .with_options(
                    crate::binding::BindingOptions::default()
                        .with_overlay_visibility(crate::binding::OverlayVisibility::Hidden),
                ),
            )
            .unwrap();

        let bindings = engine.list_bindings();
        assert_eq!(bindings.len(), 1);
        assert_eq!(
            bindings[0].overlay_visibility,
            crate::binding::OverlayVisibility::Hidden
        );
    }

    #[test]
    fn binding_for_key_respects_swallow_layer() {
        let mut engine = test_engine();

        // Global binding for X
        engine
            .register_binding(RegisteredBinding::new(
                BindingId::new(),
                Hotkey::new(Key::X),
                Action::Swallow,
            ))
            .unwrap();

        // Swallow layer with only H bound
        let layer = crate::Layer::new("modal")
            .bind(Key::H, Action::Swallow)
            .swallow();
        engine.define_layer(layer).unwrap();
        engine
            .push_layer(crate::action::LayerName::from("modal"))
            .unwrap();

        // X is not in the swallow layer — the real matcher would
        // return Swallowed, so binding_for_key should return None.
        let result = engine.binding_for_key(&Hotkey::new(Key::X));
        assert!(
            result.is_none(),
            "swallow layer should block fallthrough to global binding"
        );

        // H IS in the swallow layer — should still resolve
        let result = engine.binding_for_key(&Hotkey::new(Key::H));
        assert!(result.is_some());
    }

    #[test]
    fn binding_for_key_returns_none_for_modifier_key() {
        let mut engine = test_engine();

        // Even if someone registers a bare modifier key, the real matcher
        // ignores modifier-only presses. binding_for_key should agree.
        engine
            .register_binding(RegisteredBinding::new(
                BindingId::new(),
                Hotkey::new(Key::LeftCtrl),
                Action::Swallow,
            ))
            .unwrap();

        let result = engine.binding_for_key(&Hotkey::new(Key::LeftCtrl));
        assert!(
            result.is_none(),
            "modifier-only key should not match, consistent with real matcher"
        );
    }
}
