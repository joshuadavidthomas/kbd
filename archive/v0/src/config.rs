use std::collections::HashMap;
use std::fmt;
use std::str::FromStr;
use std::sync::Arc;

use serde::de::Error as _;
use serde::Deserialize;
use serde::Deserializer;
use serde::Serialize;
use serde::Serializer;

use crate::Error;
use crate::Handle;
use crate::Hotkey;
use crate::HotkeyManager;
use crate::HotkeySequence;
use crate::ModeOptions;
use crate::SequenceHandle;
use crate::SequenceOptions;

/// Thread-safe callback type used internally by [`ActionMap`].
pub type ActionCallback = Arc<dyn Fn() + Send + Sync + 'static>;

/// A validated, non-empty identifier for a config action.
///
/// Parse from a string or construct with [`ActionId::new`]:
///
/// ```
/// use keybound::ActionId;
///
/// let id: ActionId = "launch-terminal".parse().unwrap();
/// assert_eq!(id.as_str(), "launch-terminal");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ActionId(String);

impl ActionId {
    /// Create an action ID from a string. Returns an error if the string is
    /// empty or contains only whitespace.
    pub fn new(value: impl Into<String>) -> Result<Self, ActionIdError> {
        let value = value.into();
        let normalized = value.trim();
        if normalized.is_empty() {
            return Err(ActionIdError::Empty);
        }

        Ok(Self(normalized.to_string()))
    }

    /// Returns the action ID as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ActionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for ActionId {
    type Err = ActionIdError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        ActionId::new(s)
    }
}

impl Serialize for ActionId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for ActionId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = String::deserialize(deserializer)?;
        ActionId::new(raw).map_err(D::Error::custom)
    }
}

/// Errors from [`ActionId`] creation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActionIdError {
    /// The action ID string was empty or whitespace-only.
    Empty,
}

impl fmt::Display for ActionIdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ActionIdError::Empty => write!(f, "action id cannot be empty"),
        }
    }
}

impl std::error::Error for ActionIdError {}

/// Maps [`ActionId`]s to callbacks for use with [`HotkeyConfig::register`].
///
/// Each action ID can have exactly one callback. Attempting to insert a
/// duplicate returns [`ActionMapError::DuplicateAction`].
#[derive(Default)]
pub struct ActionMap {
    callbacks: HashMap<ActionId, ActionCallback>,
}

impl ActionMap {
    /// Create an empty action map.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Associate a callback with an action ID.
    pub fn insert<F>(&mut self, action: ActionId, callback: F) -> Result<(), ActionMapError>
    where
        F: Fn() + Send + Sync + 'static,
    {
        match self.callbacks.entry(action) {
            std::collections::hash_map::Entry::Occupied(entry) => {
                Err(ActionMapError::DuplicateAction(entry.key().clone()))
            }
            std::collections::hash_map::Entry::Vacant(entry) => {
                entry.insert(Arc::new(callback));
                Ok(())
            }
        }
    }

    fn resolve(&self, action: &ActionId) -> Option<ActionCallback> {
        self.callbacks.get(action).cloned()
    }
}

/// Errors from [`ActionMap`] operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActionMapError {
    /// A callback was already registered for this action ID.
    DuplicateAction(ActionId),
}

impl fmt::Display for ActionMapError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ActionMapError::DuplicateAction(action) => {
                write!(f, "action callback already exists for {action}")
            }
        }
    }
}

impl std::error::Error for ActionMapError {}

/// Declarative hotkey configuration, deserializable from TOML/JSON/YAML.
///
/// A config can contain hotkeys, key sequences, and mode definitions. Use
/// [`HotkeyConfig::register`] to apply the entire config to a manager.
///
/// # TOML example
///
/// ```toml
/// [[hotkeys]]
/// hotkey = "Ctrl+Shift+N"
/// action = "new_window"
///
/// [[sequences]]
/// sequence = "Ctrl+K, Ctrl+S"
/// action = "save_all"
///
/// [modes.resize]
/// bindings = [
///     { hotkey = "H", action = "shrink_left" },
///     { hotkey = "Escape", action = "exit_mode" },
/// ]
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct HotkeyConfig {
    #[serde(default)]
    hotkeys: Vec<HotkeyBinding>,
    #[serde(default)]
    sequences: Vec<SequenceBinding>,
    #[serde(default)]
    modes: HashMap<String, ModeBindings>,
}

impl HotkeyConfig {
    /// Create a config programmatically.
    #[must_use]
    pub fn new(
        hotkeys: Vec<HotkeyBinding>,
        sequences: Vec<SequenceBinding>,
        modes: HashMap<String, ModeBindings>,
    ) -> Self {
        Self {
            hotkeys,
            sequences,
            modes,
        }
    }

    /// Returns the hotkey bindings in this config.
    #[must_use]
    pub fn hotkeys(&self) -> &[HotkeyBinding] {
        &self.hotkeys
    }

    /// Returns the sequence bindings in this config.
    #[must_use]
    pub fn sequences(&self) -> &[SequenceBinding] {
        &self.sequences
    }

    /// Returns the mode definitions in this config, keyed by mode name.
    #[must_use]
    pub fn modes(&self) -> &HashMap<String, ModeBindings> {
        &self.modes
    }

    /// Register all bindings from this config with the given manager.
    ///
    /// Every action referenced in the config must have a corresponding entry
    /// in the [`ActionMap`]. On success, returns a [`RegisteredConfig`] holding
    /// all the handles. On failure, already-registered bindings are rolled back.
    ///
    /// # Panics
    ///
    /// Panics if a validated action cannot be resolved from the action map.
    pub fn register(
        &self,
        manager: &HotkeyManager,
        actions: &ActionMap,
    ) -> Result<RegisteredConfig, ConfigRegistrationError> {
        self.validate_actions(actions)?;

        let mut registered = RegisteredConfig::default();

        for binding in &self.hotkeys {
            let callback = actions
                .resolve(binding.action())
                .expect("validated action should exist");
            let handle = match manager.register(
                binding.hotkey().key(),
                binding.hotkey().modifiers(),
                move || callback(),
            ) {
                Ok(handle) => handle,
                Err(error) => {
                    registered.rollback(manager);
                    return Err(error.into());
                }
            };
            registered.hotkey_handles.push(handle);
        }

        for binding in &self.sequences {
            let callback = actions
                .resolve(binding.action())
                .expect("validated action should exist");
            let handle = match manager.register_sequence(
                binding.sequence(),
                SequenceOptions::new(),
                move || callback(),
            ) {
                Ok(handle) => handle,
                Err(error) => {
                    registered.rollback(manager);
                    return Err(error.into());
                }
            };
            registered.sequence_handles.push(handle);
        }

        let mut modes: Vec<(&String, &ModeBindings)> = self.modes.iter().collect();
        modes.sort_by(|(left_name, _), (right_name, _)| left_name.cmp(right_name));

        for (mode_name, mode) in modes {
            let resolved_bindings: Vec<(Hotkey, ActionCallback)> = mode
                .bindings()
                .iter()
                .map(|binding| {
                    let callback = actions
                        .resolve(binding.action())
                        .expect("validated action should exist");
                    (binding.hotkey().clone(), callback)
                })
                .collect();

            if let Err(error) = manager.define_mode(mode_name, ModeOptions::new(), |builder| {
                for (hotkey, callback) in &resolved_bindings {
                    let callback = callback.clone();
                    builder.register(hotkey.key(), hotkey.modifiers(), move || callback())?;
                }
                Ok(())
            }) {
                registered.rollback(manager);
                return Err(error.into());
            }

            registered.defined_modes.push(mode_name.clone());
        }

        Ok(registered)
    }

    fn validate_actions(&self, actions: &ActionMap) -> Result<(), ConfigRegistrationError> {
        for (index, binding) in self.hotkeys.iter().enumerate() {
            if actions.resolve(binding.action()).is_none() {
                return Err(ConfigRegistrationError::MissingAction {
                    action: binding.action().clone(),
                    location: BindingLocation::Hotkey {
                        index,
                        hotkey: binding.hotkey().to_string(),
                    }
                    .to_string(),
                });
            }
        }

        for (index, binding) in self.sequences.iter().enumerate() {
            if actions.resolve(binding.action()).is_none() {
                return Err(ConfigRegistrationError::MissingAction {
                    action: binding.action().clone(),
                    location: BindingLocation::Sequence {
                        index,
                        sequence: binding.sequence().to_string(),
                    }
                    .to_string(),
                });
            }
        }

        let mut modes: Vec<(&String, &ModeBindings)> = self.modes.iter().collect();
        modes.sort_by(|(left_name, _), (right_name, _)| left_name.cmp(right_name));

        for (mode_name, mode) in modes {
            for (index, binding) in mode.bindings().iter().enumerate() {
                if actions.resolve(binding.action()).is_none() {
                    return Err(ConfigRegistrationError::MissingAction {
                        action: binding.action().clone(),
                        location: BindingLocation::ModeHotkey {
                            mode_name: mode_name.clone(),
                            index,
                            hotkey: binding.hotkey().to_string(),
                        }
                        .to_string(),
                    });
                }
            }
        }

        Ok(())
    }
}

/// A single hotkey-to-action binding in a [`HotkeyConfig`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HotkeyBinding {
    hotkey: Hotkey,
    action: ActionId,
}

impl HotkeyBinding {
    /// Create a new binding.
    #[must_use]
    pub fn new(hotkey: Hotkey, action: ActionId) -> Self {
        Self { hotkey, action }
    }

    /// Returns the hotkey for this binding.
    #[must_use]
    pub fn hotkey(&self) -> &Hotkey {
        &self.hotkey
    }

    /// Returns the action ID for this binding.
    #[must_use]
    pub fn action(&self) -> &ActionId {
        &self.action
    }
}

/// A sequence-to-action binding in a [`HotkeyConfig`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SequenceBinding {
    sequence: HotkeySequence,
    action: ActionId,
}

impl SequenceBinding {
    /// Create a new sequence binding.
    #[must_use]
    pub fn new(sequence: HotkeySequence, action: ActionId) -> Self {
        Self { sequence, action }
    }

    /// Returns the key sequence for this binding.
    #[must_use]
    pub fn sequence(&self) -> &HotkeySequence {
        &self.sequence
    }

    /// Returns the action ID for this binding.
    #[must_use]
    pub fn action(&self) -> &ActionId {
        &self.action
    }
}

/// The bindings for a single mode in a [`HotkeyConfig`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct ModeBindings {
    #[serde(default)]
    bindings: Vec<HotkeyBinding>,
}

impl ModeBindings {
    /// Create mode bindings from a list of hotkey bindings.
    #[must_use]
    pub fn new(bindings: Vec<HotkeyBinding>) -> Self {
        Self { bindings }
    }

    /// Returns the hotkey bindings for this mode.
    #[must_use]
    pub fn bindings(&self) -> &[HotkeyBinding] {
        &self.bindings
    }
}

/// Holds the handles from a successful [`HotkeyConfig::register`] call.
///
/// Dropping this struct does **not** unregister the bindings — use the
/// individual handles or [`HotkeyManager::unregister_all`](crate::HotkeyManager::unregister_all)
/// for that.
#[derive(Default)]
pub struct RegisteredConfig {
    hotkey_handles: Vec<Handle>,
    sequence_handles: Vec<SequenceHandle>,
    defined_modes: Vec<String>,
}

impl RegisteredConfig {
    /// Returns the handles for registered hotkey bindings.
    #[must_use]
    pub fn hotkey_handles(&self) -> &[Handle] {
        &self.hotkey_handles
    }

    /// Returns the handles for registered sequence bindings.
    #[must_use]
    pub fn sequence_handles(&self) -> &[SequenceHandle] {
        &self.sequence_handles
    }

    /// Returns the names of modes that were defined during registration.
    #[must_use]
    pub fn defined_modes(&self) -> &[String] {
        &self.defined_modes
    }

    fn rollback(&mut self, manager: &HotkeyManager) {
        for mode_name in self.defined_modes.drain(..).rev() {
            manager.remove_mode_definition(&mode_name);
        }

        for handle in self.sequence_handles.drain(..).rev() {
            let _ = handle.unregister();
        }

        for handle in self.hotkey_handles.drain(..).rev() {
            let _ = handle.unregister();
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum BindingLocation {
    Hotkey {
        index: usize,
        hotkey: String,
    },
    Sequence {
        index: usize,
        sequence: String,
    },
    ModeHotkey {
        mode_name: String,
        index: usize,
        hotkey: String,
    },
}

impl fmt::Display for BindingLocation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BindingLocation::Hotkey { index, hotkey } => {
                write!(f, "hotkeys[{index}] ({hotkey})")
            }
            BindingLocation::Sequence { index, sequence } => {
                write!(f, "sequences[{index}] ({sequence})")
            }
            BindingLocation::ModeHotkey {
                mode_name,
                index,
                hotkey,
            } => write!(f, "modes.{mode_name}.bindings[{index}] ({hotkey})"),
        }
    }
}

/// Errors from [`HotkeyConfig::register`].
#[derive(Debug)]
pub enum ConfigRegistrationError {
    /// The config references an action ID that has no callback in the
    /// [`ActionMap`].
    MissingAction { action: ActionId, location: String },
    /// A registration call to the manager failed.
    Register(Error),
}

impl fmt::Display for ConfigRegistrationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConfigRegistrationError::MissingAction { action, location } => {
                write!(f, "missing callback for action '{action}' at {location}")
            }
            ConfigRegistrationError::Register(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for ConfigRegistrationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ConfigRegistrationError::MissingAction { .. } => None,
            ConfigRegistrationError::Register(error) => Some(error),
        }
    }
}

impl From<Error> for ConfigRegistrationError {
    fn from(value: Error) -> Self {
        ConfigRegistrationError::Register(value)
    }
}
