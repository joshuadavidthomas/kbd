# API Review for v0.1.0 Release

## Crate Names

| Crate | Verdict | Notes | Decision |
|-------|---------|-------|----------|
| `kbd` | ✅ Keep | Short, memorable, describes the domain. | |
| `kbd-crossterm` | ✅ Keep | | |
| `kbd-egui` | ✅ Keep | | |
| `kbd-iced` | ✅ Keep | | |
| `kbd-tao` | ✅ Keep | | |
| `kbd-winit` | ✅ Keep | | |
| `kbd-evdev` | ✅ Keep | | |
| `kbd-global` | ✅ Keep | Name says "global" but crate is really a runtime/manager for global hotkeys on Linux. "Global hotkeys" is standard terminology though, and `HotkeyManager` inside it makes the purpose click. | |
| `kbd-portal` | ✅ Keep | (publish=false) | |
| `kbd-derive` | ✅ Keep | (publish=false) | |
| `kbd-xkb` | ✅ Keep | (publish=false) | |

## High Confidence Changes

### 1. Rename `Matcher` → `Dispatcher` or `ShortcutEngine`

**Current:** `Matcher`

**Problem:** `Matcher` undersells the type. It doesn't just match — it manages layers, tracks sequences, handles timeouts, and applies actions. It's the stateful core of the entire system.

**Options:**
- [ ] `ShortcutEngine`
- [ ] `Dispatcher`
- [ ] `BindingEngine`
- [ ] `KeyRouter`
- [ ] Keep `Matcher`

**User Response**: engine is strong, but it's.. more than shortcuts, right? Dispatcher is good but a bit generic? BindingEngine sounds... kinky. But I agree, Matcher stinks.

### 2. Rename `Handle` → `BindingHandle` or `BindingGuard`

**Current:** `Handle` (in `kbd-global`)

**Problem:** `Handle` is extremely generic. A handle to what? It's a handle to a registered binding that auto-unregisters on drop.

**Options:**
- [ ] `BindingHandle`
- [ ] `BindingGuard` (follows Rust "guard" pattern like `MutexGuard`)
- [ ] `Registration`
- [ ] Keep `Handle`

**User Response**: KeyBindingGuard? is that too verbose? is it a handle or a guard? anything drop related makes me think guard, but i'm a relative newbie to Rust

### 3. Rename `Passthrough` enum and variants

**Current:**
```rust
pub enum Passthrough {
    Consume,  // default — swallow the event
    Enabled,  // forward the event
}
```

**Problem:** The enum name says "Passthrough" but the default is `Consume` (not passing through). `Enabled` is a weak variant name. The framing is inverted.

**Options:**
- [ ] `KeyPropagation { Stop, Continue }` (follows GTK's `Propagation` / DOM's `stopPropagation()`)
- [ ] `EventHandling { Consume, Forward }`
- [ ] Keep `Passthrough { Consume, Enabled }`

**User Response**: KeyPropagation is good

### 4. Change `Action::EmitKey(Key, Vec<Modifier>)` → `Action::EmitHotkey(Hotkey)`

**Current:**
```rust
Action::EmitKey(Key, Vec<Modifier>)
```

**Problem:** The `Hotkey` type already exists and represents exactly `Key + Vec<Modifier>`. Using loose fields creates asymmetry with `Action::EmitSequence(HotkeySequence)` which does use the composed type.

**Options:**
- [ ] `Action::EmitHotkey(Hotkey)`
- [ ] Keep `Action::EmitKey(Key, Vec<Modifier>)`

**User Response**: yes, let's make this change

## Medium Confidence Changes

### 5. Rename `Action::Swallow` → `Action::Suppress` or `Action::Discard`

**Current:** `Action::Swallow`

**Problem:** "Swallow" appears in three places with slightly different meanings:
- `Action::Swallow` — intentional no-op that eats the key
- `UnmatchedKeyBehavior::Swallow` — layer consumes unmatched keys
- `MatchResult::Swallowed` — a layer actively consumed the event

**Options:**
- [ ] `Action::Suppress`
- [ ] `Action::Discard`
- [ ] Keep `Action::Swallow`

**User Response**: hmm, this one is tough, both are fine, i'll defer to you

### 6. Rename `MatchResult::Swallowed` / `MatchResult::Ignored`

**Current:**
```rust
pub enum MatchResult<'a> {
    Matched { ... },
    Pending { ... },
    NoMatch,
    Swallowed,  // a layer actively consumed it
    Ignored,    // modifier-only press, release, or repeat
}
```

**Problem:** `Swallowed` vs `Ignored` — are these meaningfully different to callers? If yes, the names should make the distinction clearer. If no, consider merging.

**Options:**
- [ ] `Suppressed` / `Skipped` (clearer distinction, disambiguates from `Action::Swallow`)
- [ ] Merge into a single variant if callers treat them the same
- [ ] Keep `Swallowed` / `Ignored`

**User Response**: I dunno if these are meaningfully different! They might be? think through this.. i definitely agree with the rename (though Skipped and Ignored are confusing, and i guess Supressed and Ignored too) -- the two events might not be needed, but i want you to think it through first

### 7. Gate `kbd-evdev` `testing` module behind a feature flag

**Current:** `pub mod testing` is always compiled (not behind `#[cfg(test)]`).

**Problem:** `RecordingForwarder` and `ForwardedEvents` ship in release builds. If they're intentionally public for downstream test support, a `testing` feature flag would be more conventional.

**Options:**
- [ ] Add `testing` feature flag
- [ ] Move behind `#[cfg(test)]`
- [ ] Keep as-is (intentionally always available)

**User Response**: Ah... i don't generally like pub mods like that? why are they in a testing module in the first place? i'd prefer the cfg test of the two

### 8. Reduce visibility of `INPUT_DIRECTORY` / `VIRTUAL_DEVICE_NAME` in `kbd-evdev`

**Current:** Both are `pub` constants.

**Problem:** These are likely implementation details. Do downstream crates need them?

**Options:**
- [ ] Change to `pub(crate)`
- [ ] Keep `pub` (downstream crates do need them)

**User Response**: Dunno! harmless to do pub crate for now i guess? unless the other planned crates have anything to do with this one

## Low Priority (Note but Probably Keep)

### 9. `UnmatchedKeyBehavior` — verbose name

**Current:** `UnmatchedKeyBehavior` (20 characters)

**Suggestion:** `UnmatchedKeys` would be shorter with the same variants (`Fallthrough` / `Swallow`).

**Options:**
- [ ] `UnmatchedKeys`
- [ ] Keep `UnmatchedKeyBehavior`

**User Response**: UnmatchedKeys is good

### 10. `EvdevKeyExt` / `KeyCodeExt` — asymmetric naming in `kbd-evdev`

**Current:**
- `KeyCodeExt` extends `evdev::KeyCode` with `to_key()`
- `EvdevKeyExt` extends `kbd::Key` with `to_key_code()`

One is named after the source type, the other prefixed with framework. Mainly internal so low impact.

**Options:**
- [ ] `EvdevKeyCodeExt` / `KbdKeyExt` (unambiguous)
- [ ] Keep current names

**User Response**: yes, let's rename for consistency

### 11. Standalone functions in winit/tao bridges — slight inconsistency

**Current:**
- `kbd-winit` exports `physical_key_to_hotkey()`
- `kbd-tao` exports `keycode_to_hotkey()`
- Other bridges have no standalone functions

These exist because winit/tao separate physical key from event. Naming is inconsistent between the two.

**Options:**
- [ ] Align naming convention
- [ ] Keep as-is

**User Response**: align them suckers!
