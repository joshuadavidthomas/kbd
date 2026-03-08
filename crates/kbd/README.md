# kbd

[![crates.io](https://img.shields.io/crates/v/kbd.svg)](https://crates.io/crates/kbd)
[![docs.rs](https://docs.rs/kbd/badge.svg)](https://docs.rs/kbd)

The pure matching engine at the center of the [`kbd` workspace](https://github.com/joshuadavidthomas/kbd).

You describe bindings — as strings like `"Ctrl+Shift+A"` or programmatically — and the dispatcher tells you when incoming key events match. It has no platform dependencies and no runtime thread of its own; you bring the key events from wherever you have them.

```toml
[dependencies]
kbd = "0.1"
```

## Example

```rust
use kbd::action::Action;
use kbd::dispatcher::Dispatcher;
use kbd::hotkey::{Hotkey, Modifier};
use kbd::key::Key;
use kbd::layer::Layer;

let mut dispatcher = Dispatcher::new();

// Global bindings — register via string parsing...
dispatcher.register("Ctrl+S", Action::Suppress)?;

// ...or build hotkeys programmatically
dispatcher.register(
    Hotkey::new(Key::P).modifier(Modifier::Ctrl).modifier(Modifier::Shift),
    Action::Suppress,
)?;

// Layer bindings — active only when the layer is pushed
let normal = Layer::new("normal")
    .bind(Key::J, || println!("down"))?
    .bind(Key::K, || println!("up"))?
    .bind(Key::I, Action::PushLayer("insert".into()))?;

dispatcher.define_layer(normal)?;
dispatcher.push_layer("normal")?;
```

Layers stack. The most recently pushed layer is checked first, then global bindings. Layers can be oneshot (auto-pop after one match), swallowing (consume unmatched keys), or time-limited.

Multi-step bindings work too — register a sequence like `"Ctrl+K, Ctrl+C"` and the dispatcher tracks partial matches, returning `Pending` until the sequence completes or times out.

## What else is in here

Beyond hotkeys, layers, and sequences:

- **Tap-hold** — dual-function keys that do one thing on tap, another on hold. Requires grab mode in `kbd-global`.
- **Device filtering** — bind to specific keyboards by name, vendor/product ID, or physical path. Useful when you want different bindings for different devices.
- **Introspection** — query what's registered, which layers are active, and where bindings conflict or shadow each other.
- **Binding policies** — per-binding control over key propagation (consume vs. forward), repeat handling, and rate limiting.
- **String parsing** — `"Ctrl+Shift+A"`, `"Super+1"`, `"Ctrl+K, Ctrl+C"` all parse into typed values. Common aliases (`Cmd` → `Super`, `Win` → `Super`, `Return` → `Enter`) are built in.

## Why physical keys?

`kbd` matches physical key positions, not characters. `Key::A` means "the key in the A position on a QWERTY layout" regardless of whether the user's layout is AZERTY, Dvorak, or Colemak. This is the W3C `KeyboardEvent.code` model.

Physical keys are layout-independent and predictable — the same binding works everywhere without knowing the active keyboard layout. This is the right default for shortcuts. (Layout-aware symbol bindings are a planned future addition via `kbd-xkb`.)

## Feature flags

| Feature | Default | Effect |
|---|---|---|
| `serde` | off | Adds `Serialize` and `Deserialize` to key and hotkey-related types |

## License

kbd is licensed under the MIT license. See the [`LICENSE`](../../LICENSE) file for more information.
