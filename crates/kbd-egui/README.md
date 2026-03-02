# kbd-egui

Egui bridge for [`kbd`](https://crates.io/crates/kbd) — converts egui key
events and modifiers to `kbd` types.

## Installation

```toml
[dependencies]
kbd = "0.1"
kbd-egui = "0.1"
```

## Usage

```rust
use egui::{Key as EguiKey, Modifiers};
use kbd::{Hotkey, Key, Modifier};
use kbd_egui::{EguiKeyExt, EguiModifiersExt, EguiEventExt};

// Single key conversion
let key = EguiKey::A.to_key();
assert_eq!(key, Some(Key::A));

// Modifier conversion
let mods = Modifiers::CTRL.to_modifiers();
assert_eq!(mods, vec![Modifier::Ctrl]);

// Full event conversion
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

## Extension traits

- `EguiKeyExt` — converts an `egui::Key` to a `kbd::Key`
- `EguiModifiersExt` — converts `egui::Modifiers` to `Vec<Modifier>`
- `EguiEventExt` — converts an `egui::Event` keyboard event to a `kbd::Hotkey`

## License

MIT
