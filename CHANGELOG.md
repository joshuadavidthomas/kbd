# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project attempts to adhere to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

<!--
## [${version}](https://github.com/joshuadavidthomas/kbd/releases/tag/${tag})

_For multi-package releases, list package versions here_

### Added - for new features
### Changed - for changes in existing functionality
### Deprecated - for soon-to-be removed features
### Removed - for now removed features
### Fixed - for any bug fixes
### Security - in case of vulnerabilities

-->

## [Unreleased]

## [0.2.0](https://github.com/joshuadavidthomas/kbd/releases/tag/kbd-v0.2.0)

### Added

- Added sequence matching to the dispatcher and manager, enabling multi-key shortcut sequences (e.g., `g g` or `Ctrl+K Ctrl+C`).
- Added sealed `HotkeyInput` trait so `Dispatcher::register()` and `HotkeyManager::register()` accept `Hotkey`, `Key`, `&str`, or `String` directly.
- Added binding provenance tracking with `BindingSource` (Default, User, Custom) for source-aware precedence resolution.
- Added device-specific bindings via `DeviceFilter` and per-device modifier isolation, allowing bindings scoped to individual input devices.
- Added per-binding debounce, rate limiting, and repeat policy through new `BindingOptions` extensions.
- Added tap-hold dual-function key support with three resolution paths: tap (before threshold), hold by duration (timeout), and hold by interrupt (another key pressed).
- Added `BindingId` to layer bindings, unifying binding identity across global and layer scopes.
- Added benchmark infrastructure with divan benchmarks and CodSpeed CI integration.

### Changed

- **Breaking:** Replaced `Vec<Modifier>` with a `u8` bitmask (`ModifierSet`), making `Hotkey` `Copy` and eliminating heap allocations for modifier storage. Code that captures `Hotkey` in non-move closures may behave differently.
- **Breaking:** Replaced monolithic `kbd::error::Error` enum with scoped error types: `RegisterError`, `LayerError`, `QueryError`, `ShutdownError`, and `StartupError`.
- **Breaking:** Renamed `RegisteredBinding` to `Binding` and `RegisteredSequenceBinding` to `SequenceBinding`.
- **Breaking:** `MatchResult::Matched` now includes a `repeat_policy` field. Code that constructs or pattern-matches this variant will need to be updated.
- **Breaking:** `BindingInfo` now has a `source` field. Code that constructs `BindingInfo` with struct literals will need to include it.
- **Breaking:** Removed `Dispatcher::check_timeouts`; timeout handling is now internal to the dispatcher.
- **Breaking (kbd-evdev):** Removed `KbdKeyExt` and `EvdevKeyCodeExt` re-exports from the crate root. Use the explicit module paths instead.
- **Breaking (kbd-global):** Removed root re-exports of `Backend`, `Error`, `BindingGuard`, `HotkeyManagerBuilder`, and `HotkeyManager`. Use the explicit module paths instead.
- Switched sequence bindings from `HashMap` to `BTreeMap`, eliminating an O(n log n) sort on every keypress during sequence matching.
- Refreshed crate and module documentation with hero examples, architecture guides, and clearer introductions for all crates.

### Removed

- **Breaking:** Removed prelude module and root-level re-exports across `kbd`, `kbd-evdev`, and `kbd-global`. All types are now accessed via explicit module paths (e.g., `kbd::hotkey::Hotkey`).

## [0.1.0](https://github.com/joshuadavidthomas/kbd/releases/tag/kbd-v0.1.0)

### Added

- **kbd** — Pure-logic keyboard shortcut engine: key types (newtype over `keyboard-types` W3C physical key codes), modifier tracking, hotkey parsing/display with aliases, binding matching, layer stacks (oneshot, swallow, timeout), press cache, and introspection API. No platform dependencies.
- **kbd-crossterm** — crossterm bridge: `KeyCode`/`KeyEvent`/`KeyModifiers` to `kbd` type conversions via extension traits.
- **kbd-egui** — egui bridge: egui key and modifier conversions.
- **kbd-evdev** — Linux evdev backend: device discovery, hotplug via inotify, `EVIOCGRAB` exclusive grab, and uinput virtual device forwarding.
- **kbd-global** — Threaded global hotkey runtime: message-passing architecture, `HotkeyManager` with RAII handles, evdev backend integration, grab mode, and `kbd` type re-exports.
- **kbd-iced** — iced bridge: iced key event and modifier conversions.
- **kbd-tao** — tao bridge: tao (Tauri's winit fork) key event conversions.
- **kbd-winit** — winit bridge: `KeyCode`/`KeyEvent`/`ModifiersState` to `kbd` type conversions.
