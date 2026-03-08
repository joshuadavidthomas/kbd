# kbd-winit

[![crates.io](https://img.shields.io/crates/v/kbd-winit.svg)](https://crates.io/crates/kbd-winit)
[![docs.rs](https://docs.rs/kbd-winit/badge.svg)](https://docs.rs/kbd-winit)

`kbd-winit` converts winit keyboard events into `kbd` types.

Use it when a winit application wants one hotkey representation for window input and for the rest of the `kbd` stack.

```toml
[dependencies]
kbd = "0.1"
kbd-winit = "0.1"
```

## Example

Convert a winit key code and feed it to a dispatcher:

```rust
use kbd::action::Action;
use kbd::dispatcher::{Dispatcher, MatchResult};
use kbd::hotkey::{Hotkey, Modifier};
use kbd::key_state::KeyTransition;
use kbd_winit::{WinitKeyExt, WinitModifiersExt};
use winit::keyboard::{KeyCode, ModifiersState};

# fn main() -> Result<(), Box<dyn std::error::Error>> {
let mut dispatcher = Dispatcher::new();
dispatcher.register("Ctrl+S", Action::Suppress)?;

// Convert winit types to kbd types
let key = KeyCode::KeyS.to_key().unwrap();
let mods = ModifiersState::CONTROL.to_modifiers();
let hotkey = Hotkey::new(key).modifiers(mods);

let result = dispatcher.process(hotkey, KeyTransition::Press);
assert!(matches!(result, MatchResult::Matched { .. }));
# Ok(())
# }
```

Winit tracks modifiers separately from key events. When you convert a full `KeyEvent`, use [`WinitEventExt`](https://docs.rs/kbd-winit/latest/kbd_winit/trait.WinitEventExt.html) and pass in the latest `ModifiersState` from `WindowEvent::ModifiersChanged`.

See the [API docs on docs.rs](https://docs.rs/kbd-winit) for the full event-loop example and mapping tables.

## License

kbd-winit is licensed under the MIT license. See the [`LICENSE`](../../LICENSE) file for more information.
