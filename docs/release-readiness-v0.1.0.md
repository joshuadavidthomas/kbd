# Release Readiness Review (pre-v0.1.0)

## What looks strong

1. **Core crates have broad automated coverage and passed targeted tests in this environment.**
   - `kbd` has substantial unit tests around matching, layers, and parsing behavior.
   - `kbd-evdev` and `kbd-global` also include focused test suites for device/runtime behavior.
   - In this environment, `cargo test -q -p kbd -p kbd-evdev -p kbd-global -p kbd-crossterm` passed.

2. **Public package metadata is in good shape for crates.io discoverability.**
   - Published crates include `description`, `readme`, `keywords`, `categories`, `repository`, and `license-file` metadata.
   - Workspace-level package metadata is centralized (edition, rust-version, authors, license-file, repository).

3. **Docs/release signaling is already present.**
   - Root README clearly explains crate roles.
   - Changelog has a `0.1.0` entry that enumerates crate-level scope.

## Issues to address before releasing “all crates”

### 1) **Publishability mismatch in the current dependency graph (blocker for “all crates”)**

`kbd-global` depends on `kbd-evdev`, but `kbd-evdev` is currently marked `publish = false`.

- `kbd-global` dependency: `kbd-evdev = { workspace = true }`
- `kbd-evdev` package setting: `publish = false`

If the plan is truly to release *all workspace crates*, this needs a decision before release:
- either publish `kbd-evdev`,
- or mark `kbd-global` as non-publishable,
- or split out/feature-gate the dependency path.

### 2) **Formatter/tooling friction for contributors and CI reproducibility**

The repo rustfmt config uses nightly-only options (`unstable_features = true` and unstable import formatting options), while this environment uses stable `cargo fmt`, which emits warnings and can disagree with expected style.

Recommendation:
- pin a toolchain (`rust-toolchain.toml`) if nightly formatting is required, **or**
- switch `.rustfmt.toml` to stable-only options.

### 3) **Full-workspace testability requires additional system dependencies**

Running full `cargo test -q` in this environment fails while compiling `glib-sys` because `glib-2.0` headers/pkg-config metadata are unavailable.

That is expected for GUI-adjacent crates on minimal Linux images, but for release quality it should be explicit in contributor docs/CI matrix:
- which crates require system libs,
- and which CI jobs validate full workspace vs. minimal/headless subsets.

### 4) **Planning docs contain stale crate naming that can confuse first-time contributors**

`PLAN.md` still references the old `kbd-core` naming in places while the workspace now uses `kbd`.

Not release-blocking, but cleaning this now will reduce onboarding confusion at the 0.1.0 launch point.

## Suggested release checklist (short)

1. Resolve the `kbd-global` ↔ `kbd-evdev` publish policy mismatch.
2. Decide and document formatter/toolchain policy (stable vs nightly rustfmt).
3. Document required OS packages for GUI/full-workspace builds.
4. Run and record final pre-release checks (tests/lints/format) in CI and release notes.
5. Optionally: refresh stale planning nomenclature before tagging.

## Commands executed for this review

- `cargo test -q` (failed due to missing `glib-2.0` system library in this environment)
- `cargo test -q -p kbd -p kbd-evdev -p kbd-global -p kbd-crossterm` (passed)
- `cargo clippy -q -p kbd -p kbd-evdev -p kbd-global -p kbd-crossterm --all-targets --all-features` (passed)
- `cargo fmt --all --check` (failed initially; formatting differences found)
- `cargo fmt --all` (applied formatting)
