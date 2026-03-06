//! Event stream types for observing hotkey activity.

use async_channel::Receiver;
use kbd::sequence::SequenceStepInfo;

pub(crate) const EVENT_STREAM_BUFFER_CAPACITY: usize = 64;

/// Events emitted by [`manager::HotkeyManager`](crate::manager::HotkeyManager)'s event stream.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum HotkeyEvent {
    /// A sequence advanced by one matched step.
    SequenceStep {
        /// The binding whose sequence advanced.
        binding_id: kbd::binding::BindingId,
        /// The hotkey that matched this step.
        hotkey: kbd::hotkey::Hotkey,
        /// Number of steps matched so far, including this step.
        steps_matched: usize,
        /// Number of steps remaining after this step.
        steps_remaining: usize,
    },
}

impl From<SequenceStepInfo> for HotkeyEvent {
    fn from(step: SequenceStepInfo) -> Self {
        Self::SequenceStep {
            binding_id: step.binding_id,
            hotkey: step.hotkey,
            steps_matched: step.steps_matched,
            steps_remaining: step.steps_remaining,
        }
    }
}

/// Async-capable receiver for [`HotkeyEvent`] values.
///
/// Streams use a bounded internal buffer. If a receiver stops draining events
/// and its buffer fills, the engine drops that subscriber so hotkey handling
/// never accumulates unbounded memory.
#[derive(Debug)]
pub struct HotkeyEventStream {
    receiver: Receiver<HotkeyEvent>,
}

impl HotkeyEventStream {
    pub(crate) fn new(receiver: Receiver<HotkeyEvent>) -> Self {
        Self { receiver }
    }

    /// Receive the next event asynchronously.
    ///
    /// # Errors
    ///
    /// Returns [`async_channel::RecvError`] when the manager has shut down and
    /// no more events remain.
    pub async fn recv(&self) -> Result<HotkeyEvent, async_channel::RecvError> {
        self.receiver.recv().await
    }

    /// Receive the next event from synchronous code.
    ///
    /// # Errors
    ///
    /// Returns [`async_channel::RecvError`] when the manager has shut down and
    /// no more events remain.
    pub fn recv_blocking(&self) -> Result<HotkeyEvent, async_channel::RecvError> {
        self.receiver.recv_blocking()
    }

    /// Try to receive the next event without blocking.
    ///
    /// # Errors
    ///
    /// Returns [`async_channel::TryRecvError`] when the stream is empty or
    /// closed.
    pub fn try_recv(&self) -> Result<HotkeyEvent, async_channel::TryRecvError> {
        self.receiver.try_recv()
    }
}
