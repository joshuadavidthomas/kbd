# kbd

Pure-logic keyboard shortcut engine for Rust.

`kbd` provides the domain types and matching logic that every keyboard shortcut
system needs: key types, modifier tracking, binding matching, layer stacks, and
sequence resolution. It has zero platform dependencies and can be embedded in
any event loop — winit, egui, Smithay, a game loop, or a compositor.

## Installation

```toml
[dependencies]
kbd = "0.1"
```

## Usage

```rust
use kbd::{Action, Hotkey, Key, Layer, MatchResult, Matcher, Modifier};

// Parse hotkeys from strings
let hotkey: Hotkey = "Ctrl+Shift+A".parse().unwrap();
assert_eq!(hotkey.key(), Key::A);
assert_eq!(hotkey.modifiers(), &[Modifier::Ctrl, Modifier::Shift]);

// Build a matcher with bindings
let mut matcher = Matcher::new();
matcher.add_binding(hotkey, Action::from(|| println!("fired")), Default::default());

// Feed key events from any source
let result = matcher.key_down(Key::A, &[Modifier::Ctrl, Modifier::Shift]);
assert!(matches!(result, MatchResult::Matched { .. }));
```

## Features

- **String parsing** — `"Ctrl+Shift+A".parse::<Hotkey>()` with aliases (`Cmd`, `Super`, `Win`)
- **Synchronous matcher** — works in any event loop, no async runtime needed
- **Layers** — stack-based binding groups with oneshot, swallow, and timeout options
- **Introspection** — list bindings, query what would fire, detect conflicts and shadowed bindings
- **Optional serde support** — enable the `serde` feature for serialization

## Framework bridges

Use `kbd` with your framework of choice:

- [`kbd-crossterm`](https://crates.io/crates/kbd-crossterm) — crossterm (TUI)
- [`kbd-winit`](https://crates.io/crates/kbd-winit) — winit
- [`kbd-tao`](https://crates.io/crates/kbd-tao) — tao (Tauri)
- [`kbd-iced`](https://crates.io/crates/kbd-iced) — iced
- [`kbd-egui`](https://crates.io/crates/kbd-egui) — egui

For global hotkeys on Linux, see [`kbd-global`](https://crates.io/crates/kbd-global).

## License

MIT
