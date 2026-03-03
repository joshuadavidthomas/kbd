#![cfg_attr(docsrs, feature(doc_cfg))]

//! XDG `GlobalShortcuts` portal backend for `kbd`.
//!
//! Wayland-friendly global shortcut registration via the XDG
//! `GlobalShortcuts` portal (D-Bus, mediated by `ashpd`). No root access
//! required — works in sandboxed environments (Flatpak, Snap).
//!
//! # Status
//!
//! Not yet implemented. The crate structure and error types exist for
//! interface compatibility.

/// Initialize a portal session for global shortcut registration.
///
/// # Errors
///
/// Always returns [`PortalError::NotImplemented`] — the portal backend
/// is not yet available.
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
