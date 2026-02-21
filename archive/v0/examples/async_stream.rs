//! Async event stream — receive hotkey events in a tokio task.
//!
//! Instead of (or in addition to) registering callbacks, you can consume
//! events from an async stream. Every press, release, sequence step, and
//! mode change is surfaced as a `HotkeyEvent`.
//!
//! Requires the `tokio` feature.
//!
//! ```sh
//! cargo run --example async_stream --features tokio
//! ```

use keybound::HotkeyEvent;
use keybound::HotkeyManager;
use keybound::HotkeyOptions;
use keybound::Key;
use keybound::Modifier;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;

    rt.block_on(async_main())
}

async fn async_main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Async event stream example");
    println!();

    let manager = HotkeyManager::new()?;

    // Register a hotkey with a callback — it still fires alongside the stream
    let _press = manager.register(Key::A, &[Modifier::Ctrl], || {
        println!("[callback] Ctrl+A pressed");
    })?;

    // Register one with release events too
    let _release = manager.register_with_options(
        Key::B,
        &[Modifier::Ctrl],
        HotkeyOptions::new().on_release(),
        || println!("[callback] Ctrl+B"),
    )?;

    println!("  Ctrl+A  → press only");
    println!("  Ctrl+B  → press + release");
    println!();

    // Subscribe to the event stream
    let mut stream = manager.event_stream();

    println!("Listening for events (Ctrl+C to exit)...");
    println!();

    // Process events in an async loop
    while let Some(event) = stream.next().await {
        match &event {
            HotkeyEvent::Pressed(hotkey) => {
                println!("[stream] pressed: {hotkey}");
            }
            HotkeyEvent::Released(hotkey) => {
                println!("[stream] released: {hotkey}");
            }
            HotkeyEvent::SequenceStep { id, step, total } => {
                println!("[stream] sequence {id}: step {step}/{total}");
            }
            HotkeyEvent::ModeChanged(mode) => {
                println!("[stream] mode changed: {mode:?}");
            }
        }
    }

    Ok(())
}
