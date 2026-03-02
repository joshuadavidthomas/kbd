#![cfg_attr(docsrs, feature(doc_auto_cfg))]

//! Global hotkey runtime for kbd — threaded engine, device management,
//! and backend selection for Linux.
//!
//! When a specific pattern of keys happens on a Linux input device, do
//! something. The library handles platform complexity — evdev, portal,
//! permissions, hotplug, virtual devices — so you just describe what
//! patterns you care about and what should happen.
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

// Re-exports from kbd — all domain types live there.
// kbd-global re-exports them so consumers use a single `kbd_global::` import path.
pub use kbd::action::Action;
pub use kbd::action::LayerName;
pub use kbd::binding::BindingId;
pub use kbd::binding::BindingOptions;
pub use kbd::binding::DeviceFilter;
pub use kbd::binding::OverlayVisibility;
pub use kbd::binding::Passthrough;
pub use kbd::binding::RegisteredBinding;
pub use kbd::introspection::ActiveLayerInfo;
pub use kbd::introspection::BindingInfo;
pub use kbd::introspection::BindingLocation;
pub use kbd::introspection::ConflictInfo;
pub use kbd::introspection::ShadowedStatus;
pub use kbd::key::Hotkey;
pub use kbd::key::HotkeySequence;
pub use kbd::key::Key;
pub use kbd::key::Modifier;
pub use kbd::key::ParseHotkeyError;
pub use kbd::key_state::KeyTransition;
pub use kbd::layer::Layer;
pub use kbd::layer::LayerOptions;
pub use kbd::layer::UnmatchedKeyBehavior;
pub use kbd::matcher::MatchResult;
pub use kbd::matcher::Matcher;

pub use crate::backend::Backend;
pub use crate::error::Error;
pub use crate::handle::Handle;
pub use crate::manager::HotkeyManager;
pub use crate::manager::HotkeyManagerBuilder;
