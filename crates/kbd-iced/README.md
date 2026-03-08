# kbd-iced

[![crates.io](https://img.shields.io/crates/v/kbd-iced.svg)](https://crates.io/crates/kbd-iced)
[![docs.rs](https://docs.rs/kbd-iced/badge.svg)](https://docs.rs/kbd-iced)

`kbd-iced` converts iced keyboard events into `kbd` types so in-window shortcuts and global hotkeys can share the same dispatcher.

```toml
[dependencies]
kbd = "0.1"
kbd-iced = "0.1"
```

## Public API

- `IcedKeyExt` converts `iced_core::keyboard::key::Code` and `key::Physical` to `kbd::key::Key`
- `IcedModifiersExt` converts iced `Modifiers` to `kbd::hotkey::ModifierSet`
- `IcedEventExt` converts iced keyboard `Event` values to `kbd::hotkey::Hotkey`

## Example

```rust
use iced_core::keyboard::{self, Modifiers};
use kbd::hotkey::Modifier;
use kbd::key::Key;
use kbd_iced::{IcedKeyExt, IcedModifiersExt};

let key = keyboard::key::Code::KeyA.to_key();
assert_eq!(key, Some(Key::A));

let mods = Modifiers::CTRL.to_modifiers();
assert!(mods.contains(Modifier::Ctrl));
assert_eq!(mods.len(), 1);
```

## Mapping notes

- this crate converts physical key types, not iced logical key values
- modifier conversion yields a compact `ModifierSet`
- modifier keys used as triggers are normalized so they do not include themselves as active modifiers

See the [API docs on docs.rs](https://docs.rs/kbd-iced) for full event conversion details and mapping tables.

## License

kbd-iced is licensed under the MIT license. See the [`LICENSE`](../../LICENSE) file for more information.
