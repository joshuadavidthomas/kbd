//! Shared fixtures and helpers for kbd benchmarks.

use kbd::action::Action;
use kbd::dispatcher::Dispatcher;
use kbd::hotkey::Hotkey;
use kbd::hotkey::Modifier;
use kbd::hotkey::Modifiers;
use kbd::key::Key;
use kbd::layer::Layer;

/// All non-modifier keys used for generating bindings.
const KEYS: &[Key] = &[
    Key::A,
    Key::B,
    Key::C,
    Key::D,
    Key::E,
    Key::F,
    Key::G,
    Key::H,
    Key::I,
    Key::J,
    Key::K,
    Key::L,
    Key::M,
    Key::N,
    Key::O,
    Key::P,
    Key::Q,
    Key::R,
    Key::S,
    Key::T,
    Key::U,
    Key::V,
    Key::W,
    Key::X,
    Key::Y,
    Key::Z,
    Key::DIGIT0,
    Key::DIGIT1,
    Key::DIGIT2,
    Key::DIGIT3,
    Key::DIGIT4,
    Key::DIGIT5,
    Key::DIGIT6,
    Key::DIGIT7,
    Key::DIGIT8,
    Key::DIGIT9,
    Key::F1,
    Key::F2,
    Key::F3,
    Key::F4,
    Key::F5,
    Key::F6,
    Key::F7,
    Key::F8,
    Key::F9,
    Key::F10,
    Key::F11,
    Key::F12,
];

/// Modifier combinations used for generating bindings, ordered by
/// increasing complexity.
const MODIFIER_SETS: &[&[Modifier]] = &[
    &[Modifier::Ctrl],
    &[Modifier::Alt],
    &[Modifier::Super],
    &[Modifier::Ctrl, Modifier::Shift],
    &[Modifier::Ctrl, Modifier::Alt],
    &[Modifier::Alt, Modifier::Shift],
    &[Modifier::Super, Modifier::Shift],
    &[Modifier::Ctrl, Modifier::Alt, Modifier::Shift],
];

/// Generate `n` distinct hotkeys by cycling through keys and modifier sets.
#[must_use]
pub fn generate_hotkeys(n: usize) -> Vec<Hotkey> {
    let mut hotkeys = Vec::with_capacity(n);
    for i in 0..n {
        let key = KEYS[i % KEYS.len()];
        let mods = MODIFIER_SETS[i / KEYS.len() % MODIFIER_SETS.len()];
        let modifiers: Modifiers = mods.iter().copied().collect();
        hotkeys.push(Hotkey::with_modifiers(key, modifiers));
    }
    hotkeys
}

/// Build a dispatcher with `n` global bindings.
///
/// # Panics
///
/// Panics if `n` exceeds the number of unique hotkeys that can be generated.
#[must_use]
pub fn dispatcher_with_globals(n: usize) -> Dispatcher {
    let mut dispatcher = Dispatcher::new();
    for hotkey in generate_hotkeys(n) {
        dispatcher
            .register(hotkey, Action::Suppress)
            .expect("unique hotkeys");
    }
    dispatcher
}

/// Build a dispatcher with `n` bindings spread across `layer_count` layers,
/// plus a base set of global bindings.
///
/// # Panics
///
/// Panics if the total binding count exceeds the number of unique hotkeys
/// that can be generated.
#[must_use]
pub fn dispatcher_with_layers(n_per_layer: usize, layer_count: usize) -> Dispatcher {
    let mut dispatcher = Dispatcher::new();

    // Add some global bindings as a baseline.
    for hotkey in generate_hotkeys(10) {
        dispatcher
            .register(hotkey, Action::Suppress)
            .expect("unique hotkeys");
    }

    let all_hotkeys = generate_hotkeys(n_per_layer * layer_count);
    for layer_idx in 0..layer_count {
        let name = format!("layer_{layer_idx}");
        let start = layer_idx * n_per_layer;
        let end = start + n_per_layer;
        let mut layer = Layer::new(&*name);
        for hotkey in &all_hotkeys[start..end] {
            layer = layer
                .bind(*hotkey, Action::Suppress)
                .expect("unique hotkeys");
        }
        dispatcher.define_layer(layer).expect("unique layer name");
        dispatcher.push_layer(&*name).expect("layer defined");
    }

    dispatcher
}

/// Build a dispatcher with `n` global sequence bindings (2-step sequences).
///
/// # Panics
///
/// Panics if `n` exceeds the number of unique sequences that can be generated.
#[must_use]
pub fn dispatcher_with_sequences(n: usize) -> Dispatcher {
    let mut dispatcher = Dispatcher::new();
    let hotkeys = generate_hotkeys(n);
    for (i, first) in hotkeys.iter().enumerate() {
        let second_key = KEYS[(i + 1) % KEYS.len()];
        let second = Hotkey::new(second_key);
        dispatcher
            .register_sequence(vec![*first, second], Action::Suppress)
            .expect("unique sequences");
    }
    dispatcher
}

/// Binding count tiers for parameterized benchmarks.
#[derive(Clone, Copy)]
pub struct BindingCount(pub usize);

impl std::fmt::Display for BindingCount {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Standard binding count tiers.
#[must_use]
pub fn binding_counts() -> &'static [BindingCount] {
    &[
        BindingCount(10),
        BindingCount(50),
        BindingCount(100),
        BindingCount(200),
    ]
}

/// A hotkey that will never match any registered binding (for miss benchmarks).
#[must_use]
pub fn unbound_hotkey() -> Hotkey {
    Hotkey::new(Key::PAUSE)
        .modifier(Modifier::Ctrl)
        .modifier(Modifier::Shift)
        .modifier(Modifier::Alt)
        .modifier(Modifier::Super)
}
