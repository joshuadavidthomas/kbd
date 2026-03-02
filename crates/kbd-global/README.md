# kbd-global

[![crates.io](https://img.shields.io/crates/v/kbd-global.svg)](https://crates.io/crates/kbd-global)
[![docs.rs](https://docs.rs/kbd-global/badge.svg)](https://docs.rs/kbd-global)

When a key combination happens on a Linux keyboard, do something. Works on Wayland, X11, and TTY — it reads evdev directly, no display server integration needed.

```toml
[dependencies]
kbd-global = "0.1"
```

```rust,no_run
use kbd_global::{HotkeyManager, Hotkey, Key, Modifier};

let manager = HotkeyManager::new()?;

let _handle = manager.register(
    Hotkey::new(Key::C).modifier(Modifier::Ctrl).modifier(Modifier::Shift),
    || println!("Ctrl+Shift+C pressed!"),
)?;

std::thread::park();
# Ok::<(), kbd_global::Error>(())
```

All [`kbd`](https://crates.io/crates/kbd) domain types are re-exported, so this is the only dependency you need.

Your user must be in the `input` group to read `/dev/input/event*` devices:

```bash
sudo usermod -aG input $USER
```

The `grab` feature enables exclusive device capture via `EVIOCGRAB` with uinput forwarding for non-hotkey events — see the [docs](https://docs.rs/kbd-global) for udev setup. The `serde` feature adds serialization for hotkey types.

## License

MIT
