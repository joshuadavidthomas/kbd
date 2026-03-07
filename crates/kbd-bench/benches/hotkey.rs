//! Benchmarks for hotkey parsing and construction.
//!
//! These are not on the per-keypress hot path, but parsing happens at
//! setup/config time and regressions would be felt by applications that
//! parse many bindings from config files.

use divan::Bencher;
use kbd::hotkey::Hotkey;
use kbd::hotkey::HotkeySequence;
use kbd::hotkey::Modifier;
use kbd::key::Key;

fn main() {
    divan::main();
}

// Hotkey construction

#[divan::bench]
fn hotkey_new_bare_key() -> Hotkey {
    Hotkey::new(Key::A)
}

#[divan::bench]
fn hotkey_with_one_modifier() -> Hotkey {
    Hotkey::with_modifiers(Key::A, vec![Modifier::Ctrl])
}

#[divan::bench]
fn hotkey_with_three_modifiers() -> Hotkey {
    Hotkey::with_modifiers(Key::A, vec![Modifier::Ctrl, Modifier::Shift, Modifier::Alt])
}

// Hotkey parsing from strings

#[divan::bench]
fn parse_simple_key() -> Hotkey {
    "A".parse().unwrap()
}

#[divan::bench]
fn parse_ctrl_key() -> Hotkey {
    "Ctrl+S".parse().unwrap()
}

#[divan::bench]
fn parse_triple_modifier() -> Hotkey {
    "Ctrl+Shift+Alt+Delete".parse().unwrap()
}

#[divan::bench]
fn parse_alias() -> Hotkey {
    "Super+PgUp".parse().unwrap()
}

// Sequence parsing

#[divan::bench]
fn parse_two_step_sequence() -> HotkeySequence {
    "Ctrl+K, Ctrl+C".parse().unwrap()
}

#[divan::bench]
fn parse_three_step_sequence() -> HotkeySequence {
    "Ctrl+K, Ctrl+Shift+A, B".parse().unwrap()
}

// Hotkey display (used in introspection/debugging)

#[divan::bench]
fn display_hotkey(bencher: Bencher) {
    let hotkey = Hotkey::new(Key::S)
        .modifier(Modifier::Ctrl)
        .modifier(Modifier::Shift);

    bencher.bench_local(|| {
        divan::black_box(hotkey.to_string());
    });
}

// Hotkey equality (used in HashMap lookups on every keypress)

#[divan::bench]
fn hotkey_eq_match(bencher: Bencher) {
    let a = Hotkey::new(Key::S).modifier(Modifier::Ctrl);
    let b = Hotkey::new(Key::S).modifier(Modifier::Ctrl);

    bencher.bench_local(|| {
        divan::black_box(a == b);
    });
}

#[divan::bench]
fn hotkey_eq_mismatch(bencher: Bencher) {
    let a = Hotkey::new(Key::S).modifier(Modifier::Ctrl);
    let b = Hotkey::new(Key::A).modifier(Modifier::Alt);

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
