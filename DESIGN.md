# keybound: Redesign

This document captures the core ideas, domain model, and architectural
direction for restructuring keybound. It's not a task list — it's the
conceptual foundation that the task list should serve.

See [ATTRIBUTION.md](ATTRIBUTION.md) for licensing constraints on
reference projects. Some can be adapted (MIT), others are
inspiration-only (GPL).

## What is this library?

keybound is a keyboard shortcut engine for Rust on Linux.

### When to use it

**You're building an app that needs system-wide hotkeys.** A launcher
triggered by Super+Space, a screenshot tool on PrintScreen, push-to-talk
on a media key, a clipboard manager on Ctrl+Shift+V — shortcuts that
work regardless of which window has focus. Use the `keybound` facade
crate. It handles device access, backend selection, grab mode, and
hotplug so you just describe patterns and actions.

**You're building a compositor, editor, or framework that needs shortcut
matching.** Niri, COSMIC, Zed, and every tiling WM independently build
the same inner engine: key types, modifier tracking, binding tables,
layer stacks, sequence resolution. Use `kbd-core` directly. It's a
synchronous `Matcher` you drive from your own event loop — no threads,
no device access, no platform dependencies. You bring the events, it
tells you what matched.

**You're building a key remapper or input transformation tool.** A tool
that remaps CapsLock to Escape, implements vim-style layers (hjkl as
arrows), or adds tap-hold behavior (tap CapsLock = Esc, hold = Ctrl).
Use `keybound` with grab mode enabled. The library intercepts events
via evdev, matches them, and can emit different keys through a virtual
uinput device.

**You're building a sandboxed Wayland app that wants shortcuts.** A
Flatpak-packaged media player or communication tool that needs a
push-to-talk key or media controls without requiring device access. Use
`keybound` with the portal backend — it requests shortcuts through the
XDG GlobalShortcuts portal, mediated by the compositor.

### Who should use what

| You're building... | Use | Why |
|---|---|---|
| App with global hotkeys | `keybound` | Full stack: devices, matching, callbacks |
| Compositor / tiling WM | `kbd-core` + `kbd-evdev` | You have your own event loop and device access |
| GUI framework shortcuts | `kbd-core` | Embed the `Matcher` in your framework's event loop |
| Key remapper / macro tool | `keybound` (grab mode) | Intercept + transform + re-emit keys |
| Sandboxed Wayland app | `keybound` + `kbd-portal` | Desktop-mediated shortcuts, no root needed |

### The crate split

**`kbd-core`**: Pure-logic shortcut engine. Key types, modifier
tracking, binding matching, layer stacks, sequence resolution. No
platform dependencies. Embeddable in any event loop.

**`kbd-evdev`**: Linux evdev backend. Device discovery, hotplug, grab,
uinput forwarding.

**`kbd-portal`**: XDG GlobalShortcuts portal backend. D-Bus,
compositor-mediated, sandboxed.

**`kbd-xkb`**: Keyboard layout awareness via xkbcommon.

**`keybound`**: The facade. Threaded manager, backend selection, the
works. Most users start here.

## Where keybound sits in the Linux input stack

Linux keyboard input is layered. Each layer serves different consumers
and provides different capabilities:

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

**keybound operates at two levels:**

1. **evdev** (primary backend) — reads raw key events from kernel
   device nodes. This is the privileged path: it sees all keys
   regardless of which application has focus, can grab devices for
   exclusive access, and can inject remapped keys through uinput.
   Requires read access to `/dev/input/` (typically via the `input`
   group or seat management).

2. **XDG GlobalShortcuts portal** (secondary backend) — the
   unprivileged, desktop-mediated path for Wayland. Applications
   request shortcuts through D-Bus; the compositor decides whether
   to grant them. No device access needed, works in sandboxes
   (Flatpak), but limited to shortcut activation signals — no grab,
   no remapping, no raw key state.

**`kbd-core` operates at no platform level.** It's a pure-logic engine:
given a key event and some state, which binding matches? Consumers at
*any* layer — compositors processing libinput events, GUI apps
handling winit key events, even terminal tools — can drive the
`Matcher` from their own event loop.

### What keybound is not

**Not a text input library.** Text input (IME composition, dead keys,
Unicode output) is fundamentally different from key binding. A key press
is a physical event; text is the result of layout resolution, compose
sequences, and input method state. keybound deals in physical key
events. Text input belongs in xkbcommon, the toolkit's text input API,
or the compositor's text-input protocol.

**Not a terminal input handler.** Terminal apps receive keyboard input
through the TTY layer (termios), which encodes keys as byte sequences.
That's a different world with different constraints (no key release
events, escape sequence ambiguity). Libraries like crossterm handle
this. keybound's `kbd-core` *could* be useful if a terminal app maps
decoded key events to `Key` values and feeds them to the `Matcher`,
but keybound doesn't handle the terminal decoding itself.

**Not a GUI toolkit integration.** Toolkits (GTK, Qt) have their own
keyboard event systems with widget focus, accelerators, and input
method support. keybound doesn't replace those. The sweet spot is
global shortcuts (system-wide, outside any toolkit) or shared binding
logic (the `Matcher` used alongside toolkit events, not instead of
them).

## Physical keys vs logical keys

keybound works at the **physical key** level. `Key::A` means "the key
in the A position on a US QWERTY layout" — it's a position on the
keyboard, not the character it produces. On a Dvorak layout, that same
physical key produces "A" but is in a different position than you'd
expect from the label.

This is the right default for a hotkey library: most shortcuts are
defined by position (Ctrl+C means "the key where C is on QWERTY"),
and position-based bindings work across layouts without re-mapping.

The **logical key** level — "what character does this key produce
given the current layout?" — is a separate concern, handled by
xkbcommon and planned for `kbd-xkb` (Phase 4.9 in the plan). The
plan's `KeyReference` enum (`ByCode` / `BySymbol`) will support both
modes when needed.

### The key type and `keyboard-types`

The W3C UI Events specification defines two key concepts that map
directly to the physical/logical split:

- **`Code`** — physical key position (what keybound's `Key` represents)
- **`Key`** — logical key value, layout-aware (what `kbd-xkb` will add)

The [`keyboard-types`](https://crates.io/crates/keyboard-types) crate
implements these W3C types and is used by winit, iced, and most of the
Rust windowing ecosystem. Since keybound's `Key` represents the same
concept as `keyboard_types::Code`, there is an open question about
whether to adopt `Code` as the foundation for `Key` rather than
maintaining a parallel enum. See the "Alternative: Adopt
`keyboard-types` as the core key type" section in PLAN.md for the
detailed tradeoff analysis.

Regardless of that decision, the physical/logical distinction is
foundational: keybound binds physical key positions by default, with
layout-aware binding as an opt-in extension.

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
`keybound` facade, which drives a `Matcher` on an engine thread with
`kbd-evdev`/`kbd-portal` plumbing.

## The domain model

### Keys

A key is a key. Ctrl is a key. A is a key. CapsLock is a key. The
distinction between "modifier" and "key" is about *role in a combination*,
not about the key itself. In Ctrl+C, Ctrl is the modifier and C is the
trigger — but that's a property of the combination, not of Ctrl.

For the API, the Ctrl/Shift/Alt/Super abstraction is genuinely useful.
Users think in terms of "Ctrl+C", not "KEY_LEFTCTRL + KEY_C". And modifiers
need left/right canonicalization. So:

- `Key` is the complete set of keys, including modifier keys
- `Modifier` is a convenience type for the four common modifiers, handling
  left/right equivalence
- Internally, everything resolves to key codes

These two types share almost all their behavior: parsing from strings,
converting to/from evdev key codes, display formatting. The current code
duplicates all of this across two separate implementations. That duplication
goes away — either through a shared trait, a macro, or by deriving Modifier
from Key.

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

```
src/
  lib.rs              Public facade. Re-exports the curated API surface.
  error.rs            Error type (thiserror).

  key.rs              Key, Modifier, Hotkey, HotkeySequence.
                      Parsing (FromStr), display, evdev conversions (From/Into).
                      Single source of truth for all key-related logic.

  action.rs           Action enum.
  binding.rs          Binding, BindingOptions. Pattern enum (or patterns
                      are just Hotkey / HotkeySequence / TapHold config).
  layer.rs            Layer, LayerOptions.

  manager.rs          HotkeyManager. Thin public API.
                      Sends commands, returns handles.
  handle.rs           Handle (RAII unregistration via command).

  engine/
    mod.rs            Engine struct, event loop, core matching/dispatch.
    types.rs          GrabState, KeyEventDisposition, LayerEffect, MatchOutcome,
                      LayerStackEntry, LayerTimeout.
    binding.rs        RegisteredBinding (engine-internal binding storage).
    command.rs        Command enum, CommandSender (manager→engine channel).
    runtime.rs        EngineRuntime (spawn, shutdown, join).
    wake.rs           WakeFd (eventfd wrapper), LoopControl.
    key_state.rs      What's currently pressed. Modifier state derived here.
    matcher.rs        Binding lookup against current state.
    sequence.rs       Sequence state machine (partial progress, timeouts).
    tap_hold.rs       Tap-hold state machine (pending, resolved).
    devices.rs        Device discovery, hotplug, capability detection.
    forwarder.rs      uinput virtual device for event forwarding/emission.

  backend/
    mod.rs            Backend trait + selection logic.
    evdev.rs          evdev backend.
    portal.rs         XDG portal backend.

  events.rs           HotkeyEvent, async event stream (feature-gated).
```

**What's gone:**

- `config.rs` — the `ActionMap` / `HotkeyConfig` / `RegisteredConfig`
  application framework. Replaced by serde derives on the core types (`Key`,
  `Modifier`, `Hotkey`, `Action`, `Layer`) so users compose them into their
  own config structs.
- `manager/callbacks.rs`, `manager/registration.rs`, `manager/handles.rs`,
  `manager/options.rs` — the manager becomes thin enough to not need
  submodules. Handle moves to its own file.
- `listener/` (the entire directory) — replaced by `engine/`. The "listener"
  concept is subsumed by the engine, which both listens for events and
  processes commands.
- `mode/` (six files) — replaced by `layer.rs`. One file. The layer stack
  is managed by the engine, not a separate `ModeRegistry`.
- `key_state.rs` — moves into the engine where it belongs.
- `tap_hold.rs` (top-level) — moves into the engine.

**What's new:**

- `action.rs` — the `Action` enum, first-class output vocabulary.
- `binding.rs` — the unified `Binding` type.
- `engine/` — the engine that owns all state.
- `engine/matcher.rs` — binding lookup (replaces the scattered dispatch
  modules).

## Type inventory

What types does a user need to know? Roughly:

**Always needed (core):**
- `HotkeyManager` — entry point
- `Key` — a key
- `Modifier` — Ctrl/Shift/Alt/Super
- `Hotkey` — key + modifiers, parseable from strings
- `Handle` — keeps a binding alive
- `Error` — what went wrong

**When using power features:**
- `Action` — what happens on match (callback is just one variant)
- `Layer` / `LayerOptions` — named binding groups
- `TapHoldOptions` — timing for dual-function keys
- `HotkeySequence` — multi-step combos
- `BindingOptions` — per-binding configuration (device filter, passthrough,
  debounce)
- `DeviceFilter` — restrict to specific devices
- `Backend` — explicit backend selection

**When using async:**
- `HotkeyEvent` — event notifications
- `HotkeyEventStream` — async stream

That's ~15 types, down from 22+, and with a clearer hierarchy. The core 6
cover most use cases. The rest are opt-in for power users.

## What the simple API looks like

The simple case must stay simple:

```rust
use keybound::{HotkeyManager, Key, Modifier};

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
use keybound::{Action, HotkeyManager, Key, Layer, Modifier, TapHoldOptions};

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

The facade (`keybound`) handles backend selection. The core (`kbd-core`)
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
