# Tap-Hold Press Cache Integration

Timeout-resolved hold actions bypass the press cache entirely. This creates
two gaps: forwarding decisions are silently discarded, and repeat handling
is impossible for hold actions.

## Current behavior

1. **Press**: cached as `MatchedConsumed` with `RepeatPolicy::Suppress` (correct for the pending phase)
2. **Hold resolved by timeout**: action fires from the event loop's timeout check, completely outside the press/release/repeat machinery
3. **Repeat while held**: looks up the stale press cache entry, sees `Suppress`, does nothing
4. **Release after hold**: removes press cache entry, dispatches through `process_tap_hold_release`

## The gaps

### Forwarding

When a hold resolves by timeout, the `MatchResult` returned by
`match_pending_timeout` has a `propagation` field that is ignored.
The action fires but no forwarding decision is made through
`resolve_outcome`. Today the dispatcher hardcodes `Stop` for hold
actions so nothing breaks, but the engine is silently discarding the
propagation decision — the same coupling pattern we already fixed in
`process_tap_hold_release`.

### Repeat

Once the hold resolves, the press cache still has the entry from the
original press (`Suppress` repeat, no callback). OS repeat events hit
`handle_repeat_event`, find the stale entry, and suppress. There is no
mechanism to update the cache with the hold action's repeat info after
timeout resolution.

## What needs to change

### 1. Carry the key in tap-hold timeout resolutions

`TimeoutKind::TapHoldHold` currently only has `binding_id`. It needs
the `Key` so the event loop knows which press cache entry to update.

### 2. Route timeout hold actions through `resolve_outcome`

The event loop's timeout handling currently does:

```rust
for pending in &pending {
    if let Some(MatchResult::Matched { action, .. }) =
        engine.dispatcher.match_pending_timeout(pending)
    {
        execute_action(action);
    }
}
```

This should go through the same `MatchResult → MatchOutcome →
KeyEventOutcome` pipeline as press and release events, including
calling `resolve_outcome` for forwarding decisions.

### 3. Update the press cache after hold resolution

After a hold resolves by timeout, the press cache entry for that key
should be updated with:
- The hold action's `KeyEventOutcome` (from `resolve_outcome`)
- A `RepeatInfo` with the hold action's callback and repeat policy

This way subsequent OS repeat events will re-fire the hold action
(if the repeat policy allows it), and release events will replay the
correct forwarding decision.

## Files involved

- `crates/kbd/src/dispatcher/timeout.rs` — `TimeoutKind::TapHoldHold` needs `Key`
- `crates/kbd/src/dispatcher/tap_hold.rs` — `check_timeouts` needs to return keys alongside binding IDs
- `crates/kbd-global/src/engine.rs` — event loop timeout handling, press cache update
