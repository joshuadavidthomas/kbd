# kbd

[![crates.io](https://img.shields.io/crates/v/kbd.svg)](https://crates.io/crates/kbd)
[![docs.rs](https://docs.rs/kbd/badge.svg)](https://docs.rs/kbd)

Pure-logic hotkey engine — key types, modifier tracking, binding matching, layer stacks, sequence resolution. No platform dependencies.

```toml
[dependencies]
kbd = "0.1"
```

```rust
use kbd::{Action, Hotkey, Key, KeyTransition, MatchResult, Matcher, Modifier};

let mut matcher = Matcher::new();

let hotkey: Hotkey = "Ctrl+Shift+A".parse().unwrap();
matcher.register(hotkey.clone(), Action::Swallow).unwrap();

let result = matcher.process(&hotkey, KeyTransition::Press);
assert!(matches!(result, MatchResult::Matched { .. }));
```

Supports [layers](https://docs.rs/kbd/latest/kbd/layer/), [introspection](https://docs.rs/kbd/latest/kbd/introspection/), and optional `serde`. See the [API docs](https://docs.rs/kbd) for the full picture.

## License

MIT
