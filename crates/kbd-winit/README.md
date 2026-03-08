# kbd-winit

[![crates.io](https://img.shields.io/crates/v/kbd-winit.svg)](https://crates.io/crates/kbd-winit)
[![docs.rs](https://docs.rs/kbd-winit/badge.svg)](https://docs.rs/kbd-winit)

Converts [winit](https://docs.rs/winit) key events into [`kbd`](https://docs.rs/kbd) types so that in-window shortcuts and global hotkeys (from [`kbd-global`](https://docs.rs/kbd-global)) can share the same dispatcher.

[API docs](https://docs.rs/kbd-winit) — includes the full key and modifier mapping tables and an event-loop example.

```toml
[dependencies]
kbd = "0.1"
kbd-winit = "0.1"
```

## Example

```rust
use winit::keyboard::{KeyCode, ModifiersState};
use kbd_winit::{WinitKeyExt, WinitModifiersExt};

let key = KeyCode::KeyS.to_key();
// Some(Key::S)

let mods = ModifiersState::CONTROL.to_modifiers();
// ModifierSet containing Modifier::Ctrl

// Combine into a Hotkey for use with a Dispatcher
let hotkey = kbd::hotkey::Hotkey::new(key.unwrap()).modifiers(mods);
```

Winit tracks modifiers separately from key events. For full `KeyEvent` conversion inside an event loop, use [`WinitEventExt`](https://docs.rs/kbd-winit/latest/kbd_winit/trait.WinitEventExt.html) — it takes the latest `ModifiersState` from `WindowEvent::ModifiersChanged`.

## License

kbd-winit is licensed under the MIT license. See the [`LICENSE`](../../LICENSE) file for more information.
