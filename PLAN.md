# evdev-hotkey: Roadmap

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

No crate bridges these approaches. `evdev-hotkey` will be the first library
that "just works" across all Linux environments, and the only Rust crate with
key sequences, modal layers, and event interception.

---

## Phase 1: Foundation (make it worth publishing)

These items make the crate a credible alternative to existing options.

### 1.1 Unified backend: XDG portal + evdev with automatic fallback

**Status: Not Started** · **Priority: Critical — this is the moat**

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

### 1.2 Release / hold events

**Status: Not Started** · **Priority: High — low effort, high value**

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

### 1.3 String parsing for hotkey definitions

**Status: Not Started** · **Priority: High — table stakes**

Both competitors have this. Users need it for config files, CLI tools, and
anywhere hotkeys are defined by end users rather than hardcoded.

```rust
let hotkey = "Ctrl+Shift+A".parse::<Hotkey>()?;
let hotkey = "Super+Return".parse::<Hotkey>()?;
let sequence = "Ctrl+K, Ctrl+C".parse::<HotkeySequence>()?;
```

Case-insensitive, supports common aliases (`Super`/`Meta`/`Win`,
`Ctrl`/`Control`, `Return`/`Enter`). Round-trips via `Display`.

### 1.4 Conflict detection

**Status: Not Started** · **Priority: High — correctness**

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

### 1.5 Device hotplug

**Status: Not Started** · **Priority: High — reliability**

Keyboards get unplugged, Bluetooth devices reconnect. The listener should
handle this without restarting.

Use `inotify` to watch `/dev/input` for new `event*` files. When a new device
appears, probe it for keyboard capabilities and add it to the listener. When a
device disappears (fd returns errors), remove it from the poll set.

Hotplug handling must also repair key/modifier state on disconnect. If a
device vanishes while keys are held, synthesize release/state cleanup so
modifiers do not get stuck active.

### 1.6 Event loop architecture (latency + CPU correctness)

**Status: In Progress** · **Priority: High — production behavior**

> Current state: Basic sleep-based polling loop (5ms interval) with non-blocking
> reads. Needs migration to poll/epoll-driven model.

The current polling-style listener design is simple but can waste CPU and add
latency jitter under load. Move to a poll/epoll-driven wait model for device
FDs and hotplug notifications so callbacks fire promptly without busy waiting.

Requirements:
- No fixed sleep-based busy loop in steady state
- Bounded shutdown latency when stop is requested
- Deterministic behavior when many events arrive in bursts

---

## Phase 2: Power features (make it the obvious choice)

These features differentiate from every existing Rust crate.

### 2.1 Key sequences / chords

**Status: Not Started** · **Priority: Critical — no Rust crate has this**

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

### 2.2 Event grabbing / interception

**Status: Not Started** · **Priority: High — essential for real hotkey daemons**

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

### 2.3 Modes / layers

**Status: Not Started** · **Priority: High — no Rust crate has this**

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

### 2.4 Device-specific hotkeys

**Status: Not Started** · **Priority: Medium — natural fit for evdev**

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

### 2.5 Tap vs. hold (dual-function keys)

**Status: Not Started** · **Priority: Medium — popular in keyboard community**

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

---

## Phase 3: Polish (make it production-ready)

### 3.1 Async API

**Status: Not Started**

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

### 3.2 Debouncing / rate limiting

**Status: Not Started**

Prevent rapid-fire callback invocations:

```rust
HotkeyOptions::new()
    .debounce(Duration::from_millis(100))   // ignore triggers within 100ms
    .max_rate(Duration::from_millis(500))   // at most once per 500ms
```

### 3.3 Key state query API

**Status: In Progress**

> Current state: Modifier state is tracked internally in the listener thread
> (`active_modifiers: HashSet<KeyCode>`), but not exposed via any public API.

Expose the internal modifier tracking as a public API:

```rust
manager.is_key_pressed(KeyCode::KEY_LEFTCTRL)  // -> bool
manager.active_modifiers()                       // -> HashSet<KeyCode>
```

### 3.4 Configuration serialization

**Status: Not Started**

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

- **Key remapping**: This is a hotkey library, not a remapper. Use keyd or
  xremap for full remapping.
- **Text expansion / hotstrings**: Out of scope. Different problem domain.
- **General-purpose input simulation / macro playback**: Out of scope.
  Synthetic events are allowed only when required for explicit hotkey features
  (e.g., grab passthrough, timeout-key fallback, tap-hold resolution).
- **GUI / system tray**: This is a library, not a daemon.

### Cross-platform: door is open

The initial focus is Linux — that's where the gap is and where the backend
trait architecture (Phase 1.1) gets battle-tested. But that same trait design
means platform backends can be added without touching existing code:

| Platform | Potential backend          | Feature flag |
|----------|---------------------------|--------------|
| macOS    | CGEventTap / IOKit        | `macos`      |
| Windows  | Low-level keyboard hooks  | `windows`    |

If/when this happens, the crate should be renamed to something
platform-neutral (`keybind`, `hotkey-daemon`, `hkd`, or similar) and
`evdev-hotkey` becomes a thin re-export crate for backwards compatibility.

This is **not committed scope** — it's an architectural option that Phase 1.1
preserves for free. Ship Linux-first, prove the API, expand later.

---

## Implementation order

| Item | Description | Status |
|------|-------------|--------|
| **Phase 1** | **Foundation** | |
| 1.1 | Backend trait + evdev backend (refactor current code) | Not Started |
| 1.2 | Release/hold events | Not Started |
| 1.3 | String parsing | Not Started |
| 1.4 | Conflict detection | Not Started |
| 1.5 | Device hotplug | Not Started |
| 1.6 | Event loop architecture (poll/epoll + clean shutdown path) | In Progress |
| 1.1b | Portal backend (behind feature flag) | Not Started |
| **Phase 2** | **Power features** | |
| 2.1 | Key sequences / chords | Not Started |
| 2.2 | Event grabbing (EVIOCGRAB + uinput) | Not Started |
| 2.3 | Modes / layers | Not Started |
| 2.4 | Device-specific hotkeys | Not Started |
| 2.5 | Tap vs. hold | Not Started |
| **Phase 3** | **Polish** | |
| 3.1 | Async API | Not Started |
| 3.2 | Debouncing / rate limiting | Not Started |
| 3.3 | Key state query | In Progress |
| 3.4 | Configuration serialization | Not Started |
| **Phase 4** | **Expansion (not committed)** | |
| 4.1 | macOS backend (CGEventTap / IOKit) | Not Started |
| 4.2 | Windows backend (low-level keyboard hooks) | Not Started |
| 4.3 | Rename crate to something platform-neutral | Not Started |

Phase 1 makes the crate publishable. Phase 2 makes it the obvious choice.
Phase 3 makes it production-ready for demanding applications.
Phase 4 is an option, not a promise — pursue it if the API proves itself.

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
