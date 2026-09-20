# kbd-winit

[![crates.io](https://img.shields.io/crates/v/kbd-winit.svg)](https://crates.io/crates/kbd-winit)
[![docs.rs](https://docs.rs/kbd-winit/badge.svg)](https://docs.rs/kbd-winit)

Converts [winit](https://docs.rs/winit) key events into [`kbd`](https://docs.rs/kbd) types so that in-window shortcuts and global hotkeys (from [`kbd-global`](https://docs.rs/kbd-global)) can share the same dispatcher.

[API docs](https://docs.rs/kbd-winit) — includes the full key and modifier mapping tables and an event-loop example.

## Example

```rust
use winit::keyboard::{KeyCode, ModifiersState};
use kbd_winit::{WinitKeyExt, WinitModifiersExt};

let key = KeyCode::KeyS.to_key();
// Some(Key::S)

let mods = ModifiersState::CONTROL.to_modifiers();
// ModifierSet containing Modifier::Ctrl

let hotkey = kbd::hotkey::Hotkey::with_modifiers(key.unwrap(), mods);
```

Once converted, the `Hotkey` plugs into everything `kbd` offers — register bindings with strings, stack layers for modal shortcuts, define multi-step sequences. Define your shortcuts once and let the dispatcher sort out matching, instead of writing `if key == KeyCode::KeyS && modifiers.control_key()` in every event handler.

Winit tracks modifiers separately from key events. For full `KeyEvent` conversion inside an event loop, use [`WinitEventExt`](https://docs.rs/kbd-winit/latest/kbd_winit/trait.WinitEventExt.html) — it takes the latest `ModifiersState` from `WindowEvent::ModifiersChanged`.

## Keyboard observations

`event.to_observation(modifiers)` borrows the event and returns a
`kbd::observation::KeyboardObservation` for `Dispatcher::process_event`.
It retains independent physical and logical identities and press/repeat/release.
Logical characters are exact strings: `"a"`, `"A"`, `"é"`, and `"e\u{301}"`
are distinct; shifted punctuation and multi-codepoint strings are not normalized.
Logical `Space` becomes the character `" "`, and winit `Super` becomes named `Meta`.
Dead keys become generic named `Dead`; unidentified keys remain absent.

Only reported physical codes are converted. Generic physical `Meta` is omitted
rather than assigned a left side. No position is inferred from a logical name.
Modifiers are preserved without legacy trigger-self stripping. The four aggregate
flags are **not complete modifier knowledge**: do not infer AltGr from Ctrl+Alt or
AltRight, or Fn state from a Fn trigger. Retain full winit modifier events for
side metadata, which itself may be unknown.

The source remains available for location, native identifiers, dead-key accents,
key text and platform supplements. Handle IME preedit/commit separately; text is
not substituted for logical identity. `to_hotkey()` and its existing mappings
remain the legacy compatibility API.

## License

kbd-winit is licensed under the MIT license. See the [`LICENSE`](../../LICENSE) file for more information.
