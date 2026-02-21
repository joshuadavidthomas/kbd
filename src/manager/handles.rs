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

struct HandleInner {
    location: RegistrationLocation,
    registration_marker: Callback,
    manager: Arc<HotkeyManagerInner>,
}

impl Drop for HandleInner {
    fn drop(&mut self) {
        match &self.location {
            RegistrationLocation::Global(key) => {
                let _ = self.manager.remove_hotkey(key, &self.registration_marker);
            }
            RegistrationLocation::Device(id) => {
                self.manager
                    .remove_device_hotkey(*id, &self.registration_marker);
            }
        }
    }
}

/// Handle for a registered hotkey.
///
/// The hotkey remains active as long as the handle (or any clone) exists.
/// When the last clone is dropped, the hotkey is automatically unregistered.
///
/// Call [`Handle::unregister`] to explicitly remove the registration
/// immediately, regardless of how many clones exist.
#[derive(Clone)]
pub struct Handle {
    inner: Arc<HandleInner>,
}

impl Handle {
    pub(super) fn new(
        location: RegistrationLocation,
        registration_marker: Callback,
        manager: Arc<HotkeyManagerInner>,
    ) -> Self {
        Self {
            inner: Arc::new(HandleInner {
                location,
                registration_marker,
                manager,
            }),
        }
    }

    /// Remove this hotkey registration immediately. Future key presses will
    /// no longer trigger the callback.
    ///
    /// This unregisters even if other clones of this handle exist. Those
    /// clones become inert — their eventual drop is a no-op.
    pub fn unregister(self) -> Result<(), Error> {
        match &self.inner.location {
            RegistrationLocation::Global(key) => self
                .inner
                .manager
                .remove_hotkey(key, &self.inner.registration_marker),
            RegistrationLocation::Device(id) => {
                self.inner
                    .manager
                    .remove_device_hotkey(*id, &self.inner.registration_marker);
                Ok(())
            }
        }
    }
}

impl std::fmt::Debug for Handle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.inner.location {
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

struct SequenceHandleInner {
    id: SequenceId,
    manager: Arc<HotkeyManagerInner>,
}

impl Drop for SequenceHandleInner {
    fn drop(&mut self) {
        self.manager.remove_sequence(self.id);
    }
}

/// Handle for a registered key sequence.
///
/// The sequence remains active as long as the handle (or any clone) exists.
/// When the last clone is dropped, the sequence is automatically unregistered.
///
/// Call [`SequenceHandle::unregister`] to explicitly remove the registration
/// immediately, regardless of how many clones exist.
#[derive(Clone)]
pub struct SequenceHandle {
    inner: Arc<SequenceHandleInner>,
}

impl SequenceHandle {
    pub(super) fn new(id: SequenceId, manager: Arc<HotkeyManagerInner>) -> Self {
        Self {
            inner: Arc::new(SequenceHandleInner { id, manager }),
        }
    }

    /// The unique ID for this sequence registration.
    #[must_use]
    pub fn id(&self) -> SequenceId {
        self.inner.id
    }

    /// Remove this sequence registration immediately.
    ///
    /// This unregisters even if other clones of this handle exist. Those
    /// clones become inert — their eventual drop is a no-op.
    pub fn unregister(self) -> Result<(), Error> {
        self.inner.manager.remove_sequence(self.inner.id);
        Ok(())
    }
}

impl std::fmt::Debug for SequenceHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SequenceHandle")
            .field("id", &self.inner.id)
            .finish_non_exhaustive()
    }
}

struct TapHoldHandleInner {
    key: Key,
    registration_marker: Arc<()>,
    manager: Arc<HotkeyManagerInner>,
}

impl Drop for TapHoldHandleInner {
    fn drop(&mut self) {
        self.manager
            .remove_tap_hold(self.key, &self.registration_marker);
    }
}

/// Handle for a registered tap-hold key binding.
///
/// The tap-hold binding remains active as long as the handle (or any clone)
/// exists. When the last clone is dropped, the binding is automatically
/// unregistered.
///
/// Call [`TapHoldHandle::unregister`] to explicitly remove the registration
/// immediately, regardless of how many clones exist.
#[derive(Clone)]
pub struct TapHoldHandle {
    inner: Arc<TapHoldHandleInner>,
}

impl TapHoldHandle {
    pub(super) fn new(
        key: Key,
        registration_marker: Arc<()>,
        manager: Arc<HotkeyManagerInner>,
    ) -> Self {
        Self {
            inner: Arc::new(TapHoldHandleInner {
                key,
                registration_marker,
                manager,
            }),
        }
    }

    /// Remove this tap-hold registration immediately.
    ///
    /// This unregisters even if other clones of this handle exist. Those
    /// clones become inert — their eventual drop is a no-op.
    pub fn unregister(self) -> Result<(), Error> {
        self.inner
            .manager
            .remove_tap_hold(self.inner.key, &self.inner.registration_marker);
        Ok(())
    }
}

impl std::fmt::Debug for TapHoldHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TapHoldHandle")
            .field("key", &self.inner.key)
            .finish_non_exhaustive()
    }
}
