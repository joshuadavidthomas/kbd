# kbd-tao

[![crates.io](https://img.shields.io/crates/v/kbd-tao.svg)](https://crates.io/crates/kbd-tao)
[![docs.rs](https://docs.rs/kbd-tao/badge.svg)](https://docs.rs/kbd-tao)

Converts [tao](https://docs.rs/tao) key events into [`kbd`](https://docs.rs/kbd) types. Especially useful in Tauri apps that want both in-window shortcuts and system-wide hotkeys (via [`kbd-global`](https://docs.rs/kbd-global)) through a single dispatcher.

[API docs](https://docs.rs/kbd-tao) — includes the full key and modifier mapping tables and an event-loop example.

```toml
[dependencies]
kbd = "0.1"
kbd-tao = "0.1"
```

## Example

```rust
use tao::keyboard::{KeyCode, ModifiersState};
use kbd_tao::{TaoKeyExt, TaoModifiersExt};

let key = KeyCode::KeyS.to_key();
// Some(Key::S)

let mods = ModifiersState::CONTROL.to_modifiers();
// ModifierSet containing Modifier::Ctrl

// Combine into a Hotkey for use with a Dispatcher
let hotkey = kbd::hotkey::Hotkey::new(key.unwrap()).modifiers(mods);
```

Tao tracks modifiers separately from key events. For full `KeyEvent` conversion inside an event loop, use [`TaoEventExt`](https://docs.rs/kbd-tao/latest/kbd_tao/trait.TaoEventExt.html) — it takes the latest `ModifiersState` as a parameter.

## License

kbd-tao is licensed under the MIT license. See the [`LICENSE`](../../LICENSE) file for more information.
