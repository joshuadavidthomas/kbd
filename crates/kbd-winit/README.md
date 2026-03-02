# kbd-winit

[![crates.io](https://img.shields.io/crates/v/kbd-winit.svg)](https://crates.io/crates/kbd-winit)
[![docs.rs](https://docs.rs/kbd-winit/badge.svg)](https://docs.rs/kbd-winit)

[`kbd`](https://crates.io/crates/kbd) bridge for [winit](https://docs.rs/winit) — converts key events and modifiers to `kbd` types.

Both winit and `kbd` derive from the W3C UI Events specification, so the variant names are nearly identical and the mapping is mechanical. Winit tracks modifiers separately via `WindowEvent::ModifiersChanged`, so `WinitEventExt` takes `ModifiersState` as a parameter.

```toml
[dependencies]
kbd = "0.1"
kbd-winit = "0.1"
```

## Extension traits

- **`WinitKeyExt`** — converts a winit `PhysicalKey` or `KeyCode` to a `kbd::Key`
- **`WinitModifiersExt`** — converts winit `ModifiersState` to a `Vec<Modifier>`
- **`WinitEventExt`** — converts a winit `KeyEvent` plus `ModifiersState` to a `kbd::Hotkey`

## Usage

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

## Key mapping

| winit | kbd | Notes |
|---|---|---|
| `KeyCode::KeyA` – `KeyCode::KeyZ` | `Key::A` – `Key::Z` | Letters |
| `KeyCode::Digit0` – `KeyCode::Digit9` | `Key::DIGIT0` – `Key::DIGIT9` | Digits |
| `KeyCode::F1` – `KeyCode::F35` | `Key::F1` – `Key::F35` | Function keys |
| `KeyCode::Numpad0` – `KeyCode::Numpad9` | `Key::NUMPAD0` – `Key::NUMPAD9` | Numpad |
| `KeyCode::Enter`, `KeyCode::Escape`, … | `Key::ENTER`, `Key::ESCAPE`, … | Navigation / editing |
| `KeyCode::ControlLeft`, … | `Key::CONTROL_LEFT`, … | Modifier keys as triggers |
| `KeyCode::SuperLeft` / `KeyCode::Meta` | `Key::META_LEFT` | winit's Super = kbd's Meta |
| `KeyCode::SuperRight` | `Key::META_RIGHT` | |
| `KeyCode::MediaPlayPause`, … | `Key::MEDIA_PLAY_PAUSE`, … | Media keys |
| `KeyCode::BrowserBack`, … | `Key::BROWSER_BACK`, … | Browser keys |
| `PhysicalKey::Unidentified(_)` | `None` | No mapping possible |

## Modifier mapping

| winit | kbd |
|---|---|
| `CONTROL` | `Modifier::Ctrl` |
| `SHIFT` | `Modifier::Shift` |
| `ALT` | `Modifier::Alt` |
| `SUPER` | `Modifier::Super` |

## License

MIT
