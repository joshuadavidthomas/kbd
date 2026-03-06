//! Global hotkey listener — registers system-wide hotkeys and waits
//! for them to fire.
//!
//! Requires access to `/dev/input/` devices (add your user to the
//! `input` group or run as root).
//!
//! ```sh
//! cargo run -p kbd-global --example global
//! ```

use std::sync::mpsc;

use kbd::prelude::*;
use kbd_global::error::Error;
use kbd_global::manager::HotkeyManager;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Ignore terminal job-control signals so Ctrl+Z doesn't background us
    unsafe {
        libc::signal(libc::SIGTSTP, libc::SIG_IGN);
    }

    // Disable terminal echo so raw keypresses don't appear
    let original_termios = disable_echo();

    let result = match run() {
        Ok(()) => Ok(()),
        Err(e) => {
            let msg = format!("{e}");
            if msg.contains("Permission denied") || msg.contains("permission") {
                eprintln!("Error: {e}");
                eprintln!();
                eprintln!("This example needs access to /dev/input/ devices.");
                eprintln!("Either:");
                eprintln!("  1. Add your user to the 'input' group:");
                eprintln!("     sudo usermod -aG input $USER");
                eprintln!("  2. Or run with sudo (not recommended for regular use)");
                Ok(())
            } else {
                Err(e.into())
            }
        }
    };

    // Restore terminal settings
    restore_termios(original_termios);
    result
}

fn disable_echo() -> Option<libc::termios> {
    unsafe {
        let mut termios = std::mem::zeroed::<libc::termios>();
        if libc::tcgetattr(libc::STDIN_FILENO, &raw mut termios) != 0 {
            return None;
        }
        let original = termios;
        termios.c_lflag &= !(libc::ECHO | libc::ICANON);
        libc::tcsetattr(libc::STDIN_FILENO, libc::TCSANOW, &raw const termios);
        Some(original)
    }
}

fn restore_termios(original: Option<libc::termios>) {
    if let Some(termios) = original {
        unsafe {
            libc::tcsetattr(libc::STDIN_FILENO, libc::TCSANOW, &raw const termios);
        }
    }
}

fn run() -> Result<(), Error> {
    let manager = HotkeyManager::new()?;
    let (tx, rx) = mpsc::channel::<String>();

    println!("kbd-global example — global hotkey listener");
    println!("Backend: {:?}", manager.active_backend());
    println!();

    let tx1 = tx.clone();
    let _save = manager.register(Hotkey::new(Key::S).modifier(Modifier::Ctrl), move || {
        let _ = tx1.send("Ctrl+S → Save!".to_string());
    })?;

    let tx2 = tx.clone();
    let _undo = manager.register(Hotkey::new(Key::Z).modifier(Modifier::Ctrl), move || {
        let _ = tx2.send("Ctrl+Z → Undo!".to_string());
    })?;

    let tx3 = tx.clone();
    let _help = manager.register(Hotkey::new(Key::F1), move || {
        let _ = tx3.send("F1 → Help!".to_string());
    })?;

    let tx4 = tx.clone();
    let _quit = manager.register(Hotkey::new(Key::Q).modifier(Modifier::Ctrl), move || {
        let _ = tx4.send("quit".to_string());
    })?;

    // F2 pushes the nav layer (handled on the main thread)
    let tx_nav = tx.clone();
    let _nav_toggle = manager.register(Hotkey::new(Key::F2), move || {
        let _ = tx_nav.send("push_nav".to_string());
    })?;

    // Nav layer: hjkl for arrows, Escape pops back
    let tx5 = tx.clone();
    let tx6 = tx.clone();
    let tx7 = tx.clone();
    let tx8 = tx.clone();
    let nav = Layer::new("nav")
        .bind(
            Hotkey::new(Key::H),
            Action::from(move || {
                let _ = tx5.send("[nav] H → Left".to_string());
            }),
        )
        .unwrap()
        .bind(
            Hotkey::new(Key::J),
            Action::from(move || {
                let _ = tx6.send("[nav] J → Down".to_string());
            }),
        )
        .unwrap()
        .bind(
            Hotkey::new(Key::K),
            Action::from(move || {
                let _ = tx7.send("[nav] K → Up".to_string());
            }),
        )
        .unwrap()
        .bind(
            Hotkey::new(Key::L),
            Action::from(move || {
                let _ = tx8.send("[nav] L → Right".to_string());
            }),
        )
        .unwrap()
        .bind(Hotkey::new(Key::ESCAPE), Action::PopLayer)
        .unwrap()
        .description("Navigation layer — hjkl as arrows, Escape to pop");
    manager.define_layer(nav)?;

    println!("Registered hotkeys (global — works in any window):");
    println!("  Ctrl+S  → Save");
    println!("  Ctrl+Z  → Undo");
    println!("  F1      → Help");
    println!("  F2      → Toggle nav layer");
    println!("  Ctrl+Q  → Quit");
    println!();
    println!("Nav layer (when active via F2):");
    println!("  H/J/K/L → arrow navigation");
    println!("  Escape  → exit nav layer");
    println!();
    println!("Listening for hotkeys... Ctrl+Q to exit.");
    println!();

    loop {
        match rx.recv() {
            Ok(msg) if msg == "quit" => {
                println!("Ctrl+Q → Quit!");
                break;
            }
            Ok(msg) if msg == "push_nav" => match manager.push_layer("nav") {
                Ok(()) => println!("F2 → nav layer ON (hjkl, Escape to exit)"),
                Err(e) => println!("F2 → failed to push layer: {e}"),
            },
            Ok(msg) => println!("{msg}"),
            Err(_) => break,
        }
    }

    manager.shutdown()?;
    println!("Shut down cleanly.");

    Ok(())
}
