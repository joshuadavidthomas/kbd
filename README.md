# kbd

Keyboard shortcut engine for Rust.

The core crate (`kbd`) is platform-agnostic — key types, modifier tracking, hotkey parsing, binding matching, layer stacks. It works anywhere you have key events: GUI apps, TUI apps, compositors, game engines.

Bridge crates convert framework-specific key events into `kbd` types. A Linux runtime (`kbd-global`) adds system-wide global hotkeys on top.

## Quick start

### In-app shortcut matching (any platform)

```rust
use kbd::{Action, Hotkey, Key, MatchResult, Matcher, Modifier};

let mut matcher = Matcher::new();

let hotkey: Hotkey = "Ctrl+Shift+A".parse().unwrap();
matcher.add_binding(hotkey, Action::from(|| println!("fired")), Default::default());

let result = matcher.key_down(Key::A, &[Modifier::Ctrl, Modifier::Shift]);
assert!(matches!(result, MatchResult::Matched { .. }));
```

```toml
[dependencies]
kbd = "0.1"
```

### Global hotkeys on Linux

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

```toml
[dependencies]
kbd-global = "0.1"
```

## Crates

| Crate | Description |
|---|---|
| [`kbd`](crates/kbd) | Core engine — key types, matcher, layers, string parsing |
| [`kbd-global`](crates/kbd-global) | Linux global hotkey runtime (evdev, grab mode, hotplug) |
| [`kbd-evdev`](crates/kbd-evdev) | Linux evdev backend (used by `kbd-global`) |
| [`kbd-crossterm`](crates/kbd-crossterm) | Bridge for [crossterm](https://docs.rs/crossterm) |
| [`kbd-winit`](crates/kbd-winit) | Bridge for [winit](https://docs.rs/winit) |
| [`kbd-tao`](crates/kbd-tao) | Bridge for [tao](https://docs.rs/tao) (Tauri) |
| [`kbd-iced`](crates/kbd-iced) | Bridge for [iced](https://docs.rs/iced) |
| [`kbd-egui`](crates/kbd-egui) | Bridge for [egui](https://docs.rs/egui) |

## Features

- **String parsing** — `"Ctrl+Shift+A".parse::<Hotkey>()` with aliases (`Cmd`, `Super`, `Win`)
- **Synchronous matcher** — embeds in any event loop, no async runtime needed
- **Layers** — stack-based binding groups with oneshot, swallow, and timeout options
- **Introspection** — list bindings, query what would fire, detect conflicts and shadowed bindings
- **Grab mode** — exclusive device capture via `EVIOCGRAB` with uinput forwarding (`kbd-global`)
- **Framework bridges** — crossterm, winit, tao, iced, egui key event conversions

## License

MIT
