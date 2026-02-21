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
- **Simple API**: Type-safe `Key` and `Modifier` enums
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
```

## Usage

### Basic Example

```rust
use keybound::{HotkeyManager, Key, Modifier};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let manager = HotkeyManager::new()?;

    let _handle = manager.register(
        Key::C,
        &[Modifier::Ctrl, Modifier::Shift],
        || {
            println!("Hotkey triggered!");
        },
    )?;

    // Keep program running
    std::thread::park();

    Ok(())
}
```

### Multiple Hotkeys

```rust
use keybound::{HotkeyManager, Key, Modifier};

let manager = HotkeyManager::new()?;

let _handle1 = manager.register(
    Key::A,
    &[Modifier::Ctrl, Modifier::Shift],
    || println!("Action A!"),
)?;

let _handle2 = manager.register(
    Key::B,
    &[Modifier::Ctrl, Modifier::Shift],
    || println!("Action B!"),
)?;
```

### Unregister Hotkeys

```rust
let handle = manager.register(
    Key::C,
    &[Modifier::Ctrl, Modifier::Shift],
    || println!("Triggered!"),
)?;

// Later
handle.unregister()?;
```

### Press / Release and Hold Options

```rust
use std::time::Duration;
use keybound::{HotkeyManager, HotkeyOptions, Key, Modifier};

let manager = HotkeyManager::new()?;

let _handle = manager.register_with_options(
    Key::F1,
    &[Modifier::Ctrl],
    HotkeyOptions::new()
        .on_release_callback(|| println!("Released"))
        .min_hold(Duration::from_millis(500))
        .debounce(Duration::from_millis(100))
        .max_rate(Duration::from_millis(300)),
    || println!("Pressed (only after min hold)"),
)?;
```

`register(...)` triggers on key press immediately. Use `register_with_options(...)` when you need release callbacks, hold thresholds, debounce/rate limiting, repeat behavior control, or passthrough behavior in grab mode. Use `.on_release()` if you want release to reuse the same callback as press.

### Event Grabbing / Interception (feature-gated)

Enable exclusive capture with the `grab` feature when you need daemon-style interception:

```toml
[dependencies]
keybound = { version = "0.1", features = ["grab"] }
```

```rust
use keybound::{HotkeyManager, HotkeyOptions, Key, Modifier};

let manager = HotkeyManager::builder()
    .grab()
    .build()?;

// Consumed by default while grab is active.
let _consumed = manager.register(Key::L, &[Modifier::Super], || {
    println!("Lock screen");
})?;

// Passthrough hotkeys still fire callbacks but are re-emitted.
let _passthrough = manager.register_with_options(
    Key::A,
    &[Modifier::Ctrl],
    HotkeyOptions::new().passthrough(),
    || println!("Observed Ctrl+A"),
)?;
```

Grab mode is only supported on the evdev backend. Requesting grab on portal (or without compiling the `grab` feature) returns a clear `UnsupportedFeature` error.

### Tap-Hold / Dual-Function Keys (feature-gated)

A key can perform different actions based on tap vs. hold. Requires event grabbing:

```toml
[dependencies]
keybound = { version = "0.1", features = ["grab"] }
```

```rust
use std::time::Duration;
use keybound::{HoldAction, HotkeyManager, Key, Modifier, TapAction, TapHoldOptions};

let manager = HotkeyManager::builder().grab().build()?;

// CapsLock: tap = Escape, hold = Ctrl
let _caps = manager.register_tap_hold(
    Key::CapsLock,
    TapAction::emit(Key::Escape),
    HoldAction::modifier(Modifier::Ctrl),
    TapHoldOptions::new().threshold(Duration::from_millis(200)),
)?;
```

### Parse Hotkeys from Strings

```rust
use keybound::{Hotkey, HotkeySequence};

let hotkey = "Ctrl+Shift+A".parse::<Hotkey>()?;
let sequence = "Ctrl+K, Ctrl+C".parse::<HotkeySequence>()?;

assert_eq!(hotkey.to_string(), "Ctrl+Shift+A");
assert_eq!(sequence.to_string(), "Ctrl+K, Ctrl+C");
```

Modifier aliases: `Control`, `Meta`, `Win`, `Windows` are also accepted. Key aliases include `Esc`/`Escape`, `Del`/`Delete`, `PgUp`/`PageUp`, etc.

### Config Serialization (feature-gated)

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
[[hotkeys]]
hotkey = "Ctrl+Shift+A"
action = "launch-terminal"
"#)?;

let mut actions = ActionMap::new();
actions.insert("launch-terminal".parse::<ActionId>()?, || {
    println!("Launching terminal");
})?;

let _registered = config.register(&manager, &actions)?;
```

Config files can also define key sequences and modes:

```toml
[[hotkeys]]
hotkey = "Ctrl+Shift+N"
action = "new_window"

[[sequences]]
sequence = "Ctrl+K, Ctrl+S"
action = "save_all"

[modes.resize]
bindings = [
    { hotkey = "H", action = "shrink_left" },
    { hotkey = "Escape", action = "exit_mode" },
]
```

### Using Keys and Modifiers

The crate provides its own `Key` and `Modifier` types, abstracting over platform-specific key codes:

**Modifier keys:**
- `Modifier::Ctrl` — Control (matches both left and right physical keys)
- `Modifier::Shift` — Shift
- `Modifier::Alt` — Alt
- `Modifier::Super` — Super/Meta/Windows

**Target keys:**
- Letters: `Key::A` through `Key::Z`
- Numbers: `Key::Num0` through `Key::Num9`
- Function keys: `Key::F1` through `Key::F24`
- Special: `Key::Space`, `Key::Enter`, `Key::Escape`, `Key::Tab`, `Key::CapsLock`
- Navigation: `Key::Home`, `Key::End`, `Key::PageUp`, `Key::PageDown`, `Key::Up`, `Key::Down`, `Key::Left`, `Key::Right`
- Numpad: `Key::Numpad0` through `Key::Numpad9`, `Key::NumpadEnter`, etc.

### Same Key, Different Modifiers

You can register the same target key with different modifier combinations:

```rust
use keybound::{HotkeyManager, Key, Modifier};

let manager = HotkeyManager::new()?;

let _handle1 = manager.register(
    Key::C,
    &[Modifier::Ctrl, Modifier::Shift],
    || println!("Ctrl+Shift+C triggered!"),
)?;

let _handle2 = manager.register(
    Key::C,
    &[Modifier::Ctrl, Modifier::Alt],
    || println!("Ctrl+Alt+C triggered!"),
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
