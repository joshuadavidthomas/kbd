# Documentation Plan

Plan to bring every crate in the `kbd` workspace to docs.rs gold-standard quality, modeled after the best-documented crates in the Rust ecosystem.

## Reference Crates

These are the crates whose documentation we're using as models. Most already have READMEs saved in `reference/`.

| Crate | docs.rs | Why |
|---|---|---|
| csv | [docs.rs/csv](https://docs.rs/csv) | Embedded tutorial module, step-by-step guides — the community's #1 pick |
| regex | [docs.rs/regex](https://docs.rs/regex) | Structured front-page: Usage → Examples → Performance → Syntax → Features |
| itertools | [docs.rs/itertools](https://docs.rs/itertools) | Every method has a practical example showing *why*, not just *how* |
| once_cell | [docs.rs/once_cell](https://docs.rs/once_cell) | Small, focused, perfectly documented — good model for bridge crates |
| tokio | [docs.rs/tokio](https://docs.rs/tokio) | Rich module-level docs, feature-gated doc annotations |
| serde | [docs.rs/serde](https://docs.rs/serde) | Derive examples, trait docs, companion site |
| clap | [docs.rs/clap](https://docs.rs/clap) | Tutorial modules for different API styles |
| axum | [docs.rs/axum](https://docs.rs/axum) | Crate-level walkthrough with progressive disclosure |
| anyhow | [docs.rs/anyhow](https://docs.rs/anyhow) | dtolnay-quality README that doubles as crate docs |
| thiserror | [docs.rs/thiserror](https://docs.rs/thiserror) | Concise, example-driven, every variant documented |
| ratatui | [docs.rs/ratatui](https://docs.rs/ratatui) | Module hierarchy with widget examples |
| rayon | [docs.rs/rayon](https://docs.rs/rayon) | Parallel iterator docs with usage patterns |
| crossbeam | [docs.rs/crossbeam](https://docs.rs/crossbeam) | Channel docs with bounded/unbounded patterns |
| syn / quote | [docs.rs/syn](https://docs.rs/syn) | Proc-macro ecosystem docs with worked examples |

## Baseline Audit

Current state of documentation across all 11 crates.

### Quantitative Summary

| Crate | Files | Lines | `pub` items | `///` lines | `//!` lines | Doc tests | README |
|---|---|---|---|---|---|---|---|
| kbd | 9 | 3854 | 353 | 231 | 93 | 3 | ❌ |
| kbd-crossterm | 1 | 633 | 3 | 18 | 36 | 1 | ✅ 29 lines |
| kbd-egui | 1 | 517 | 3 | 27 | 44 | 1 | ✅ 35 lines |
| kbd-iced | 1 | 692 | 3 | 18 | 33 | 1 | ✅ 28 lines |
| kbd-tao | 1 | 615 | 4 | 29 | 33 | 1 | ✅ 28 lines |
| kbd-winit | 1 | 744 | 4 | 29 | 38 | 1 | ✅ 31 lines |
| kbd-evdev | 5 | 1536 | 31 | 49 | 55 | 1 | ✅ 15 lines |
| kbd-global | 17 | 3601 | 56 | 102 | 170 | 3 | ✅ 39 lines |
| kbd-derive | 1 | 14 | 0 | 0 | 14 | 0 | ❌ |
| kbd-portal | 1 | 32 | 2 | 7 | 15 | 0 | ❌ |
| kbd-xkb | 1 | 12 | 0 | 0 | 12 | 0 | ❌ |

### Qualitative Gaps

| Gap | Crates affected |
|---|---|
| `#![warn(missing_docs)]` not enabled | All 11 |
| No `[package.metadata.docs.rs]` in Cargo.toml | All 11 |
| No `#[doc = include_str!("../README.md")]` sync | All 11 |
| `pub mod` declarations undocumented | kbd, kbd-evdev |
| `pub use` re-exports undocumented | kbd, kbd-global |
| Module files missing `//!` header | kbd-global: `engine/command.rs`, `engine/runtime.rs`, `engine/types.rs`, `engine/wake.rs` |
| Feature flags undocumented | kbd (`serde`), kbd-global (`grab`, `serde`) |
| No `cfg_attr(docsrs, ...)` feature badges | All crates with features |
| Doc tests: most are `no_run` crate-level only | Bridge crates have 1 each; core needs more on individual items |
| Missing README.md | kbd, kbd-derive, kbd-portal, kbd-xkb |
| `#[doc(hidden)]` not used anywhere | May want for internal helpers |

## Standards

Every crate should meet these standards before we consider it done.

### Crate-Level (`//!` in lib.rs)

- [ ] One-sentence summary on the first line
- [ ] What problem this crate solves and when to use it
- [ ] Quick-start example (compiling doc test, not `no_run` when possible)
- [ ] Feature flags section (if any) with `#[doc = "..."]` or prose
- [ ] Links to important types using intra-doc links (`[Matcher]`, `[Key]`)
- [ ] "See also" links to related crates in the workspace

### Module-Level (`//!` at top of each .rs file)

- [ ] One-sentence summary of what the module contains
- [ ] How types in this module relate to the rest of the crate
- [ ] Link to the primary type(s) in the module

### Public Items (`///` on every `pub` item)

- [ ] Summary line (≤15 words)
- [ ] `# Examples` section with runnable doc test
- [ ] `# Errors` section on any `Result`-returning function
- [ ] `# Panics` section on any function that can panic
- [ ] Intra-doc links to related types

### Re-exports

- [ ] `pub use` items get a `#[doc(inline)]` or a `///` comment explaining the re-export
- [ ] `pub mod` declarations get a `///` one-liner

### Cargo.toml Metadata

- [x] `description` — concise (already done)
- [x] `keywords` — up to 5 (already done)
- [x] `categories` — valid crates.io categories (already done)
- [x] `[package.metadata.docs.rs]` — `all-features = true`, `rustdoc-args = ["--cfg", "docsrs"]`

### Feature Flags on docs.rs

- [x] `#![cfg_attr(docsrs, feature(doc_auto_cfg))]` in lib.rs for auto-badging
- [ ] Or manual `#[cfg_attr(docsrs, doc(cfg(feature = "...")))]` on gated items

### Doc Tests

- [ ] Use `?` operator with hidden `# fn main() -> Result<...>` boilerplate
- [ ] Prefer compiling+running over `no_run` where feasible
- [ ] Use `# ` hidden lines for setup that distracts from the point

### README ↔ Crate Docs Sync

- [x] Every published crate has a `README.md`
- [x] Keep `//!` for rustdoc, separate README for crates.io/GitHub

### Workspace Lints

- [x] Add `missing_docs = "warn"` to `[workspace.lints.rust]` in root `Cargo.toml`

## Phases

### Phase 1 — Workspace Infrastructure

Set up the scaffolding so every crate benefits automatically.

- [x] Add `missing_docs = "warn"` to `[workspace.lints.rust]` in root `Cargo.toml`
- [x] Add `#![cfg_attr(docsrs, feature(doc_auto_cfg))]` to every `lib.rs`
- [x] Add `[package.metadata.docs.rs]` to every `Cargo.toml`
- [x] Decide on README sync strategy: keep `//!` for rustdoc, separate README for crates.io/GitHub
- [x] Create missing READMEs: `kbd`, `kbd-derive`, `kbd-portal`, `kbd-xkb`
- [-] Fix all `missing_docs` warnings (deferred — each phase fixes its own crate's warnings)

### Phase 2 — `kbd` (Core Crate)

The most important crate. Everything depends on it. Model after `regex` (structured front-page) and `itertools` (examples on every method).

- [x] **lib.rs crate docs**: Rewrite to structured format — Summary → Quick Start → Modules Overview → Feature Flags → Example
- [x] **`pub mod` docs**: Add `///` summary to each of the 8 `pub mod` declarations
- [x] **`pub use` docs**: Add `///` or `#[doc(inline)]` to all 25 re-exports
- [x] **key.rs**: `#[allow(missing_docs)]` on constants impl block with group doc comment, `Hotkey` and `HotkeySequence` struct docs with examples, `Modifier` enum and variant docs, `ParseHotkeyError` variant docs
- [x] **binding.rs**: Examples for `BindingOptions`, `DeviceFilter`, `Passthrough`, `OverlayVisibility`
- [x] **layer.rs**: Examples for `Layer` construction and `UnmatchedKeyBehavior` variants
- [x] **matcher.rs**: Examples for `Matcher` lifecycle — create → register bindings → feed keys → match results
- [x] **action.rs**: Docs for `LayerName::new`, `LayerName::as_str`
- [x] **error.rs**: Docs for every error variant
- [x] **key_state.rs**: Docs for `KeyTransition`, `KeyState`, `apply_device_event`, `disconnect_device`, `is_pressed`
- [x] **introspection.rs**: Docs for all info types (`ActiveLayerInfo`, `BindingInfo`, `ConflictInfo`, etc.)
- [-] **Feature: `serde`**: Document what becomes serializable, show JSON example (deferred — serde derives not yet implemented)
- [x] **binding.rs**: Docs for all undocumented methods on `BindingId`, `BindingOptions`, `RegisteredBinding`, `DeviceFilter` fields
- [x] **matcher.rs**: Docs for `MatchResult` variant fields
- [x] **Zero `missing_docs` warnings** for `kbd` crate

### Phase 3 — Bridge Crates (crossterm, egui, iced, tao, winit)

These follow a consistent pattern. Model after `once_cell` (small, focused, perfect docs).

For each of the 5 bridge crates:

- [x] **Crate docs**: Ensure quick-start example compiles (not just `no_run`)
- [x] **Extension traits**: Add `# Examples` to each trait method, not just the crate-level example
- [x] **Conversion tables**: Add a module-level doc table showing the mapping (e.g., `crossterm::KeyCode::Char('a')` → `kbd::Key::A`)
- [x] **Trait method docs**: Every `fn` on every trait gets a `///` with at least a summary line
- [x] **README**: Update to match crate-level docs

### Phase 4 — `kbd-evdev`

Linux-specific, more complex. Model after `crossbeam` (clear module docs with usage patterns).

- [x] **lib.rs**: Expand crate docs with prerequisites (Linux, `/dev/input/` access, root/input group)
- [x] **convert.rs**: Add examples for `EvdevKeyExt` and `KeyCodeExt` traits
- [x] **devices.rs**: Document `INPUT_DIRECTORY`, `DeviceEvent` struct fields, `classify_change`
- [x] **forwarder.rs**: Document `Forwarder::new()` with `# Errors` section
- [x] **error.rs**: Document every error variant
- [x] **`pub mod` / `pub use`**: Add summary docs to all re-exports
- [x] **README**: Expand beyond the current 15 lines

### Phase 5 — `kbd-global`

Largest non-core crate (17 files). Model after `tokio` (rich module-level docs, feature annotations).

- [x] **lib.rs**: Expand crate docs — architecture overview, backend selection, lifecycle diagram
- [x] **`pub use` re-exports**: Add `///` or `#[doc(inline)]` to all 29 re-exports
- [x] **Module docs for engine/**: Add `//!` headers to `command.rs`, `runtime.rs`, `types.rs`, `wake.rs`
- [x] **manager.rs**: `HotkeyManager` and `HotkeyManagerBuilder` — full lifecycle examples
- [x] **handle.rs**: `Handle` — what you can do with it, examples
- [x] **events.rs**: Event types — what each event means
- [x] **backend.rs**: `Backend` enum — when to use each variant
- [x] **error.rs**: Every error variant documented
- [x] **Feature: `grab`**: Document what it enables, platform requirements
- [x] **Feature: `serde`**: Document what becomes serializable
- [x] **README**: Expand with architecture diagram and usage example

### Phase 6 — Placeholder Crates (derive, portal, xkb)

These are stubs. Keep docs proportional but correct.

- [x] **kbd-derive**: README explaining planned functionality
- [x] **kbd-portal**: README explaining planned functionality, current stub status
- [x] **kbd-xkb**: README explaining planned functionality
- [x] All three: Ensure `//!` docs clearly state "not yet implemented" status

### Phase 7 — Workspace-Level Polish

- [x] **Root README.md**: Workspace overview with crate map table and per-crate badges
- [ ] **`cargo doc` check**: Build docs for entire workspace, fix all warnings
- [ ] **`cargo test --doc`**: Ensure all doc tests pass
- [ ] **Cross-crate links**: Verify intra-doc links between crates resolve correctly
- [ ] **Consistent voice**: Review all crate docs for consistent tone and terminology
- [ ] **`just doc`**: Add a Justfile recipe for `cargo doc --all-features` with `RUSTDOCFLAGS="--cfg docsrs"`

## Follow-ups

Post-docs work to do on separate branches:

- **Remove all `pub use` re-exports from `kbd` lib.rs.** 25 top-level re-exports flatten the entire API into the crate root. Popular crates (regex, once_cell, anyhow, thiserror) re-export 0–2 types. Users should import from modules directly (`kbd::key::Hotkey`, `kbd::matcher::Matcher`, etc.). `kbd-global` already does this. Update doc examples and downstream crates to use module paths.

## Tracking

Mark items done by changing `[ ]` to `[x]` as each task is completed. Phases are meant to be worked in order — each builds on the previous — but items within a phase can be done in any order.
