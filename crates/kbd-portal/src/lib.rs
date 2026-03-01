//! XDG `GlobalShortcuts` portal backend for kbd.
//!
//! This crate provides Wayland-friendly global shortcut registration via the
//! XDG `GlobalShortcuts` portal (D-Bus, mediated by `ashpd`). No root access
//! required — works in sandboxed environments (Flatpak, Snap).
//!
//! # Status
//!
//! Stub implementation. Entry points exist for interface compatibility but are
//! not yet functional. Full implementation is planned for Phase 4.5.
//!
//! # Dependencies
//!
//! Depends on `ashpd` (async D-Bus client) and `kbd`. Pulls in an async
//! runtime — isolated here so it doesn't infect the rest of the workspace.

/// Initialize a portal session for global shortcut registration.
///
/// # Errors
///
/// Returns an error because the portal backend is not yet implemented.
pub fn init_session() -> Result<(), PortalError> {
    Err(PortalError::NotImplemented)
}

/// Portal backend error type.
#[derive(Debug, thiserror::Error)]
pub enum PortalError {
    /// The portal backend is not yet implemented.
    #[error("XDG GlobalShortcuts portal backend is not yet implemented (planned for Phase 4.5)")]
    NotImplemented,
}
