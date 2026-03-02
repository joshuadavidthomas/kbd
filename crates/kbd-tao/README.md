# kbd-tao

Tao bridge for [`kbd`](https://crates.io/crates/kbd) — converts tao key events and modifiers to `kbd` types.

[Tao](https://github.com/nicegui-dev/nicegui-tao) is Tauri's fork of winit.

## Installation

```toml
[dependencies]
kbd = "0.1"
kbd-tao = "0.1"
```

## Usage

```rust
use kbd::{Key, Modifier};
use kbd_tao::{TaoKeyExt, TaoModifiersExt};
use tao::keyboard::{KeyCode, ModifiersState};

// KeyCode conversion
let key = KeyCode::KeyA.to_key();
assert_eq!(key, Some(Key::A));

// Modifier conversion
let mods = ModifiersState::CONTROL.to_modifiers();
assert_eq!(mods, vec![Modifier::Ctrl]);
```

## Extension traits

- `TaoKeyExt` — converts a tao `KeyCode` to a `kbd::Key`
- `TaoModifiersExt` — converts `ModifiersState` to `Vec<Modifier>`
- `TaoEventExt` — converts a `KeyEvent` + `ModifiersState` to a `kbd::Hotkey`

## License

MIT
