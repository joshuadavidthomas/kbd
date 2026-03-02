# kbd-egui

[![crates.io](https://img.shields.io/crates/v/kbd-egui.svg)](https://crates.io/crates/kbd-egui)
[![docs.rs](https://docs.rs/kbd-egui/badge.svg)](https://docs.rs/kbd-egui)

[`kbd`](https://crates.io/crates/kbd) bridge for [egui](https://docs.rs/egui) — converts key events and modifiers to `kbd` types.

Egui has a smaller, custom key enum that is not 1:1 with the W3C specification — some egui keys are logical/shifted characters without a single physical key equivalent (e.g., `Colon`, `Pipe`, `Plus`), and those return `None`.

```toml
[dependencies]
kbd = "0.1"
kbd-egui = "0.1"
```

## Extension traits

- **`EguiKeyExt`** — converts an `egui::Key` to a `kbd::Key`
- **`EguiModifiersExt`** — converts `egui::Modifiers` to a `Vec<Modifier>`
- **`EguiEventExt`** — converts a full `egui::Event` keyboard event to a `kbd::Hotkey`

## Usage

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

## Key mapping

| egui | kbd | Notes |
|---|---|---|
| `Key::A` – `Key::Z` | `Key::A` – `Key::Z` | Letters |
| `Key::Num0` – `Key::Num9` | `Key::DIGIT0` – `Key::DIGIT9` | Digits |
| `Key::F1` – `Key::F35` | `Key::F1` – `Key::F35` | Function keys |
| `Key::Minus`, `Key::Period`, … | `Key::MINUS`, `Key::PERIOD`, … | Physical-position punctuation |
| `Key::ArrowDown`, `Key::Enter`, … | `Key::ARROW_DOWN`, `Key::ENTER`, … | Navigation / editing |
| `Key::Copy`, `Key::Cut`, `Key::Paste` | `Key::COPY`, `Key::CUT`, `Key::PASTE` | Clipboard |
| `Key::Colon`, `Key::Pipe`, `Key::Plus`, … | `None` | Logical/shifted — no single physical key |

## Modifier mapping

| egui | kbd | Notes |
|---|---|---|
| `ctrl` | `Modifier::Ctrl` | |
| `shift` | `Modifier::Shift` | |
| `alt` | `Modifier::Alt` | |
| `mac_cmd` | `Modifier::Super` | Avoids double-counting with `command` on macOS |

## License

MIT
