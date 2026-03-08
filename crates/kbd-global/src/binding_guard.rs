//! [`BindingGuard`] keeps a registered binding alive.
//!
//! The guard stores a binding ID and a command sender. Dropping it attempts to
//! unregister the binding once, giving the manager a simple RAII story for
//! hotkeys, sequences, and tap-hold registrations. Shutdown-related errors are
//! ignored during `Drop`; call [`BindingGuard::unregister`] if you need to
//! observe them explicitly.

use std::fmt;

use kbd::binding::BindingId;

use crate::engine::Command;
use crate::engine::CommandSender;
use crate::error::ManagerStopped;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HandleState {
    Active,
    Released,
}

/// Keeps a registered binding alive.
pub struct BindingGuard {
    id: BindingId,
    commands: CommandSender,
    state: HandleState,
}

impl fmt::Debug for BindingGuard {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BindingGuard")
            .field("id", &self.id)
            .field("state", &self.state)
            .finish_non_exhaustive()
    }
}

impl BindingGuard {
    pub(crate) fn new(id: BindingId, commands: CommandSender) -> Self {
        Self {
            id,
            commands,
            state: HandleState::Active,
        }
    }

    /// Returns the unique identifier for this guard's binding.
    #[must_use]
    pub const fn binding_id(&self) -> BindingId {
        self.id
    }

    /// Explicitly unregister this guard's binding.
    ///
    /// Sends an unregister command to the engine. This is a one-shot operation:
    /// after a successful call, dropping the guard does nothing.
    ///
    /// # Errors
    ///
    /// Returns [`ManagerStopped`] if the manager has already been shut down.
    pub fn unregister(mut self) -> Result<(), ManagerStopped> {
        self.unregister_inner()
    }

    fn unregister_inner(&mut self) -> Result<(), ManagerStopped> {
        match self.state {
            HandleState::Active => {
                self.state = HandleState::Released;
                self.commands.send(Command::Unregister { id: self.id })
            }
            HandleState::Released => Ok(()),
        }
    }
}

impl Drop for BindingGuard {
    fn drop(&mut self) {
        let _ = self.unregister_inner();
    }
}
