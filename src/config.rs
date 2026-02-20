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

pub type ActionCallback = Arc<dyn Fn() + Send + Sync + 'static>;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ActionId(String);

impl ActionId {
    pub fn new(value: impl Into<String>) -> Result<Self, ActionIdError> {
        let value = value.into();
        let normalized = value.trim();
        if normalized.is_empty() {
            return Err(ActionIdError::Empty);
        }

        Ok(Self(normalized.to_string()))
    }

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActionIdError {
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

#[derive(Default)]
pub struct ActionMap {
    callbacks: HashMap<ActionId, ActionCallback>,
}

impl ActionMap {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActionMapError {
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

    #[must_use]
    pub fn hotkeys(&self) -> &[HotkeyBinding] {
        &self.hotkeys
    }

    #[must_use]
    pub fn sequences(&self) -> &[SequenceBinding] {
        &self.sequences
    }

    #[must_use]
    pub fn modes(&self) -> &HashMap<String, ModeBindings> {
        &self.modes
    }

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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HotkeyBinding {
    hotkey: Hotkey,
    action: ActionId,
}

impl HotkeyBinding {
    #[must_use]
    pub fn new(hotkey: Hotkey, action: ActionId) -> Self {
        Self { hotkey, action }
    }

    #[must_use]
    pub fn hotkey(&self) -> &Hotkey {
        &self.hotkey
    }

    #[must_use]
    pub fn action(&self) -> &ActionId {
        &self.action
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SequenceBinding {
    sequence: HotkeySequence,
    action: ActionId,
}

impl SequenceBinding {
    #[must_use]
    pub fn new(sequence: HotkeySequence, action: ActionId) -> Self {
        Self { sequence, action }
    }

    #[must_use]
    pub fn sequence(&self) -> &HotkeySequence {
        &self.sequence
    }

    #[must_use]
    pub fn action(&self) -> &ActionId {
        &self.action
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct ModeBindings {
    #[serde(default)]
    bindings: Vec<HotkeyBinding>,
}

impl ModeBindings {
    #[must_use]
    pub fn new(bindings: Vec<HotkeyBinding>) -> Self {
        Self { bindings }
    }

    #[must_use]
    pub fn bindings(&self) -> &[HotkeyBinding] {
        &self.bindings
    }
}

#[derive(Default)]
pub struct RegisteredConfig {
    hotkey_handles: Vec<Handle>,
    sequence_handles: Vec<SequenceHandle>,
    defined_modes: Vec<String>,
}

impl RegisteredConfig {
    #[must_use]
    pub fn hotkey_handles(&self) -> &[Handle] {
        &self.hotkey_handles
    }

    #[must_use]
    pub fn sequence_handles(&self) -> &[SequenceHandle] {
        &self.sequence_handles
    }

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

#[derive(Debug)]
pub enum ConfigRegistrationError {
    MissingAction { action: ActionId, location: String },
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
