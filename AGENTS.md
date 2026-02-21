# Agent Guidelines

## Start here

1. Read [DESIGN.md](DESIGN.md) — the domain model, architecture, and design decisions.
2. Read [PLAN.md](PLAN.md) — the phased implementation plan. Find your phase.
3. Read the `src/` scaffold — every file has doc comments explaining its purpose and TODO items.

## Project state

This is a **ground-up rebuild**. The prior implementation is archived in
`archive/v0/` (and git-tagged `v0-archive`). Use it as reference for how
specific problems were solved — evdev hotplug edge cases, portal
initialization, modifier canonicalization — but do not copy its architecture.

The keyd C project in `reference/keyd/` is a reference for the key
remapping / layer model.

## Build/Test Commands
```bash
cargo build -q
cargo test -q
cargo test test_name
just clippy                      # Lint with clippy (auto-fixes)
just fmt                         # Format code (requires nightly)
# NEVER use `cargo doc --open` - it requires browser interaction
```

**Before pushing**, always run `just clippy` and `just fmt`.

## Clippy/Fmt scope
When running `just clippy` or `just fmt`, all resulting changes are in scope
for the current task. Nothing is "unrelated" just because tooling touched it.
Do not revert or ignore clippy/fmt changes as "unrelated" noise.

## Testing
**All tests must pass.** If a test fails, it is your responsibility to fix
it — even if you didn't cause the failure. Never dismiss failures as
"pre-existing" or "unrelated".

## Architecture rules (non-negotiable)

These are enforced across all phases. See DESIGN.md for rationale.

- **No `Arc<Mutex<>>`** in the engine. The engine owns its state. The
  manager communicates via message passing (command channel).
- **No bool fields** in types. Use enums. Even for two states.
- **No duplicated logic** between `Key` and `Modifier`. Share via trait,
  macro, or derivation.
- **No bare `String`** for domain values. Newtype it.
- **One binding type**, not four. Pattern + Action + Options.
- **One handle type**, not three. BindingId + CommandSender.
- **`thiserror`** for error types. No `Error(String)`.
- **Standard traits** for conversions (`From`/`Into`), not ad-hoc methods.
- **Callbacks panic-isolated** — a panicking user callback never kills the
  engine thread.
