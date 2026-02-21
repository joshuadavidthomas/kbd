use std::collections::HashMap;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::sync::Mutex;

use crate::key::Key;
use crate::key_state::SharedKeyState;
use crate::manager::DeviceHotkeyRegistration;
use crate::manager::DeviceRegistrationId;
use crate::manager::HotkeyKey;
use crate::manager::HotkeyRegistration;
use crate::manager::SequenceId;
use crate::manager::SequenceRegistration;
use crate::mode::ModeRegistry;

// SMELL: awful thin module? hard to see why and what's going on here

pub(crate) struct ListenerState {
    pub(crate) registrations: Arc<Mutex<HashMap<HotkeyKey, HotkeyRegistration>>>,
    pub(crate) sequence_registrations: Arc<Mutex<HashMap<SequenceId, SequenceRegistration>>>,
    pub(crate) device_registrations:
        Arc<Mutex<HashMap<DeviceRegistrationId, DeviceHotkeyRegistration>>>,
    pub(crate) tap_hold_registrations:
        Arc<Mutex<HashMap<Key, crate::tap_hold::TapHoldRegistration>>>,
    pub(crate) stop_flag: Arc<AtomicBool>,
    pub(crate) key_state: SharedKeyState,
    pub(crate) mode_registry: ModeRegistry,
}

pub(crate) const POLL_TIMEOUT_MS: i32 = 25;
pub(crate) const INOTIFY_BUFFER_SIZE: usize = 4096;
pub(crate) const VIRTUAL_FORWARDER_DEVICE_NAME: &str = "keybound-virtual-keyboard";
#[cfg(feature = "grab")]
pub(crate) const MAX_FORWARDABLE_KEY_CODE: u16 = 767;

#[derive(Clone, Copy, Default)]
pub(crate) struct ListenerConfig {
    pub(crate) grab: bool,
}
