# evdev-hotkey

Global hotkey listener for Linux using evdev. Works on both X11 and Wayland.

## Features

- **Cross-desktop**: Works on X11 and Wayland by reading directly from `/dev/input`
- **Multiple hotkeys**: Register any number of global shortcuts
- **Simple API**: Type-safe with `evdev::KeyCode` and optional string parsing for config-driven shortcuts
- **Automatic device discovery**: No need to manually glob for keyboard devices
- **Permission checking**: Helpful error messages guide users through setup
- **Lightweight dependencies**: Uses `evdev`, `libc`, and `tracing`

## Requirements

Your user must be allowed to read `/dev/input/event*` devices. On many systems this means being in the `input` group:

```bash
sudo usermod -aG input $USER
# Then log out and log back in
```

## Installation

```toml
[dependencies]
evdev-hotkey = "0.1"
evdev = "0.13"
```

## Usage

### Basic Example

```rust
use evdev::KeyCode;
use evdev_hotkey::HotkeyManager;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let manager = HotkeyManager::new()?;

    manager.register(
        KeyCode::KEY_C,
        &[KeyCode::KEY_LEFTCTRL, KeyCode::KEY_LEFTSHIFT],
        || {
            println!("Hotkey triggered!");
        }
    )?;

    // Keep program running
    std::thread::sleep(std::time::Duration::from_secs(60));

    Ok(())
}
```

### Multiple Hotkeys

```rust
use evdev::KeyCode;

let manager = HotkeyManager::new()?;

let _handle1 = manager.register(
    KeyCode::KEY_A,
    &[KeyCode::KEY_LEFTCTRL, KeyCode::KEY_LEFTSHIFT],
    || {
        println!("Action A!");
    }
)?;

let _handle2 = manager.register(
    KeyCode::KEY_B,
    &[KeyCode::KEY_LEFTCTRL, KeyCode::KEY_LEFTSHIFT],
    || {
        println!("Action B!");
    }
)?;
```

### Unregister Hotkeys

```rust
let handle = manager.register(
    KeyCode::KEY_C,
    &[KeyCode::KEY_LEFTCTRL, KeyCode::KEY_LEFTSHIFT],
    || {
        println!("Triggered!");
    }
)?;

// Later
handle.unregister()?;
```

### Press / Release and Hold Options

```rust
use evdev::KeyCode;
use evdev_hotkey::{HotkeyManager, HotkeyOptions};
use std::time::Duration;

let manager = HotkeyManager::new()?;

let _handle = manager.register_with_options(
    KeyCode::KEY_F1,
    &[KeyCode::KEY_LEFTCTRL],
    HotkeyOptions::new()
        .on_release_callback(|| println!("Released"))
        .min_hold(Duration::from_millis(500))
        .trigger_on_repeat(false),
    || {
        println!("Pressed (only after min hold)");
    },
)?;
```

`register(...)` still triggers on key press immediately. Use `register_with_options(...)` when you need release callbacks, hold thresholds, repeat behavior control, or passthrough behavior in grab mode. Use `.on_release()` if you want release to reuse the same callback as press.

### Event grabbing / interception (feature-gated)

Enable exclusive capture with the `grab` feature when you need daemon-style interception:

```toml
[dependencies]
evdev-hotkey = { version = "0.1", features = ["grab"] }
```

```rust
use evdev::KeyCode;
use evdev_hotkey::{Backend, HotkeyManager, HotkeyOptions};

let manager = HotkeyManager::builder()
    .backend(Backend::Evdev)
    .grab(true)
    .build()?;

// Consumed by default while grab is active.
let _consumed = manager.register(KeyCode::KEY_L, &[KeyCode::KEY_LEFTMETA], || {
    println!("Lock screen");
})?;

// Passthrough hotkeys still fire callbacks but are re-emitted.
let _passthrough = manager.register_with_options(
    KeyCode::KEY_A,
    &[KeyCode::KEY_LEFTCTRL],
    HotkeyOptions::new().passthrough(true),
    || println!("Observed Ctrl+A"),
)?;
```

Grab mode is only supported on the evdev backend. Requesting grab on portal (or without compiling the `grab` feature) returns a clear `UnsupportedFeature` error.

### Parse Hotkeys from Strings

```rust
use evdev_hotkey::{Hotkey, HotkeySequence};

let hotkey = "Ctrl+Shift+A".parse::<Hotkey>()?;
let sequence = "Ctrl+K, Ctrl+C".parse::<HotkeySequence>()?;

assert_eq!(hotkey.to_string(), "Ctrl+Shift+A");
assert_eq!(sequence.to_string(), "Ctrl+K, Ctrl+C");
```

### Using Modifier Keys

The crate uses `evdev::KeyCode` for both the target key and modifiers:

**Modifier keys:**
- `KeyCode::KEY_LEFTCTRL`, `KeyCode::KEY_RIGHTCTRL` - Control
- `KeyCode::KEY_LEFTSHIFT`, `KeyCode::KEY_RIGHTSHIFT` - Shift
- `KeyCode::KEY_LEFTALT`, `KeyCode::KEY_RIGHTALT` - Alt
- `KeyCode::KEY_LEFTMETA`, `KeyCode::KEY_RIGHTMETA` - Super/Meta/Windows

**Target keys:**
- Letters: `KeyCode::KEY_A` through `KeyCode::KEY_Z`
- Numbers: `KeyCode::KEY_0` through `KeyCode::KEY_9`
- Special: `KeyCode::KEY_SPACE`, `KeyCode::KEY_ENTER`, `KeyCode::KEY_ESC`

**Note**: The modifier matching accepts either left or right variant. For example, if you register with `KeyCode::KEY_LEFTCTRL`, both left and right Ctrl will satisfy the modifier requirement.

### Same Key, Different Modifiers

You can register the same target key with different modifier combinations:

```rust
let _handle1 = manager.register(
    KeyCode::KEY_C,
    &[KeyCode::KEY_LEFTCTRL, KeyCode::KEY_LEFTSHIFT],
    || {
        println!("Ctrl+Shift+C triggered!");
    }
)?;

let _handle2 = manager.register(
    KeyCode::KEY_C,
    &[KeyCode::KEY_LEFTCTRL, KeyCode::KEY_LEFTALT],
    || {
        println!("Ctrl+Alt+C triggered!");
    }
)?;
```

## Platform Support

This crate is **Linux-only**. For macOS or Windows, consider:
- macOS: `hotkey` crate
- Windows: use a native Windows hotkey crate (this crate is Linux-only)

## How It Works

Unlike other hotkey crates that rely on X11 or desktop-specific APIs, `evdev-hotkey` reads directly from `/dev/input/event*` devices. This means it works on **both X11 and Wayland** without any desktop environment integration.

## Comparison to evdev-shortcut

| Feature | evdev-hotkey | evdev-shortcut |
|---------|--------------|----------------|
| Key type | `evdev::KeyCode` directly | Custom `Key` enum |
| API style | Sync closures | Async streams |
| Device discovery | Automatic | Manual (`glob`) |
| Permission checking | Yes, with helpful errors | No |
| Requires tokio | No | Yes |

## License

MIT
