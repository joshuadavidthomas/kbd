# kbd-tao

[![crates.io](https://img.shields.io/crates/v/kbd-tao.svg)](https://crates.io/crates/kbd-tao)
[![docs.rs](https://docs.rs/kbd-tao/badge.svg)](https://docs.rs/kbd-tao)

[`kbd`](https://crates.io/crates/kbd) bridge for [tao](https://docs.rs/tao) (Tauri's winit fork) — converts key events and modifiers to `kbd` types.

Tao is Tauri's fork of winit — both derive from the W3C UI Events specification, so the variant names are nearly identical and the mapping is mechanical. Unlike winit, tao uses `KeyCode` directly in `KeyEvent` rather than wrapping it in a `PhysicalKey` type.

```toml
[dependencies]
kbd = "0.1"
kbd-tao = "0.1"
```

## Extension traits

- **`TaoKeyExt`** — converts a tao `KeyCode` to a `kbd::Key`
- **`TaoModifiersExt`** — converts tao `ModifiersState` to a `Vec<Modifier>`
- **`TaoEventExt`** — converts a tao `KeyEvent` plus `ModifiersState` to a `kbd::Hotkey`

## Usage

```rust
use kbd::key::{Key, Modifier};
use kbd_tao::{TaoKeyExt, TaoModifiersExt};
use tao::keyboard::{KeyCode, ModifiersState};

let key = KeyCode::KeyA.to_key();
assert_eq!(key, Some(Key::A));

let mods = ModifiersState::CONTROL.to_modifiers();
assert_eq!(mods, vec![Modifier::Ctrl]);
```

## Key mapping

| tao | kbd | Notes |
|---|---|---|
| `KeyCode::KeyA` – `KeyCode::KeyZ` | `Key::A` – `Key::Z` | Letters |
| `KeyCode::Digit0` – `KeyCode::Digit9` | `Key::DIGIT0` – `Key::DIGIT9` | Digits |
| `KeyCode::F1` – `KeyCode::F35` | `Key::F1` – `Key::F35` | Function keys |
| `KeyCode::Numpad0` – `KeyCode::Numpad9` | `Key::NUMPAD0` – `Key::NUMPAD9` | Numpad |
| `KeyCode::Enter`, `KeyCode::Escape`, … | `Key::ENTER`, `Key::ESCAPE`, … | Navigation / editing |
| `KeyCode::ControlLeft`, … | `Key::CONTROL_LEFT`, … | Modifier keys as triggers |
| `KeyCode::SuperLeft` / `KeyCode::SuperRight` | `Key::META_LEFT` / `Key::META_RIGHT` | tao's Super = kbd's Meta |
| `KeyCode::Equal` / `KeyCode::Plus` | `Key::EQUAL` | Same physical key |
| `KeyCode::MediaPlayPause`, … | `Key::MEDIA_PLAY_PAUSE`, … | Media keys |
| `KeyCode::BrowserBack`, … | `Key::BROWSER_BACK`, … | Browser keys |
| `KeyCode::Unidentified(_)` | `None` | No mapping possible |

## Modifier mapping

| tao | kbd |
|---|---|
| `CONTROL` | `Modifier::Ctrl` |
| `SHIFT` | `Modifier::Shift` |
| `ALT` | `Modifier::Alt` |
| `SUPER` | `Modifier::Super` |

## License

MIT
