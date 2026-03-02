# kbd-iced

Iced bridge for [`kbd`](https://crates.io/crates/kbd) — converts iced key
events and modifiers to `kbd` types.

## Installation

```toml
[dependencies]
kbd = "0.1"
kbd-iced = "0.1"
```

## Usage

```rust
use iced_core::keyboard::{key::Code, Modifiers};
use kbd::{Key, Modifier};
use kbd_iced::{IcedKeyExt, IcedModifiersExt};

// Code conversion
let key = Code::KeyA.to_key();
assert_eq!(key, Some(Key::A));

// Modifier conversion
let mods = Modifiers::CTRL.to_modifiers();
assert_eq!(mods, vec![Modifier::Ctrl]);
```

## Extension traits

- `IcedKeyExt` — converts an iced `key::Code` or `key::Physical` to a `kbd::Key`
- `IcedModifiersExt` — converts iced `Modifiers` to `Vec<Modifier>`
- `IcedEventExt` — converts an iced keyboard `Event` to a `kbd::Hotkey`

## License

MIT
