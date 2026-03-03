# kbd-iced

[![crates.io](https://img.shields.io/crates/v/kbd-iced.svg)](https://crates.io/crates/kbd-iced)
[![docs.rs](https://docs.rs/kbd-iced/badge.svg)](https://docs.rs/kbd-iced)

[`kbd`](https://crates.io/crates/kbd) bridge for [iced](https://docs.rs/iced) — converts key events and modifiers to `kbd` types.

Iced defines its own W3C-derived key types: `key::Code` for physical key positions and `key::Physical` wrapping `Code` with an unidentified fallback. This crate only converts physical keys — they are layout-independent and match `kbd`'s model.

```toml
[dependencies]
kbd = "0.1"
kbd-iced = "0.1"
```

## Extension traits

- **`IcedKeyExt`** — converts an iced `key::Code` or `key::Physical` to a `kbd::Key`
- **`IcedModifiersExt`** — converts iced `Modifiers` to a `Vec<Modifier>`
- **`IcedEventExt`** — converts an iced keyboard `Event` to a `kbd::Hotkey`

## Usage

```rust
use iced_core::keyboard::{key::Code, Modifiers};
use kbd::prelude::*;
use kbd_iced::{IcedKeyExt, IcedModifiersExt};

let key = Code::KeyA.to_key();
assert_eq!(key, Some(Key::A));

let mods = Modifiers::CTRL.to_modifiers();
assert_eq!(mods, vec![Modifier::Ctrl]);
```

## Key mapping

| iced | kbd | Notes |
|---|---|---|
| `Code::KeyA` – `Code::KeyZ` | `Key::A` – `Key::Z` | Letters |
| `Code::Digit0` – `Code::Digit9` | `Key::DIGIT0` – `Key::DIGIT9` | Digits |
| `Code::F1` – `Code::F35` | `Key::F1` – `Key::F35` | Function keys |
| `Code::Numpad0` – `Code::Numpad9` | `Key::NUMPAD0` – `Key::NUMPAD9` | Numpad |
| `Code::Enter`, `Code::Escape`, … | `Key::ENTER`, `Key::ESCAPE`, … | Navigation / editing |
| `Code::ControlLeft`, … | `Key::CONTROL_LEFT`, … | Modifier keys as triggers |
| `Code::SuperLeft` / `Code::Meta` | `Key::META_LEFT` | iced's Super = kbd's Meta |
| `Code::MediaPlayPause`, … | `Key::MEDIA_PLAY_PAUSE`, … | Media keys |
| `Code::BrowserBack`, … | `Key::BROWSER_BACK`, … | Browser keys |
| `Physical::Unidentified(_)` | `None` | No mapping possible |

## Modifier mapping

| iced | kbd |
|---|---|
| `CTRL` | `Modifier::Ctrl` |
| `SHIFT` | `Modifier::Shift` |
| `ALT` | `Modifier::Alt` |
| `LOGO` | `Modifier::Super` |

## License

MIT
