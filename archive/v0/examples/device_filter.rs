//! Device-specific hotkeys — bind to specific keyboards or macro pads.
//!
//! You can restrict hotkeys to events from a specific device, identified
//! by name substring or USB vendor/product ID. Perfect for binding macro
//! pads, secondary keyboards, or specific hardware.
//!
//! **Evdev backend only** — portal does not expose per-device info.
//!
//! ```sh
//! cargo run --example device_filter
//! ```

use keybound::DeviceFilter;
use keybound::HotkeyManager;
use keybound::HotkeyOptions;
use keybound::Key;
use keybound::Modifier;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Device-specific hotkeys example");
    println!();

    let manager = HotkeyManager::new()?;

    // Filter by device name substring.
    // Matches any device whose name contains "StreamDeck" (case-sensitive).
    let _macro_pad = manager.register_with_options(
        Key::F13,
        &[],
        HotkeyOptions::new().device(DeviceFilter::name_contains("StreamDeck")),
        || println!("[StreamDeck] F13 pressed on macro pad"),
    )?;
    println!("  F13 (StreamDeck only) → macro pad action");

    // Filter by USB vendor/product ID.
    // Useful when the device name is generic but the USB ID is known.
    // (0x1234, 0x5678 is a placeholder — replace with your device's IDs)
    let _usb_device = manager.register_with_options(
        Key::F14,
        &[],
        HotkeyOptions::new().device(DeviceFilter::usb(0x1234, 0x5678)),
        || println!("[USB 1234:5678] F14 pressed on specific device"),
    )?;
    println!("  F14 (USB 1234:5678)   → USB device action");

    // Same key, different devices — independent callbacks
    let _laptop = manager.register_with_options(
        Key::F1,
        &[Modifier::Ctrl],
        HotkeyOptions::new().device(DeviceFilter::name_contains("AT Translated")),
        || println!("[laptop keyboard] Ctrl+F1"),
    )?;

    let _external = manager.register_with_options(
        Key::F1,
        &[Modifier::Ctrl],
        HotkeyOptions::new().device(DeviceFilter::name_contains("USB Keyboard")),
        || println!("[external keyboard] Ctrl+F1"),
    )?;
    println!("  Ctrl+F1 → different callback per keyboard");

    // A global hotkey still works alongside device-specific ones
    let _global = manager.register(Key::F15, &[], || {
        println!("[global] F15 pressed on any device");
    })?;
    println!("  F15 → global (any device)");

    println!();
    println!("Tip: check /proc/bus/input/devices or `evtest` for your device names and IDs");
    println!("Press Ctrl+C to exit");

    std::thread::park();
    Ok(())
}
