//! Minimal tao window that converts key events via `kbd-tao`, feeds them
//! to a `Matcher`, and prints matches. tao is Tauri's fork of winit.
//!
//! ```sh
//! cargo run -p kbd-tao --example tao
//! ```

use kbd_core::Action;
use kbd_core::Hotkey;
use kbd_core::Key;
use kbd_core::KeyTransition;
use kbd_core::MatchResult;
use kbd_core::Matcher;
use kbd_core::Modifier;
use kbd_tao::TaoEventExt;
use kbd_tao::TaoKeyExt;
use kbd_tao::TaoModifiersExt;
use tao::event::Event;
use tao::event::WindowEvent;
use tao::event_loop::ControlFlow;
use tao::event_loop::EventLoop;
use tao::keyboard::ModifiersState;
use tao::window::WindowBuilder;

fn main() {
    let event_loop = EventLoop::new();
    let _window = WindowBuilder::new()
        .with_title("kbd-tao example")
        .build(&event_loop)
        .expect("create window");

    let mut matcher = Matcher::new();
    let mut modifiers = ModifiersState::empty();

    matcher
        .register(
            Hotkey::new(Key::S).modifier(Modifier::Ctrl),
            Action::from(|| println!("  → Save!")),
        )
        .expect("register Ctrl+S");
    matcher
        .register(
            Hotkey::new(Key::Q).modifier(Modifier::Ctrl),
            Action::from(|| println!("  → Quit!")),
        )
        .expect("register Ctrl+Q");
    matcher
        .register(
            Hotkey::new(Key::SPACE),
            Action::from(|| println!("  → Space!")),
        )
        .expect("register Space");

    println!("Bindings:");
    println!("  Ctrl+S  → Save");
    println!("  Ctrl+Q  → Quit");
    println!("  Space   → Space");
    println!();
    println!("Focus the window and press keys.");

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        if let Event::WindowEvent { event, .. } = event {
            match event {
                WindowEvent::CloseRequested => {
                    *control_flow = ControlFlow::Exit;
                }
                WindowEvent::ModifiersChanged(mods) => {
                    modifiers = mods;
                    let kbd_mods: Vec<_> = modifiers.to_modifiers();
                    println!("Modifiers changed: {kbd_mods:?}");
                }
                WindowEvent::KeyboardInput { event, .. } => {
                    if event.state != tao::event::ElementState::Pressed {
                        return;
                    }

                    // Show the raw tao key
                    let key = event.physical_key.to_key();
                    print!("tao: {:?} → kbd-core: {:?} → ", event.physical_key, key);

                    // Convert to a kbd-core Hotkey
                    let Some(hotkey) = event.to_hotkey(modifiers) else {
                        println!("(unmappable)");
                        return;
                    };

                    // Process through the matcher
                    match matcher.process(&hotkey, KeyTransition::Press) {
                        MatchResult::Matched { action, .. } => {
                            if let Action::Callback(cb) = action {
                                cb();
                            }
                        }
                        MatchResult::NoMatch => println!("no match for {hotkey}"),
                        MatchResult::Swallowed => println!("swallowed"),
                        MatchResult::Pending { .. } => println!("pending..."),
                        MatchResult::Ignored => println!("ignored"),
                    }
                }
                _ => {}
            }
        }
    });
}
