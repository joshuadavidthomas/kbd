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

```rust
use egui::{Event, Key as EguiKey, Modifiers};
use kbd::hotkey::{Hotkey, Modifier};
use kbd::key::Key;
use kbd_egui::{EguiEventExt, EguiKeyExt, EguiModifiersExt};

let key = EguiKey::A.to_key();
assert_eq!(key, Some(Key::A));

let mods = Modifiers::CTRL.to_modifiers();
assert!(mods.contains(Modifier::Ctrl));

let event = Event::Key {
    key: EguiKey::C,
    physical_key: None,
    pressed: true,
    repeat: false,
    modifiers: Modifiers::CTRL,
};
let hotkey = event.to_hotkey();
assert_eq!(hotkey, Some(Hotkey::new(Key::C).modifier(Modifier::Ctrl)));
```

Egui does not expose the full W3C physical-key space. Logical or shifted keys such as `Colon` or `Plus` do not have a single physical-key mapping, so they return `None`.

See the [API docs on docs.rs](https://docs.rs/kbd-egui) for the complete mapping details.

## License

kbd-egui is licensed under the MIT license. See the [`LICENSE`](../../LICENSE) file for more information.
