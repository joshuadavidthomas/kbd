use evdev::KeyCode;

#[cfg(feature = "grab")]
use super::state::MAX_FORWARDABLE_KEY_CODE;
#[cfg(feature = "grab")]
use super::state::VIRTUAL_FORWARDER_DEVICE_NAME;
use crate::error::Error;

pub(crate) trait KeyEventForwarder: Send {
    fn forward_key_event(&mut self, key: KeyCode, value: i32) -> Result<(), Error>;
}

#[cfg(feature = "grab")]
pub(crate) struct UinputForwarder {
    device: evdev::uinput::VirtualDevice,
}

#[cfg(feature = "grab")]
impl UinputForwarder {
    pub(crate) fn new() -> Result<Self, Error> {
        let mut keys = evdev::AttributeSet::<KeyCode>::new();
        for code in 0..=MAX_FORWARDABLE_KEY_CODE {
            keys.insert(KeyCode::new(code));
        }

        let device = evdev::uinput::VirtualDevice::builder()
            .map_err(|err| Error::DeviceAccess(format!("Failed to open /dev/uinput: {err}")))?
            .name(VIRTUAL_FORWARDER_DEVICE_NAME)
            .with_keys(&keys)
            .map_err(|err| Error::DeviceAccess(format!("Failed to configure uinput keys: {err}")))?
            .build()
            .map_err(|err| Error::DeviceAccess(format!("Failed to create uinput device: {err}")))?;

        Ok(Self { device })
    }
}

#[cfg(feature = "grab")]
impl KeyEventForwarder for UinputForwarder {
    fn forward_key_event(&mut self, key: KeyCode, value: i32) -> Result<(), Error> {
        let key_event = evdev::InputEvent::new(evdev::EventType::KEY.0, key.code(), value);
        self.device.emit(&[key_event]).map_err(|err| {
            Error::DeviceAccess(format!("Failed forwarding key event via uinput: {err}"))
        })
    }
}

pub(crate) fn create_key_event_forwarder(
    grab_enabled: bool,
) -> Result<Option<Box<dyn KeyEventForwarder>>, Error> {
    if !grab_enabled {
        return Ok(None);
    }

    #[cfg(feature = "grab")]
    {
        Ok(Some(Box::new(UinputForwarder::new()?)))
    }

    #[cfg(not(feature = "grab"))]
    {
        Err(Error::UnsupportedFeature(
            "event grabbing support is not compiled in (enable the `grab` feature)".to_string(),
        ))
    }
}
