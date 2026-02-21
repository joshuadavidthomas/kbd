//! Hotkey options: release callbacks, hold duration, repeat, debounce, and rate limiting.
//!
//! Demonstrates the full range of `HotkeyOptions` for controlling when and how
//! callbacks fire — from release events to invocation throttling.
//!
//! ```sh
//! cargo run --example options
//! ```

use std::time::Duration;

use keybound::HotkeyManager;
use keybound::HotkeyOptions;
use keybound::Key;
use keybound::Modifier;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Hotkey options example");
    println!();

    let manager = HotkeyManager::new()?;

    // 1. Release callback: fires on both press and release
    let _release = manager.register_with_options(
        Key::F1,
        &[Modifier::Ctrl],
        HotkeyOptions::new().on_release(),
        || println!("[F1] Press AND release (same callback)"),
    )?;
    println!("  Ctrl+F1  → fires on press and release");

    // 2. Separate release callback: different behavior on press vs release
    let _separate_release = manager.register_with_options(
        Key::F2,
        &[Modifier::Ctrl],
        HotkeyOptions::new().on_release_callback(|| {
            println!("[F2] Released!");
        }),
        || println!("[F2] Pressed!"),
    )?;
    println!("  Ctrl+F2  → separate press/release callbacks");

    // 3. Minimum hold duration: only fires if held long enough
    let _hold = manager.register_with_options(
        Key::F3,
        &[Modifier::Ctrl],
        HotkeyOptions::new().min_hold(Duration::from_millis(500)),
        || println!("[F3] Held for 500ms!"),
    )?;
    println!("  Ctrl+F3  → requires 500ms hold");

    // 4. Trigger on key repeat: fires continuously while held
    let _repeat = manager.register_with_options(
        Key::F4,
        &[Modifier::Ctrl],
        HotkeyOptions::new().trigger_on_repeat(),
        || println!("[F4] Repeat!"),
    )?;
    println!("  Ctrl+F4  → fires on key repeat (autorepeat)");

    // 5. Debounce: suppress rapid retriggers (great for noisy keys)
    let _debounce = manager.register_with_options(
        Key::F5,
        &[Modifier::Ctrl],
        HotkeyOptions::new().debounce(Duration::from_millis(200)),
        || println!("[F5] Debounced (200ms quiet time required)"),
    )?;
    println!("  Ctrl+F5  → debounced at 200ms");

    // 6. Rate limiting: cap invocation frequency
    let _rate = manager.register_with_options(
        Key::F6,
        &[Modifier::Ctrl],
        HotkeyOptions::new().max_rate(Duration::from_secs(1)),
        || println!("[F6] Rate-limited (at most 1/sec)"),
    )?;
    println!("  Ctrl+F6  → rate limited to once per second");

    // 7. Combined: debounce + rate limit + repeat
    let _combo = manager.register_with_options(
        Key::F7,
        &[Modifier::Ctrl],
        HotkeyOptions::new()
            .trigger_on_repeat()
            .debounce(Duration::from_millis(50))
            .max_rate(Duration::from_millis(250)),
        || println!("[F7] Debounced + rate-limited repeat"),
    )?;
    println!("  Ctrl+F7  → repeat + debounce(50ms) + rate(250ms)");

    println!();
    println!("Press Ctrl+C to exit");

    std::thread::park();
    Ok(())
}
