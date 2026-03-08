# kbd-crossterm

[![crates.io](https://img.shields.io/crates/v/kbd-crossterm.svg)](https://crates.io/crates/kbd-crossterm)
[![docs.rs](https://docs.rs/kbd-crossterm/badge.svg)](https://docs.rs/kbd-crossterm)

`kbd-crossterm` converts crossterm key events into `kbd` types.

Use it when a TUI already receives input through crossterm and you want those events to drive the same `kbd::dispatcher::Dispatcher` you use elsewhere.

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

# fn main() -> Result<(), Box<dyn std::error::Error>> {
let mut dispatcher = Dispatcher::new();
dispatcher.register("Ctrl+C", Action::Suppress)?;

let event = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
let result = dispatcher.process(event.to_hotkey().unwrap(), KeyTransition::Press);
assert!(matches!(result, MatchResult::Matched { .. }));
# Ok(())
# }
```

Crossterm reports many keys as characters rather than physical positions, so unmappable inputs return `None`. Modifier keys used as triggers are normalized so they do not include themselves as active modifiers.

See the [API docs on docs.rs](https://docs.rs/kbd-crossterm) for mapping details and trait-level examples.

## License

kbd-crossterm is licensed under the MIT license. See the [`LICENSE`](../../LICENSE) file for more information.
