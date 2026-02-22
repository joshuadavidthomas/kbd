//! uinput virtual device for event forwarding and emission.
//!
//! In grab mode, unmatched key events are re-emitted through a virtual
//! device so they reach applications normally. Also used for `Action::EmitKey`
//! to produce synthetic key events.
//!
//! # Reference
//!
//! Prior art: `archive/v0/src/listener/forwarding.rs`,
//! `reference/keyd/src/vkbd/uinput.c`
//!
//! Note: keyd creates two virtual devices (keyboard + pointer). For now
//! we only need one (keyboard). Pointer device is a future stretch goal.

use crate::engine::key_state::KeyTransition;
use crate::Error;
use crate::Key;

/// Name of the virtual device we create, used for self-detection.
pub(crate) const VIRTUAL_DEVICE_NAME: &str = "keybound-virtual-keyboard";

/// Maximum key code we register with uinput.
const MAX_FORWARDABLE_KEY_CODE: u16 = 767;

/// Sink for forwarding key events through a virtual device.
///
/// The engine uses this trait to forward unmatched events (in grab mode)
/// and to emit synthetic key events (for remapping actions).
pub(crate) trait ForwardSink: Send {
    fn forward_key(&mut self, key: Key, transition: KeyTransition) -> Result<(), Error>;
}

/// Virtual uinput device for forwarding unmatched key events in grab mode.
///
/// Creates a virtual keyboard device via `/dev/uinput`. Unmatched events
/// are re-emitted through this device so they reach applications normally.
pub(crate) struct UinputForwarder {
    device: evdev::uinput::VirtualDevice,
}

impl UinputForwarder {
    pub(crate) fn new() -> Result<Self, Error> {
        let mut keys = evdev::AttributeSet::<evdev::KeyCode>::new();
        for code in 0..=MAX_FORWARDABLE_KEY_CODE {
            keys.insert(evdev::KeyCode::new(code));
        }

        let device = evdev::uinput::VirtualDevice::builder()
            .map_err(|_| Error::DeviceError)?
            .name(VIRTUAL_DEVICE_NAME)
            .with_keys(&keys)
            .map_err(|_| Error::DeviceError)?
            .build()
            .map_err(|_| Error::DeviceError)?;

        Ok(Self { device })
    }
}

impl ForwardSink for UinputForwarder {
    fn forward_key(&mut self, key: Key, transition: KeyTransition) -> Result<(), Error> {
        let key_code: evdev::KeyCode = key.into();
        let value = match transition {
            KeyTransition::Press => 1,
            KeyTransition::Release => 0,
            KeyTransition::Repeat => 2,
        };

        let event = evdev::InputEvent::new(evdev::EventType::KEY.0, key_code.code(), value);
        self.device.emit(&[event]).map_err(|_| Error::DeviceError)
    }
}

// TODO: emit_key() — produce a synthetic key event (for remapping/actions)

#[cfg(test)]
pub(super) mod testing {
    use std::sync::Arc;
    use std::sync::Mutex;

    use super::ForwardSink;
    use crate::engine::key_state::KeyTransition;
    use crate::Error;
    use crate::Key;

    /// Shared buffer for inspecting forwarded events in tests.
    pub(in crate::engine) type ForwardedEvents = Arc<Mutex<Vec<(Key, KeyTransition)>>>;

    /// A forwarder that records forwarded events for test assertions.
    pub(in crate::engine) struct RecordingForwarder {
        events: ForwardedEvents,
    }

    impl RecordingForwarder {
        /// Create a new recording forwarder and return the shared event buffer.
        pub(in crate::engine) fn new() -> (Self, ForwardedEvents) {
            let events = Arc::new(Mutex::new(Vec::new()));
            (Self { events: Arc::clone(&events) }, events)
        }
    }

    impl ForwardSink for RecordingForwarder {
        fn forward_key(&mut self, key: Key, transition: KeyTransition) -> Result<(), Error> {
            self.events.lock().unwrap().push((key, transition));
            Ok(())
        }
    }
}
