//! Engine thread lifecycle — spawn, shutdown, and join.
//!
//! [`EngineRuntime`] owns the thread handle and command sender. Created
//! by [`HotkeyManager`](crate::manager::HotkeyManager) during construction.

use std::path::Path;
use std::sync::Arc;
use std::sync::mpsc;
use std::thread;

use super::Engine;
use super::command::Command;
use super::command::CommandSender;
use super::run;
use super::types::GrabState;
use super::wake::WakeFd;
use crate::error::ShutdownError;
use crate::error::StartupError;

pub(crate) struct EngineRuntime {
    commands: CommandSender,
    join_handle: thread::JoinHandle<Result<(), ShutdownError>>,
}

impl EngineRuntime {
    pub(crate) fn spawn(grab_state: GrabState) -> Result<Self, StartupError> {
        Self::spawn_with_input_dir(grab_state, Path::new(super::devices::INPUT_DIRECTORY))
    }

    pub(crate) fn spawn_with_input_dir(
        grab_state: GrabState,
        input_directory: &Path,
    ) -> Result<Self, StartupError> {
        let wake_fd = Arc::new(WakeFd::new()?);
        let (command_tx, command_rx) = mpsc::channel();
        let commands = CommandSender::new(command_tx, Arc::clone(&wake_fd));

        let engine = Engine::new_with_input_dir(command_rx, wake_fd, grab_state, input_directory);
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

    pub(crate) fn shutdown(self) -> Result<(), ShutdownError> {
        let send_result = self.commands.send(Command::Shutdown);
        let join_result = self.join();

        match (send_result, join_result) {
            (Ok(()), Ok(())) => Ok(()),
            (Err(stopped), Ok(())) => Err(stopped.into()),
            (_, Err(error)) => Err(error),
        }
    }

    pub(crate) fn join(self) -> Result<(), ShutdownError> {
        self.join_handle.join().map_err(|_| ShutdownError::Engine)?
    }
}
