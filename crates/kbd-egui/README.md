# kbd-egui

[![crates.io](https://img.shields.io/crates/v/kbd-egui.svg)](https://crates.io/crates/kbd-egui)
[![docs.rs](https://docs.rs/kbd-egui/badge.svg)](https://docs.rs/kbd-egui)

Egui key event conversions for the [`kbd` workspace](https://github.com/joshuadavidthomas/kbd).

Use it when an egui app wants one hotkey model for both in-window shortcuts and external bindings handled by `kbd-global`.

[API docs](https://docs.rs/kbd-egui) — includes the full key and modifier mapping tables.

```toml
[dependencies]
kbd = "0.1"
kbd-egui = "0.1"
```

## Example

```rust
use egui::{Event, Key as EguiKey, Modifiers};
use kbd::action::Action;
use kbd::dispatcher::{Dispatcher, MatchResult};
use kbd::key_state::KeyTransition;
use kbd_egui::EguiEventExt;

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
```

Egui does not expose the full W3C physical-key space. Logical or shifted keys such as `Colon` or `Plus` do not have a single physical-key mapping, so `to_hotkey()` returns `None` for those.

## License

kbd-egui is licensed under the MIT license. See the [`LICENSE`](../../LICENSE) file for more information.
