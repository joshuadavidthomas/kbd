#![cfg_attr(docsrs, feature(doc_cfg))]

//! Linux evdev backend for `kbd`.
//!
//! This crate provides the low-level Linux input layer that
//! [`kbd-global`](https://docs.rs/kbd-global) builds on. It handles:
//!
//! - **Device discovery** — scan [`/dev/input/`](devices::INPUT_DIRECTORY) for
//!   keyboards (devices supporting A–Z + Enter)
//! - **Hotplug** — inotify watch for device add/remove at runtime
//! - **Exclusive grab** — [`EVIOCGRAB`](devices::DeviceGrabMode::Exclusive) for
//!   intercepting events before other applications see them
//! - **Event forwarding** — [`UinputForwarder`](forwarder::UinputForwarder)
//!   re-emits unmatched events through a virtual device so they still reach
//!   applications in grab mode
//! - **Key conversion** — extension traits ([`EvdevKeyCodeExt`], [`KbdKeyExt`])
//!   for converting between `evdev::KeyCode` and [`kbd::key::Key`]
//!
//! # Prerequisites
//!
//! - **Linux only** — this crate uses `/dev/input/`, `inotify`, and `/dev/uinput`.
//! - **Read access to `/dev/input/`** — either run as root, or add your user to
//!   the `input` group:
//!
//!   ```sh
//!   sudo usermod -aG input $USER
//!   # log out and back in for the group change to take effect
//!   ```
//!
//! - **Write access to `/dev/uinput`** (only for grab mode) — needed to create
//!   the virtual device that forwards unmatched events.
//!
//! # Architecture
//!
//! ```text
//! /dev/input/event*          DeviceManager
//!   ├─ event0  ──┐       ┌─ discover + poll ──→ DeviceKeyEvent
//!   ├─ event1  ──┼──────→│  hotplug (inotify)   │
//!   └─ event2  ──┘       └───────────────────────┘
//!                                                │
//!                                      EvdevKeyCodeExt::to_key()
//!                                                │
//!                                                ▼
//!                                           kbd::Key
//!                                                │
//!                              ┌─────────────────┼─────────────────┐
//!                              │                 │                 │
//!                           Dispatcher          KeyState     UinputForwarder
//!                        (kbd core)        (kbd core)     (grab mode only)
//! ```
//!
//! # Usage
//!
//! Most users should use [`kbd-global`](https://docs.rs/kbd-global) which wraps
//! this crate in a threaded runtime. Use `kbd-evdev` directly when you need
//! low-level control over the poll loop.
//!
//! # See also
//!
//! - [`kbd`](https://docs.rs/kbd) — core key types, matching, and layers
//! - [`kbd-global`](https://docs.rs/kbd-global) — threaded runtime built on
//!   this crate

/// Extension traits for converting between `evdev::KeyCode` and [`kbd::key::Key`].
pub mod convert;
/// Device discovery, hotplug monitoring, and event polling.
pub mod devices;
/// Error types for the evdev backend.
pub mod error;
/// Virtual uinput device for forwarding and emitting key events.
pub mod forwarder;

/// Convert an [`evdev::KeyCode`] to a [`kbd::key::Key`].
pub use crate::convert::EvdevKeyCodeExt;
/// Convert a [`kbd::key::Key`] to an [`evdev::KeyCode`].
pub use crate::convert::KbdKeyExt;
