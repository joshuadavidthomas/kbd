# kbd-iced

[![crates.io](https://img.shields.io/crates/v/kbd-iced.svg)](https://crates.io/crates/kbd-iced)
[![docs.rs](https://docs.rs/kbd-iced/badge.svg)](https://docs.rs/kbd-iced)

Iced key event conversions for the [`kbd` workspace](https://github.com/joshuadavidthomas/kbd).

Use it when an iced application wants to keep its in-window shortcuts in the same hotkey format used by `kbd` and `kbd-global`.

[API docs](https://docs.rs/kbd-iced) — includes the full key and modifier mapping tables.

```toml
[dependencies]
kbd = "0.1"
kbd-iced = "0.1"
```

## Example

```rust
use iced_core::keyboard::{key::Code, Modifiers};
use kbd::action::Action;
use kbd::dispatcher::{Dispatcher, MatchResult};
use kbd::hotkey::{Hotkey, Modifier};
use kbd::key_state::KeyTransition;
use kbd_iced::{IcedKeyExt, IcedModifiersExt};

let mut dispatcher = Dispatcher::new();
dispatcher.register("Ctrl+S", Action::Suppress)?;

let key = Code::KeyS.to_key().unwrap();
let mods = Modifiers::CTRL.to_modifiers();
let hotkey = Hotkey::new(key).modifiers(mods);

let result = dispatcher.process(hotkey, KeyTransition::Press);
assert!(matches!(result, MatchResult::Matched { .. }));
```

This crate converts iced's physical key types. Logical key values are out of scope — `kbd` matches physical positions.

## License

kbd-iced is licensed under the MIT license. See the [`LICENSE`](../../LICENSE) file for more information.
