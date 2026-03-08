# kbd

[![CI](https://github.com/joshuadavidthomas/kbd/actions/workflows/test.yml/badge.svg)](https://github.com/joshuadavidthomas/kbd/actions/workflows/test.yml)
[![MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![MSRV](https://img.shields.io/badge/MSRV-1.85-blue.svg)](#)

A keyboard shortcut engine for Rust. You describe the bindings you care about, feed in key events from whatever source you have, and `kbd` tells you when something matches.

```rust
use kbd::action::Action;
use kbd::dispatcher::{Dispatcher, MatchResult};
use kbd::key_state::KeyTransition;

let mut dispatcher = Dispatcher::new();

dispatcher.register("Ctrl+S", || println!("saved"))?;
dispatcher.register("Ctrl+Shift+P", Action::Suppress)?;

// process() tells you: matched, partially matched (sequence), or no match
let result = dispatcher.process("Ctrl+S".parse()?, KeyTransition::Press);
```

Bindings use physical key positions (W3C key codes), so they work the same regardless of keyboard layout. Layers, sequences, tap-hold, device filtering, and introspection are all built in — see the [`kbd` crate docs](https://docs.rs/kbd) for the full picture.

## Getting started

Add `kbd` to your project:

```toml
[dependencies]
kbd = "0.1"
```

The core crate is pure logic — no threads, no platform dependencies, no async runtime. You bring the key events, `kbd` does the matching.

If your events come from a GUI framework, add the bridge crate for your framework. It converts the framework's key types into `kbd` types so you can feed them straight to the dispatcher:

```toml
[dependencies]
kbd = "0.1"
kbd-winit = "0.1"  # or kbd-egui, kbd-iced, kbd-tao, kbd-crossterm
```

For system-wide hotkeys on Linux, `kbd-global` runs a background thread that reads from evdev devices directly — works on Wayland, X11, and TTY without display-server integration:

```toml
[dependencies]
kbd = "0.1"
kbd-global = "0.1"
```

```rust,no_run
use kbd::hotkey::{Hotkey, Modifier};
use kbd::key::Key;
use kbd_global::manager::HotkeyManager;

let manager = HotkeyManager::new()?;

// Registration returns a guard — the binding stays active until it's dropped
let _guard = manager.register(
    Hotkey::new(Key::C).modifier(Modifier::Ctrl).modifier(Modifier::Shift),
    || println!("Ctrl+Shift+C"),
)?;
```

## How it fits together

`kbd` is the matching engine. It has no idea where key events come from — it just evaluates bindings against whatever you hand it.

Backend crates connect to actual input sources. Today that's `kbd-evdev` (Linux input devices) and `kbd-global` (threaded runtime around evdev). Bridge crates convert framework events into `kbd` types so you don't write the mapping yourself.

You can mix sources. A Tauri app might use `kbd-tao` for in-window shortcuts and `kbd-global` for system-wide hotkeys, both feeding the same `Dispatcher`.

## Crates

| Crate | | |
|---|---|---|
| [`kbd`](crates/kbd) | [![crates.io](https://img.shields.io/crates/v/kbd.svg)](https://crates.io/crates/kbd) | Core engine — key types, hotkeys, dispatcher, layers, string parsing |
| [`kbd-evdev`](crates/kbd-evdev) | [![crates.io](https://img.shields.io/crates/v/kbd-evdev.svg)](https://crates.io/crates/kbd-evdev) | Linux evdev backend — device discovery, hotplug, grab, forwarding |
| [`kbd-global`](crates/kbd-global) | [![crates.io](https://img.shields.io/crates/v/kbd-global.svg)](https://crates.io/crates/kbd-global) | System-wide hotkeys on Linux (evdev, grab mode, hotplug) |
| [`kbd-crossterm`](crates/kbd-crossterm) | [![crates.io](https://img.shields.io/crates/v/kbd-crossterm.svg)](https://crates.io/crates/kbd-crossterm) | [crossterm](https://docs.rs/crossterm) bridge — TUI apps |
| [`kbd-egui`](crates/kbd-egui) | [![crates.io](https://img.shields.io/crates/v/kbd-egui.svg)](https://crates.io/crates/kbd-egui) | [egui](https://docs.rs/egui) bridge |
| [`kbd-iced`](crates/kbd-iced) | [![crates.io](https://img.shields.io/crates/v/kbd-iced.svg)](https://crates.io/crates/kbd-iced) | [iced](https://docs.rs/iced) bridge |
| [`kbd-tao`](crates/kbd-tao) | [![crates.io](https://img.shields.io/crates/v/kbd-tao.svg)](https://crates.io/crates/kbd-tao) | [tao](https://docs.rs/tao) bridge (Tauri) |
| [`kbd-winit`](crates/kbd-winit) | [![crates.io](https://img.shields.io/crates/v/kbd-winit.svg)](https://crates.io/crates/kbd-winit) | [winit](https://docs.rs/winit) bridge |

## Contributing

[Issues](https://github.com/joshuadavidthomas/kbd/issues) and pull requests are welcome. See the [changelog](CHANGELOG.md) for release history.

## License

kbd is licensed under the MIT license. See the [`LICENSE`](LICENSE) file for more information.
