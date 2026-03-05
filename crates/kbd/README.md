# kbd

[![crates.io](https://img.shields.io/crates/v/kbd.svg)](https://crates.io/crates/kbd)
[![docs.rs](https://docs.rs/kbd/badge.svg)](https://docs.rs/kbd)

Pure-logic hotkey engine — key types, modifier tracking, binding matching, layer stacks, sequence resolution. No platform dependencies.

```toml
[dependencies]
kbd = "0.1"
```

```rust
use kbd::action::Action;
use kbd::dispatcher::{Dispatcher, MatchResult};
use kbd::hotkey::{Hotkey, Modifier};
use kbd::key::Key;
use kbd::key_state::KeyTransition;

let mut dispatcher = Dispatcher::new();

let hotkey: Hotkey = "Ctrl+Shift+A".parse()?;
dispatcher.register(hotkey.clone(), Action::Suppress)?;

let result = dispatcher.process(&hotkey, KeyTransition::Press);
assert!(matches!(result, MatchResult::Matched { .. }));
# Ok::<(), kbd::error::Error>(())
```

Supports [layers](https://docs.rs/kbd/latest/kbd/layer/), [introspection](https://docs.rs/kbd/latest/kbd/introspection/), and optional `serde`. See the [API docs](https://docs.rs/kbd) for the full picture.

## License

kbd is licensed under the MIT license. See the [`LICENSE`](../../LICENSE) file for more information.
