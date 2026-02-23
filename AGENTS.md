# Agent Guidelines

## Build/Test Commands
Prefer to use cargo commands with the `-q` flag, it saves on tokens.

```bash
cargo build -q
cargo test -q
cargo test test_name
just clippy                      # Lint with clippy (auto-fixes)
just fmt                         # Format code (requires nightly)
```

**NEVER use `cargo doc --open`** - it requires browser interaction.

**Before pushing**, always run `just clippy` and `just fmt`.

## Clippy/Fmt scope
When running `just clippy` or `just fmt`, all resulting changes are in scope for the current task. Nothing is "unrelated" just because tooling touched it. Do not revert or ignore clippy/fmt changes as "unrelated" noise.

## Testing
**All tests must pass.** If a test fails, it is your responsibility to fix it — even if you didn't cause the failure. Never dismiss failures as "pre-existing" or "unrelated".
