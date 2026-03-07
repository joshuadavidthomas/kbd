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

## [0.2.0](https://github.com/joshuadavidthomas/kbd/compare/kbd-v0.1.0...kbd-v0.2.0)

### Added
- Add per-binding debounce, rate limiting, and repeat policy ([#127](https://github.com/joshuadavidthomas/kbd/pull/127))
- Device-specific bindings with per-device modifier isolation ([#126](https://github.com/joshuadavidthomas/kbd/pull/126))
- Add binding provenance tracking w/source-aware precedence ([#125](https://github.com/joshuadavidthomas/kbd/pull/125))
- *(kdb)* Add sealed HotkeyInput trait for ergonomic hotkey registration ([#115](https://github.com/joshuadavidthomas/kbd/pull/115))
- Add sequence matching to dispatcher and manager ([#106](https://github.com/joshuadavidthomas/kbd/pull/106))

### Other
- Represent modifiers as a bitmask instead of Vec<Modifier> ([#138](https://github.com/joshuadavidthomas/kbd/pull/138))
- align binding module types with their domains ([#129](https://github.com/joshuadavidthomas/kbd/pull/129))
- fmt
- Clarify list_bindings doc comment ([#124](https://github.com/joshuadavidthomas/kbd/pull/124))
- *(kbd)* Unify per-scope classification for layer bindings ([#118](https://github.com/joshuadavidthomas/kbd/pull/118))
- *(kbd)* Extract shared candidate resolution helpers into dispatcher/resolve.rs ([#114](https://github.com/joshuadavidthomas/kbd/pull/114))
- *(kbd)* Extract registration/storage helpers into dispatcher/registry.rs ([#113](https://github.com/joshuadavidthomas/kbd/pull/113))
- *(kbd)* Extract layer stack operations into dispatcher/layers.rs ([#112](https://github.com/joshuadavidthomas/kbd/pull/112))
- *(kbd)* Extract layer timeout and oneshot logic into dispatcher/timeout.rs ([#111](https://github.com/joshuadavidthomas/kbd/pull/111))

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
