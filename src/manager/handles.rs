use std::sync::Arc;

use crate::error::Error;
use crate::key::Key;

use super::callbacks::Callback;
use super::registration::DeviceRegistrationId;
use super::registration::HotkeyKey;
use super::registration::SequenceId;
use super::HotkeyManagerInner;

#[derive(Clone)]
pub(crate) enum RegistrationLocation {
    Global(HotkeyKey),
    Device(DeviceRegistrationId),
}

/// Handle for unregistering a specific hotkey
#[derive(Clone)]
pub struct Handle {
    pub(super) location: RegistrationLocation,
    pub(super) registration_marker: Callback,
    pub(super) manager: Arc<HotkeyManagerInner>,
}

impl std::fmt::Debug for Handle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.location {
            RegistrationLocation::Global(key) => f
                .debug_struct("Handle")
                .field("key", key)
                .finish_non_exhaustive(),
            RegistrationLocation::Device(id) => f
                .debug_struct("Handle")
                .field("device_registration_id", id)
                .finish_non_exhaustive(),
        }
    }
}

impl Handle {
    pub fn unregister(self) -> Result<(), Error> {
        match &self.location {
            RegistrationLocation::Global(key) => {
                self.manager.remove_hotkey(key, &self.registration_marker)
            }
            RegistrationLocation::Device(id) => {
                self.manager
                    .remove_device_hotkey(*id, &self.registration_marker);
                Ok(())
            }
        }
    }
}

#[derive(Clone)]
pub struct SequenceHandle {
    pub(super) id: SequenceId,
    pub(super) manager: Arc<HotkeyManagerInner>,
}

impl std::fmt::Debug for SequenceHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SequenceHandle")
            .field("id", &self.id)
            .finish_non_exhaustive()
    }
}

impl SequenceHandle {
    pub fn unregister(self) -> Result<(), Error> {
        self.manager.remove_sequence(self.id);
        Ok(())
    }
}

/// Handle for unregistering a tap-hold key binding.
#[derive(Clone)]
pub struct TapHoldHandle {
    pub(super) key: Key,
    pub(super) registration_marker: Arc<()>,
    pub(super) manager: Arc<HotkeyManagerInner>,
}

impl std::fmt::Debug for TapHoldHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TapHoldHandle")
            .field("key", &self.key)
            .finish_non_exhaustive()
    }
}

impl TapHoldHandle {
    pub fn unregister(self) -> Result<(), Error> {
        self.manager
            .remove_tap_hold(self.key, &self.registration_marker);
        Ok(())
    }
}
