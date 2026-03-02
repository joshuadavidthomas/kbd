# kbd-crossterm

[![crates.io](https://img.shields.io/crates/v/kbd-crossterm.svg)](https://crates.io/crates/kbd-crossterm)
[![docs.rs](https://docs.rs/kbd-crossterm/badge.svg)](https://docs.rs/kbd-crossterm)

[`kbd`](https://crates.io/crates/kbd) bridge for [crossterm](https://docs.rs/crossterm) — converts key events and modifiers to `kbd` types.

```toml
[dependencies]
kbd = "0.1"
kbd-crossterm = "0.1"
```

```rust
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use kbd::{Hotkey, Key, Modifier};
use kbd_crossterm::{CrosstermEventExt, CrosstermKeyExt, CrosstermModifiersExt};

let key = KeyCode::Char('a').to_key();
assert_eq!(key, Some(Key::A));

let event = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
let hotkey = event.to_hotkey();
assert_eq!(hotkey, Some(Hotkey::new(Key::C).modifier(Modifier::Ctrl)));
```

## License

MIT
