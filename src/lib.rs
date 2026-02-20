pub use backend::Backend;
#[cfg(feature = "serde")]
pub use config::{
    ActionId, ActionIdError, ActionMap, ActionMapError, ConfigRegistrationError, HotkeyBinding,
    HotkeyConfig, ModeBindings, RegisteredConfig, SequenceBinding,
};
pub use device::DeviceFilter;
pub use error::Error;
pub use events::HotkeyEvent;
#[cfg(any(feature = "tokio", feature = "async-std"))]
pub use events::HotkeyEventStream;
pub use hotkey::{Hotkey, HotkeySequence, ParseHotkeyError};
pub use manager::{
    Handle, HotkeyManager, HotkeyManagerBuilder, HotkeyOptions, SequenceHandle, SequenceOptions,
    TapHoldHandle,
};
pub use mode::{ModeBuilder, ModeController, ModeOptions};
pub use tap_hold::{HoldAction, TapAction, TapHoldOptions};

mod backend;
#[cfg(feature = "serde")]
mod config;
mod device;
mod error;
mod events;
mod hotkey;
mod key_state;
mod listener;
mod manager;
mod mode;
mod permission;
mod tap_hold;
