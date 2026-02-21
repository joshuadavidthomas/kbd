# keybound

Global hotkey library for Linux — works on Wayland, X11, and TTY.

## Features

- **Cross-desktop**: Works on Wayland, X11, and TTY with automatic backend selection
- **Dual backend**: XDG GlobalShortcuts portal (no root needed) with evdev fallback
- **Key sequences**: Multi-step combos like `Ctrl+K, Ctrl+C` with configurable timeout
- **Modes / layers**: Named groups of hotkeys with stack-based activation (oneshot, swallow, timeout)
- **Event grabbing**: Exclusive capture via `EVIOCGRAB` with uinput re-emission of non-hotkey events
- **Tap vs. hold**: Dual-function keys (tap for one action, hold for another)
- **Device-specific hotkeys**: Filter by device name, vendor/product ID
- **Press / release / hold**: Separate callbacks, minimum hold duration, repeat control
- **String parsing**: `"Ctrl+Shift+A".parse::<Hotkey>()` with aliases and round-trip display
- **Async API**: Optional tokio/async-std event streams
- **Config serialization**: Load hotkey definitions from TOML/JSON/YAML via serde
- **Debouncing / rate limiting**: Per-hotkey timing controls
- **Key state queries**: Thread-safe access to currently pressed keys and active modifiers
- **Device hotplug**: Automatic detection of connected/disconnected keyboards
- **Simple API**: Type-safe with `evdev::KeyCode`
- **Lightweight**: Minimal dependencies, feature-gated extras

## Requirements

When using the evdev backend, your user must be allowed to read `/dev/input/event*` devices. On many systems this means being in the `input` group:

```bash
sudo usermod -aG input $USER
# Then log out and log back in
```

The portal backend (XDG GlobalShortcuts) does not require special permissions.

## Installation

```toml
[dependencies]
keybound = "0.1"
evdev = "0.13"
```

## Usage

### Basic Example

```rust
use evdev::KeyCode;
use keybound::HotkeyManager;

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
use keybound::{HotkeyManager, HotkeyOptions};
use std::time::Duration;

let manager = HotkeyManager::new()?;

let _handle = manager.register_with_options(
    KeyCode::KEY_F1,
    &[KeyCode::KEY_LEFTCTRL],
    HotkeyOptions::new()
        .on_release_callback(|| println!("Released"))
        .min_hold(Duration::from_millis(500))
        .debounce(Duration::from_millis(100))
        .max_rate(Duration::from_millis(300)),
    || {
        println!("Pressed (only after min hold)");
    },
)?;
```

`register(...)` still triggers on key press immediately. Use `register_with_options(...)` when you need release callbacks, hold thresholds, debounce/rate limiting, repeat behavior control, or passthrough behavior in grab mode. Use `.on_release()` if you want release to reuse the same callback as press.

### Event grabbing / interception (feature-gated)

Enable exclusive capture with the `grab` feature when you need daemon-style interception:

```toml
[dependencies]
keybound = { version = "0.1", features = ["grab"] }
```

```rust
use evdev::KeyCode;
use keybound::{Backend, HotkeyManager, HotkeyOptions};

let manager = HotkeyManager::builder()
    .backend(Backend::Evdev)
    .grab()
    .build()?;

// Consumed by default while grab is active.
let _consumed = manager.register(KeyCode::KEY_L, &[KeyCode::KEY_LEFTMETA], || {
    println!("Lock screen");
})?;

// Passthrough hotkeys still fire callbacks but are re-emitted.
let _passthrough = manager.register_with_options(
    KeyCode::KEY_A,
    &[KeyCode::KEY_LEFTCTRL],
    HotkeyOptions::new().passthrough(),
    || println!("Observed Ctrl+A"),
)?;
```

Grab mode is only supported on the evdev backend. Requesting grab on portal (or without compiling the `grab` feature) returns a clear `UnsupportedFeature` error.

### Parse Hotkeys from Strings

```rust
use keybound::{Hotkey, HotkeySequence};

let hotkey = "Ctrl+Shift+A".parse::<Hotkey>()?;
let sequence = "Ctrl+K, Ctrl+C".parse::<HotkeySequence>()?;

assert_eq!(hotkey.to_string(), "Ctrl+Shift+A");
assert_eq!(sequence.to_string(), "Ctrl+K, Ctrl+C");
```

### Config serialization (feature-gated)

Enable serde support when loading bindings from config files:

```toml
[dependencies]
keybound = { version = "0.1", features = ["serde"] }
serde = { version = "1", features = ["derive"] }
toml = "0.8"
```

```rust
use keybound::{ActionId, ActionMap, HotkeyConfig, HotkeyManager};

let manager = HotkeyManager::new()?;

let config: HotkeyConfig = toml::from_str(r#"
hotkeys = [
  { hotkey = "Ctrl+Shift+A", action = "launch-terminal" }
]
"#)?;

let mut actions = ActionMap::new();
actions.insert(ActionId::new("launch-terminal")?, || {
    println!("Launching terminal");
})?;

let _registered = config.register(&manager, &actions)?;
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

This crate currently targets **Linux**. The backend trait architecture is designed to support additional platforms in the future.

## How It Works

`keybound` auto-selects the best available backend:

1. **XDG GlobalShortcuts portal** — tried first when the `portal` feature is enabled. Works without root on compositors that support it (KDE Plasma, GNOME, Hyprland).
2. **evdev** — falls back to reading directly from `/dev/input/event*` devices. Works everywhere on Linux (Wayland, X11, TTY, headless) but requires `input` group membership.

The caller never needs to know which backend is active. For explicit control, use `HotkeyManager::with_backend(Backend::Evdev)` or `HotkeyManager::with_backend(Backend::Portal)`.

## License

MIT
