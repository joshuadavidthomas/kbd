# kbd-crossterm

[![crates.io](https://img.shields.io/crates/v/kbd-crossterm.svg)](https://crates.io/crates/kbd-crossterm)
[![docs.rs](https://docs.rs/kbd-crossterm/badge.svg)](https://docs.rs/kbd-crossterm)

Converts [crossterm](https://docs.rs/crossterm) key events into [`kbd`](https://docs.rs/kbd) types so you can use the same dispatcher, hotkey parsing, layers, and sequences in a TUI app.

[API docs](https://docs.rs/kbd-crossterm) — includes the full key and modifier mapping tables.

## Example

```rust
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use kbd_crossterm::CrosstermEventExt;

let event = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
let hotkey = event.to_hotkey();
// Some(Hotkey { key: Key::C, modifiers: {Ctrl} })
```

The resulting `Hotkey` works with everything in `kbd` — register it as a binding, match it in the dispatcher, use it in a layer. You get one shortcut model for your whole TUI instead of hand-matching crossterm events in every input handler.

Crossterm reports keys as characters rather than physical positions, so some inputs don't have a `kbd` equivalent — `to_hotkey()` returns `None` for those. Modifier keys used as triggers are normalized so they don't include themselves as active modifiers.

## License

kbd-crossterm is licensed under the MIT license. See the [`LICENSE`](../../LICENSE) file for more information.
