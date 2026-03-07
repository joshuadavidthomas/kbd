#![cfg_attr(docsrs, feature(doc_cfg))]

//! Crossterm key event conversions for `kbd`.
//!
//! This crate converts crossterm's key events into `kbd`'s unified types
//! so you can use `kbd`'s [`Dispatcher`](kbd::dispatcher::Dispatcher),
//! hotkey parsing, layers, and sequences in a TUI app.
//!
//! Crossterm reports keys as characters (`Char('a')`) and modifier
//! bitflags, while `kbd` uses physical key positions (`Key::A`) and
//! typed `Modifier` values.
//!
//! # Extension traits
//!
//! - [`CrosstermKeyExt`] — converts a [`crossterm::event::KeyCode`] to a
//!   [`kbd::key::Key`].
//! - [`CrosstermModifiersExt`] — converts [`crossterm::event::KeyModifiers`]
//!   to a [`ModifierSet`].
//! - [`CrosstermEventExt`] — converts a full [`crossterm::event::KeyEvent`]
//!   to a [`kbd::hotkey::Hotkey`].
//!
//! # Key mapping
//!
//! | Crossterm | kbd | Notes |
//! |---|---|---|
//! | `Char('a')` – `Char('z')` | [`Key::A`] – [`Key::Z`] | Case-insensitive |
//! | `Char('0')` – `Char('9')` | [`Key::DIGIT0`] – [`Key::DIGIT9`] | |
//! | `Char('-')`, `Char('=')`, … | [`Key::MINUS`], [`Key::EQUAL`], … | Physical position |
//! | `F(1)` – `F(35)` | [`Key::F1`] – [`Key::F35`] | `F(0)` and `F(36+)` → `None` |
//! | `Enter`, `Esc`, `Tab`, … | [`Key::ENTER`], [`Key::ESCAPE`], [`Key::TAB`], … | Named keys |
//! | `Media(PlayPause)`, … | [`Key::MEDIA_PLAY_PAUSE`], … | Media keys |
//! | `Modifier(LeftControl)`, … | [`Key::CONTROL_LEFT`], … | Modifier keys as triggers |
//! | `BackTab`, `Null`, `KeypadBegin` | `None` | No `kbd` equivalent |
//! | Non-ASCII `Char` (e.g., `'é'`) | `None` | No physical key mapping |
//!
//! # Modifier mapping
//!
//! | Crossterm | kbd |
//! |---|---|
//! | `CONTROL` | [`Modifier::Ctrl`] |
//! | `SHIFT` | [`Modifier::Shift`] |
//! | `ALT` | [`Modifier::Alt`] |
//! | `SUPER` | [`Modifier::Super`] |
//! | `HYPER`, `META` | *(ignored)* |
//!
//! # Usage
//!
//! ```
//! use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
//! use kbd::prelude::*;
//! use kbd_crossterm::{CrosstermEventExt, CrosstermKeyExt, CrosstermModifiersExt};
//!
//! // Single key conversion
//! let key = KeyCode::Char('a').to_key();
//! assert_eq!(key, Some(Key::A));
//!
//! // Modifier conversion
//! let mods = KeyModifiers::CONTROL.to_modifiers();
//! assert_eq!(mods, ModifierSet::CTRL);
//!
//! // Full event conversion
//! let event = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
//! let hotkey = event.to_hotkey();
//! assert_eq!(hotkey, Some(Hotkey::new(Key::C).modifier(Modifier::Ctrl)));
//! ```

use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyModifiers;
use crossterm::event::MediaKeyCode;
use crossterm::event::ModifierKeyCode;
use kbd::hotkey::Hotkey;
use kbd::hotkey::Modifier;
use kbd::hotkey::ModifierSet;
use kbd::key::Key;

mod private {
    pub trait Sealed {}
    impl Sealed for crossterm::event::KeyCode {}
    impl Sealed for crossterm::event::KeyModifiers {}
    impl Sealed for crossterm::event::KeyEvent {}
}

/// Convert a crossterm [`KeyCode`] to a `kbd` [`Key`].
///
/// Returns `None` for keys that have no `kbd` equivalent (e.g.,
/// `BackTab`, `Null`, `KeypadBegin`, non-ASCII characters).
///
/// This trait is sealed and cannot be implemented outside this crate.
pub trait CrosstermKeyExt: private::Sealed {
    /// Convert this key code to a `kbd` [`Key`], or `None` if unmappable.
    ///
    /// # Examples
    ///
    /// ```
    /// use crossterm::event::KeyCode;
    /// use kbd::prelude::*;
    /// use kbd_crossterm::CrosstermKeyExt;
    ///
    /// assert_eq!(KeyCode::Char('a').to_key(), Some(Key::A));
    /// assert_eq!(KeyCode::F(5).to_key(), Some(Key::F5));
    /// assert_eq!(KeyCode::Null.to_key(), None);
    /// ```
    #[must_use]
    fn to_key(&self) -> Option<Key>;
}

impl CrosstermKeyExt for KeyCode {
    fn to_key(&self) -> Option<Key> {
        match self {
            KeyCode::Char(ch) => char_to_key(*ch),
            KeyCode::F(n) => function_key(*n),
            KeyCode::Enter => Some(Key::ENTER),
            KeyCode::Esc => Some(Key::ESCAPE),
            KeyCode::Backspace => Some(Key::BACKSPACE),
            KeyCode::Tab => Some(Key::TAB),
            KeyCode::Delete => Some(Key::DELETE),
            KeyCode::Insert => Some(Key::INSERT),
            KeyCode::Home => Some(Key::HOME),
            KeyCode::End => Some(Key::END),
            KeyCode::PageUp => Some(Key::PAGE_UP),
            KeyCode::PageDown => Some(Key::PAGE_DOWN),
            KeyCode::Up => Some(Key::ARROW_UP),
            KeyCode::Down => Some(Key::ARROW_DOWN),
            KeyCode::Left => Some(Key::ARROW_LEFT),
            KeyCode::Right => Some(Key::ARROW_RIGHT),
            KeyCode::CapsLock => Some(Key::CAPS_LOCK),
            KeyCode::ScrollLock => Some(Key::SCROLL_LOCK),
            KeyCode::NumLock => Some(Key::NUM_LOCK),
            KeyCode::PrintScreen => Some(Key::PRINT_SCREEN),
            KeyCode::Pause => Some(Key::PAUSE),
            KeyCode::Menu => Some(Key::CONTEXT_MENU),
            KeyCode::Media(media) => media_to_key(*media),
            KeyCode::Modifier(modifier) => modifier_keycode_to_key(*modifier),
            KeyCode::BackTab | KeyCode::Null | KeyCode::KeypadBegin => None,
        }
    }
}

/// Convert crossterm [`KeyModifiers`] bitflags to a [`ModifierSet`].
///
/// Crossterm's `HYPER` and `META` flags have no `kbd` equivalent and
/// are silently ignored.
///
/// This trait is sealed and cannot be implemented outside this crate.
pub trait CrosstermModifiersExt: private::Sealed {
    /// Convert these modifier flags to a [`ModifierSet`].
    ///
    /// # Examples
    ///
    /// ```
    /// use crossterm::event::KeyModifiers;
    /// use kbd::prelude::*;
    /// use kbd_crossterm::CrosstermModifiersExt;
    ///
    /// let mods = (KeyModifiers::CONTROL | KeyModifiers::SHIFT).to_modifiers();
    /// assert_eq!(mods, ModifierSet::CTRL.with(Modifier::Shift));
    /// ```
    #[must_use]
    fn to_modifiers(&self) -> ModifierSet;
}

impl CrosstermModifiersExt for KeyModifiers {
    fn to_modifiers(&self) -> ModifierSet {
        Modifier::collect_active([
            (self.contains(KeyModifiers::CONTROL), Modifier::Ctrl),
            (self.contains(KeyModifiers::SHIFT), Modifier::Shift),
            (self.contains(KeyModifiers::ALT), Modifier::Alt),
            (self.contains(KeyModifiers::SUPER), Modifier::Super),
        ])
    }
}

/// Convert a crossterm [`KeyEvent`] to a `kbd` [`Hotkey`].
///
/// Returns `None` if the key code has no `kbd` equivalent.
///
/// When the key is itself a modifier (e.g., `LeftShift`), the corresponding
/// modifier flag is stripped from the modifiers — crossterm includes the
/// pressed modifier key in its own modifier bitflags, but `kbd` treats
/// the key as the trigger, not as a modifier of itself.
///
/// This trait is sealed and cannot be implemented outside this crate.
pub trait CrosstermEventExt: private::Sealed {
    /// Convert this key event to a [`Hotkey`], or `None` if the key is unmappable.
    ///
    /// # Examples
    ///
    /// ```
    /// use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    /// use kbd::prelude::*;
    /// use kbd_crossterm::CrosstermEventExt;
    ///
    /// let event = KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL);
    /// assert_eq!(
    ///     event.to_hotkey(),
    ///     Some(Hotkey::new(Key::S).modifier(Modifier::Ctrl)),
    /// );
    /// ```
    #[must_use]
    fn to_hotkey(&self) -> Option<Hotkey>;
}

impl CrosstermEventExt for KeyEvent {
    fn to_hotkey(&self) -> Option<Hotkey> {
        let key = self.code.to_key()?;
        let mut flags = self.modifiers;

        // Strip the modifier that corresponds to the key itself.
        // When pressing LeftShift, crossterm reports SHIFT in the modifiers,
        // but we want the hotkey to be just "ShiftLeft", not "Shift+ShiftLeft".
        if let Some(self_modifier) = modifier_keycode_flag(self.code) {
            flags.remove(self_modifier);
        }

        let modifiers = flags.to_modifiers();
        Some(Hotkey::with_modifiers(key, modifiers))
    }
}

fn char_to_key(ch: char) -> Option<Key> {
    match ch.to_ascii_uppercase() {
        'A' => Some(Key::A),
        'B' => Some(Key::B),
        'C' => Some(Key::C),
        'D' => Some(Key::D),
        'E' => Some(Key::E),
        'F' => Some(Key::F),
        'G' => Some(Key::G),
        'H' => Some(Key::H),
        'I' => Some(Key::I),
        'J' => Some(Key::J),
        'K' => Some(Key::K),
        'L' => Some(Key::L),
        'M' => Some(Key::M),
        'N' => Some(Key::N),
        'O' => Some(Key::O),
        'P' => Some(Key::P),
        'Q' => Some(Key::Q),
        'R' => Some(Key::R),
        'S' => Some(Key::S),
        'T' => Some(Key::T),
        'U' => Some(Key::U),
        'V' => Some(Key::V),
        'W' => Some(Key::W),
        'X' => Some(Key::X),
        'Y' => Some(Key::Y),
        'Z' => Some(Key::Z),
        '0' => Some(Key::DIGIT0),
        '1' => Some(Key::DIGIT1),
        '2' => Some(Key::DIGIT2),
        '3' => Some(Key::DIGIT3),
        '4' => Some(Key::DIGIT4),
        '5' => Some(Key::DIGIT5),
        '6' => Some(Key::DIGIT6),
        '7' => Some(Key::DIGIT7),
        '8' => Some(Key::DIGIT8),
        '9' => Some(Key::DIGIT9),
        ' ' => Some(Key::SPACE),
        '-' => Some(Key::MINUS),
        '=' => Some(Key::EQUAL),
        '[' => Some(Key::BRACKET_LEFT),
        ']' => Some(Key::BRACKET_RIGHT),
        '\\' => Some(Key::BACKSLASH),
        ';' => Some(Key::SEMICOLON),
        '\'' => Some(Key::QUOTE),
        '`' => Some(Key::BACKQUOTE),
        ',' => Some(Key::COMMA),
        '.' => Some(Key::PERIOD),
        '/' => Some(Key::SLASH),
        _ => None,
    }
}

fn function_key(n: u8) -> Option<Key> {
    match n {
        1 => Some(Key::F1),
        2 => Some(Key::F2),
        3 => Some(Key::F3),
        4 => Some(Key::F4),
        5 => Some(Key::F5),
        6 => Some(Key::F6),
        7 => Some(Key::F7),
        8 => Some(Key::F8),
        9 => Some(Key::F9),
        10 => Some(Key::F10),
        11 => Some(Key::F11),
        12 => Some(Key::F12),
        13 => Some(Key::F13),
        14 => Some(Key::F14),
        15 => Some(Key::F15),
        16 => Some(Key::F16),
        17 => Some(Key::F17),
        18 => Some(Key::F18),
        19 => Some(Key::F19),
        20 => Some(Key::F20),
        21 => Some(Key::F21),
        22 => Some(Key::F22),
        23 => Some(Key::F23),
        24 => Some(Key::F24),
        25 => Some(Key::F25),
        26 => Some(Key::F26),
        27 => Some(Key::F27),
        28 => Some(Key::F28),
        29 => Some(Key::F29),
        30 => Some(Key::F30),
        31 => Some(Key::F31),
        32 => Some(Key::F32),
        33 => Some(Key::F33),
        34 => Some(Key::F34),
        35 => Some(Key::F35),
        _ => None,
    }
}

fn media_to_key(media: MediaKeyCode) -> Option<Key> {
    match media {
        MediaKeyCode::PlayPause => Some(Key::MEDIA_PLAY_PAUSE),
        MediaKeyCode::Stop => Some(Key::MEDIA_STOP),
        MediaKeyCode::TrackNext => Some(Key::MEDIA_TRACK_NEXT),
        MediaKeyCode::TrackPrevious => Some(Key::MEDIA_TRACK_PREVIOUS),
        MediaKeyCode::RaiseVolume => Some(Key::AUDIO_VOLUME_UP),
        MediaKeyCode::LowerVolume => Some(Key::AUDIO_VOLUME_DOWN),
        MediaKeyCode::MuteVolume => Some(Key::AUDIO_VOLUME_MUTE),
        MediaKeyCode::Play => Some(Key::MEDIA_PLAY),
        MediaKeyCode::Pause => Some(Key::MEDIA_PAUSE),
        MediaKeyCode::FastForward => Some(Key::MEDIA_FAST_FORWARD),
        MediaKeyCode::Rewind => Some(Key::MEDIA_REWIND),
        MediaKeyCode::Record => Some(Key::MEDIA_RECORD),
        MediaKeyCode::Reverse => None,
    }
}

fn modifier_keycode_to_key(modifier: ModifierKeyCode) -> Option<Key> {
    match modifier {
        ModifierKeyCode::LeftControl => Some(Key::CONTROL_LEFT),
        ModifierKeyCode::RightControl => Some(Key::CONTROL_RIGHT),
        ModifierKeyCode::LeftShift => Some(Key::SHIFT_LEFT),
        ModifierKeyCode::RightShift => Some(Key::SHIFT_RIGHT),
        ModifierKeyCode::LeftAlt => Some(Key::ALT_LEFT),
        ModifierKeyCode::RightAlt => Some(Key::ALT_RIGHT),
        ModifierKeyCode::LeftSuper => Some(Key::META_LEFT),
        ModifierKeyCode::RightSuper => Some(Key::META_RIGHT),
        ModifierKeyCode::LeftHyper | ModifierKeyCode::RightHyper => Some(Key::HYPER),
        ModifierKeyCode::LeftMeta
        | ModifierKeyCode::RightMeta
        | ModifierKeyCode::IsoLevel3Shift
        | ModifierKeyCode::IsoLevel5Shift => None,
    }
}

/// Returns the `KeyModifiers` flag that corresponds to a modifier `KeyCode`,
/// so we can strip it when the key itself IS the modifier.
fn modifier_keycode_flag(code: KeyCode) -> Option<KeyModifiers> {
    match code {
        KeyCode::Modifier(ModifierKeyCode::LeftControl | ModifierKeyCode::RightControl) => {
            Some(KeyModifiers::CONTROL)
        }
        KeyCode::Modifier(ModifierKeyCode::LeftShift | ModifierKeyCode::RightShift) => {
            Some(KeyModifiers::SHIFT)
        }
        KeyCode::Modifier(ModifierKeyCode::LeftAlt | ModifierKeyCode::RightAlt) => {
            Some(KeyModifiers::ALT)
        }
        KeyCode::Modifier(ModifierKeyCode::LeftSuper | ModifierKeyCode::RightSuper) => {
            Some(KeyModifiers::SUPER)
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use crossterm::event::KeyCode;
    use crossterm::event::KeyEvent;
    use crossterm::event::KeyModifiers;
    use crossterm::event::MediaKeyCode;
    use crossterm::event::ModifierKeyCode;
    use kbd::hotkey::Hotkey;
    use kbd::hotkey::Modifier;
    use kbd::key::Key;

    use super::*;

    // CrosstermKeyExt tests

    #[test]
    fn char_lowercase_to_key() {
        assert_eq!(KeyCode::Char('a').to_key(), Some(Key::A));
        assert_eq!(KeyCode::Char('z').to_key(), Some(Key::Z));
    }

    #[test]
    fn char_uppercase_to_key() {
        assert_eq!(KeyCode::Char('A').to_key(), Some(Key::A));
        assert_eq!(KeyCode::Char('Z').to_key(), Some(Key::Z));
    }

    #[test]
    fn digit_chars_to_key() {
        assert_eq!(KeyCode::Char('0').to_key(), Some(Key::DIGIT0));
        assert_eq!(KeyCode::Char('9').to_key(), Some(Key::DIGIT9));
    }

    #[test]
    fn punctuation_chars_to_key() {
        assert_eq!(KeyCode::Char('-').to_key(), Some(Key::MINUS));
        assert_eq!(KeyCode::Char('=').to_key(), Some(Key::EQUAL));
        assert_eq!(KeyCode::Char('[').to_key(), Some(Key::BRACKET_LEFT));
        assert_eq!(KeyCode::Char(']').to_key(), Some(Key::BRACKET_RIGHT));
        assert_eq!(KeyCode::Char('\\').to_key(), Some(Key::BACKSLASH));
        assert_eq!(KeyCode::Char(';').to_key(), Some(Key::SEMICOLON));
        assert_eq!(KeyCode::Char('\'').to_key(), Some(Key::QUOTE));
        assert_eq!(KeyCode::Char('`').to_key(), Some(Key::BACKQUOTE));
        assert_eq!(KeyCode::Char(',').to_key(), Some(Key::COMMA));
        assert_eq!(KeyCode::Char('.').to_key(), Some(Key::PERIOD));
        assert_eq!(KeyCode::Char('/').to_key(), Some(Key::SLASH));
    }

    #[test]
    fn named_keys_to_key() {
        assert_eq!(KeyCode::Enter.to_key(), Some(Key::ENTER));
        assert_eq!(KeyCode::Esc.to_key(), Some(Key::ESCAPE));
        assert_eq!(KeyCode::Backspace.to_key(), Some(Key::BACKSPACE));
        assert_eq!(KeyCode::Tab.to_key(), Some(Key::TAB));
        assert_eq!(KeyCode::Delete.to_key(), Some(Key::DELETE));
        assert_eq!(KeyCode::Insert.to_key(), Some(Key::INSERT));
        assert_eq!(KeyCode::Home.to_key(), Some(Key::HOME));
        assert_eq!(KeyCode::End.to_key(), Some(Key::END));
        assert_eq!(KeyCode::PageUp.to_key(), Some(Key::PAGE_UP));
        assert_eq!(KeyCode::PageDown.to_key(), Some(Key::PAGE_DOWN));
        assert_eq!(KeyCode::Up.to_key(), Some(Key::ARROW_UP));
        assert_eq!(KeyCode::Down.to_key(), Some(Key::ARROW_DOWN));
        assert_eq!(KeyCode::Left.to_key(), Some(Key::ARROW_LEFT));
        assert_eq!(KeyCode::Right.to_key(), Some(Key::ARROW_RIGHT));
        assert_eq!(KeyCode::CapsLock.to_key(), Some(Key::CAPS_LOCK));
        assert_eq!(KeyCode::ScrollLock.to_key(), Some(Key::SCROLL_LOCK));
        assert_eq!(KeyCode::NumLock.to_key(), Some(Key::NUM_LOCK));
        assert_eq!(KeyCode::PrintScreen.to_key(), Some(Key::PRINT_SCREEN));
        assert_eq!(KeyCode::Pause.to_key(), Some(Key::PAUSE));
        assert_eq!(KeyCode::Menu.to_key(), Some(Key::CONTEXT_MENU));
    }

    #[test]
    fn function_keys_to_key() {
        assert_eq!(KeyCode::F(1).to_key(), Some(Key::F1));
        assert_eq!(KeyCode::F(12).to_key(), Some(Key::F12));
        assert_eq!(KeyCode::F(24).to_key(), Some(Key::F24));
        assert_eq!(KeyCode::F(25).to_key(), Some(Key::F25));
        assert_eq!(KeyCode::F(35).to_key(), Some(Key::F35));
        assert_eq!(KeyCode::F(36).to_key(), None);
        assert_eq!(KeyCode::F(0).to_key(), None);
    }

    #[test]
    fn media_keys_to_key() {
        assert_eq!(
            KeyCode::Media(MediaKeyCode::PlayPause).to_key(),
            Some(Key::MEDIA_PLAY_PAUSE)
        );
        assert_eq!(
            KeyCode::Media(MediaKeyCode::Stop).to_key(),
            Some(Key::MEDIA_STOP)
        );
        assert_eq!(
            KeyCode::Media(MediaKeyCode::TrackNext).to_key(),
            Some(Key::MEDIA_TRACK_NEXT)
        );
        assert_eq!(
            KeyCode::Media(MediaKeyCode::TrackPrevious).to_key(),
            Some(Key::MEDIA_TRACK_PREVIOUS)
        );
        assert_eq!(
            KeyCode::Media(MediaKeyCode::RaiseVolume).to_key(),
            Some(Key::AUDIO_VOLUME_UP)
        );
        assert_eq!(
            KeyCode::Media(MediaKeyCode::LowerVolume).to_key(),
            Some(Key::AUDIO_VOLUME_DOWN)
        );
        assert_eq!(
            KeyCode::Media(MediaKeyCode::MuteVolume).to_key(),
            Some(Key::AUDIO_VOLUME_MUTE)
        );
    }

    #[test]
    fn extended_media_keys_to_key() {
        assert_eq!(
            KeyCode::Media(MediaKeyCode::Play).to_key(),
            Some(Key::MEDIA_PLAY)
        );
        assert_eq!(
            KeyCode::Media(MediaKeyCode::Pause).to_key(),
            Some(Key::MEDIA_PAUSE)
        );
        assert_eq!(
            KeyCode::Media(MediaKeyCode::FastForward).to_key(),
            Some(Key::MEDIA_FAST_FORWARD)
        );
        assert_eq!(
            KeyCode::Media(MediaKeyCode::Rewind).to_key(),
            Some(Key::MEDIA_REWIND)
        );
        assert_eq!(
            KeyCode::Media(MediaKeyCode::Record).to_key(),
            Some(Key::MEDIA_RECORD)
        );
    }

    #[test]
    fn modifier_keycode_to_key() {
        assert_eq!(
            KeyCode::Modifier(ModifierKeyCode::LeftControl).to_key(),
            Some(Key::CONTROL_LEFT)
        );
        assert_eq!(
            KeyCode::Modifier(ModifierKeyCode::RightControl).to_key(),
            Some(Key::CONTROL_RIGHT)
        );
        assert_eq!(
            KeyCode::Modifier(ModifierKeyCode::LeftShift).to_key(),
            Some(Key::SHIFT_LEFT)
        );
        assert_eq!(
            KeyCode::Modifier(ModifierKeyCode::RightShift).to_key(),
            Some(Key::SHIFT_RIGHT)
        );
        assert_eq!(
            KeyCode::Modifier(ModifierKeyCode::LeftAlt).to_key(),
            Some(Key::ALT_LEFT)
        );
        assert_eq!(
            KeyCode::Modifier(ModifierKeyCode::RightAlt).to_key(),
            Some(Key::ALT_RIGHT)
        );
        assert_eq!(
            KeyCode::Modifier(ModifierKeyCode::LeftSuper).to_key(),
            Some(Key::META_LEFT)
        );
        assert_eq!(
            KeyCode::Modifier(ModifierKeyCode::RightSuper).to_key(),
            Some(Key::META_RIGHT)
        );
    }

    #[test]
    fn hyper_modifier_keys_to_key() {
        assert_eq!(
            KeyCode::Modifier(ModifierKeyCode::LeftHyper).to_key(),
            Some(Key::HYPER)
        );
        assert_eq!(
            KeyCode::Modifier(ModifierKeyCode::RightHyper).to_key(),
            Some(Key::HYPER)
        );
    }

    #[test]
    fn unmappable_keys_return_none() {
        assert_eq!(KeyCode::Null.to_key(), None);
        assert_eq!(KeyCode::BackTab.to_key(), None);
        assert_eq!(KeyCode::KeypadBegin.to_key(), None);
    }

    #[test]
    fn non_ascii_chars_return_none() {
        assert_eq!(KeyCode::Char('é').to_key(), None);
        assert_eq!(KeyCode::Char('中').to_key(), None);
    }

    #[test]
    fn reverse_media_key_returns_none() {
        assert_eq!(KeyCode::Media(MediaKeyCode::Reverse).to_key(), None);
    }

    #[test]
    fn unmappable_modifier_keycodes_return_none() {
        assert_eq!(KeyCode::Modifier(ModifierKeyCode::LeftMeta).to_key(), None);
        assert_eq!(KeyCode::Modifier(ModifierKeyCode::RightMeta).to_key(), None);
        assert_eq!(
            KeyCode::Modifier(ModifierKeyCode::IsoLevel3Shift).to_key(),
            None
        );
        assert_eq!(
            KeyCode::Modifier(ModifierKeyCode::IsoLevel5Shift).to_key(),
            None
        );
    }

    // CrosstermModifiersExt tests

    #[test]
    fn empty_modifiers() {
        assert_eq!(KeyModifiers::NONE.to_modifiers(), ModifierSet::EMPTY);
    }

    #[test]
    fn single_modifier() {
        assert_eq!(KeyModifiers::CONTROL.to_modifiers(), ModifierSet::CTRL);
        assert_eq!(KeyModifiers::SHIFT.to_modifiers(), ModifierSet::SHIFT);
        assert_eq!(KeyModifiers::ALT.to_modifiers(), ModifierSet::ALT);
        assert_eq!(KeyModifiers::SUPER.to_modifiers(), ModifierSet::SUPER);
    }

    #[test]
    fn combined_modifiers() {
        let mods = KeyModifiers::CONTROL | KeyModifiers::SHIFT;
        let result = mods.to_modifiers();
        assert_eq!(result, ModifierSet::CTRL.with(Modifier::Shift));
    }

    #[test]
    fn all_modifiers() {
        let mods =
            KeyModifiers::CONTROL | KeyModifiers::SHIFT | KeyModifiers::ALT | KeyModifiers::SUPER;
        let result = mods.to_modifiers();
        assert_eq!(
            result,
            ModifierSet::EMPTY
                .with(Modifier::Ctrl)
                .with(Modifier::Shift)
                .with(Modifier::Alt)
                .with(Modifier::Super)
        );
    }

    #[test]
    fn hyper_and_meta_ignored() {
        let mods = KeyModifiers::HYPER | KeyModifiers::META;
        assert_eq!(mods.to_modifiers(), ModifierSet::EMPTY);
    }

    // CrosstermEventExt tests

    #[test]
    fn simple_key_event_to_hotkey() {
        let event = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::NONE);
        let hotkey = event.to_hotkey();
        assert_eq!(hotkey, Some(Hotkey::new(Key::C)));
    }

    #[test]
    fn key_event_with_modifiers_to_hotkey() {
        let event = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
        let hotkey = event.to_hotkey();
        assert_eq!(hotkey, Some(Hotkey::new(Key::C).modifier(Modifier::Ctrl)));
    }

    #[test]
    fn key_event_with_multiple_modifiers() {
        let event = KeyEvent::new(
            KeyCode::Char('a'),
            KeyModifiers::CONTROL | KeyModifiers::SHIFT,
        );
        let hotkey = event.to_hotkey();
        assert_eq!(
            hotkey,
            Some(
                Hotkey::new(Key::A)
                    .modifier(Modifier::Ctrl)
                    .modifier(Modifier::Shift)
            )
        );
    }

    #[test]
    fn unmappable_key_event_returns_none() {
        let event = KeyEvent::new(KeyCode::Null, KeyModifiers::NONE);
        assert_eq!(event.to_hotkey(), None);
    }

    #[test]
    fn modifier_key_event_strips_self_modifier() {
        // When crossterm reports pressing LeftShift, the modifiers already include SHIFT.
        // The hotkey should represent "just Shift pressed" (ShiftLeft with no extra modifiers),
        // not "Shift+ShiftLeft".
        let event = KeyEvent::new(
            KeyCode::Modifier(ModifierKeyCode::LeftShift),
            KeyModifiers::SHIFT,
        );
        let hotkey = event.to_hotkey();
        assert_eq!(hotkey, Some(Hotkey::new(Key::SHIFT_LEFT)));
    }

    #[test]
    fn modifier_key_event_keeps_other_modifiers() {
        // Pressing Ctrl while Shift is already held
        let event = KeyEvent::new(
            KeyCode::Modifier(ModifierKeyCode::LeftControl),
            KeyModifiers::SHIFT | KeyModifiers::CONTROL,
        );
        let hotkey = event.to_hotkey();
        assert_eq!(
            hotkey,
            Some(Hotkey::new(Key::CONTROL_LEFT).modifier(Modifier::Shift))
        );
    }

    #[test]
    fn uppercase_char_treated_as_physical_key() {
        // crossterm reports 'A' (uppercase) when Shift is held — this is the same
        // physical key as 'a', just with Shift modifier
        let event = KeyEvent::new(KeyCode::Char('A'), KeyModifiers::SHIFT);
        let hotkey = event.to_hotkey();
        assert_eq!(hotkey, Some(Hotkey::new(Key::A).modifier(Modifier::Shift)));
    }

    #[test]
    fn space_key_event() {
        let event = KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE);
        let hotkey = event.to_hotkey();
        assert_eq!(hotkey, Some(Hotkey::new(Key::SPACE)));
    }

    #[test]
    fn ctrl_shift_f5() {
        let event = KeyEvent::new(KeyCode::F(5), KeyModifiers::CONTROL | KeyModifiers::SHIFT);
        let hotkey = event.to_hotkey();
        assert_eq!(
            hotkey,
            Some(
                Hotkey::new(Key::F5)
                    .modifier(Modifier::Ctrl)
                    .modifier(Modifier::Shift)
            )
        );
    }
}
