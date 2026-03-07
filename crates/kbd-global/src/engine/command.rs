//! Command protocol for manager→engine communication.
//!
//! Every [`HotkeyManager`](crate::HotkeyManager) method translates to a
//! [`Command`] sent through the channel. Commands that need a response
//! include a one-shot reply sender.

use std::sync::Arc;
use std::sync::mpsc;

use kbd::binding::BindingId;
use kbd::binding::RegisteredBinding;
use kbd::hotkey::Hotkey;
use kbd::hotkey::HotkeySequence;
use kbd::hotkey::Modifiers;
use kbd::introspection::ActiveLayerInfo;
use kbd::introspection::BindingInfo;
use kbd::introspection::ConflictInfo;
use kbd::key::Key;
use kbd::layer::Layer;
use kbd::layer::LayerName;
use kbd::sequence::PendingSequenceInfo;
use kbd::sequence::SequenceOptions;

use super::wake::WakeFd;
use crate::Error;

pub(crate) enum Command {
    Register {
        binding: RegisteredBinding,
        reply: mpsc::Sender<Result<(), Error>>,
    },
    RegisterSequence {
        sequence: HotkeySequence,
        action: kbd::action::Action,
        options: SequenceOptions,
        reply: mpsc::Sender<Result<BindingId, Error>>,
    },
    PendingSequence {
        reply: mpsc::Sender<Option<PendingSequenceInfo>>,
    },
    Unregister {
        id: BindingId,
    },
    DefineLayer {
        layer: Layer,
        reply: mpsc::Sender<Result<(), Error>>,
    },
    PushLayer {
        name: LayerName,
        reply: mpsc::Sender<Result<(), Error>>,
    },
    PopLayer {
        reply: mpsc::Sender<Result<LayerName, Error>>,
    },
    ToggleLayer {
        name: LayerName,
        reply: mpsc::Sender<Result<(), Error>>,
    },
    IsRegistered {
        hotkey: Hotkey,
        reply: mpsc::Sender<bool>,
    },
    IsKeyPressed {
        key: Key,
        reply: mpsc::Sender<bool>,
    },
    ActiveModifiers {
        reply: mpsc::Sender<Modifiers>,
    },
    ListBindings {
        reply: mpsc::Sender<Vec<BindingInfo>>,
    },
    BindingsForKey {
        hotkey: Hotkey,
        reply: mpsc::Sender<Option<BindingInfo>>,
    },
    ActiveLayers {
        reply: mpsc::Sender<Vec<ActiveLayerInfo>>,
    },
    Conflicts {
        reply: mpsc::Sender<Vec<ConflictInfo>>,
    },
    Shutdown,
}

#[derive(Clone)]
pub(crate) struct CommandSender {
    command_tx: mpsc::Sender<Command>,
    wake_fd: Arc<WakeFd>,
}

impl CommandSender {
    pub(super) fn new(command_tx: mpsc::Sender<Command>, wake_fd: Arc<WakeFd>) -> Self {
        Self {
            command_tx,
            wake_fd,
        }
    }

    pub(crate) fn send(&self, command: Command) -> Result<(), Error> {
        self.command_tx
            .send(command)
            .map_err(|_| Error::ManagerStopped)?;
        self.wake_fd.wake().map_err(|_| Error::ManagerStopped)?;
        Ok(())
    }
}
