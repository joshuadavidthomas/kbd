#![cfg_attr(docsrs, feature(doc_cfg))]

//! Egui key event conversions for `kbd`.
//!
//! This crate converts egui's key events into `kbd`'s unified types so
//! that GUI key events (from egui) and global hotkey events (from
//! [`kbd-global`](https://docs.rs/kbd-global)) can feed into the same
//! [`Dispatcher`](kbd::dispatcher::Dispatcher). This is useful in egui
//! apps that want both in-window shortcuts and system-wide hotkeys
//! handled through a single hotkey registry.
//!
//! Egui has a smaller, custom key enum that is not 1:1 with the W3C
//! specification — some physical keys have no egui equivalent, some egui
//! keys are logical/shifted characters without a single physical key
//! equivalent (e.g., `Colon`, `Pipe`, `Plus`), and egui combines some
//! concepts differently.
//!
//! # Extension traits
//!
//! - [`EguiKeyExt`] — converts an [`egui::Key`] to a [`kbd::key::Key`].
//! - [`EguiModifiersExt`] — converts [`egui::Modifiers`] to a
//!   [`ModifierSet`].
//! - [`EguiEventExt`] — converts a full [`egui::Event`] keyboard event
//!   to a [`kbd::hotkey::Hotkey`].
//!
//! # Key mapping
//!
//! | egui | kbd | Notes |
//! |---|---|---|
//! | `Key::A` – `Key::Z` | [`Key::A`] – [`Key::Z`] | Letters |
//! | `Key::Num0` – `Key::Num9` | [`Key::DIGIT0`] – [`Key::DIGIT9`] | Digits |
//! | `Key::F1` – `Key::F35` | [`Key::F1`] – [`Key::F35`] | Function keys |
//! | `Key::Minus`, `Key::Period`, … | [`Key::MINUS`], [`Key::PERIOD`], … | Physical-position punctuation |
//! | `Key::ArrowDown`, `Key::Enter`, … | [`Key::ARROW_DOWN`], [`Key::ENTER`], … | Navigation / editing |
//! | `Key::Copy`, `Key::Cut`, `Key::Paste` | [`Key::COPY`], [`Key::CUT`], [`Key::PASTE`] | Clipboard |
//! | `Key::Colon`, `Key::Pipe`, `Key::Plus`, … | `None` | Logical/shifted — no single physical key |
//!
//! # Modifier mapping
//!
//! | egui | kbd | Notes |
//! |---|---|---|
//! | `ctrl` | [`Modifier::Ctrl`] | |
//! | `shift` | [`Modifier::Shift`] | |
//! | `alt` | [`Modifier::Alt`] | |
//! | `mac_cmd` | [`Modifier::Super`] | Avoids double-counting with `command` on macOS |
//!
//! # Usage
//!
//! ```
//! use egui::{Key as EguiKey, Modifiers};
//! use kbd::prelude::*;
//! use kbd_egui::{EguiEventExt, EguiKeyExt, EguiModifiersExt};
//!
//! // Single key conversion
//! let key = EguiKey::A.to_key();
//! assert_eq!(key, Some(Key::A));
//!
//! // Modifier conversion
//! let mods = Modifiers::CTRL.to_modifiers();
//! assert_eq!(mods, ModifierSet::CTRL);
//!
//! // Full event conversion
//! let event = egui::Event::Key {
//!     key: EguiKey::C,
//!     physical_key: None,
//!     pressed: true,
//!     repeat: false,
//!     modifiers: Modifiers::CTRL,
//! };
//! let hotkey = event.to_hotkey();
//! assert_eq!(hotkey, Some(Hotkey::new(Key::C).modifier(Modifier::Ctrl)));
//! ```

use egui::Key as EguiKey;
use egui::Modifiers;
use kbd::hotkey::Hotkey;
use kbd::hotkey::Modifier;
use kbd::hotkey::ModifierSet;
use kbd::key::Key;

mod private {
    pub trait Sealed {}
    impl Sealed for egui::Key {}
    impl Sealed for egui::Modifiers {}
    impl Sealed for egui::Event {}
}

/// Convert an [`egui::Key`] to a `kbd` [`Key`].
///
/// Returns `None` for egui keys that represent logical/shifted characters
/// without a single physical key equivalent (e.g., `Colon`, `Pipe`,
/// `Plus`, `Questionmark`). Most egui keys map directly to a physical
/// key position.
///
/// This trait is sealed and cannot be implemented outside this crate.
pub trait EguiKeyExt: private::Sealed {
    /// Convert this egui key to a `kbd` [`Key`], or `None` if unmappable.
    ///
    /// # Examples
    ///
    /// ```
    /// use egui::Key as EguiKey;
    /// use kbd::prelude::*;
    /// use kbd_egui::EguiKeyExt;
    ///
    /// assert_eq!(EguiKey::A.to_key(), Some(Key::A));
    /// assert_eq!(EguiKey::F5.to_key(), Some(Key::F5));
    /// // Shifted characters have no physical key equivalent
    /// assert_eq!(EguiKey::Colon.to_key(), None);
    /// ```
    #[must_use]
    fn to_key(&self) -> Option<Key>;
}

impl EguiKeyExt for EguiKey {
    #[allow(clippy::too_many_lines)]
    fn to_key(&self) -> Option<Key> {
        match self {
            // Commands
            EguiKey::ArrowDown => Some(Key::ARROW_DOWN),
            EguiKey::ArrowLeft => Some(Key::ARROW_LEFT),
            EguiKey::ArrowRight => Some(Key::ARROW_RIGHT),
            EguiKey::ArrowUp => Some(Key::ARROW_UP),
            EguiKey::Escape => Some(Key::ESCAPE),
            EguiKey::Tab => Some(Key::TAB),
            EguiKey::Backspace => Some(Key::BACKSPACE),
            EguiKey::Enter => Some(Key::ENTER),
            EguiKey::Space => Some(Key::SPACE),
            EguiKey::Insert => Some(Key::INSERT),
            EguiKey::Delete => Some(Key::DELETE),
            EguiKey::Home => Some(Key::HOME),
            EguiKey::End => Some(Key::END),
            EguiKey::PageUp => Some(Key::PAGE_UP),
            EguiKey::PageDown => Some(Key::PAGE_DOWN),
            EguiKey::Copy => Some(Key::COPY),
            EguiKey::Cut => Some(Key::CUT),
            EguiKey::Paste => Some(Key::PASTE),

            // Punctuation — physical key positions
            EguiKey::Comma => Some(Key::COMMA),
            EguiKey::Backslash => Some(Key::BACKSLASH),
            EguiKey::Slash => Some(Key::SLASH),
            EguiKey::OpenBracket => Some(Key::BRACKET_LEFT),
            EguiKey::CloseBracket => Some(Key::BRACKET_RIGHT),
            EguiKey::Backtick => Some(Key::BACKQUOTE),
            EguiKey::Minus => Some(Key::MINUS),
            EguiKey::Period => Some(Key::PERIOD),
            EguiKey::Equals => Some(Key::EQUAL),
            EguiKey::Semicolon => Some(Key::SEMICOLON),
            EguiKey::Quote => Some(Key::QUOTE),

            // Punctuation — logical/shifted characters with no physical key equivalent.
            // These are produced by Shift+<physical key> and don't correspond to a
            // single physical key position.
            EguiKey::Colon
            | EguiKey::Pipe
            | EguiKey::Questionmark
            | EguiKey::Exclamationmark
            | EguiKey::Plus
            | EguiKey::OpenCurlyBracket
            | EguiKey::CloseCurlyBracket => None,

            // Digits
            EguiKey::Num0 => Some(Key::DIGIT0),
            EguiKey::Num1 => Some(Key::DIGIT1),
            EguiKey::Num2 => Some(Key::DIGIT2),
            EguiKey::Num3 => Some(Key::DIGIT3),
            EguiKey::Num4 => Some(Key::DIGIT4),
            EguiKey::Num5 => Some(Key::DIGIT5),
            EguiKey::Num6 => Some(Key::DIGIT6),
            EguiKey::Num7 => Some(Key::DIGIT7),
            EguiKey::Num8 => Some(Key::DIGIT8),
            EguiKey::Num9 => Some(Key::DIGIT9),

            // Letters
            EguiKey::A => Some(Key::A),
            EguiKey::B => Some(Key::B),
            EguiKey::C => Some(Key::C),
            EguiKey::D => Some(Key::D),
            EguiKey::E => Some(Key::E),
            EguiKey::F => Some(Key::F),
            EguiKey::G => Some(Key::G),
            EguiKey::H => Some(Key::H),
            EguiKey::I => Some(Key::I),
            EguiKey::J => Some(Key::J),
            EguiKey::K => Some(Key::K),
            EguiKey::L => Some(Key::L),
            EguiKey::M => Some(Key::M),
            EguiKey::N => Some(Key::N),
            EguiKey::O => Some(Key::O),
            EguiKey::P => Some(Key::P),
            EguiKey::Q => Some(Key::Q),
            EguiKey::R => Some(Key::R),
            EguiKey::S => Some(Key::S),
            EguiKey::T => Some(Key::T),
            EguiKey::U => Some(Key::U),
            EguiKey::V => Some(Key::V),
            EguiKey::W => Some(Key::W),
            EguiKey::X => Some(Key::X),
            EguiKey::Y => Some(Key::Y),
            EguiKey::Z => Some(Key::Z),

            // Function keys
            EguiKey::F1 => Some(Key::F1),
            EguiKey::F2 => Some(Key::F2),
            EguiKey::F3 => Some(Key::F3),
            EguiKey::F4 => Some(Key::F4),
            EguiKey::F5 => Some(Key::F5),
            EguiKey::F6 => Some(Key::F6),
            EguiKey::F7 => Some(Key::F7),
            EguiKey::F8 => Some(Key::F8),
            EguiKey::F9 => Some(Key::F9),
            EguiKey::F10 => Some(Key::F10),
            EguiKey::F11 => Some(Key::F11),
            EguiKey::F12 => Some(Key::F12),
            EguiKey::F13 => Some(Key::F13),
            EguiKey::F14 => Some(Key::F14),
            EguiKey::F15 => Some(Key::F15),
            EguiKey::F16 => Some(Key::F16),
            EguiKey::F17 => Some(Key::F17),
            EguiKey::F18 => Some(Key::F18),
            EguiKey::F19 => Some(Key::F19),
            EguiKey::F20 => Some(Key::F20),
            EguiKey::F21 => Some(Key::F21),
            EguiKey::F22 => Some(Key::F22),
            EguiKey::F23 => Some(Key::F23),
            EguiKey::F24 => Some(Key::F24),
            EguiKey::F25 => Some(Key::F25),
            EguiKey::F26 => Some(Key::F26),
            EguiKey::F27 => Some(Key::F27),
            EguiKey::F28 => Some(Key::F28),
            EguiKey::F29 => Some(Key::F29),
            EguiKey::F30 => Some(Key::F30),
            EguiKey::F31 => Some(Key::F31),
            EguiKey::F32 => Some(Key::F32),
            EguiKey::F33 => Some(Key::F33),
            EguiKey::F34 => Some(Key::F34),
            EguiKey::F35 => Some(Key::F35),

            // Browser keys
            EguiKey::BrowserBack => Some(Key::BROWSER_BACK),
        }
    }
}

/// Convert [`egui::Modifiers`] to a [`ModifierSet`].
///
/// Egui's `mac_cmd` and `command` fields are platform-dependent
/// abstractions. On non-macOS platforms, `command` mirrors `ctrl`.
/// This implementation maps `ctrl`, `shift`, `alt`, and either
/// `mac_cmd` or `command` (whichever represents the platform's
/// command key) to `kbd` modifiers.
///
/// To avoid double-counting on macOS (where `command` == `mac_cmd`),
/// we use `ctrl` and `mac_cmd` as the canonical sources:
/// - `ctrl` → `Modifier::Ctrl`
/// - `shift` → `Modifier::Shift`
/// - `alt` → `Modifier::Alt`
/// - `mac_cmd` → `Modifier::Super`
///
/// This trait is sealed and cannot be implemented outside this crate.
pub trait EguiModifiersExt: private::Sealed {
    /// Convert these egui modifiers to a [`ModifierSet`].
    ///
    /// # Examples
    ///
    /// ```
    /// use egui::Modifiers;
    /// use kbd::prelude::*;
    /// use kbd_egui::EguiModifiersExt;
    ///
    /// let mods = Modifiers {
    ///     alt: false, ctrl: true, shift: true,
    ///     mac_cmd: false, command: false,
    /// };
    /// assert_eq!(mods.to_modifiers(), ModifierSet::CTRL.with(Modifier::Shift));
    /// ```
    #[must_use]
    fn to_modifiers(&self) -> ModifierSet;
}

impl EguiModifiersExt for Modifiers {
    fn to_modifiers(&self) -> ModifierSet {
        Modifier::collect_active([
            (self.ctrl, Modifier::Ctrl),
            (self.shift, Modifier::Shift),
            (self.alt, Modifier::Alt),
            (self.mac_cmd, Modifier::Super),
        ])
    }
}

/// Convert an [`egui::Event`] keyboard event to a `kbd` [`Hotkey`].
///
/// Returns `None` if the event is not a keyboard event, or if the key
/// has no `kbd` equivalent.
///
/// Only `Event::Key { .. }` variants produce a hotkey. All other event
/// variants return `None`.
///
/// This trait is sealed and cannot be implemented outside this crate.
pub trait EguiEventExt: private::Sealed {
    /// Convert this event to a [`Hotkey`], or `None` if not a keyboard event.
    ///
    /// # Examples
    ///
    /// ```
    /// use egui::{Key as EguiKey, Modifiers};
    /// use kbd::prelude::*;
    /// use kbd_egui::EguiEventExt;
    ///
    /// let event = egui::Event::Key {
    ///     key: EguiKey::S,
    ///     physical_key: None,
    ///     pressed: true,
    ///     repeat: false,
    ///     modifiers: Modifiers::CTRL,
    /// };
    /// assert_eq!(
    ///     event.to_hotkey(),
    ///     Some(Hotkey::new(Key::S).modifier(Modifier::Ctrl)),
    /// );
    /// ```
    #[must_use]
    fn to_hotkey(&self) -> Option<Hotkey>;
}

impl EguiEventExt for egui::Event {
    fn to_hotkey(&self) -> Option<Hotkey> {
        if let egui::Event::Key { key, modifiers, .. } = self {
            let kbd_key = key.to_key()?;
            let mods = modifiers.to_modifiers();
            Some(Hotkey::with_modifiers(kbd_key, mods))
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use egui::Key as EguiKey;
    use egui::Modifiers;
    use kbd::hotkey::Hotkey;
    use kbd::hotkey::Modifier;
    use kbd::key::Key;

    use super::*;

    // EguiKeyExt tests

    #[test]
    fn letter_keys() {
        assert_eq!(EguiKey::A.to_key(), Some(Key::A));
        assert_eq!(EguiKey::B.to_key(), Some(Key::B));
        assert_eq!(EguiKey::Z.to_key(), Some(Key::Z));
    }

    #[test]
    fn digit_keys() {
        assert_eq!(EguiKey::Num0.to_key(), Some(Key::DIGIT0));
        assert_eq!(EguiKey::Num1.to_key(), Some(Key::DIGIT1));
        assert_eq!(EguiKey::Num9.to_key(), Some(Key::DIGIT9));
    }

    #[test]
    fn function_keys() {
        assert_eq!(EguiKey::F1.to_key(), Some(Key::F1));
        assert_eq!(EguiKey::F12.to_key(), Some(Key::F12));
        assert_eq!(EguiKey::F20.to_key(), Some(Key::F20));
        assert_eq!(EguiKey::F35.to_key(), Some(Key::F35));
    }

    #[test]
    fn navigation_keys() {
        assert_eq!(EguiKey::ArrowDown.to_key(), Some(Key::ARROW_DOWN));
        assert_eq!(EguiKey::ArrowUp.to_key(), Some(Key::ARROW_UP));
        assert_eq!(EguiKey::ArrowLeft.to_key(), Some(Key::ARROW_LEFT));
        assert_eq!(EguiKey::ArrowRight.to_key(), Some(Key::ARROW_RIGHT));
        assert_eq!(EguiKey::Home.to_key(), Some(Key::HOME));
        assert_eq!(EguiKey::End.to_key(), Some(Key::END));
        assert_eq!(EguiKey::PageUp.to_key(), Some(Key::PAGE_UP));
        assert_eq!(EguiKey::PageDown.to_key(), Some(Key::PAGE_DOWN));
    }

    #[test]
    fn command_keys() {
        assert_eq!(EguiKey::Escape.to_key(), Some(Key::ESCAPE));
        assert_eq!(EguiKey::Tab.to_key(), Some(Key::TAB));
        assert_eq!(EguiKey::Backspace.to_key(), Some(Key::BACKSPACE));
        assert_eq!(EguiKey::Enter.to_key(), Some(Key::ENTER));
        assert_eq!(EguiKey::Space.to_key(), Some(Key::SPACE));
        assert_eq!(EguiKey::Insert.to_key(), Some(Key::INSERT));
        assert_eq!(EguiKey::Delete.to_key(), Some(Key::DELETE));
    }

    #[test]
    fn clipboard_keys() {
        assert_eq!(EguiKey::Copy.to_key(), Some(Key::COPY));
        assert_eq!(EguiKey::Cut.to_key(), Some(Key::CUT));
        assert_eq!(EguiKey::Paste.to_key(), Some(Key::PASTE));
    }

    #[test]
    fn punctuation_keys() {
        assert_eq!(EguiKey::Minus.to_key(), Some(Key::MINUS));
        assert_eq!(EguiKey::Period.to_key(), Some(Key::PERIOD));
        assert_eq!(EguiKey::Comma.to_key(), Some(Key::COMMA));
        assert_eq!(EguiKey::Semicolon.to_key(), Some(Key::SEMICOLON));
        assert_eq!(EguiKey::Backslash.to_key(), Some(Key::BACKSLASH));
        assert_eq!(EguiKey::Slash.to_key(), Some(Key::SLASH));
        assert_eq!(EguiKey::Backtick.to_key(), Some(Key::BACKQUOTE));
        assert_eq!(EguiKey::OpenBracket.to_key(), Some(Key::BRACKET_LEFT));
        assert_eq!(EguiKey::CloseBracket.to_key(), Some(Key::BRACKET_RIGHT));
        assert_eq!(EguiKey::Equals.to_key(), Some(Key::EQUAL));
        assert_eq!(EguiKey::Quote.to_key(), Some(Key::QUOTE));
    }

    #[test]
    fn browser_back_key() {
        assert_eq!(EguiKey::BrowserBack.to_key(), Some(Key::BROWSER_BACK));
    }

    #[test]
    fn keys_with_no_physical_equivalent_return_none() {
        // Egui has some keys that don't map cleanly to physical positions.
        // Colon, Pipe, Questionmark, Exclamationmark, Plus are logical/shifted
        // characters, not physical key positions.
        assert_eq!(EguiKey::Colon.to_key(), None);
        assert_eq!(EguiKey::Pipe.to_key(), None);
        assert_eq!(EguiKey::Questionmark.to_key(), None);
        assert_eq!(EguiKey::Exclamationmark.to_key(), None);
        assert_eq!(EguiKey::Plus.to_key(), None);
        assert_eq!(EguiKey::OpenCurlyBracket.to_key(), None);
        assert_eq!(EguiKey::CloseCurlyBracket.to_key(), None);
    }

    // EguiModifiersExt tests

    #[test]
    fn empty_modifiers() {
        assert_eq!(Modifiers::NONE.to_modifiers(), ModifierSet::EMPTY);
    }

    #[test]
    fn single_ctrl_modifier() {
        assert_eq!(Modifiers::CTRL.to_modifiers(), ModifierSet::CTRL);
    }

    #[test]
    fn single_shift_modifier() {
        assert_eq!(Modifiers::SHIFT.to_modifiers(), ModifierSet::SHIFT);
    }

    #[test]
    fn single_alt_modifier() {
        assert_eq!(Modifiers::ALT.to_modifiers(), ModifierSet::ALT);
    }

    #[test]
    fn mac_cmd_maps_to_super() {
        let mods = Modifiers {
            alt: false,
            ctrl: false,
            shift: false,
            mac_cmd: true,
            command: true,
        };
        assert_eq!(mods.to_modifiers(), ModifierSet::SUPER);
    }

    #[test]
    fn combined_modifiers() {
        let mods = Modifiers {
            alt: false,
            ctrl: true,
            shift: true,
            mac_cmd: false,
            command: false,
        };
        assert_eq!(mods.to_modifiers(), ModifierSet::CTRL.with(Modifier::Shift));
    }

    #[test]
    fn all_modifiers() {
        let mods = Modifiers {
            alt: true,
            ctrl: true,
            shift: true,
            mac_cmd: true,
            command: true,
        };
        assert_eq!(
            mods.to_modifiers(),
            ModifierSet::EMPTY
                .with(Modifier::Ctrl)
                .with(Modifier::Shift)
                .with(Modifier::Alt)
                .with(Modifier::Super)
        );
    }

    // EguiEventExt tests

    #[test]
    fn key_event_to_hotkey() {
        let event = egui::Event::Key {
            key: EguiKey::C,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE,
        };
        assert_eq!(event.to_hotkey(), Some(Hotkey::new(Key::C)));
    }

    #[test]
    fn key_event_with_ctrl() {
        let event = egui::Event::Key {
            key: EguiKey::C,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::CTRL,
        };
        assert_eq!(
            event.to_hotkey(),
            Some(Hotkey::new(Key::C).modifier(Modifier::Ctrl))
        );
    }

    #[test]
    fn key_event_with_multiple_modifiers() {
        let event = egui::Event::Key {
            key: EguiKey::A,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers {
                alt: false,
                ctrl: true,
                shift: true,
                mac_cmd: false,
                command: false,
            },
        };
        assert_eq!(
            event.to_hotkey(),
            Some(
                Hotkey::new(Key::A)
                    .modifier(Modifier::Ctrl)
                    .modifier(Modifier::Shift)
            )
        );
    }

    #[test]
    fn non_key_event_returns_none() {
        let event = egui::Event::PointerMoved(egui::pos2(10.0, 20.0));
        assert_eq!(event.to_hotkey(), None);
    }

    #[test]
    fn unmappable_key_event_returns_none() {
        let event = egui::Event::Key {
            key: EguiKey::Colon,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE,
        };
        assert_eq!(event.to_hotkey(), None);
    }

    #[test]
    fn ctrl_shift_f5() {
        let event = egui::Event::Key {
            key: EguiKey::F5,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers {
                alt: false,
                ctrl: true,
                shift: true,
                mac_cmd: false,
                command: false,
            },
        };
        assert_eq!(
            event.to_hotkey(),
            Some(
                Hotkey::new(Key::F5)
                    .modifier(Modifier::Ctrl)
                    .modifier(Modifier::Shift)
            )
        );
    }

    #[test]
    fn space_event() {
        let event = egui::Event::Key {
            key: EguiKey::Space,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE,
        };
        assert_eq!(event.to_hotkey(), Some(Hotkey::new(Key::SPACE)));
    }
}
