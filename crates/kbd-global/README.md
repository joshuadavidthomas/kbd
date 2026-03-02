# kbd-global

Global hotkey runtime for Linux — threaded engine, device management, and backend selection. Works on Wayland, X11, and TTY.

Part of the [`kbd`](https://crates.io/crates/kbd) keyboard shortcut engine.

## Installation

```toml
[dependencies]
kbd-global = "0.1"
```

Optional features: `grab`, `serde`.

## Usage

```rust,no_run
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

All `kbd` domain types (`Key`, `Modifier`, `Hotkey`, `Matcher`, `Layer`, etc.) are re-exported so you only need a single dependency.

## Permissions

Your user must be able to read `/dev/input/event*` devices. On most systems this means joining the `input` group:

```bash
sudo usermod -aG input $USER
# Log out and back in
```

### Grab mode

Grab mode uses `EVIOCGRAB` for exclusive device capture and writes to `/dev/uinput` to re-emit non-hotkey events. Grant access with a udev rule:

```bash
sudo tee /etc/udev/rules.d/99-uinput.rules <<< 'KERNEL=="uinput", GROUP="input", MODE="0660"'
sudo udevadm control --reload-rules
sudo udevadm trigger /dev/uinput
```

## How it works

`kbd-global` uses the Linux evdev subsystem to read key events directly from `/dev/input/event*` device nodes. This works on both X11 and Wayland without requiring display server integration.

## License

MIT
