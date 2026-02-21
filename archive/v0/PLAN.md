# keybound: Roadmap

## Vision

The universal global hotkey library for Linux. Works everywhere — Wayland,
X11, TTY, sandboxed apps — by auto-negotiating the best available backend.
Provides power-user features (key sequences, modes, event grabbing) that no
other Rust crate offers.

### Why this crate should exist

The Linux hotkey ecosystem has a gap:

- `global-hotkey` (1.5M downloads) — X11 only, no Wayland
- `evdev-shortcut` — requires root/input group, no portal fallback
- `hotkey-listener` — only 15 supported keys, no Meta/Super
- XDG GlobalShortcuts portal — doesn't work on Sway/wlroots, no TTY

No crate bridges these approaches. `keybound` will be the first library
that "just works" across all Linux environments, and the only Rust crate with
key sequences, modal layers, and event interception.

---

## How to use this plan

**Phases are strictly sequential.** Each phase has a completion gate (a summary
table at the end of the phase). A phase is complete only when every `- [x]`
checklist item in every section of that phase is checked. Do not begin work on
Phase N+1 until Phase N's gate is satisfied.

**Within a phase, work on the earliest incomplete section first.** Scan from
the top (e.g., 1.1 → 1.2 → … → 1.6) and pick the first section that still has
unchecked `- [ ]` items. Complete all of that section's checklist items before
moving to the next section.

**When you finish a checklist item**, change its `- [ ]` to `- [x]` in this
file and update the completion-gate summary table counts. This makes progress
visible and prevents re-work.

---

## Phase 1: Foundation (make it worth publishing)

These items make the crate a credible alternative to existing options.

### 1.1 Unified backend: XDG portal + evdev with automatic fallback

**Status: Complete** · **Priority: Critical — this is the moat**

Try the XDG GlobalShortcuts portal first (no root needed where available).
Fall back to evdev when the portal is unavailable or unsupported (common on
Sway/wlroots, TTY, headless, and some compositor/portal combinations).

| Environment          | Preferred backend | Notes |
|----------------------|-------------------|-------|
| KDE Plasma (Wayland) | Portal            | Use portal when GlobalShortcuts is implemented by the session. |
| GNOME (Wayland)      | Portal            | Must gracefully fall back if the interface is missing/disabled. |
| Hyprland             | Portal            | Depends on portal backend support; fall back automatically otherwise. |
| Sway / wlroots       | evdev             | Usually no GlobalShortcuts support today. |
| X11                  | evdev             | Portal often unavailable for this use case. |
| TTY / headless       | evdev             | No desktop portal expected. |
| Flatpak / sandboxed  | Portal            | Primary path when host exposes the portal. |

Architecture:

```
pub trait HotkeyBackend: Send + Sync {
    fn register(&self, hotkey: Hotkey, callback: Callback) -> Result<Id>;
    fn unregister(&self, id: Id) -> Result<()>;
    fn supports_grab(&self) -> bool;
    fn supports_sequences(&self) -> bool;
    // ...
}

struct PortalBackend { /* ashpd / zbus */ }
struct EvdevBackend  { /* current implementation */ }
```

The public `HotkeyManager::new()` probes for the portal via D-Bus (when the
`portal` feature is enabled). If the compositor responds and supports
GlobalShortcuts, use `PortalBackend`. Otherwise, fall back to `EvdevBackend`.
The caller never needs to know which backend is active.

Important initialization rule: **backend probing must happen before evdev
permission checks**. If portal is selected, manager creation must not fail due
to missing `input` group membership.

Permission checks should validate actual device access capability (or attempt to
open candidate event devices) rather than relying only on group-name heuristics
so environments with ACLs/capabilities behave correctly.

Users who need a specific backend can opt in:

```rust
HotkeyManager::with_backend(Backend::Evdev)?;
HotkeyManager::with_backend(Backend::Portal)?;
```

Dependencies: `ashpd` (XDG portal bindings), `zbus` (D-Bus). These should be
behind a `portal` feature flag so pure-evdev users don't pay the cost.

Backend selection must respect compile-time availability:
- if both `portal` and `evdev` are compiled, use runtime probing/fallback
- if only one backend is compiled, use it directly
- if requested backend is not compiled in, return a clear feature-disabled error
- do not silently fall back when the caller explicitly requests a backend;
  return the backend-specific initialization error instead

Success criteria checklist (Phase 1.1 is complete only when all are checked):
- [x] The portal backend is a real implementation (not a stub) that can register and unregister hotkeys through XDG GlobalShortcuts.
- [x] Default initialization prefers portal when available and functional, otherwise falls back to evdev automatically.
- [x] Portal probing occurs before any evdev permission/device checks, so portal-capable environments do not fail due to `/dev/input` access constraints.
- [x] Explicitly requesting a specific backend never silently falls back to the other; it returns the backend-specific error.
- [x] Compile-time feature gating is verified for all combinations (`evdev` only, `portal` only, both).
- [x] Integration tests cover runtime selection/fallback paths and feature-gated error cases.

Progress update (implemented so far):
- Added backend abstraction with `Backend` selection and explicit `with_backend(...)` API.
- Added clear `BackendUnavailable(...)` errors for non-compiled backend requests.
- Improved evdev listener reliability with deterministic modifier canonicalization,
  startup failure surfacing, and callback panic containment.
- Added focused regression tests for backend resolution, modifier normalization,
  and listener callback panic handling.
- Added compile-time backend feature gating (`evdev`, `portal`) and strict backend
  request behavior (`BackendUnavailable` when portal is not compiled in).
- Added backend-specific initialization errors (`BackendInit(...)`) so explicit portal
  requests fail clearly when portal initialization fails.
- Added D-Bus portal probing logic and regression tests to verify portal owner/interface
  checks before backend selection.
- Added `HotkeyManager::new()` fallback behavior so automatic backend selection falls back
  to evdev if portal initialization fails, while explicit backend requests remain strict.
- Implemented portal registration synchronization using XDG GlobalShortcuts via `ashpd`.
- Added integration tests for backend detection fallback and feature-gated backend request errors.

### 1.2 Release / hold events

**Status: Complete** · **Priority: High — low effort, high value**

The evdev layer already delivers key release (value=0) and repeat (value=2)
events. The current code ignores them. Expose them to enable:

- **Press + release callbacks**: push-to-talk, hold-to-activate
- **Hold duration**: fire only after key held for N ms
- **Repeat awareness**: optionally fire on autorepeat, or suppress it

API sketch:

```rust
manager.register(
    KeyCode::KEY_F1,
    &[Mod::Ctrl],
    HotkeyOptions::new()
        .on_press(|| println!("pressed"))
        .on_release(|| println!("released"))
        .min_hold(Duration::from_millis(500)),  // only fire after 500ms hold
)?;
```

Success criteria checklist:
- [x] Separate callbacks can be registered for key press and key release events.
- [x] A hotkey can require a minimum hold duration before the press callback fires.
- [x] Autorepeat events can be optionally forwarded to or suppressed from callbacks.
- [x] Tests cover press, release, hold duration, and repeat scenarios without hardware.

### 1.3 String parsing for hotkey definitions

**Status: Complete** · **Priority: High — table stakes**

Both competitors have this. Users need it for config files, CLI tools, and
anywhere hotkeys are defined by end users rather than hardcoded.

```rust
let hotkey = "Ctrl+Shift+A".parse::<Hotkey>()?;
let hotkey = "Super+Return".parse::<Hotkey>()?;
let sequence = "Ctrl+K, Ctrl+C".parse::<HotkeySequence>()?;
```

Case-insensitive, supports common aliases (`Super`/`Meta`/`Win`,
`Ctrl`/`Control`, `Return`/`Enter`). Round-trips via `Display`.

Success criteria checklist:
- [x] Hotkeys can be parsed from human-readable strings (e.g., `"Ctrl+Shift+A"`).
- [x] Parsing is case-insensitive and supports common aliases (Super/Meta/Win, Ctrl/Control, Return/Enter).
- [x] Parsed hotkeys round-trip through display (parse → format → parse produces the same result).
- [x] Multi-step sequences can be parsed from comma-separated strings (e.g., `"Ctrl+K, Ctrl+C"`).
- [x] Parser covers the full range of typical keys: F-keys (F1–F24), arrow keys, Delete, Backspace, Insert, Home/End/PageUp/PageDown, numpad, and common punctuation/symbols.

### 1.4 Conflict detection

**Status: Complete** · **Priority: High — correctness**

The current code silently overwrites duplicate registrations. This is a bug
magnet. Instead:

```rust
manager.register(...)  // Ok(Handle)
manager.register(...)  // Err(Error::AlreadyRegistered { key, modifiers })
```

Add `Error::AlreadyRegistered` variant. Provide `manager.is_registered()` for
checking before registering. Provide `manager.replace()` for intentional
overwrites.

Conflict checks must use the same modifier-normalization rules as runtime
matching. In particular, left/right variants (Ctrl/Alt/Shift/Meta) should be
canonicalized so semantically equivalent registrations cannot coexist and cause
non-deterministic callback selection.

Success criteria checklist:
- [x] Registering a hotkey that is already bound returns an error (not a silent overwrite).
- [x] Callers can query whether a given key+modifier combo is already registered.
- [x] An intentional replacement path exists for callers who want to overwrite an existing binding.
- [x] Conflict detection uses the same modifier canonicalization as runtime matching (left/right variants are equivalent).
- [x] Tests cover: duplicate registration error, query, intentional replacement, and left/right modifier equivalence.

### 1.5 Device hotplug

**Status: Complete** · **Priority: High — reliability**

Keyboards get unplugged, Bluetooth devices reconnect. The listener should
handle this without restarting.

Use `inotify` to watch `/dev/input` for new `event*` files. When a new device
appears, probe it for keyboard capabilities and add it to the listener. When a
device disappears (fd returns errors), remove it from the poll set.

Hotplug handling must also repair key/modifier state on disconnect. If a
device vanishes while keys are held, synthesize release/state cleanup so
modifiers do not get stuck active.

Success criteria checklist:
- [x] Keyboards connected after the listener starts are automatically detected and begin delivering hotkey events.
- [x] Keyboards disconnected at runtime are removed gracefully without crashing the listener.
- [x] Modifier state is cleaned up on device disconnect (no stuck modifiers from a vanished keyboard).
- [x] Tests cover: device addition, device removal, and modifier-state cleanup on disconnect.

### 1.6 Event loop architecture (latency + CPU correctness)

**Status: Complete** · **Priority: High — production behavior**

> Current state: Basic sleep-based polling loop (5ms interval) with non-blocking
> reads. Needs migration to poll/epoll-driven model.

The current polling-style listener design is simple but can waste CPU and add
latency jitter under load. Move to a poll/epoll-driven wait model for device
FDs and hotplug notifications so callbacks fire promptly without busy waiting.

Requirements:
- No fixed sleep-based busy loop in steady state
- Bounded shutdown latency when stop is requested
- Deterministic behavior when many events arrive in bursts

Success criteria checklist:
- [x] The listener blocks efficiently waiting for input — no busy-loop or fixed-interval sleep in steady state.
- [x] Shutdown completes within a bounded, small time after being requested (not dependent on accumulated sleep cycles).
- [x] Burst events (many keys in rapid succession) are delivered to callbacks without added latency.
- [x] CPU usage at idle is near zero.
- [x] Tests verify prompt event delivery and clean shutdown timing.

### Phase 1 completion gate

**Phase 1 is complete only when every checklist item in sections 1.1–1.6 is
checked.** Do not begin any Phase 2 work until this gate is satisfied. When
picking up work, find the earliest section (1.1–1.6) with unchecked items and
complete those first.

| Section | Status |
|---------|--------|
| 1.1 Backend trait + portal/evdev fallback | Complete (6/6 checked) |
| 1.2 Release / hold events | Complete (4/4 checked) |
| 1.3 String parsing | Complete (5/5 checked) |
| 1.4 Conflict detection | Complete (5/5 checked) |
| 1.5 Device hotplug | Complete (4/4 checked) |
| 1.6 Event loop architecture | Complete (5/5 checked) |

---

## Phase 2: Power features (make it the obvious choice)

These features differentiate from every existing Rust crate.

### 2.1 Key sequences / chords

**Status: Complete** · **Priority: Critical — no Rust crate has this**

Multi-step hotkey combos with configurable timeout:

```rust
// Emacs-style: Ctrl+X followed by Ctrl+S within 1 second
manager.register_sequence(
    &["Ctrl+X", "Ctrl+S"],
    SequenceOptions::new().timeout(Duration::from_secs(1)),
    || save_file(),
)?;

// Leader key pattern: press Super, then 'f', then 'b' for "firefox browser"
manager.register_sequence(
    &["Super", "F", "B"],
    SequenceOptions::default(),
    || launch_firefox(),
)?;
```

Note: this requires explicit support for modifier-only sequence steps. If that
support is deferred, the leader-key example should use a non-modifier key.

Implementation: a state machine per registered sequence. On partial match,
start a timer. If the next step matches before timeout, advance. If timeout
expires or wrong key, reset. The `timeout_key` option (from xremap) lets you
specify what to emit when a partial sequence times out.

If `timeout_key` is supported, document backend behavior explicitly: evdev can
emit via uinput, while portal-only sessions should either reject this option or
degrade predictably with a clear error.

Edge cases to handle:
- Overlapping prefixes (`Ctrl+K` is both a standalone hotkey and the first
  step of `Ctrl+K, Ctrl+C`) — standalone fires on timeout if no second step
  (and provide an option for immediate standalone firing for low-latency binds)
- Multiple active sequences — track independently
- Sequence cancelled by pressing Escape (configurable abort key)

Success criteria checklist:
- [x] Multi-step hotkey sequences can be registered with a configurable timeout between steps.
- [x] Completing all steps of a sequence within the timeout fires the callback.
- [x] Partial sequences reset when the timeout expires (no stale state lingers).
- [x] Pressing the wrong key mid-sequence resets that sequence.
- [x] When a hotkey is both a standalone binding and the first step of a sequence, the standalone fires on timeout if no second step arrives.
- [x] A configurable abort key (default: Escape) cancels any active sequence.
- [x] Multiple in-progress sequences are tracked independently (one doesn't interfere with another).
- [x] On sequence timeout, an optional fallback key can be emitted (evdev only; portal returns a clear error since it cannot emit synthetic events).
- [x] Tests cover: complete sequence, timeout, wrong key, overlapping prefixes, abort key, and concurrent sequences.

### 2.2 Event grabbing / interception

**Status: Complete** · **Priority: High — essential for real hotkey daemons**

Use `EVIOCGRAB` (evdev's `device.grab()`) to exclusively capture keyboard
input. When grabbed, events don't reach other applications. Re-emit
non-hotkey events through a virtual `uinput` device.

```rust
let manager = HotkeyManager::builder()
    .grab(true)  // exclusive capture
    .build()?;

// This hotkey is consumed — the compositor never sees Super+L
manager.register(KeyCode::KEY_L, &[Mod::Super], || lock_screen())?;

// Passthrough: trigger callback AND let the key reach applications
manager.register_with_options(
    KeyCode::KEY_A, &[Mod::Ctrl],
    HotkeyOptions::new().passthrough(true),
    || log_shortcut(),
)?;
```

This is what makes keyd and xremap powerful. The evdev crate already exposes
`device.grab()`. The hard part is the uinput re-emission of non-matched
events — the `evdev` crate's `UinputDevice` or the `uinput` crate can handle
this.

Only available with the evdev backend. The portal backend should return a
clear `UnsupportedFeature`-style error when grab/interception is requested.

Success criteria checklist:
- [x] Exclusive keyboard capture prevents matched hotkey events from reaching other applications (compositor, other programs).
- [x] Non-hotkey events are re-emitted through a virtual device so normal typing is unaffected.
- [x] Individual hotkeys can be marked as passthrough: callback fires AND the event still reaches applications.
- [x] The portal backend returns a clear unsupported-feature error when grab is requested.
- [x] Grab is behind a feature flag; requesting it when the feature is not compiled in returns a clear error.
- [x] Tests cover: grabbed hotkey consumed, passthrough forwarding, portal rejection, and feature-disabled error.

### 2.3 Modes / layers

**Status: Complete** · **Priority: High — no Rust crate has this**

Named groups of hotkeys that can be pushed/popped like a stack. Inspired by
swhkd's mode system and QMK firmware layers.

```rust
// Normal mode (always active at the bottom of the stack)
manager.register(KeyCode::KEY_R, &[Mod::Super], Mode::push("resize"))?;
manager.register(KeyCode::KEY_N, &[Mod::Super], Mode::push("launch"))?;

// Resize mode — h/j/k/l without modifiers control window size
manager.mode("resize", |m| {
    m.register(KeyCode::KEY_H, &[], || shrink_left())?;
    m.register(KeyCode::KEY_J, &[], || grow_down())?;
    m.register(KeyCode::KEY_K, &[], || shrink_up())?;
    m.register(KeyCode::KEY_L, &[], || grow_right())?;
    m.register(KeyCode::KEY_ESC, &[], Mode::pop())?;  // return to normal
    Ok(())
})?;

// Launch mode — oneshot (auto-pops after one keypress)
manager.mode_with_options("launch", ModeOptions::oneshot(), |m| {
    m.register(KeyCode::KEY_F, &[], || launch("firefox"))?;
    m.register(KeyCode::KEY_T, &[], || launch("terminal"))?;
    m.register(KeyCode::KEY_E, &[], || launch("editor"))?;
    Ok(())
})?;
```

Mode options (from swhkd):
- `oneshot` — auto-pop after one keypress fires
- `swallow` — suppress all non-matching events while in this mode
- `timeout` — auto-pop after N seconds of inactivity

Implementation: maintain a stack of layers. Each layer stores exact hotkey
bindings, and lookup checks from top to bottom. This avoids `HashMap` key
collisions and supports same key combos in different modes by design. Mode
transitions are push/pop operations on that stack.

Success criteria checklist:
- [x] Named groups of hotkeys (modes) can be defined and activated/deactivated at runtime.
- [x] Modes behave as a stack: the most recently activated mode's bindings take priority.
- [x] The same key combo can exist in different modes without conflict.
- [x] A oneshot option auto-deactivates the mode after one hotkey fires.
- [x] A swallow option suppresses all non-matching key events while the mode is active.
- [x] A timeout option auto-deactivates the mode after a period of inactivity.
- [x] Tests cover: activation/deactivation, oneshot, swallow, timeout, stack ordering, and same-key-different-mode.

### 2.4 Device-specific hotkeys

**Status: Complete** · **Priority: Medium — natural fit for evdev**

Different hotkeys for different keyboards. The evdev backend already has
per-device file descriptors — just need to associate registrations with device
filters.

```rust
// Only trigger on a specific device (e.g., a macro pad)
manager.register_with_options(
    KeyCode::KEY_1, &[],
    HotkeyOptions::new().device(DeviceFilter::name_contains("StreamDeck")),
    || scene_1(),
)?;

// Filter by vendor/product ID
manager.register_with_options(
    KeyCode::KEY_F1, &[],
    HotkeyOptions::new().device(DeviceFilter::usb(0x1234, 0x5678)),
    || custom_action(),
)?;
```

Use cases: macro pads, secondary keyboards, foot pedals. The listener loop
already iterates per-device — just need to tag events with their source device
and filter registrations accordingly.

Device-scoped matching also requires device-scoped key state. Modifier tracking
must be maintained per device (with a safe aggregate view for global binds) so
a modifier held on keyboard A does not accidentally satisfy a hotkey bound to
keyboard B.

Success criteria checklist:
- [x] Hotkeys can be restricted to specific input devices (by name pattern, vendor/product ID, or similar filter).
- [x] A hotkey bound to device A does not fire from events on device B.
- [x] Modifier state is tracked per device: a modifier held on one keyboard does not satisfy a device-specific hotkey bound to a different keyboard.
- [x] Global (unfiltered) hotkeys still use aggregate modifier state across all devices.
- [x] Tests cover: device-specific match, device-specific miss, per-device modifier isolation, and global aggregate behavior.

### 2.5 Tap vs. hold (dual-function keys)

**Status: Complete** · **Priority: Medium — popular in keyboard community**

A key does one thing when tapped, another when held. This is keyd's
`overload()` and QMK's `LT()`/`MT()`.

```rust
manager.register_tap_hold(
    KeyCode::KEY_CAPSLOCK,
    TapAction::emit(KeyCode::KEY_ESC),          // tap: Escape
    HoldAction::modifier(KeyCode::KEY_LEFTCTRL), // hold: Ctrl
    TapHoldOptions::new().threshold(Duration::from_millis(200)),
)?;
```

Requires event grabbing (Phase 2.2) since we need to intercept the key and
decide whether to re-emit it as the tap action or apply it as a modifier.
This feature depends on synthetic event emission in the evdev backend (via
uinput) and should return a clear unsupported-feature error on backends that
cannot safely emulate tap/hold behavior. The timing heuristic follows keyd's
model: resolve as "hold" if another key is pressed while the key is down, or
if held past the threshold duration.

Success criteria checklist:
- [x] A key can be configured to perform one action on tap and a different action when held.
- [x] Tap resolves when the key is released before the threshold duration.
- [x] Hold resolves when the key is held past the threshold duration.
- [x] Hold resolves early if another key is pressed while the dual-function key is down (keyd model).
- [x] Tap/hold requires event grabbing; requesting it without grab support returns a clear error.
- [x] The tap action produces the expected key event visible to other applications (synthetic emission).
- [x] Tests cover: tap, hold by duration, hold by interrupting keypress, and missing-grab error.

### Phase 2 completion gate

**Phase 2 is complete only when every checklist item in sections 2.1–2.5 is
checked.** Do not begin any Phase 3 work until this gate is satisfied. When
picking up work, find the earliest section (2.1–2.5) with unchecked items and
complete those first.

| Section | Status |
|---------|--------|
| 2.1 Key sequences / chords | Complete (9/9 checked) |
| 2.2 Event grabbing | Complete (6/6 checked) |
| 2.3 Modes / layers | Complete (7/7 checked) |
| 2.4 Device-specific hotkeys | Complete (5/5 checked) |
| 2.5 Tap vs. hold | Complete (7/7 checked) |

---

## Phase 3: Polish (make it production-ready)

### 3.1 Async API

**Status: Complete**

Provide an `async` interface alongside the callback API. Feature-gated behind
`tokio` and `async-std` features.

```rust
let mut stream = manager.event_stream();
while let Some(event) = stream.next().await {
    match event {
        HotkeyEvent::Pressed(id) => { /* ... */ }
        HotkeyEvent::Released(id) => { /* ... */ }
        HotkeyEvent::SequenceStep { id, step, total } => { /* ... */ }
        HotkeyEvent::ModeChanged(name) => { /* ... */ }
    }
}
```

Success criteria checklist:
- [x] An async stream-based interface is available as an alternative to callbacks for receiving hotkey events.
- [x] The stream delivers press, release, sequence-step, and mode-change events.
- [x] The async interface is behind a feature flag so non-async users pay no dependency cost.
- [x] The async and callback interfaces can coexist (enabling one does not disable the other).
- [x] The stream completes cleanly when the manager shuts down.
- [x] Tests cover: event delivery, clean completion on shutdown, and feature-gated compilation.

### 3.2 Debouncing / rate limiting

**Status: Complete**

Prevent rapid-fire callback invocations:

```rust
HotkeyOptions::new()
    .debounce(Duration::from_millis(100))   // ignore triggers within 100ms
    .max_rate(Duration::from_millis(500))   // at most once per 500ms
```

Success criteria checklist:
- [x] A per-hotkey debounce option suppresses repeated triggers within a configurable time window.
- [x] A per-hotkey rate-limit option caps callback invocations to at most once per configurable interval.
- [x] Debounce and rate-limit can be combined on the same hotkey.
- [x] Tests cover: debounce suppression, rate limiting, and combined behavior.

### 3.3 Key state query API

**Status: Complete**

> Current state: Modifier state is tracked internally in the listener thread
> (`active_modifiers: HashSet<KeyCode>`), but not exposed via any public API.

Expose the internal modifier tracking as a public API:

```rust
manager.is_key_pressed(KeyCode::KEY_LEFTCTRL)  // -> bool
manager.active_modifiers()                       // -> HashSet<KeyCode>
```

Success criteria checklist:
- [x] Callers can query whether a specific key is currently pressed.
- [x] Callers can retrieve the set of currently active modifiers.
- [x] Queried state is consistent with the listener's internal tracking.
- [x] Queries are thread-safe and do not block the listener.
- [x] Tests cover: query while key is held, query after release, and concurrent access from multiple threads.

### 3.4 Configuration serialization

**Status: Complete**

Support loading hotkey definitions from structured data (serde):

```rust
#[derive(Deserialize)]
struct HotkeyConfig {
    hotkeys: Vec<HotkeyDef>,
    modes: HashMap<String, Vec<HotkeyDef>>,
    sequences: Vec<SequenceDef>,
}
```

This lets applications load hotkey configs from TOML/JSON/YAML files without
writing parsing code.

Success criteria checklist:
- [x] Hotkey definitions (single hotkeys, sequences, and mode bindings) can be deserialized from structured data (TOML, JSON, YAML, etc.).
- [x] Deserialized definitions can be registered directly without manual conversion.
- [x] Serialization is behind a feature flag so non-config users pay no dependency cost.
- [x] Invalid configuration data produces clear, actionable error messages.
- [x] Tests cover: deserialization, round-trip serialization, and invalid config errors.

### Phase 3 completion gate

**Phase 3 is complete only when every checklist item in sections 3.1–3.4 is
checked.** When picking up work, find the earliest section (3.1–3.4) with
unchecked items and complete those first.

| Section | Status |
|---------|--------|
| 3.1 Async API | Complete (6/6 checked) |
| 3.2 Debouncing / rate limiting | Complete (4/4 checked) |
| 3.3 Key state query | Complete (5/5 checked) |
| 3.4 Configuration serialization | Complete (5/5 checked) |

---

## Cross-cutting architecture constraints

These constraints apply across phases and should guide all implementations:

- **Deterministic fallback policy**: backend selection should be predictable and
  inspectable (`manager.active_backend()` or equivalent), so debugging
  environment-specific behavior is straightforward.
- **Feature-gated behavior must be explicit**: when `portal` or `grab` features
  are disabled at compile time, API errors/messages should explain that the
  capability is not compiled in (not just "unavailable").
- **Modifier state correctness across devices**: track modifier state in a way
  that tolerates multiple keyboards and disconnects without producing sticky
  modifiers or phantom releases.

---

## Feature flags

```toml
[features]
default = ["evdev"]
evdev = ["dep:evdev", "dep:libc"]
portal = ["dep:ashpd", "dep:zbus"]
grab = ["evdev", "dep:uinput"]
tokio = ["dep:tokio"]
async-std = ["dep:async-std"]
serde = ["dep:serde"]
```

Pure evdev users get a minimal dependency tree. Portal users opt in.
Grab mode pulls in uinput. Async is optional.

Define expected behavior for feature combinations in docs/tests (e.g.
`--no-default-features --features portal`, `--features evdev,portal`, and
invalid requests like grab without evdev) so compile-time/runtime behavior is
unambiguous.

Decide and document a release policy for default features (`evdev`-minimal vs
`full`) so README examples and user expectations align with what is enabled
out-of-the-box.

---

## Non-goals (for now)

Things this crate will NOT do in Phases 1–3:

- **Text expansion / hotstrings**: Out of scope. Different problem domain.
- **GUI / system tray**: This is a library, not a daemon.

Note: Key remapping and synthetic event emission were originally non-goals.
Phase 4 promotes them to first-class features, building on the grab/uinput
infrastructure from Phase 2.

### Cross-platform: door is open

The initial focus is Linux — that's where the gap is and where the backend
trait architecture (Phase 1.1) gets battle-tested. But that same trait design
means platform backends can be added without touching existing code:

| Platform | Potential backend          | Feature flag |
|----------|---------------------------|--------------|
| macOS    | CGEventTap / IOKit        | `macos`      |
| Windows  | Low-level keyboard hooks  | `windows`    |

If/when this happens, new platform backends plug into the existing
`HotkeyBackend` trait without touching Linux code. The crate was renamed
from `evdev-hotkey` to `keybound` in Phase 5.3 to reflect this
platform-neutral aspiration.

macOS and Windows backends are **not committed scope** — they are
architectural options that the backend trait preserves for free. Ship
Linux-first, prove the API, expand later.

---

## Phase 4: Key Remapping & Event Transformation (keyd-class capabilities)

Phase 4 evolves keybound from a hotkey *detection* library into a hotkey
*transformation* engine. The core insight: the grab + uinput infrastructure
from Phase 2 already intercepts every keystroke and re-emits non-matches.
Phase 4 adds the ability to emit *different* events than what was pressed —
the fundamental primitive that powers keyd, xremap, and QMK firmware.

Sections 4.1–4.7 are the core of this phase — they make keybound a genuinely
powerful input transformation library for app developers, tool builders, and
tiling WM accessories. Sections 4.8–4.9 are stretch goals that extend into
territory more relevant to full remapping daemons (mouse events, layout
switching). They're included because they have a natural home here and the
door should stay open, but the library is compelling without them. If demand
materializes — a downstream keyd-rs project, user requests, contributor
PRs — they're ready to be built. Until then, someone building a remapping
daemon can drop to raw evdev for those edges without keybound failing them.

Reference implementation: keyd (cloned to `reference/keyd/`).

### 4.1 Codebase cleanup

**Status: Not Started** · **Priority: Critical — pay down debt before building on it**

The library works, but Phases 1–3 accumulated significant structural debt.
Before building remapping and transformation features on top, clean up the
foundation. This is a refactoring-only section — no new public API, no new
features, all existing tests must continue to pass.

Tracked via `// SMELL:` comments throughout the codebase. Remove each comment
as the issue is resolved.

#### 4.1a Module organization and file structure

**`listener.rs` decomposition**: The `listener_loop` function is 357 lines
with a `#[allow(clippy::too_many_lines)]` suppression. It orchestrates device
polling, hotplug detection, modifier tracking, sequence processing, mode
handling, tap-hold logic, and key forwarding — all in one function. Extract
coherent phases into named functions that the loop calls.

**`listener.rs` test migration**: When the listener was split into submodules
(`dispatch.rs`, `io.rs`, `sequence.rs`, `hotplug.rs`, `forwarding.rs`,
`state.rs`), the tests all stayed in `listener.rs`. Move each test to the
module it actually exercises:
- Sequence tests → `sequence.rs`
- Device/modifier tests → appropriate submodule
- Hotplug parsing tests → `hotplug.rs`
- IO tests → `io.rs`

**`listener/state.rs`**: Too thin to justify its own module — just a struct
and constants. Absorb into a more relevant module or consolidate with related
state.

**`listener/forwarding.rs` feature gating**: `#[cfg(feature = "grab")]` is
scattered across every item in the file. Gate the module once at the
`mod forwarding` declaration in the parent instead.

**`manager.rs` splitting**: Over 3,000 lines mixing high-level public API
with internal implementation details. Split into focused submodules.

**`mode.rs` stray code**: Test utilities sitting in the main module file
(flagged: "what's this doing here?"). Move to test modules.

#### 4.1b Type design and idiomatic Rust

**`Key` / `Modifier` duplication**: These types have near-identical
`Display` impls, `as_str()` methods, and evdev conversion patterns. Options:
- Shared trait for common behavior
- Macro to generate the boilerplate
- Higher-level enum that contains both

**`from_evdev` / `to_evdev` → `From` / `Into` traits**: Both `Key` and
`Modifier` use ad-hoc methods for evdev conversion (flagged: "why not a impl
from?", "why not a impl to?"). Use standard `From<KeyCode>` /
`Into<KeyCode>` implementations.

**Parsing consolidation**: `parse_key()` and `parse_modifier()` live in
`hotkey.rs` but duplicate knowledge that belongs in `key.rs` (flagged:
"doesn't this overlap with or belong in key.rs?"). Move parsing into the
types themselves — `FromStr` implementations on `Key` and `Modifier`, called
by `Hotkey::from_str`.

**Hardcoded modifier mappings**: `key_state.rs` has a `MODIFIER_KEYS` array
(flagged: "why this? why here?") that duplicates what `Modifier::from_evdev()`
already knows. Single source of truth in `key.rs`, derive everything else.

**Bool fields → enums**: Multiple structs use bool fields for state that has
semantic meaning:
- `DeviceSpecificDispatch.matched` / `.passthrough` → dispatch result enum
- `NonModifierDispatch.matched_hotkey` / `.passthrough` → same (also
  duplicates the above struct, flagged)
- `SequenceDispatch.suppress_current_key_press` → suppression enum
- `PendingStandalone.press_dispatched` → dispatch state enum
- `EventHub.running` → lifecycle state enum
- `BackendCapabilities` bool fields → could be a bitflag or capability enum

**Stringly-typed patterns**: `DeviceFilter::Name(String)` and hotplug event
handling use raw strings where newtypes or validated types would be safer
(flagged: "Stringly typed here").

#### 4.1c Error handling

**Error type consolidation**: `ParseHotkeyError` in `hotkey.rs` and
`Error::InvalidHotkey` in `error.rs` handle overlapping domains. Manual
`std::error::Error` impl (flagged: "thiserror?"). Evaluate adopting
`thiserror` for all error types and unifying the error hierarchy.

#### 4.1d YAGNI removal

**`KeyEventForwarder` trait**: A trait with exactly one implementation
(flagged: "trait for one impl? is this future proofing or YAGNI"). Replace
with the concrete `UinputForwarder` type. If a second forwarder is ever
needed, introduce the trait then.

**`invoke_callback` / `catch_unwind`**: The panic-catching wrapper is
defensive but the current placement in `dispatch.rs` is confusing (flagged:
"what is this, why is this here?"). Keep the behavior but document the
rationale clearly and consider whether it belongs closer to the callback
invocation site.

**`io.rs` bare function call**: Unexplained call flagged as "what's going on
here, is this mutating state?" — clarify or restructure.

#### 4.1e Config system reframe

The current `config.rs` is an over-engineered application-level config system
(`ActionId`, `ActionMap`, transactional registration with rollback, validation
with location tracking). For a library crate, this is the wrong abstraction —
library consumers will have their own config systems.

Reframe to focus on what actually helps:
- **Keep**: serde `Serialize`/`Deserialize` on `Hotkey`, `HotkeySequence`,
  `Key`, `Modifier`, and options types — lets consumers put these in their
  own config structs
- **Keep**: `FromStr` / `Display` round-tripping — human-readable config
- **Remove or deprecate**: `ActionMap`, `ActionId`, `HotkeyConfig`,
  `RegisteredConfig` — these impose opinions that belong in applications
- **Consider**: a simple example showing how to wire serde types to
  registration, without the library owning the pattern

#### 4.1f Missing tests

Several modules flagged as having no tests:
- `hotkey.rs` (flagged: "no tests?") — parsing, display, round-trip
- `events.rs` (flagged: "no tests?") — event stream behavior

Add tests for these modules. Not exhaustive coverage — focus on the parsing
and public-facing behavior that the rest of the library depends on.

Success criteria checklist:
- [ ] `listener_loop` is decomposed into named functions — no single function exceeds ~80 lines.
- [ ] All tests in `listener.rs` are moved to the submodule they exercise.
- [ ] `listener/state.rs` is absorbed or justified.
- [ ] `listener/forwarding.rs` feature gating is at the module declaration, not scattered.
- [ ] `manager.rs` is split into focused submodules.
- [ ] `Key` and `Modifier` share conversion logic — no near-identical code blocks.
- [ ] Evdev conversions use `From`/`Into` traits.
- [ ] Key/modifier parsing lives on the types (`FromStr`), not in `hotkey.rs`.
- [ ] Modifier key mappings have a single source of truth.
- [ ] Bool fields in dispatch/sequence/event types are replaced with enums.
- [ ] Stringly-typed device identifiers use newtypes.
- [ ] Error types are consolidated (evaluate `thiserror`).
- [ ] `KeyEventForwarder` trait is removed in favor of concrete type.
- [ ] Config system is reframed: serde on primitives stays, `ActionMap`/`HotkeyConfig` removed or deprecated.
- [ ] `hotkey.rs` and `events.rs` have tests.
- [ ] All `// SMELL:` comments are resolved and removed.
- [ ] All existing tests pass. No public API changes (internal restructuring only).

### 4.2 First-class output actions

**Status: Not Started** · **Priority: Critical — foundation for everything else**

Today, hotkey registrations bind to callbacks (`Box<dyn Fn() + Send>`).
Tap-hold binds to `TapAction`/`HoldAction`. Mode transitions use
`ModeController`. These are all separate, ad-hoc mechanisms. Unify them into
a composable `Action` type that can be used everywhere — hotkeys, remaps,
tap-hold, mode entries, sequences.

```rust
pub enum Action {
    /// Emit a key event (with optional modifiers) through uinput
    EmitKey(Key, Vec<Modifier>),
    /// Emit a sequence of key events with configurable inter-key delay
    EmitSequence(Vec<(Key, Vec<Modifier>)>, Option<Duration>),
    /// Activate a named mode/layer
    ActivateMode(String),
    /// Deactivate the current mode (pop stack)
    DeactivateMode,
    /// Toggle a named mode on/off
    ToggleMode(String),
    /// Run a callback
    Callback(Box<dyn Fn() + Send + Sync>),
    /// Run a shell command
    Command(String),
    /// Do nothing (useful as a placeholder or to explicitly swallow a key)
    None,
}
```

This makes all existing features expressible through a single type:

```rust
// Tap-hold with action types instead of separate TapAction/HoldAction
manager.register_tap_hold(
    Key::CapsLock,
    Action::EmitKey(Key::Escape, vec![]),           // tap
    Action::ActivateMode("nav".into()),              // hold
    TapHoldOptions::new().threshold(Duration::from_millis(200)),
)?;

// Mode hotkey that emits a key instead of running a callback
manager.define_mode("nav", ModeOptions::new(), |mode| {
    mode.register_action(Key::H, &[], Action::EmitKey(Key::Left, vec![]))?;
    mode.register_action(Key::J, &[], Action::EmitKey(Key::Down, vec![]))?;
    mode.register_action(Key::Escape, &[], Action::DeactivateMode)?;
    Ok(())
})?;
```

The `Action` type should be serializable (behind `serde` feature) so that
config files can express full remapping behavior, not just callback IDs.

Existing callback-based APIs remain unchanged — `Action::Callback` wraps
them. This is additive, not breaking.

Success criteria checklist:
- [ ] An `Action` enum exists that represents all possible output behaviors (emit key, emit sequence, mode control, callback, command, none).
- [ ] `Action::EmitKey` produces a key event visible to other applications via uinput.
- [ ] `Action::EmitSequence` produces multiple key events with configurable inter-key timing.
- [ ] `Action::Command` executes a shell command asynchronously (non-blocking to the listener).
- [ ] `Action` can be used in hotkey registration, tap-hold, mode bindings, and sequence callbacks.
- [ ] Existing callback-based APIs continue to work unchanged (backwards compatible).
- [ ] `Action` variants (except `Callback`) are serializable/deserializable behind the `serde` feature.
- [ ] Tests cover: each action variant, action in tap-hold context, action in mode context, and serde round-trip.

### 4.3 Key event caching across layer transitions

**Status: Not Started** · **Priority: Critical — correctness for existing modes**

This is a correctness fix for the existing mode system. Today, if a mode pops
while a key is held, the release event is processed in the wrong context. keyd
solves this with a `cache_entry` system that records the descriptor (action)
used at press time and replays it for the corresponding release.

```
Press CapsLock → activates "nav" mode, hold resolves
Press H → "nav" mode maps H to Left, emit Left press
Pop "nav" mode (e.g., CapsLock released)
Release H → should emit Left release, NOT H release
```

Without caching, releasing H after the mode pops would either:
- Emit an H release (wrong — Left is what was pressed)
- Match nothing (the nav-mode binding is gone) and forward H release

Implementation: maintain a `HashMap<Key, CachedAction>` that records what
action was taken on each key press. On key release, look up the cache instead
of re-running the dispatch pipeline. Clear the cache entry after the release
is processed.

This also matters for remapping (4.4) — if a remap is removed while a
remapped key is held, the release must still emit the remapped key's release.

Success criteria checklist:
- [ ] When a key is pressed, the action taken is cached for that key.
- [ ] When the same key is released, the cached action is used (not the current dispatch result).
- [ ] Cache entries are cleared after the release is processed.
- [ ] Mode transitions mid-keypress produce correct release events (release matches the press action, not the post-transition action).
- [ ] Remap removal mid-keypress produces correct release events.
- [ ] Tests cover: mode pop during keypress, remap removal during keypress, and cache cleanup.

### 4.4 Key remapping (input → different output)

**Status: Not Started** · **Priority: Critical — the headline feature**

The single most impactful addition. Today, matched hotkeys either fire a
callback and get swallowed, or fire a callback and pass through unchanged.
There is no path to "press A, emit B." This adds that path.

```rust
// Simple remap: CapsLock becomes Escape
manager.remap(Key::CapsLock, &[], Key::Escape, &[])?;

// Remap with modifier transformation: Ctrl+A becomes Home
manager.remap(Key::A, &[Modifier::Ctrl], Key::Home, &[])?;

// Remap adding modifiers: pressing ` emits Ctrl+Shift+U (Unicode input)
manager.remap(Key::Grave, &[Modifier::Alt], Key::U, &[Modifier::Ctrl, Modifier::Shift])?;
```

Implementation: extend the dispatch pipeline so that matched events can carry
a remap `Action` instead of (or alongside) a callback. The `UinputForwarder`
already emits key events — it just needs to emit the *remapped* key instead
of the *original* key. Key releases use the event cache (4.3) to emit the
correct remapped release.

Remapping requires grab mode (like tap-hold). Requesting remaps without grab
returns a clear error. Portal backend returns `UnsupportedFeature`.

Success criteria checklist:
- [ ] A key (with optional modifiers) can be remapped to a different key (with optional modifiers).
- [ ] The remapped key event is visible to other applications via the virtual uinput device.
- [ ] The original key event is consumed and does not reach other applications.
- [ ] Key releases emit the release for the remapped key, not the original.
- [ ] Remapping requires grab mode; requesting it without grab returns a clear error.
- [ ] Portal backend returns a clear unsupported-feature error for remap requests.
- [ ] Remaps coexist with callback-based hotkeys (both can be registered simultaneously).
- [ ] Tests cover: simple remap, modifier-changing remap, remap + callback coexistence, release correctness, and missing-grab error.

### 4.5 Oneshot layers

**Status: Not Started** · **Priority: Medium — natural mode extension**

Distinct from the existing mode `oneshot` option (which auto-pops after one
hotkey fires within the mode). keyd's oneshot activates a layer for exactly
one *subsequent keypress* — even keys that aren't explicitly bound in the
layer. The classic use case is sticky modifiers:

```rust
// Sticky Shift: press and release CapsLock, then press 'a' → 'A'
manager.register_action(
    Key::CapsLock, &[],
    Action::Oneshot("shift_layer".into()),
)?;

manager.define_mode("shift_layer", ModeOptions::new(), |mode| {
    // This layer applies Shift to whatever key comes next
    mode.set_default_modifier(Modifier::Shift);
    Ok(())
})?;
```

The key difference from existing oneshot modes:
- Current: auto-pops after a *registered* hotkey fires (unregistered keys
  are swallowed or passed through depending on swallow setting)
- keyd-style: auto-pops after *any* keypress, applying the layer's
  transformation to that keypress

This requires the concept of a "default action" for a layer — what happens
when a key is pressed that has no explicit binding in the layer. Options:
- Apply modifier(s) to the key and pass through
- Pass through with the layer's keymap transformation
- Swallow (existing behavior)

Implementation: extend `ModeOptions` with a `default_action` that applies
to unbound keys. Track `oneshot_depth` (number of keypresses remaining) in
the mode stack entry.

Success criteria checklist:
- [ ] A oneshot layer activates and automatically deactivates after one subsequent keypress.
- [ ] The oneshot layer's transformation applies to the triggering keypress (not just registered hotkeys within the layer).
- [ ] Layers can define a default action for keys not explicitly bound (e.g., apply Shift modifier).
- [ ] Oneshot depth is configurable (deactivate after N keypresses, not just one).
- [ ] Pressing only modifier keys does not consume the oneshot (it waits for a non-modifier key).
- [ ] Tests cover: basic oneshot, sticky modifier, configurable depth, modifier-key passthrough, and auto-deactivation.

### 4.6 Overload variants

**Status: Not Started** · **Priority: Medium — tap-hold refinements**

The existing tap-hold (Phase 2.5) implements keyd's basic `overload`. keyd
has additional variants that improve the feel for fast typists:

```rust
pub enum OverloadStrategy {
    /// Current behavior: hold if held past threshold OR interrupted by another key
    Basic,
    /// Hold triggers only after a specific timeout (ignores interrupting keys)
    Timeout(Duration),
    /// Tap triggers only within timeout window; everything else is hold
    TimeoutTap(Duration),
    /// Uses idle time before keypress for disambiguation: if the user was
    /// idle for longer than the threshold before pressing, it's a hold
    IdleTimeout(Duration),
}

manager.register_tap_hold_with_strategy(
    Key::CapsLock,
    Action::EmitKey(Key::Escape, vec![]),
    Action::ActivateMode("nav".into()),
    OverloadStrategy::IdleTimeout(Duration::from_millis(150)),
)?;
```

The idle timeout variant is particularly important — it uses the time since
the *previous* keypress to disambiguate, which eliminates the awkward delay
that basic tap-hold introduces during fast typing.

Implementation: extend `TapHoldConfig` with a strategy enum. The listener
already tracks press timestamps — idle timeout just needs the timestamp of
the last key event (from any key, not just the tap-hold key).

Success criteria checklist:
- [ ] The basic overload strategy (current behavior) continues to work unchanged.
- [ ] A timeout-only strategy ignores interrupting keypresses and resolves purely on duration.
- [ ] A timeout-tap strategy treats everything outside the tap window as a hold.
- [ ] An idle-timeout strategy uses time since previous keypress for disambiguation.
- [ ] Strategy is selectable per tap-hold registration.
- [ ] Tests cover: each strategy variant, fast-typing scenarios with idle timeout, and backwards compatibility.

### 4.7 Chord support (simultaneous key combinations)

**Status: Not Started** · **Priority: Medium — new input primitive**

Chords are multiple non-modifier keys pressed *simultaneously*. Different from
hotkeys (modifier + key) and sequences (keys pressed in order). keyd example:

```rust
// Press J and K at the same time → Escape
manager.register_chord(
    &[Key::J, Key::K],
    ChordOptions::new().window(Duration::from_millis(50)),
    Action::EmitKey(Key::Escape, vec![]),
)?;

// Three-key chord
manager.register_chord(
    &[Key::J, Key::K, Key::L],
    ChordOptions::default(),
    Action::ActivateMode("special".into()),
)?;
```

Implementation: a state machine with timeout-based disambiguation. When the
first key of a potential chord is pressed, enter `Resolving` state and buffer
events. If all chord keys arrive within the time window, fire the chord
action. If the window expires or a non-chord key arrives, flush the buffered
events as normal keypresses.

keyd's chord states: `CHORD_RESOLVING` → `CHORD_PENDING_DISAMBIGUATION` →
resolved. The disambiguation phase handles the case where a chord is also a
prefix of a longer chord.

Chords require grab mode (buffered events must be re-emitted if the chord
doesn't complete). Per-layer chord definitions (chords that only exist in
certain modes) are supported by integrating with the mode dispatch pipeline.

Success criteria checklist:
- [ ] Two or more non-modifier keys pressed within a configurable time window are recognized as a chord.
- [ ] The chord action fires only when all constituent keys are pressed within the window.
- [ ] If the time window expires before all keys arrive, the buffered keys are emitted as normal keypresses.
- [ ] Chord prefixes are disambiguated correctly (J+K chord doesn't block J+K+L chord).
- [ ] Chords can be defined per-mode (only active when a specific mode is on the stack).
- [ ] Chords require grab mode; requesting them without grab returns a clear error.
- [ ] Tests cover: successful chord, timeout fallback, prefix disambiguation, per-mode chords, and missing-grab error.

### 4.8 Non-key event support (mouse, scroll)

**Status: Not Started** · **Priority: Stretch — build if demand exists**

> This is a stretch goal. The library is complete and compelling without it.
> Build it when a downstream project or user request demonstrates real need.

Extend the listener beyond `EV_KEY` to handle `EV_REL` (mouse movement,
scroll wheel) and `EV_ABS` (touchpad) events. This enables:

- Hotkeys triggered by scroll wheel or mouse buttons
- Remapping scroll events (e.g., Ctrl+Scroll → horizontal scroll)
- Mouse keys: emit mouse movement from keyboard keys in a layer
- Scroll-to-zoom or scroll direction inversion

```rust
// Bind mouse button
manager.register(Key::MouseMiddle, &[Modifier::Ctrl], || paste())?;

// Remap Ctrl+Scroll to horizontal scroll
manager.remap_scroll(
    ScrollAxis::Vertical, &[Modifier::Ctrl],
    ScrollAxis::Horizontal, &[],
)?;

// Mouse keys in a mode
manager.define_mode("mouse", ModeOptions::new(), |mode| {
    mode.register_action(Key::H, &[], Action::EmitMouseMove(-20, 0))?;
    mode.register_action(Key::J, &[], Action::EmitMouseMove(0, 20))?;
    mode.register_action(Key::K, &[], Action::EmitMouseMove(0, -20))?;
    mode.register_action(Key::L, &[], Action::EmitMouseMove(20, 0))?;
    Ok(())
})?;
```

This requires:
- Extending the listener's event filter beyond `EV_KEY`
- Creating a second virtual device for pointer events (keyd's pattern — one
  virtual keyboard, one virtual pointer)
- Extending the `Key` enum or adding a parallel `InputEvent` enum for
  non-key events
- Extending `Action` with mouse/scroll output variants

Success criteria checklist:
- [ ] Mouse button events (`BTN_LEFT`, `BTN_RIGHT`, `BTN_MIDDLE`, etc.) can trigger hotkeys.
- [ ] Scroll wheel events can trigger hotkeys or be remapped.
- [ ] Actions can emit mouse movement and scroll events through a virtual pointer device.
- [ ] A separate virtual pointer device is created (not mixed into the virtual keyboard).
- [ ] Mouse/scroll support works alongside keyboard hotkeys without interference.
- [ ] Tests cover: mouse button hotkey, scroll remap, mouse movement action, and dual virtual device isolation.

### 4.9 Full layer keymaps

**Status: Not Started** · **Priority: Stretch — build if demand exists**

> This is a stretch goal. Full layout switching is typically handled by the
> kernel, desktop environment, or a dedicated daemon. Build it when a
> downstream project demonstrates the need for library-level layout layers.

The current mode system binds specific hotkeys within a mode. keyd's layer
system defines a complete `keymap[256]` — every key can be remapped when the
layer is active. This is what enables full layout switching (QWERTY ↔ Dvorak
↔ Colemak) and comprehensive remapping layers.

```rust
// Define a full keymap layer
let mut dvorak = LayerKeymap::new();
dvorak.remap(Key::Q, Key::Apostrophe);
dvorak.remap(Key::W, Key::Comma);
dvorak.remap(Key::E, Key::Period);
dvorak.remap(Key::R, Key::P);
// ... full layout

manager.define_layer("dvorak", dvorak)?;
manager.activate_layer("dvorak")?;

// Layers compose: higher layers override lower layers per-key
manager.define_mode_with_keymap("nav", keymap, ModeOptions::new(), |mode| {
    // Explicit hotkeys override the keymap for specific combos
    mode.register(Key::Escape, &[], Action::DeactivateMode)?;
    Ok(())
})?;
```

Layer lookup precedence (from keyd, proven in production):
1. Chords (highest priority)
2. Composite layers (by constituent count — more specific wins)
3. Regular layers (by activation time — most recent wins)
4. Default keymap (passthrough)

Implementation: extend mode stack entries to carry an optional `[Option<Key>; 256]`
keymap array. During dispatch, before checking explicit hotkey registrations,
check the active layer stack for a keymap entry for the pressed key. This
integrates with the caching system (4.3) so that layer changes mid-keypress
produce correct releases.

Success criteria checklist:
- [ ] A mode/layer can define a complete keymap (remap for every key position).
- [ ] Layer keymaps compose via the mode stack (higher layers override per-key).
- [ ] Explicit hotkey registrations within a mode take precedence over the layer keymap.
- [ ] Layer lookup follows defined precedence: chords > composite > by-activation-time > default.
- [ ] Full keyboard layout switching (e.g., QWERTY to Dvorak) works via layer keymaps.
- [ ] Layer keymaps integrate with key event caching (4.3) for correct releases across transitions.
- [ ] Tests cover: basic layer keymap, layer composition, hotkey-over-keymap precedence, layout switching, and mid-transition release correctness.

### Phase 4 completion gate

**Phase 4's core (4.1–4.7) is complete when every checklist item in those
sections is checked.** Sections 4.8–4.9 are stretch goals — they do not
block Phase 5 or the phase being considered "done" for practical purposes.
When picking up work, find the earliest incomplete core section (4.1–4.7)
and complete it first.

| Section | Type | Status |
|---------|------|--------|
| 4.1 Codebase cleanup | Core | Not Started (0/17) |
| 4.2 First-class output actions | Core | Not Started (0/8) |
| 4.3 Key event caching | Core | Not Started (0/6) |
| 4.4 Key remapping | Core | Not Started (0/8) |
| 4.5 Oneshot layers | Core | Not Started (0/6) |
| 4.6 Overload variants | Core | Not Started (0/6) |
| 4.7 Chord support | Core | Not Started (0/7) |
| 4.8 Non-key event support | Stretch | Not Started (0/6) |
| 4.9 Full layer keymaps | Stretch | Not Started (0/7) |

---

## Implementation order

| Item | Description | Status | Checklist |
|------|-------------|--------|-----------|
| **Phase 1** | **Foundation** | **✅ Complete** | |
| 1.1 | Backend trait + evdev backend (refactor current code) | Complete | 6/6 ✓ |
| 1.2 | Release/hold events | Complete | 4/4 ✓ |
| 1.3 | String parsing | Complete | 5/5 ✓ |
| 1.4 | Conflict detection | Complete | 5/5 ✓ |
| 1.5 | Device hotplug | Complete | 4/4 ✓ |
| 1.6 | Event loop architecture (poll/epoll + clean shutdown path) | Complete | 5/5 ✓ |
| 1.1b | Portal backend (behind feature flag) | Complete | (part of 1.1) |
| **Phase 2** | **Power features** | **✅ Complete** | |
| 2.1 | Key sequences / chords | Complete | 9/9 ✓ |
| 2.2 | Event grabbing (EVIOCGRAB + uinput) | Complete | 6/6 ✓ |
| 2.3 | Modes / layers | Complete | 7/7 ✓ |
| 2.4 | Device-specific hotkeys | Complete | 5/5 ✓ |
| 2.5 | Tap vs. hold | Complete | 7/7 ✓ |
| **Phase 3** | **Polish** | **✅ Complete** | |
| 3.1 | Async API | Complete | 6/6 ✓ |
| 3.2 | Debouncing / rate limiting | Complete | 4/4 ✓ |
| 3.3 | Key state query | Complete | 5/5 |
| 3.4 | Configuration serialization | Complete | 5/5 ✓ |
| **Phase 4** | **Key Remapping & Event Transformation** | **Not Started** | |
| 4.1 | Codebase cleanup | Not Started | 0/17 |
| 4.2 | First-class output actions | Not Started | 0/8 |
| 4.3 | Key event caching across layer transitions | Not Started | 0/6 |
| 4.4 | Key remapping (input → different output) | Not Started | 0/8 |
| 4.5 | Oneshot layers | Not Started | 0/6 |
| 4.6 | Overload variants | Not Started | 0/6 |
| 4.7 | Chord support (simultaneous key combinations) | Not Started | 0/7 |
| 4.8 | Non-key event support (mouse, scroll) — *stretch* | Not Started | 0/6 |
| 4.9 | Full layer keymaps — *stretch* | Not Started | 0/7 |
| **Phase 5** | **Expansion (not committed)** | **Not Started** | |
| 5.1 | macOS backend (CGEventTap / IOKit) | Not Started | — |
| 5.2 | Windows backend (low-level keyboard hooks) | Not Started | — |
| 5.3 | Rename crate to something platform-neutral | Complete | ✓ |

Phase 1 makes the crate publishable. Phase 2 makes it the obvious choice.
Phase 3 makes it production-ready for demanding applications.
Phase 4 adds keyd-class key remapping and event transformation capabilities.
Phase 5 is an option, not a promise — pursue it if the API proves itself.

---

## Definition of done (quality gates)

Because this is not an MVP, each major item must meet explicit acceptance
criteria before it is considered complete:

1. **Behavioral tests**
   - Unit tests for parser/state-machine logic and edge cases.
   - Integration tests for register/unregister semantics and conflict errors.
   - Feature-gated tests for portal/grab behavior where supported.
   - CI coverage across key feature matrices to catch compile-time gating
     regressions.
2. **Failure-mode coverage**
   - Explicit error variants for backend selection failures, unsupported
     features, permission denial, and device disconnects.
   - No silent downgrade that changes security behavior (e.g., grab requested
     but unavailable).
3. **Docs parity**
   - README and rustdoc examples must match current API and feature flags.
   - Default-feature behavior and opt-in requirements are documented
     unambiguously (especially portal availability expectations).
   - Backend-specific caveats documented (portal availability varies by
     compositor/session).
4. **Threading/runtime guarantees**
   - No callback invocation while holding registry locks.
   - Callback panics must not silently kill hotkey processing; define panic
     policy (contain/log/disable callback) and test it.
   - Clean shutdown semantics for listener/backends and deterministic resource
     cleanup.
