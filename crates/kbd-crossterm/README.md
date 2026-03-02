# kbd-crossterm

Crossterm bridge for [`kbd`](https://crates.io/crates/kbd) — converts crossterm
key events and modifiers to `kbd` types.

## Installation

```toml
[dependencies]
kbd = "0.1"
kbd-crossterm = "0.1"
```

## Usage

```rust
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use kbd::{Hotkey, Key, Modifier};
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

## Extension traits

- `CrosstermKeyExt` — converts a `KeyCode` to a `kbd::Key`
- `CrosstermModifiersExt` — converts `KeyModifiers` to `Vec<Modifier>`
- `CrosstermEventExt` — converts a full `KeyEvent` to a `kbd::Hotkey`

## License

MIT
