//! Tap-hold — dual-function keys.
//!
//! A single key behaves differently based on how long it's held:
//! - **Tap** (quick press+release): emits a synthetic key event
//! - **Hold** (past threshold): acts as a modifier
//!
//! Classic use case: `CapsLock` = tap for Escape, hold for Ctrl.
//!
//! Requires **event grabbing** (the `grab` feature and builder config).
//!
//! ```sh
//! cargo run --example tap_hold --features grab
//! ```

use std::time::Duration;

use keybound::HoldAction;
use keybound::HotkeyManager;
use keybound::Key;
use keybound::Modifier;
use keybound::TapAction;
use keybound::TapHoldOptions;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Tap-hold (dual-function keys) example");
    println!();

    // Tap-hold requires event grabbing
    let manager = HotkeyManager::builder().grab().build()?;

    // CapsLock: tap = Escape, hold = Ctrl
    let _caps = manager.register_tap_hold(
        Key::CapsLock,
        TapAction::emit(Key::Escape),
        HoldAction::modifier(Modifier::Ctrl),
        TapHoldOptions::new().threshold(Duration::from_millis(200)),
    )?;
    println!("  CapsLock:");
    println!("    tap (<200ms)  → Escape");
    println!("    hold (≥200ms) → Ctrl");

    // Tab: tap = Tab, hold = Alt
    let _tab = manager.register_tap_hold(
        Key::Tab,
        TapAction::emit(Key::Tab),
        HoldAction::modifier(Modifier::Alt),
        TapHoldOptions::new(), // uses default 200ms threshold
    )?;
    println!("  Tab:");
    println!("    tap  → Tab");
    println!("    hold → Alt");

    // Enter: tap = Enter, hold = Shift (useful for one-handed typing)
    let _enter = manager.register_tap_hold(
        Key::Enter,
        TapAction::emit(Key::Enter),
        HoldAction::modifier(Modifier::Shift),
        TapHoldOptions::new().threshold(Duration::from_millis(150)),
    )?;
    println!("  Enter:");
    println!("    tap (<150ms)  → Enter");
    println!("    hold (≥150ms) → Shift");

    println!();
    println!("How it works:");
    println!("  - Tap quickly: the tap action (synthetic key) is emitted on release");
    println!("  - Hold past threshold: the hold action (modifier) activates");
    println!("  - Typing another key while holding: resolves as hold immediately");
    println!();
    println!("Press Ctrl+C to exit (real Ctrl, or hold CapsLock + C)");

    std::thread::park();
    Ok(())
}
