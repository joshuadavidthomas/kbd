//! Read real evdev key events from keyboard devices and feed them to a
//! `Matcher`.
//!
//! Requires read access to `/dev/input/` (typically via the `input` group
//! or running as root).
//!
//! ```sh
//! cargo run -p kbd-evdev --example evdev
//! ```

use kbd::Action;
use kbd::Hotkey;
use kbd::Key;
use kbd::KeyTransition;
use kbd::MatchResult;
use kbd::Matcher;
use kbd_evdev::devices::DeviceGrabMode;
use kbd_evdev::devices::DeviceManager;

fn main() {
    println!("kbd-evdev example — press keys to see matches");
    println!();

    let mut matcher = Matcher::new();

    matcher
        .register(
            Hotkey::new(Key::A),
            Action::from(|| println!("  → A pressed!")),
        )
        .expect("register A");
    matcher
        .register(
            Hotkey::new(Key::ESCAPE),
            Action::from(|| println!("  → Escape!")),
        )
        .expect("register Escape");
    matcher
        .register(
            Hotkey::new(Key::SPACE),
            Action::from(|| println!("  → Space!")),
        )
        .expect("register Space");
    matcher
        .register(
            Hotkey::new(Key::ENTER),
            Action::from(|| println!("  → Enter!")),
        )
        .expect("register Enter");

    println!("Bindings:");
    println!("  A       → A pressed");
    println!("  Space   → Space");
    println!("  Enter   → Enter");
    println!("  Escape  → Escape (also exits)");
    println!();

    let mut manager =
        DeviceManager::new(std::path::Path::new("/dev/input"), DeviceGrabMode::Shared);

    if manager.poll_fds().is_empty() {
        eprintln!("No keyboard devices found.");
        eprintln!();
        eprintln!("Tip: add your user to the 'input' group:");
        eprintln!("  sudo usermod -aG input $USER");
        std::process::exit(1);
    }

    println!("Listening for key events... press Escape to exit.");
    println!();

    loop {
        let mut pollfds: Vec<libc::pollfd> = manager
            .poll_fds()
            .iter()
            .map(|&fd| libc::pollfd {
                fd,
                events: libc::POLLIN,
                revents: 0,
            })
            .collect();

        let ret = unsafe { libc::poll(pollfds.as_mut_ptr(), pollfds.len() as _, -1) };
        if ret < 0 {
            eprintln!("poll error: {}", std::io::Error::last_os_error());
            break;
        }

        let result = manager.process_polled_events(&pollfds);

        for event in &result.key_events {
            let hotkey = Hotkey::new(event.key);

            match event.transition {
                KeyTransition::Press => {
                    print!("{hotkey}: ");
                    match matcher.process(&hotkey, event.transition) {
                        MatchResult::Matched { action, .. } => {
                            if let Action::Callback(cb) = action {
                                cb();
                            }
                            if event.key == Key::ESCAPE {
                                println!("Exiting...");
                                return;
                            }
                        }
                        MatchResult::NoMatch => println!("no match"),
                        MatchResult::Suppressed => println!("suppressed"),
                        MatchResult::Pending { .. } => println!("pending..."),
                        MatchResult::Ignored => println!("ignored"),
                    }
                }
                KeyTransition::Release | KeyTransition::Repeat => {
                    matcher.process(&hotkey, event.transition);
                }
            }
        }

        for fd in &result.disconnected_devices {
            println!("Device disconnected (fd {fd})");
        }
    }
}
