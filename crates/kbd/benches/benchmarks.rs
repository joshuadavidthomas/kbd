use kbd::prelude::*;

fn main() {
    divan::main();
}

#[divan::bench]
fn parse_simple_hotkey() -> Hotkey {
    "Ctrl+S".parse().unwrap()
}

#[divan::bench]
fn parse_multi_modifier_hotkey() -> Hotkey {
    "Ctrl+Shift+Alt+F5".parse().unwrap()
}

#[divan::bench]
fn parse_hotkey_sequence() -> HotkeySequence {
    "Ctrl+K, Ctrl+C".parse().unwrap()
}

#[divan::bench]
fn dispatcher_process_match() {
    let mut dispatcher = Dispatcher::new();
    dispatcher
        .register(
            Hotkey::new(Key::S).modifier(Modifier::Ctrl),
            Action::Suppress,
        )
        .unwrap();

    let hotkey = Hotkey::new(Key::S).modifier(Modifier::Ctrl);
    let _ = dispatcher.process(&hotkey, KeyTransition::Press);
}

#[divan::bench]
fn dispatcher_process_no_match() {
    let mut dispatcher = Dispatcher::new();
    dispatcher
        .register(
            Hotkey::new(Key::S).modifier(Modifier::Ctrl),
            Action::Suppress,
        )
        .unwrap();

    let hotkey = Hotkey::new(Key::A);
    let _ = dispatcher.process(&hotkey, KeyTransition::Press);
}

#[divan::bench]
fn dispatcher_layer_match() {
    let mut dispatcher = Dispatcher::new();
    dispatcher
        .define_layer(
            Layer::new("nav")
                .bind(Key::H, Action::Suppress)
                .unwrap()
                .bind(Key::J, Action::Suppress)
                .unwrap()
                .bind(Key::K, Action::Suppress)
                .unwrap()
                .bind(Key::L, Action::Suppress)
                .unwrap(),
        )
        .unwrap();
    dispatcher.push_layer("nav").unwrap();

    let hotkey = Hotkey::new(Key::H);
    let _ = dispatcher.process(&hotkey, KeyTransition::Press);
}

#[divan::bench(args = [10, 50, 100])]
fn dispatcher_many_bindings(n: usize) {
    let keys = [
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
    ];

    let modifiers = [Modifier::Ctrl, Modifier::Shift, Modifier::Alt];

    let mut dispatcher = Dispatcher::new();
    for i in 0..n {
        let key = keys[i % keys.len()];
        let modifier = modifiers[i % modifiers.len()];
        // Ignore duplicate registration errors
        let _ = dispatcher.register(Hotkey::new(key).modifier(modifier), Action::Suppress);
    }

    let hotkey = Hotkey::new(Key::Z).modifier(Modifier::Alt);
    let _ = dispatcher.process(&hotkey, KeyTransition::Press);
}

#[divan::bench]
fn hotkey_display() -> String {
    let hotkey = Hotkey::new(Key::S)
        .modifier(Modifier::Ctrl)
        .modifier(Modifier::Shift);
    hotkey.to_string()
}
