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
//!   → sends Command::Register { binding, reply }
//!   → engine processes command, sends Result back on reply
//!   → manager returns BindingGuard or error to caller
//! ```

use std::fmt;
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::mpsc;

use kbd::action::Action;
use kbd::binding::BindingId;
use kbd::binding::BindingOptions;
use kbd::binding::RegisteredBinding;
use kbd::hotkey::HotkeyInput;
use kbd::hotkey::ModifierSet;
use kbd::introspection::ActiveLayerInfo;
use kbd::introspection::BindingInfo;
use kbd::introspection::ConflictInfo;
use kbd::key::Key;
use kbd::layer::Layer;
use kbd::layer::LayerName;
use kbd::sequence::PendingSequenceInfo;
use kbd::sequence::SequenceInput;
use kbd::sequence::SequenceOptions;

use crate::ManagerStopped;
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

    /// Enable grab mode for exclusive device capture.
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
    pub fn build(self) -> Result<HotkeyManager, crate::StartupError> {
        let backend = resolve_backend(self.backend)?;
        validate_grab_configuration(backend, self.grab)?;

        let grab_state = create_grab_state(self.grab)?;
        let runtime = if let Some(input_directory) = internal_test_input_directory() {
            EngineRuntime::spawn_with_input_dir(grab_state, &input_directory)?
        } else {
            EngineRuntime::spawn(grab_state)?
        };
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
    pub fn new() -> Result<Self, crate::StartupError> {
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
    /// Accepts any type implementing [`HotkeyInput`]: a [`Hotkey`], a
    /// [`Key`], or a string (`&str` / `String`).
    ///
    /// # Errors
    ///
    /// Returns [`RegisterError::Parse`](crate::RegisterError::Parse) when
    /// string input conversion fails,
    /// [`RegisterError::AlreadyRegistered`](crate::RegisterError::AlreadyRegistered)
    /// if the hotkey is already bound, or
    /// [`RegisterError::ManagerStopped`](crate::RegisterError::ManagerStopped)
    /// if the engine has shut down.
    pub fn register<F>(
        &self,
        hotkey: impl HotkeyInput,
        callback: F,
    ) -> Result<BindingGuard, crate::RegisterError>
    where
        F: Fn() + Send + Sync + 'static,
    {
        self.register_with_options(hotkey, Action::from(callback), BindingOptions::default())
    }

    /// Register a multi-step sequence callback.
    ///
    /// # Errors
    ///
    /// Returns [`RegisterError::Parse`](crate::RegisterError::Parse) when
    /// sequence input conversion fails,
    /// [`RegisterError::AlreadyRegistered`](crate::RegisterError::AlreadyRegistered)
    /// if the sequence is already bound, or
    /// [`RegisterError::ManagerStopped`](crate::RegisterError::ManagerStopped)
    /// if the engine has shut down.
    pub fn register_sequence<F>(
        &self,
        sequence: impl SequenceInput,
        callback: F,
    ) -> Result<BindingGuard, crate::RegisterError>
    where
        F: Fn() + Send + Sync + 'static,
    {
        self.register_sequence_with_options(
            sequence,
            Action::from(callback),
            SequenceOptions::default(),
        )
    }

    /// Register a multi-step sequence with explicit action and options.
    ///
    /// # Errors
    ///
    /// Returns [`RegisterError::Parse`](crate::RegisterError::Parse) when
    /// sequence input conversion fails,
    /// [`RegisterError::AlreadyRegistered`](crate::RegisterError::AlreadyRegistered)
    /// if the sequence is already bound, or
    /// [`RegisterError::ManagerStopped`](crate::RegisterError::ManagerStopped)
    /// if the engine has shut down.
    pub fn register_sequence_with_options(
        &self,
        sequence: impl SequenceInput,
        action: impl Into<Action>,
        options: SequenceOptions,
    ) -> Result<BindingGuard, crate::RegisterError> {
        let sequence = sequence.into_sequence()?;
        let action = action.into();
        let id = self.request(|reply| Command::RegisterSequence {
            sequence,
            action,
            options,
            reply,
        })??;
        Ok(BindingGuard::new(id, self.commands.clone()))
    }

    /// Register a hotkey with an explicit action and binding options.
    ///
    /// Accepts any type implementing [`HotkeyInput`]: a [`Hotkey`], a
    /// [`Key`], or a string (`&str` / `String`).
    ///
    /// Use when you need metadata (description, overlay visibility) or
    /// behavioral options beyond what [`register()`](Self::register) provides.
    ///
    /// # Errors
    ///
    /// Returns [`RegisterError::Parse`](crate::RegisterError::Parse) when
    /// string input conversion fails,
    /// [`RegisterError::AlreadyRegistered`](crate::RegisterError::AlreadyRegistered)
    /// if the hotkey is already bound, or
    /// [`RegisterError::ManagerStopped`](crate::RegisterError::ManagerStopped)
    /// if the engine has shut down.
    pub fn register_with_options(
        &self,
        hotkey: impl HotkeyInput,
        action: impl Into<Action>,
        options: BindingOptions,
    ) -> Result<BindingGuard, crate::RegisterError> {
        let id = BindingId::new();
        let hotkey = hotkey.into_hotkey()?;
        let binding = RegisteredBinding::new(id, hotkey, action.into()).with_options(options);

        self.request(|reply| Command::Register { binding, reply })??;
        Ok(BindingGuard::new(id, self.commands.clone()))
    }

    /// Query whether a hotkey is currently registered.
    ///
    /// Accepts any type implementing [`HotkeyInput`]: a [`Hotkey`], a
    /// [`Key`], or a string (`&str` / `String`).
    ///
    /// # Errors
    ///
    /// Returns [`QueryError::Parse`](crate::QueryError::Parse) when string
    /// input conversion fails, or
    /// [`QueryError::ManagerStopped`](crate::QueryError::ManagerStopped) if
    /// the engine has shut down.
    pub fn is_registered(&self, hotkey: impl HotkeyInput) -> Result<bool, crate::QueryError> {
        let hotkey = hotkey.into_hotkey()?;
        Ok(self.request(|reply| Command::IsRegistered { hotkey, reply })?)
    }

    /// Query whether a specific key is currently pressed on any device.
    ///
    /// # Errors
    ///
    /// Returns [`ManagerStopped`] if the engine has shut down.
    pub fn is_key_pressed(&self, key: Key) -> Result<bool, ManagerStopped> {
        self.request(|reply| Command::IsKeyPressed { key, reply })
    }

    /// Query the set of modifiers currently held, derived from key state.
    ///
    /// Left/right variants are canonicalized: if either `LeftCtrl` or `RightCtrl`
    /// is held, `Modifier::Ctrl` is in the returned set.
    ///
    /// # Errors
    ///
    /// Returns [`ManagerStopped`] if the engine has shut down.
    pub fn active_modifiers(&self) -> Result<ModifierSet, ManagerStopped> {
        self.request(|reply| Command::ActiveModifiers { reply })
    }

    /// Define a named layer.
    ///
    /// Sends the layer definition to the engine for storage. The layer
    /// is not active until explicitly pushed via [`push_layer()`](Self::push_layer).
    ///
    /// # Errors
    ///
    /// Returns [`LayerError::AlreadyDefined`](crate::LayerError::AlreadyDefined)
    /// if a layer with the same name has already been defined, or
    /// [`LayerError::ManagerStopped`](crate::LayerError::ManagerStopped)
    /// if the engine has shut down.
    pub fn define_layer(&self, layer: Layer) -> Result<(), crate::LayerError> {
        self.request(|reply| Command::DefineLayer { layer, reply })?
    }

    /// Stop the manager and join the engine thread.
    ///
    /// All registered bindings are dropped. This is also called
    /// automatically when the manager is dropped.
    ///
    /// # Errors
    ///
    /// Returns [`ShutdownError::Engine`](crate::ShutdownError::Engine) if
    /// the engine thread panicked.
    pub fn shutdown(self) -> Result<(), crate::ShutdownError> {
        self.shutdown_inner()
    }

    /// Send a command that carries a reply channel and wait for the response.
    ///
    /// Encapsulates the channel-create → send → recv → map-error boilerplate
    /// shared by every request/reply manager method.
    fn request<T>(
        &self,
        build: impl FnOnce(mpsc::Sender<T>) -> Command,
    ) -> Result<T, ManagerStopped> {
        let (reply_tx, reply_rx) = mpsc::channel();
        self.commands.send(build(reply_tx))?;
        reply_rx.recv().map_err(|_| ManagerStopped)
    }

    fn shutdown_inner(&self) -> Result<(), crate::ShutdownError> {
        let mut runtime = self
            .runtime
            .lock()
            .map_err(|_| crate::ShutdownError::Engine)?;
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
    /// Returns [`LayerError::NotDefined`](crate::LayerError::NotDefined) if
    /// no layer with the given name exists, or
    /// [`LayerError::ManagerStopped`](crate::LayerError::ManagerStopped) if
    /// the engine has shut down.
    pub fn push_layer(&self, name: impl Into<LayerName>) -> Result<(), crate::LayerError> {
        self.request(|reply| Command::PushLayer {
            name: name.into(),
            reply,
        })?
    }

    /// Pop the topmost layer from the layer stack.
    ///
    /// Returns the name of the popped layer.
    ///
    /// # Errors
    ///
    /// Returns [`LayerError::EmptyStack`](crate::LayerError::EmptyStack)
    /// if no layers are active, or
    /// [`LayerError::ManagerStopped`](crate::LayerError::ManagerStopped) if
    /// the engine has shut down.
    pub fn pop_layer(&self) -> Result<LayerName, crate::LayerError> {
        self.request(|reply| Command::PopLayer { reply })?
    }

    /// Toggle a named layer on or off.
    ///
    /// If the layer is currently in the stack, it is removed.
    /// If the layer is not in the stack, it is pushed.
    ///
    /// # Errors
    ///
    /// Returns [`LayerError::NotDefined`](crate::LayerError::NotDefined) if
    /// no layer with the given name exists, or
    /// [`LayerError::ManagerStopped`](crate::LayerError::ManagerStopped) if
    /// the engine has shut down.
    pub fn toggle_layer(&self, name: impl Into<LayerName>) -> Result<(), crate::LayerError> {
        self.request(|reply| Command::ToggleLayer {
            name: name.into(),
            reply,
        })?
    }

    /// List all registered bindings with current shadowed status.
    ///
    /// Returns global bindings and all layer bindings (active or not).
    /// Each entry includes whether the binding is currently reachable
    /// or shadowed by a higher-priority layer.
    ///
    /// # Errors
    ///
    /// Returns [`ManagerStopped`] if the engine has shut down.
    pub fn list_bindings(&self) -> Result<Vec<BindingInfo>, ManagerStopped> {
        self.request(|reply| Command::ListBindings { reply })
    }

    /// Query what would fire if the given hotkey were pressed now.
    ///
    /// Considers the current layer stack. Returns `None` if no binding
    /// matches the hotkey.
    ///
    /// # Errors
    ///
    /// Returns [`QueryError::Parse`](crate::QueryError::Parse) when string
    /// input conversion fails, or
    /// [`QueryError::ManagerStopped`](crate::QueryError::ManagerStopped) if
    /// the engine has shut down.
    pub fn bindings_for_key(
        &self,
        hotkey: impl HotkeyInput,
    ) -> Result<Option<BindingInfo>, crate::QueryError> {
        let hotkey = hotkey.into_hotkey()?;
        Ok(self.request(|reply| Command::BindingsForKey { hotkey, reply })?)
    }

    /// Query the current layer stack.
    ///
    /// Returns layers in stack order (bottom to top).
    ///
    /// # Errors
    ///
    /// Returns [`ManagerStopped`] if the engine has shut down.
    pub fn active_layers(&self) -> Result<Vec<ActiveLayerInfo>, ManagerStopped> {
        self.request(|reply| Command::ActiveLayers { reply })
    }

    /// Return current in-progress sequence state, if any.
    ///
    /// # Errors
    ///
    /// Returns [`ManagerStopped`] if the engine has shut down.
    pub fn pending_sequence(&self) -> Result<Option<PendingSequenceInfo>, ManagerStopped> {
        self.request(|reply| Command::PendingSequence { reply })
    }

    /// Register a tap-hold binding for a dual-function key.
    ///
    /// The `tap_action` fires when the key is pressed and released quickly
    /// (before the threshold). The `hold_action` fires when the key is held
    /// past the threshold or interrupted by another keypress.
    ///
    /// **Requires grab mode.** Tap-hold must intercept and buffer key events
    /// before they reach other applications — without grab, the original key
    /// event would be delivered immediately.
    ///
    /// # Errors
    ///
    /// Returns [`RegisterError::UnsupportedFeature`](crate::RegisterError::UnsupportedFeature)
    /// if grab mode is not enabled,
    /// [`RegisterError::AlreadyRegistered`](crate::RegisterError::AlreadyRegistered)
    /// if the key already has a tap-hold binding, or
    /// [`RegisterError::ManagerStopped`](crate::RegisterError::ManagerStopped)
    /// if the engine has shut down.
    pub fn register_tap_hold(
        &self,
        key: Key,
        tap_action: impl Into<Action>,
        hold_action: impl Into<Action>,
        options: kbd::tap_hold::TapHoldOptions,
    ) -> Result<BindingGuard, crate::RegisterError> {
        let id = self.request(|reply| Command::RegisterTapHold {
            key,
            tap_action: tap_action.into(),
            hold_action: hold_action.into(),
            options,
            reply,
        })??;
        Ok(BindingGuard::new(id, self.commands.clone()))
    }

    /// Find bindings that are shadowed by higher-priority layers.
    ///
    /// Returns conflict pairs: each entry shows the shadowed binding
    /// and the binding that shadows it.
    ///
    /// # Errors
    ///
    /// Returns [`ManagerStopped`] if the engine has shut down.
    pub fn conflicts(&self) -> Result<Vec<ConflictInfo>, ManagerStopped> {
        self.request(|reply| Command::Conflicts { reply })
    }
}

impl Drop for HotkeyManager {
    fn drop(&mut self) {
        let _ = self.shutdown_inner();
    }
}

fn resolve_backend(selection: BackendSelection) -> Result<Backend, crate::StartupError> {
    match selection {
        BackendSelection::Auto => Ok(Backend::Evdev),
        BackendSelection::Explicit(backend) => validate_explicit_backend(backend),
    }
}

#[allow(clippy::unnecessary_wraps)]
fn validate_explicit_backend(backend: Backend) -> Result<Backend, crate::StartupError> {
    match backend {
        Backend::Evdev => Ok(Backend::Evdev),
    }
}

#[allow(clippy::unnecessary_wraps)]
fn validate_grab_configuration(
    _backend: Backend,
    _grab: GrabConfiguration,
) -> Result<(), crate::StartupError> {
    Ok(())
}

fn internal_test_input_directory() -> Option<PathBuf> {
    std::env::var_os("_KBD_GLOBAL_INTERNAL_TEST_INPUT_DIR").map(PathBuf::from)
}

fn create_grab_state(grab: GrabConfiguration) -> Result<GrabState, crate::StartupError> {
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
                Err(crate::StartupError::UnsupportedFeature)
            }
        }
    }
}
