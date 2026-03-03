# kbd-crossterm

[![crates.io](https://img.shields.io/crates/v/kbd-crossterm.svg)](https://crates.io/crates/kbd-crossterm)
[![docs.rs](https://docs.rs/kbd-crossterm/badge.svg)](https://docs.rs/kbd-crossterm)

[`kbd`](https://crates.io/crates/kbd) bridge for [crossterm](https://docs.rs/crossterm) — converts key events and modifiers to `kbd` types.

Crossterm reports keys as characters (`Char('a')`) and modifier bitflags, while `kbd` uses physical key positions (`Key::A`) and typed `Modifier` values.

```toml
[dependencies]
kbd = "0.1"
kbd-crossterm = "0.1"
```

## Extension traits

- **`CrosstermKeyExt`** — converts a `crossterm::event::KeyCode` to a `kbd::Key`
- **`CrosstermModifiersExt`** — converts `crossterm::event::KeyModifiers` to a `Vec<Modifier>`
- **`CrosstermEventExt`** — converts a full `crossterm::event::KeyEvent` to a `kbd::Hotkey`

## Usage

```rust
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use kbd::prelude::*;
use kbd_crossterm::{CrosstermEventExt, CrosstermKeyExt, CrosstermModifiersExt};

// Single key conversion
let key = KeyCode::Char('a').to_key();
assert_eq!(key, Some(Key::A));

// Modifier conversion
let mods = KeyModifiers::CONTROL.to_modifiers();
assert_eq!(mods, vec![Modifier::Ctrl]);

// Full event conversion
let event = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
let hotkey = event.to_hotkey();
assert_eq!(hotkey, Some(Hotkey::new(Key::C).modifier(Modifier::Ctrl)));
```

## Key mapping

| Crossterm | kbd | Notes |
|---|---|---|
| `Char('a')` – `Char('z')` | `Key::A` – `Key::Z` | Case-insensitive |
| `Char('0')` – `Char('9')` | `Key::DIGIT0` – `Key::DIGIT9` | |
| `Char('-')`, `Char('=')`, … | `Key::MINUS`, `Key::EQUAL`, … | Physical position |
| `F(1)` – `F(35)` | `Key::F1` – `Key::F35` | `F(0)` and `F(36+)` → `None` |
| `Enter`, `Esc`, `Tab`, … | `Key::ENTER`, `Key::ESCAPE`, `Key::TAB`, … | Named keys |
| `Media(PlayPause)`, … | `Key::MEDIA_PLAY_PAUSE`, … | Media keys |
| `Modifier(LeftControl)`, … | `Key::CONTROL_LEFT`, … | Modifier keys as triggers |
| `BackTab`, `Null`, `KeypadBegin` | `None` | No `kbd` equivalent |
| Non-ASCII `Char` (e.g., `'é'`) | `None` | No physical key mapping |

## Modifier mapping

| Crossterm | kbd |
|---|---|
| `CONTROL` | `Modifier::Ctrl` |
| `SHIFT` | `Modifier::Shift` |
| `ALT` | `Modifier::Alt` |
| `SUPER` | `Modifier::Super` |
| `HYPER`, `META` | *(ignored)* |

## License

MIT
