pub use backend::Backend;
pub use error::Error;
pub use hotkey::{Hotkey, HotkeySequence, ParseHotkeyError};
pub use manager::{
    Handle, HotkeyManager, HotkeyManagerBuilder, HotkeyOptions, SequenceHandle, SequenceOptions,
};
pub use mode::{ModeBuilder, ModeController, ModeOptions};

mod backend;
mod device;
mod error;
mod hotkey;
mod listener;
mod manager;
mod mode;
mod permission;
