# kbd-winit

[![crates.io](https://img.shields.io/crates/v/kbd-winit.svg)](https://crates.io/crates/kbd-winit)
[![docs.rs](https://docs.rs/kbd-winit/badge.svg)](https://docs.rs/kbd-winit)

Winit key event conversions for the [`kbd` workspace](https://github.com/joshuadavidthomas/kbd).

Use it when a winit application wants one hotkey representation for window input and the rest of the `kbd` stack.

[API docs](https://docs.rs/kbd-winit) — includes the full key and modifier mapping tables and an event-loop example.

```toml
[dependencies]
kbd = "0.1"
kbd-winit = "0.1"
```

## Example

```rust
use kbd::action::Action;
use kbd::dispatcher::{Dispatcher, MatchResult};
use kbd::hotkey::{Hotkey, Modifier};
use kbd::key_state::KeyTransition;
use kbd_winit::{WinitKeyExt, WinitModifiersExt};
use winit::keyboard::{KeyCode, ModifiersState};

let mut dispatcher = Dispatcher::new();
dispatcher.register("Ctrl+S", Action::Suppress)?;

let key = KeyCode::KeyS.to_key().unwrap();
let mods = ModifiersState::CONTROL.to_modifiers();
let hotkey = Hotkey::new(key).modifiers(mods);

let result = dispatcher.process(hotkey, KeyTransition::Press);
assert!(matches!(result, MatchResult::Matched { .. }));
```

Winit tracks modifiers separately from key events. When you convert a full `KeyEvent`, use [`WinitEventExt`](https://docs.rs/kbd-winit/latest/kbd_winit/trait.WinitEventExt.html) and pass in the latest `ModifiersState` from `WindowEvent::ModifiersChanged`.

## License

kbd-winit is licensed under the MIT license. See the [`LICENSE`](../../LICENSE) file for more information.
