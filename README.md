# kbd

Keyboard shortcut engine for Rust.

The core (`kbd-core`) is platform-agnostic — it handles key types,
modifier tracking, binding matching, layer stacks, and sequence
resolution. It works anywhere you have key events: GUI apps, TUI apps,
compositors, game engines.

The facade (`kbd-global`) adds a Linux global hotkey backend on top,
with evdev device access, grab mode, and XDG portal support.

## Features

- **Dual backend** — XDG GlobalShortcuts portal (no root) with evdev fallback
- **Key sequences** — multi-step combos like `Ctrl+K, Ctrl+C`
- **Layers** — stack-based groups of hotkeys (oneshot, swallow, timeout)
- **Event grabbing** — exclusive capture via `EVIOCGRAB` with uinput re-emission
- **Tap vs. hold** — dual-function keys (e.g. CapsLock → tap Escape / hold Ctrl)
- **Device filtering** — bind hotkeys to specific keyboards by name or USB ID
- **Press / release / hold** — separate callbacks, min-hold, debounce, rate limiting
- **String parsing** — `"Ctrl+Shift+A".parse::<Hotkey>()`
- **Async streams** — optional tokio / async-std event streams
- **Embeddable matcher** — `kbd-core`'s `Matcher` works in any event loop (winit, ratatui, Smithay, etc.)

## Installation

For global hotkeys on Linux:

```toml
[dependencies]
kbd-global = "0.1"
```

For in-app shortcut matching (any platform):

```toml
[dependencies]
kbd-core = "0.1"
```

Optional features on `kbd-global`: `grab`, `portal`, `tokio`, `async-std`, `serde`.

## Quick start

```rust
use kbd_global::{HotkeyManager, Hotkey, Key, Modifier};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let manager = HotkeyManager::new()?;

    let _handle = manager.register(
        Hotkey::new(Key::C).modifier(Modifier::Ctrl).modifier(Modifier::Shift),
        || println!("Ctrl+Shift+C pressed!"),
    )?;

    // Keep the program running to receive hotkey events
    std::thread::park();

    Ok(())
}
```

## Permissions

### evdev backend

Your user must be able to read `/dev/input/event*` devices. On most systems this means joining the `input` group:

```bash
sudo usermod -aG input $USER
# Log out and back in
```

### Grab mode

Grab mode writes to `/dev/uinput` to re-emit non-hotkey events. Grant access with a udev rule:

```bash
sudo tee /etc/udev/rules.d/99-uinput.rules <<< 'KERNEL=="uinput", GROUP="input", MODE="0660"'
sudo udevadm control --reload-rules
sudo udevadm trigger /dev/uinput
```

### Portal backend

The XDG GlobalShortcuts portal requires no special permissions.

## How it works

`kbd-global` auto-selects the best available backend:

1. **XDG GlobalShortcuts portal** — tried first when the `portal` feature is enabled. Works without root on compositors that support it (KDE Plasma, GNOME, Hyprland).
2. **evdev** — reads `/dev/input/event*` directly. Works everywhere on Linux but requires `input` group membership.

For explicit control: `HotkeyManager::with_backend(Backend::Evdev)`.

## License

MIT
