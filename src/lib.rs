pub use backend::Backend;
pub use error::Error;
pub use hotkey::{Hotkey, HotkeySequence, ParseHotkeyError};
pub use manager::{Handle, HotkeyManager, HotkeyOptions, SequenceHandle, SequenceOptions};

mod backend;
mod device;
mod error;
mod hotkey;
mod listener;
mod manager;
mod permission;
