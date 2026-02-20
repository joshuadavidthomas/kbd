# Agent Guidelines

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

## Testing
**All tests must pass.** If a test fails, it is your responsibility to fix it — even if you didn't cause the failure. Never dismiss failures as "pre-existing" or "unrelated".
