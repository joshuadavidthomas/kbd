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

The core crate is pure logic — no platform dependencies, no async runtime, no threads. You bring key events from wherever you have them. Bridge crates (`kbd-winit`, `kbd-egui`, `kbd-iced`, `kbd-tao`, `kbd-crossterm`) convert framework key types into `kbd` types. For system-wide hotkeys on Linux, `kbd-global` runs a background thread reading evdev devices directly — works on Wayland, X11, and TTY. You can mix sources: a Tauri app might use `kbd-tao` for in-window shortcuts and `kbd-global` for global hotkeys, both feeding the same `Dispatcher`.

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
