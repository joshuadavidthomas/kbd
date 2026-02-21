//! Global hotkey library for Linux — works on Wayland, X11, and TTY.
//!
//! `keybound` lets you register system-wide keyboard shortcuts from any Linux
//! environment. It auto-selects the best available backend:
//!
//! - **XDG `GlobalShortcuts` portal** — works without root on compositors that
//!   support it (KDE Plasma, GNOME, Hyprland). Enabled with the `portal`
//!   feature.
//! - **evdev** — reads directly from `/dev/input/event*` devices. Works
//!   everywhere on Linux but requires `input` group membership. Enabled by
//!   default.
//!
//! # Quick start
//!
//! ```rust,no_run
//! use keybound::{HotkeyManager, Key, Modifier};
//!
//! let manager = HotkeyManager::new()?;
//!
//! let _handle = manager.register(
//!     Key::C,
//!     &[Modifier::Ctrl, Modifier::Shift],
//!     || println!("Ctrl+Shift+C pressed!"),
//! )?;
//!
//! // Keep program running to receive hotkey events
//! std::thread::park();
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! The returned [`Handle`] keeps the hotkey alive — drop it or call
//! [`Handle::unregister`] to remove the binding.
//!
//! # Key concepts
//!
//! | Type | Purpose |
//! |------|---------|
//! | [`HotkeyManager`] | Central entry point — registers hotkeys, sequences, modes, and tap-hold keys |
//! | [`Key`] / [`Modifier`] | Platform-independent key and modifier enums |
//! | [`Hotkey`] | A single key + modifier combination, parseable from strings (`"Ctrl+Shift+A"`) |
//! | [`HotkeySequence`] | Multi-step combo like `"Ctrl+K, Ctrl+C"` |
//! | [`HotkeyOptions`] | Per-hotkey options: release callbacks, hold thresholds, debounce, rate limiting |
//! | [`Handle`] | RAII guard for a registered hotkey |
//! | [`ModeController`] | Stack-based mode activation from inside callbacks |
//! | [`Backend`] | Explicit backend selection (usually auto-detected) |
//!
//! # Press, release, and hold
//!
//! Use [`HotkeyManager::register_with_options`] for finer control:
//!
//! ```rust,no_run
//! use std::time::Duration;
//! use keybound::{HotkeyManager, HotkeyOptions, Key, Modifier};
//!
//! let manager = HotkeyManager::new()?;
//!
//! let _handle = manager.register_with_options(
//!     Key::F1,
//!     &[Modifier::Ctrl],
//!     HotkeyOptions::new()
//!         .on_release_callback(|| println!("Released"))
//!         .min_hold(Duration::from_millis(500))
//!         .debounce(Duration::from_millis(100)),
//!     || println!("Pressed (after min hold)"),
//! )?;
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! # Key sequences
//!
//! Register multi-step combos with a configurable timeout between steps:
//!
//! ```rust,no_run
//! use keybound::{HotkeyManager, HotkeySequence, Key, Modifier};
//!
//! let manager = HotkeyManager::new()?;
//!
//! let seq = "Ctrl+K, Ctrl+C".parse::<HotkeySequence>()?;
//! let _handle = manager.register_sequence(&seq, Default::default(), || {
//!     println!("Sequence completed!");
//! })?;
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! # Modes / layers
//!
//! Named groups of hotkeys with stack-based activation. Useful for modal
//! interfaces (vim-style, resize mode, etc.):
//!
//! ```rust,no_run
//! use keybound::{HotkeyManager, Key, Modifier, ModeOptions};
//!
//! let manager = HotkeyManager::new()?;
//! let ctrl = manager.mode_controller();
//!
//! manager.define_mode("resize", ModeOptions::new(), |mode| {
//!     mode.register(Key::H, &[], || println!("shrink left"))?;
//!     let ctrl = mode.mode_controller();
//!     mode.register(Key::Escape, &[], move || { ctrl.pop(); })?;
//!     Ok(())
//! })?;
//!
//! // Push the mode from a global hotkey
//! manager.register(Key::R, &[Modifier::Super], move || {
//!     ctrl.push("resize");
//! })?;
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! # Event grabbing
//!
//! With the `grab` feature, the evdev backend exclusively captures keyboard
//! input and re-emits non-hotkey keys via uinput. This prevents other
//! applications from seeing consumed hotkeys.
//!
//! ```rust,no_run
//! use keybound::{HotkeyManager, Key, Modifier};
//!
//! let manager = HotkeyManager::builder().grab().build()?;
//!
//! // This hotkey is consumed — other apps won't see Super+L
//! let _handle = manager.register(Key::L, &[Modifier::Super], || {
//!     println!("Lock screen");
//! })?;
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! Grab mode requires `/dev/uinput` access. See the repository README for
//! udev setup instructions.
//!
//! # Tap-hold / dual-function keys
//!
//! Requires the `grab` feature. A key performs different actions depending on
//! whether it's tapped or held:
//!
//! ```rust,no_run
//! use std::time::Duration;
//! use keybound::{HoldAction, HotkeyManager, Key, Modifier, TapAction, TapHoldOptions};
//!
//! let manager = HotkeyManager::builder().grab().build()?;
//!
//! // CapsLock: tap → Escape, hold → Ctrl
//! let _handle = manager.register_tap_hold(
//!     Key::CapsLock,
//!     TapAction::emit(Key::Escape),
//!     HoldAction::modifier(Modifier::Ctrl),
//!     TapHoldOptions::new().threshold(Duration::from_millis(200)),
//! )?;
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! # String parsing
//!
//! [`Hotkey`] and [`HotkeySequence`] implement [`FromStr`](std::str::FromStr)
//! and [`Display`](std::fmt::Display) for round-trip conversion:
//!
//! ```
//! use keybound::{Hotkey, HotkeySequence};
//!
//! let hotkey: Hotkey = "Ctrl+Shift+A".parse().unwrap();
//! assert_eq!(hotkey.to_string(), "Ctrl+Shift+A");
//!
//! let seq: HotkeySequence = "Ctrl+K, Ctrl+C".parse().unwrap();
//! assert_eq!(seq.to_string(), "Ctrl+K, Ctrl+C");
//! ```
//!
//! # Config serialization
//!
//! With the `serde` feature, load hotkey definitions from TOML, JSON, or YAML:
//!
//! ```rust,ignore
//! use keybound::{ActionId, ActionMap, HotkeyConfig, HotkeyManager};
//!
//! let manager = HotkeyManager::new()?;
//!
//! let config: HotkeyConfig = toml::from_str(r#"
//! [[hotkeys]]
//! hotkey = "Ctrl+Shift+A"
//! action = "launch-terminal"
//! "#)?;
//!
//! let mut actions = ActionMap::new();
//! actions.insert("launch-terminal".parse::<ActionId>()?, || {
//!     println!("Launching terminal");
//! })?;
//!
//! let _registered = config.register(&manager, &actions)?;
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! # Async event streams
//!
//! With the `tokio` or `async-std` feature, subscribe to a
//! [`HotkeyEventStream`] for async notification of hotkey presses, releases,
//! sequence progress, and mode changes.
//!
//! # Feature flags
//!
//! | Feature | Default | Description |
//! |---------|---------|-------------|
//! | `evdev` | ✓ | evdev backend (reads `/dev/input/event*`) |
//! | `portal` | | XDG `GlobalShortcuts` portal backend |
//! | `grab` | | Exclusive key capture via `EVIOCGRAB` + uinput re-emission |
//! | `tokio` | | [`HotkeyEventStream`] via tokio |
//! | `async-std` | | [`HotkeyEventStream`] via async-std |
//! | `serde` | | Config deserialization types ([`HotkeyConfig`], [`ActionMap`], etc.) |
//!
//! # Permissions
//!
//! The **evdev backend** requires read access to `/dev/input/event*` devices —
//! typically via `input` group membership. **Grab mode** additionally requires
//! write access to `/dev/uinput`. The **portal backend** does not require
//! special permissions.
//!
//! See the repository README for detailed setup instructions.

pub use backend::Backend;
#[cfg(feature = "serde")]
pub use config::ActionId;
#[cfg(feature = "serde")]
pub use config::ActionIdError;
#[cfg(feature = "serde")]
pub use config::ActionMap;
#[cfg(feature = "serde")]
pub use config::ActionMapError;
#[cfg(feature = "serde")]
pub use config::ConfigRegistrationError;
#[cfg(feature = "serde")]
pub use config::HotkeyBinding;
#[cfg(feature = "serde")]
pub use config::HotkeyConfig;
#[cfg(feature = "serde")]
pub use config::ModeBindings;
#[cfg(feature = "serde")]
pub use config::RegisteredConfig;
#[cfg(feature = "serde")]
pub use config::SequenceBinding;
pub use device::DeviceFilter;
pub use error::Error;
pub use events::HotkeyEvent;
#[cfg(any(feature = "tokio", feature = "async-std"))]
pub use events::HotkeyEventStream;
pub use hotkey::Hotkey;
pub use hotkey::HotkeySequence;
pub use hotkey::ParseHotkeyError;
pub use key::Key;
pub use key::Modifier;
pub use manager::Handle;
pub use manager::HotkeyManager;
pub use manager::HotkeyManagerBuilder;
pub use manager::HotkeyOptions;
pub use manager::SequenceHandle;
pub use manager::SequenceOptions;
pub use manager::TapHoldHandle;
pub use mode::ModeBuilder;
pub use mode::ModeController;
pub use mode::ModeOptions;
pub use tap_hold::HoldAction;
pub use tap_hold::TapAction;
pub use tap_hold::TapHoldOptions;

mod backend;
#[cfg(feature = "serde")]
mod config;
mod device;
mod error;
mod events;
mod hotkey;
mod key;
mod key_state;
mod listener;
mod manager;
mod mode;
mod tap_hold;
