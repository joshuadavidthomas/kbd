//! Minimal winit window that converts key events via `kbd-winit`, feeds
//! them to a `Matcher`, and prints matches.
//!
//! ```sh
//! cargo run -p kbd-winit --example winit
//! ```

use kbd_core::{Action, Hotkey, Key, KeyTransition, MatchResult, Matcher, Modifier};
use kbd_winit::{WinitEventExt, WinitKeyExt, WinitModifiersExt};
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::ModifiersState;
use winit::window::{Window, WindowId};

struct App {
    matcher: Matcher,
    modifiers: ModifiersState,
    window: Option<Window>,
}

impl App {
    fn new() -> Self {
        let mut matcher = Matcher::new();

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

        Self {
            matcher,
            modifiers: ModifiersState::empty(),
            window: None,
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_none() {
            let attrs = Window::default_attributes().with_title("kbd-winit example");
            match event_loop.create_window(attrs) {
                Ok(w) => self.window = Some(w),
                Err(e) => eprintln!("Failed to create window: {e}"),
            }
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }
            WindowEvent::ModifiersChanged(mods) => {
                self.modifiers = mods.state();
                let kbd_mods: Vec<_> = self.modifiers.to_modifiers();
                println!("Modifiers changed: {kbd_mods:?}");
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if !event.state.is_pressed() {
                    return;
                }

                // Show the raw winit key
                let key = event.physical_key.to_key();
                print!("winit: {:?} → kbd-core: {:?} → ", event.physical_key, key);

                // Convert to a kbd-core Hotkey
                let hotkey = match event.to_hotkey(self.modifiers) {
                    Some(hk) => hk,
                    None => {
                        println!("(unmappable)");
                        return;
                    }
                };

                // Process through the matcher
                match self.matcher.process(&hotkey, KeyTransition::Press) {
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
}

fn main() {
    let event_loop = EventLoop::new().expect("create event loop");
    let mut app = App::new();
    event_loop.run_app(&mut app).expect("run event loop");
}
