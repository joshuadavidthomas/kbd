# kbd-egui

[![crates.io](https://img.shields.io/crates/v/kbd-egui.svg)](https://crates.io/crates/kbd-egui)
[![docs.rs](https://docs.rs/kbd-egui/badge.svg)](https://docs.rs/kbd-egui)

[`kbd`](https://crates.io/crates/kbd) bridge for [egui](https://docs.rs/egui) — converts key events and modifiers to `kbd` types.

```toml
[dependencies]
kbd = "0.1"
kbd-egui = "0.1"
```

```rust
use egui::{Key as EguiKey, Modifiers};
use kbd::{Hotkey, Key, Modifier};
use kbd_egui::{EguiKeyExt, EguiModifiersExt, EguiEventExt};

let key = EguiKey::A.to_key();
assert_eq!(key, Some(Key::A));

let event = egui::Event::Key {
    key: EguiKey::C,
    physical_key: None,
    pressed: true,
    repeat: false,
    modifiers: Modifiers::CTRL,
};
let hotkey = event.to_hotkey();
assert_eq!(hotkey, Some(Hotkey::new(Key::C).modifier(Modifier::Ctrl)));
```

## License

MIT
