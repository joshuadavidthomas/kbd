//! [`Handle`] — RAII guard that keeps a binding alive.
//!
//! When dropped, sends `Command::Unregister` to the engine. No shared
//! state, no locks — just a binding ID and a command sender.
//!
//! One handle type for all binding kinds (replaces v0's `Handle`,
//! `SequenceHandle`, `TapHoldHandle`).
//!
//! # Reference
//!
//! Prior art: `archive/v0/src/manager/handles.rs`

use crate::binding::BindingId;
use crate::engine::Command;
use crate::engine::CommandSender;
use crate::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HandleState {
    Active,
    Released,
}

/// Keeps a registered binding alive.
pub struct Handle {
    id: BindingId,
    commands: CommandSender,
    state: HandleState,
}

impl Handle {
    pub(crate) fn new(id: BindingId, commands: CommandSender) -> Self {
        Self {
            id,
            commands,
            state: HandleState::Active,
        }
    }

    #[must_use]
    pub const fn binding_id(&self) -> BindingId {
        self.id
    }

    /// Explicitly unregister this handle's binding.
    ///
    /// The same unregistration is attempted automatically on drop.
    pub fn unregister(mut self) -> Result<(), Error> {
        self.unregister_inner()
    }

    fn unregister_inner(&mut self) -> Result<(), Error> {
        match self.state {
            HandleState::Active => {
                self.state = HandleState::Released;
                self.commands.send(Command::Unregister { id: self.id })
            }
            HandleState::Released => Ok(()),
        }
    }
}

impl Drop for Handle {
    fn drop(&mut self) {
        let _ = self.unregister_inner();
    }
}
