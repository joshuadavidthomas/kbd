#![cfg_attr(docsrs, feature(doc_cfg))]

//! Global hotkey runtime for `kbd` on Linux.
//!
//! `kbd-global` wraps the pure matching engine from [`kbd`] in a threaded
//! runtime that owns device discovery, hotplug handling, and command-based
//! registration APIs. The current implementation uses the evdev backend
//! directly, so it works on Wayland, X11, and TTY without display-server
//! specific integrations.
//!
//! # Quick start
//!
//! ```rust,no_run
//! use kbd::hotkey::{Hotkey, Modifier};
//! use kbd::key::Key;
//! use kbd_global::manager::HotkeyManager;
//!
//! let manager = HotkeyManager::new()?;
//!
//! let _guard = manager.register(
//!     Hotkey::new(Key::C).modifier(Modifier::Ctrl).modifier(Modifier::Shift),
//!     || println!("fired"),
//! )?;
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! Most application code goes through [`HotkeyManager`](manager::HotkeyManager)
//! and the returned [`BindingGuard`](binding_guard::BindingGuard). Key types,
//! actions, and layer definitions still come from [`kbd`].
//!
//! # Architecture
//!
//! [`HotkeyManager`](manager::HotkeyManager) is the public API. Internally it
//! sends typed commands to a dedicated engine thread over a channel, using an
//! `eventfd` wake mechanism to interrupt `poll()`. All mutable runtime state
//! lives in the engine thread.
//!
//! ```text
//! +------------------+                +------------------+
//! |  HotkeyManager   | -- commands -->|  Engine thread   |
//! | (command sender) |<--- replies ---|   (event loop)   |
//! +------------------+                +------------------+
//!                                              |
//!                                              v
//!                                   poll(devices + wake_fd)
//! ```
//!
//! Create a manager with [`HotkeyManager::new()`](manager::HotkeyManager::new)
//! or [`HotkeyManager::builder()`](manager::HotkeyManager::builder), register
//! bindings and optional layers, and keep the returned
//! [`BindingGuard`](binding_guard::BindingGuard)s alive for as long as the
//! bindings should remain active. Dropping a guard unregisters its binding;
//! dropping the manager, or calling [`HotkeyManager::shutdown()`](manager::HotkeyManager::shutdown), stops the runtime.
//!
//! # Backend selection
//!
//! Currently only [`Backend::Evdev`](backend::Backend::Evdev) is available.
//! It reads `/dev/input/event*` directly and requires permission to access
//! Linux input devices:
//!
//! ```bash
//! sudo usermod -aG input $USER
//! ```
//!
//! Use the builder for explicit backend selection or grab mode:
//!
//! ```rust,no_run
//! use kbd_global::backend::Backend;
//! use kbd_global::manager::HotkeyManager;
//!
//! let manager = HotkeyManager::builder()
//!     .backend(Backend::Evdev)
//!     .build()?;
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! # Feature flags
//!
//! | Feature | Effect |
//! |---------|--------|
//! | `grab` | Enables exclusive device capture via `EVIOCGRAB` with uinput forwarding for unmatched events |
//! | `serde` | Adds `Serialize`/`Deserialize` to shared key and hotkey types via [`kbd`] |
//!
//! # Current limitations
//!
//! - Linux only
//! - evdev is the only backend currently available
//! - [`Action::EmitHotkey`](kbd::action::Action::EmitHotkey) and [`Action::EmitSequence`](kbd::action::Action::EmitSequence) are not yet implemented in the runtime
//!
//! # See also
//!
//! - [`kbd`] — core dispatch engine, key types, and layer logic
//! - [`kbd-evdev`](https://docs.rs/kbd-evdev) — low-level device backend used by this crate

pub mod backend;
pub mod binding_guard;
mod engine;
pub mod error;
pub mod manager;
