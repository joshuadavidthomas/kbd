use crate::hotkey::Hotkey;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HotkeyEvent {
    Pressed(Hotkey),
    Released(Hotkey),
    SequenceStep { id: u64, step: usize, total: usize },
    ModeChanged(Option<String>),
}

#[cfg(any(feature = "tokio", feature = "async-std"))]
#[derive(Clone, Default)]
pub(crate) struct EventHub {
    state: std::sync::Arc<std::sync::Mutex<EventHubState>>,
}

#[cfg(any(feature = "tokio", feature = "async-std"))]
#[derive(Default)]
struct EventHubState {
    subscribers: Vec<async_channel::Sender<HotkeyEvent>>,
    closed: bool,
}

#[cfg(not(any(feature = "tokio", feature = "async-std")))]
#[derive(Clone, Default)]
pub(crate) struct EventHub;

impl EventHub {
    pub(crate) fn new() -> Self {
        #[cfg(any(feature = "tokio", feature = "async-std"))]
        {
            Self::default()
        }

        #[cfg(not(any(feature = "tokio", feature = "async-std")))]
        {
            Self
        }
    }

    #[cfg(any(feature = "tokio", feature = "async-std"))]
    pub(crate) fn subscribe(&self) -> HotkeyEventStream {
        let (tx, rx) = async_channel::unbounded();

        let mut state = self.state.lock().unwrap();
        if state.closed {
            drop(tx);
        } else {
            state.subscribers.push(tx);
        }

        HotkeyEventStream { receiver: rx }
    }

    pub(crate) fn emit(&self, event: &HotkeyEvent) {
        #[cfg(any(feature = "tokio", feature = "async-std"))]
        {
            let mut state = self.state.lock().unwrap();
            if state.closed {
                return;
            }
            state
                .subscribers
                .retain(|sender| sender.try_send(event.clone()).is_ok());
        }

        #[cfg(not(any(feature = "tokio", feature = "async-std")))]
        {
            let _ = event;
        }
    }

    pub(crate) fn close(&self) {
        #[cfg(any(feature = "tokio", feature = "async-std"))]
        {
            let mut state = self.state.lock().unwrap();
            state.closed = true;
            state.subscribers.clear();
        }
    }
}

#[cfg(any(feature = "tokio", feature = "async-std"))]
pub struct HotkeyEventStream {
    receiver: async_channel::Receiver<HotkeyEvent>,
}

#[cfg(any(feature = "tokio", feature = "async-std"))]
impl HotkeyEventStream {
    pub async fn next(&mut self) -> Option<HotkeyEvent> {
        self.receiver.recv().await.ok()
    }

    pub fn try_next(&mut self) -> Option<HotkeyEvent> {
        self.receiver.try_recv().ok()
    }
}
