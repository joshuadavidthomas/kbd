use std::sync::Arc;
use std::sync::mpsc;
use std::thread;

use super::Engine;
use super::command::Command;
use super::command::CommandSender;
use super::run;
use super::types::GrabState;
use super::wake::WakeFd;
use crate::Error;

pub(crate) struct EngineRuntime {
    commands: CommandSender,
    join_handle: thread::JoinHandle<Result<(), Error>>,
}

impl EngineRuntime {
    pub(crate) fn spawn(grab_state: GrabState) -> Result<Self, Error> {
        let wake_fd = Arc::new(WakeFd::new()?);
        let (command_tx, command_rx) = mpsc::channel();
        let commands = CommandSender::new(command_tx, Arc::clone(&wake_fd));

        let engine = Engine::new(command_rx, wake_fd, grab_state);
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

    pub(crate) fn shutdown(self) -> Result<(), Error> {
        let send_result = self.commands.send(Command::Shutdown);
        let join_result = self.join();

        match (send_result, join_result) {
            (Ok(()), Ok(())) => Ok(()),
            (Err(error), Ok(())) | (_, Err(error)) => Err(error),
        }
    }

    pub(crate) fn join(self) -> Result<(), Error> {
        self.join_handle.join().map_err(|_| Error::EngineError)?
    }
}
