use std::sync::Arc;

use super::callbacks::Callback;
use super::registration::DeviceRegistrationId;
use super::registration::HotkeyKey;
use super::registration::SequenceId;
use super::HotkeyManagerInner;
use crate::error::Error;
use crate::key::Key;

#[derive(Clone)]
pub(crate) enum RegistrationLocation {
    Global(HotkeyKey),
    Device(DeviceRegistrationId),
}

/// Handle for a registered hotkey.
///
/// The hotkey remains active as long as the handle (or any clone) exists.
/// Call [`Handle::unregister`] to explicitly remove it, or simply drop the
/// handle if you want the binding to live for the lifetime of the manager.
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
    /// Remove this hotkey registration. Future key presses will no longer
    /// trigger the callback.
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

/// Handle for a registered key sequence.
///
/// See [`Handle`] for lifetime semantics.
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
    /// Remove this sequence registration.
    pub fn unregister(self) -> Result<(), Error> {
        self.manager.remove_sequence(self.id);
        Ok(())
    }
}

/// Handle for a registered tap-hold key binding.
///
/// See [`Handle`] for lifetime semantics.
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
    /// Remove this tap-hold registration.
    pub fn unregister(self) -> Result<(), Error> {
        self.manager
            .remove_tap_hold(self.key, &self.registration_marker);
        Ok(())
    }
}
