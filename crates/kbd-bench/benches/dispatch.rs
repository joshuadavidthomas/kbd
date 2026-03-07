//! Benchmarks for the dispatcher's `process()` hot path.
//!
//! Measures per-keypress latency across:
//! - varying binding counts (10, 50, 100, 200)
//! - hit vs. miss paths
//! - global bindings vs. layered bindings
//! - sequence prefix matching

#![allow(clippy::trivially_copy_pass_by_ref)]

use divan::Bencher;
use kbd::hotkey::Hotkey;
use kbd::key::Key;
use kbd::key_state::KeyTransition;
use kbd_bench::BindingCount;
use kbd_bench::binding_counts;
use kbd_bench::dispatcher_with_globals;
use kbd_bench::dispatcher_with_layers;
use kbd_bench::dispatcher_with_sequences;
use kbd_bench::generate_hotkeys;
use kbd_bench::unbound_hotkey;

fn main() {
    divan::main();
}

// Global binding: hit (last registered binding)

#[divan::bench(args = binding_counts())]
fn global_hit_last(bencher: Bencher, count: &BindingCount) {
    let mut dispatcher = dispatcher_with_globals(count.0);
    let hotkeys = generate_hotkeys(count.0);
    let last = &hotkeys[hotkeys.len() - 1];

    bencher.bench_local(|| {
        dispatcher.process(last, KeyTransition::Press);
    });
}

// Global binding: hit (first registered binding)

#[divan::bench(args = binding_counts())]
fn global_hit_first(bencher: Bencher, count: &BindingCount) {
    let mut dispatcher = dispatcher_with_globals(count.0);
    let hotkeys = generate_hotkeys(count.0);
    let first = &hotkeys[0];

    bencher.bench_local(|| {
        dispatcher.process(first, KeyTransition::Press);
    });
}

// Global binding: miss

#[divan::bench(args = binding_counts())]
fn global_miss(bencher: Bencher, count: &BindingCount) {
    let mut dispatcher = dispatcher_with_globals(count.0);
    let miss = unbound_hotkey();

    bencher.bench_local(|| {
        dispatcher.process(&miss, KeyTransition::Press);
    });
}

// Layer stack: hit in top layer

#[divan::bench(args = [1, 3, 5, 10])]
fn layer_stack_hit_top(bencher: Bencher, layer_count: usize) {
    let bindings_per_layer = 20;
    let mut dispatcher = dispatcher_with_layers(bindings_per_layer, layer_count);

    // Pick a hotkey from the top layer.
    let total = bindings_per_layer * layer_count;
    let hotkeys = generate_hotkeys(total);
    let top_layer_hotkey = &hotkeys[total - 1];

    bencher.bench_local(|| {
        dispatcher.process(top_layer_hotkey, KeyTransition::Press);
    });
}

// Layer stack: hit in bottom layer (worst case traversal)

#[divan::bench(args = [1, 3, 5, 10])]
fn layer_stack_hit_bottom(bencher: Bencher, layer_count: usize) {
    let bindings_per_layer = 20;
    let mut dispatcher = dispatcher_with_layers(bindings_per_layer, layer_count);

    // Pick a hotkey from the bottom layer (first pushed).
    let hotkeys = generate_hotkeys(bindings_per_layer * layer_count);
    let bottom_layer_hotkey = &hotkeys[0];

    bencher.bench_local(|| {
        dispatcher.process(bottom_layer_hotkey, KeyTransition::Press);
    });
}

// Layer stack: miss with fallthrough (no swallow)

#[divan::bench(args = [1, 3, 5, 10])]
fn layer_stack_miss_fallthrough(bencher: Bencher, layer_count: usize) {
    let mut dispatcher = dispatcher_with_layers(20, layer_count);
    let miss = unbound_hotkey();

    bencher.bench_local(|| {
        dispatcher.process(&miss, KeyTransition::Press);
    });
}

// Sequence: prefix match (entering pending state)

#[divan::bench(args = binding_counts())]
fn sequence_prefix_match(bencher: Bencher, count: &BindingCount) {
    let hotkeys = generate_hotkeys(count.0);
    let first_step = &hotkeys[0];

    bencher.bench_local(|| {
        // Fresh dispatcher each time to avoid accumulated state.
        let mut dispatcher = dispatcher_with_sequences(count.0);
        dispatcher.process(first_step, KeyTransition::Press);
    });
}

// Sequence: miss (no prefix matches)

#[divan::bench(args = binding_counts())]
fn sequence_miss(bencher: Bencher, count: &BindingCount) {
    let mut dispatcher = dispatcher_with_sequences(count.0);
    let miss = unbound_hotkey();

    bencher.bench_local(|| {
        dispatcher.process(&miss, KeyTransition::Press);
    });
}

// Ignored events (release/repeat) — should be near-zero cost

#[divan::bench]
fn ignored_release(bencher: Bencher) {
    let mut dispatcher = dispatcher_with_globals(100);
    let hotkey = Hotkey::new(Key::A);

    bencher.bench_local(|| {
        dispatcher.process(&hotkey, KeyTransition::Release);
    });
}

#[divan::bench]
fn ignored_modifier_only(bencher: Bencher) {
    let mut dispatcher = dispatcher_with_globals(100);
    let modifier_hotkey = Hotkey::new(Key::CONTROL_LEFT);

    bencher.bench_local(|| {
        dispatcher.process(&modifier_hotkey, KeyTransition::Press);
    });
}
