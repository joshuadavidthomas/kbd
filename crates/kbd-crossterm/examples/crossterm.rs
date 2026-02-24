//! Minimal TUI loop using `kbd-crossterm` + `kbd-core`.
//!
//! Reads real terminal key events, converts them via the crossterm
//! extension traits, and feeds them to a `Matcher`. Prints match results
//! to the terminal.
//!
//! ```sh
//! cargo run -p kbd-crossterm --example crossterm
//! ```

use std::io::{self, Write};
use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::terminal;
use kbd_core::{Action, Hotkey, Key, KeyTransition, MatchResult, Matcher, Modifier};
use kbd_crossterm::CrosstermEventExt;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("kbd-crossterm example — press keys to see matches");
    println!();

    let mut matcher = Matcher::new();

    // Register some bindings
    matcher.register(
        Hotkey::new(Key::S).modifier(Modifier::Ctrl),
        Action::from(|| println!("  → Save!")),
    )?;
    matcher.register(
        Hotkey::new(Key::Q).modifier(Modifier::Ctrl),
        Action::from(|| println!("  → Quit!")),
    )?;
    matcher.register(
        Hotkey::new(Key::SPACE),
        Action::from(|| println!("  → Space pressed!")),
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
    let result = run_event_loop(&mut matcher);
    terminal::disable_raw_mode()?;

    result
}

fn run_event_loop(matcher: &mut Matcher) -> Result<(), Box<dyn std::error::Error>> {
    loop {
        // Poll for events with a 100ms timeout
        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key_event) = event::read()? {
                // Only process key press events (not release/repeat)
                if key_event.kind != KeyEventKind::Press {
                    continue;
                }

                // Exit on Ctrl+C
                if key_event.code == KeyCode::Char('c')
                    && key_event.modifiers.contains(crossterm::event::KeyModifiers::CONTROL)
                {
                    print!("\r\nExiting...\r\n");
                    io::stdout().flush()?;
                    return Ok(());
                }

                // Convert crossterm event to kbd-core Hotkey
                let hotkey = match key_event.to_hotkey() {
                    Some(hk) => hk,
                    None => {
                        print!("  (unmappable key: {key_event:?})\r\n");
                        io::stdout().flush()?;
                        continue;
                    }
                };

                // Process through the matcher
                print!("{hotkey}: ");
                match matcher.process(&hotkey, KeyTransition::Press) {
                    MatchResult::Matched { action, .. } => {
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
                    MatchResult::NoMatch => print!("no match\r\n"),
                    MatchResult::Swallowed => print!("swallowed\r\n"),
                    MatchResult::Pending { .. } => print!("pending...\r\n"),
                    MatchResult::Ignored => print!("ignored\r\n"),
                }
                io::stdout().flush()?;
            }
        }
    }
}
