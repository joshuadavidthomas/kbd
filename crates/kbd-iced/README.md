# kbd-iced

[![crates.io](https://img.shields.io/crates/v/kbd-iced.svg)](https://crates.io/crates/kbd-iced)
[![docs.rs](https://docs.rs/kbd-iced/badge.svg)](https://docs.rs/kbd-iced)

Converts [iced](https://docs.rs/iced) key events into [`kbd`](https://docs.rs/kbd) types so that in-window shortcuts and global hotkeys (from [`kbd-global`](https://docs.rs/kbd-global)) can share the same dispatcher.

[API docs](https://docs.rs/kbd-iced) — includes the full key and modifier mapping tables.

## Example

```rust
use iced_core::keyboard::{key::Code, Modifiers};
use kbd_iced::{IcedKeyExt, IcedModifiersExt};

let key = Code::KeyS.to_key();
// Some(Key::S)

let mods = Modifiers::CTRL.to_modifiers();
// ModifierSet containing Modifier::Ctrl

let hotkey = kbd::hotkey::Hotkey::with_modifiers(key.unwrap(), mods);
```

Once converted, the `Hotkey` plugs into everything `kbd` offers — register bindings with strings, stack layers for modal shortcuts, define multi-step sequences. One shortcut model for both your iced UI and any system-wide hotkeys you add later.

The legacy hotkey API converts iced's physical key types. Use observations for
independent physical and logical identities.

## Keyboard observations

`event.to_observation()` borrows an iced keyboard event and returns
`Some(KeyboardObservation)` for presses and releases, or `None` for
`ModifiersChanged`. Pass the observation to `Dispatcher::process_event`.
Logical identity comes from **`modified_key`**, not modifier-stripped `key` or text.
Case, shifted punctuation and entire Unicode strings are preserved exactly;
no case folding, normalization or layout inference occurs. Logical Space becomes
`" "`, and Super becomes named Meta. Iced has no dead-key variant to recover.

Physical codes retain their positions, except generic `Meta`, which is omitted
rather than assigned a left side. Unknown physical/logical identities stay absent
independently. Modifiers retain their reported values without self-stripping;
the four aggregate flags do not establish sides, AltGr/Fn or complete knowledge.

The source remains available for location, the unmodified key and press text.
Releases contain no text or repeat flag. Iced's winit backend uses Ctrl-sensitive
text supplements outside Wasm and filters private-use text; its modifier-stripped
`key` also differs on Wasm. No such data is substituted for `modified_key`.
Handle input-method events separately; IME commits are not keyboard observations.
The legacy `to_hotkey()` API and its mappings remain unchanged.

## License

kbd-iced is licensed under the MIT license. See the [`LICENSE`](../../LICENSE) file for more information.
