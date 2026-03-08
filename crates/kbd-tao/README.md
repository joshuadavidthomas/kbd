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

```rust
use kbd::hotkey::Modifier;
use kbd::key::Key;
use kbd_tao::{TaoKeyExt, TaoModifiersExt};
use tao::keyboard::{KeyCode, ModifiersState};

let key = KeyCode::KeyA.to_key();
assert_eq!(key, Some(Key::A));

let mods = ModifiersState::CONTROL.to_modifiers();
assert!(mods.contains(Modifier::Ctrl));
```

Tao tracks modifiers separately from key events. When you convert a full `KeyEvent`, pass in the latest `ModifiersState` you received from the event loop.

See the [API docs on docs.rs](https://docs.rs/kbd-tao) for the event-loop example and mapping tables.

## License

kbd-tao is licensed under the MIT license. See the [`LICENSE`](../../LICENSE) file for more information.
