# kbd-bench

Internal benchmark fixtures and microbenchmarks for the `kbd` workspace.

This crate is not published and is intended for workspace development only. It provides reusable helpers for constructing dispatchers with many bindings and benchmark suites for hotkey parsing, hashing, equality, and dispatch hot paths.

## What is here today

- Shared benchmark fixtures in `src/lib.rs`
- Dispatch benchmarks in `benches/dispatch.rs`
- Hotkey benchmarks in `benches/hotkey.rs`

## Status

- unpublished (`publish = false`)
- internal to this workspace
- versioned as `0.0.0`

## License

kbd-bench is licensed under the MIT license. See the [`LICENSE`](../../LICENSE) file for more information.
