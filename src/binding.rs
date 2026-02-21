//! The unified [`Binding`] type — pattern + action + options.
//!
//! A binding is the core unit: "when this input pattern matches, do this
//! action." Replaces the four separate registration types from v0:
//!
//! | v0 type                      | Now expressed as              |
//! |------------------------------|-------------------------------|
//! | `HotkeyRegistration`         | Binding with Hotkey pattern   |
//! | `SequenceRegistration`       | Binding with Sequence pattern |
//! | `DeviceHotkeyRegistration`   | Binding with device filter    |
//! | `TapHoldRegistration`        | Binding with TapHold pattern  |
//!
//! One storage structure. One dispatch path. One handle type.
//!
//! # Reference
//!
//! Prior art: `archive/v0/src/manager/registration.rs`,
//! `archive/v0/src/listener/dispatch.rs`

// TODO: Binding struct (pattern + action + options)
// TODO: BindingOptions (device filter, passthrough, debounce, min hold, press/release)
// TODO: DeviceFilter enum (name pattern, USB vendor/product ID, device path)
// TODO: BindingId newtype for unique identification

/// Placeholder — see module docs.
pub struct BindingOptions;

/// Placeholder — see module docs.
#[cfg(feature = "evdev")]
pub enum DeviceFilter {}
