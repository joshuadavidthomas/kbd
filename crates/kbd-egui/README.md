# kbd-egui

[![crates.io](https://img.shields.io/crates/v/kbd-egui.svg)](https://crates.io/crates/kbd-egui)
[![docs.rs](https://docs.rs/kbd-egui/badge.svg)](https://docs.rs/kbd-egui)

Converts [egui](https://docs.rs/egui) key events into [`kbd`](https://docs.rs/kbd) types so that GUI key events and global hotkey events (from [`kbd-global`](https://docs.rs/kbd-global)) can feed into the same dispatcher.

[API docs](https://docs.rs/kbd-egui) — includes the full key and modifier mapping tables.

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

## Conservative keyboard observations

`event.to_observation()` borrows an `egui::Event` and returns
`Some(KeyboardObservation)` for key events, even when both identities are absent.
Pass the observation to `Dispatcher::process_event`.

**Logical identity is always omitted.** Egui's shortcut `key` folds letter case
and may be a physical fallback for non-Latin layouts. Its provenance is unavailable,
so this API does not claim exact logical strings or named-key identity. For exact
logical shortcuts, convert the underlying window-system event before egui loses
that information; do not reconstruct keys from `Event::Text`.

Physical identity comes only from `physical_key`, never from `key`. Digits,
Enter, Slash and Minus are omitted because egui merges main-row and numpad codes.
Shifted-only labels and unmapped codes also stay absent. Other supported positions,
such as an explicitly reported physical Q, retain that position regardless of the
shortcut label. These restrictions do not change legacy `to_hotkey()` behavior.

Press/release/repeat are preserved as reported. Egui-winit initially supplies
`repeat=false`, which egui updates during input processing. Modifier knowledge is
incomplete: `command` is an alias, `mac_cmd` does not report non-Mac Super, and
AltGr/Fn/sides are unavailable. Text, Paste and Ime return `None`; retain and handle
those source events separately. No location or original casing can be recovered.

## License

kbd-egui is licensed under the MIT license. See the [`LICENSE`](../../LICENSE) file for more information.
