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
//!   → manager returns BindingGuard or Error to caller
//! ```
//!
//! # Reference
//!
//! Prior art: `archive/v0/src/manager.rs` (3000+ lines mixing API with
//! shared-state management). This file should stay small — if it grows
//! past a few hundred lines, something is wrong.

use std::fmt;
use std::sync::Mutex;
use std::sync::mpsc;

use kbd::action::Action;
use kbd::action::LayerName;
use kbd::binding::BindingId;
use kbd::binding::BindingOptions;
use kbd::binding::RegisteredBinding;
use kbd::introspection::ActiveLayerInfo;
use kbd::introspection::BindingInfo;
use kbd::introspection::ConflictInfo;
use kbd::key::Hotkey;
use kbd::key::Key;
use kbd::key::Modifier;
use kbd::layer::Layer;

use crate::Error;
use crate::backend::Backend;
use crate::binding_guard::BindingGuard;
use crate::engine::Command;
use crate::engine::CommandSender;
use crate::engine::EngineRuntime;
use crate::engine::GrabState;

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
#[derive(Debug)]
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
    ///
    /// Spawns the engine thread and begins listening for input device events.
    ///
    /// # Errors
    ///
    /// Returns an error if the backend cannot be initialized, grab mode
    /// is requested without the `grab` feature, or the engine thread
    /// fails to start.
    pub fn build(self) -> Result<HotkeyManager, Error> {
        let backend = resolve_backend(self.backend)?;
        validate_grab_configuration(backend, self.grab)?;

        let grab_state = create_grab_state(self.grab)?;
        let runtime = EngineRuntime::spawn(grab_state)?;
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

impl fmt::Debug for HotkeyManager {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let running = self
            .runtime
            .lock()
            .map(|guard| guard.is_some())
            .unwrap_or(false);

        f.debug_struct("HotkeyManager")
            .field("backend", &self.backend)
            .field("running", &running)
            .finish_non_exhaustive()
    }
}

impl HotkeyManager {
    /// Create a manager with backend auto-detection.
    ///
    /// # Errors
    ///
    /// Returns an error if the backend cannot be initialized or input
    /// devices are not accessible.
    pub fn new() -> Result<Self, Error> {
        Self::builder().build()
    }

    /// Configure manager startup options.
    #[must_use]
    pub fn builder() -> HotkeyManagerBuilder {
        HotkeyManagerBuilder::default()
    }

    /// Returns the backend this manager is using.
    #[must_use]
    pub const fn active_backend(&self) -> Backend {
        self.backend
    }

    /// Register a simple hotkey callback.
    ///
    /// # Errors
    ///
    /// Returns [`Error::AlreadyRegistered`] if the hotkey is already bound,
    /// or [`Error::ManagerStopped`] if the engine has shut down.
    pub fn register<F>(&self, hotkey: impl Into<Hotkey>, callback: F) -> Result<BindingGuard, Error>
    where
        F: Fn() + Send + Sync + 'static,
    {
        self.register_action(hotkey.into(), Action::from(callback))
    }

    /// Register a hotkey with an explicit action and binding options.
    ///
    /// Use when you need metadata (description, overlay visibility) or
    /// behavioral options beyond what [`register()`](Self::register) provides.
    ///
    /// # Errors
    ///
    /// Returns [`Error::AlreadyRegistered`] if the hotkey is already bound,
    /// or [`Error::ManagerStopped`] if the engine has shut down.
    pub fn register_with_options(
        &self,
        hotkey: impl Into<Hotkey>,
        action: impl Into<Action>,
        options: BindingOptions,
    ) -> Result<BindingGuard, Error> {
        let id = BindingId::new();
        let binding =
            RegisteredBinding::new(id, hotkey.into(), action.into()).with_options(options);
        let (reply_tx, reply_rx) = mpsc::channel();

        self.commands.send(Command::Register {
            binding,
            reply: reply_tx,
        })?;

        match reply_rx.recv().map_err(|_| Error::ManagerStopped)? {
            Ok(()) => Ok(BindingGuard::new(id, self.commands.clone())),
            Err(error) => Err(error),
        }
    }

    /// Query whether a hotkey is currently registered.
    ///
    /// # Errors
    ///
    /// Returns [`Error::ManagerStopped`] if the engine has shut down.
    pub fn is_registered(&self, hotkey: impl Into<Hotkey>) -> Result<bool, Error> {
        let hotkey = hotkey.into();
        let (reply_tx, reply_rx) = mpsc::channel();

        self.commands.send(Command::IsRegistered {
            hotkey,
            reply: reply_tx,
        })?;

        reply_rx.recv().map_err(|_| Error::ManagerStopped)
    }

    /// Query whether a specific key is currently pressed on any device.
    ///
    /// # Errors
    ///
    /// Returns [`Error::ManagerStopped`] if the engine has shut down.
    pub fn is_key_pressed(&self, key: Key) -> Result<bool, Error> {
        let (reply_tx, reply_rx) = mpsc::channel();

        self.commands.send(Command::IsKeyPressed {
            key,
            reply: reply_tx,
        })?;

        reply_rx.recv().map_err(|_| Error::ManagerStopped)
    }

    /// Query the set of modifiers currently held, derived from key state.
    ///
    /// Left/right variants are canonicalized: if either `LeftCtrl` or `RightCtrl`
    /// is held, `Modifier::Ctrl` is in the returned set.
    ///
    /// # Errors
    ///
    /// Returns [`Error::ManagerStopped`] if the engine has shut down.
    pub fn active_modifiers(&self) -> Result<Vec<Modifier>, Error> {
        let (reply_tx, reply_rx) = mpsc::channel();

        self.commands
            .send(Command::ActiveModifiers { reply: reply_tx })?;

        reply_rx.recv().map_err(|_| Error::ManagerStopped)
    }

    /// Define a named layer.
    ///
    /// Sends the layer definition to the engine for storage. The layer
    /// is not active until explicitly pushed via [`push_layer()`](Self::push_layer).
    ///
    /// # Errors
    ///
    /// Returns [`Error::LayerAlreadyDefined`] if a layer with the same
    /// name has already been defined, or [`Error::ManagerStopped`] if
    /// the engine has shut down.
    pub fn define_layer(&self, layer: Layer) -> Result<(), Error> {
        let (reply_tx, reply_rx) = mpsc::channel();

        self.commands.send(Command::DefineLayer {
            layer,
            reply: reply_tx,
        })?;

        reply_rx.recv().map_err(|_| Error::ManagerStopped)?
    }

    /// Stop the manager and join the engine thread.
    ///
    /// All registered bindings are dropped. This is also called
    /// automatically when the manager is dropped.
    ///
    /// # Errors
    ///
    /// Returns [`Error::EngineError`] if the engine thread panicked.
    pub fn shutdown(self) -> Result<(), Error> {
        self.shutdown_inner()
    }

    fn register_action(&self, hotkey: Hotkey, action: Action) -> Result<BindingGuard, Error> {
        self.register_with_options(hotkey, action, BindingOptions::default())
    }

    fn shutdown_inner(&self) -> Result<(), Error> {
        let mut runtime = self.runtime.lock().map_err(|_| Error::EngineError)?;
        if let Some(runtime) = runtime.take() {
            return runtime.shutdown();
        }

        Ok(())
    }

    /// Push a named layer onto the layer stack.
    ///
    /// The layer must have been previously defined via [`define_layer`](Self::define_layer).
    /// The pushed layer becomes the highest-priority layer for matching.
    ///
    /// # Errors
    ///
    /// Returns [`Error::LayerNotDefined`] if no layer with the given name exists,
    /// or [`Error::ManagerStopped`] if the engine has shut down.
    pub fn push_layer(&self, name: impl Into<LayerName>) -> Result<(), Error> {
        let (reply_tx, reply_rx) = mpsc::channel();

        self.commands.send(Command::PushLayer {
            name: name.into(),
            reply: reply_tx,
        })?;

        reply_rx.recv().map_err(|_| Error::ManagerStopped)?
    }

    /// Pop the topmost layer from the layer stack.
    ///
    /// Returns the name of the popped layer.
    ///
    /// # Errors
    ///
    /// Returns [`Error::EmptyLayerStack`] if no layers are active,
    /// or [`Error::ManagerStopped`] if the engine has shut down.
    pub fn pop_layer(&self) -> Result<LayerName, Error> {
        let (reply_tx, reply_rx) = mpsc::channel();

        self.commands.send(Command::PopLayer { reply: reply_tx })?;

        reply_rx.recv().map_err(|_| Error::ManagerStopped)?
    }

    /// Toggle a named layer on or off.
    ///
    /// If the layer is currently in the stack, it is removed.
    /// If the layer is not in the stack, it is pushed.
    ///
    /// # Errors
    ///
    /// Returns [`Error::LayerNotDefined`] if no layer with the given name exists,
    /// or [`Error::ManagerStopped`] if the engine has shut down.
    pub fn toggle_layer(&self, name: impl Into<LayerName>) -> Result<(), Error> {
        let (reply_tx, reply_rx) = mpsc::channel();

        self.commands.send(Command::ToggleLayer {
            name: name.into(),
            reply: reply_tx,
        })?;

        reply_rx.recv().map_err(|_| Error::ManagerStopped)?
    }

    /// List all registered bindings with current shadowed status.
    ///
    /// Returns global bindings and all layer bindings (active or not).
    /// Each entry includes whether the binding is currently reachable
    /// or shadowed by a higher-priority layer.
    ///
    /// # Errors
    ///
    /// Returns [`Error::ManagerStopped`] if the engine has shut down.
    pub fn list_bindings(&self) -> Result<Vec<BindingInfo>, Error> {
        let (reply_tx, reply_rx) = mpsc::channel();

        self.commands
            .send(Command::ListBindings { reply: reply_tx })?;

        reply_rx.recv().map_err(|_| Error::ManagerStopped)
    }

    /// Query what would fire if the given hotkey were pressed now.
    ///
    /// Considers the current layer stack. Returns `None` if no binding
    /// matches the hotkey.
    ///
    /// # Errors
    ///
    /// Returns [`Error::ManagerStopped`] if the engine has shut down.
    pub fn bindings_for_key(
        &self,
        hotkey: impl Into<Hotkey>,
    ) -> Result<Option<BindingInfo>, Error> {
        let (reply_tx, reply_rx) = mpsc::channel();

        self.commands.send(Command::BindingsForKey {
            hotkey: hotkey.into(),
            reply: reply_tx,
        })?;

        reply_rx.recv().map_err(|_| Error::ManagerStopped)
    }

    /// Query the current layer stack.
    ///
    /// Returns layers in stack order (bottom to top).
    ///
    /// # Errors
    ///
    /// Returns [`Error::ManagerStopped`] if the engine has shut down.
    pub fn active_layers(&self) -> Result<Vec<ActiveLayerInfo>, Error> {
        let (reply_tx, reply_rx) = mpsc::channel();

        self.commands
            .send(Command::ActiveLayers { reply: reply_tx })?;

        reply_rx.recv().map_err(|_| Error::ManagerStopped)
    }

    /// Find bindings that are shadowed by higher-priority layers.
    ///
    /// Returns conflict pairs: each entry shows the shadowed binding
    /// and the binding that shadows it.
    ///
    /// # Errors
    ///
    /// Returns [`Error::ManagerStopped`] if the engine has shut down.
    pub fn conflicts(&self) -> Result<Vec<ConflictInfo>, Error> {
        let (reply_tx, reply_rx) = mpsc::channel();

        self.commands.send(Command::Conflicts { reply: reply_tx })?;

        reply_rx.recv().map_err(|_| Error::ManagerStopped)
    }

    // TODO: register_sequence() — multi-step hotkey
    // TODO: register_tap_hold() — dual-function key
}

impl Drop for HotkeyManager {
    fn drop(&mut self) {
        let _ = self.shutdown_inner();
    }
}

fn resolve_backend(selection: BackendSelection) -> Result<Backend, Error> {
    match selection {
        BackendSelection::Auto => Ok(Backend::Evdev),
        BackendSelection::Explicit(backend) => validate_explicit_backend(backend),
    }
}

#[allow(clippy::unnecessary_wraps)]
fn validate_explicit_backend(backend: Backend) -> Result<Backend, Error> {
    match backend {
        Backend::Evdev => Ok(Backend::Evdev),
    }
}

#[allow(clippy::unnecessary_wraps)]
fn validate_grab_configuration(_backend: Backend, _grab: GrabConfiguration) -> Result<(), Error> {
    Ok(())
}

fn create_grab_state(grab: GrabConfiguration) -> Result<GrabState, Error> {
    match grab {
        GrabConfiguration::Disabled => Ok(GrabState::Disabled),
        GrabConfiguration::Enabled => {
            #[cfg(feature = "grab")]
            {
                let forwarder = crate::engine::forwarder::UinputForwarder::new()?;
                Ok(GrabState::Enabled {
                    forwarder: Box::new(forwarder),
                })
            }
            #[cfg(not(feature = "grab"))]
            {
                Err(Error::UnsupportedFeature)
            }
        }
    }
}
