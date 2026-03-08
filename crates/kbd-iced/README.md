# kbd-iced

[![crates.io](https://img.shields.io/crates/v/kbd-iced.svg)](https://crates.io/crates/kbd-iced)
[![docs.rs](https://docs.rs/kbd-iced/badge.svg)](https://docs.rs/kbd-iced)

Converts [iced](https://docs.rs/iced) key events into [`kbd`](https://docs.rs/kbd) types so that in-window shortcuts and global hotkeys (from [`kbd-global`](https://docs.rs/kbd-global)) can share the same dispatcher.

[API docs](https://docs.rs/kbd-iced) — includes the full key and modifier mapping tables.

```toml
[dependencies]
kbd = "0.1"
kbd-iced = "0.1"
```

## Example

```rust
use iced_core::keyboard::{key::Code, Modifiers};
use kbd_iced::{IcedKeyExt, IcedModifiersExt};

let key = Code::KeyS.to_key();
// Some(Key::S)

let mods = Modifiers::CTRL.to_modifiers();
// ModifierSet containing Modifier::Ctrl

let hotkey = kbd::hotkey::Hotkey::new(key.unwrap()).modifiers(mods);
```

Once converted, the `Hotkey` plugs into everything `kbd` offers — register bindings with strings, stack layers for modal shortcuts, define multi-step sequences. One shortcut model for both your iced UI and any system-wide hotkeys you add later.

This crate converts iced's physical key types. Logical key values are out of scope — `kbd` matches physical positions.

## License

kbd-iced is licensed under the MIT license. See the [`LICENSE`](../../LICENSE) file for more information.
