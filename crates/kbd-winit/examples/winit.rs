//! Minimal winit window that converts key events via `kbd-winit`, feeds
//! them to a `Dispatcher`, and prints matches.
//!
//! ```sh
//! cargo run -p kbd-winit --example winit
//! ```

use std::num::NonZeroU32;
use std::sync::Arc;

use kbd::Action;
use kbd::Hotkey;
use kbd::Key;
use kbd::KeyTransition;
use kbd::MatchResult;
use kbd::Dispatcher;
use kbd::Modifier;
use kbd_winit::WinitEventExt;
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::ActiveEventLoop;
use winit::event_loop::EventLoop;
use winit::keyboard::ModifiersState;
use winit::window::Window;
use winit::window::WindowId;

struct App {
    matcher: Dispatcher,
    modifiers: ModifiersState,
    window: Option<Arc<Window>>,
    surface: Option<softbuffer::Surface<Arc<Window>, Arc<Window>>>,
}

impl App {
    fn new() -> Self {
        let mut matcher = Dispatcher::new();

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
            surface: None,
        }
    }

    fn fill_window(&mut self) {
        let (Some(surface), Some(window)) = (&mut self.surface, &self.window) else {
            return;
        };
        let size = window.inner_size();
        let (Some(width), Some(height)) =
            (NonZeroU32::new(size.width), NonZeroU32::new(size.height))
        else {
            return;
        };
        let _ = surface.resize(width, height);
        if let Ok(mut buffer) = surface.buffer_mut() {
            // Dark gray background
            for pixel in buffer.iter_mut() {
                *pixel = 0xFF_2D_2D_2D;
            }
            let _ = buffer.present();
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_none() {
            let attrs = Window::default_attributes().with_title("kbd-winit example");
            match event_loop.create_window(attrs) {
                Ok(window) => {
                    let window = Arc::new(window);
                    let context =
                        softbuffer::Context::new(window.clone()).expect("softbuffer context");
                    let surface = softbuffer::Surface::new(&context, window.clone())
                        .expect("softbuffer surface");
                    self.window = Some(window);
                    self.surface = Some(surface);
                    self.fill_window();
                }
                Err(e) => eprintln!("Failed to create window: {e}"),
            }
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }
            WindowEvent::RedrawRequested | WindowEvent::Resized(_) => {
                self.fill_window();
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
                    MatchResult::Suppressed | MatchResult::Ignored => {}
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
