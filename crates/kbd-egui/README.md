# kbd-egui

[![crates.io](https://img.shields.io/crates/v/kbd-egui.svg)](https://crates.io/crates/kbd-egui)
[![docs.rs](https://docs.rs/kbd-egui/badge.svg)](https://docs.rs/kbd-egui)

Converts [egui](https://docs.rs/egui) key events into [`kbd`](https://docs.rs/kbd) types so that GUI key events and global hotkey events (from [`kbd-global`](https://docs.rs/kbd-global)) can feed into the same dispatcher.

[API docs](https://docs.rs/kbd-egui) — includes the full key and modifier mapping tables.

```toml
[dependencies]
kbd = "0.1"
kbd-egui = "0.1"
```

## Example

```rust
use egui::{Event, Key as EguiKey, Modifiers};
use kbd_egui::EguiEventExt;

let event = Event::Key {
    key: EguiKey::C,
    physical_key: None,
    pressed: true,
    repeat: false,
    modifiers: Modifiers::CTRL,
};

let hotkey = event.to_hotkey();
// Some(Hotkey { key: Key::C, modifiers: {Ctrl} })
```

Once converted, the `Hotkey` works with everything in `kbd` — string-based registration, layers, sequences, introspection. Define your shortcuts once and let the dispatcher handle matching, instead of scattering key checks across your egui `update()` calls.

Egui doesn't expose the full W3C physical-key space. Logical or shifted keys like `Colon` or `Plus` don't have a single physical-key mapping, so `to_hotkey()` returns `None` for those.

## License

kbd-egui is licensed under the MIT license. See the [`LICENSE`](../../LICENSE) file for more information.
