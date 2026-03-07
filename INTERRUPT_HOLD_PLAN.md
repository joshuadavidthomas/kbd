# Move interrupt hold execution from Dispatcher to Engine

## Core insight

Interrupt-resolved holds and timeout-resolved holds are the same thing from
the engine's perspective: a tap-hold key resolved as hold, needs its action
executed and its press cache updated. The difference is just the trigger. We
can unify them by buffering interrupt-resolved holds in `TapHoldState` and
surfacing them through the existing `pending_timeouts` / `match_pending_timeout`
pipeline.

## What changes

### `crates/kbd/src/dispatcher/tap_hold.rs`

1. Add `resolved_holds: Vec<(Key, BindingId)>` to `TapHoldState` (update
   `Default`)
2. Change `resolve_pending_for_interrupt` to push `(*key, binding_id)` into
   `self.resolved_holds` instead of returning a `Vec`. Return type becomes
   `()` (or `bool` if we need "did anything resolve")
3. Simplify `on_press`: no more `HoldResolved` paths. If the pressing key
   is a tap-hold key → `Consumed`. If not → `PassThrough`. The resolved
   holds are a separate channel
4. Remove `HoldResolved` from `TapHoldOutcome`
5. Remove `TapHoldDecision` entirely
6. Add `pub(crate) fn drain_resolved_holds(&mut self) -> Vec<(Key, BindingId)>`
   that drains the buffer

### `crates/kbd/src/dispatcher/timeout.rs`

7. In `pending_timeouts`, after draining `check_timeouts`, also drain
   `self.tap_hold.drain_resolved_holds()` and wrap each as
   `PendingTimeout { kind: TapHoldHold { key, binding_id } }`

### `crates/kbd/src/dispatcher.rs`

8. Remove `TapHoldDecision` import
9. `process_tap_hold` returns `TapHoldOutcome` directly — no callback
   execution, no `is_registered` check. It becomes a thin wrapper:
   fast-path check, `Instant::now()`, dispatch to
   `on_press`/`on_release`/`on_repeat`
10. `process_internal` handles `TapHoldOutcome` directly:
    - `Consumed | RepeatConsumed` → return `Matched { Suppress, Stop, Suppress }`
    - `TapResolved { binding_id }` → look up tap action (same as today)
    - `PassThrough` → fall through to normal matching

### `crates/kbd-global/src/engine.rs`

11. No changes needed. The existing `pending_timeouts` loop already handles
    `TapHoldHold` correctly — executes the action, routes through
    `resolve_outcome`, updates the press cache via `cache_press`

## What this eliminates

- `TapHoldDecision` enum (entirely)
- `TapHoldOutcome::HoldResolved` variant
- Inline `catch_unwind(|| cb())` in the Dispatcher
- The `is_registered(key)` check in `process_tap_hold` (no longer needed —
  `on_press` returns `Consumed` directly when the key is tap-hold)

## Execution order change

Hold actions that resolve by interrupt currently fire during
`process_key_event` (inline in the Dispatcher). With this change, they fire
during the `pending_timeouts` phase at the end of the event loop iteration,
after all key events in the poll cycle are processed.

This is safe because:

- User callbacks are opaque — they don't affect dispatcher matching state
- The `active` entry is marked `Resolved` immediately during `on_press`,
  so repeat/release handling is unaffected
- The press cache gets properly updated (actually an improvement —
  interrupt holds currently DON'T update the press cache, but now they
  will, via the same path as timeout holds)
- `check_timeouts` already skips `Resolved` entries, so there's no
  double-resolution

## No double-resolution concern

When `pending_timeouts` runs:

1. `check_timeouts` iterates `active`, sees interrupt-resolved entries as
   `Resolved`, skips them
2. `drain_resolved_holds` returns the interrupt-resolved IDs
3. Both sets get wrapped as `PendingTimeout::TapHoldHold` — the engine
   handles them identically
