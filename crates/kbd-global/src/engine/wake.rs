//! Eventfd-based wake mechanism for interrupting `poll()`.
//!
//! [`WakeFd`] wraps a Linux `eventfd` descriptor. The manager writes to it
//! after sending a command, causing the engine's `poll()` to return so it
//! can drain the command queue.

use std::io;
use std::mem::size_of;
use std::os::fd::AsRawFd;
use std::os::fd::FromRawFd;
use std::os::fd::OwnedFd;
use std::os::fd::RawFd;

use crate::Error;

pub(crate) struct WakeFd {
    fd: OwnedFd,
}

impl WakeFd {
    pub(crate) fn new() -> Result<Self, Error> {
        // SAFETY: Calling libc `eventfd` with constant flags.
        let raw_fd = unsafe { libc::eventfd(0, libc::EFD_CLOEXEC | libc::EFD_NONBLOCK) };
        if raw_fd < 0 {
            return Err(Error::EngineError);
        }

        // SAFETY: `raw_fd` is an owned descriptor returned by `eventfd`.
        let fd = unsafe { OwnedFd::from_raw_fd(raw_fd) };
        Ok(Self { fd })
    }

    pub(crate) fn wake(&self) -> io::Result<()> {
        let increment = 1_u64;

        loop {
            // SAFETY: `increment` points to an initialized `u64` with the exact
            // byte size required by eventfd writes.
            let result = unsafe {
                libc::write(
                    self.fd.as_raw_fd(),
                    (&raw const increment).cast::<libc::c_void>(),
                    size_of::<u64>(),
                )
            };

            if result == 8 {
                return Ok(());
            }

            if result < 0 {
                let error = io::Error::last_os_error();
                if error.kind() == io::ErrorKind::Interrupted {
                    continue;
                }
                if error.kind() == io::ErrorKind::WouldBlock {
                    return Ok(());
                }
                return Err(error);
            }

            return Err(io::Error::new(
                io::ErrorKind::WriteZero,
                "short write to wake eventfd",
            ));
        }
    }

    pub(crate) fn clear(&self) -> io::Result<()> {
        let mut value = 0_u64;

        loop {
            // SAFETY: `value` points to valid writable memory for a single
            // `u64`, which is the required eventfd read size.
            let result = unsafe {
                libc::read(
                    self.fd.as_raw_fd(),
                    (&raw mut value).cast::<libc::c_void>(),
                    size_of::<u64>(),
                )
            };

            if result == 8 {
                // Non-semaphore eventfd: one read returns the full counter
                // and resets it to 0. No need to read again.
                return Ok(());
            }

            if result < 0 {
                let error = io::Error::last_os_error();
                if error.kind() == io::ErrorKind::Interrupted {
                    continue;
                }
                if error.kind() == io::ErrorKind::WouldBlock {
                    return Ok(());
                }
                return Err(error);
            }

            if result == 0 {
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "wake eventfd closed while clearing",
                ));
            }

            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "short read from wake eventfd",
            ));
        }
    }

    pub(crate) fn raw_fd(&self) -> RawFd {
        self.fd.as_raw_fd()
    }
}
