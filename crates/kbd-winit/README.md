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

```rust
use kbd::hotkey::Modifier;
use kbd::key::Key;
use kbd_winit::{WinitKeyExt, WinitModifiersExt};
use winit::keyboard::{KeyCode, ModifiersState, PhysicalKey};

let key = KeyCode::KeyA.to_key();
assert_eq!(key, Some(Key::A));

let physical = PhysicalKey::Code(KeyCode::KeyA).to_key();
assert_eq!(physical, Some(Key::A));

let mods = ModifiersState::CONTROL.to_modifiers();
assert!(mods.contains(Modifier::Ctrl));
```

Winit tracks modifiers separately from key events. When you convert a full `KeyEvent`, pass in the latest `ModifiersState` from `WindowEvent::ModifiersChanged`.

See the [API docs on docs.rs](https://docs.rs/kbd-winit) for the full event-loop example and mapping tables.

## License

kbd-winit is licensed under the MIT license. See the [`LICENSE`](../../LICENSE) file for more information.
