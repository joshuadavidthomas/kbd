//! Minimal winit window that converts key events via `kbd-winit`, feeds
//! them to a `Matcher`, and prints matches.
//!
//! ```sh
//! cargo run -p kbd-winit --example winit
//! ```

use kbd_core::Action;
use kbd_core::Hotkey;
use kbd_core::Key;
use kbd_core::KeyTransition;
use kbd_core::MatchResult;
use kbd_core::Matcher;
use kbd_core::Modifier;
use kbd_winit::WinitEventExt;
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::ActiveEventLoop;
use winit::event_loop::EventLoop;
use winit::keyboard::ModifiersState;
use winit::window::Window;
use winit::window::WindowId;

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
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if !event.state.is_pressed() {
                    return;
                }

                let Some(hotkey) = event.to_hotkey(self.modifiers) else {
                    return;
                };

                match self.matcher.process(&hotkey, KeyTransition::Press) {
                    MatchResult::Matched { action, .. } => {
                        println!("{hotkey} → matched!");
                        if let Action::Callback(cb) = action {
                            cb();
                        }
                    }
                    MatchResult::NoMatch => println!("{hotkey} → no match"),
                    MatchResult::Pending { .. } => println!("{hotkey} → pending..."),
                    MatchResult::Swallowed | MatchResult::Ignored => {}
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
