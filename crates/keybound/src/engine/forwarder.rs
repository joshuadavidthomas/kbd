//! Re-exports forwarder types from [`kbd_evdev::forwarder`].

#[allow(unused_imports)]
pub(crate) use kbd_evdev::forwarder::ForwardSink;
#[allow(unused_imports)]
pub(crate) use kbd_evdev::forwarder::UinputForwarder;
#[allow(unused_imports)]
pub(crate) use kbd_evdev::forwarder::VIRTUAL_DEVICE_NAME;

#[cfg(test)]
pub(super) mod testing {
    pub(in crate::engine) use kbd_evdev::forwarder::testing::ForwardedEvents;
    pub(in crate::engine) use kbd_evdev::forwarder::testing::RecordingForwarder;
}
