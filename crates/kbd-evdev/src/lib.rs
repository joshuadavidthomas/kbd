//! evdev backend for keybound.
//!
//! This crate provides the Linux input device layer:
//!
//! - Device discovery and keyboard capability detection
//! - Hotplug via inotify (add/remove devices at runtime)
//! - `EVIOCGRAB` for exclusive device capture
//! - `Forwarder` — uinput virtual device for event forwarding/emission
//! - `From<evdev::KeyCode> for Key` and reverse — the evdev↔core bridge
//! - Device filtering and self-detection
//!
//! # Dependencies
//!
//! Depends on `evdev` (Linux C library, needs `/dev/input/` access) and `kbd-core`.

// TODO: Phase 3.8 — move engine/devices.rs here
// TODO: Phase 3.8 — move engine/forwarder.rs here
// TODO: Phase 3.8 — move From<evdev::KeyCode> / Into<evdev::KeyCode> impls here
