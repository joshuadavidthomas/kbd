# kbd-egui

[![crates.io](https://img.shields.io/crates/v/kbd-egui.svg)](https://crates.io/crates/kbd-egui)
[![docs.rs](https://docs.rs/kbd-egui/badge.svg)](https://docs.rs/kbd-egui)

`kbd-egui` converts egui keyboard events into `kbd` types so egui input and global hotkeys can share the same `kbd::dispatcher::Dispatcher`.

```toml
[dependencies]
kbd = "0.1"
kbd-egui = "0.1"
```

## Public API

- `EguiKeyExt` converts `egui::Key` to `kbd::key::Key`
- `EguiModifiersExt` converts `egui::Modifiers` to `kbd::hotkey::ModifierSet`
- `EguiEventExt` converts keyboard `egui::Event` values to `kbd::hotkey::Hotkey`

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
assert_eq!(mods.len(), 1);

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

## Mapping notes

- egui has a smaller key model than W3C physical keys, so some values cannot be mapped
- logical or shifted keys such as `Colon` or `Plus` return `None`
- modifier conversion yields a compact `ModifierSet`

See the [API docs on docs.rs](https://docs.rs/kbd-egui) for the complete mapping details.

## License

kbd-egui is licensed under the MIT license. See the [`LICENSE`](../../LICENSE) file for more information.
