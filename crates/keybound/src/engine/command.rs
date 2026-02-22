use std::sync::Arc;
use std::sync::mpsc;

use super::binding::RegisteredBinding;
use super::wake::WakeFd;
use crate::Error;
use crate::Key;
use crate::Modifier;
use crate::action::LayerName;
use crate::binding::BindingId;
use crate::introspection::ActiveLayerInfo;
use crate::introspection::BindingInfo;
use crate::introspection::ConflictInfo;
use crate::key::Hotkey;
use crate::layer::Layer;

pub(crate) enum Command {
    Register {
        binding: RegisteredBinding,
        reply: mpsc::Sender<Result<(), Error>>,
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
        reply: mpsc::Sender<Vec<Modifier>>,
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
