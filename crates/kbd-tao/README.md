# kbd-tao

[![crates.io](https://img.shields.io/crates/v/kbd-tao.svg)](https://crates.io/crates/kbd-tao)
[![docs.rs](https://docs.rs/kbd-tao/badge.svg)](https://docs.rs/kbd-tao)

`kbd-tao` converts tao keyboard events into `kbd` types.

Use it when a tao or Tauri application wants one hotkey representation for window input and the rest of the `kbd` stack.

```toml
[dependencies]
kbd = "0.1"
kbd-tao = "0.1"
```

## Example

Convert tao key types and feed them to a dispatcher:

```rust
use kbd::action::Action;
use kbd::dispatcher::{Dispatcher, MatchResult};
use kbd::hotkey::{Hotkey, Modifier};
use kbd::key_state::KeyTransition;
use kbd_tao::{TaoKeyExt, TaoModifiersExt};
use tao::keyboard::{KeyCode, ModifiersState};

# fn main() -> Result<(), Box<dyn std::error::Error>> {
let mut dispatcher = Dispatcher::new();
dispatcher.register("Ctrl+S", Action::Suppress)?;

let key = KeyCode::KeyS.to_key().unwrap();
let mods = ModifiersState::CONTROL.to_modifiers();
let hotkey = Hotkey::new(key).modifiers(mods);

let result = dispatcher.process(hotkey, KeyTransition::Press);
assert!(matches!(result, MatchResult::Matched { .. }));
# Ok(())
# }
```

Tao tracks modifiers separately from key events. When you convert a full `KeyEvent`, use [`TaoEventExt`](https://docs.rs/kbd-tao/latest/kbd_tao/trait.TaoEventExt.html) and pass in the latest `ModifiersState` from the event loop.

See the [API docs on docs.rs](https://docs.rs/kbd-tao) for the event-loop example and mapping tables.

## License

kbd-tao is licensed under the MIT license. See the [`LICENSE`](../../LICENSE) file for more information.
