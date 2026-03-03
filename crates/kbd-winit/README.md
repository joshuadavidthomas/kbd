# kbd-winit

[![crates.io](https://img.shields.io/crates/v/kbd-winit.svg)](https://crates.io/crates/kbd-winit)
[![docs.rs](https://docs.rs/kbd-winit/badge.svg)](https://docs.rs/kbd-winit)

[`kbd`](https://crates.io/crates/kbd) bridge for [winit](https://docs.rs/winit) — converts key events and modifiers to `kbd` types.

This lets window-focused key events (from winit) and global hotkey events (from [`kbd-global`](https://docs.rs/kbd-global)) feed into the same `Dispatcher`. Useful in applications where you want both in-window shortcuts and system-wide hotkeys handled through a single hotkey registry.

Both winit and `kbd` derive from the W3C UI Events specification, so the variant names are nearly identical and the mapping is mechanical. Winit wraps key codes in a `PhysicalKey` type and tracks modifiers separately via `WindowEvent::ModifiersChanged`, so `WinitEventExt` takes `ModifiersState` as a parameter.

```toml
[dependencies]
kbd = "0.1"
kbd-winit = "0.1"
```

## Extension traits

- **`WinitKeyExt`** — converts a winit `PhysicalKey` or `KeyCode` to a `kbd::Key`
- **`WinitModifiersExt`** — converts winit `ModifiersState` to a `Vec<Modifier>`
- **`WinitEventExt`** — converts a winit `KeyEvent` plus `ModifiersState` to a `kbd::Hotkey`

## Usage

Inside winit's event loop, use `WinitEventExt` to convert key events directly:

```rust,no_run
use kbd::prelude::*;
use kbd_winit::WinitEventExt;
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::ModifiersState;
use winit::window::{Window, WindowId};

struct App {
    modifiers: ModifiersState,
    window: Option<Window>,
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_none() {
            let attrs = Window::default_attributes().with_title("kbd-winit example");
            self.window = Some(event_loop.create_window(attrs).unwrap());
        }
    }

    fn window_event(&mut self, _event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::ModifiersChanged(mods) => {
                self.modifiers = mods.state();
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if let Some(hotkey) = event.to_hotkey(self.modifiers) {
                    println!("{hotkey}");
                }
            }
            _ => {}
        }
    }
}

let event_loop = EventLoop::new().unwrap();
let mut app = App { modifiers: ModifiersState::empty(), window: None };
event_loop.run_app(&mut app).unwrap();
```

The individual conversion traits can also be used separately:

```rust
use kbd::prelude::*;
use kbd_winit::{WinitKeyExt, WinitModifiersExt};
use winit::keyboard::{KeyCode, ModifiersState, PhysicalKey};

let key = KeyCode::KeyA.to_key();
assert_eq!(key, Some(Key::A));

let key = PhysicalKey::Code(KeyCode::KeyA).to_key();
assert_eq!(key, Some(Key::A));

let mods = ModifiersState::CONTROL.to_modifiers();
assert_eq!(mods, vec![Modifier::Ctrl]);
```

## Key mapping

| winit | kbd | Notes |
|---|---|---|
| `KeyCode::KeyA` – `KeyCode::KeyZ` | `Key::A` – `Key::Z` | Letters |
| `KeyCode::Digit0` – `KeyCode::Digit9` | `Key::DIGIT0` – `Key::DIGIT9` | Digits |
| `KeyCode::F1` – `KeyCode::F35` | `Key::F1` – `Key::F35` | Function keys |
| `KeyCode::Numpad0` – `KeyCode::Numpad9` | `Key::NUMPAD0` – `Key::NUMPAD9` | Numpad |
| `KeyCode::Enter`, `KeyCode::Escape`, … | `Key::ENTER`, `Key::ESCAPE`, … | Navigation / editing |
| `KeyCode::ControlLeft`, … | `Key::CONTROL_LEFT`, … | Modifier keys as triggers |
| `KeyCode::SuperLeft` / `KeyCode::Meta` | `Key::META_LEFT` | winit's Super = kbd's Meta |
| `KeyCode::SuperRight` | `Key::META_RIGHT` | |
| `KeyCode::MediaPlayPause`, … | `Key::MEDIA_PLAY_PAUSE`, … | Media keys |
| `KeyCode::BrowserBack`, … | `Key::BROWSER_BACK`, … | Browser keys |
| `PhysicalKey::Unidentified(_)` | `None` | No mapping possible |

## Modifier mapping

| winit | kbd |
|---|---|
| `CONTROL` | `Modifier::Ctrl` |
| `SHIFT` | `Modifier::Shift` |
| `ALT` | `Modifier::Alt` |
| `SUPER` | `Modifier::Super` |

## License

MIT
