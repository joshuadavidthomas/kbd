# keybound: Implementation Plan

Ground-up rebuild based on [DESIGN.md](DESIGN.md).

Prior implementation archived in `archive/v0/` and tagged `v0-archive` in git.
Reference implementation (keyd) in `reference/keyd/`.

## How to use this plan

**Read [DESIGN.md](DESIGN.md) first.** It defines the domain model,
architecture, and design decisions. This plan is the task breakdown.

**Read [ATTRIBUTION.md](ATTRIBUTION.md) before referencing other
projects.** Some are MIT-compatible (keyd, global-hotkey,
livesplit-hotkey) — code can be adapted with attribution. Others are
GPL (Niri, COSMIC, Zed editor) — inspiration only, clean-room
implementation required.

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

- [x] Given a key event + current modifier state, find the matching binding.
- [x] Modifier state derived from key state (what modifier keys are currently pressed).
- [x] Match fires callback via `Action::Callback`.
- [x] Unmatched events ignored (no grab mode yet).
- [x] Tests: single hotkey match, modifier combinations, no match.

### 1.8 Integration and public API (`src/lib.rs`)

- [x] Public re-exports are correct and minimal.
- [x] The example from DESIGN.md compiles and works: `manager.register(Key::C, &[Modifier::Ctrl, Modifier::Shift], || ...)`.
- [x] `cargo test` passes, `cargo clippy` clean, `cargo doc` builds.

### Phase 1 gate

| Section | Items |
|---------|-------|
| 1.1 Key types | 9/9 |
| 1.2 Action and binding | 6/6 |
| 1.3 Error type | 4/4 |
| 1.4 Engine skeleton | 7/7 |
| 1.5 Device reading | 7/7 |
| 1.6 Manager and handle | 7/7 |
| 1.7 Basic matching | 5/5 |
| 1.8 Integration | 3/3 |

---

## Phase 2: Grab mode, key state, and event forwarding

**Goal**: Grab mode works. Matched hotkeys are consumed, unmatched events
are forwarded through uinput. Key state is queryable.

### 2.1 Grab mode (`src/engine/devices.rs`, `src/engine/forwarder.rs`)

- [x] `EVIOCGRAB` on devices when grab mode is enabled.
- [x] Virtual uinput device creation for event forwarding.
- [x] Unmatched key events forwarded through virtual device.
- [x] Matched events consumed (not forwarded) by default.
- [x] Passthrough option: matched events forwarded AND action executed.
- [x] Self-detection: ignore our own virtual device in device discovery.
- [x] Portal backend returns clear `UnsupportedFeature` error for grab.
- [x] Tests: event consumption, forwarding, passthrough, self-detection.

Reference: `archive/v0/src/listener/forwarding.rs`,
`archive/v0/src/listener/io.rs` (EVIOCGRAB, self-detection)

### 2.2 Key state queries

- [x] `manager.is_key_pressed(key)` — queries engine via command/reply.
- [x] `manager.active_modifiers()` — returns set of held modifiers, derived from key state.
- [x] Per-device key state tracking (for device-specific bindings in Phase 4).
- [x] Modifier state cleaned up on device disconnect.
- [x] Tests: key state during press/release, modifier derivation, disconnect cleanup.

### Phase 2 gate

| Section | Items |
|---------|-------|
| 2.1 Grab mode | 8/8 |
| 2.2 Key state queries | 5/5 |

---

## Phase 3: Layers

**Goal**: Named groups of bindings that stack. `Layer::new("nav").bind(...)`
works. Push/pop/toggle from callbacks and manager.

### 3.1 Layer definition and registration (`src/layer.rs`)

- [x] `Layer` builder: `Layer::new("name").bind(key, mods, action).swallow().build()`.
- [x] `LayerOptions`: oneshot (auto-pop after N keys), swallow (suppress unmatched), timeout (auto-pop after duration).
- [x] `manager.define_layer(layer)` — sends layer definition to engine.
- [x] Engine stores layers by name.
- [x] Tests: layer construction, option configuration.

### 3.2 Layer stack operations

- [x] `manager.push_layer("name")` / `manager.pop_layer()`.
- [x] `Action::PushLayer` / `Action::PopLayer` / `Action::ToggleLayer` — layer control from within callbacks/bindings.
- [x] Engine maintains layer stack. Matching walks stack top-down then global.
- [x] Oneshot: layer auto-pops after N keypresses.
- [x] Swallow: unmatched keys in the active layer are consumed, not passed to lower layers.
- [x] Timeout: layer auto-pops after inactivity period.
- [x] Tests: push/pop, stack priority, oneshot, swallow, timeout, same key in different layers.

### 3.3 Press cache (`src/engine/`)

- [x] On key press, cache the action that was executed for that key.
- [x] On key release, use cached action (not current matching result).
- [x] Cache entries cleared after release processing.
- [x] Correct release behavior across layer transitions (press in layer A, release after layer A is popped).
- [x] Tests: layer pop during keypress, cache cleanup.

Reference: `reference/keyd/src/keyboard.c` (cache_entry system)

### 3.4 Binding metadata

Every project that builds shortcut UIs — Zed's keymap editor, Niri's
hotkey overlay — needs metadata on bindings. Without it, consumers
rebuild the same "description + visibility" plumbing on their own.

- [x] `description: Option<String>` on `BindingOptions` — human-readable label ("Copy to clipboard").
- [x] `OverlayVisibility` enum (`Visible` / `Hidden`) on `BindingOptions` — lets consumers build hotkey overlays where some bindings are excluded (Niri's `hotkey-overlay-title=null` pattern).
- [x] `description: Option<String>` on `LayerOptions` — layer-level label for overlay grouping.
- [x] Tests: metadata round-trips through register/introspect.

### 3.5 Introspection API

The #1 thing apps need beyond "register and fire." Zed builds a full
keymap editor with conflict tooltips. Niri has a `show-hotkey-overlay`
action. Without introspection, every consumer rebuilds this from scratch.

Layers make introspection interesting — "what's active, what's shadowed"
only matters once a layer stack exists.

- [x] `manager.list_bindings()` → returns active bindings with key, description, layer, and shadowed status.
- [x] `manager.bindings_for_key(key, mods)` → what would fire if this key were pressed now (considering layer stack).
- [x] `manager.active_layers()` → current layer stack with names and options.
- [x] `manager.conflicts()` → bindings shadowed by higher-priority layers.
- [x] All queries via command/reply through existing message-passing channel.
- [x] Tests: introspect after register, introspect with layers, shadowed binding detection.

### Phase 3 gate

| Section | Items |
|---------|-------|
| 3.1 Layer definition | 5/5 |
| 3.2 Layer stack | 7/7 |
| 3.3 Press cache | 5/5 |
| 3.4 Binding metadata | 4/4 |
| 3.5 Introspection | 6/6 |

---

## Phase 3.5: Workspace split and core extraction

**Goal**: Split keybound into a multi-crate workspace. Each crate
boundary is a dependency boundary — consumers pull in only what they
need.

Every project we studied — Zed, COSMIC, Niri, every tiling WM — builds
the same inner engine: key types, modifier tracking, layer/context
stack, binding matching, sequence resolution. The only difference
between global and in-app is where events come from and what "context"
means. This phase recognizes that keybound already has that engine and
makes it independently usable.

This happens before Phase 4 because Phase 4 adds many features
(sequences, tap-hold, portal, serde, XKB). Each one lands in a
specific crate with clear ownership. The portal backend (4.5) and XKB
support (4.9) each become their own crate rather than feature flags
pulling heavy deps into a monolith.

### Crate layout

```
keybound/                         workspace root
├── crates/
│   ├── kbd-core/                 Pure types + matcher + layers
│   ├── kbd-evdev/                evdev backend
│   ├── kbd-portal/               XDG GlobalShortcuts portal backend
│   ├── kbd-xkb/                  Keyboard layout awareness
│   ├── kbd-derive/               #[derive(Bindings)] proc macro
│   └── keybound/                 Facade — HotkeyManager, ties it all together
```

### What goes where

**`kbd-core`** — zero platform deps, the thing everyone can use.

- `Key`, `Modifier`, `Hotkey`, `HotkeySequence` (types + parsing)
- `Action`, `Binding`, `BindingOptions`, `BindingId`
- `Layer`, `LayerOptions`, `LayerName`
- `Matcher`, `MatchResult`, `KeyState` (the synchronous engine)
- Core error types (parse, conflict, layer)
- Only external dep: `thiserror`
- Optional feature flags: `serde` (derives), `winit` (key conversions)

**`kbd-evdev`** — Linux input device layer.

- Device discovery, hotplug (inotify), `EVIOCGRAB`
- `Forwarder` (uinput virtual device)
- `From<evdev::KeyCode> for Key` and reverse — the evdev↔core bridge
- Device filtering, self-detection
- Deps: `evdev`, `kbd-core`

**`kbd-portal`** — Wayland-friendly, no root needed.

- XDG GlobalShortcuts portal implementation (DBus via `ashpd`)
- Session management, shortcut binding/activation signals
- Deps: `ashpd`, `kbd-core`
- Pulls in async — isolated here so it doesn't infect the rest

**`kbd-xkb`** — keyboard layout awareness.

- xkbcommon integration: keycode → keysym resolution
- `KeyReference` enum (`ByCode` / `BySymbol`)
- Layout change detection
- Deps: `xkbcommon`, `kbd-core`

**`kbd-derive`** — proc macro (Phase 4+ timeframe, crate created now).

- `#[derive(Bindings)]`, `#[hotkey(...)]`, `#[flatten]`
- Compile-time hotkey string validation
- Deps: `syn`, `quote`, `proc-macro2`, `kbd-core`
- Starts as an empty crate with a doc comment explaining intent

**`keybound`** — the facade, what most global-hotkey users depend on.

- `HotkeyManager`, `Handle` — threaded engine + message passing
- `ConsumePreference`, backend selection logic
- Re-exports everything from `kbd-core`
- Lock/inhibitor awareness, context hooks
- Deps: `kbd-core`, optional `kbd-evdev`, `kbd-portal`, `kbd-xkb`,
  `kbd-derive`

### Why this split

Each boundary is a **dependency boundary**:

| Crate | Key external dep | Why separate |
|-------|-----------------|--------------|
| `kbd-core` | none | The whole point — zero deps, anyone can use it |
| `kbd-evdev` | `evdev` | Linux C library, needs `/dev/input` access |
| `kbd-portal` | `ashpd` (async DBus) | Pulls in async runtime, different paradigm |
| `kbd-xkb` | `xkbcommon` | Optional C library, not everyone needs layouts |
| `kbd-derive` | `syn`/`quote` | Proc macros must be separate crates (Rust req) |
| `keybound` | all of the above | Glue + the threaded manager |

Things that stay as feature flags, not crates:

- **Serde** — just derives on `kbd-core` types, not a dep boundary
- **Winit conversions** — small `From` impls in `kbd-core`, feature-gated
- **Async event streams** — thin wrappers in `keybound`, feature-gated
  on `tokio` / `async-std`

### Consumer matrix

| Consumer | Depends on |
|----------|-----------|
| Iced/Dioxus app with own shortcuts | `kbd-core` |
| Iced app + layout awareness | `kbd-core` + `kbd-xkb` |
| Tauri-style app needing global hotkeys | `keybound` |
| Compositor (Niri-like) | `kbd-core` + `kbd-evdev` (direct, no manager) |
| Flatpak sandboxed app | `keybound` with `kbd-portal` |
| Declarative bindings | `keybound` + `kbd-derive` |

### 3.6 Workspace scaffolding

- [x] Create `crates/` directory with all six crate dirs and `Cargo.toml` for each.
- [x] Root `Cargo.toml` becomes workspace manifest with `members = ["crates/*"]`.
- [x] `kbd-core/Cargo.toml`: only `thiserror`, optional `serde` feature.
- [x] `kbd-evdev/Cargo.toml`: `evdev`, `kbd-core`.
- [x] `kbd-portal/Cargo.toml`: `ashpd`, `kbd-core`. Starts as stub with `unimplemented!()` entry points and a doc comment.
- [x] `kbd-xkb/Cargo.toml`: placeholder, no deps yet. Doc comment explains Phase 4.9.
- [x] `kbd-derive/Cargo.toml`: placeholder `proc-macro` crate. Doc comment explains future intent.
- [x] `keybound/Cargo.toml`: depends on `kbd-core`, optional deps on `kbd-evdev`, `kbd-portal`, `kbd-xkb`, `kbd-derive`.
- [x] All crates compile. `cargo build --workspace` succeeds.

### 3.7 Move types into `kbd-core`

- [x] Move `key.rs`, `action.rs`, `binding.rs`, `layer.rs` into `kbd-core/src/`.
- [x] Move `engine/matcher.rs`, `engine/key_state.rs` into `kbd-core/src/` (these are pure logic).
- [x] Move core error variants into `kbd-core/src/error.rs`.
- [x] `evdev::KeyCode` conversions stay in `kbd-core` behind `evdev` feature flag — orphan rule requires `From<ForeignType> for LocalType` to be in the crate that defines the local type. `kbd-evdev` cannot implement `From<KeyCode> for Key` because `Key` is defined in `kbd-core`.
- [x] `kbd-core` builds and tests pass independently: `cargo test -p kbd-core`.

### 3.8 Move evdev code into `kbd-evdev`

- [ ] Move `engine/devices.rs` into `kbd-evdev/src/`.
- [ ] Move `engine/forwarder.rs` into `kbd-evdev/src/`.
- [ ] Move `From<evdev::KeyCode>` / `Into<evdev::KeyCode>` impls into `kbd-evdev`.
- [ ] `kbd-evdev` exposes a backend trait or struct that `keybound` consumes.
- [ ] `kbd-evdev` builds and tests pass: `cargo test -p kbd-evdev`.

### 3.9 Public synchronous `Matcher` in `kbd-core`

The `Matcher` is the embeddable engine. No threads, no channels, no
evdev. Consumers drive it from their own event loop — winit, GPUI,
Smithay, a game loop, whatever.

```rust
use kbd_core::{Matcher, Key, Modifier, Hotkey, Layer, Action};

let mut matcher = Matcher::new();
matcher.register(Hotkey::parse("Ctrl+S")?, Action::from(|| save()));

// In your event loop — you bring the events:
let hotkey = Hotkey::new(key, mods);
match matcher.process(hotkey, transition) {
    MatchResult::Matched { action, .. } => action.execute(),
    MatchResult::Pending { .. } => show_sequence_indicator(),
    MatchResult::NoMatch => pass_to_focused_widget(),
    MatchResult::Swallowed => {}
}
```

- [ ] `Matcher` as a public synchronous type: `matcher.process(hotkey, transition) → MatchResult`.
- [ ] `MatchResult::Pending` variant for mid-sequence state — consumers need this for UI feedback ("waiting for next key…").
- [ ] `Matcher` exposes layer operations directly: `push_layer()`, `pop_layer()`, `toggle_layer()`, `define_layer()`.
- [ ] `Matcher` exposes introspection: `list_bindings()`, `bindings_for_key()`, `active_layers()`, `conflicts()`.
- [ ] `HotkeyManager` in `keybound` wraps `Matcher` internally — the message-passing architecture stays, it just drives a `Matcher` on the engine thread.
- [ ] Tests: `Matcher` used standalone without any `HotkeyManager` or engine thread.

### 3.10 Rewire `keybound` facade

- [ ] `keybound` re-exports all `kbd-core` public types — existing public API unchanged.
- [ ] `HotkeyManager` uses `kbd-evdev` for device management (behind `evdev` feature).
- [ ] `HotkeyManager` uses `kbd-portal` for portal backend (behind `portal` feature).
- [ ] Existing integration tests pass against the `keybound` crate: `cargo test -p keybound`.
- [ ] `cargo test --workspace` passes.

### 3.11 Windowing library conversions

- [ ] Feature-gated `From<winit::keyboard::KeyCode> for Key` in `kbd-core` behind `winit` feature flag.
- [ ] Conversion coverage: all keys that have equivalents in both enums, others map to `Key::Unknown`.
- [ ] Other frameworks (Smithay keysyms, etc.) added on demand via additional feature flags.
- [ ] Tests: round-trip conversion for common keys.

### Phase 3.5 gate

| Section | Items |
|---------|-------|
| 3.6 Workspace scaffolding | 9/9 |
| 3.7 Move types to kbd-core | 5/5 |
| 3.8 Move evdev to kbd-evdev | 0/5 |
| 3.9 Public Matcher | 0/6 |
| 3.10 Rewire keybound facade | 0/5 |
| 3.11 Windowing conversions | 0/4 |

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
- [ ] Pending state exposed: `MatchResult::Pending { steps_matched, steps_remaining }` so consumers can show progress mid-sequence (Zed returns this for "Ctrl+K → waiting…" UI).
- [ ] `manager.pending_sequence()` → current in-progress sequence info (if any), for UI display.
- [ ] Event stream (§4.6) emits `SequenceStep` events as each step matches.
- [ ] Tests: complete, timeout, wrong key, abort, overlapping prefixes, concurrent, pending state queries.

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

### 4.4 Debounce, rate limiting, and repeat policy

- [ ] Per-binding debounce (suppress triggers within time window).
- [ ] Per-binding rate limit (cap invocations per interval).
- [ ] `BindingOptions::repeat_policy(RepeatPolicy)` — `Allow`, `Suppress`, `Custom { rate, delay }`.
- [ ] Engine filters evdev repeat events per-binding based on policy. Distinct from debounce: repeat is the OS auto-repeating a held key, debounce suppresses rapid re-presses.
- [ ] Tests: debounce suppression, rate limiting, repeat suppression, custom repeat rate, interaction between debounce and repeat.

### 4.5 Portal backend and consume preference (`kbd-portal`)

- [ ] XDG GlobalShortcuts portal implementation in `kbd-portal` crate.
- [ ] Auto-detection in `keybound`: try portal, fall back to evdev.
- [ ] Explicit backend selection via `HotkeyManager::builder()`.
- [ ] Clear errors when portal unavailable or `kbd-portal` not compiled.
- [ ] `ConsumePreference` enum in `kbd-core`: `NoPreference`, `PreferConsume`, `PreferNoConsume`, `MustConsume`, `MustNotConsume` (proven model from livesplit-hotkey — real users have different permission levels and sandbox constraints).
- [ ] Builder-level: `HotkeyManager::builder().consume_preference(ConsumePreference::PreferConsume)`.
- [ ] Preference guides backend selection: `MustConsume` → needs grab or portal, fails on plain evdev. `MustNotConsume` → plain evdev only. `PreferConsume` → tries grab/portal first, falls back to observe.
- [ ] Tests: backend selection, fallback, feature-gated errors, preference-guided selection, failure on impossible preference.

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

### 4.8 Modifier aliases

Every tiling WM has a "Mod key" concept — Niri's `COMPOSITOR` modifier,
i3/sway's `$mod`, Hyprland's `SUPER`. Users swap between Super, Alt,
etc. Without aliases, consumers hand-roll string replacement before
parsing hotkeys.

- [ ] `manager.define_modifier_alias("Mod", Modifier::Super)` — abstract modifier that resolves at runtime.
- [ ] `Hotkey::parse("Mod+T")` accepts aliases. Alias resolution happens in the matcher, not in parsing — bindings are portable across alias configurations.
- [ ] Aliases configurable on `Matcher` directly (for `kbd-core` consumers) and via command/reply on `HotkeyManager`.
- [ ] Alias reassignment: changing "Mod" from Super to Alt updates resolution for all existing bindings using that alias.
- [ ] Tests: alias definition, resolution during matching, reassignment, unknown alias errors.

### 4.9 Keyboard layout awareness (`kbd-xkb`)

keybound works at the evdev keycode level, which is position-based. On a
Dvorak layout, `Key::S` is still physical position S (which types "O").
COSMIC and Niri both solved this with xkbcommon because real users
switch layouts. This is the difference between "works for QWERTY
Americans" and "works for everyone."

- [ ] `KeyReference` enum in `kbd-core`: `ByCode(Key)` (position-based, current behavior) | `BySymbol(Keysym)` (character-based, layout-aware). Core type, no xkb dep.
- [ ] xkbcommon integration in `kbd-xkb`: resolve keycodes → keysyms based on active XKB layout.
- [ ] Hotkey parsing disambiguation: `"Ctrl+a"` (character) vs `"Ctrl+KeyA"` (position), or equivalent scheme.
- [ ] Layout change detection in `kbd-xkb`: subscribe to xkb layout change events, re-resolve symbol-based bindings.
- [ ] `keybound` facade integrates `kbd-xkb` when the `xkb` feature is enabled.
- [ ] `kbd-core` `Matcher` handles `KeyReference` natively — symbol resolution provided by `kbd-xkb`, but matching logic is in core.
- [ ] Tests: QWERTY vs Dvorak binding resolution, layout switch mid-session, mixed code/symbol bindings.

### 4.10 Binding provenance

Zed tracks whether a binding came from "base keymap", "vim extension",
or "user keymap" for conflict resolution and display. Without
provenance, consumers rebuild source tracking on their own — especially
when loading defaults + user overrides from config files (§4.7 serde).

- [ ] `BindingSource` newtype (wraps a string label: `"default"`, `"user"`, `"plugin"`, or custom).
- [ ] `BindingOptions::source(BindingSource::new("user"))`.
- [ ] Introspection API (§3.5) returns source info per binding.
- [ ] Optional source-aware precedence: user-sourced bindings override default-sourced for the same hotkey, without requiring explicit unregister + re-register.
- [ ] Tests: source tagging, source in introspection results, source-aware conflict resolution.

### Phase 4 gate

| Section | Items |
|---------|-------|
| 4.1 Sequences | 0/11 |
| 4.2 Tap-hold | 0/6 |
| 4.3 Device filtering | 0/5 |
| 4.4 Debounce/rate/repeat | 0/5 |
| 4.5 Portal/consume pref | 0/8 |
| 4.6 Async events | 0/5 |
| 4.7 Serde | 0/4 |
| 4.8 Modifier aliases | 0/5 |
| 4.9 XKB layout | 0/7 |
| 4.10 Provenance | 0/5 |

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

### 5.5 Lock and inhibitor awareness

Niri has `allow-when-locked` and `allow-inhibiting` per binding. These
are real Wayland concepts — compositors can inhibit keyboard shortcuts
(e.g., during screen sharing), and some bindings (media keys,
push-to-talk) should work regardless of lock state.

- [ ] `BindingOptions::allow_when_locked()` — binding fires even when screen is locked.
- [ ] `BindingOptions::allow_when_inhibited()` — binding fires even when compositor inhibits shortcuts.
- [ ] Engine queries lock/inhibitor state from compositor (Wayland-specific, portal-mediated where available).
- [ ] Graceful degradation: on backends that can't detect lock/inhibitor state, all bindings fire (current behavior preserved).
- [ ] Tests: lock-aware filtering, inhibitor-aware filtering, graceful fallback on unaware backends.

### 5.6 External context hooks

Global hotkey consumers sometimes want to condition on external state —
focused application, active workspace, user-defined modes. The layer
stack is the right primitive; this section makes the pattern explicit
rather than leaving consumers to reinvent it.

- [ ] `ContextEvent` type consumers can send to the engine: `ContextEvent::FocusChanged { app_id: String }`, `ContextEvent::Custom(String)`.
- [ ] `manager.send_context(event)` — inject external context change into the engine.
- [ ] Layer definitions can declare `activate_on` context predicates — simple string matching (e.g., `activate_on: "app_id == firefox"`), not a full expression language.
- [ ] Automatic layer push/pop when context predicates match/unmatch.
- [ ] Pattern documentation: "subscribe to your compositor's focus-change signal, send `ContextEvent::FocusChanged` — keybound handles the layer transitions."
- [ ] Tests: external event triggers layer push, predicate matching, auto-pop on context change.

### Phase 5 gate

| Section | Items |
|---------|-------|
| 5.1 Key emission | 0/5 |
| 5.2 Key remapping | 0/4 |
| 5.3 Oneshot layers | 0/5 |
| 5.4 Overload variants | 0/5 |
| 5.5 Lock/inhibitor | 0/5 |
| 5.6 Context hooks | 0/6 |

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

## Future idea: derive macro for declarative bindings

Not planned for any phase. Captured here so the idea isn't lost.

The builder API (`Hotkey::new(Key::C).modifier(Modifier::Ctrl)`,
`Layer::new("nav").bind(...)`) follows standard Rust builder conventions.
A complementary derive macro could offer a declarative alternative for
bindings, similar to how clap offers both `Command::new()` and
`#[derive(Parser)]`.

### The core pattern

The struct IS the state — each field is a `Handle`, and dropping the
struct unregisters everything. Follows the clap model where
`#[derive(Parser)]` generates a `FromArgMatches` impl that populates
struct fields from parsed input.

```rust
#[derive(Bindings)]
struct MyApp {
    #[hotkey("ctrl+c", action = on_copy)]
    copy: Handle,

    #[hotkey("ctrl+shift+v", action = on_paste)]
    paste: Handle,
}

fn on_copy() { println!("copied"); }
fn on_paste() { println!("pasted"); }

let app = MyApp::register(&manager)?;
// app.copy and app.paste are live Handle values
// drop(app) → both handles dropped → both bindings unregistered
```

Action callbacks referenced by function path (like serde's
`serialize_with`).

The generated `register()` method uses `?` on each registration.
If a later registration fails, earlier handles drop and unregister —
partial registration rollback via RAII, for free.

### Composition via flatten

The strongest argument for the derive. Mirrors clap's
`#[command(flatten)]`:

```rust
#[derive(Bindings)]
struct EditorBindings {
    #[hotkey("ctrl+c", action = on_copy)]
    copy: Handle,

    #[hotkey("ctrl+v", action = on_paste)]
    paste: Handle,
}

#[derive(Bindings)]
struct NavigationBindings {
    #[hotkey("ctrl+g", action = on_goto)]
    goto: Handle,
}

#[derive(Bindings)]
struct MyApp {
    #[flatten]
    editor: EditorBindings,

    #[flatten]
    navigation: NavigationBindings,

    #[hotkey("ctrl+q", action = on_quit)]
    quit: Handle,
}
```

Each group is independently definable, testable, composable. The
generated `register` calls `register` recursively on nested types.

### Stateful callbacks

Fields without an `action` attribute become parameters on the
generated `register` method:

```rust
#[derive(Bindings)]
struct MyApp {
    #[hotkey("ctrl+c", action = on_copy)]
    copy: Handle,

    #[hotkey("ctrl+v")]  // no action — becomes a parameter
    paste: Handle,
}

// Generated:
// fn register(
//     manager: &HotkeyManager,
//     paste: impl Fn() + Send + Sync + 'static,
// ) -> Result<Self, Error>

let clipboard = Arc::clone(&shared_clipboard);
let app = MyApp::register(&manager, move || {
    paste_from(&clipboard.lock().unwrap());
})?;
```

This gets unwieldy with many dynamic callbacks. At that point, use the
builder. Same split as clap: derive for the common declarative case,
builder for the dynamic/stateful case.

### Scope

The derive covers **bindings only**. Layers stay builder-only — they're
already declarative and clean, produce no handles, and don't benefit
from the struct-as-state pattern. Not everything needs two ways to
do it.

### Compile-time string validation

One thing the derive can do that the builder can't: validate hotkey
strings at compile time. `#[hotkey("ctrl+z")]` fails the build if `"z"`
isn't a valid key name. The builder already gets compile-time safety
through the `Key` enum, but string-based configuration is common in
keybinding-heavy apps and catching typos at build time is valuable.

### When to build

After Phase 4, when the full action vocabulary exists (sequences,
tap-hold, emit). The derive should generate against a settled builder
API. Adds a `keybound-derive` proc-macro crate dependency (syn, quote,
proc-macro2).

---

## Implementation order summary

| Phase | Delivers | Items |
|-------|----------|-------|
| **1** | Core types + basic hotkeys (the tracer bullet) | 48 |
| **2** | Grab mode + key state | 13 |
| **3** | Layers + metadata + introspection | 27 |
| **3.5** | Workspace split (`kbd-core`, `kbd-evdev`, `kbd-portal`, `kbd-xkb`, `kbd-derive`, `keybound`) | 34 |
| **4** | Sequences, tap-hold, device filtering, portal, async, serde, aliases, XKB, provenance | 61 |
| **5** | Key remapping, transformation, lock/inhibitor, context hooks | 30 |
| **6** | Stretch: chords, mouse, full keymaps | 11+ |
| **7** | Cross-platform | 3 |

Phase 1 makes it work. Phase 2 makes it intercept. Phase 3 makes it
modal and introspectable. Phase 3.5 splits the workspace — `kbd-core`
becomes an embeddable engine any Rust app can use, backends get their
own crates, deps stay isolated. Phase 4 makes it feature-complete
and layout-aware. Phase 5 makes it a transformation engine that's
context-aware.

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
