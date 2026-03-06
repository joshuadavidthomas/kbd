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
//! use kbd::prelude::*;
//! use kbd_global::manager::HotkeyManager;
//!
//! let manager = HotkeyManager::new()?;
//!
//! let _guard = manager.register(
//!     Hotkey::new(Key::C).modifier(Modifier::Ctrl).modifier(Modifier::Shift),
//!     || println!("fired"),
//! )?;
//! # Ok::<(), kbd_global::error::Error>(())
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
//! [`manager::HotkeyManager`] is the public API. Internally it sends commands to a
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
//! 1. Create a manager with [`manager::HotkeyManager::new()`] or [`manager::HotkeyManager::builder()`]
//! 2. Register hotkeys with [`manager::HotkeyManager::register()`] — returns a [`binding_guard::BindingGuard`]
//! 3. Optionally define and push [`Layer`](kbd::layer::Layer)s for context-dependent bindings
//! 4. The engine thread processes key events and fires callbacks
//! 5. Drop the [`binding_guard::BindingGuard`] to unregister, or call [`binding_guard::BindingGuard::unregister()`]
//! 6. Drop the manager (or call [`manager::HotkeyManager::shutdown()`]) to stop
//!
//! # Backend selection
//!
//! Currently only [`backend::Backend::Evdev`] is available — it reads `/dev/input/event*`
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
//! # Ok::<(), kbd_global::error::Error>(())
//! ```
//!
//! # Event stream
//!
//! For callback-free consumers, subscribe to [`manager::HotkeyManager::event_stream()`]
//! and react to [`events::HotkeyEvent`] values in your own loop:
//!
//! ```rust,no_run
//! use kbd::prelude::*;
//! use kbd_global::events::HotkeyEvent;
//! use kbd_global::manager::HotkeyManager;
//!
//! let manager = HotkeyManager::new()?;
//! let stream = manager.event_stream()?;
//! let _guard = manager.register_sequence("Ctrl+K, Ctrl+C", || {})?;
//!
//! while let Ok(event) = stream.recv_blocking() {
//!     match event {
//!         HotkeyEvent::SequenceStep {
//!             steps_matched,
//!             steps_remaining,
//!             ..
//!         } => {
//!             println!("sequence progress: {steps_matched} matched, {steps_remaining} remaining");
//!         }
//!         _ => {}
//!     }
//! }
//! # Ok::<(), kbd_global::error::Error>(())
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

pub mod backend;
pub mod binding_guard;
mod engine;
pub mod error;
pub mod events;
pub mod manager;
