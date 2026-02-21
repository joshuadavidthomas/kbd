//! Modes/layers — context-sensitive hotkey groups.
//!
//! Modes let you define named groups of hotkeys that activate on demand.
//! When a mode is active, its bindings take precedence. Modes stack: push
//! one on, pop it off to return to the previous layer.
//!
//! Think of it like Vim's normal/insert modes, or a "resize" mode in a
//! tiling window manager.
//!
//! **Evdev backend only** — modes require direct input access.
//!
//! ```sh
//! cargo run --example modes
//! ```

use std::time::Duration;

use keybound::HotkeyManager;
use keybound::Key;
use keybound::ModeOptions;
use keybound::Modifier;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Modes/layers example");
    println!();

    let manager = HotkeyManager::new()?;
    let controller = manager.mode_controller();

    // Define a "resize" mode with h/j/k/l bindings
    let exit_controller = controller.clone();
    manager.define_mode("resize", ModeOptions::new(), |mode| {
        mode.register(Key::H, &[], || println!("  [resize] ← shrink left"))?;
        mode.register(Key::J, &[], || println!("  [resize] ↓ shrink down"))?;
        mode.register(Key::K, &[], || println!("  [resize] ↑ grow up"))?;
        mode.register(Key::L, &[], || println!("  [resize] → grow right"))?;

        // Escape exits resize mode
        let ctl = exit_controller.clone();
        mode.register(Key::Escape, &[], move || {
            ctl.pop();
            println!("  [resize] exited");
        })?;

        Ok(())
    })?;

    // Super+R enters resize mode
    let resize_controller = controller.clone();
    let _enter_resize = manager.register(Key::R, &[Modifier::Super], move || {
        resize_controller.push("resize");
        println!("Entered resize mode! Use h/j/k/l, Escape to exit");
    })?;
    println!("  Super+R  → enter resize mode");
    println!("    h/j/k/l  → resize actions");
    println!("    Escape   → exit resize mode");
    println!();

    // Oneshot mode: auto-pops after one keypress
    manager.define_mode("launcher", ModeOptions::new().oneshot(), |mode| {
        mode.register(Key::F, &[], || println!("  [launcher] open file manager"))?;
        mode.register(Key::B, &[], || println!("  [launcher] open browser"))?;
        mode.register(Key::T, &[], || println!("  [launcher] open terminal"))?;
        Ok(())
    })?;

    let launcher_controller = controller.clone();
    let _enter_launcher = manager.register(Key::Space, &[Modifier::Super], move || {
        launcher_controller.push("launcher");
        println!("Launcher mode! Press f/b/t (auto-exits after one key)");
    })?;
    println!("  Super+Space  → oneshot launcher mode");
    println!("    f → file manager, b → browser, t → terminal");
    println!();

    // Swallow mode: suppresses non-matching keypresses
    manager.define_mode("passthrough_block", ModeOptions::new().swallow(), |mode| {
        let ctl = mode.mode_controller();
        mode.register(Key::Escape, &[], move || {
            ctl.pop();
            println!("  [block] exited");
        })?;
        mode.register(Key::Y, &[], || println!("  [block] confirmed: YES"))?;
        mode.register(Key::N, &[], || println!("  [block] confirmed: NO"))?;
        Ok(())
    })?;

    let block_controller = controller.clone();
    let _enter_block = manager.register(Key::D, &[Modifier::Super], move || {
        block_controller.push("passthrough_block");
        println!("Confirm mode! Only y/n/Escape work (all other keys swallowed)");
    })?;
    println!("  Super+D  → swallow mode (only y/n/Escape work)");
    println!();

    // Mode with timeout: auto-pops after inactivity
    manager.define_mode(
        "quick_nav",
        ModeOptions::new().timeout(Duration::from_secs(3)),
        |mode| {
            mode.register(Key::Num1, &[], || println!("  [nav] workspace 1"))?;
            mode.register(Key::Num2, &[], || println!("  [nav] workspace 2"))?;
            mode.register(Key::Num3, &[], || println!("  [nav] workspace 3"))?;
            Ok(())
        },
    )?;

    let nav_controller = controller.clone();
    let _enter_nav = manager.register(Key::N, &[Modifier::Super], move || {
        nav_controller.push("quick_nav");
        println!("Quick nav mode! Press 1/2/3 (auto-exits after 3s idle)");
    })?;
    println!("  Super+N  → timeout mode (3s auto-exit)");
    println!("    1/2/3 → navigate workspaces");
    println!();

    // You can query the active mode at any time
    println!("Active mode: {:?}", controller.active_mode());

    println!("Press Ctrl+C to exit");

    std::thread::park();
    Ok(())
}
