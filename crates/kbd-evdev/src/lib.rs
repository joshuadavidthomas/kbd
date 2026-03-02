#![cfg_attr(docsrs, feature(doc_auto_cfg))]

//! evdev backend for kbd.
//!
//! This crate provides the Linux input device layer:
//!
//! - Device discovery and keyboard capability detection
//! - Hotplug via inotify (add/remove devices at runtime)
//! - `EVIOCGRAB` for exclusive device capture
//! - `Forwarder` — uinput virtual device for event forwarding/emission
//! - Extension traits for evdev↔core key conversions
//! - Device filtering and self-detection
//!
//! # Dependencies
//!
//! Depends on `evdev` (Linux C library, needs `/dev/input/` access) and `kbd`.

pub mod convert;
pub mod devices;
pub mod error;
pub mod forwarder;

pub use crate::convert::EvdevKeyExt;
pub use crate::convert::KeyCodeExt;
