# kbd

Keyboard shortcut engine for Rust.

The core (`kbd`) is platform-agnostic — it handles key types,
modifier tracking, binding matching, and layer stacks. It works
anywhere you have key events: GUI apps, TUI apps, compositors,
game engines.

The runtime (`kbd-global`) adds a Linux global hotkey backend on top,
with evdev device access and grab mode.

## Features

- **Embeddable matcher** — `kbd`'s `Matcher` works synchronously in any event loop (winit, ratatui, Smithay, etc.)
- **Layers** — stack-based binding groups with oneshot, swallow, and timeout options
- **String parsing** — `"Ctrl+Shift+A".parse::<Hotkey>()`
- **Introspection** — list bindings, query what would fire, detect conflicts and shadowed bindings
- **Event grabbing** — exclusive capture via `EVIOCGRAB` with uinput forwarding (`kbd-global`)
- **Framework bridges** — crossterm, winit, tao, iced, egui key event conversions

### Planned

- Key sequences (`Ctrl+K, Ctrl+C`)
- Tap-hold dual-function keys
- XDG GlobalShortcuts portal backend
- Device filtering
- Async event streams (tokio / async-std)
- Serde support

## Installation

For global hotkeys on Linux:

```toml
[dependencies]
kbd-global = "0.1"
```

For in-app shortcut matching (any platform):

```toml
[dependencies]
kbd = "0.1"
```

Optional features on `kbd-global`: `grab`, `serde`.

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

## How it works

`kbd-global` uses the Linux evdev subsystem to read key events directly from
`/dev/input/event*` device nodes. This works on both X11 and Wayland without
requiring display server integration.

For explicit backend selection: `HotkeyManager::builder().backend(Backend::Evdev).build()`.

## License

MIT
