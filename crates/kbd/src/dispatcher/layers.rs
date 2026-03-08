use std::time::Duration;
use std::time::Instant;

use super::Dispatcher;
use crate::action::Action;
use crate::layer::LayerName;

/// An entry in the layer stack, pairing the layer name with runtime state.
pub(super) struct LayerStackEntry {
    pub(super) name: LayerName,
    /// Remaining keypress count for oneshot layers. `None` means not oneshot.
    pub(super) oneshot_remaining: Option<usize>,
    /// Timeout configuration and last activity timestamp.
    /// If set, the layer auto-pops when `Instant::now() - last_activity > timeout`.
    pub(super) timeout: Option<LayerTimeout>,
}

pub(super) struct LayerTimeout {
    pub(super) duration: Duration,
    pub(super) last_activity: Instant,
}

/// Layer stack mutation extracted from a matched action.
pub(super) enum LayerEffect {
    None,
    Push(LayerName),
    Pop,
    Toggle(LayerName),
}

impl LayerEffect {
    pub(super) fn from_action(action: &Action) -> Self {
        match action {
            Action::PushLayer(name) => Self::Push(name.clone()),
            Action::PopLayer => Self::Pop,
            Action::ToggleLayer(name) => Self::Toggle(name.clone()),
            Action::Callback(_)
            | Action::EmitHotkey(..)
            | Action::EmitSequence(..)
            | Action::Suppress => Self::None,
        }
    }
}

impl Dispatcher {
    /// Push a named layer onto the stack, activating its bindings.
    ///
    /// # Errors
    ///
    /// Returns [`LayerError::NotDefined`](crate::error::LayerError::NotDefined)
    /// if no layer with this name is defined.
    pub fn push_layer(
        &mut self,
        name: impl Into<LayerName>,
    ) -> Result<(), crate::error::LayerError> {
        let name = name.into();
        let stored = self
            .layers
            .get(&name)
            .ok_or(crate::error::LayerError::NotDefined)?;
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

    /// Pop the topmost layer from the stack.
    ///
    /// # Errors
    ///
    /// Returns [`LayerError::EmptyStack`](crate::error::LayerError::EmptyStack)
    /// if no layers are on the stack.
    pub fn pop_layer(&mut self) -> Result<LayerName, crate::error::LayerError> {
        let name = self
            .layer_stack
            .pop()
            .map(|entry| entry.name)
            .ok_or(crate::error::LayerError::EmptyStack)?;
        self.clear_sequences_for_layer_if_inactive(&name);
        Ok(name)
    }

    /// Toggle a layer: push if not active, remove if active.
    ///
    /// # Errors
    ///
    /// Returns [`LayerError::NotDefined`](crate::error::LayerError::NotDefined)
    /// if no layer with this name is defined.
    pub fn toggle_layer(
        &mut self,
        name: impl Into<LayerName>,
    ) -> Result<(), crate::error::LayerError> {
        let name = name.into();
        if !self.layers.contains_key(&name) {
            return Err(crate::error::LayerError::NotDefined);
        }
        if let Some(pos) = self
            .layer_stack
            .iter()
            .rposition(|entry| entry.name == name)
        {
            let removed = self.layer_stack.remove(pos);
            self.clear_sequences_for_layer_if_inactive(&removed.name);
        } else {
            self.push_layer(name)?;
        }
        Ok(())
    }

    /// Apply a layer effect extracted from a matched action.
    pub(super) fn apply_layer_effect(&mut self, effect: &LayerEffect) {
        match effect {
            LayerEffect::None => {}
            LayerEffect::Push(name) => {
                let _ = self.push_layer(name.clone());
            }
            LayerEffect::Pop => {
                let _ = self.pop_layer();
            }
            LayerEffect::Toggle(name) => {
                let _ = self.toggle_layer(name.clone());
            }
        }
    }
}
