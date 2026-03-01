# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] - 2026-03-01

Initial release.

### Crates

- **kbd** — Pure-logic keyboard shortcut engine: key types (newtype over
  `keyboard-types` W3C physical key codes), modifier tracking, hotkey
  parsing/display with aliases, binding matching, layer stacks (oneshot,
  swallow, timeout), press cache, and introspection API. No platform
  dependencies.
- **kbd-evdev** — Linux evdev backend: device discovery, hotplug via
  inotify, `EVIOCGRAB` exclusive grab, and uinput virtual device
  forwarding.
- **kbd-global** — Threaded global hotkey runtime: message-passing
  architecture, `HotkeyManager` with RAII handles, evdev backend
  integration, grab mode, and `kbd` type re-exports.
- **kbd-crossterm** — crossterm bridge: `KeyCode`/`KeyEvent`/`KeyModifiers`
  to `kbd` type conversions via extension traits.
- **kbd-winit** — winit bridge: `KeyCode`/`KeyEvent`/`ModifiersState` to
  `kbd` type conversions.
- **kbd-tao** — tao bridge: tao (Tauri's winit fork) key event conversions.
- **kbd-iced** — iced bridge: iced key event and modifier conversions.
- **kbd-egui** — egui bridge: egui key and modifier conversions.

[0.1.0]: https://github.com/joshuadavidthomas/kbd/releases/tag/v0.1.0
