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

Legacy hotkeys use physical key positions (W3C key codes), so they work the same regardless of keyboard layout. Explicit `BindingPattern` values also match exact logical characters or named keys, and `BindingSequence` can mix both domains. For example, `Ctrl+physical:K, logical:"c"` describes two input events; `logical:Enter` and `logical:"Enter"` are distinct. Feed a `KeyboardObservation` to `process_event` to resolve both identities together without layout inference. Layers, sequences, tap-hold, device filtering, and introspection are all built in — see the [`kbd` crate docs](https://docs.rs/kbd) for the full picture.

The core crate is pure logic — no platform dependencies, no async runtime, no threads. You bring key events from wherever you have them.

Bridge crates (`kbd-winit`, `kbd-egui`, `kbd-iced`, `kbd-tao`, `kbd-crossterm`) convert framework key types into `kbd` types so you can feed them straight to the dispatcher. For system-wide hotkeys on Linux, `kbd-global` runs a background thread that reads from evdev devices directly — works on Wayland, X11, and TTY without display-server integration.

You can mix sources — a Tauri app might use `kbd-tao` for in-window shortcuts and `kbd-global` for global hotkeys, both feeding the same `Dispatcher`.

## Input lifetime and modifier evidence

Use `process_event_from_source` or `process_event_with_device` for independent
input sources; device-less dispatch has one anonymous source. Tap-holds use the
physical source/key identity, never logical text. Drain `pending_timeouts` and
handle its results between events. `cancel_source` and `reset_input` cancel
transient work without tap actions; call full reset on focus loss. Reset also
revokes collected timeout tokens, preserves registrations/layers, and does not
rewind actions already executed. Hosts maintaining their own key state must clear
it too.

`KeyboardObservation::modifier_observation` separates physical flags from optional
semantic logical modifiers. `ModifierState` records active and known masks plus
unrepresentable active extras. Exact matching compares known active flags; unknown
flags cannot satisfy requirements, and reported extras block matching. Unreported
Fn/AltGraph remain unknown rather than known-inactive. Legacy `modifiers` remains
a compatibility set; rich metadata is authoritative when supplied.

Logical matching stays exact by default. Immediate bindings can explicitly select
`BindingOptions::with_logical_match_policy(LogicalMatchPolicy::Consumed)` to allow
extra modifiers reported consumed by a layout. Required modifiers remain required;
unknown consumption falls back to exact matching. This can match logical `!` while
physical Shift+Digit1 remains available, or logical Tab on Shift+Tab when Shift is
consumed. Sequence steps stay exact. Backend masks need semantic, keymap-aware
translation; right Alt is not proof of AltGraph, and no layout is inferred here.

Parse configuration aliases into `ConfiguredPattern`, then call
`register_configured_pattern` with explicit `PrimaryModifier::Ctrl` or `Super`.
For example, `Primary+logical:"s"` resolves before conflict checking. Keep the
configuration value for serialization or later policy changes; Primary is never
an observed modifier. `Modifier::keys()` now returns optional physical associations
(one key for Fn, none for AltGraph); use `Key::try_from(modifier)` for a fallible
physical projection.

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

## Versioning

kbd is pre-1.0 and under active development. The public API may change between minor versions.

All published crates in the workspace currently share a single version number and are released together, even if only some crates have changes in a given release. After 1.0, crates may move to independent versioning.

## Contributing

[Issues](https://github.com/joshuadavidthomas/kbd/issues) and pull requests are welcome. See the [changelog](CHANGELOG.md) for release history.

## License

kbd is licensed under the MIT license. See the [`LICENSE`](LICENSE) file for more information.
