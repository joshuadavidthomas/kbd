#![cfg_attr(docsrs, feature(doc_cfg))]

//! Global hotkey runtime for `kbd` on Linux.
//!
//! `kbd-global` wraps the pure matching engine from [`kbd`] in a threaded
//! runtime that owns device discovery, hotplug handling, and command-based
//! registration APIs. It reads from evdev devices directly, so it works
//! on Wayland, X11, and TTY without display-server integration.
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
//! [`HotkeyManager`](manager::HotkeyManager) is the main entry point.
//! Registration returns a [`BindingGuard`](binding_guard::BindingGuard)
//! that keeps the binding alive — drop it to unregister. Key types,
//! actions, and layer definitions come from [`kbd`].
//!
//! # Layers
//!
//! Layers let you swap between different binding sets at runtime:
//!
//! ```rust,no_run
//! use kbd::action::Action;
//! use kbd::key::Key;
//! use kbd::layer::Layer;
//! use kbd_global::manager::HotkeyManager;
//!
//! let manager = HotkeyManager::new()?;
//!
//! let layer = Layer::new("vim-normal")
//!     .bind(Key::J, || println!("down"))?
//!     .bind(Key::K, || println!("up"))?;
//!
//! manager.define_layer(layer)?;
//! manager.push_layer("vim-normal")?;
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! Layers stack — the most recently pushed layer is checked first.
//! See [`kbd::layer`] for oneshot, swallow, and timeout options.
//!
//! # Architecture
//!
//! [`HotkeyManager`](manager::HotkeyManager) sends typed commands to a
//! dedicated engine thread over a channel, using an `eventfd` wake
//! mechanism to interrupt `poll()`. All mutable runtime state lives in
//! the engine thread.
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
//! bindings, and keep the returned guards alive. Dropping a guard unregisters
//! its binding; dropping the manager stops the runtime.
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
//! | `grab` | Exclusive device capture via `EVIOCGRAB` with uinput forwarding for unmatched events |
//! | `serde` | Adds `Serialize`/`Deserialize` to shared key and hotkey types via [`kbd`] |
//!
//! # Current limitations
//!
//! - Linux only
//! - evdev is the only backend
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
