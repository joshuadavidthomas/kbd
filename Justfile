set dotenv-load := true
set unstable := true

# List all available commands
[private]
default:
    @just --list --list-submodules

check *ARGS:
    cargo check {{ ARGS }}

clean:
    cargo clean

clippy *ARGS:
    cargo clippy --all-targets --all-features --benches --fix {{ ARGS }} -- -D warnings

fmt *ARGS:
    cargo +nightly fmt {{ ARGS }}
