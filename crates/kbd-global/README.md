# kbd-global

[![crates.io](https://img.shields.io/crates/v/kbd-global.svg)](https://crates.io/crates/kbd-global)
[![docs.rs](https://docs.rs/kbd-global/badge.svg)](https://docs.rs/kbd-global)

Global hotkey runtime for kbd — threaded engine, device management, and backend selection for Linux.

When a key combination happens on a Linux keyboard, do something. Works on Wayland, X11, and TTY — it reads evdev directly, no display server integration needed.

```toml
[dependencies]
kbd-global = "0.1"
```

## Quick start

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

## Prerequisites

Your user must be in the `input` group to read `/dev/input/event*` devices:

```bash
sudo usermod -aG input $USER
```

Log out and back in for the group change to take effect.

## Architecture

`HotkeyManager` is the public API. Internally it sends commands to a
dedicated engine thread over an `mpsc` channel, with an `eventfd` wake
mechanism to interrupt `poll()`. All mutable state lives in the engine —
no locks, no shared mutation.

```text
┌──────────────────┐    Command     ┌──────────────────┐
│  HotkeyManager   │ ─────────────► │  Engine thread   │
│  (command sender) │ ◄───────────── │  (event loop)    │
└──────────────────┘    Reply        └──────────────────┘
                                          │
                                     poll(devices + wake_fd)
```

## Layers

Layers let you define context-dependent bindings. Define a layer, push
it onto the stack, and its bindings take priority over global ones:

```rust,no_run
use kbd_global::{HotkeyManager, Hotkey, Key, Modifier, Layer, Action};

let manager = HotkeyManager::new()?;

let mut layer = Layer::new("vim-normal");
layer.add(
    Hotkey::new(Key::J),
    Action::from(|| println!("down")),
)?;

manager.define_layer(layer)?;
manager.push_layer("vim-normal")?;
// Key::J now fires "down" instead of any global binding
# Ok::<(), kbd_global::Error>(())
```

## Feature flags

| Feature | Effect |
|---------|--------|
| `grab` | Enables exclusive device capture via `EVIOCGRAB` with uinput forwarding for non-hotkey events |
| `serde` | Adds `Serialize`/`Deserialize` to key and hotkey types (via `kbd`) |

The `grab` feature requires udev rules for uinput access — see the [docs](https://docs.rs/kbd-global) for setup instructions.

## License

MIT
