//! Re-exports forwarder types from [`kbd_evdev::forwarder`].

pub(crate) use kbd_evdev::forwarder::ForwardSink;
#[cfg(feature = "grab")]
pub(crate) use kbd_evdev::forwarder::UinputForwarder;

#[cfg(test)]
pub(super) mod testing {
    pub(in crate::engine) use kbd_evdev::forwarder::testing::ForwardedEvents;
    pub(in crate::engine) use kbd_evdev::forwarder::testing::RecordingForwarder;
}
