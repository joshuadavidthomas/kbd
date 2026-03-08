//! uinput virtual device for event forwarding and emission.
//!
//! In grab mode, unmatched key events are re-emitted through a virtual
//! device so they reach applications normally.
//!
//! Note: keyd creates two virtual devices (keyboard + pointer). For now
//! we only need one (keyboard).

use kbd::key::Key;
use kbd::key_state::KeyTransition;

use crate::convert::KbdKeyExt;
use crate::error::Error;

/// Name of the virtual device we create, used for self-detection.
pub(crate) const VIRTUAL_DEVICE_NAME: &str = "kbd-virtual-keyboard";

/// Sink for forwarding key events through a virtual device.
///
/// The engine uses this trait to forward unmatched events (in grab mode)
/// and to emit synthetic key events (for remapping actions).
pub trait ForwardSink: Send {
    /// Forward a single key event through the virtual device.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Uinput`] if the underlying `write` to the virtual
    /// device fails.
    fn forward_key(&mut self, key: Key, transition: KeyTransition) -> Result<(), Error>;
}

/// Maximum key code we register with uinput.
const MAX_FORWARDABLE_KEY_CODE: u16 = 767;

/// Virtual uinput device for forwarding unmatched key events in grab mode.
///
/// Creates a virtual keyboard device via `/dev/uinput`. Unmatched events
/// are re-emitted through this device so they reach applications normally.
pub struct UinputForwarder {
    device: evdev::uinput::VirtualDevice,
}

impl UinputForwarder {
    /// Create a new virtual keyboard device via `/dev/uinput`.
    ///
    /// The device is named `kbd-virtual-keyboard` and supports all key
    /// codes up to code 767. [`DeviceManager`](crate::devices::DeviceManager)
    /// automatically skips this device during discovery to prevent
    /// feedback loops.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Uinput`] if `/dev/uinput` cannot be opened or
    /// the virtual device cannot be created (e.g., missing permissions).
    pub fn new() -> Result<Self, Error> {
        let mut keys = evdev::AttributeSet::<evdev::KeyCode>::new();
        for code in 0..=MAX_FORWARDABLE_KEY_CODE {
            keys.insert(evdev::KeyCode::new(code));
        }

        let device = evdev::uinput::VirtualDevice::builder()
            .map_err(Error::Uinput)?
            .name(VIRTUAL_DEVICE_NAME)
            .with_keys(&keys)
            .map_err(Error::Uinput)?
            .build()
            .map_err(Error::Uinput)?;

        Ok(Self { device })
    }
}

impl ForwardSink for UinputForwarder {
    fn forward_key(&mut self, key: Key, transition: KeyTransition) -> Result<(), Error> {
        let key_code = key.to_key_code();
        let value = match transition {
            KeyTransition::Press => 1,
            KeyTransition::Release => 0,
            KeyTransition::Repeat => 2,
            _ => return Ok(()),
        };

        let event = evdev::InputEvent::new(evdev::EventType::KEY.0, key_code.code(), value);
        self.device.emit(&[event]).map_err(Error::Uinput)
    }
}

// TODO: emit_key() — produce a synthetic key event (for remapping/actions)

/// Test utilities for the forwarder — recording forwarder for assertions.
///
/// Gated behind the `testing` feature flag. Enable it in downstream
/// `[dev-dependencies]` to use `RecordingForwarder` in your own tests.
#[cfg(any(test, feature = "testing"))]
pub mod testing {
    use std::sync::Arc;
    use std::sync::Mutex;

    use kbd::key::Key;
    use kbd::key_state::KeyTransition;

    use super::ForwardSink;
    use crate::error::Error;

    /// Shared buffer for inspecting forwarded events in tests.
    pub type ForwardedEvents = Arc<Mutex<Vec<(Key, KeyTransition)>>>;

    /// A forwarder that records forwarded events for test assertions.
    pub struct RecordingForwarder {
        events: ForwardedEvents,
    }

    impl RecordingForwarder {
        /// Create a new recording forwarder and return the shared event buffer.
        #[must_use]
        pub fn new() -> (Self, ForwardedEvents) {
            let events = Arc::new(Mutex::new(Vec::new()));
            (
                Self {
                    events: Arc::clone(&events),
                },
                events,
            )
        }
    }

    impl ForwardSink for RecordingForwarder {
        fn forward_key(&mut self, key: Key, transition: KeyTransition) -> Result<(), Error> {
            self.events.lock().unwrap().push((key, transition));
            Ok(())
        }
    }
}
