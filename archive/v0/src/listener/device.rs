use std::collections::HashMap;
use std::collections::HashSet;
use std::os::unix::io::AsRawFd;
use std::path::Path;
use std::path::PathBuf;

use evdev::Device;
use evdev::KeyCode;

use crate::device::DeviceInfo;
use crate::key::Key;
use crate::key::Modifier;
use crate::manager::ActiveHotkeyPress;

pub(crate) struct DeviceState {
    pub(crate) path: PathBuf,
    pub(crate) info: DeviceInfo,
    pub(crate) device: Device,
    pub(crate) active_presses: HashMap<Key, ActiveHotkeyPress>,
    pub(crate) pressed_keys: HashSet<KeyCode>,
}

impl DeviceState {
    pub(crate) fn fd(&self) -> i32 {
        self.device.as_raw_fd()
    }
}

#[derive(Default)]
pub(crate) struct ModifierTracker {
    pressed_modifiers: HashMap<PathBuf, HashSet<Modifier>>,
}

impl ModifierTracker {
    pub(crate) fn press(&mut self, device_path: &Path, modifier: Modifier) {
        self.pressed_modifiers
            .entry(device_path.to_path_buf())
            .or_default()
            .insert(modifier);
    }

    pub(crate) fn release(&mut self, device_path: &Path, modifier: Modifier) {
        if let Some(modifiers) = self.pressed_modifiers.get_mut(device_path) {
            modifiers.remove(&modifier);
            if modifiers.is_empty() {
                self.pressed_modifiers.remove(device_path);
            }
        }
    }

    pub(crate) fn disconnect(&mut self, device_path: &Path) {
        self.pressed_modifiers.remove(device_path);
    }

    pub(crate) fn active_modifiers(&self) -> HashSet<Modifier> {
        self.pressed_modifiers
            .values()
            .flat_map(|mods| mods.iter().copied())
            .collect()
    }

    pub(crate) fn device_modifiers(&self, device_path: &Path) -> HashSet<Modifier> {
        self.pressed_modifiers
            .get(device_path)
            .cloned()
            .unwrap_or_default()
    }
}
