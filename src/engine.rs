//! The engine — owns all mutable state, runs the event loop.
//!
//! # Architecture
//!
//! The engine runs in a dedicated thread. It owns:
//! - All registered bindings
//! - The layer stack
//! - Key state (what's currently pressed)
//! - Sequence and tap-hold state machines
//! - The press cache (for correct releases across layer transitions)
//! - Device handles and the uinput forwarder
//!
//! No shared mutable state. The manager communicates via a command channel.
//! An eventfd (or pipe) wakes the engine's `poll()` when commands arrive.
//!
//! # Event loop
//!
//! ```text
//! loop {
//!     poll(device_fds + wake_fd, timeout)
//!     drain_commands()        // process register/unregister/layer ops
//!     process_key_events()    // for each ready device
//!     check_timers()          // sequence timeouts, tap-hold thresholds
//! }
//! ```
//!
//! # Modules
//!
//! - [`key_state`] — tracks what's currently pressed, derives modifier state
//! - [`matcher`] — finds matching bindings for a key event
//! - [`sequence`] — sequence pattern state machine
//! - [`tap_hold`] — tap-hold pattern state machine
//! - [`devices`] — device discovery, hotplug, capability detection
//! - [`forwarder`] — uinput virtual device for event forwarding/emission
//!
//! # Reference
//!
//! Prior art: `archive/v0/src/listener.rs` (357-line listener_loop),
//! `archive/v0/src/listener/` (dispatch, io, sequence, hotplug, forwarding, state).
//! The engine replaces all of this.

pub(crate) mod key_state;
pub(crate) mod matcher;
pub(crate) mod sequence;
pub(crate) mod tap_hold;
pub(crate) mod devices;

#[cfg(feature = "grab")]
pub(crate) mod forwarder;

// TODO: Engine struct — all the owned state listed above
// TODO: Command enum — Register, Unregister, DefineLayer, PushLayer,
//       PopLayer, QueryKeyState, Shutdown
// TODO: engine::run() — the event loop
// TODO: Wake mechanism (eventfd or pipe) for command notification
// TODO: Reply channels for fallible commands (oneshot sender)
