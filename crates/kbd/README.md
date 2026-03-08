# kbd

[![crates.io](https://img.shields.io/crates/v/kbd.svg)](https://crates.io/crates/kbd)
[![docs.rs](https://docs.rs/kbd/badge.svg)](https://docs.rs/kbd)

The pure matching engine at the center of the [`kbd` workspace](https://github.com/joshuadavidthomas/kbd).

Use it when you already have key events from somewhere — a GUI framework, a terminal library, raw evdev — and want a single binding model across all of them. It has no platform dependencies and no runtime thread of its own.

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

dispatcher.register("Ctrl+S", || println!("saved"))?;
dispatcher.register("Ctrl+Shift+P", Action::Suppress)?;

let result = dispatcher.process("Ctrl+S".parse()?, KeyTransition::Press);
assert!(matches!(result, MatchResult::Matched { .. }));
# Ok(())
# }
```

Register bindings with strings (`"Ctrl+Shift+A"`) or build them with [`Hotkey::new`](https://docs.rs/kbd/latest/kbd/hotkey/struct.Hotkey.html). Feed key events to the dispatcher with [`process`](https://docs.rs/kbd/latest/kbd/dispatcher/struct.Dispatcher.html#method.process) and match on the result.

## Layers

Layers are named, stackable groups of bindings. When active, their bindings take priority over the layers beneath them. Use them for modes (vim normal/insert), context-dependent shortcuts, or temporary overrides.

```rust
use kbd::action::Action;
use kbd::dispatcher::Dispatcher;
use kbd::key::Key;
use kbd::layer::Layer;

# fn main() -> Result<(), Box<dyn std::error::Error>> {
let mut dispatcher = Dispatcher::new();

let layer = Layer::new("vim-normal")
    .bind(Key::J, || println!("down"))?
    .bind(Key::K, || println!("up"))?;

dispatcher.define_layer(layer)?;
dispatcher.push_layer("vim-normal")?;
# Ok(())
# }
```

Layers can be oneshot (auto-pop after one match), swallowing (consume unmatched keys), or time-limited.

## Sequences

Multi-step bindings like `Ctrl+K, Ctrl+C`:

```rust
use kbd::action::Action;
use kbd::dispatcher::Dispatcher;

# fn main() -> Result<(), Box<dyn std::error::Error>> {
let mut dispatcher = Dispatcher::new();
dispatcher.register_sequence("Ctrl+K, Ctrl+C", Action::Suppress)?;
# Ok(())
# }
```

The dispatcher tracks partial matches — after `Ctrl+K` it returns `MatchResult::Pending`, and completes (or resets) on the next key.

## Why physical keys?

`kbd` matches physical key positions, not characters. `Key::A` means "the key in the A position on a QWERTY layout" regardless of whether the user's layout is AZERTY, Dvorak, or Colemak. This is the W3C `KeyboardEvent.code` model.

Physical keys are layout-independent and predictable — the same binding works everywhere without knowing the active keyboard layout. This is the right default for shortcuts. (Layout-aware symbol bindings are a planned future addition via `kbd-xkb`.)

## In this workspace

- [`kbd-global`](https://docs.rs/kbd-global) — threaded Linux runtime for system-wide hotkeys
- [`kbd-evdev`](https://docs.rs/kbd-evdev) — low-level Linux device backend
- Bridge crates for framework integration: [`kbd-crossterm`](https://docs.rs/kbd-crossterm), [`kbd-egui`](https://docs.rs/kbd-egui), [`kbd-iced`](https://docs.rs/kbd-iced), [`kbd-tao`](https://docs.rs/kbd-tao), [`kbd-winit`](https://docs.rs/kbd-winit)

## Feature flags

| Feature | Default | Effect |
|---|---|---|
| `serde` | off | Adds `Serialize` and `Deserialize` to key and hotkey-related types |

See the [API docs on docs.rs](https://docs.rs/kbd) for the full reference.

## License

kbd is licensed under the MIT license. See the [`LICENSE`](../../LICENSE) file for more information.
