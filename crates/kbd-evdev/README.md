# kbd-evdev

[![crates.io](https://img.shields.io/crates/v/kbd-evdev.svg)](https://crates.io/crates/kbd-evdev)
[![docs.rs](https://docs.rs/kbd-evdev/badge.svg)](https://docs.rs/kbd-evdev)

Linux evdev backend for [`kbd`](https://crates.io/crates/kbd) — device discovery, hotplug, grab, and event forwarding. Most users want [`kbd-global`](https://crates.io/crates/kbd-global), which wraps this in a threaded runtime.

```toml
[dependencies]
kbd-evdev = "0.1"
```

## License

MIT
