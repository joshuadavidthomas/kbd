//! [`HotkeyManager`] — the public API entry point.
//!
//! Thin. Sends commands to the engine, returns handles. Does not own
//! mutable state — the engine owns everything.
//!
//! # Architecture
//!
//! The manager holds a command channel sender and a wake mechanism.
//! Every public method translates to a `Command` sent to the engine.
//! Operations that can fail (register, `define_layer`) use a reply
//! channel to return `Result` synchronously to the caller.
//!
//! ```text
//! HotkeyManager::register()
//!   → sends Command::Register { id, binding, reply_tx }
//!   → engine processes command, sends Result back on reply_tx
//!   → manager returns Handle or Error to caller
//! ```
//!
//! # Reference
//!
//! Prior art: `archive/v0/src/manager.rs` (3000+ lines mixing API with
//! shared-state management). This file should stay small — if it grows
//! past a few hundred lines, something is wrong.

use std::sync::mpsc;
use std::sync::Mutex;

use crate::action::Action;
use crate::backend::Backend;
use crate::binding::BindingId;
use crate::engine::Command;
use crate::engine::CommandSender;
use crate::engine::EngineRuntime;
use crate::engine::RegisteredBinding;
use crate::handle::Handle;
use crate::key::Hotkey;
use crate::key::Key;
use crate::key::Modifier;
use crate::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BackendSelection {
    Auto,
    Explicit(Backend),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GrabConfiguration {
    Disabled,
    Enabled,
}

/// Builder for explicit backend and runtime options.
pub struct HotkeyManagerBuilder {
    backend: BackendSelection,
    grab: GrabConfiguration,
}

impl Default for HotkeyManagerBuilder {
    fn default() -> Self {
        Self {
            backend: BackendSelection::Auto,
            grab: GrabConfiguration::Disabled,
        }
    }
}

impl HotkeyManagerBuilder {
    /// Force a specific backend instead of auto-detection.
    #[must_use]
    pub fn backend(mut self, backend: Backend) -> Self {
        self.backend = BackendSelection::Explicit(backend);
        self
    }

    /// Enable grab mode (backend support added in Phase 2).
    #[must_use]
    pub fn grab(mut self) -> Self {
        self.grab = GrabConfiguration::Enabled;
        self
    }

    /// Build and start a new manager instance.
    pub fn build(self) -> Result<HotkeyManager, Error> {
        let backend = resolve_backend(self.backend)?;
        validate_grab_configuration(backend, self.grab)?;

        let runtime = EngineRuntime::spawn()?;
        let commands = runtime.commands();

        Ok(HotkeyManager {
            backend,
            commands,
            runtime: Mutex::new(Some(runtime)),
        })
    }
}

/// Public manager API.
pub struct HotkeyManager {
    backend: Backend,
    commands: CommandSender,
    runtime: Mutex<Option<EngineRuntime>>,
}

impl HotkeyManager {
    /// Create a manager with backend auto-detection.
    pub fn new() -> Result<Self, Error> {
        Self::builder().build()
    }

    /// Configure manager startup options.
    #[must_use]
    pub fn builder() -> HotkeyManagerBuilder {
        HotkeyManagerBuilder::default()
    }

    #[must_use]
    pub const fn active_backend(&self) -> Backend {
        self.backend
    }

    /// Register a simple hotkey callback.
    pub fn register<F>(&self, key: Key, modifiers: &[Modifier], callback: F) -> Result<Handle, Error>
    where
        F: Fn() + Send + Sync + 'static,
    {
        self.register_action(key, modifiers, Action::from(callback))
    }

    /// Query whether a hotkey is currently registered.
    pub fn is_registered(&self, key: Key, modifiers: &[Modifier]) -> Result<bool, Error> {
        let hotkey = Hotkey::new(key, modifiers.to_vec());
        let (reply_tx, reply_rx) = mpsc::channel();

        self.commands.send(Command::IsRegistered {
            hotkey,
            reply: reply_tx,
        })?;

        reply_rx.recv().map_err(|_| Error::ManagerStopped)
    }

    /// Stop the manager and join the engine thread.
    pub fn shutdown(self) -> Result<(), Error> {
        self.shutdown_inner()
    }

    fn register_action(
        &self,
        key: Key,
        modifiers: &[Modifier],
        action: Action,
    ) -> Result<Handle, Error> {
        let id = BindingId::new();
        let hotkey = Hotkey::new(key, modifiers.to_vec());
        let binding = RegisteredBinding::new(id, hotkey, action);
        let (reply_tx, reply_rx) = mpsc::channel();

        self.commands.send(Command::Register {
            binding,
            reply: reply_tx,
        })?;

        match reply_rx.recv().map_err(|_| Error::ManagerStopped)? {
            Ok(()) => Ok(Handle::new(id, self.commands.clone())),
            Err(error) => Err(error),
        }
    }

    fn shutdown_inner(&self) -> Result<(), Error> {
        let mut runtime = self.runtime.lock().map_err(|_| Error::EngineError)?;
        if let Some(runtime) = runtime.take() {
            return runtime.shutdown();
        }

        Ok(())
    }

    // TODO: register_sequence() — multi-step hotkey
    // TODO: register_tap_hold() — dual-function key
    // TODO: define_layer() — register a Layer
    // TODO: push_layer() / pop_layer() — layer stack control
    // TODO: is_key_pressed() / active_modifiers() — state queries
}

impl Drop for HotkeyManager {
    fn drop(&mut self) {
        let _ = self.shutdown_inner();
    }
}

fn resolve_backend(selection: BackendSelection) -> Result<Backend, Error> {
    match selection {
        BackendSelection::Auto => auto_backend(),
        BackendSelection::Explicit(backend) => validate_explicit_backend(backend),
    }
}

// With `--all-features` (evdev on), this always returns Ok. But under
// portal-only or no-backend builds it genuinely fails, so the Result is needed.
#[allow(clippy::unnecessary_wraps)]
fn auto_backend() -> Result<Backend, Error> {
    #[cfg(feature = "evdev")]
    {
        Ok(Backend::Evdev)
    }
    #[cfg(not(feature = "evdev"))]
    {
        Err(Error::BackendUnavailable)
    }
}

fn validate_explicit_backend(backend: Backend) -> Result<Backend, Error> {
    match backend {
        #[cfg(feature = "evdev")]
        Backend::Evdev => Ok(Backend::Evdev),
        #[cfg(feature = "portal")]
        Backend::Portal => Err(Error::BackendUnavailable),
    }
}

fn validate_grab_configuration(backend: Backend, grab: GrabConfiguration) -> Result<(), Error> {
    let _ = backend;

    if matches!(grab, GrabConfiguration::Enabled) {
        #[cfg(feature = "portal")]
        if matches!(backend, Backend::Portal) {
            return Err(Error::UnsupportedFeature);
        }
    }

    Ok(())
}
