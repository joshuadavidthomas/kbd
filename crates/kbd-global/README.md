# kbd-global

[![crates.io](https://img.shields.io/crates/v/kbd-global.svg)](https://crates.io/crates/kbd-global)
[![docs.rs](https://docs.rs/kbd-global/badge.svg)](https://docs.rs/kbd-global)

System-wide hotkeys on Linux for the [`kbd` workspace](https://github.com/joshuadavidthomas/kbd).

`kbd-global` runs a background thread that reads from evdev input devices, feeds events through `kbd`'s dispatcher, and fires your callbacks when bindings match. It handles device discovery, hotplug, and the event loop so you don't have to. Works on Wayland, X11, and TTY — no display server integration needed.

```toml
[dependencies]
kbd = "0.1"
kbd-global = "0.1"
```

## Requirements

`kbd-global` reads `/dev/input/event*` directly. Your user needs permission to access input devices:

```bash
sudo usermod -aG input $USER
```

Log out and back in for the group change to take effect.

## Example

```rust,no_run
use kbd::hotkey::{Hotkey, Modifier};
use kbd::key::Key;
use kbd_global::manager::HotkeyManager;

let manager = HotkeyManager::new()?;

// Registration returns a guard — the binding lives until the guard is dropped
let _guard = manager.register(
    Hotkey::new(Key::C).modifier(Modifier::Ctrl).modifier(Modifier::Shift),
    || println!("Ctrl+Shift+C pressed"),
)?;

// Keep the main thread alive so the background listener keeps running
std::thread::park();
```

`HotkeyManager` is the main entry point. It spawns the engine thread on creation and communicates with it over a channel. Registration returns a [`BindingGuard`](https://docs.rs/kbd-global/latest/kbd_global/binding_guard/struct.BindingGuard.html) — dropping it unregisters the binding. Dropping the manager (or calling `shutdown()`) stops the runtime.

## Layers

Layers let you swap between different binding sets at runtime — think vim modes, or a "gaming" layer that disables desktop shortcuts:

```rust,no_run
use kbd::action::Action;
use kbd::key::Key;
use kbd::layer::Layer;
use kbd_global::manager::HotkeyManager;

let manager = HotkeyManager::new()?;

let layer = Layer::new("vim-normal")
    .bind(Key::J, || println!("down"))?
    .bind(Key::K, || println!("up"))?
    .bind(Key::I, Action::PushLayer("vim-insert".into()))?;

let insert = Layer::new("vim-insert")
    .bind("Escape".parse()?, Action::PopLayer)?;

manager.define_layer(layer)?;
manager.define_layer(insert)?;
manager.push_layer("vim-normal")?;
```

Layers stack — the most recently pushed layer is checked first. `PopLayer` removes the top layer, `ToggleLayer` adds or removes by name.

## Grab mode

With the `grab` feature enabled, `kbd-global` can exclusively capture input devices so matched keys never reach other applications. Unmatched events are forwarded through a virtual uinput device.

```toml
[dependencies]
kbd-global = { version = "0.1", features = ["grab"] }
```

```rust,no_run
use kbd_global::manager::HotkeyManager;

let manager = HotkeyManager::builder()
    .grab()
    .build()?;
```

Grab mode requires write access to `/dev/uinput` in addition to read access on `/dev/input/`.

## Feature flags

| Feature | Effect |
|---|---|
| `grab` | Exclusive device capture via `EVIOCGRAB` with uinput forwarding for unmatched events |
| `serde` | Serde support for shared `kbd` key and hotkey types |

## Current status

- Linux only
- evdev is the only backend
- `Action::EmitHotkey` and `Action::EmitSequence` are not yet implemented in the runtime

## Related crates

- [`kbd`](https://docs.rs/kbd) — the core matching engine, used directly for in-process shortcuts
- [`kbd-evdev`](https://docs.rs/kbd-evdev) — the low-level device backend this crate wraps, for when you need to own the poll loop yourself
- Bridge crates for framework integration: [`kbd-crossterm`](https://docs.rs/kbd-crossterm), [`kbd-egui`](https://docs.rs/kbd-egui), [`kbd-iced`](https://docs.rs/kbd-iced), [`kbd-tao`](https://docs.rs/kbd-tao), [`kbd-winit`](https://docs.rs/kbd-winit)

## License

kbd-global is licensed under the MIT license. See the [`LICENSE`](../../LICENSE) file for more information.
