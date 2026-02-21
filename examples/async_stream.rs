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
use keybound::ModeOptions;
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

    // Register hotkeys as usual — callbacks still work
    let _press = manager.register(Key::A, &[Modifier::Ctrl], || {
        println!("[callback] Ctrl+A pressed");
    })?;

    // With release events
    let _release = manager.register_with_options(
        Key::B,
        &[Modifier::Ctrl],
        HotkeyOptions::new().on_release(),
        || println!("[callback] Ctrl+B"),
    )?;

    // A mode for demonstrating ModeChanged events
    let controller = manager.mode_controller();
    manager.define_mode("demo", ModeOptions::new().oneshot(), |mode| {
        mode.register(Key::X, &[], || println!("[callback] mode: X pressed"))?;
        Ok(())
    })?;

    let mode_controller = controller.clone();
    let _mode_trigger = manager.register(Key::M, &[Modifier::Ctrl], move || {
        mode_controller.push("demo");
    })?;

    println!("  Ctrl+A  → press event");
    println!("  Ctrl+B  → press + release events");
    println!("  Ctrl+M  → push 'demo' mode (ModeChanged event)");
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
