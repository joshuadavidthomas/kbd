# keybound: Redesign

This document captures the core ideas, domain model, and architectural
direction for restructuring keybound. It's not a task list — it's the
conceptual foundation that the task list should serve.

See [ATTRIBUTION.md](ATTRIBUTION.md) for licensing constraints on
reference projects. Some can be adapted (MIT), others are
inspiration-only (GPL).

## What is this library?

Two layers:

**Core** (`kbd-core`): A pure-logic keyboard shortcut engine.
Key types, modifier tracking, binding matching, layer stacks, sequence
resolution — the parts that every Rust project rebuilds from scratch.
No platform dependencies. Embeddable in any event loop.

**Backends** (`kbd-evdev`, `kbd-portal`, `kbd-xkb`): Platform-specific
crates, each isolated behind its own dependency boundary. evdev for
Linux input devices. Portal for Wayland's XDG GlobalShortcuts. XKB for
keyboard layout awareness.

**Facade** (`keybound`): A Linux global hotkey library built on the
core engine and backends. Adds the threaded manager, backend selection,
grab mode, device hotplug — the platform complexity so users just
describe patterns and actions.

One sentence for the core: **Given a key event and some state, which
binding matches?**

One sentence for the facade: **When a specific pattern of keys happens
on a Linux input device, do something.**

The core exists because Zed, COSMIC, Niri, and every Rust compositor
and editor independently build the same matching engine. The facade
exists because no Rust crate handles Linux hotkeys properly —
especially on Wayland.

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
    mod.rs            Engine. Event loop. Command processing.
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
