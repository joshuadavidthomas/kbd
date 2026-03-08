# kbd-crossterm

[![crates.io](https://img.shields.io/crates/v/kbd-crossterm.svg)](https://crates.io/crates/kbd-crossterm)
[![docs.rs](https://docs.rs/kbd-crossterm/badge.svg)](https://docs.rs/kbd-crossterm)

Crossterm key event conversions for the [`kbd` workspace](https://github.com/joshuadavidthomas/kbd).

Use it when a TUI already receives input through crossterm and you want those events to drive a `kbd` dispatcher.

[API docs](https://docs.rs/kbd-crossterm) — includes the full key and modifier mapping tables.

```toml
[dependencies]
kbd = "0.1"
kbd-crossterm = "0.1"
```

## Example

```rust
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use kbd::action::Action;
use kbd::dispatcher::{Dispatcher, MatchResult};
use kbd::key_state::KeyTransition;
use kbd_crossterm::CrosstermEventExt;

let mut dispatcher = Dispatcher::new();
dispatcher.register("Ctrl+C", Action::Suppress)?;

let event = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
let result = dispatcher.process(event.to_hotkey().unwrap(), KeyTransition::Press);
assert!(matches!(result, MatchResult::Matched { .. }));
```

Crossterm reports many keys as characters rather than physical positions, so unmappable inputs return `None`. Modifier keys used as triggers are normalized so they do not include themselves as active modifiers.

## License

kbd-crossterm is licensed under the MIT license. See the [`LICENSE`](../../LICENSE) file for more information.
