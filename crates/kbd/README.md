# kbd

[![crates.io](https://img.shields.io/crates/v/kbd.svg)](https://crates.io/crates/kbd)
[![docs.rs](https://docs.rs/kbd/badge.svg)](https://docs.rs/kbd)

`kbd` is a pure-logic hotkey engine for Rust. It provides the domain types and matching logic that sit underneath hotkey systems: physical keys, modifiers, global bindings, layers, sequences, tap-hold bindings, device-aware matching, and introspection.

It has no platform dependencies. You feed it key events from your own event loop or from a runtime such as [`kbd-global`](https://docs.rs/kbd-global).

```toml
[dependencies]
kbd = "0.1"
```

## Quick start

```rust
use kbd::action::Action;
use kbd::dispatcher::{Dispatcher, MatchResult};
use kbd::key_state::KeyTransition;

# fn main() -> Result<(), Box<dyn std::error::Error>> {
let mut dispatcher = Dispatcher::new();

dispatcher.register("Ctrl+Shift+A", Action::Suppress)?;

let result = dispatcher.process("Ctrl+Shift+A".parse()?, KeyTransition::Press);
assert!(matches!(result, MatchResult::Matched { .. }));
# Ok(())
# }
```

## What the crate provides

- `Key`, `Modifier`, `Hotkey`, and `HotkeySequence` types
- A synchronous `Dispatcher` for matching key events
- Global bindings and stackable named layers
- Multi-step sequence matching with timeouts
- Tap-hold bindings
- Per-binding propagation, debounce, rate limiting, and repeat policy
- Device filters and per-device modifier isolation
- Introspection APIs for overlays, debugging, and conflict reporting

## Ecosystem

- [`kbd-global`](https://docs.rs/kbd-global) for a threaded Linux runtime backed by evdev
- [`kbd-evdev`](https://docs.rs/kbd-evdev) for low-level Linux device discovery and forwarding
- Bridge crates for event-loop integration:
  - [`kbd-crossterm`](https://docs.rs/kbd-crossterm)
  - [`kbd-egui`](https://docs.rs/kbd-egui)
  - [`kbd-iced`](https://docs.rs/kbd-iced)
  - [`kbd-tao`](https://docs.rs/kbd-tao)
  - [`kbd-winit`](https://docs.rs/kbd-winit)

## Feature flags

| Feature | Default | Effect |
|---|---|---|
| `serde` | off | Adds `Serialize` and `Deserialize` to key and hotkey-related types |

## Documentation

See the [API docs on docs.rs](https://docs.rs/kbd) for the full reference.

## License

kbd is licensed under the MIT license. See the [`LICENSE`](../../LICENSE) file for more information.
