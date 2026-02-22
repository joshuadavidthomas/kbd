//! Global hotkey library for Linux — works on Wayland, X11, and TTY.
//!
//! When a specific pattern of keys happens on a Linux input device, do
//! something. The library handles platform complexity — evdev, portal,
//! permissions, hotplug, virtual devices — so you just describe what
//! patterns you care about and what should happen.
//!
//! # Quick start
//!
//! ```rust,no_run
//! use keybound::{HotkeyManager, Key, Modifier};
//!
//! let manager = HotkeyManager::new()?;
//!
//! let _handle = manager.register(
//!     Key::C, &[Modifier::Ctrl, Modifier::Shift],
//!     || println!("fired"),
//! )?;
//! # Ok::<(), keybound::Error>(())
//! ```
//!
//! # Concepts
//!
//! Four ideas cover everything this library does:
//!
//! - **Keys** — physical keys on a keyboard ([`Key`], [`Modifier`], [`Hotkey`])
//! - **Bindings** — "when this pattern matches, do that" ([`Action`], [`BindingOptions`])
//! - **Layers** — named groups of bindings, stackable ([`Layer`], [`LayerOptions`])
//! - **Grab mode** — exclusive device capture for interception and remapping

mod action;
mod backend;
mod binding;
#[allow(dead_code)]
mod engine;
mod error;
mod handle;
mod key;
mod layer;
mod manager;

#[cfg(any(feature = "tokio", feature = "async-std"))]
mod events;

// Public API surface — curated re-exports only.
// Keep this small. `pub(crate)` for internal sharing.

pub use crate::action::Action;
pub use crate::action::LayerName;
pub use crate::backend::Backend;
pub use crate::binding::BindingId;
pub use crate::binding::BindingOptions;
// Re-export device filter when evdev is available.
#[cfg(feature = "evdev")]
pub use crate::binding::DeviceFilter;
pub use crate::binding::Passthrough;
pub use crate::error::Error;
// #[cfg(any(feature = "tokio", feature = "async-std"))]
// pub use crate::events::HotkeyEvent;
// #[cfg(any(feature = "tokio", feature = "async-std"))]
// pub use crate::events::HotkeyEventStream;
pub use crate::handle::Handle;
pub use crate::key::Hotkey;
pub use crate::key::HotkeySequence;
pub use crate::key::Key;
pub use crate::key::Modifier;
pub use crate::key::ParseHotkeyError;
pub use crate::layer::Layer;
pub use crate::layer::LayerOptions;
pub use crate::manager::HotkeyManager;
pub use crate::manager::HotkeyManagerBuilder;
