# kbd-iced

[![crates.io](https://img.shields.io/crates/v/kbd-iced.svg)](https://crates.io/crates/kbd-iced)
[![docs.rs](https://docs.rs/kbd-iced/badge.svg)](https://docs.rs/kbd-iced)

`kbd-iced` converts iced keyboard types into `kbd` types.

Use it when an iced application wants to keep its in-window shortcuts in the same hotkey format used by `kbd` and `kbd-global`.

```toml
[dependencies]
kbd = "0.1"
kbd-iced = "0.1"
```

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
```

This crate converts iced's physical key types. Logical key values are intentionally out of scope because `kbd` matches physical positions.

See the [API docs on docs.rs](https://docs.rs/kbd-iced) for the event conversion APIs and mapping tables.

## License

kbd-iced is licensed under the MIT license. See the [`LICENSE`](../../LICENSE) file for more information.
