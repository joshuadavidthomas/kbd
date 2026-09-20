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

## Keyboard observations

`event.to_observation()` borrows a `KeyEvent` and returns a
`kbd::observation::KeyboardObservation` for `Dispatcher::process_event`.
**Physical identity is always absent**, including for ASCII letters, named keys,
and keypad events. Characters preserve the supplied scalar and case: `A` without
Shift and `a` with Shift remain different observations. Shifted punctuation and
Unicode characters such as `+` and `λ` work as logical identities even when the
legacy hotkey conversion returns `None`.

Named keys, media keys and F1–F35 are converted explicitly. ISO level 3 maps to
the logical AltGraph key, not an AltGr modifier flag. BackTab, Null, KeypadBegin,
media Reverse, ISO level 5 and unsupported function keys have no exact mapping;
their logical identity remains absent. Modifiers are not self-stripped.

**Modifier and transition knowledge is incomplete.** The existing modifier set
cannot represent Hyper/Meta flags or AltGr/Fn state. Unix repeat/release reporting
requires terminal enhancement support; a default Press does not prove a fresh
hardware press. Keep the source for keypad and lock-state evidence; absent flags
do not establish standard location or inactive locks. Handle paste separately;
this conversion neither reconstructs IME nor synthesizes text events.

`to_hotkey()` retains its legacy US-label projection and normalization unchanged.

## License

kbd-crossterm is licensed under the MIT license. See the [`LICENSE`](../../LICENSE) file for more information.
