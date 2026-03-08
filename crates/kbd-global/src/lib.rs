#![cfg_attr(docsrs, feature(doc_cfg))]

//! Global hotkey runtime for `kbd`.
//!
//! Threaded engine, device management, and backend selection for Linux.
//! The library handles platform complexity — evdev, portal, permissions,
//! hotplug, virtual devices — so callers describe what patterns they care
//! about and what should happen.
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
//! # Concepts
//!
//! Four concepts cover the library's surface:
//!
//! - **Keys** — physical keys on a keyboard ([`Key`](kbd::key::Key), [`Modifier`](kbd::hotkey::Modifier), [`Hotkey`](kbd::hotkey::Hotkey))
//! - **Bindings** — "when this pattern matches, do that" ([`Action`](kbd::action::Action), [`BindingOptions`](kbd::binding::BindingOptions))
//! - **Layers** — named groups of bindings, stackable ([`Layer`](kbd::layer::Layer), [`LayerOptions`](kbd::layer::LayerOptions))
//! - **Grab mode** — exclusive device capture for interception and remapping
//!
//! # Architecture
//!
//! [`HotkeyManager`] is the public API. Internally it sends commands to a
//! dedicated engine thread over an `mpsc` channel, with an `eventfd` wake
//! mechanism to interrupt `poll()`. All mutable state lives in the engine —
//! no locks, no shared mutation.
//!
//! ```text
//! ┌──────────────────┐    Command     ┌──────────────────┐
//! │  HotkeyManager   │ ─────────────► │  Engine thread   │
//! │  (command sender) │ ◄───────────── │  (event loop)    │
//! └──────────────────┘    Reply        └──────────────────┘
//!                                           │
//!                                      poll(devices + wake_fd)
//! ```
//!
//! # Lifecycle
//!
//! 1. Create a manager with [`HotkeyManager::new()`] or [`HotkeyManager::builder()`]
//! 2. Register hotkeys with [`HotkeyManager::register()`] — returns a [`BindingGuard`]
//! 3. Optionally define and push [`Layer`](kbd::layer::Layer)s for context-dependent bindings
//! 4. The engine thread processes key events and fires callbacks
//! 5. Drop the [`BindingGuard`] to unregister, or call [`BindingGuard::unregister()`]
//! 6. Drop the manager (or call [`HotkeyManager::shutdown()`]) to stop
//!
//! # Backend selection
//!
//! Currently only [`Backend::Evdev`] is available — it reads `/dev/input/event*`
//! directly and works on Wayland, X11, and TTY. Your user must be in the
//! `input` group:
//!
//! ```bash
//! sudo usermod -aG input $USER
//! ```
//!
//! Use the builder for explicit backend selection:
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
//! | `grab` | Enables exclusive device capture via `EVIOCGRAB` with uinput forwarding for non-hotkey events |
//! | `serde` | Adds `Serialize`/`Deserialize` to key and hotkey types (via [`kbd`]) |
//!
//! # See also
//!
//! - [`kbd`] — core dispatch engine, key types, and layer logic

/// Which input backend to use.
pub mod backend;
/// RAII guard that keeps a binding alive until dropped.
pub mod binding_guard;
mod engine;
/// Error types for the global hotkey manager.
pub mod error;
/// The main entry point — manages the engine thread and hotkey registration.
pub mod manager;
