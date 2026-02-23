# kbd: Redesign

This document captures the core ideas, domain model, and architectural
direction for kbd. It's not a task list — it's the conceptual
foundation that the task list should serve.

**Naming**: The project is `kbd`. All crates use the `kbd-` prefix:
`kbd-core`, `kbd-global`, `kbd-crossterm`, etc.

See [ATTRIBUTION.md](ATTRIBUTION.md) for licensing constraints on
reference projects. Some can be adapted (MIT), others are
inspiration-only (GPL).

## What is this library?

kbd is a keyboard shortcut engine for Rust.

The core (`kbd-core`) is platform-agnostic — it handles key types,
modifier tracking, binding matching, layer stacks, and sequence
resolution. It works anywhere you have key events: GUI apps, TUI apps,
compositors, game engines.

The facade (`kbd-global`) adds a Linux global hotkey backend on top,
with evdev device access, grab mode, and XDG portal support.

### When to use it

**You're building a TUI app with modal keybindings.** A file manager,
editor, or dashboard with vim-style modes — normal, insert, command,
search — where different keys do different things depending on context.
Every ratatui app hand-rolls this with nested match statements. Use
`kbd-core`'s `Matcher` with layers instead. It handles mode switching,
key sequences, configurable bindings from strings, and introspection
(list all bindings for a help screen). Add `kbd-crossterm` for
crossterm key event conversion.

**You're building a GUI app that needs shortcut matching.** An iced,
egui, or winit-based app with configurable shortcuts, multi-step key
sequences (Ctrl+K → Ctrl+C), or context-dependent bindings. GUI
frameworks expose raw key events but don't provide shortcut
infrastructure — no layers, no sequences, no conflict detection. Use
`kbd-core`'s `Matcher` in your event loop. Conversion crates
(`kbd-winit`, `kbd-iced`, `kbd-egui`) bridge each framework's key
types to `kbd-core`'s — the conversions are mechanical since everyone
derives from the same W3C spec.

**You're building a compositor, editor, or framework that needs shortcut
matching.** Niri, COSMIC, Zed, Helix, and every tiling WM independently
build the same inner engine: key types, modifier tracking, binding
tables, layer stacks, sequence resolution. Use `kbd-core` directly.
It's a synchronous `Matcher` you drive from your own event loop — no
threads, no device access, no platform dependencies.

**You're building an app that needs system-wide hotkeys on Linux.** A
launcher triggered by Super+Space, a screenshot tool on PrintScreen,
push-to-talk on a media key — shortcuts that work regardless of which
window has focus. Use the `kbd-global` facade crate. It handles device
access, backend selection, grab mode, and hotplug.

**You're building a key remapper or input transformation tool.** A tool
that remaps CapsLock to Escape, implements vim-style layers (hjkl as
arrows), or adds tap-hold behavior (tap CapsLock = Esc, hold = Ctrl).
Use `kbd-global` with grab mode enabled. The library intercepts events
via evdev, matches them, and can emit different keys through a virtual
uinput device.

**You're building a Tauri app that needs global hotkeys on Linux.**
Tauri's `tauri-plugin-global-shortcut` (backed by the `global-hotkey`
crate) uses X11 `XGrabKey`, which doesn't work on Wayland. Use
`kbd-global` as your Linux backend — evdev works on both X11 and
Wayland, and the portal backend handles sandboxed environments.

**You're building a sandboxed Wayland app that wants shortcuts.** A
Flatpak-packaged media player that needs push-to-talk or media
controls without requiring device access. Use `kbd-global` with the
portal backend — it requests shortcuts through the XDG GlobalShortcuts
portal, mediated by the compositor.

### Who should use what

| You're building... | Use | Why |
|---|---|---|
| TUI app (ratatui/crossterm) | `kbd-core` + `kbd-crossterm` | Layers, sequences, configurable bindings |
| TUI app (termion) | `kbd-core` + `kbd-termion` | Same, for termion-based apps |
| winit app | `kbd-core` + `kbd-winit` | Mechanical conversion, same W3C key names |
| iced app | `kbd-core` + `kbd-iced` | Mechanical conversion, same W3C key names |
| egui app | `kbd-core` + `kbd-egui` | Bridge for egui's custom key types |
| Dioxus app | `kbd-core` | Uses `keyboard_types::Code` natively |
| Floem / GPUI app | `kbd-core` + `kbd-winit` | Built on winit, key events come from there |
| Makepad app | `kbd-core` + `kbd-makepad` | Custom key types, conversion needed |
| GTK app (gtk-rs) | `kbd-core` + `kbd-gtk` | Bridge for GTK native key events |
| Tauri app (Linux) | `kbd-global` | Tauri's global-shortcut plugin uses X11 grabs that fail on Wayland; kbd-global's evdev+portal backends cover all Linux |
| Compositor / tiling WM | `kbd-core` + `kbd-evdev` | Your own event loop and device access |
| App with global hotkeys (Linux) | `kbd-global` | Full stack: devices, matching, callbacks |
| Key remapper / macro tool | `kbd-global` (grab mode) | Intercept + transform + re-emit keys |
| Sandboxed Wayland app | `kbd-global` + `kbd-portal` | Desktop-mediated shortcuts, no root needed |

### The crate split

**`kbd-core`**: Pure-logic shortcut engine. Key types (built on
`keyboard_types::Code`), modifier tracking, binding matching, layer
stacks, sequence resolution. Platform-agnostic. Embeddable in any
event loop.

**Framework bridge crates** — conversion traits between each
framework's key types and `kbd-core`. Each is a thin crate with the
framework and `kbd-core` as its only dependencies.

| Crate | Bridges | Notes |
|---|---|---|
| `kbd-crossterm` | crossterm | TUI apps (ratatui). Logical key model (`Char('a')`) |
| `kbd-winit` | winit | Also covers floem, GPUI (built on winit) |
| `kbd-iced` | iced | Mirrors winit's types independently |
| `kbd-egui` | egui | Custom key enum |
| `kbd-termion` | termion | Legacy TUI. Modifiers baked into key variants |
| `kbd-makepad` | Makepad | Custom platform bindings, no winit |
| `kbd-gtk` | gtk-rs | GTK native key events |

All GUI frameworks (winit, iced, egui, floem, Makepad) derive their
key types from the same W3C spec, so conversions are mechanical 1:1
mappings. crossterm and termion use logical key models (characters,
not physical positions) — slightly different but straightforward.

Not every crate needs to exist at launch. `kbd-crossterm` is the
priority — it proves the conversion pattern and serves the TUI
ecosystem (ratatui). The GUI bridges (`kbd-winit`, `kbd-iced`,
`kbd-egui`) are built when downstream projects adopt `kbd-core`.
The rest (`kbd-termion`, `kbd-makepad`, `kbd-gtk`) are niche.

**Backend crates** — platform-specific input and device access:

**`kbd-evdev`**: Linux evdev backend. Device discovery, hotplug, grab,
uinput forwarding.

**`kbd-portal`**: XDG GlobalShortcuts portal backend. D-Bus,
compositor-mediated, sandboxed.

**`kbd-xkb`**: Keyboard layout awareness via xkbcommon.

**`kbd-global`**: The facade. Threaded manager, backend selection, the
works. Linux global hotkey users start here. Also a potential
backend for Tauri apps on Linux — Tauri's `global-shortcut` plugin
uses X11 grabs that don't work on Wayland; kbd-global's evdev and
portal backends cover all Linux configurations.

## Where kbd-core fits

`kbd-core` is the matching engine. It has no opinion about where key
events come from. You give it a key + modifiers + press/release, it
tells you which binding matched. The `Matcher` is synchronous and
single-threaded — it fits inside any event loop.

```
Your event source          kbd-core              Your app
─────────────────          ────────              ────────
crossterm KeyEvent ──┐
termion Key ─────────┤
winit KeyCode ───────┤
iced keyboard::Key ──┤
egui Key ────────────┼──▶ Matcher.process() ──▶ MatchResult
Dioxus Code ─────────┤      │                    │
evdev key event ─────┤      │                    ├─ Matched { action }
Wayland wl_keyboard ─┤      │                    ├─ Pending { sequence }
Smithay input ───────┘      │                    ├─ Swallowed
                         layers                  └─ NoMatch
                         bindings
                         key state
```

Conversion crates (`kbd-crossterm`, `kbd-winit`, `kbd-iced`,
`kbd-egui`, etc.) and backend crates (`kbd-evdev`, `kbd-portal`)
handle the translation from each event source to `kbd-core` types.
Dioxus uses `keyboard_types::Code` directly and needs no conversion
crate.

## The Linux global hotkey backend

The `kbd-global` facade adds Linux-specific plumbing on top of `kbd-core`:
a threaded engine, device management, and two backends.

Linux keyboard input is layered:

```
┌─────────────────────────────────────────────┐
│  GUI toolkits (GTK, Qt, iced, egui)         │  Widget-level key handling
│  Windowing (winit, Wayland wl_keyboard)     │  App-level key events
├─────────────────────────────────────────────┤
│  Display server                             │
│    X11: XGrabKey for global shortcuts       │
│    Wayland: focus-bound, no global grabs    │
│    XDG Portal: GlobalShortcuts (sandboxed)  │
├─────────────────────────────────────────────┤
│  libinput                                   │  Userspace device handling
│  evdev (/dev/input/event*)                  │  Kernel device events
│  uinput                                     │  Virtual device injection
├─────────────────────────────────────────────┤
│  Kernel HID / input subsystem               │  Hardware → input_event
└─────────────────────────────────────────────┘
```

The `kbd-global` facade operates at two levels:

1. **evdev** (primary) — reads raw key events from kernel device nodes.
   Sees all keys regardless of focus, can grab devices for exclusive
   access, and can inject remapped keys through uinput. Requires read
   access to `/dev/input/` (typically via the `input` group or seat
   management).

2. **XDG GlobalShortcuts portal** (secondary) — the unprivileged,
   desktop-mediated path for Wayland. Applications request shortcuts
   through D-Bus; the compositor decides whether to grant them. No
   device access needed, works in sandboxes (Flatpak), but limited to
   shortcut activation signals — no grab, no remapping, no raw key
   state.

### What kbd is not

**Not a text input library.** Text input (IME composition, dead keys,
Unicode output) is fundamentally different from key binding. kbd deals
in key identities and modifier combinations, not composed text.

**Not a terminal or GUI framework.** The library doesn't decode terminal
escape sequences (that's crossterm) or manage widget focus (that's your
toolkit). `kbd-crossterm` and `kbd-egui` bridge the key types;
`kbd-core`'s `Matcher` handles the shortcut logic. The framework
handles everything else.

## Key identity: physical position, logical character, or both

`Key` in `kbd-core` is a **key identity** — which key was pressed. Most
of the time this is a physical key position: `Key::A` means "the key
labeled A on a US QWERTY layout." Shortcuts defined by position
(Ctrl+C) work across keyboard layouts without re-mapping.

The source of the event determines the semantics:

- **evdev** — physical position (scancode-based)
- **winit / iced** — physical position (`keyboard_types::Code`)
- **crossterm** — logical character (terminal resolves the layout)

`kbd-core` doesn't distinguish between these. `Key::A` from evdev and
`Key::A` from crossterm match the same bindings. This is correct for
shortcuts — "Ctrl+A" means the same thing regardless of which layer
reported it.

For **layout-aware binding** — "bind the key that produces `/` on the
current layout" — `kbd-xkb` (Phase 4.9) will add `KeyReference::BySymbol`,
resolving keysyms via xkbcommon.

### The key type and `keyboard-types`

`kbd-core`'s `Key` type is a newtype over
[`keyboard_types::Code`](https://crates.io/crates/keyboard-types),
the W3C standard for physical key positions.

```rust
#[repr(transparent)]
pub struct Key(pub keyboard_types::Code);
```

Associated constants provide a clean API: `Key::A`, `Key::ENTER`,
`Key::VOLUME_UP`. The inner `Code` is public.

Why `keyboard-types` instead of maintaining our own enum:

- **We don't maintain 250+ key variants** — new keys in the crate
  are automatically available.
- **It's the W3C standard** — the same spec that winit, iced, and
  every other framework derives their key types from.
- **It's lightweight** — zero transitive deps (only optional serde).

An important nuance: **most frameworks do not depend on
`keyboard-types`**. winit, iced, egui, floem, and Makepad each
define their own key enums — derived from the same W3C spec, but
different Rust types. Only Dioxus directly uses
`keyboard_types::Code`.

The Rust GUI/TUI keyboard type landscape:

| Framework | Key type source | Conversion to kbd-core |
|---|---|---|
| Dioxus | `keyboard_types::Code` | Free (same type) |
| winit | Own `KeyCode` (W3C-derived) | `kbd-winit` (1:1 mapping) |
| iced | Own `Code` (mirrors winit) | `kbd-iced` (1:1 mapping) |
| floem | Via winit | `kbd-winit` covers it |
| GPUI (Zed) | Wraps winit | `kbd-winit` covers it |
| egui | Own `Key` enum | `kbd-egui` |
| Makepad | Custom `KeyCode` | `kbd-makepad` |
| gtk-rs | GTK native events | `kbd-gtk` |
| crossterm | `KeyCode::Char('a')` (logical) | `kbd-crossterm` |
| termion | `Key::Char('a')` (logical, mods baked in) | `kbd-termion` |
| Tauri | JS `KeyboardEvent` via webview | Use `kbd-global` as backend |

`keyboard-types` is a required dependency of `kbd-core` (zero-dep
itself, platform-agnostic).

## What concepts does a user need?

**Core concepts** (both in-app and global):

1. **Keys** — what physical keys you're working with
2. **Bindings** — "when this happens, do that"
3. **Layers** — "in this context, these bindings are active"

**Global-only concept:**

4. **Grab mode** — "intercept everything, not just listen"

That's the mental model. Everything the library does should trace back to
one of these four ideas. If a type or module can't explain which concept it
serves, it probably shouldn't exist.

In-app consumers use concepts 1–3 through the `Matcher` in `kbd-core`
directly. Global consumers get all four through `HotkeyManager` in the
`kbd-global` facade, which drives a `Matcher` on an engine thread with
`kbd-evdev`/`kbd-portal` plumbing.

## The domain model

### Keys

A key is a key. Ctrl is a key. A is a key. CapsLock is a key. The
distinction between "modifier" and "key" is about *role in a combination*,
not about the key itself. In Ctrl+C, Ctrl is the modifier and C is the
trigger — but that's a property of the combination, not of Ctrl.

`Key` is a newtype over `keyboard_types::Code` — the W3C standard enum
of physical key positions. This gives `kbd-core` the full key vocabulary
(250+ keys including media, browser, system keys) without maintaining a
parallel enum. Associated constants (`Key::A`, `Key::ENTER`,
`Key::VOLUME_UP`) provide the API surface.

For the API, the Ctrl/Shift/Alt/Super abstraction is genuinely useful.
Users think in terms of "Ctrl+C", not "KEY_LEFTCTRL + KEY_C". And modifiers
need left/right canonicalization. So:

- `Key` is the complete set of keys, including modifier keys
- `Modifier` is a convenience type for the four common modifiers, handling
  left/right equivalence
- Internally, everything resolves to key codes

A `Hotkey` is a trigger key plus a set of modifiers: the parsed form of
`"Ctrl+Shift+A"`. A `HotkeySequence` is a series of hotkeys performed in
order: `"Ctrl+K, Ctrl+C"`. Both implement `FromStr` / `Display` for
round-trip parsing. These are value types — small, cloneable, comparable,
hashable, serializable.

### Bindings

A binding is the core unit of the library. It answers: **what are we
listening for, and what do we do when we hear it?**

```
Binding = Pattern + Action + Options
```

**Pattern** — the input condition being matched:

- `Hotkey` — key + modifiers, matched immediately (Ctrl+C)
- `Sequence` — ordered series of hotkeys with timeout (Ctrl+K → Ctrl+C)
- `TapHold` — same key, different meaning based on timing
  (tap CapsLock = Esc, hold CapsLock = Ctrl)
- `Chord` — multiple keys pressed simultaneously (j+k = Esc) *(future)*

**Action** — what happens when the pattern matches:

- `Callback` — run user code
- `EmitKey` — emit a different key through uinput *(future, requires grab)*
- `EmitSequence` — emit a series of keys *(future, requires grab)*
- `PushLayer` — push a named layer onto the stack
- `PopLayer` — pop the current layer
- `ToggleLayer` — toggle a layer on/off
- `Swallow` — explicitly consume the key, do nothing

Actions are the output vocabulary. Today the library only has `Callback`.
Phase 4 adds `EmitKey` and the rest. But the `Action` type should exist from
the start — `Callback` is just the first variant. Every place the library
currently takes a bare `Fn()` closure should accept an `Action` instead (with
a convenience conversion from closures, so the simple API stays simple).

**Options** — per-binding modifiers on behavior:

- Device filter (only match on specific devices)
- Passthrough (fire the action but also let the key reach applications)
- Debounce / rate limiting
- Min hold duration
- Press vs release callbacks

One binding type replaces the current four separate registration types
(`HotkeyRegistration`, `SequenceRegistration`, `DeviceHotkeyRegistration`,
`TapHoldRegistration`). One storage structure. One dispatch path. One handle
type.

### Layers

A layer is a named collection of bindings that can be activated and
deactivated at runtime. When active, its bindings participate in matching.
When not active, they don't.

Layers stack. The most recently activated layer's bindings are checked first.
If no binding matches in the top layer, check the next layer down, and so on
down to the global bindings (which are always active, like a base layer).

This is keyd's model, proven in production. It's also what the current
"mode" system is trying to be, but the current implementation spreads the
concept across six types (`ModeOptions`, `ModeDefinition`, `ModeBuilder`,
`ModeRegistry`, `ModeController`, and the mode dispatch module). It should
be two: `Layer` (the definition) and layer operations on the manager (the
control).

Layer options:

- **Oneshot** — auto-deactivate after N keypresses
- **Swallow** — suppress unmatched keys while active (vs letting them fall
  through to lower layers)
- **Timeout** — auto-deactivate after a period of inactivity

### Grab mode

Grab mode is an operational concern, not a domain concept. It means: "take
exclusive ownership of keyboard devices so events don't reach other
applications. Re-emit unmatched events through a virtual device so normal
typing still works."

Grab is a precondition for certain features (remapping, tap-hold, chords)
because those features need to intercept events before they reach other
programs. It's configured at the manager level, not per-binding.

## The tracer bullet: what happens when a key is pressed

This is the throughline. Every piece of code in the library exists to serve
this path.

```
1. Input source reports a key event
2. Engine updates key state
3. Engine matches against active bindings
4. Engine executes the matched action
5. Engine forwards unmatched events (if grab mode)
```

Five steps. Let's trace each one.

### Step 1: Input source reports a key event

The engine polls device file descriptors (evdev) or receives D-Bus signals
(portal). When a key event arrives, it's a triple:

```
(device, key_code, event_type)    // event_type: press, release, repeat
```

The engine converts the raw key code to the library's `Key` type. If the
conversion fails (unknown key), the event is forwarded in grab mode and
otherwise ignored.

### Step 2: Engine updates key state

The engine maintains a `KeyState` that tracks what's currently pressed:

- Which keys are held (for modifier state)
- Per-device key state (for device-specific bindings)
- Timestamp of last key event (for idle-timeout overload strategies)

Modifier state is derived from key state, not tracked separately. "Is Ctrl
held?" is the same question as "is KEY_LEFTCTRL or KEY_RIGHTCTRL in the
pressed set?" The current code maintains a separate `ModifierTracker` with
per-device `HashSet<Modifier>` — that's a parallel data structure that has
to be kept in sync. Key state is the single source of truth; modifier state
is a view over it.

### Step 3: Engine matches against active bindings

This is where the four pattern types matter. The engine processes them in a
specific order because some patterns are *speculative* — they need to buffer
events before deciding whether they've matched.

**Speculative patterns** (checked first, may buffer the event):
- **TapHold**: Key is pressed, but we don't know yet if it's a tap or hold.
  Buffer the event, start a timer. Resolve on: release (tap), timeout
  (hold), or interrupting keypress (hold).
- **Sequence**: First step matched, but we don't know if subsequent steps
  will follow. Buffer, start timeout. Resolve on: next step (advance),
  timeout (fire standalone if registered), wrong key (reset).
- **Chord**: One key of a chord pressed, waiting for the others. Buffer,
  start window timer. Resolve on: all keys pressed (fire), timeout (flush
  as individual presses), wrong key (flush). *(future)*

**Immediate patterns** (checked if event wasn't buffered):
- **Hotkey**: Does this key + current modifiers match a binding? Direct
  lookup, instant decision.

The matching walks the layer stack top-down:
1. For each active layer (most recent first):
   - Check that layer's bindings against the current event
   - If a binding matches, stop searching — this layer "owns" this event
2. Check global bindings (always-active base layer)
3. If nothing matched, the event is unmatched

Within each layer, bindings are checked in pattern-type order: speculative
first, then immediate. This ensures tap-hold and sequence bindings get first
crack at events before a simpler hotkey binding claims them.

### Step 4: Engine executes the matched action

Once a binding matches, its `Action` is executed:

- `Callback` → invoke the closure (with panic isolation)
- `EmitKey` → write a key event to the uinput virtual device
- `PushLayer` / `PopLayer` / `ToggleLayer` → modify the layer stack
- `Swallow` → do nothing (event is consumed)

For key press events, the engine **caches** the action taken:

```
press_cache[key] = action_that_was_executed
```

When the corresponding release event arrives, the engine looks up the cache
instead of re-matching. This ensures the release uses the same action as the
press, even if layers changed in between. This is keyd's `cache_entry`
system and it's essential for correctness.

### Step 5: Engine forwards unmatched events

In grab mode, events that didn't match any binding are re-emitted through a
virtual uinput device so they reach applications normally. Events that
matched a binding with `passthrough: true` are also forwarded.

Without grab mode, this step is a no-op — events reach applications through
the normal kernel path.

## Architecture: message passing, not shared state

The current architecture has the manager and listener sharing state through
nine `Arc<Mutex<HashMap>>` instances. This is the root cause of most
complexity — lock management, operation serialization, state duplication
between manager and listener.

The redesign uses **message passing**:

```
┌─────────────────────────┐
│     HotkeyManager       │  Public API. Thin.
│                         │  Sends commands, receives replies.
│  register() ─────┐     │
│  define_layer() ──┤     │
│  unregister() ────┤     │
│  push_layer() ────┤     │
│  pop_layer() ─────┤     │
│  shutdown() ──────┤     │
└───────────────────┼─────┘
                    │ Command channel
                    ▼
┌─────────────────────────┐
│        Engine            │  Owns all mutable state.
│                         │  Runs in a dedicated thread.
│  bindings               │
│  layer_stack             │
│  key_state               │
│  sequence_state          │
│  tap_hold_state          │
│  press_cache             │
│  devices                 │
│  forwarder               │
│                         │
│  Event loop:             │
│   poll(devices + wake)   │
│   drain commands         │
│   process key events     │
│   execute actions        │
│   forward unmatched      │
└─────────────────────────┘
```

**No shared mutable state.** The engine owns its binding table, its key
state, its layer stack — everything. The manager sends commands and
optionally waits for a reply.

**Commands** are the API between manager and engine:

```
Register { id, binding } → Result<(), Error>
Unregister { id }
DefineLayer { name, layer } → Result<(), Error>
PushLayer { name }
PopLayer
QueryKeyState { key } → bool
Shutdown
```

Operations that can fail (registration conflicts, unknown layers) use a
reply channel so the manager can return `Result` to the caller. Fire-and-
forget operations (unregister, push/pop) don't need replies.

**Handles** become simple:

```rust
struct Handle {
    id: BindingId,
    commands: CommandSender,
}

impl Drop for Handle {
    fn drop(&mut self) {
        let _ = self.commands.send(Command::Unregister { id: self.id });
    }
}
```

No `Arc<HotkeyManagerInner>`. No locks. No shared state. The handle just
sends a message.

**Waking the engine**: The engine's event loop uses `poll()` to wait on
device file descriptors. To also receive commands without spinning, use an
eventfd (or pipe) that the manager writes to when it sends a command. The
eventfd is added to the engine's poll set alongside device fds.

## Module structure

The workspace split (Phase 3.5) distributes code across crates. Each
crate has a focused responsibility:

```
crates/
  kbd-core/src/
    lib.rs              Public API surface for the platform-agnostic engine.
    key.rs              Key (newtype over keyboard_types::Code), Modifier,
                        Hotkey, HotkeySequence. Parsing, display, aliases.
    action.rs           Action enum (Callback, EmitKey, PushLayer, etc.).
    binding.rs          Binding, BindingOptions, BindingId.
    layer.rs            Layer, LayerOptions.
    matcher.rs          Matcher — synchronous binding engine. Layers,
                        sequences, key state, press cache.
    key_state.rs        What's currently pressed. Modifier state derived here.
    error.rs            Core error types (parse, conflict, layer).

  kbd-crossterm/src/
    lib.rs              CrosstermKeyExt, CrosstermEventExt traits.
                        crossterm KeyCode/KeyEvent → Key/Hotkey conversion.

  kbd-winit/src/        (on demand)
    lib.rs              WinitKeyExt — winit KeyCode ↔ Key. 1:1 W3C mapping.

  kbd-iced/src/         (on demand)
    lib.rs              IcedKeyExt — iced key::Code ↔ Key.

  kbd-egui/src/         (on demand)
    lib.rs              EguiKeyExt, EguiModifiersExt — egui Key → Key.

  kbd-evdev/src/
    lib.rs              Extension traits: evdev KeyCode ↔ Key.
    devices.rs          Device discovery, hotplug (inotify), capability detection.
    forwarder.rs        uinput virtual device for event forwarding/emission.

  kbd-portal/src/
    lib.rs              XDG GlobalShortcuts portal (DBus via ashpd).

  kbd-xkb/src/
    lib.rs              xkbcommon integration: keycode → keysym, layout detection.

  kbd-derive/src/
    lib.rs              #[derive(Bindings)] proc macro (future).

  kbd-global/src/
    lib.rs              Facade. Re-exports kbd-core types.
    manager.rs          HotkeyManager — thin command sender.
    handle.rs           Handle — RAII unregistration via command.
    engine/
      mod.rs            Engine struct, event loop, core matching/dispatch.
      types.rs          GrabState, KeyEventDisposition, MatchOutcome, etc.
      command.rs        Command enum, CommandSender (manager→engine channel).
      runtime.rs        EngineRuntime (spawn, shutdown, join).
      wake.rs           WakeFd (eventfd wrapper), LoopControl.
    backend/
      mod.rs            Backend trait + selection logic.
      evdev.rs          evdev backend (delegates to kbd-evdev).
      portal.rs         Portal backend (delegates to kbd-portal).
    events.rs           HotkeyEvent, async event stream (feature-gated).
```

**What's gone from v0:**

- `config.rs` — the application framework (`ActionMap`, `HotkeyConfig`,
  `RegisteredConfig`). Users compose their own config using serde on
  the core types.
- `listener/` — replaced by `engine/` with message passing.
- `mode/` (six files) — replaced by `layer.rs` in `kbd-core`.
- Duplicated key/modifier logic — shared via the type system.

## Type inventory

What types does a user need to know? Roughly:

**`kbd-core` types (any consumer):**
- `Key` — a key (newtype over `keyboard_types::Code`)
- `Modifier` — Ctrl/Shift/Alt/Super
- `Hotkey` — key + modifiers, parseable from strings
- `Matcher` — synchronous binding engine, the core of everything
- `MatchResult` — what the matcher decided (matched, pending, swallowed, no match)
- `Action` — what happens on match (callback, emit key, push layer, etc.)
- `Layer` / `LayerOptions` — named binding groups
- `HotkeySequence` — multi-step combos
- `BindingOptions` — per-binding configuration
- `Error` — what went wrong

**`kbd-global` facade types (global hotkey consumers):**
- `HotkeyManager` — entry point for global hotkeys
- `Handle` — keeps a binding alive (RAII unregistration)
- `DeviceFilter` — restrict to specific devices
- `Backend` — explicit backend selection
- `ConsumePreference` — observe vs intercept intent

**When using async (feature-gated):**
- `HotkeyEvent` — event notifications
- `HotkeyEventStream` — async stream

In-app consumers (TUI, GUI) use `kbd-core` types directly with the
`Matcher`. Global hotkey consumers use `kbd-global`'s `HotkeyManager`,
which drives a `Matcher` on an engine thread internally.

## What the simple API looks like

The simple case must stay simple:

```rust
use kbd_global::{HotkeyManager, Key, Modifier};

let manager = HotkeyManager::new()?;

let _handle = manager.register(
    Key::C, &[Modifier::Ctrl, Modifier::Shift],
    || println!("fired"),
)?;
```

Three imports. One line of registration. The closure is automatically
wrapped in `Action::Callback`. The user never sees `Action`, `Binding`,
`Pattern`, or `BindingOptions` unless they need them.

The power case is composable:

```rust
use kbd_global::{Action, HotkeyManager, Key, Layer, Modifier, TapHoldOptions};

let manager = HotkeyManager::builder().grab().build()?;

// Remap CapsLock: tap = Escape, hold = activate nav layer
manager.register_tap_hold(
    Key::CapsLock,
    Action::EmitKey(Key::Escape, vec![]),
    Action::PushLayer("nav".into()),
    TapHoldOptions::new().threshold(Duration::from_millis(200)),
)?;

// Nav layer: hjkl = arrow keys
let nav = Layer::new("nav")
    .bind(Key::H, &[], Action::EmitKey(Key::Left, vec![]))
    .bind(Key::J, &[], Action::EmitKey(Key::Down, vec![]))
    .bind(Key::K, &[], Action::EmitKey(Key::Up, vec![]))
    .bind(Key::L, &[], Action::EmitKey(Key::Right, vec![]))
    .bind(Key::Escape, &[], Action::PopLayer);

manager.define_layer(nav)?;
```

The layer definition is a builder. No closure-based `ModeBuilder`, no
separate `ModeController`, no `ModeOptions` struct. Just a `Layer` value
that you build and hand to the manager.

## Key design decisions

### Why message passing instead of shared state

The current `Arc<Mutex<HashMap>>` pattern has these consequences:

- **Lock contention**: Every registration operation and every key event
  competes for the same locks.
- **Operation serialization**: An `operation_lock` mutex exists solely to
  serialize operations that touch multiple HashMaps.
- **Complexity budget**: ~80% of the code manages concurrent access. ~20%
  is actual domain logic.
- **Deadlock risk**: Multiple nested lock acquisitions with ordering
  requirements.

Message passing eliminates all of this. The engine owns its state
exclusively. No locks, no contention, no ordering requirements. The manager
is a thin command sender.

The tradeoff: registration becomes asynchronous under the hood (send command,
wait for reply). In practice this is unnoticeable — registration happens
during setup, not in hot loops.

### Why one binding type instead of four

The current four registration types exist because features were added
incrementally: first hotkeys, then sequences, then device-specific hotkeys,
then tap-hold. Each got its own storage, its own handle type, its own
dispatch path.

But they're all the same idea: pattern + action + options. The differences:

| Current type | Pattern | Options |
|---|---|---|
| HotkeyRegistration | Hotkey | debounce, hold, passthrough |
| SequenceRegistration | Sequence | timeout, abort key |
| DeviceHotkeyRegistration | Hotkey | device filter |
| TapHoldRegistration | TapHold | threshold |

These aren't four things. They're one thing with different configurations.
Unifying them means one storage structure, one dispatch path, one handle
type, one set of tests.

### Why layers instead of modes

"Mode" implies mutual exclusion — you're in one mode at a time. "Layer"
implies composition — multiple layers can be active, stacked, with priority.
keyd uses layers. QMK uses layers. The keyboard community thinks in layers.

The current "mode" system already behaves like a stack (push/pop). Calling
it "layer" and letting multiple layers be active simultaneously is a small
conceptual shift that opens up composability (e.g., a "shift" layer that
adds Shift to all keys, composed with a "nav" layer that remaps hjkl).

### Why Action from the start

The current library takes closures everywhere. Adding `Action::EmitKey`
later means either:
- A breaking API change (closures → Action)
- A parallel API (`register` takes closure, `register_action` takes Action)
- Conversion shims everywhere

If `Action` exists from the start, closures are just `Action::Callback`
with a `From` impl. The simple API stays simple. The power API composes.
And config serialization works for free — `Action` variants (except
`Callback`) are data, not code, so they serialize naturally.

### Why two backends with different capabilities

The two backends exist because Linux doesn't have one "global shortcut"
mechanism — it has a privileged path and an unprivileged path, and they
can do different things:

| Capability | evdev | XDG Portal |
|---|---|---|
| See all key events | ✅ | ❌ (activation signals only) |
| Key state queries | ✅ | ❌ |
| Grab (exclusive access) | ✅ | ❌ |
| Remap / emit keys (uinput) | ✅ | ❌ |
| Works without root/input group | ❌ | ✅ |
| Works in Flatpak sandbox | ❌ | ✅ |
| Desktop-mediated consent | ❌ | ✅ |
| Tap-hold, chords, sequences | ✅ | ❌ (no raw events) |

These aren't two implementations of the same thing. They're different
tools for different situations. A system daemon that remaps CapsLock
needs evdev. A sandboxed media player that wants play/pause on a
global shortcut uses the portal.

`ConsumePreference` (planned in Phase 4.5) lets the user express their
intent — "I need to consume keys" vs "I just need to observe" — and
the library selects or validates the backend accordingly. This is
modeled after livesplit-hotkey's approach, which solved the same
problem across multiple platforms.

The facade (`kbd-global`) handles backend selection. The core (`kbd-core`)
doesn't know backends exist — it's the pure matching engine that both
backends feed into.

### Why not the current config system

The current `config.rs` builds an application framework: `ActionId` for
string-based action references, `ActionMap` for registering callbacks by
name, `HotkeyConfig` for declarative config files, `RegisteredConfig` with
transactional rollback.

This is the wrong abstraction for a library. Library consumers will have
their own config systems — TOML files, CLI args, GUI settings panels. They
don't want the library imposing an opinion about config structure.

What they want: types that are serde-friendly. `Key`, `Modifier`, `Hotkey`,
`Action`, `Layer` should all derive `Serialize` / `Deserialize` (behind a
feature flag). Then users write:

```rust
#[derive(Deserialize)]
struct MyConfig {
    quit: Hotkey,
    reload: Hotkey,
    leader_key: HotkeySequence,
}
```

The library provides the types. The application provides the structure.

## What this means for the plan

Phase 4.1 (codebase cleanup) as currently written is a tactical fix list:
move tests, swap bools for enums, split files. Those are real problems but
they're symptoms of the architectural issues described here.

The restructuring should be:

1. **Introduce the core types** — `Action`, unified `Binding`, `Layer`.
   These can coexist with the current implementation initially.
2. **Build the engine** — message-passing event loop that owns its state.
   This replaces the listener and its shared-state machinery.
3. **Rewire the manager** — from shared-state accessor to command sender.
   The public API signature stays the same; the internals change completely.
4. **Collapse the type surface** — unify handle types, unify registration
   paths, remove the mode subsystem in favor of layers.
5. **Clean up the rest** — Key/Modifier deduplication, From/Into traits,
   error consolidation, bool→enum, test migration. These fall out naturally
   once the architecture is right.

The tactical fixes aren't a separate phase — they're consequences of getting
the architecture right. You don't need a checklist item for "replace bool
fields with enums" if the types those bools lived in no longer exist.
