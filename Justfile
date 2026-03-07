set dotenv-load := true
set unstable := true

# List all available commands
[private]
default:
    @just --list --list-submodules

bench *ARGS:
    cargo bench -p kbd-bench {{ ARGS }}

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

profile bench filter="":
    #!/usr/bin/env bash
    set -euo pipefail

    for cmd in valgrind jq rg; do
        if ! command -v "$cmd" &>/dev/null; then
            echo "Error: required command '$cmd' not found" >&2
            exit 1
        fi
    done

    if ! valgrind --version 2>/dev/null | rg -q 'codspeed'; then
        echo "Error: requires the valgrind-codspeed fork, not stock valgrind." >&2
        exit 1
    fi

    bench="{{ bench }}"
    filter="{{ filter }}"
    profiles_dir="profiles"

    label="${bench}"
    if [ -n "$filter" ]; then
        label="${bench}-${filter}"
    fi

    mkdir -p "$profiles_dir"

    echo "Building bench '${bench}' with debug info..."
    cargo bench -p kbd-bench --bench "$bench" --no-run -q

    binary=$(cargo bench -p kbd-bench --bench "$bench" --no-run --message-format=json 2>/dev/null \
        | jq -r 'select(.executable != null and .target.name == "'"$bench"'") | .executable' \
        | head -1)

    if [ -z "$binary" ] || [ ! -f "$binary" ]; then
        echo "Error: could not find bench binary for '${bench}'" >&2
        exit 1
    fi

    outfile="${profiles_dir}/${label}.callgrind"

    echo "Profiling with callgrind..."
    filter_args=""
    if [ -n "$filter" ]; then
        filter_args="$filter"
    fi

    valgrind --tool=callgrind \
        --callgrind-out-file="$outfile" \
        --cache-sim=yes \
        --collect-jumps=yes \
        "$binary" $filter_args 2>&1 \
        | rg -v '^==' || true

    rm -f "${outfile}."*

    callgrind_annotate --auto=no "$outfile" > "${outfile}.flat.txt" 2>/dev/null
    callgrind_annotate --auto=no --inclusive=yes --tree=calling "$outfile" > "${outfile}.tree.txt" 2>/dev/null

    echo ""
    echo "Profile written to:"
    echo "  ${outfile}          (raw callgrind)"
    echo "  ${outfile}.flat.txt (flat annotation)"
    echo "  ${outfile}.tree.txt (call-tree annotation)"
    echo ""
    echo "View interactively: kcachegrind ${outfile}"
