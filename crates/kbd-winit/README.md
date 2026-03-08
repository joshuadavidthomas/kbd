# kbd-winit

[![crates.io](https://img.shields.io/crates/v/kbd-winit.svg)](https://crates.io/crates/kbd-winit)
[![docs.rs](https://docs.rs/kbd-winit/badge.svg)](https://docs.rs/kbd-winit)

`kbd-winit` converts winit keyboard events into `kbd` types so applications can share one hotkey model between window input and global hotkeys.

```toml
[dependencies]
kbd = "0.1"
kbd-winit = "0.1"
```

## Public API

- `WinitKeyExt` converts `winit::keyboard::KeyCode` and `PhysicalKey` to `kbd::key::Key`
- `WinitModifiersExt` converts `winit::keyboard::ModifiersState` to `kbd::hotkey::ModifierSet`
- `WinitEventExt` converts `winit::event::KeyEvent` plus `ModifiersState` to `kbd::hotkey::Hotkey`
- `winit_key_to_hotkey(...)` provides the standalone helper used by the event trait

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
assert_eq!(mods.len(), 1);
```

## Mapping notes

- winit key codes map closely to `kbd`'s physical key model
- modifier conversion yields a compact `ModifierSet`
- modifier keys used as triggers are normalized so they do not include themselves as active modifiers
- `PhysicalKey::Unidentified(_)` returns `None`

See the [API docs on docs.rs](https://docs.rs/kbd-winit) for the full event-loop example and mapping tables.

## License

kbd-winit is licensed under the MIT license. See the [`LICENSE`](../../LICENSE) file for more information.
