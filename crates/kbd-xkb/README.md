# kbd-xkb

Enrich `kbd::observation::KeyboardObservation` using a **caller-owned,
authoritative `xkb::State`**. Physical identity, legacy modifiers, rich physical
modifier evidence, and transition are unchanged. Logical identity and semantic
logical modifiers are replaced. This crate is not yet published.

## Usage and ownership

Call `enrich(&mut observation, &state, xkb_keycode, mapping)`, then pass the
observation to the existing dispatcher's `process_event` or
`process_event_with_device` **once**. There is no second dispatcher and no
rewriting logical bindings into physical bindings.

The caller supplies the matching keymap, current state, keycode namespace, and
modifier mapping. `xkb::State` retains its keymap; no separate keymap parameter is
needed. An XKB state compiled from default names is **not** evidence of the active
desktop layout. There is no automatic desktop discovery or `kbd-global` hookup.

- For raw evdev codes with an evdev-compatible map, `evdev_keycode(code)` adds 8.
  X11/XKB keycodes already have the offset; pass them directly.
- A direct-device state owner queries before `State::update_key`, balances each
  real press/release, and never adds another Down for a repeat.
- A client receiving authoritative serialized state applies **all six** values
  to `State::update_mask` in protocol order. Do not also apply `update_key`.
- The caller owns initial held keys, locked/latched modifiers, active groups,
  multi-device aggregation, lost-input/disconnect resynchronization, and keymap
  replacement. Unknown state is not equivalent to all modifiers being released.
- Enrichment never updates state, on any transition. It queries the supplied
  state afresh, so subsequent events reflect group changes without a cache or
  binding rewrite. The caller can inspect `STATE_LAYOUT_EFFECTIVE` on updates
  and `key_get_layout` for the effective per-key group.
- Keep physical identity/source stable across a hold even if its logical symbol
  changes. The core owns hold/release lifetimes; call `cancel_source` or
  `reset_input` on lost input as appropriate, alongside XKB resynchronization.

## Explicit semantic modifier mapping

`ModifierMapping` contains pairs of `(kbd::hotkey::Modifier, u8)` and an
`ignored: u8` mask. These are the active keymap's **eight real XKB modifier bits**,
not `ModifierSet` bits or virtual modifier indices. Derive real masks from the
authoritative keymap (e.g. `1 << keymap.mod_get_index("Shift")`, after checking
that the index is a real modifier, below 8). There are no default semantic
assignments: Mod1 is not universally Alt, Mod4 is not universally Super, and
physical RightAlt is not proof of AltGraph. Replace mappings on keymap changes.

For a caller-established conventional map, the following might apply; it is an
example, **not a fallback layout or universal mapping**:

```rust
use kbd::hotkey::Modifier;
use kbd_xkb::ModifierMapping;

let mapping = ModifierMapping {
    modifiers: &[
        (Modifier::Shift, 0x01),
        (Modifier::Ctrl, 0x04),
        (Modifier::Alt, 0x08),
        (Modifier::Super, 0x40),
        (Modifier::AltGraph, 0x80),
    ],
    ignored: 0x02 | 0x10, // Only if Lock/Mod2 are known insignificant locks.
};
```

Omitted semantic modifiers remain unknown; a zero-mask entry explicitly declares
one known inactive. Multiple bits for one semantic modifier are combined using
any-active semantics. It is consumed only when **all its active real bits** are
consumed (or all mapped bits, if inactive). Unmapped active real bits become
`extra_active`, blocking matching, unless the caller explicitly includes them
in `ignored`. Ignoring a bit does not
disable its explicit semantic mapping. Firmware Fn state cannot be discovered
by XKB; do not declare it known merely because a mapping omits it.

Logical metadata is authoritative and independent of physical flags. In
particular, AltGr can be logically active while the original observation retains
physical Alt. Nothing is subtracted from physical modifiers.

## Exact matching and consumed modifiers

The adapter reports semantic consumed modifiers using libxkbcommon's
XKB consumed mode, respecting keymap `preserve` entries. This mode can differ
from GTK heuristics; the safe 0.9.0 binding does not expose GTK mode. XKB can
report an inactive modifier consumed because it could affect translation. That
information is preserved, but never satisfies a binding requiring an inactive
modifier. A valid key with no mapped consumed modifiers reports
`Some(ModifierSet::NONE)`; an invalid key reports `None` (unknown), not known-empty.

Matching stays exact by default. An immediate logical binding can explicitly use
`BindingOptions::with_logical_match_policy(LogicalMatchPolicy::Consumed)`. The
core then requires every requested modifier to be active and permits only extra
modifiers known consumed. Thus logical `!` can match Shift+1, while explicitly
required Shift remains required and the physical Shift+Digit1 binding still
sees Shift. Unknown consumption falls back to exact matching. Sequence steps
remain exact.

`ISO_Left_Tab` maps to named Tab. If the map consumes Shift for it, opt-in
consumed matching lets a plain Tab binding match Shift+Tab. Use exact matching
to keep those distinct. Characters stay case-sensitive: `"a"` is not `"A"`,
even when a binding explicitly requires Shift.

## Logical identity is not text input

Named keysyms map explicitly to named keys (navigation, editing, functions,
modifiers, keypad navigation, and supported media/system keys). Other printable
keysyms become exact Unicode character strings. Ctrl+A resolves to `"a"`, not
the U+0001 control text from `State::key_get_utf8`. Generic dead keys become
named Dead; there is no accent payload or composition. Unknown, missing, and
multiple-symbol results leave logical identity absent instead of choosing the
first symbol or concatenating text. Existing native text/compose/IME processing
remains entirely with the caller.

## Build and tests

The dependency is pinned to `xkbcommon = "=0.9.0"`, with default features off:
no Wayland FD helpers, X11 connection helpers, or dynamic-loader abstraction.
It links native `libxkbcommon`; it does not bundle the C library. On Debian/Ubuntu
install `libxkbcommon-dev` to build and the corresponding runtime library to run.
Use a platform/toolchain supporting that native library; this is not a portable
fallback for systems without XKB. The workspace test CI explicitly installs the
development package. The binding has no declared Rust MSRV of its own; the
adapter's targeted tests and doctest have been verified on Rust 1.85.0 with
native libxkbcommon 1.5.0. This does not assert the MSRV of other workspace bridges.

`cargo test -q -p kbd-xkb` uses a checked-in, self-contained keymap with
US/AZERTY/QWERTZ subsets and special test keys. Contexts disable default includes
and environment names. Tests do not require installed `xkeyboard-config`, a
display server, input devices, or a particular desktop layout. The fixture is
not intended to describe complete national layouts.

Authoritative references:
- [Released Rust binding source](https://docs.rs/crate/xkbcommon/0.9.0/source/)
- [libxkbcommon state and consumption contract](https://xkbcommon.org/doc/current/group__state.html)

## License

kbd-xkb is licensed under the MIT license. See the [`LICENSE`](../../LICENSE) file for more information.
