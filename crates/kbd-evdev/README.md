# kbd-evdev

[![crates.io](https://img.shields.io/crates/v/kbd-evdev.svg)](https://crates.io/crates/kbd-evdev)
[![docs.rs](https://docs.rs/kbd-evdev/badge.svg)](https://docs.rs/kbd-evdev)

Linux evdev backend for [`kbd`](https://crates.io/crates/kbd) — device discovery, hotplug, grab, and event forwarding.

## Features

- **Device discovery** — scans `/dev/input/` for keyboards (devices supporting A–Z + Enter)
- **Hotplug** — inotify watch for device add/remove at runtime
- **Exclusive grab** — `EVIOCGRAB` for intercepting events before other applications
- **Event forwarding** — uinput virtual device re-emits unmatched events in grab mode
- **Key conversion** — extension traits for `evdev::KeyCode` ↔ `kbd::Key`

## Prerequisites

- **Linux only** — uses `/dev/input/`, `inotify`, and `/dev/uinput`
- **Read access to `/dev/input/`** — run as root or add your user to the `input` group:

  ```sh
  sudo usermod -aG input $USER
  # log out and back in for the group change to take effect
  ```

- **Write access to `/dev/uinput`** (grab mode only) — needed for the virtual device that forwards unmatched events

## Usage

```toml
[dependencies]
kbd-evdev = "0.1"
```

### Key conversion

```rust
use evdev::KeyCode;
use kbd::prelude::*;
use kbd_evdev::{EvdevKeyCodeExt, KbdKeyExt};

// evdev → kbd
let key: Key = KeyCode::KEY_A.to_key();
assert_eq!(key, Key::A);

// kbd → evdev
let code: KeyCode = Key::A.to_key_code();
assert_eq!(code, KeyCode::KEY_A);
```

### Device polling

```rust,no_run
use std::path::Path;
use kbd_evdev::devices::{DeviceManager, DeviceGrabMode};

let mut manager = DeviceManager::new(
    Path::new("/dev/input"),
    DeviceGrabMode::Shared,
);

// Build pollfd array from manager's file descriptors
let mut pollfds: Vec<libc::pollfd> = manager
    .poll_fds()
    .iter()
    .map(|&fd| libc::pollfd { fd, events: libc::POLLIN, revents: 0 })
    .collect();

// Poll and process events
unsafe { libc::poll(pollfds.as_mut_ptr(), pollfds.len() as _, 100) };
let result = manager.process_polled_events(&pollfds);

for event in &result.key_events {
    println!("{:?} {:?}", event.key, event.transition);
}
```

## Architecture

```text
/dev/input/event*          DeviceManager
  ├─ event0  ──┐       ┌─ discover + poll ──→ DeviceKeyEvent
  ├─ event1  ──┼──────→│  hotplug (inotify)   │
  └─ event2  ──┘       └───────────────────────┘
                                               │
                                     EvdevKeyCodeExt::to_key()
                                               │
                                               ▼
                                          kbd::Key
```

## See also

- [`kbd`](https://crates.io/crates/kbd) — core key types, matching, and layers
- [`kbd-global`](https://crates.io/crates/kbd-global) — threaded runtime built on this crate

## License

kbd-evdev is licensed under the MIT license. See the [`LICENSE`](../../LICENSE) file for more information.
