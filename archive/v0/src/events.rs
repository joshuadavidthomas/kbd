use crate::hotkey::Hotkey;

/// Events emitted by the hotkey system.
///
/// Subscribe via [`HotkeyEventStream`] (requires the `tokio` or `async-std`
/// feature).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HotkeyEvent {
    /// A registered hotkey was pressed.
    Pressed(Hotkey),
    /// A registered hotkey was released.
    Released(Hotkey),
    /// A step in a key sequence was completed.
    ///
    /// `step` is 1-indexed and increments toward `total`. When
    /// `step == total`, the sequence callback has been invoked.
    SequenceStep { id: u64, step: usize, total: usize },
    /// The active mode changed. `None` means no mode is active.
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
    // SMELL: bool field -- this definitely looks like an enum opportunity
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

/// Async stream of [`HotkeyEvent`]s.
///
/// Obtain one from [`HotkeyManager::event_stream`](crate::HotkeyManager::event_stream).
/// Each stream is an independent subscriber — multiple streams each receive
/// every event.
///
/// Requires the `tokio` or `async-std` feature.
#[cfg(any(feature = "tokio", feature = "async-std"))]
pub struct HotkeyEventStream {
    receiver: async_channel::Receiver<HotkeyEvent>,
}

#[cfg(any(feature = "tokio", feature = "async-std"))]
impl HotkeyEventStream {
    /// Wait for the next event. Returns `None` when the manager is stopped.
    pub async fn next(&mut self) -> Option<HotkeyEvent> {
        self.receiver.recv().await.ok()
    }

    /// Non-blocking poll. Returns `None` if no event is available.
    pub fn try_next(&mut self) -> Option<HotkeyEvent> {
        self.receiver.try_recv().ok()
    }
}

// SMELL: no tests?
