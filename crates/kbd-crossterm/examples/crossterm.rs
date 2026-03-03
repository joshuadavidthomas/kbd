//! Minimal TUI loop using `kbd-crossterm` + `kbd`.
//!
//! Reads real terminal key events, converts them via the crossterm
//! extension traits, and feeds them to a `Dispatcher`. Prints match results
//! to the terminal.
//!
//! ```sh
//! cargo run -p kbd-crossterm --example crossterm
//! ```

use std::io::Write;
use std::io::{self};
use std::time::Duration;

use crossterm::event::Event;
use crossterm::event::KeyCode;
use crossterm::event::KeyEventKind;
use crossterm::event::{self};
use crossterm::terminal;
use kbd::action::Action;
use kbd::dispatcher::Dispatcher;
use kbd::dispatcher::MatchResult;
use kbd::key::Hotkey;
use kbd::key::Key;
use kbd::key::Modifier;
use kbd::key_state::KeyTransition;
use kbd_crossterm::CrosstermEventExt;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("kbd-crossterm example — press keys to see matches");
    println!();

    let mut dispatcher = Dispatcher::new();

    // Register some bindings
    dispatcher.register(
        Hotkey::new(Key::S).modifier(Modifier::Ctrl),
        Action::from(|| print!("  → Save!\r\n")),
    )?;
    dispatcher.register(
        Hotkey::new(Key::Q).modifier(Modifier::Ctrl),
        Action::from(|| print!("  → Quit!\r\n")),
    )?;
    dispatcher.register(
        Hotkey::new(Key::SPACE),
        Action::from(|| print!("  → Space pressed!\r\n")),
    )?;

    println!("Bindings:");
    println!("  Ctrl+S  → Save");
    println!("  Ctrl+Q  → Quit (also exits this example)");
    println!("  Space   → Space pressed");
    println!();
    println!("Press keys to see matches. Ctrl+C or Ctrl+Q to exit.");
    println!();

    // Enable raw mode so we get individual key events
    terminal::enable_raw_mode()?;
    let result = run_event_loop(&mut dispatcher);
    terminal::disable_raw_mode()?;

    result
}

fn run_event_loop(dispatcher: &mut Dispatcher) -> Result<(), Box<dyn std::error::Error>> {
    loop {
        // Poll for events with a 100ms timeout
        if event::poll(Duration::from_millis(100))?
            && let Event::Key(key_event) = event::read()?
        {
            // Only process key press events (not release/repeat)
            if key_event.kind != KeyEventKind::Press {
                continue;
            }

            // Exit on Ctrl+C
            if key_event.code == KeyCode::Char('c')
                && key_event
                    .modifiers
                    .contains(crossterm::event::KeyModifiers::CONTROL)
            {
                print!("\r\nExiting...\r\n");
                io::stdout().flush()?;
                return Ok(());
            }

            // Convert crossterm event to kbd Hotkey
            let Some(hotkey) = key_event.to_hotkey() else {
                continue;
            };

            // Process through the dispatcher
            match dispatcher.process(&hotkey, KeyTransition::Press) {
                MatchResult::Matched { action, .. } => {
                    print!("{hotkey}: matched!\r\n");
                    if let Action::Callback(cb) = action {
                        cb();
                    }

                    // Exit on Ctrl+Q
                    if hotkey == Hotkey::new(Key::Q).modifier(Modifier::Ctrl) {
                        print!("\r\nExiting via Ctrl+Q...\r\n");
                        io::stdout().flush()?;
                        return Ok(());
                    }
                }
                MatchResult::NoMatch => print!("{hotkey}: no match\r\n"),
                MatchResult::Pending { .. } => print!("{hotkey}: pending...\r\n"),
                MatchResult::Suppressed | MatchResult::Ignored => {}
            }
            io::stdout().flush()?;
        }
    }
}
