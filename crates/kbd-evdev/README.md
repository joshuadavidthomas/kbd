# kbd-evdev

[![crates.io](https://img.shields.io/crates/v/kbd-evdev.svg)](https://crates.io/crates/kbd-evdev)
[![docs.rs](https://docs.rs/kbd-evdev/badge.svg)](https://docs.rs/kbd-evdev)

`kbd-evdev` is the low-level Linux input backend for the `kbd` ecosystem. It handles device discovery, hotplug, optional exclusive grabbing, and virtual-device forwarding.

Most applications should start with [`kbd-global`](https://docs.rs/kbd-global), which wraps this crate in a threaded runtime. Use `kbd-evdev` directly when you need explicit control over the device loop.

```toml
[dependencies]
kbd = "0.1"
kbd-evdev = "0.1"
```

## What the crate provides

- `convert` for `evdev::KeyCode` ↔ `kbd::key::Key` conversions
- `devices::DeviceManager` for discovery, hotplug, and polling
- `devices::DeviceGrabMode` for shared vs exclusive access
- `forwarder::UinputForwarder` for forwarding unmatched events in grab mode

## Key conversion

```rust
use evdev::KeyCode;
use kbd::key::Key;
use kbd_evdev::convert::{EvdevKeyCodeExt, KbdKeyExt};

let key: Key = KeyCode::KEY_A.to_key();
assert_eq!(key, Key::A);

let code: KeyCode = Key::A.to_key_code();
assert_eq!(code, KeyCode::KEY_A);
```

## Device management

```rust,no_run
use std::path::Path;
use kbd_evdev::devices::{DeviceGrabMode, DeviceManager};

let manager = DeviceManager::new(Path::new("/dev/input"), DeviceGrabMode::Shared);
let _poll_fds = manager.poll_fds();
```

Call `poll(2)` on `DeviceManager::poll_fds()`, then pass the ready descriptors to `DeviceManager::process_polled_events()` to receive `DeviceKeyEvent` values and disconnection notifications.

## Prerequisites

- Linux only
- Read access to `/dev/input/`
- Write access to `/dev/uinput` if you use grab mode and forwarding

To read input devices without running as root, add your user to the `input` group:

```bash
sudo usermod -aG input $USER
```

Log out and back in for the group change to take effect.

## Documentation

See the [API docs on docs.rs](https://docs.rs/kbd-evdev) for the complete module reference.

## License

kbd-evdev is licensed under the MIT license. See the [`LICENSE`](../../LICENSE) file for more information.
