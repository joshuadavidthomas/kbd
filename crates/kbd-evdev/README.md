# kbd-evdev

Linux evdev backend for [`kbd`](https://crates.io/crates/kbd) — device discovery, hotplug, grab, and event forwarding.

Most users want [`kbd-global`](https://crates.io/crates/kbd-global) instead, which wraps this crate in a threaded runtime with a higher-level API.

## Installation

```toml
[dependencies]
kbd-evdev = "0.1"
```

## What it provides

- Device discovery and keyboard capability detection
- Hotplug via inotify (add/remove devices at runtime)
- `EVIOCGRAB` for exclusive device capture
- `Forwarder` — uinput virtual device for event forwarding/emission
- Extension traits for evdev↔`kbd` key conversions
- Device filtering and self-detection

## License

MIT
