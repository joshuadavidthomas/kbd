# kbd-tao

[![crates.io](https://img.shields.io/crates/v/kbd-tao.svg)](https://crates.io/crates/kbd-tao)
[![docs.rs](https://docs.rs/kbd-tao/badge.svg)](https://docs.rs/kbd-tao)

Converts [tao](https://docs.rs/tao) key events into [`kbd`](https://docs.rs/kbd) types. Especially useful in Tauri apps that want both in-window shortcuts and system-wide hotkeys (via [`kbd-global`](https://docs.rs/kbd-global)) through a single dispatcher.

[API docs](https://docs.rs/kbd-tao) — includes the full key and modifier mapping tables and an event-loop example.

## Example

```rust
use tao::keyboard::{KeyCode, ModifiersState};
use kbd_tao::{TaoKeyExt, TaoModifiersExt};

let key = KeyCode::KeyS.to_key();
// Some(Key::S)

let mods = ModifiersState::CONTROL.to_modifiers();
// ModifierSet containing Modifier::Ctrl

let hotkey = kbd::hotkey::Hotkey::with_modifiers(key.unwrap(), mods);
```

The resulting `Hotkey` works with everything in `kbd` — layers, sequences, string-based registration, introspection. For a Tauri app, that means your in-window shortcuts and your global hotkeys (via `kbd-global`) share the same binding model and the same dispatcher.

Tao tracks modifiers separately from key events. For full `KeyEvent` conversion inside an event loop, use [`TaoEventExt`](https://docs.rs/kbd-tao/latest/kbd_tao/trait.TaoEventExt.html) — it takes the latest `ModifiersState` as a parameter.

## Keyboard observations

`event.to_observation(modifiers)` borrows the event and returns
`Some(kbd::observation::KeyboardObservation)` for `Dispatcher::process_event`,
or `None` for an unsupported future tao element state.
Physical and logical identity remain independent; characters preserve exact case,
shifted punctuation and full Unicode strings, without layout inference or text
substitution. Logical Space becomes `" "`, Super becomes named Meta, and dead keys
become generic named Dead. Unknown identities stay absent.

Physical `Plus` is omitted: its shifted/accelerator meaning does not establish an
Equal position. Explicit physical Equal still maps to Equal. Modifiers retain
reported values without legacy trigger-self stripping. The four aggregate flags
are incomplete knowledge: sides and AltGr/Fn state are unavailable, and Ctrl+Alt
must not be assumed to mean AltGr.

Keep the original event for location, text, native identifiers and dead accents.
Press/repeat/release are preserved, but `ReceivedImeText` remains a separate
stream. Tao does not expose winit's modern preedit/commit event enum. Android/iOS
modifier supplements fall back to the ordinary logical key/text; this conversion
does not use those supplements. `to_hotkey()` and its legacy Plus alias remain
unchanged.

## License

kbd-tao is licensed under the MIT license. See the [`LICENSE`](../../LICENSE) file for more information.
