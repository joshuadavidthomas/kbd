# kbd-tao

[![crates.io](https://img.shields.io/crates/v/kbd-tao.svg)](https://crates.io/crates/kbd-tao)
[![docs.rs](https://docs.rs/kbd-tao/badge.svg)](https://docs.rs/kbd-tao)

`kbd-tao` converts tao keyboard events into `kbd` types so tao apps can share one hotkey model between in-window input and global hotkeys.

```toml
[dependencies]
kbd = "0.1"
kbd-tao = "0.1"
```

## Public API

- `TaoKeyExt` converts `tao::keyboard::KeyCode` to `kbd::key::Key`
- `TaoModifiersExt` converts `tao::keyboard::ModifiersState` to `kbd::hotkey::ModifierSet`
- `TaoEventExt` converts `tao::event::KeyEvent` plus `ModifiersState` to `kbd::hotkey::Hotkey`
- `tao_key_to_hotkey(...)` provides the standalone helper used by the event trait

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
assert_eq!(mods.len(), 1);
```

## Mapping notes

- tao key codes map mechanically to `kbd`'s physical key model
- tao `SUPER` maps to `kbd::hotkey::Modifier::Super`
- modifier conversion yields a compact `ModifierSet`
- modifier keys used as triggers are normalized so they do not include themselves as active modifiers

See the [API docs on docs.rs](https://docs.rs/kbd-tao) for the full event-loop example and mapping tables.

## License

kbd-tao is licensed under the MIT license. See the [`LICENSE`](../../LICENSE) file for more information.
