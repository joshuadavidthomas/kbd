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

Convert an iced key code and feed it to a dispatcher:

```rust
use iced_core::keyboard::{key::Code, Modifiers};
use kbd::action::Action;
use kbd::dispatcher::{Dispatcher, MatchResult};
use kbd::hotkey::{Hotkey, Modifier};
use kbd::key_state::KeyTransition;
use kbd_iced::{IcedKeyExt, IcedModifiersExt};

# fn main() -> Result<(), Box<dyn std::error::Error>> {
let mut dispatcher = Dispatcher::new();
dispatcher.register("Ctrl+S", Action::Suppress)?;

let key = Code::KeyS.to_key().unwrap();
let mods = Modifiers::CTRL.to_modifiers();
let hotkey = Hotkey::new(key).modifiers(mods);

let result = dispatcher.process(hotkey, KeyTransition::Press);
assert!(matches!(result, MatchResult::Matched { .. }));
# Ok(())
# }
```

This crate converts iced's physical key types. Logical key values are out of scope — `kbd` matches physical positions.

See the [API docs on docs.rs](https://docs.rs/kbd-iced) for the event conversion APIs and mapping tables.

## License

kbd-iced is licensed under the MIT license. See the [`LICENSE`](../../LICENSE) file for more information.
