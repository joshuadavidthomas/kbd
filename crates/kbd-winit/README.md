# kbd-winit

Winit bridge for [`kbd`](https://crates.io/crates/kbd) — converts winit key
events and modifiers to `kbd` types.

## Installation

```toml
[dependencies]
kbd = "0.1"
kbd-winit = "0.1"
```

## Usage

```rust
use kbd::{Hotkey, Key, Modifier};
use kbd_winit::{WinitKeyExt, WinitModifiersExt};
use winit::keyboard::{KeyCode, ModifiersState, PhysicalKey};

// KeyCode conversion
let key = KeyCode::KeyA.to_key();
assert_eq!(key, Some(Key::A));

// PhysicalKey conversion
let key = PhysicalKey::Code(KeyCode::KeyA).to_key();
assert_eq!(key, Some(Key::A));

// Modifier conversion
let mods = ModifiersState::CONTROL.to_modifiers();
assert_eq!(mods, vec![Modifier::Ctrl]);
```

Winit's `KeyEvent` does not carry modifier state — modifiers are tracked
separately via `WindowEvent::ModifiersChanged`. The `WinitEventExt` trait
takes `ModifiersState` as a parameter.

## Extension traits

- `WinitKeyExt` — converts a `PhysicalKey` or `KeyCode` to a `kbd::Key`
- `WinitModifiersExt` — converts `ModifiersState` to `Vec<Modifier>`
- `WinitEventExt` — converts a `KeyEvent` + `ModifiersState` to a `kbd::Hotkey`

## License

MIT
