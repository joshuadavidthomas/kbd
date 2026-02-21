# keybound: Implementation Plan

Ground-up rebuild based on [DESIGN.md](DESIGN.md).

Prior implementation archived in `archive/v0/` and tagged `v0-archive` in git.
Reference implementation (keyd) in `reference/keyd/`.

## How to use this plan

**Read [DESIGN.md](DESIGN.md) first.** It defines the domain model,
architecture, and design decisions. This plan is the task breakdown.

**Phases are sequential.** Each phase produces a working, testable library.
The output of one phase is the input for the next. Do not skip ahead.

**Each phase is self-contained for a fresh agent.** An agent picking up
Phase N needs: this file, DESIGN.md, the scaffolded `src/` files (with
their doc comments and TODO items), and optionally `archive/v0/` for
reference on how specific problems were solved before.

**When you finish a checklist item**, change its `- [ ]` to `- [x]`.

---

## Phase 1: Core types and the tracer bullet

**Goal**: `manager.register(Key::C, &[Modifier::Ctrl], || println!("fired"))`
works end-to-end. A key is physically pressed, a callback fires.

This phase builds the minimum vertical slice through the entire architecture:
types → manager → engine → evdev → callback. Everything else is layered on
in later phases.

### 1.1 Key types (`src/key.rs`)

Implement `Key`, `Modifier`, `Hotkey`, `HotkeySequence`.

- [x] `Key` enum with full key set (letters, numbers, F-keys, arrows, navigation, punctuation, numpad, modifiers).
- [x] `Modifier` enum (Ctrl, Shift, Alt, Super) with left/right canonicalization.
- [x] Shared logic between `Key` and `Modifier` — no duplicated `as_str()`, `Display`, or conversion implementations.
- [x] `From<evdev::KeyCode>` and `Into<evdev::KeyCode>` on `Key` (standard traits, not ad-hoc methods).
- [x] `Modifier` derivable from `Key` — `Modifier::Ctrl` maps to `Key::LeftCtrl`/`Key::RightCtrl`.
- [x] `Hotkey` struct (trigger `Key` + `Vec<Modifier>` or small set type). `FromStr` parses `"Ctrl+Shift+A"`, `Display` round-trips.
- [x] `HotkeySequence` struct (`Vec<Hotkey>`). `FromStr` parses `"Ctrl+K, Ctrl+C"`, `Display` round-trips.
- [x] Case-insensitive parsing with aliases (Super/Meta/Win, Ctrl/Control, Return/Enter).
- [x] Tests: parsing, display, round-trip, evdev conversion, modifier canonicalization.

Reference: `archive/v0/src/key.rs`, `archive/v0/src/hotkey.rs`

### 1.2 Action and binding types (`src/action.rs`, `src/binding.rs`)

- [x] `Action` enum with `Callback(Box<dyn Fn() + Send + Sync>)` variant. Other variants (`EmitKey`, `PushLayer`, etc.) defined but not yet functional — they exist in the type so the API is forward-compatible.
- [x] `impl<F: Fn() + Send + Sync + 'static> From<F> for Action` — closures auto-convert.
- [x] `BindingId` newtype (u64 or similar) for unique identification.
- [x] `BindingOptions` struct with `passthrough` field (as enum, not bool). Other option fields can be added in later phases.
- [x] `DeviceFilter` enum (name pattern, USB vendor/product) — type defined, filtering implemented in Phase 4.
- [x] Tests: Action from closure, BindingId uniqueness.

### 1.3 Error type (`src/error.rs`)

- [x] `Error` enum using `thiserror`.
- [x] Variants: `Parse`, `AlreadyRegistered`, `BackendInit`, `BackendUnavailable`, `PermissionDenied`, `DeviceError`, `UnsupportedFeature`, `ManagerStopped`, `EngineError`.
- [x] Absorb `ParseHotkeyError` — either as `Error::Parse` variant or as a separate type convertible via `From`.
- [x] Tests: error display messages are useful.

### 1.4 Engine skeleton (`src/engine/`)

The message-passing architecture.

- [x] `Command` enum: `Register`, `Unregister`, `Shutdown`. (Layer commands added in Phase 3.)
- [x] Reply mechanism: `Register` carries a oneshot sender for `Result<(), Error>`.
- [x] `Engine` struct that owns: bindings (`Vec` or `HashMap`), devices, key state.
- [x] `engine::run()` — event loop: `poll()` on device fds + wake fd, drain commands, process events.
- [x] Wake mechanism: eventfd (or pipe) so command sends wake the poll.
- [x] Shutdown: `Command::Shutdown` breaks the event loop, thread exits.
- [x] Tests: engine starts, accepts commands, shuts down cleanly.

Reference: `archive/v0/src/listener.rs` (event loop structure),
`archive/v0/src/listener/io.rs` (poll mechanics)

### 1.5 Device reading (`src/engine/devices.rs`)

- [x] Discover keyboard devices in `/dev/input/`.
- [x] Read key events from evdev devices (press, release, repeat).
- [x] Convert raw `KeyCode` to `Key`.
- [x] Device hotplug via inotify (add/remove devices at runtime).
- [x] Clean up key state on device disconnect.
- [x] Ignore non-keyboard devices.
- [x] Tests: device discovery, hotplug event parsing.

Reference: `archive/v0/src/listener/io.rs`, `archive/v0/src/listener/hotplug.rs`

### 1.6 Manager and handle (`src/manager.rs`, `src/handle.rs`)

- [x] `HotkeyManager::new()` — spawn engine thread, return manager.
- [x] `HotkeyManager::builder()` — builder for explicit backend/grab configuration.
- [x] `manager.register(key, modifiers, callback)` — sends `Command::Register`, waits for reply, returns `Handle`.
- [x] `Handle` holds `BindingId` + command sender. `Drop` sends `Command::Unregister`.
- [x] Conflict detection: registering a duplicate hotkey returns `Error::AlreadyRegistered`.
- [x] `manager.is_registered(key, modifiers)` — query via command/reply.
- [x] Tests: register, unregister via drop, conflict detection, shutdown.

### 1.7 Basic hotkey matching (`src/engine/matcher.rs`)

- [ ] Given a key event + current modifier state, find the matching binding.
- [ ] Modifier state derived from key state (what modifier keys are currently pressed).
- [ ] Match fires callback via `Action::Callback`.
- [ ] Unmatched events ignored (no grab mode yet).
- [ ] Tests: single hotkey match, modifier combinations, no match.

### 1.8 Integration and public API (`src/lib.rs`)

- [ ] Public re-exports are correct and minimal.
- [ ] The example from DESIGN.md compiles and works: `manager.register(Key::C, &[Modifier::Ctrl, Modifier::Shift], || ...)`.
- [ ] `cargo test` passes, `cargo clippy` clean, `cargo doc` builds.

### Phase 1 gate

| Section | Items |
|---------|-------|
| 1.1 Key types | 9/9 |
| 1.2 Action and binding | 6/6 |
| 1.3 Error type | 4/4 |
| 1.4 Engine skeleton | 7/7 |
| 1.5 Device reading | 7/7 |
| 1.6 Manager and handle | 7/7 |
| 1.7 Basic matching | 0/5 |
| 1.8 Integration | 0/3 |

---

## Phase 2: Grab mode, key state, and event forwarding

**Goal**: Grab mode works. Matched hotkeys are consumed, unmatched events
are forwarded through uinput. Key state is queryable.

### 2.1 Grab mode (`src/engine/devices.rs`, `src/engine/forwarder.rs`)

- [ ] `EVIOCGRAB` on devices when grab mode is enabled.
- [ ] Virtual uinput device creation for event forwarding.
- [ ] Unmatched key events forwarded through virtual device.
- [ ] Matched events consumed (not forwarded) by default.
- [ ] Passthrough option: matched events forwarded AND action executed.
- [ ] Self-detection: ignore our own virtual device in device discovery.
- [ ] Portal backend returns clear `UnsupportedFeature` error for grab.
- [ ] Tests: event consumption, forwarding, passthrough, self-detection.

Reference: `archive/v0/src/listener/forwarding.rs`,
`archive/v0/src/listener/io.rs` (EVIOCGRAB, self-detection)

### 2.2 Key state queries

- [ ] `manager.is_key_pressed(key)` — queries engine via command/reply.
- [ ] `manager.active_modifiers()` — returns set of held modifiers, derived from key state.
- [ ] Per-device key state tracking (for device-specific bindings in Phase 4).
- [ ] Modifier state cleaned up on device disconnect.
- [ ] Tests: key state during press/release, modifier derivation, disconnect cleanup.

### Phase 2 gate

| Section | Items |
|---------|-------|
| 2.1 Grab mode | 0/8 |
| 2.2 Key state queries | 0/5 |

---

## Phase 3: Layers

**Goal**: Named groups of bindings that stack. `Layer::new("nav").bind(...)`
works. Push/pop/toggle from callbacks and manager.

### 3.1 Layer definition and registration (`src/layer.rs`)

- [ ] `Layer` builder: `Layer::new("name").bind(key, mods, action).swallow().build()`.
- [ ] `LayerOptions`: oneshot (auto-pop after N keys), swallow (suppress unmatched), timeout (auto-pop after duration).
- [ ] `manager.define_layer(layer)` — sends layer definition to engine.
- [ ] Engine stores layers by name.
- [ ] Tests: layer construction, option configuration.

### 3.2 Layer stack operations

- [ ] `manager.push_layer("name")` / `manager.pop_layer()`.
- [ ] `Action::PushLayer` / `Action::PopLayer` / `Action::ToggleLayer` — layer control from within callbacks/bindings.
- [ ] Engine maintains layer stack. Matching walks stack top-down then global.
- [ ] Oneshot: layer auto-pops after N keypresses.
- [ ] Swallow: unmatched keys in the active layer are consumed, not passed to lower layers.
- [ ] Timeout: layer auto-pops after inactivity period.
- [ ] Tests: push/pop, stack priority, oneshot, swallow, timeout, same key in different layers.

### 3.3 Press cache (`src/engine/`)

- [ ] On key press, cache the action that was executed for that key.
- [ ] On key release, use cached action (not current matching result).
- [ ] Cache entries cleared after release processing.
- [ ] Correct release behavior across layer transitions (press in layer A, release after layer A is popped).
- [ ] Tests: layer pop during keypress, cache cleanup.

Reference: `reference/keyd/src/keyboard.c` (cache_entry system)

### Phase 3 gate

| Section | Items |
|---------|-------|
| 3.1 Layer definition | 0/5 |
| 3.2 Layer stack | 0/7 |
| 3.3 Press cache | 0/5 |

---

## Phase 4: Sequences, tap-hold, device filtering, and polish

**Goal**: All power features from v0 are reimplemented on the new
architecture. Library is feature-complete relative to v0.

### 4.1 Key sequences (`src/engine/sequence.rs`)

- [ ] Multi-step hotkey sequences with configurable timeout.
- [ ] Sequence completes → callback fires.
- [ ] Timeout → reset (fire standalone binding if one exists).
- [ ] Wrong key → reset.
- [ ] Abort key (configurable, default Escape).
- [ ] Overlapping prefixes handled (standalone fires on timeout if no next step).
- [ ] Multiple active sequences tracked independently.
- [ ] Tests: complete, timeout, wrong key, abort, overlapping prefixes, concurrent.

Reference: `archive/v0/src/listener/sequence.rs`

### 4.2 Tap-hold (`src/engine/tap_hold.rs`)

- [ ] Tap resolves on release before threshold.
- [ ] Hold resolves on threshold expiry.
- [ ] Hold resolves on interrupting keypress (keyd model).
- [ ] Tap-hold requires grab mode; clear error without it.
- [ ] Tap action produces visible key event (synthetic emission via forwarder).
- [ ] Tests: tap, hold by duration, hold by interrupt, missing-grab error.

Reference: `archive/v0/src/tap_hold.rs`

### 4.3 Device-specific bindings

- [ ] `BindingOptions::device(DeviceFilter::Name("..."))` restricts binding to specific devices.
- [ ] Device filter matching in the engine's matcher (per-binding check).
- [ ] Per-device modifier isolation (modifier on device A doesn't satisfy binding on device B).
- [ ] Global bindings use aggregate modifier state.
- [ ] Tests: device match, device miss, modifier isolation, global aggregate.

Reference: `archive/v0/src/listener/dispatch.rs` (device-specific dispatch)

### 4.4 Debounce and rate limiting

- [ ] Per-binding debounce (suppress triggers within time window).
- [ ] Per-binding rate limit (cap invocations per interval).
- [ ] Tests: debounce suppression, rate limiting.

### 4.5 Portal backend (`src/backend/portal.rs`)

- [ ] XDG GlobalShortcuts portal implementation.
- [ ] Auto-detection: try portal, fall back to evdev.
- [ ] Explicit backend selection via builder.
- [ ] Clear errors when portal unavailable or feature not compiled.
- [ ] Tests: backend selection, fallback, feature-gated errors.

Reference: `archive/v0/src/backend.rs` (portal implementation)

### 4.6 Async event stream (`src/events.rs`)

- [ ] `HotkeyEvent` enum: `Pressed`, `Released`, `LayerChanged`, `SequenceStep`.
- [ ] `HotkeyEventStream` for async consumers (feature-gated: `tokio`, `async-std`).
- [ ] Engine emits events; stream consumes them.
- [ ] Stream completes on manager shutdown.
- [ ] Tests: event delivery, shutdown completion.

### 4.7 Serde support

- [ ] `Serialize`/`Deserialize` on `Key`, `Modifier`, `Hotkey`, `HotkeySequence`, `Action` (data variants only), `Layer`, `LayerOptions`, `BindingOptions`.
- [ ] Behind `serde` feature flag.
- [ ] No `ActionMap`/`HotkeyConfig` — users compose types into their own configs.
- [ ] Tests: round-trip serialization.

### Phase 4 gate

| Section | Items |
|---------|-------|
| 4.1 Sequences | 0/8 |
| 4.2 Tap-hold | 0/6 |
| 4.3 Device filtering | 0/5 |
| 4.4 Debounce/rate limit | 0/3 |
| 4.5 Portal backend | 0/5 |
| 4.6 Async events | 0/5 |
| 4.7 Serde | 0/4 |

---

## Phase 5: Key remapping and event transformation

**Goal**: The library can emit different keys than what was pressed.
`Action::EmitKey` works. This is the phase that makes keybound a
transformation engine, not just a detection library.

### 5.1 Key emission

- [ ] `Action::EmitKey(Key, Vec<Modifier>)` emits a key event through the uinput forwarder.
- [ ] `Action::EmitSequence` emits a series of key events with configurable inter-key delay.
- [ ] `Action::Command(String)` executes a shell command asynchronously.
- [ ] Emission requires grab mode; clear error without it.
- [ ] Tests: emit key visible to virtual device, emit sequence timing, command execution.

### 5.2 Key remapping

- [ ] `manager.remap(from_key, from_mods, to_key, to_mods)` — convenience for common case.
- [ ] Remap uses press cache for correct releases (press emits remapped key, release emits remapped release).
- [ ] Remaps coexist with callback bindings.
- [ ] Tests: simple remap, modifier-changing remap, release correctness, coexistence.

### 5.3 Oneshot layers

- [ ] Oneshot layer applies transformation to *any* next keypress, not just registered bindings.
- [ ] Default action for unbound keys in a layer (e.g., apply Shift modifier).
- [ ] Configurable depth (deactivate after N keypresses).
- [ ] Modifier-only keys don't consume the oneshot.
- [ ] Tests: basic oneshot, sticky modifier, depth, modifier passthrough.

### 5.4 Overload variants

- [ ] `OverloadStrategy::Basic` (current behavior — hold on threshold or interrupt).
- [ ] `OverloadStrategy::Timeout` (ignore interrupts, resolve purely on duration).
- [ ] `OverloadStrategy::TimeoutTap` (tap only within window, else hold).
- [ ] `OverloadStrategy::IdleTimeout` (use idle time before keypress for disambiguation).
- [ ] Tests: each strategy, fast-typing scenarios.

### Phase 5 gate

| Section | Items |
|---------|-------|
| 5.1 Key emission | 0/5 |
| 5.2 Key remapping | 0/4 |
| 5.3 Oneshot layers | 0/5 |
| 5.4 Overload variants | 0/5 |

---

## Phase 6: Stretch goals (build if demand exists)

Not required for the library to be compelling. Build when a downstream
project or user request demonstrates real need.

### 6.1 Chord support

- [ ] Simultaneous non-modifier keys recognized as a chord within a time window.
- [ ] Timeout flushes buffered keys as normal keypresses.
- [ ] Prefix disambiguation (j+k doesn't block j+k+l).
- [ ] Per-layer chord definitions.
- [ ] Requires grab mode.
- [ ] Tests: successful chord, timeout, disambiguation, per-layer, missing-grab.

Reference: `reference/keyd/src/keyboard.c` (chord state machine)

### 6.2 Non-key event support

- [ ] Mouse button events trigger bindings.
- [ ] Scroll wheel events trigger bindings or are remappable.
- [ ] Actions can emit mouse/scroll events via separate virtual pointer device.
- [ ] Tests: mouse button binding, scroll remap, dual virtual device.

### 6.3 Full layer keymaps

- [ ] Layer defines complete keymap (remap for every key position).
- [ ] Keymaps compose via layer stack.
- [ ] Integrates with press cache for correct releases.
- [ ] Tests: keymap layer, composition, layout switching.

---

## Phase 7: Cross-platform expansion (not committed)

- [ ] macOS backend (CGEventTap / IOKit).
- [ ] Windows backend (low-level keyboard hooks).
- [ ] Platform-neutral crate rename if not already done.

---

## Implementation order summary

| Phase | Delivers | Items |
|-------|----------|-------|
| **1** | Core types + basic hotkeys (the tracer bullet) | 48 |
| **2** | Grab mode + key state | 13 |
| **3** | Layers | 17 |
| **4** | Sequences, tap-hold, device filtering, portal, async, serde | 36 |
| **5** | Key remapping and transformation | 19 |
| **6** | Stretch: chords, mouse, full keymaps | 11+ |
| **7** | Cross-platform | 3 |

Phase 1 makes it work. Phase 2 makes it intercept. Phase 3 makes it modal.
Phase 4 makes it feature-complete. Phase 5 makes it a transformation engine.

---

## Quality gates (all phases)

1. **All tests pass.** No skipped, no ignored.
2. **`cargo clippy` clean** with pedantic lints.
3. **`cargo doc` builds** with no warnings.
4. **No `// SMELL:` comments** — if something smells, fix it or file an issue.
5. **No `Arc<Mutex<>>`** in the engine — it owns its state exclusively.
6. **No bool fields** in public or internal types — use enums.
7. **No duplicated logic** between `Key` and `Modifier`.
8. **Callbacks panic-isolated** — a panicking callback never kills the engine.
