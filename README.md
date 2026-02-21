# keybound

Global hotkey library for Linux — works on Wayland, X11, and TTY.

[API docs](https://docs.rs/keybound) · [Examples](examples/)

## Features

- **Dual backend** — XDG GlobalShortcuts portal (no root) with evdev fallback
- **Key sequences** — multi-step combos like `Ctrl+K, Ctrl+C`
- **Modes / layers** — stack-based groups of hotkeys (oneshot, swallow, timeout)
- **Event grabbing** — exclusive capture via `EVIOCGRAB` with uinput re-emission
- **Tap vs. hold** — dual-function keys (e.g. CapsLock → tap Escape / hold Ctrl)
- **Device filtering** — bind hotkeys to specific keyboards by name or USB ID
- **Press / release / hold** — separate callbacks, min-hold, debounce, rate limiting
- **String parsing** — `"Ctrl+Shift+A".parse::<Hotkey>()`
- **Async streams** — optional tokio / async-std event streams
- **Config files** — load bindings from TOML/JSON/YAML via serde

## Installation

```toml
[dependencies]
keybound = "0.1"
```

Optional features: `grab`, `portal`, `tokio`, `async-std`, `serde`. See the [API docs](https://docs.rs/keybound) for the full feature flag table.

## Quick start

```rust
use keybound::{HotkeyManager, Key, Modifier};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let manager = HotkeyManager::new()?;

    let _handle = manager.register(
        Key::C,
        &[Modifier::Ctrl, Modifier::Shift],
        || println!("Ctrl+Shift+C pressed!"),
    )?;

    // Keep the program running to receive hotkey events
    std::thread::park();

    Ok(())
}
```

See the [`examples/`](examples/) directory for sequences, modes, grab mode, tap-hold, config files, async streams, and more.

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

`keybound` auto-selects the best available backend:

1. **XDG GlobalShortcuts portal** — tried first when the `portal` feature is enabled. Works without root on compositors that support it (KDE Plasma, GNOME, Hyprland).
2. **evdev** — reads `/dev/input/event*` directly. Works everywhere on Linux but requires `input` group membership.

For explicit control: `HotkeyManager::with_backend(Backend::Evdev)`.

## License

MIT
