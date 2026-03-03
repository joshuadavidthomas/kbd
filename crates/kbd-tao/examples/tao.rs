//! Minimal tao window that converts key events via `kbd-tao`, feeds them
//! to a `Dispatcher`, and prints matches. tao is Tauri's fork of winit.
//!
//! ```sh
//! cargo run -p kbd-tao --example tao
//! ```

use kbd::Action;
use kbd::Hotkey;
use kbd::Key;
use kbd::KeyTransition;
use kbd::MatchResult;
use kbd::Dispatcher;
use kbd::Modifier;
use kbd_tao::TaoEventExt;
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

    let mut matcher = Dispatcher::new();
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
                }
                WindowEvent::KeyboardInput { event, .. } => {
                    if event.state != tao::event::ElementState::Pressed {
                        return;
                    }

                    // Convert to a kbd Hotkey
                    let Some(hotkey) = event.to_hotkey(modifiers) else {
                        return;
                    };

                    // Process through the matcher
                    match matcher.process(&hotkey, KeyTransition::Press) {
                        MatchResult::Matched { action, .. } => {
                            println!("{hotkey} → matched!");
                            if let Action::Callback(cb) = action {
                                cb();
                            }
                        }
                        MatchResult::NoMatch => println!("{hotkey} → no match"),
                        MatchResult::Pending { .. } => println!("{hotkey} → pending..."),
                        MatchResult::Suppressed | MatchResult::Ignored => {}
                    }
                }
                _ => {}
            }
        }
    });
}
