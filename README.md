# kbd

[![crates.io](https://img.shields.io/crates/v/kbd.svg)](https://crates.io/crates/kbd)
[![docs.rs](https://docs.rs/kbd/badge.svg)](https://docs.rs/kbd)
[![CI](https://github.com/joshuadavidthomas/kbd/actions/workflows/test.yml/badge.svg)](https://github.com/joshuadavidthomas/kbd/actions/workflows/test.yml)
[![MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![MSRV](https://img.shields.io/badge/MSRV-1.85-blue.svg)](#)

A keyboard shortcut engine for Rust. You describe the shortcuts you care about, feed in key events from whatever source you have, and `kbd` tells you when something matches. Same engine whether you're building a text editor, a tiling compositor, or a global hotkey daemon.

```toml
[dependencies]
kbd = "0.1"
```

```rust
use kbd::{Action, Hotkey, Key, MatchResult, Matcher, Modifier};

let mut matcher = Matcher::new();

let hotkey: Hotkey = "Ctrl+Shift+A".parse().unwrap();
matcher.add_binding(hotkey, Action::from(|| println!("fired")), Default::default());

let result = matcher.key_down(Key::A, &[Modifier::Ctrl, Modifier::Shift]);
assert!(matches!(result, MatchResult::Matched { .. }));
```

The core crate has no platform dependencies and works synchronously in any event loop. String parsing supports aliases (`Cmd`, `Super`, `Win` all map to `Meta`). Layers let you group bindings into named stacks with oneshot, swallow, and timeout options. The introspection API lists active bindings, detects conflicts, and reports shadowing. Enable `serde` for serialization.

Bridge crates convert framework key events into `kbd` types. [`kbd-global`](crates/kbd-global) adds system-wide hotkeys on Linux.

| Crate | |
|---|---|
| [`kbd`](crates/kbd) | Core engine — key types, matcher, layers, string parsing |
| [`kbd-global`](crates/kbd-global) | Linux global hotkey runtime (evdev, grab mode, hotplug) |
| [`kbd-evdev`](crates/kbd-evdev) | Linux evdev backend (used by `kbd-global`) |
| [`kbd-crossterm`](crates/kbd-crossterm) | [crossterm](https://docs.rs/crossterm) bridge |
| [`kbd-winit`](crates/kbd-winit) | [winit](https://docs.rs/winit) bridge |
| [`kbd-tao`](crates/kbd-tao) | [tao](https://docs.rs/tao) bridge (Tauri) |
| [`kbd-iced`](crates/kbd-iced) | [iced](https://docs.rs/iced) bridge |
| [`kbd-egui`](crates/kbd-egui) | [egui](https://docs.rs/egui) bridge |

## Contributing

[Issues](https://github.com/joshuadavidthomas/kbd/issues) and pull requests are welcome. See the [changelog](CHANGELOG.md) for release history.

## License

MIT
