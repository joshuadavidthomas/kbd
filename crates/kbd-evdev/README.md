# kbd-evdev

[![crates.io](https://img.shields.io/crates/v/kbd-evdev.svg)](https://crates.io/crates/kbd-evdev)
[![docs.rs](https://docs.rs/kbd-evdev/badge.svg)](https://docs.rs/kbd-evdev)

Low-level Linux input backend for the [`kbd` workspace](https://github.com/joshuadavidthomas/kbd).

Most applications should start with [`kbd-global`](https://docs.rs/kbd-global), which wraps this crate in a threaded runtime. Use `kbd-evdev` directly when you need to own the poll loop yourself — your own event loop, your own threading, your own timing.

```toml
[dependencies]
kbd = "0.1"
kbd-evdev = "0.1"
```

## What it handles

- **Device discovery** — scans `/dev/input/` for keyboards (devices that support A–Z + Enter)
- **Hotplug** — inotify watch picks up devices added or removed at runtime
- **Exclusive grab** — `EVIOCGRAB` intercepts events before other applications see them
- **Event forwarding** — a uinput virtual device re-emits unmatched events in grab mode so other apps still work
- **Key conversion** — extension traits for `evdev::KeyCode` ↔ `kbd::key::Key`

## Example

Convert between evdev and kbd key types:

```rust
use evdev::KeyCode;
use kbd::key::Key;
use kbd_evdev::convert::{EvdevKeyCodeExt, KbdKeyExt};

let key: Key = KeyCode::KEY_A.to_key();
assert_eq!(key, Key::A);

let code: KeyCode = Key::A.to_key_code();
assert_eq!(code, KeyCode::KEY_A);
```

Discover and poll devices:

```rust,no_run
use std::path::Path;
use kbd_evdev::devices::{DeviceGrabMode, DeviceManager};

let manager = DeviceManager::new(Path::new("/dev/input"), DeviceGrabMode::Shared);
let _poll_fds = manager.poll_fds();
```

Call `poll(2)` on `DeviceManager::poll_fds()`, then pass the ready descriptors to `DeviceManager::process_polled_events()` — you get back key events (with device identity and press/release state) and disconnection notifications.

## Requirements

- Linux only
- Read access to `/dev/input/`
- Write access to `/dev/uinput` if you use grab mode and forwarding

To read input devices without running as root, add your user to the `input` group:

```bash
sudo usermod -aG input $USER
```

Log out and back in for the group change to take effect.

## License

kbd-evdev is licensed under the MIT license. See the [`LICENSE`](../../LICENSE) file for more information.
