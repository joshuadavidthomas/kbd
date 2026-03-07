//! Benchmarks for hotkey operations on the per-keypress hot path.
//!
//! Equality and hashing are exercised on every `process()` call via
//! the `HashMap<Hotkey, Vec<BindingId>>` lookup. Parsing is config-time
//! only, but one representative bench catches regressions.

use divan::Bencher;
use kbd::hotkey::Hotkey;
use kbd::hotkey::HotkeySequence;
use kbd::hotkey::Modifier;
use kbd::key::Key;

fn main() {
    divan::main();
}

#[divan::bench]
fn hotkey_eq(bencher: Bencher) {
    let a = Hotkey::new(Key::S).modifier(Modifier::Ctrl);
    let b = Hotkey::new(Key::S).modifier(Modifier::Ctrl);

    bencher.bench_local(|| {
        divan::black_box(a == b);
    });
}

#[divan::bench]
fn hotkey_hash(bencher: Bencher) {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::Hash;
    use std::hash::Hasher;

    let hotkey = Hotkey::new(Key::S).modifier(Modifier::Ctrl);

    bencher.bench_local(|| {
        let mut hasher = DefaultHasher::new();
        hotkey.hash(&mut hasher);
        divan::black_box(hasher.finish());
    });
}

#[divan::bench]
fn parse_hotkey() -> Hotkey {
    "Ctrl+Shift+A".parse().unwrap()
}

#[divan::bench]
fn parse_sequence() -> HotkeySequence {
    "Ctrl+K, Ctrl+C".parse().unwrap()
}
