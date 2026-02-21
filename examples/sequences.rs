//! Key sequences — multi-step hotkey combos.
//!
//! Sequences fire a callback only after the full chain of hotkeys is pressed
//! in order within a timeout. Think Emacs-style `Ctrl+K, Ctrl+C` or
//! VS Code's `Ctrl+K, Ctrl+T`.
//!
//! **Evdev backend only** — sequences require direct input access.
//!
//! ```sh
//! cargo run --example sequences
//! ```

use std::time::Duration;

use keybound::HotkeyManager;
use keybound::Key;
use keybound::SequenceOptions;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Key sequences example");
    println!();

    let manager = HotkeyManager::new()?;

    // Basic two-step sequence: Ctrl+K, Ctrl+C
    let seq = "Ctrl+K, Ctrl+C".parse()?;
    let _seq1 = manager.register_sequence(&seq, SequenceOptions::new(), || {
        println!("[Seq 1] Ctrl+K → Ctrl+C completed!");
    })?;
    println!("  Ctrl+K, Ctrl+C  → basic sequence (1s timeout)");

    // Custom timeout: give the user more time between steps
    let seq = "Ctrl+K, Ctrl+T".parse()?;
    let _seq2 = manager.register_sequence(
        &seq,
        SequenceOptions::new().timeout(Duration::from_secs(3)),
        || println!("[Seq 2] Ctrl+K → Ctrl+T completed! (3s timeout)"),
    )?;
    println!("  Ctrl+K, Ctrl+T  → 3 second timeout");

    // Three-step sequence
    let seq = "Ctrl+X, Ctrl+K, Ctrl+S".parse()?;
    let _seq3 = manager.register_sequence(
        &seq,
        SequenceOptions::new().timeout(Duration::from_secs(2)),
        || println!("[Seq 3] Ctrl+X → Ctrl+K → Ctrl+S completed!"),
    )?;
    println!("  Ctrl+X, Ctrl+K, Ctrl+S  → three-step sequence");

    // Custom abort key: press Tab to cancel instead of Escape
    let seq = "Alt+A, Alt+B".parse()?;
    let _seq4 =
        manager.register_sequence(&seq, SequenceOptions::new().abort_key(Key::Tab), || {
            println!("[Seq 4] Alt+A → Alt+B completed!");
        })?;
    println!("  Alt+A, Alt+B    → Tab aborts instead of Escape");

    // Sequence with timeout fallback: on timeout after the first step,
    // dispatch the partial match as a regular hotkey instead of dropping it
    let seq = "Ctrl+K, Ctrl+D".parse()?;
    let fallback_hotkey = "Ctrl+K".parse()?;
    let _seq5 = manager.register_sequence(
        &seq,
        SequenceOptions::new().timeout_fallback(fallback_hotkey),
        || println!("[Seq 5] Ctrl+K → Ctrl+D completed!"),
    )?;
    println!("  Ctrl+K, Ctrl+D  → Ctrl+K dispatched on timeout");

    println!();
    println!("Try pressing the sequences above. Escape aborts a pending sequence.");
    println!("Press Ctrl+C to exit");

    std::thread::park();
    Ok(())
}
