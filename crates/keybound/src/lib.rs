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
//! use keybound::{Hotkey, HotkeyManager, Key, Modifier};
//!
//! let manager = HotkeyManager::new()?;
//!
//! let _handle = manager.register(
//!     Hotkey::new(Key::C).modifier(Modifier::Ctrl).modifier(Modifier::Shift),
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

mod backend;
mod engine;
mod error;
mod handle;
mod manager;

#[cfg(any(feature = "tokio", feature = "async-std"))]
mod events;

// Re-exports from kbd-core — all domain types live there.
// keybound re-exports them so consumers use a single `keybound::` import path.
pub use kbd_core::action::Action;
pub use kbd_core::action::LayerName;
pub use kbd_core::binding::BindingId;
pub use kbd_core::binding::BindingOptions;
pub use kbd_core::binding::DeviceFilter;
pub use kbd_core::binding::OverlayVisibility;
pub use kbd_core::binding::Passthrough;
pub use kbd_core::binding::RegisteredBinding;
pub use kbd_core::introspection::ActiveLayerInfo;
pub use kbd_core::introspection::BindingInfo;
pub use kbd_core::introspection::BindingLocation;
pub use kbd_core::introspection::ConflictInfo;
pub use kbd_core::introspection::ShadowedStatus;
pub use kbd_core::key::Hotkey;
pub use kbd_core::key::HotkeySequence;
pub use kbd_core::key::Key;
pub use kbd_core::key::Modifier;
pub use kbd_core::key::ParseHotkeyError;
pub use kbd_core::key_state::KeyTransition;
pub use kbd_core::layer::Layer;
pub use kbd_core::layer::LayerOptions;
pub use kbd_core::layer::UnmatchedKeyBehavior;
pub use kbd_core::matcher::MatchResult;
pub use kbd_core::matcher::Matcher;

pub use crate::backend::Backend;
pub use crate::error::Error;
pub use crate::handle::Handle;
pub use crate::manager::HotkeyManager;
pub use crate::manager::HotkeyManagerBuilder;
