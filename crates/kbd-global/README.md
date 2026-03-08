# kbd-global

[![crates.io](https://img.shields.io/crates/v/kbd-global.svg)](https://crates.io/crates/kbd-global)
[![docs.rs](https://docs.rs/kbd-global/badge.svg)](https://docs.rs/kbd-global)

`kbd-global` is the Linux runtime for `kbd`. It owns device discovery, hotplug handling, the engine thread, and the manager API used to register global hotkeys and layers.

Today the runtime uses the evdev backend directly, so it works on Wayland, X11, and TTY without display-server-specific integrations.

```toml
[dependencies]
kbd = "0.1"
kbd-global = "0.1"
```

## Quick start

```rust,no_run
use kbd::hotkey::{Hotkey, Modifier};
use kbd::key::Key;
use kbd_global::manager::HotkeyManager;

# fn main() -> Result<(), Box<dyn std::error::Error>> {
let manager = HotkeyManager::new()?;

let _guard = manager.register(
    Hotkey::new(Key::C)
        .modifier(Modifier::Ctrl)
        .modifier(Modifier::Shift),
    || println!("Ctrl+Shift+C pressed"),
)?;

std::thread::park();
# Ok(())
# }
```

## What the crate provides

- `manager::HotkeyManager` for registration, queries, layers, and shutdown
- `binding_guard::BindingGuard` for RAII-style unregistration
- `backend::Backend` for explicit backend selection
- A threaded engine that owns all mutable runtime state
- Access to `kbd` features such as sequences, tap-hold bindings, binding metadata, and introspection

## Layers

```rust,no_run
use kbd::action::Action;
use kbd::key::Key;
use kbd::layer::Layer;
use kbd_global::manager::HotkeyManager;

# fn main() -> Result<(), Box<dyn std::error::Error>> {
let manager = HotkeyManager::new()?;

let layer = Layer::new("vim-normal")
    .bind(Key::J, Action::Suppress)?
    .bind(Key::K, Action::Suppress)?;

manager.define_layer(layer)?;
manager.push_layer("vim-normal")?;
# Ok(())
# }
```

## Prerequisites

`kbd-global` reads `/dev/input/event*`, so your user must have permission to access Linux input devices.

```bash
sudo usermod -aG input $USER
```

Log out and back in for the group change to take effect.

If you enable grab mode, you also need permission to create and write to `/dev/uinput`.

## Feature flags

| Feature | Effect |
|---|---|
| `grab` | Enables exclusive device capture via `EVIOCGRAB` with uinput forwarding for unmatched events |
| `serde` | Enables serde support for shared `kbd` key and hotkey types |

## Current limitations

- Linux only
- evdev is the only backend currently available
- `Action::EmitHotkey` and `Action::EmitSequence` are not yet implemented in the runtime

## Documentation

See the [API docs on docs.rs](https://docs.rs/kbd-global) for the full reference.

## License

kbd-global is licensed under the MIT license. See the [`LICENSE`](../../LICENSE) file for more information.
