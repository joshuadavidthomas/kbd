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

use kbd_global::Action;
use kbd_global::Error;
use kbd_global::Hotkey;
use kbd_global::HotkeyManager;
use kbd_global::Key;
use kbd_global::Layer;
use kbd_global::Modifier;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    match run() {
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
    }
}

fn run() -> Result<(), Error> {
    let manager = HotkeyManager::new()?;
    let (tx, rx) = mpsc::channel::<String>();

    println!("kbd-global example — global hotkey listener");
    println!("Backend: {:?}", manager.active_backend());
    println!();

    // Register hotkeys that send events back to the main thread
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

    // Layer example
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
        .bind(
            Hotkey::new(Key::J),
            Action::from(move || {
                let _ = tx6.send("[nav] J → Down".to_string());
            }),
        )
        .bind(
            Hotkey::new(Key::K),
            Action::from(move || {
                let _ = tx7.send("[nav] K → Up".to_string());
            }),
        )
        .bind(
            Hotkey::new(Key::L),
            Action::from(move || {
                let _ = tx8.send("[nav] L → Right".to_string());
            }),
        )
        .bind(Hotkey::new(Key::ESCAPE), Action::PopLayer)
        .description("Navigation layer — hjkl as arrows, Escape to pop");
    manager.define_layer(nav)?;

    println!("Registered hotkeys (global — works in any window):");
    println!("  Ctrl+S  → Save");
    println!("  Ctrl+Z  → Undo");
    println!("  F1      → Help");
    println!("  Ctrl+Q  → Quit");
    println!();
    println!("Defined layer 'nav' (push with manager.push_layer):");
    println!("  H/J/K/L → arrow navigation");
    println!("  Escape  → pop layer");
    println!();
    println!("Listening for hotkeys... Ctrl+Q to exit.");
    println!();

    // Block on the channel, printing events as they arrive
    loop {
        match rx.recv() {
            Ok(msg) if msg == "quit" => {
                println!("Ctrl+Q → Quit!");
                break;
            }
            Ok(msg) => println!("{msg}"),
            Err(_) => break,
        }
    }

    manager.shutdown()?;
    println!("Shut down cleanly.");

    Ok(())
}
