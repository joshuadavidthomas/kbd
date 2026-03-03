//! Opens an iced window and feeds keyboard events through a `Dispatcher`.
//! Press keys to see matches in the GUI.
//!
//! ```sh
//! cargo run -p kbd-iced --example iced
//! ```

use iced::Element;
use iced::Subscription;
use iced::Task;
use iced::widget::column;
use iced::widget::scrollable;
use iced::widget::text;
use kbd::dispatcher::Dispatcher;
use kbd::dispatcher::MatchResult;
use kbd::key::Hotkey;
use kbd::key::Key;
use kbd::key::Modifier;
use kbd::key_state::KeyTransition;
use kbd_iced::IcedEventExt;

fn main() -> iced::Result {
    iced::application(App::new, App::update, App::view)
        .title("kbd-iced example")
        .subscription(App::subscription as fn(&App) -> _)
        .run()
}

struct App {
    matcher: Dispatcher,
    log: Vec<String>,
}

impl App {
    fn new() -> (Self, Task<Message>) {
        let mut matcher = Dispatcher::new();

        matcher
            .register(Hotkey::new(Key::S).modifier(Modifier::Ctrl), || {})
            .expect("register Ctrl+S");
        matcher
            .register(Hotkey::new(Key::Z).modifier(Modifier::Ctrl), || {})
            .expect("register Ctrl+Z");
        matcher
            .register(
                Hotkey::new(Key::Z)
                    .modifier(Modifier::Ctrl)
                    .modifier(Modifier::Shift),
                || {},
            )
            .expect("register Ctrl+Shift+Z");
        matcher
            .register(Hotkey::new(Key::SPACE), || {})
            .expect("register Space");
        matcher
            .register(Hotkey::new(Key::ESCAPE), || {})
            .expect("register Escape");

        (
            Self {
                matcher,
                log: Vec::new(),
            },
            Task::none(),
        )
    }
}

#[derive(Debug, Clone)]
enum Message {
    KeyEvent(iced::keyboard::Event),
}

impl App {
    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::KeyEvent(event) => {
                let iced_core::keyboard::Event::KeyPressed { .. } = event else {
                    return Task::none();
                };

                let Some(hotkey) = event.to_hotkey() else {
                    return Task::none();
                };

                // Standard app-level quit shortcut
                if hotkey == Hotkey::new(Key::Q).modifier(Modifier::Ctrl) {
                    return iced::exit();
                }

                let line = match self.matcher.process(&hotkey, KeyTransition::Press) {
                    MatchResult::Matched { .. } => format!("{hotkey} → matched!"),
                    MatchResult::NoMatch => format!("{hotkey} → no match"),
                    MatchResult::Pending { .. } => format!("{hotkey} → pending..."),
                    MatchResult::Suppressed | MatchResult::Ignored => return Task::none(),
                };
                self.log.push(line);
                Task::none()
            }
        }
    }

    fn view(&self) -> Element<'_, Message> {
        let header = text!("kbd-iced example").size(24);

        let bindings = column![
            text("Registered bindings:"),
            text("  Ctrl+S        Save").font(iced::Font::MONOSPACE),
            text("  Ctrl+Z        Undo").font(iced::Font::MONOSPACE),
            text("  Ctrl+Shift+Z  Redo").font(iced::Font::MONOSPACE),
            text("  Space         Space").font(iced::Font::MONOSPACE),
            text("  Escape        Escape").font(iced::Font::MONOSPACE),
        ]
        .spacing(2);

        let log_entries: Vec<Element<Message>> = self
            .log
            .iter()
            .map(|line| text(line.clone()).font(iced::Font::MONOSPACE).into())
            .collect();

        let log = scrollable(column(log_entries).spacing(2));

        column![header, bindings, text("Press keys to see matches:"), log]
            .spacing(10)
            .padding(20)
            .into()
    }

    fn subscription(_state: &Self) -> Subscription<Message> {
        iced::keyboard::listen().map(Message::KeyEvent)
    }
}
