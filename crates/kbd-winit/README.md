# kbd-winit

[![crates.io](https://img.shields.io/crates/v/kbd-winit.svg)](https://crates.io/crates/kbd-winit)
[![docs.rs](https://docs.rs/kbd-winit/badge.svg)](https://docs.rs/kbd-winit)

[`kbd`](https://crates.io/crates/kbd) bridge for [winit](https://docs.rs/winit) — converts key events and modifiers to `kbd` types. Winit tracks modifiers separately via `WindowEvent::ModifiersChanged`, so `WinitEventExt` takes `ModifiersState` as a parameter.

```toml
[dependencies]
kbd = "0.1"
kbd-winit = "0.1"
```

```rust
use kbd::{Key, Modifier};
use kbd_winit::{WinitKeyExt, WinitModifiersExt};
use winit::keyboard::{KeyCode, ModifiersState, PhysicalKey};

let key = KeyCode::KeyA.to_key();
assert_eq!(key, Some(Key::A));

let key = PhysicalKey::Code(KeyCode::KeyA).to_key();
assert_eq!(key, Some(Key::A));

let mods = ModifiersState::CONTROL.to_modifiers();
assert_eq!(mods, vec![Modifier::Ctrl]);
```

## License

MIT
