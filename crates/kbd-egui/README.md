# kbd-egui

[![crates.io](https://img.shields.io/crates/v/kbd-egui.svg)](https://crates.io/crates/kbd-egui)
[![docs.rs](https://docs.rs/kbd-egui/badge.svg)](https://docs.rs/kbd-egui)

`kbd-egui` converts egui keyboard events into `kbd` types.

Use it when an egui app wants one hotkey model for both in-window shortcuts and external bindings handled by the rest of the `kbd` workspace.

```toml
[dependencies]
kbd = "0.1"
kbd-egui = "0.1"
```

## Example

Convert an egui event and feed it to a dispatcher:

```rust
use egui::{Event, Key as EguiKey, Modifiers};
use kbd::action::Action;
use kbd::dispatcher::{Dispatcher, MatchResult};
use kbd::key_state::KeyTransition;
use kbd_egui::EguiEventExt;

# fn main() -> Result<(), Box<dyn std::error::Error>> {
let mut dispatcher = Dispatcher::new();
dispatcher.register("Ctrl+C", Action::Suppress)?;

let event = Event::Key {
    key: EguiKey::C,
    physical_key: None,
    pressed: true,
    repeat: false,
    modifiers: Modifiers::CTRL,
};

if let Some(hotkey) = event.to_hotkey() {
    let result = dispatcher.process(hotkey, KeyTransition::Press);
    assert!(matches!(result, MatchResult::Matched { .. }));
}
# Ok(())
# }
```

Egui does not expose the full W3C physical-key space. Logical or shifted keys such as `Colon` or `Plus` do not have a single physical-key mapping, so `to_hotkey()` returns `None` for those.

See the [API docs on docs.rs](https://docs.rs/kbd-egui) for the complete mapping details.

## License

kbd-egui is licensed under the MIT license. See the [`LICENSE`](../../LICENSE) file for more information.
