#![cfg_attr(docsrs, feature(doc_auto_cfg))]

//! Iced key event conversions for `kbd`.
//!
//! This crate bridges iced's keyboard types to `kbd`'s key types.
//! iced defines its own W3C-derived key types: [`key::Code`] for physical
//! key positions and [`key::Physical`] wrapping `Code` with an unidentified
//! fallback. iced also has a logical key type for character/named key
//! identity, but this crate only converts physical keys — they are
//! layout-independent and match `kbd`'s model.
//!
//! # Extension traits
//!
//! - [`IcedKeyExt`] — converts an iced [`key::Code`] or [`key::Physical`]
//!   to a [`kbd::Key`].
//! - [`IcedModifiersExt`] — converts iced [`Modifiers`] to a
//!   `Vec<Modifier>`.
//! - [`IcedEventExt`] — converts an iced keyboard [`Event`] to a
//!   [`kbd::Hotkey`].
//!
//! # Key mapping
//!
//! | iced | kbd | Notes |
//! |---|---|---|
//! | `Code::KeyA` – `Code::KeyZ` | [`Key::A`] – [`Key::Z`] | Letters |
//! | `Code::Digit0` – `Code::Digit9` | [`Key::DIGIT0`] – [`Key::DIGIT9`] | Digits |
//! | `Code::F1` – `Code::F35` | [`Key::F1`] – [`Key::F35`] | Function keys |
//! | `Code::Numpad0` – `Code::Numpad9` | [`Key::NUMPAD0`] – [`Key::NUMPAD9`] | Numpad |
//! | `Code::Enter`, `Code::Escape`, … | [`Key::ENTER`], [`Key::ESCAPE`], … | Navigation / editing |
//! | `Code::ControlLeft`, … | [`Key::CONTROL_LEFT`], … | Modifier keys as triggers |
//! | `Code::SuperLeft` / `Code::Meta` | [`Key::META_LEFT`] | iced's Super = kbd's Meta |
//! | `Code::MediaPlayPause`, … | [`Key::MEDIA_PLAY_PAUSE`], … | Media keys |
//! | `Code::BrowserBack`, … | [`Key::BROWSER_BACK`], … | Browser keys |
//! | `Code::Convert`, `Code::Lang1`, … | [`Key::CONVERT`], [`Key::LANG1`], … | CJK / international |
//! | `Physical::Unidentified(_)` | `None` | No mapping possible |
//!
//! # Modifier mapping
//!
//! | iced | kbd |
//! |---|---|
//! | `CTRL` | [`Modifier::Ctrl`] |
//! | `SHIFT` | [`Modifier::Shift`] |
//! | `ALT` | [`Modifier::Alt`] |
//! | `LOGO` | [`Modifier::Super`] |
//!
//! # Usage
//!
//! ```
//! use iced_core::keyboard::{key::Code, Modifiers};
//! use kbd::{Key, Modifier};
//! use kbd_iced::{IcedKeyExt, IcedModifiersExt};
//!
//! // Code conversion
//! let key = Code::KeyA.to_key();
//! assert_eq!(key, Some(Key::A));
//!
//! // Modifier conversion
//! let mods = Modifiers::CTRL.to_modifiers();
//! assert_eq!(mods, vec![Modifier::Ctrl]);
//! ```

use iced_core::keyboard::Event;
use iced_core::keyboard::Modifiers;
use iced_core::keyboard::key;
use kbd::Hotkey;
use kbd::Key;
use kbd::Modifier;

/// Convert an iced physical key type to a `kbd` [`Key`].
///
/// Returns `None` for keys that have no `kbd` equivalent (e.g.,
/// `Unidentified`, keys beyond F24, international input keys).
pub trait IcedKeyExt {
    /// Convert this iced key to a `kbd` [`Key`], or `None` if unmappable.
    ///
    /// # Examples
    ///
    /// ```
    /// use iced_core::keyboard::key;
    /// use kbd::Key;
    /// use kbd_iced::IcedKeyExt;
    ///
    /// assert_eq!(key::Code::KeyA.to_key(), Some(Key::A));
    /// assert_eq!(key::Code::F5.to_key(), Some(Key::F5));
    ///
    /// let physical = key::Physical::Code(key::Code::Enter);
    /// assert_eq!(physical.to_key(), Some(Key::ENTER));
    /// ```
    fn to_key(&self) -> Option<Key>;
}

impl IcedKeyExt for key::Code {
    #[allow(clippy::too_many_lines)]
    fn to_key(&self) -> Option<Key> {
        match self {
            // Letters
            key::Code::KeyA => Some(Key::A),
            key::Code::KeyB => Some(Key::B),
            key::Code::KeyC => Some(Key::C),
            key::Code::KeyD => Some(Key::D),
            key::Code::KeyE => Some(Key::E),
            key::Code::KeyF => Some(Key::F),
            key::Code::KeyG => Some(Key::G),
            key::Code::KeyH => Some(Key::H),
            key::Code::KeyI => Some(Key::I),
            key::Code::KeyJ => Some(Key::J),
            key::Code::KeyK => Some(Key::K),
            key::Code::KeyL => Some(Key::L),
            key::Code::KeyM => Some(Key::M),
            key::Code::KeyN => Some(Key::N),
            key::Code::KeyO => Some(Key::O),
            key::Code::KeyP => Some(Key::P),
            key::Code::KeyQ => Some(Key::Q),
            key::Code::KeyR => Some(Key::R),
            key::Code::KeyS => Some(Key::S),
            key::Code::KeyT => Some(Key::T),
            key::Code::KeyU => Some(Key::U),
            key::Code::KeyV => Some(Key::V),
            key::Code::KeyW => Some(Key::W),
            key::Code::KeyX => Some(Key::X),
            key::Code::KeyY => Some(Key::Y),
            key::Code::KeyZ => Some(Key::Z),

            // Digits
            key::Code::Digit0 => Some(Key::DIGIT0),
            key::Code::Digit1 => Some(Key::DIGIT1),
            key::Code::Digit2 => Some(Key::DIGIT2),
            key::Code::Digit3 => Some(Key::DIGIT3),
            key::Code::Digit4 => Some(Key::DIGIT4),
            key::Code::Digit5 => Some(Key::DIGIT5),
            key::Code::Digit6 => Some(Key::DIGIT6),
            key::Code::Digit7 => Some(Key::DIGIT7),
            key::Code::Digit8 => Some(Key::DIGIT8),
            key::Code::Digit9 => Some(Key::DIGIT9),

            // Function keys
            key::Code::F1 => Some(Key::F1),
            key::Code::F2 => Some(Key::F2),
            key::Code::F3 => Some(Key::F3),
            key::Code::F4 => Some(Key::F4),
            key::Code::F5 => Some(Key::F5),
            key::Code::F6 => Some(Key::F6),
            key::Code::F7 => Some(Key::F7),
            key::Code::F8 => Some(Key::F8),
            key::Code::F9 => Some(Key::F9),
            key::Code::F10 => Some(Key::F10),
            key::Code::F11 => Some(Key::F11),
            key::Code::F12 => Some(Key::F12),
            key::Code::F13 => Some(Key::F13),
            key::Code::F14 => Some(Key::F14),
            key::Code::F15 => Some(Key::F15),
            key::Code::F16 => Some(Key::F16),
            key::Code::F17 => Some(Key::F17),
            key::Code::F18 => Some(Key::F18),
            key::Code::F19 => Some(Key::F19),
            key::Code::F20 => Some(Key::F20),
            key::Code::F21 => Some(Key::F21),
            key::Code::F22 => Some(Key::F22),
            key::Code::F23 => Some(Key::F23),
            key::Code::F24 => Some(Key::F24),
            key::Code::F25 => Some(Key::F25),
            key::Code::F26 => Some(Key::F26),
            key::Code::F27 => Some(Key::F27),
            key::Code::F28 => Some(Key::F28),
            key::Code::F29 => Some(Key::F29),
            key::Code::F30 => Some(Key::F30),
            key::Code::F31 => Some(Key::F31),
            key::Code::F32 => Some(Key::F32),
            key::Code::F33 => Some(Key::F33),
            key::Code::F34 => Some(Key::F34),
            key::Code::F35 => Some(Key::F35),

            // Navigation and editing
            key::Code::Enter => Some(Key::ENTER),
            key::Code::Escape => Some(Key::ESCAPE),
            key::Code::Space => Some(Key::SPACE),
            key::Code::Tab => Some(Key::TAB),
            key::Code::Delete => Some(Key::DELETE),
            key::Code::Backspace => Some(Key::BACKSPACE),
            key::Code::Insert => Some(Key::INSERT),
            key::Code::CapsLock => Some(Key::CAPS_LOCK),
            key::Code::Home => Some(Key::HOME),
            key::Code::End => Some(Key::END),
            key::Code::PageUp => Some(Key::PAGE_UP),
            key::Code::PageDown => Some(Key::PAGE_DOWN),
            key::Code::ArrowUp => Some(Key::ARROW_UP),
            key::Code::ArrowDown => Some(Key::ARROW_DOWN),
            key::Code::ArrowLeft => Some(Key::ARROW_LEFT),
            key::Code::ArrowRight => Some(Key::ARROW_RIGHT),

            // Punctuation
            key::Code::Minus => Some(Key::MINUS),
            key::Code::Equal => Some(Key::EQUAL),
            key::Code::BracketLeft => Some(Key::BRACKET_LEFT),
            key::Code::BracketRight => Some(Key::BRACKET_RIGHT),
            key::Code::Backslash => Some(Key::BACKSLASH),
            key::Code::Semicolon => Some(Key::SEMICOLON),
            key::Code::Quote => Some(Key::QUOTE),
            key::Code::Backquote => Some(Key::BACKQUOTE),
            key::Code::Comma => Some(Key::COMMA),
            key::Code::Period => Some(Key::PERIOD),
            key::Code::Slash => Some(Key::SLASH),

            // Numpad
            key::Code::Numpad0 => Some(Key::NUMPAD0),
            key::Code::Numpad1 => Some(Key::NUMPAD1),
            key::Code::Numpad2 => Some(Key::NUMPAD2),
            key::Code::Numpad3 => Some(Key::NUMPAD3),
            key::Code::Numpad4 => Some(Key::NUMPAD4),
            key::Code::Numpad5 => Some(Key::NUMPAD5),
            key::Code::Numpad6 => Some(Key::NUMPAD6),
            key::Code::Numpad7 => Some(Key::NUMPAD7),
            key::Code::Numpad8 => Some(Key::NUMPAD8),
            key::Code::Numpad9 => Some(Key::NUMPAD9),
            key::Code::NumpadDecimal => Some(Key::NUMPAD_DECIMAL),
            key::Code::NumpadAdd => Some(Key::NUMPAD_ADD),
            key::Code::NumpadSubtract => Some(Key::NUMPAD_SUBTRACT),
            key::Code::NumpadMultiply => Some(Key::NUMPAD_MULTIPLY),
            key::Code::NumpadDivide => Some(Key::NUMPAD_DIVIDE),
            key::Code::NumpadEnter => Some(Key::NUMPAD_ENTER),
            key::Code::NumpadEqual => Some(Key::NUMPAD_EQUAL),
            key::Code::NumpadComma => Some(Key::NUMPAD_COMMA),
            key::Code::NumpadBackspace => Some(Key::NUMPAD_BACKSPACE),
            key::Code::NumpadClear => Some(Key::NUMPAD_CLEAR),
            key::Code::NumpadClearEntry => Some(Key::NUMPAD_CLEAR_ENTRY),
            key::Code::NumpadHash => Some(Key::NUMPAD_HASH),
            key::Code::NumpadMemoryAdd => Some(Key::NUMPAD_MEMORY_ADD),
            key::Code::NumpadMemoryClear => Some(Key::NUMPAD_MEMORY_CLEAR),
            key::Code::NumpadMemoryRecall => Some(Key::NUMPAD_MEMORY_RECALL),
            key::Code::NumpadMemoryStore => Some(Key::NUMPAD_MEMORY_STORE),
            key::Code::NumpadMemorySubtract => Some(Key::NUMPAD_MEMORY_SUBTRACT),
            key::Code::NumpadParenLeft => Some(Key::NUMPAD_PAREN_LEFT),
            key::Code::NumpadParenRight => Some(Key::NUMPAD_PAREN_RIGHT),
            key::Code::NumpadStar => Some(Key::NUMPAD_STAR),

            // Modifiers — iced uses SuperLeft/SuperRight where W3C uses MetaLeft/MetaRight.
            // Meta is iced's legacy alias for the Super key (no left/right distinction).
            key::Code::ControlLeft => Some(Key::CONTROL_LEFT),
            key::Code::ControlRight => Some(Key::CONTROL_RIGHT),
            key::Code::ShiftLeft => Some(Key::SHIFT_LEFT),
            key::Code::ShiftRight => Some(Key::SHIFT_RIGHT),
            key::Code::AltLeft => Some(Key::ALT_LEFT),
            key::Code::AltRight => Some(Key::ALT_RIGHT),
            key::Code::SuperLeft | key::Code::Meta => Some(Key::META_LEFT),
            key::Code::SuperRight => Some(Key::META_RIGHT),

            // Media keys
            key::Code::AudioVolumeUp => Some(Key::AUDIO_VOLUME_UP),
            key::Code::AudioVolumeDown => Some(Key::AUDIO_VOLUME_DOWN),
            key::Code::AudioVolumeMute => Some(Key::AUDIO_VOLUME_MUTE),
            key::Code::MediaPlayPause => Some(Key::MEDIA_PLAY_PAUSE),
            key::Code::MediaStop => Some(Key::MEDIA_STOP),
            key::Code::MediaTrackNext => Some(Key::MEDIA_TRACK_NEXT),
            key::Code::MediaTrackPrevious => Some(Key::MEDIA_TRACK_PREVIOUS),
            key::Code::MediaSelect => Some(Key::MEDIA_SELECT),

            // Browser keys
            key::Code::BrowserBack => Some(Key::BROWSER_BACK),
            key::Code::BrowserFavorites => Some(Key::BROWSER_FAVORITES),
            key::Code::BrowserForward => Some(Key::BROWSER_FORWARD),
            key::Code::BrowserHome => Some(Key::BROWSER_HOME),
            key::Code::BrowserRefresh => Some(Key::BROWSER_REFRESH),
            key::Code::BrowserSearch => Some(Key::BROWSER_SEARCH),
            key::Code::BrowserStop => Some(Key::BROWSER_STOP),

            // System keys
            key::Code::PrintScreen => Some(Key::PRINT_SCREEN),
            key::Code::ScrollLock => Some(Key::SCROLL_LOCK),
            key::Code::Pause => Some(Key::PAUSE),
            key::Code::NumLock => Some(Key::NUM_LOCK),
            key::Code::ContextMenu => Some(Key::CONTEXT_MENU),
            key::Code::Power => Some(Key::POWER),
            key::Code::Sleep => Some(Key::SLEEP),
            key::Code::WakeUp => Some(Key::WAKE_UP),
            key::Code::Eject => Some(Key::EJECT),

            // Clipboard / editing keys
            key::Code::Copy => Some(Key::COPY),
            key::Code::Cut => Some(Key::CUT),
            key::Code::Paste => Some(Key::PASTE),
            key::Code::Undo => Some(Key::UNDO),
            key::Code::Find => Some(Key::FIND),
            key::Code::Help => Some(Key::HELP),
            key::Code::Open => Some(Key::OPEN),
            key::Code::Select => Some(Key::SELECT),
            key::Code::Again => Some(Key::AGAIN),
            key::Code::Props => Some(Key::PROPS),
            key::Code::Abort => Some(Key::ABORT),
            key::Code::Resume => Some(Key::RESUME),
            key::Code::Suspend => Some(Key::SUSPEND),

            // Fn and legacy
            key::Code::Fn => Some(Key::FN),
            key::Code::FnLock => Some(Key::FN_LOCK),
            key::Code::Hyper => Some(Key::HYPER),
            key::Code::Turbo => Some(Key::TURBO),

            // CJK / international
            key::Code::Convert => Some(Key::CONVERT),
            key::Code::NonConvert => Some(Key::NON_CONVERT),
            key::Code::KanaMode => Some(Key::KANA_MODE),
            key::Code::Hiragana => Some(Key::HIRAGANA),
            key::Code::Katakana => Some(Key::KATAKANA),
            key::Code::Lang1 => Some(Key::LANG1),
            key::Code::Lang2 => Some(Key::LANG2),
            key::Code::Lang3 => Some(Key::LANG3),
            key::Code::Lang4 => Some(Key::LANG4),
            key::Code::Lang5 => Some(Key::LANG5),
            key::Code::IntlBackslash => Some(Key::INTL_BACKSLASH),
            key::Code::IntlRo => Some(Key::INTL_RO),
            key::Code::IntlYen => Some(Key::INTL_YEN),

            // App launch keys
            key::Code::LaunchApp1 => Some(Key::LAUNCH_APP1),
            key::Code::LaunchApp2 => Some(Key::LAUNCH_APP2),
            key::Code::LaunchMail => Some(Key::LAUNCH_MAIL),

            _ => None,
        }
    }
}

impl IcedKeyExt for key::Physical {
    fn to_key(&self) -> Option<Key> {
        match self {
            key::Physical::Code(code) => code.to_key(),
            key::Physical::Unidentified(_) => None,
        }
    }
}

/// Convert iced [`Modifiers`] bitflags to a sorted `Vec<Modifier>`.
///
/// Iced uses `LOGO` for the Super/Meta/Windows key. This maps to
/// `Modifier::Super` in `kbd`.
pub trait IcedModifiersExt {
    /// Convert these iced modifier flags to a `Vec<Modifier>`.
    ///
    /// # Examples
    ///
    /// ```
    /// use iced_core::keyboard::Modifiers;
    /// use kbd::Modifier;
    /// use kbd_iced::IcedModifiersExt;
    ///
    /// let mods = (Modifiers::CTRL | Modifiers::SHIFT).to_modifiers();
    /// assert_eq!(mods, vec![Modifier::Ctrl, Modifier::Shift]);
    /// ```
    fn to_modifiers(&self) -> Vec<Modifier>;
}

impl IcedModifiersExt for Modifiers {
    fn to_modifiers(&self) -> Vec<Modifier> {
        let mut modifiers = Vec::new();
        if self.control() {
            modifiers.push(Modifier::Ctrl);
        }
        if self.shift() {
            modifiers.push(Modifier::Shift);
        }
        if self.alt() {
            modifiers.push(Modifier::Alt);
        }
        if self.logo() {
            modifiers.push(Modifier::Super);
        }
        modifiers
    }
}

/// Convert an iced keyboard [`Event`] to a `kbd` [`Hotkey`].
///
/// Uses the physical key from the event for layout-independent matching.
/// Returns `None` for `ModifiersChanged` events (no key trigger) and
/// for events with unidentified physical keys.
///
/// When the key is itself a modifier (e.g., `ControlLeft`), the
/// corresponding modifier flag is stripped from the modifiers — iced
/// includes the pressed modifier key in its own modifier state, but
/// `kbd` treats the key as the trigger, not as a modifier of itself.
pub trait IcedEventExt {
    /// Convert this keyboard event to a [`Hotkey`], or `None` if unmappable.
    ///
    /// # Examples
    ///
    /// ```
    /// use iced_core::keyboard::{Event, Location, Modifiers, key};
    /// use kbd::{Hotkey, Key, Modifier};
    /// use kbd_iced::IcedEventExt;
    ///
    /// let event = Event::KeyPressed {
    ///     key: iced_core::keyboard::Key::Unidentified,
    ///     modified_key: iced_core::keyboard::Key::Unidentified,
    ///     physical_key: key::Physical::Code(key::Code::KeyS),
    ///     location: Location::Standard,
    ///     modifiers: Modifiers::CTRL,
    ///     text: None,
    ///     repeat: false,
    /// };
    /// assert_eq!(
    ///     event.to_hotkey(),
    ///     Some(Hotkey::new(Key::S).modifier(Modifier::Ctrl)),
    /// );
    /// ```
    fn to_hotkey(&self) -> Option<Hotkey>;
}

impl IcedEventExt for Event {
    fn to_hotkey(&self) -> Option<Hotkey> {
        let (physical_key, modifiers) = match self {
            Event::KeyPressed {
                physical_key,
                modifiers,
                ..
            }
            | Event::KeyReleased {
                physical_key,
                modifiers,
                ..
            } => (physical_key, modifiers),
            Event::ModifiersChanged(_) => return None,
        };

        let key = physical_key.to_key()?;
        let mut mods = modifiers.to_modifiers();

        // Strip the modifier that corresponds to the key itself.
        if let Some(self_modifier) = Modifier::from_key(key) {
            mods.retain(|m| *m != self_modifier);
        }

        Some(Hotkey::with_modifiers(key, mods))
    }
}

#[cfg(test)]
mod tests {
    use iced_core::keyboard::Event;
    use iced_core::keyboard::Location;
    use iced_core::keyboard::Modifiers;
    use iced_core::keyboard::key;
    use kbd::Hotkey;
    use kbd::Key;
    use kbd::Modifier;

    use super::*;

    // IcedKeyExt — Code

    #[test]
    fn code_letters() {
        assert_eq!(key::Code::KeyA.to_key(), Some(Key::A));
        assert_eq!(key::Code::KeyZ.to_key(), Some(Key::Z));
    }

    #[test]
    fn code_digits() {
        assert_eq!(key::Code::Digit0.to_key(), Some(Key::DIGIT0));
        assert_eq!(key::Code::Digit9.to_key(), Some(Key::DIGIT9));
    }

    #[test]
    fn code_function_keys() {
        assert_eq!(key::Code::F1.to_key(), Some(Key::F1));
        assert_eq!(key::Code::F12.to_key(), Some(Key::F12));
        assert_eq!(key::Code::F24.to_key(), Some(Key::F24));
        assert_eq!(key::Code::F25.to_key(), Some(Key::F25));
    }

    #[test]
    fn code_navigation() {
        assert_eq!(key::Code::Enter.to_key(), Some(Key::ENTER));
        assert_eq!(key::Code::Escape.to_key(), Some(Key::ESCAPE));
        assert_eq!(key::Code::Backspace.to_key(), Some(Key::BACKSPACE));
        assert_eq!(key::Code::Tab.to_key(), Some(Key::TAB));
        assert_eq!(key::Code::Space.to_key(), Some(Key::SPACE));
        assert_eq!(key::Code::Delete.to_key(), Some(Key::DELETE));
        assert_eq!(key::Code::Insert.to_key(), Some(Key::INSERT));
        assert_eq!(key::Code::Home.to_key(), Some(Key::HOME));
        assert_eq!(key::Code::End.to_key(), Some(Key::END));
        assert_eq!(key::Code::PageUp.to_key(), Some(Key::PAGE_UP));
        assert_eq!(key::Code::PageDown.to_key(), Some(Key::PAGE_DOWN));
        assert_eq!(key::Code::ArrowUp.to_key(), Some(Key::ARROW_UP));
        assert_eq!(key::Code::ArrowDown.to_key(), Some(Key::ARROW_DOWN));
        assert_eq!(key::Code::ArrowLeft.to_key(), Some(Key::ARROW_LEFT));
        assert_eq!(key::Code::ArrowRight.to_key(), Some(Key::ARROW_RIGHT));
    }

    #[test]
    fn code_modifiers() {
        assert_eq!(key::Code::ControlLeft.to_key(), Some(Key::CONTROL_LEFT));
        assert_eq!(key::Code::ControlRight.to_key(), Some(Key::CONTROL_RIGHT));
        assert_eq!(key::Code::ShiftLeft.to_key(), Some(Key::SHIFT_LEFT));
        assert_eq!(key::Code::ShiftRight.to_key(), Some(Key::SHIFT_RIGHT));
        assert_eq!(key::Code::AltLeft.to_key(), Some(Key::ALT_LEFT));
        assert_eq!(key::Code::AltRight.to_key(), Some(Key::ALT_RIGHT));
        // iced's SuperLeft/Right → kbd's MetaLeft/MetaRight
        assert_eq!(key::Code::SuperLeft.to_key(), Some(Key::META_LEFT));
        assert_eq!(key::Code::SuperRight.to_key(), Some(Key::META_RIGHT));
        // iced's legacy Meta (no left/right) → defaults to MetaLeft
        assert_eq!(key::Code::Meta.to_key(), Some(Key::META_LEFT));
    }

    #[test]
    fn code_punctuation() {
        assert_eq!(key::Code::Minus.to_key(), Some(Key::MINUS));
        assert_eq!(key::Code::Equal.to_key(), Some(Key::EQUAL));
        assert_eq!(key::Code::BracketLeft.to_key(), Some(Key::BRACKET_LEFT));
        assert_eq!(key::Code::BracketRight.to_key(), Some(Key::BRACKET_RIGHT));
        assert_eq!(key::Code::Backslash.to_key(), Some(Key::BACKSLASH));
        assert_eq!(key::Code::Semicolon.to_key(), Some(Key::SEMICOLON));
        assert_eq!(key::Code::Quote.to_key(), Some(Key::QUOTE));
        assert_eq!(key::Code::Backquote.to_key(), Some(Key::BACKQUOTE));
        assert_eq!(key::Code::Comma.to_key(), Some(Key::COMMA));
        assert_eq!(key::Code::Period.to_key(), Some(Key::PERIOD));
        assert_eq!(key::Code::Slash.to_key(), Some(Key::SLASH));
    }

    #[test]
    fn code_numpad() {
        assert_eq!(key::Code::Numpad0.to_key(), Some(Key::NUMPAD0));
        assert_eq!(key::Code::Numpad9.to_key(), Some(Key::NUMPAD9));
        assert_eq!(key::Code::NumpadDecimal.to_key(), Some(Key::NUMPAD_DECIMAL));
        assert_eq!(key::Code::NumpadAdd.to_key(), Some(Key::NUMPAD_ADD));
        assert_eq!(
            key::Code::NumpadSubtract.to_key(),
            Some(Key::NUMPAD_SUBTRACT)
        );
        assert_eq!(
            key::Code::NumpadMultiply.to_key(),
            Some(Key::NUMPAD_MULTIPLY)
        );
        assert_eq!(key::Code::NumpadDivide.to_key(), Some(Key::NUMPAD_DIVIDE));
        assert_eq!(key::Code::NumpadEnter.to_key(), Some(Key::NUMPAD_ENTER));
    }

    #[test]
    fn code_media() {
        assert_eq!(
            key::Code::MediaPlayPause.to_key(),
            Some(Key::MEDIA_PLAY_PAUSE)
        );
        assert_eq!(key::Code::MediaStop.to_key(), Some(Key::MEDIA_STOP));
        assert_eq!(
            key::Code::MediaTrackNext.to_key(),
            Some(Key::MEDIA_TRACK_NEXT)
        );
        assert_eq!(
            key::Code::MediaTrackPrevious.to_key(),
            Some(Key::MEDIA_TRACK_PREVIOUS)
        );
        assert_eq!(
            key::Code::AudioVolumeUp.to_key(),
            Some(Key::AUDIO_VOLUME_UP)
        );
        assert_eq!(
            key::Code::AudioVolumeDown.to_key(),
            Some(Key::AUDIO_VOLUME_DOWN)
        );
        assert_eq!(
            key::Code::AudioVolumeMute.to_key(),
            Some(Key::AUDIO_VOLUME_MUTE)
        );
    }

    #[test]
    fn code_system() {
        assert_eq!(key::Code::PrintScreen.to_key(), Some(Key::PRINT_SCREEN));
        assert_eq!(key::Code::ScrollLock.to_key(), Some(Key::SCROLL_LOCK));
        assert_eq!(key::Code::Pause.to_key(), Some(Key::PAUSE));
        assert_eq!(key::Code::NumLock.to_key(), Some(Key::NUM_LOCK));
        assert_eq!(key::Code::ContextMenu.to_key(), Some(Key::CONTEXT_MENU));
        assert_eq!(key::Code::Power.to_key(), Some(Key::POWER));
    }

    #[test]
    fn code_extended_keys() {
        assert_eq!(key::Code::F25.to_key(), Some(Key::F25));
        assert_eq!(key::Code::F35.to_key(), Some(Key::F35));
        assert_eq!(key::Code::BrowserBack.to_key(), Some(Key::BROWSER_BACK));
        assert_eq!(key::Code::Copy.to_key(), Some(Key::COPY));
        assert_eq!(key::Code::Sleep.to_key(), Some(Key::SLEEP));
        assert_eq!(key::Code::IntlBackslash.to_key(), Some(Key::INTL_BACKSLASH));
        assert_eq!(key::Code::NumpadEqual.to_key(), Some(Key::NUMPAD_EQUAL));
        assert_eq!(key::Code::Fn.to_key(), Some(Key::FN));
        assert_eq!(key::Code::LaunchMail.to_key(), Some(Key::LAUNCH_MAIL));
        assert_eq!(key::Code::Convert.to_key(), Some(Key::CONVERT));
        assert_eq!(key::Code::Lang1.to_key(), Some(Key::LANG1));
    }

    // IcedKeyExt — Physical

    #[test]
    fn physical_code_to_key() {
        let physical = key::Physical::Code(key::Code::KeyA);
        assert_eq!(physical.to_key(), Some(Key::A));
    }

    #[test]
    fn physical_unidentified_returns_none() {
        let physical = key::Physical::Unidentified(key::NativeCode::Unidentified);
        assert_eq!(physical.to_key(), None);
    }

    // IcedModifiersExt

    #[test]
    fn empty_modifiers() {
        assert_eq!(Modifiers::empty().to_modifiers(), Vec::<Modifier>::new());
    }

    #[test]
    fn single_modifiers() {
        assert_eq!(Modifiers::CTRL.to_modifiers(), vec![Modifier::Ctrl]);
        assert_eq!(Modifiers::SHIFT.to_modifiers(), vec![Modifier::Shift]);
        assert_eq!(Modifiers::ALT.to_modifiers(), vec![Modifier::Alt]);
        // iced's LOGO = kbd's Super
        assert_eq!(Modifiers::LOGO.to_modifiers(), vec![Modifier::Super]);
    }

    #[test]
    fn combined_modifiers() {
        let mods = Modifiers::CTRL | Modifiers::SHIFT;
        assert_eq!(mods.to_modifiers(), vec![Modifier::Ctrl, Modifier::Shift]);
    }

    #[test]
    fn all_modifiers() {
        let mods = Modifiers::CTRL | Modifiers::SHIFT | Modifiers::ALT | Modifiers::LOGO;
        assert_eq!(
            mods.to_modifiers(),
            vec![
                Modifier::Ctrl,
                Modifier::Shift,
                Modifier::Alt,
                Modifier::Super,
            ]
        );
    }

    // IcedEventExt

    fn make_key_pressed(physical_key: key::Physical, modifiers: Modifiers) -> Event {
        Event::KeyPressed {
            key: iced_core::keyboard::Key::Unidentified,
            modified_key: iced_core::keyboard::Key::Unidentified,
            physical_key,
            location: Location::Standard,
            modifiers,
            text: None,
            repeat: false,
        }
    }

    fn make_key_released(physical_key: key::Physical, modifiers: Modifiers) -> Event {
        Event::KeyReleased {
            key: iced_core::keyboard::Key::Unidentified,
            modified_key: iced_core::keyboard::Key::Unidentified,
            physical_key,
            location: Location::Standard,
            modifiers,
        }
    }

    #[test]
    fn simple_key_press_to_hotkey() {
        let event = make_key_pressed(key::Physical::Code(key::Code::KeyC), Modifiers::empty());
        assert_eq!(event.to_hotkey(), Some(Hotkey::new(Key::C)));
    }

    #[test]
    fn key_press_with_ctrl_to_hotkey() {
        let event = make_key_pressed(key::Physical::Code(key::Code::KeyC), Modifiers::CTRL);
        assert_eq!(
            event.to_hotkey(),
            Some(Hotkey::new(Key::C).modifier(Modifier::Ctrl))
        );
    }

    #[test]
    fn key_press_with_multiple_modifiers() {
        let event = make_key_pressed(
            key::Physical::Code(key::Code::KeyA),
            Modifiers::CTRL | Modifiers::SHIFT,
        );
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
    fn key_release_to_hotkey() {
        let event = make_key_released(key::Physical::Code(key::Code::KeyC), Modifiers::CTRL);
        assert_eq!(
            event.to_hotkey(),
            Some(Hotkey::new(Key::C).modifier(Modifier::Ctrl))
        );
    }

    #[test]
    fn modifiers_changed_returns_none() {
        let event = Event::ModifiersChanged(Modifiers::CTRL);
        assert_eq!(event.to_hotkey(), None);
    }

    #[test]
    fn unidentified_key_event_returns_none() {
        let event = make_key_pressed(
            key::Physical::Unidentified(key::NativeCode::Unidentified),
            Modifiers::empty(),
        );
        assert_eq!(event.to_hotkey(), None);
    }

    #[test]
    fn modifier_key_strips_self() {
        // Pressing ShiftLeft — iced includes SHIFT in modifiers.
        // Hotkey should be just "ShiftLeft", not "Shift+ShiftLeft".
        let event = make_key_pressed(key::Physical::Code(key::Code::ShiftLeft), Modifiers::SHIFT);
        assert_eq!(event.to_hotkey(), Some(Hotkey::new(Key::SHIFT_LEFT)));
    }

    #[test]
    fn modifier_key_keeps_other_modifiers() {
        // Pressing ControlLeft while Shift is already held
        let event = make_key_pressed(
            key::Physical::Code(key::Code::ControlLeft),
            Modifiers::SHIFT | Modifiers::CTRL,
        );
        assert_eq!(
            event.to_hotkey(),
            Some(Hotkey::new(Key::CONTROL_LEFT).modifier(Modifier::Shift))
        );
    }

    #[test]
    fn ctrl_shift_f5_to_hotkey() {
        let event = make_key_pressed(
            key::Physical::Code(key::Code::F5),
            Modifiers::CTRL | Modifiers::SHIFT,
        );
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
    fn space_to_hotkey() {
        let event = make_key_pressed(key::Physical::Code(key::Code::Space), Modifiers::empty());
        assert_eq!(event.to_hotkey(), Some(Hotkey::new(Key::SPACE)));
    }

    #[test]
    fn super_key_strips_self() {
        // Pressing SuperLeft — iced includes LOGO in modifiers.
        let event = make_key_pressed(key::Physical::Code(key::Code::SuperLeft), Modifiers::LOGO);
        assert_eq!(event.to_hotkey(), Some(Hotkey::new(Key::META_LEFT)));
    }
}
