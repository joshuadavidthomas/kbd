# kbd-iced

[![crates.io](https://img.shields.io/crates/v/kbd-iced.svg)](https://crates.io/crates/kbd-iced)
[![docs.rs](https://docs.rs/kbd-iced/badge.svg)](https://docs.rs/kbd-iced)

[`kbd`](https://crates.io/crates/kbd) bridge for [iced](https://docs.rs/iced) — converts key events and modifiers to `kbd` types.

```toml
[dependencies]
kbd = "0.1"
kbd-iced = "0.1"
```

```rust
use iced_core::keyboard::{key::Code, Modifiers};
use kbd::{Key, Modifier};
use kbd_iced::{IcedKeyExt, IcedModifiersExt};

let key = Code::KeyA.to_key();
assert_eq!(key, Some(Key::A));

let mods = Modifiers::CTRL.to_modifiers();
assert_eq!(mods, vec![Modifier::Ctrl]);
```

## License

MIT
