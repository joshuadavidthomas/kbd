# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project attempts to adhere to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

<!--
## [${version}]

_For multi-package releases, list package versions here_

### Added - for new features
### Changed - for changes in existing functionality
### Deprecated - for soon-to-be removed features
### Removed - for now removed features
### Fixed - for any bug fixes
### Security - in case of vulnerabilities

[${version}]: https://github.com/joshuadavidthomas/kbd/releases/tag/${tag}
-->

## [Unreleased]

## [0.1.0](https://github.com/joshuadavidthomas/kbd/releases/tag/kbd-v0.1.0)

### Fixed
- fix API documentation errors

### Other
- update License section in all READMEs
- Add Modifier::collect_active and use across all bridge crates ([#96](https://github.com/joshuadavidthomas/kbd/pull/96))
- *(kbd)* Restructure crate for v0.1.0 publish ([#77](https://github.com/joshuadavidthomas/kbd/pull/77))
- Update docs to reflect API renames and kbd-evdev status ([#63](https://github.com/joshuadavidthomas/kbd/pull/63))
- Pre-release API cleanup for v0.1.0 ([#62](https://github.com/joshuadavidthomas/kbd/pull/62))
- Refresh and rewrite documentation workspace-wide ([#61](https://github.com/joshuadavidthomas/kbd/pull/61))
- rewrite and polish READMEs
- unwrap lines
- add crate specific READMEs and adjust package metadata
- Prepare workspace for 0.1.0 crates.io release ([#60](https://github.com/joshuadavidthomas/kbd/pull/60))

### Added

- **kbd** — Pure-logic keyboard shortcut engine: key types (newtype over `keyboard-types` W3C physical key codes), modifier tracking, hotkey parsing/display with aliases, binding matching, layer stacks (oneshot, swallow, timeout), press cache, and introspection API. No platform dependencies.
- **kbd-crossterm** — crossterm bridge: `KeyCode`/`KeyEvent`/`KeyModifiers` to `kbd` type conversions via extension traits.
- **kbd-egui** — egui bridge: egui key and modifier conversions.
- **kbd-evdev** — Linux evdev backend: device discovery, hotplug via inotify, `EVIOCGRAB` exclusive grab, and uinput virtual device forwarding.
- **kbd-global** — Threaded global hotkey runtime: message-passing architecture, `HotkeyManager` with RAII handles, evdev backend integration, grab mode, and `kbd` type re-exports.
- **kbd-iced** — iced bridge: iced key event and modifier conversions.
- **kbd-tao** — tao bridge: tao (Tauri's winit fork) key event conversions.
- **kbd-winit** — winit bridge: `KeyCode`/`KeyEvent`/`ModifiersState` to `kbd` type conversions.

[0.1.0]: https://github.com/joshuadavidthomas/kbd/releases/tag/v0.1.0
