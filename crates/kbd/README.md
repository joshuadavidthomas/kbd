# kbd

[![crates.io](https://img.shields.io/crates/v/kbd.svg)](https://crates.io/crates/kbd)
[![docs.rs](https://docs.rs/kbd/badge.svg)](https://docs.rs/kbd)

The pure matching engine at the center of the [`kbd` workspace](https://github.com/joshuadavidthomas/kbd).

You describe bindings — as strings like `"Ctrl+Shift+A"` or programmatically — and the dispatcher tells you when incoming key events match. It has no platform dependencies and no runtime thread of its own; you bring the key events from wherever you have them.

## Example

```rust
use kbd::action::Action;
use kbd::dispatcher::Dispatcher;
use kbd::hotkey::{Hotkey, Modifier};
use kbd::key::Key;
use kbd::layer::Layer;

let mut dispatcher = Dispatcher::new();

// Global bindings — register via string parsing...
dispatcher.register("Ctrl+S", Action::Suppress)?;

// ...or build hotkeys programmatically
dispatcher.register(
    Hotkey::new(Key::P).modifier(Modifier::Ctrl).modifier(Modifier::Shift),
    Action::Suppress,
)?;

// Layer bindings — active only when the layer is pushed
let normal = Layer::new("normal")
    .bind(Key::J, || println!("down"))?
    .bind(Key::K, || println!("up"))?
    .bind(Key::I, Action::PushLayer("insert".into()))?;

dispatcher.define_layer(normal)?;
dispatcher.push_layer("normal")?;
```

Layers stack. The most recently pushed layer is checked first, then global bindings. Layers can be oneshot (auto-pop after one match), swallowing (consume unmatched keys), or time-limited.

Multi-step bindings work too — register a sequence like `"Ctrl+K, Ctrl+C"` and the dispatcher tracks partial matches, returning `Pending` until the sequence completes or times out.

## What else is in here

Beyond hotkeys, layers, and sequences:

- **Tap-hold** — dual-function keys that do one thing on tap, another on hold. Requires grab mode in `kbd-global`.
- **Device filtering** — bind to specific keyboards by name, vendor/product ID, or physical path. Useful when you want different bindings for different devices.
- **Introspection** — query what's registered, which layers are active, and where bindings conflict or shadow each other.
- **Binding policies** — per-binding control over key propagation (consume vs. forward), repeat handling, and rate limiting.
- **String parsing** — `"Ctrl+Shift+A"`, `"Super+1"`, `"Ctrl+K, Ctrl+C"` all parse into typed values. Common aliases (`Cmd` → `Super`, `Win` → `Super`, `Return` → `Enter`) are built in.

## Physical and logical input

Legacy `Key`, `Hotkey`, `HotkeySequence`, `register`, and `process` use physical key positions. `Key::A` means "the key in the A position on a QWERTY layout" regardless of whether the user's layout is AZERTY, Dvorak, or Colemak. Their string formats and serialized output are unchanged.

For layout-sensitive shortcuts, parse an explicit `BindingPattern` and pass observations to `process_event`:

```rust
use kbd::action::Action;
use kbd::binding::BindingOptions;
use kbd::dispatcher::Dispatcher;
use kbd::observation::NamedKey;
use kbd::sequence::SequenceOptions;

let mut dispatcher = Dispatcher::new();
dispatcher.register_pattern(
    r#"Ctrl+logical:"s""#.parse()?,
    Action::Suppress,
    BindingOptions::default(),
)?;
dispatcher.register_sequence_pattern(
    r#"Ctrl+physical:K, logical:",", logical:Enter"#.parse()?,
    Action::Suppress,
    SequenceOptions::default().with_logical_abort_key(NamedKey::Escape),
)?;
```

- `physical:A` selects a position; `logical:"a"` selects an exact character string; `logical:Enter` selects a named key. `logical:"Enter"` is a character string, not the named key. Strings preserve case, whitespace, and Unicode (including multi-scalar and empty values), without normalization or layout inference.
- Quotes use JSON escapes. `+` and `,` inside quotes are literal characters. Unqualified old strings still mean physical keys. New patterns/sequences display and serialize with explicit domain markers; old `Hotkey`/`HotkeySequence` output stays unchanged. Parsing works without the `serde` feature.
- `BindingSequence` is a non-empty list of patterns for **input matching**. Use `register_sequence_pattern` or `Layer::bind_sequence_pattern` with explicit `SequenceOptions`. Physical `SequenceInput` and `Action::EmitSequence` do not accept logical patterns.
- `KeyboardObservation` carries optional physical/logical identities, supplied modifiers, and transition. A press advances each candidate at most one step, even when both identities match; Repeat/Release do not advance sequences. Missing identities are not inferred.
- Active sequence candidates resolve before fresh bindings. Layers rank above globals. Within a scope, single-step sequences precede multi-step prefixes, which defer standalone bindings. Keep all matching prefixes; simultaneous completions prefer physical at the earliest differing step, then stable declaration/ID order. No sequence source tiers are added.
- Immediate layer bindings retain first eligible declaration within each domain, with physical above logical; source labels do not rank layers. Global immediate bindings rank device scope, source, domain, then registration order.
- `bindings_for_event` and its device-aware variant are nonmutating **fresh-event classifiers**, not simulations of pending sequences, tap-hold, or throttling. Static listing/conflicts cannot infer cross-domain overlap without an observation.

### Timeout and cancellation semantics

Each successful step refreshes the timeout. A deferred standalone fires only when all remaining candidates expire while waiting for step 2. Progress, a live mismatch, abort, unregister, or layer removal never fires that fallback. A mismatch retries the current event against fresh bindings. For compatibility, if all candidates are already expired when an event arrives, returning the fallback consumes that event; poll `pending_timeouts` **before** input processing to resolve expiry separately. Resolve collected timeout tokens before mutating registrations/state.

The default abort remains **physical Escape**. `with_logical_abort_key` selects an exact logical identity for logical-only input; abort ignores modifiers, and a matching expected next step wins over abort. `SequenceOptions` is now `Clone`, not `Copy`, and `abort_key()` returns `&SequenceAbortKey` so logical aborts are represented truthfully.

`cancel_pending_sequence()` silently and idempotently clears sequence candidates and their fallback and revokes previously collected sequence/fallback timeout tokens. It does not clear registrations, layers, throttle history, tap-hold, or held-key/modifier state; collected tap-hold timeout tokens remain valid. Hosts can compose it into their own lifecycle reset.

## Feature flags

| Feature | Default | Effect |
|---|---|---|
| `serde` | off | Adds `Serialize` and `Deserialize` to key/hotkey types, `BindingPattern`, and `BindingSequence` |

## License

kbd is licensed under the MIT license. See the [`LICENSE`](../../LICENSE) file for more information.
