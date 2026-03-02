# kbd-tao

[![crates.io](https://img.shields.io/crates/v/kbd-tao.svg)](https://crates.io/crates/kbd-tao)
[![docs.rs](https://docs.rs/kbd-tao/badge.svg)](https://docs.rs/kbd-tao)

[`kbd`](https://crates.io/crates/kbd) bridge for [tao](https://docs.rs/tao) (Tauri's winit fork) — converts key events and modifiers to `kbd` types.

```toml
[dependencies]
kbd = "0.1"
kbd-tao = "0.1"
```

```rust
use kbd::{Key, Modifier};
use kbd_tao::{TaoKeyExt, TaoModifiersExt};
use tao::keyboard::{KeyCode, ModifiersState};

let key = KeyCode::KeyA.to_key();
assert_eq!(key, Some(Key::A));

let mods = ModifiersState::CONTROL.to_modifiers();
assert_eq!(mods, vec![Modifier::Ctrl]);
```

## License

MIT
