# kbd

[![CI](https://github.com/joshuadavidthomas/kbd/actions/workflows/test.yml/badge.svg)](https://github.com/joshuadavidthomas/kbd/actions/workflows/test.yml)
[![MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![MSRV](https://img.shields.io/badge/MSRV-1.85-blue.svg)](#)

A keyboard shortcut engine for Rust. You describe the shortcuts you care about, feed in key events from whatever source you have, and `kbd` tells you when something matches. Same engine whether you're building a text editor, a tiling compositor, or a global hotkey daemon.

## Crates

| Crate | | |
|---|---|---|
| [`kbd`](crates/kbd) | [![crates.io](https://img.shields.io/crates/v/kbd.svg)](https://crates.io/crates/kbd) | Core engine — key types, dispatcher, layers, string parsing |
| [`kbd-evdev`](crates/kbd-evdev) | [![crates.io](https://img.shields.io/crates/v/kbd-evdev.svg)](https://crates.io/crates/kbd-evdev) | Linux evdev backend (device discovery, hotplug, grab, forwarding) |
| [`kbd-global`](crates/kbd-global) | [![crates.io](https://img.shields.io/crates/v/kbd-global.svg)](https://crates.io/crates/kbd-global) | Linux global hotkey runtime (evdev, grab mode, hotplug) |
| [`kbd-crossterm`](crates/kbd-crossterm) | [![crates.io](https://img.shields.io/crates/v/kbd-crossterm.svg)](https://crates.io/crates/kbd-crossterm) | [crossterm](https://docs.rs/crossterm) bridge |
| [`kbd-winit`](crates/kbd-winit) | [![crates.io](https://img.shields.io/crates/v/kbd-winit.svg)](https://crates.io/crates/kbd-winit) | [winit](https://docs.rs/winit) bridge |
| [`kbd-tao`](crates/kbd-tao) | [![crates.io](https://img.shields.io/crates/v/kbd-tao.svg)](https://crates.io/crates/kbd-tao) | [tao](https://docs.rs/tao) bridge (Tauri) |
| [`kbd-iced`](crates/kbd-iced) | [![crates.io](https://img.shields.io/crates/v/kbd-iced.svg)](https://crates.io/crates/kbd-iced) | [iced](https://docs.rs/iced) bridge |
| [`kbd-egui`](crates/kbd-egui) | [![crates.io](https://img.shields.io/crates/v/kbd-egui.svg)](https://crates.io/crates/kbd-egui) | [egui](https://docs.rs/egui) bridge |

The core [`kbd`](crates/kbd) crate has no platform dependencies and works synchronously in any event loop. Bridge crates convert framework key events into `kbd` types. [`kbd-global`](crates/kbd-global) adds system-wide hotkeys on Linux.

## Contributing

[Issues](https://github.com/joshuadavidthomas/kbd/issues) and pull requests are welcome. See the [changelog](CHANGELOG.md) for release history.

## License

MIT
