use std::sync::Arc;
use std::sync::mpsc;

use kbd_core::Key;
use kbd_core::Modifier;
use kbd_core::action::LayerName;
use kbd_core::binding::BindingId;
use kbd_core::binding::RegisteredBinding;
use kbd_core::introspection::ActiveLayerInfo;
use kbd_core::introspection::BindingInfo;
use kbd_core::introspection::ConflictInfo;
use kbd_core::key::Hotkey;
use kbd_core::layer::Layer;

use super::wake::WakeFd;
use crate::Error;

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
