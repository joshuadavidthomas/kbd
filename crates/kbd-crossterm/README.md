# kbd-crossterm

[![crates.io](https://img.shields.io/crates/v/kbd-crossterm.svg)](https://crates.io/crates/kbd-crossterm)
[![docs.rs](https://docs.rs/kbd-crossterm/badge.svg)](https://docs.rs/kbd-crossterm)

`kbd-crossterm` converts crossterm key types into `kbd` types so you can feed terminal input into `kbd::dispatcher::Dispatcher`.

```toml
[dependencies]
kbd = "0.1"
kbd-crossterm = "0.1"
```

## Public API

- `CrosstermKeyExt` converts `crossterm::event::KeyCode` to `kbd::key::Key`
- `CrosstermModifiersExt` converts `crossterm::event::KeyModifiers` to `kbd::hotkey::ModifierSet`
- `CrosstermEventExt` converts `crossterm::event::KeyEvent` to `kbd::hotkey::Hotkey`

## Example

```rust
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use kbd::hotkey::{Hotkey, Modifier};
use kbd::key::Key;
use kbd_crossterm::{CrosstermEventExt, CrosstermKeyExt, CrosstermModifiersExt};

let key = KeyCode::Char('a').to_key();
assert_eq!(key, Some(Key::A));

let mods = KeyModifiers::CONTROL.to_modifiers();
assert!(mods.contains(Modifier::Ctrl));
assert_eq!(mods.len(), 1);

let event = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
let hotkey = event.to_hotkey();
assert_eq!(hotkey, Some(Hotkey::new(Key::C).modifier(Modifier::Ctrl)));
```

## Mapping notes

- crossterm reports keys as characters, while `kbd` models physical key positions
- modifier conversion yields a compact `ModifierSet`
- unsupported or non-physical inputs return `None`
- modifier keys used as triggers are normalized so they do not include themselves as active modifiers

See the [API docs on docs.rs](https://docs.rs/kbd-crossterm) for the full mapping tables and examples.

## License

kbd-crossterm is licensed under the MIT license. See the [`LICENSE`](../../LICENSE) file for more information.
