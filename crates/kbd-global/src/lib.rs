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
//! use kbd_global::{Hotkey, HotkeyManager, Key, Modifier};
//!
//! let manager = HotkeyManager::new()?;
//!
//! let _handle = manager.register(
//!     Hotkey::new(Key::C).modifier(Modifier::Ctrl).modifier(Modifier::Shift),
//!     || println!("fired"),
//! )?;
//! # Ok::<(), kbd_global::Error>(())
//! ```
//!
//! # Concepts
//!
//! Four concepts cover the library's surface:
//!
//! - **Keys** — physical keys on a keyboard ([`Key`], [`Modifier`], [`Hotkey`])
//! - **Bindings** — "when this pattern matches, do that" ([`Action`], [`BindingOptions`])
//! - **Layers** — named groups of bindings, stackable ([`Layer`], [`LayerOptions`])
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
//! 3. Optionally define and push [`Layer`]s for context-dependent bindings
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
//! use kbd_global::{Backend, HotkeyManager};
//!
//! let manager = HotkeyManager::builder()
//!     .backend(Backend::Evdev)
//!     .build()?;
//! # Ok::<(), kbd_global::Error>(())
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

mod backend;
mod binding_guard;
mod engine;
mod error;
mod manager;

// Re-exports from kbd — all domain types live there.
// kbd-global re-exports them so consumers use a single `kbd_global::` import path.

/// What happens when a binding matches (callback, layer operation, etc.).
pub use kbd::action::Action;
/// Identifier for a named layer.
pub use kbd::action::LayerName;
/// Unique identifier for a registered binding.
pub use kbd::binding::BindingId;
/// Optional metadata and behavior for a binding.
pub use kbd::binding::BindingOptions;
/// Restricts a binding to specific input devices.
pub use kbd::binding::DeviceFilter;
/// Controls whether matched key events are forwarded to the OS.
pub use kbd::binding::KeyPropagation;
/// Controls whether a binding is visible through overlay layers.
pub use kbd::binding::OverlayVisibility;
/// A fully-configured binding ready for registration.
pub use kbd::binding::RegisteredBinding;
/// Core dispatch engine that tracks bindings, layers, and sequences.
pub use kbd::dispatcher::Dispatcher;
/// The outcome of feeding a key event into the dispatcher.
pub use kbd::dispatcher::MatchResult;
/// Introspection snapshot of an active layer.
pub use kbd::introspection::ActiveLayerInfo;
/// Introspection snapshot of a registered binding.
pub use kbd::introspection::BindingInfo;
/// Where a binding lives — global or in a named layer.
pub use kbd::introspection::BindingLocation;
/// A pair of bindings where one shadows the other.
pub use kbd::introspection::ConflictInfo;
/// Whether a binding is currently reachable or shadowed.
pub use kbd::introspection::ShadowedStatus;
/// A key plus modifiers — the pattern to match.
pub use kbd::key::Hotkey;
/// A sequence of hotkeys matched in order.
pub use kbd::key::HotkeySequence;
/// A physical key on the keyboard.
pub use kbd::key::Key;
/// Ctrl, Shift, Alt, or Super.
pub use kbd::key::Modifier;
/// Error returned when parsing a hotkey string like `"Ctrl+A"`.
pub use kbd::key::ParseHotkeyError;
/// Whether a key was pressed or released.
pub use kbd::key_state::KeyTransition;
/// A named group of bindings that can be pushed onto the layer stack.
pub use kbd::layer::Layer;
/// Configuration for layer behavior (unmatched key handling, overlay mode).
pub use kbd::layer::LayerOptions;
/// What happens to key events that don't match any binding in a layer.
pub use kbd::layer::UnmatchedKeys;

/// Which input backend to use.
pub use crate::backend::Backend;
/// RAII guard that keeps a binding alive until dropped.
pub use crate::binding_guard::BindingGuard;
/// Error type for all kbd-global operations.
pub use crate::error::Error;
/// The main entry point — manages the engine thread and hotkey registration.
pub use crate::manager::HotkeyManager;
/// Builder for configuring backend and runtime options before starting.
pub use crate::manager::HotkeyManagerBuilder;
