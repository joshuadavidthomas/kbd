#![cfg_attr(docsrs, feature(doc_cfg))]

//! Planned XDG `GlobalShortcuts` portal backend for `kbd`.
//!
//! The intended goal is a Wayland-friendly, sandbox-friendly global
//! shortcut backend built on the desktop portal and mediated by `ashpd`.
//! This would complement the direct-evdev approach used by
//! [`kbd-global`](https://docs.rs/kbd-global) in environments where direct
//! device access is unavailable or undesirable.
//!
//! # Status
//!
//! The backend is not implemented yet. The crate currently exposes only a
//! placeholder entry point and error type.

/// Initialize a portal session for global shortcut registration.
///
/// # Errors
///
/// Always returns [`PortalError::NotImplemented`]. The portal backend has
/// not been implemented yet.
pub fn init_session() -> Result<(), PortalError> {
    Err(PortalError::NotImplemented)
}

/// Portal backend error type.
#[derive(Debug, thiserror::Error)]
pub enum PortalError {
    /// The portal backend is not yet implemented.
    #[error("XDG GlobalShortcuts portal backend is not yet implemented")]
    NotImplemented,
}
