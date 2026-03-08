# kbd

[![crates.io](https://img.shields.io/crates/v/kbd.svg)](https://crates.io/crates/kbd)
[![docs.rs](https://docs.rs/kbd/badge.svg)](https://docs.rs/kbd)

`kbd` is the pure matching engine at the center of the workspace.

Use it when you already have key events from somewhere else and want a reusable binding model: physical keys, hotkeys, layers, sequences, tap-hold bindings, device-aware matching, and introspection. It has no platform dependencies and no runtime thread of its own.

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

## In this workspace

- Use [`kbd-global`](https://docs.rs/kbd-global) if you want Linux global hotkeys backed by evdev.
- Use [`kbd-evdev`](https://docs.rs/kbd-evdev) if you want direct control over Linux input devices and polling.
- Use one of the bridge crates if your events come from an application framework:
  - [`kbd-crossterm`](https://docs.rs/kbd-crossterm)
  - [`kbd-egui`](https://docs.rs/kbd-egui)
  - [`kbd-iced`](https://docs.rs/kbd-iced)
  - [`kbd-tao`](https://docs.rs/kbd-tao)
  - [`kbd-winit`](https://docs.rs/kbd-winit)

## Feature flags

| Feature | Default | Effect |
|---|---|---|
| `serde` | off | Adds `Serialize` and `Deserialize` to key and hotkey-related types |

See the [API docs on docs.rs](https://docs.rs/kbd) for the full reference.

## License

kbd is licensed under the MIT license. See the [`LICENSE`](../../LICENSE) file for more information.
