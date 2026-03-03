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
    cargo clippy --all-targets --all-features --benches --fix --allow-dirty {{ ARGS }} -- -D warnings

doc *ARGS:
    RUSTDOCFLAGS="--cfg docsrs" cargo +nightly doc --all-features --workspace --no-deps {{ ARGS }}

fmt *ARGS:
    cargo +nightly fmt {{ ARGS }}

lint *ARGS:
    uvx prek run --all-files --show-diff-on-failure --color always
