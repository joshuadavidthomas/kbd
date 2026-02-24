//! Winit key event conversions for `kbd-core`.
//!
//! This crate bridges winit's physical key model to `kbd-core`'s key types.
//! Both derive from the W3C UI Events specification, so the variant names
//! are nearly identical — the mapping is mechanical.
//!
//! Winit's `KeyEvent` does not carry modifier state; modifiers are tracked
//! separately via `WindowEvent::ModifiersChanged`. The [`WinitEventExt`]
//! trait therefore takes `ModifiersState` as a parameter.
//!
//! # Extension traits
//!
//! - [`WinitKeyExt`] — converts a winit [`PhysicalKey`] or [`KeyCode`] to
//!   a [`kbd_core::Key`].
//! - [`WinitModifiersExt`] — converts winit [`ModifiersState`] to a
//!   `Vec<Modifier>`.
//! - [`WinitEventExt`] — converts a winit [`KeyEvent`] plus
//!   [`ModifiersState`] to a [`kbd_core::Hotkey`].
//!
//! # Usage
//!
//! ```
//! use kbd_core::{Hotkey, Key, Modifier};
//! use kbd_winit::{WinitKeyExt, WinitModifiersExt};
//! use winit::keyboard::{KeyCode, ModifiersState, PhysicalKey};
//!
//! // KeyCode conversion
//! let key = KeyCode::KeyA.to_key();
//! assert_eq!(key, Some(Key::A));
//!
//! // PhysicalKey conversion
//! let key = PhysicalKey::Code(KeyCode::KeyA).to_key();
//! assert_eq!(key, Some(Key::A));
//!
//! // Modifier conversion
//! let mods = ModifiersState::CONTROL.to_modifiers();
//! assert_eq!(mods, vec![Modifier::Ctrl]);
//! ```

use kbd_core::Hotkey;
use kbd_core::Key;
use kbd_core::Modifier;
use winit::event::KeyEvent;
use winit::keyboard::KeyCode;
use winit::keyboard::ModifiersState;
use winit::keyboard::PhysicalKey;

/// Convert a winit key type to a `kbd-core` [`Key`].
///
/// Returns `None` for keys that have no `kbd-core` equivalent (e.g.,
/// `Unidentified`, keys beyond F24, TV remote keys).
pub trait WinitKeyExt {
    fn to_key(&self) -> Option<Key>;
}

impl WinitKeyExt for KeyCode {
    #[allow(clippy::too_many_lines)]
    fn to_key(&self) -> Option<Key> {
        match self {
            // Letters
            KeyCode::KeyA => Some(Key::A),
            KeyCode::KeyB => Some(Key::B),
            KeyCode::KeyC => Some(Key::C),
            KeyCode::KeyD => Some(Key::D),
            KeyCode::KeyE => Some(Key::E),
            KeyCode::KeyF => Some(Key::F),
            KeyCode::KeyG => Some(Key::G),
            KeyCode::KeyH => Some(Key::H),
            KeyCode::KeyI => Some(Key::I),
            KeyCode::KeyJ => Some(Key::J),
            KeyCode::KeyK => Some(Key::K),
            KeyCode::KeyL => Some(Key::L),
            KeyCode::KeyM => Some(Key::M),
            KeyCode::KeyN => Some(Key::N),
            KeyCode::KeyO => Some(Key::O),
            KeyCode::KeyP => Some(Key::P),
            KeyCode::KeyQ => Some(Key::Q),
            KeyCode::KeyR => Some(Key::R),
            KeyCode::KeyS => Some(Key::S),
            KeyCode::KeyT => Some(Key::T),
            KeyCode::KeyU => Some(Key::U),
            KeyCode::KeyV => Some(Key::V),
            KeyCode::KeyW => Some(Key::W),
            KeyCode::KeyX => Some(Key::X),
            KeyCode::KeyY => Some(Key::Y),
            KeyCode::KeyZ => Some(Key::Z),

            // Digits
            KeyCode::Digit0 => Some(Key::DIGIT0),
            KeyCode::Digit1 => Some(Key::DIGIT1),
            KeyCode::Digit2 => Some(Key::DIGIT2),
            KeyCode::Digit3 => Some(Key::DIGIT3),
            KeyCode::Digit4 => Some(Key::DIGIT4),
            KeyCode::Digit5 => Some(Key::DIGIT5),
            KeyCode::Digit6 => Some(Key::DIGIT6),
            KeyCode::Digit7 => Some(Key::DIGIT7),
            KeyCode::Digit8 => Some(Key::DIGIT8),
            KeyCode::Digit9 => Some(Key::DIGIT9),

            // Function keys
            KeyCode::F1 => Some(Key::F1),
            KeyCode::F2 => Some(Key::F2),
            KeyCode::F3 => Some(Key::F3),
            KeyCode::F4 => Some(Key::F4),
            KeyCode::F5 => Some(Key::F5),
            KeyCode::F6 => Some(Key::F6),
            KeyCode::F7 => Some(Key::F7),
            KeyCode::F8 => Some(Key::F8),
            KeyCode::F9 => Some(Key::F9),
            KeyCode::F10 => Some(Key::F10),
            KeyCode::F11 => Some(Key::F11),
            KeyCode::F12 => Some(Key::F12),
            KeyCode::F13 => Some(Key::F13),
            KeyCode::F14 => Some(Key::F14),
            KeyCode::F15 => Some(Key::F15),
            KeyCode::F16 => Some(Key::F16),
            KeyCode::F17 => Some(Key::F17),
            KeyCode::F18 => Some(Key::F18),
            KeyCode::F19 => Some(Key::F19),
            KeyCode::F20 => Some(Key::F20),
            KeyCode::F21 => Some(Key::F21),
            KeyCode::F22 => Some(Key::F22),
            KeyCode::F23 => Some(Key::F23),
            KeyCode::F24 => Some(Key::F24),
            KeyCode::F25 => Some(Key::F25),
            KeyCode::F26 => Some(Key::F26),
            KeyCode::F27 => Some(Key::F27),
            KeyCode::F28 => Some(Key::F28),
            KeyCode::F29 => Some(Key::F29),
            KeyCode::F30 => Some(Key::F30),
            KeyCode::F31 => Some(Key::F31),
            KeyCode::F32 => Some(Key::F32),
            KeyCode::F33 => Some(Key::F33),
            KeyCode::F34 => Some(Key::F34),
            KeyCode::F35 => Some(Key::F35),

            // Navigation and editing
            KeyCode::Enter => Some(Key::ENTER),
            KeyCode::Escape => Some(Key::ESCAPE),
            KeyCode::Space => Some(Key::SPACE),
            KeyCode::Tab => Some(Key::TAB),
            KeyCode::Delete => Some(Key::DELETE),
            KeyCode::Backspace => Some(Key::BACKSPACE),
            KeyCode::Insert => Some(Key::INSERT),
            KeyCode::CapsLock => Some(Key::CAPS_LOCK),
            KeyCode::Home => Some(Key::HOME),
            KeyCode::End => Some(Key::END),
            KeyCode::PageUp => Some(Key::PAGE_UP),
            KeyCode::PageDown => Some(Key::PAGE_DOWN),
            KeyCode::ArrowUp => Some(Key::ARROW_UP),
            KeyCode::ArrowDown => Some(Key::ARROW_DOWN),
            KeyCode::ArrowLeft => Some(Key::ARROW_LEFT),
            KeyCode::ArrowRight => Some(Key::ARROW_RIGHT),

            // Punctuation
            KeyCode::Minus => Some(Key::MINUS),
            KeyCode::Equal => Some(Key::EQUAL),
            KeyCode::BracketLeft => Some(Key::BRACKET_LEFT),
            KeyCode::BracketRight => Some(Key::BRACKET_RIGHT),
            KeyCode::Backslash => Some(Key::BACKSLASH),
            KeyCode::Semicolon => Some(Key::SEMICOLON),
            KeyCode::Quote => Some(Key::QUOTE),
            KeyCode::Backquote => Some(Key::BACKQUOTE),
            KeyCode::Comma => Some(Key::COMMA),
            KeyCode::Period => Some(Key::PERIOD),
            KeyCode::Slash => Some(Key::SLASH),

            // Numpad
            KeyCode::Numpad0 => Some(Key::NUMPAD0),
            KeyCode::Numpad1 => Some(Key::NUMPAD1),
            KeyCode::Numpad2 => Some(Key::NUMPAD2),
            KeyCode::Numpad3 => Some(Key::NUMPAD3),
            KeyCode::Numpad4 => Some(Key::NUMPAD4),
            KeyCode::Numpad5 => Some(Key::NUMPAD5),
            KeyCode::Numpad6 => Some(Key::NUMPAD6),
            KeyCode::Numpad7 => Some(Key::NUMPAD7),
            KeyCode::Numpad8 => Some(Key::NUMPAD8),
            KeyCode::Numpad9 => Some(Key::NUMPAD9),
            KeyCode::NumpadDecimal => Some(Key::NUMPAD_DECIMAL),
            KeyCode::NumpadAdd => Some(Key::NUMPAD_ADD),
            KeyCode::NumpadSubtract => Some(Key::NUMPAD_SUBTRACT),
            KeyCode::NumpadMultiply => Some(Key::NUMPAD_MULTIPLY),
            KeyCode::NumpadDivide => Some(Key::NUMPAD_DIVIDE),
            KeyCode::NumpadEnter => Some(Key::NUMPAD_ENTER),
            KeyCode::NumpadEqual => Some(Key::NUMPAD_EQUAL),
            KeyCode::NumpadComma => Some(Key::NUMPAD_COMMA),
            KeyCode::NumpadBackspace => Some(Key::NUMPAD_BACKSPACE),
            KeyCode::NumpadClear => Some(Key::NUMPAD_CLEAR),
            KeyCode::NumpadClearEntry => Some(Key::NUMPAD_CLEAR_ENTRY),
            KeyCode::NumpadHash => Some(Key::NUMPAD_HASH),
            KeyCode::NumpadMemoryAdd => Some(Key::NUMPAD_MEMORY_ADD),
            KeyCode::NumpadMemoryClear => Some(Key::NUMPAD_MEMORY_CLEAR),
            KeyCode::NumpadMemoryRecall => Some(Key::NUMPAD_MEMORY_RECALL),
            KeyCode::NumpadMemoryStore => Some(Key::NUMPAD_MEMORY_STORE),
            KeyCode::NumpadMemorySubtract => Some(Key::NUMPAD_MEMORY_SUBTRACT),
            KeyCode::NumpadParenLeft => Some(Key::NUMPAD_PAREN_LEFT),
            KeyCode::NumpadParenRight => Some(Key::NUMPAD_PAREN_RIGHT),
            KeyCode::NumpadStar => Some(Key::NUMPAD_STAR),

            // Modifiers — winit uses SuperLeft/SuperRight where W3C uses MetaLeft/MetaRight
            KeyCode::ControlLeft => Some(Key::CONTROL_LEFT),
            KeyCode::ControlRight => Some(Key::CONTROL_RIGHT),
            KeyCode::ShiftLeft => Some(Key::SHIFT_LEFT),
            KeyCode::ShiftRight => Some(Key::SHIFT_RIGHT),
            KeyCode::AltLeft => Some(Key::ALT_LEFT),
            KeyCode::AltRight => Some(Key::ALT_RIGHT),
            KeyCode::SuperLeft => Some(Key::META_LEFT),
            KeyCode::SuperRight => Some(Key::META_RIGHT),

            // Media keys
            KeyCode::AudioVolumeUp => Some(Key::AUDIO_VOLUME_UP),
            KeyCode::AudioVolumeDown => Some(Key::AUDIO_VOLUME_DOWN),
            KeyCode::AudioVolumeMute => Some(Key::AUDIO_VOLUME_MUTE),
            KeyCode::MediaPlayPause => Some(Key::MEDIA_PLAY_PAUSE),
            KeyCode::MediaStop => Some(Key::MEDIA_STOP),
            KeyCode::MediaTrackNext => Some(Key::MEDIA_TRACK_NEXT),
            KeyCode::MediaTrackPrevious => Some(Key::MEDIA_TRACK_PREVIOUS),
            KeyCode::MediaSelect => Some(Key::MEDIA_SELECT),

            // Browser keys
            KeyCode::BrowserBack => Some(Key::BROWSER_BACK),
            KeyCode::BrowserFavorites => Some(Key::BROWSER_FAVORITES),
            KeyCode::BrowserForward => Some(Key::BROWSER_FORWARD),
            KeyCode::BrowserHome => Some(Key::BROWSER_HOME),
            KeyCode::BrowserRefresh => Some(Key::BROWSER_REFRESH),
            KeyCode::BrowserSearch => Some(Key::BROWSER_SEARCH),
            KeyCode::BrowserStop => Some(Key::BROWSER_STOP),

            // System keys
            KeyCode::PrintScreen => Some(Key::PRINT_SCREEN),
            KeyCode::ScrollLock => Some(Key::SCROLL_LOCK),
            KeyCode::Pause => Some(Key::PAUSE),
            KeyCode::NumLock => Some(Key::NUM_LOCK),
            KeyCode::ContextMenu => Some(Key::CONTEXT_MENU),
            KeyCode::Power => Some(Key::POWER),
            KeyCode::Sleep => Some(Key::SLEEP),
            KeyCode::WakeUp => Some(Key::WAKE_UP),
            KeyCode::Eject => Some(Key::EJECT),

            // Clipboard / editing keys
            KeyCode::Copy => Some(Key::COPY),
            KeyCode::Cut => Some(Key::CUT),
            KeyCode::Paste => Some(Key::PASTE),
            KeyCode::Undo => Some(Key::UNDO),
            KeyCode::Find => Some(Key::FIND),
            KeyCode::Help => Some(Key::HELP),
            KeyCode::Open => Some(Key::OPEN),
            KeyCode::Select => Some(Key::SELECT),
            KeyCode::Again => Some(Key::AGAIN),
            KeyCode::Props => Some(Key::PROPS),
            KeyCode::Abort => Some(Key::ABORT),
            KeyCode::Resume => Some(Key::RESUME),
            KeyCode::Suspend => Some(Key::SUSPEND),

            // Fn and legacy
            KeyCode::Fn => Some(Key::FN),
            KeyCode::FnLock => Some(Key::FN_LOCK),
            KeyCode::Hyper => Some(Key::HYPER),
            KeyCode::Turbo => Some(Key::TURBO),

            // CJK / international
            KeyCode::Convert => Some(Key::CONVERT),
            KeyCode::NonConvert => Some(Key::NON_CONVERT),
            KeyCode::KanaMode => Some(Key::KANA_MODE),
            KeyCode::Hiragana => Some(Key::HIRAGANA),
            KeyCode::Katakana => Some(Key::KATAKANA),
            KeyCode::Lang1 => Some(Key::LANG1),
            KeyCode::Lang2 => Some(Key::LANG2),
            KeyCode::Lang3 => Some(Key::LANG3),
            KeyCode::Lang4 => Some(Key::LANG4),
            KeyCode::Lang5 => Some(Key::LANG5),
            KeyCode::IntlBackslash => Some(Key::INTL_BACKSLASH),
            KeyCode::IntlRo => Some(Key::INTL_RO),
            KeyCode::IntlYen => Some(Key::INTL_YEN),

            // App launch keys
            KeyCode::LaunchApp1 => Some(Key::LAUNCH_APP1),
            KeyCode::LaunchApp2 => Some(Key::LAUNCH_APP2),
            KeyCode::LaunchMail => Some(Key::LAUNCH_MAIL),

            _ => None,
        }
    }
}

impl WinitKeyExt for PhysicalKey {
    fn to_key(&self) -> Option<Key> {
        match self {
            PhysicalKey::Code(code) => code.to_key(),
            PhysicalKey::Unidentified(_) => None,
        }
    }
}

/// Convert winit [`ModifiersState`] bitflags to a sorted `Vec<Modifier>`.
pub trait WinitModifiersExt {
    fn to_modifiers(&self) -> Vec<Modifier>;
}

impl WinitModifiersExt for ModifiersState {
    fn to_modifiers(&self) -> Vec<Modifier> {
        let mut modifiers = Vec::new();
        if self.control_key() {
            modifiers.push(Modifier::Ctrl);
        }
        if self.shift_key() {
            modifiers.push(Modifier::Shift);
        }
        if self.alt_key() {
            modifiers.push(Modifier::Alt);
        }
        if self.super_key() {
            modifiers.push(Modifier::Super);
        }
        modifiers
    }
}

/// Build a [`Hotkey`] from a physical key and modifier state.
///
/// This is the logic behind [`WinitEventExt::to_hotkey`], exposed as a
/// standalone function for use when a [`KeyEvent`] is not available.
///
/// When the key is itself a modifier (e.g., `ControlLeft`), the
/// corresponding modifier flag is stripped — winit includes the pressed
/// modifier key in its own state, but `kbd-core` treats the key as the
/// trigger, not as a modifier of itself.
///
/// Returns `None` if the physical key has no `kbd-core` equivalent.
#[must_use]
pub fn physical_key_to_hotkey(
    physical_key: PhysicalKey,
    modifiers: ModifiersState,
) -> Option<Hotkey> {
    let key = physical_key.to_key()?;

    let mut mods = modifiers.to_modifiers();
    if let Some(self_modifier) = Modifier::from_key(key) {
        mods.retain(|m| *m != self_modifier);
    }

    Some(Hotkey::with_modifiers(key, mods))
}

/// Convert a winit [`KeyEvent`] (plus modifier state) to a `kbd-core`
/// [`Hotkey`].
///
/// Winit's `KeyEvent` does not include modifier state — modifiers are
/// tracked via `WindowEvent::ModifiersChanged`. Pass the current
/// `ModifiersState` alongside the event.
///
/// Returns `None` if the physical key has no `kbd-core` equivalent.
///
/// When the key is itself a modifier (e.g., `ControlLeft`), the
/// corresponding modifier flag is stripped from the modifiers — winit
/// includes the pressed modifier key in its own state, but `kbd-core`
/// treats the key as the trigger, not as a modifier of itself.
pub trait WinitEventExt {
    fn to_hotkey(&self, modifiers: ModifiersState) -> Option<Hotkey>;
}

impl WinitEventExt for KeyEvent {
    fn to_hotkey(&self, modifiers: ModifiersState) -> Option<Hotkey> {
        physical_key_to_hotkey(self.physical_key, modifiers)
    }
}

#[cfg(test)]
mod tests {
    use kbd_core::Hotkey;
    use kbd_core::Key;
    use kbd_core::Modifier;
    use winit::keyboard::KeyCode;
    use winit::keyboard::ModifiersState;
    use winit::keyboard::NativeKeyCode;
    use winit::keyboard::PhysicalKey;

    use super::*;

    // WinitKeyExt — KeyCode

    #[test]
    fn keycode_letters() {
        assert_eq!(KeyCode::KeyA.to_key(), Some(Key::A));
        assert_eq!(KeyCode::KeyZ.to_key(), Some(Key::Z));
    }

    #[test]
    fn keycode_digits() {
        assert_eq!(KeyCode::Digit0.to_key(), Some(Key::DIGIT0));
        assert_eq!(KeyCode::Digit9.to_key(), Some(Key::DIGIT9));
    }

    #[test]
    fn keycode_function_keys() {
        assert_eq!(KeyCode::F1.to_key(), Some(Key::F1));
        assert_eq!(KeyCode::F12.to_key(), Some(Key::F12));
        assert_eq!(KeyCode::F24.to_key(), Some(Key::F24));
        assert_eq!(KeyCode::F25.to_key(), Some(Key::F25));
    }

    #[test]
    fn keycode_navigation() {
        assert_eq!(KeyCode::Enter.to_key(), Some(Key::ENTER));
        assert_eq!(KeyCode::Escape.to_key(), Some(Key::ESCAPE));
        assert_eq!(KeyCode::Backspace.to_key(), Some(Key::BACKSPACE));
        assert_eq!(KeyCode::Tab.to_key(), Some(Key::TAB));
        assert_eq!(KeyCode::Space.to_key(), Some(Key::SPACE));
        assert_eq!(KeyCode::Delete.to_key(), Some(Key::DELETE));
        assert_eq!(KeyCode::Insert.to_key(), Some(Key::INSERT));
        assert_eq!(KeyCode::Home.to_key(), Some(Key::HOME));
        assert_eq!(KeyCode::End.to_key(), Some(Key::END));
        assert_eq!(KeyCode::PageUp.to_key(), Some(Key::PAGE_UP));
        assert_eq!(KeyCode::PageDown.to_key(), Some(Key::PAGE_DOWN));
        assert_eq!(KeyCode::ArrowUp.to_key(), Some(Key::ARROW_UP));
        assert_eq!(KeyCode::ArrowDown.to_key(), Some(Key::ARROW_DOWN));
        assert_eq!(KeyCode::ArrowLeft.to_key(), Some(Key::ARROW_LEFT));
        assert_eq!(KeyCode::ArrowRight.to_key(), Some(Key::ARROW_RIGHT));
    }

    #[test]
    fn keycode_modifiers() {
        assert_eq!(KeyCode::ControlLeft.to_key(), Some(Key::CONTROL_LEFT));
        assert_eq!(KeyCode::ControlRight.to_key(), Some(Key::CONTROL_RIGHT));
        assert_eq!(KeyCode::ShiftLeft.to_key(), Some(Key::SHIFT_LEFT));
        assert_eq!(KeyCode::ShiftRight.to_key(), Some(Key::SHIFT_RIGHT));
        assert_eq!(KeyCode::AltLeft.to_key(), Some(Key::ALT_LEFT));
        assert_eq!(KeyCode::AltRight.to_key(), Some(Key::ALT_RIGHT));
        // winit's SuperLeft/Right → kbd-core's MetaLeft/Right
        assert_eq!(KeyCode::SuperLeft.to_key(), Some(Key::META_LEFT));
        assert_eq!(KeyCode::SuperRight.to_key(), Some(Key::META_RIGHT));
    }

    #[test]
    fn keycode_punctuation() {
        assert_eq!(KeyCode::Minus.to_key(), Some(Key::MINUS));
        assert_eq!(KeyCode::Equal.to_key(), Some(Key::EQUAL));
        assert_eq!(KeyCode::BracketLeft.to_key(), Some(Key::BRACKET_LEFT));
        assert_eq!(KeyCode::BracketRight.to_key(), Some(Key::BRACKET_RIGHT));
        assert_eq!(KeyCode::Backslash.to_key(), Some(Key::BACKSLASH));
        assert_eq!(KeyCode::Semicolon.to_key(), Some(Key::SEMICOLON));
        assert_eq!(KeyCode::Quote.to_key(), Some(Key::QUOTE));
        assert_eq!(KeyCode::Backquote.to_key(), Some(Key::BACKQUOTE));
        assert_eq!(KeyCode::Comma.to_key(), Some(Key::COMMA));
        assert_eq!(KeyCode::Period.to_key(), Some(Key::PERIOD));
        assert_eq!(KeyCode::Slash.to_key(), Some(Key::SLASH));
    }

    #[test]
    fn keycode_numpad() {
        assert_eq!(KeyCode::Numpad0.to_key(), Some(Key::NUMPAD0));
        assert_eq!(KeyCode::Numpad9.to_key(), Some(Key::NUMPAD9));
        assert_eq!(KeyCode::NumpadDecimal.to_key(), Some(Key::NUMPAD_DECIMAL));
        assert_eq!(KeyCode::NumpadAdd.to_key(), Some(Key::NUMPAD_ADD));
        assert_eq!(KeyCode::NumpadSubtract.to_key(), Some(Key::NUMPAD_SUBTRACT));
        assert_eq!(KeyCode::NumpadMultiply.to_key(), Some(Key::NUMPAD_MULTIPLY));
        assert_eq!(KeyCode::NumpadDivide.to_key(), Some(Key::NUMPAD_DIVIDE));
        assert_eq!(KeyCode::NumpadEnter.to_key(), Some(Key::NUMPAD_ENTER));
    }

    #[test]
    fn keycode_media() {
        assert_eq!(
            KeyCode::MediaPlayPause.to_key(),
            Some(Key::MEDIA_PLAY_PAUSE)
        );
        assert_eq!(KeyCode::MediaStop.to_key(), Some(Key::MEDIA_STOP));
        assert_eq!(
            KeyCode::MediaTrackNext.to_key(),
            Some(Key::MEDIA_TRACK_NEXT)
        );
        assert_eq!(
            KeyCode::MediaTrackPrevious.to_key(),
            Some(Key::MEDIA_TRACK_PREVIOUS)
        );
        assert_eq!(KeyCode::AudioVolumeUp.to_key(), Some(Key::AUDIO_VOLUME_UP));
        assert_eq!(
            KeyCode::AudioVolumeDown.to_key(),
            Some(Key::AUDIO_VOLUME_DOWN)
        );
        assert_eq!(
            KeyCode::AudioVolumeMute.to_key(),
            Some(Key::AUDIO_VOLUME_MUTE)
        );
    }

    #[test]
    fn keycode_system() {
        assert_eq!(KeyCode::PrintScreen.to_key(), Some(Key::PRINT_SCREEN));
        assert_eq!(KeyCode::ScrollLock.to_key(), Some(Key::SCROLL_LOCK));
        assert_eq!(KeyCode::Pause.to_key(), Some(Key::PAUSE));
        assert_eq!(KeyCode::NumLock.to_key(), Some(Key::NUM_LOCK));
        assert_eq!(KeyCode::ContextMenu.to_key(), Some(Key::CONTEXT_MENU));
        assert_eq!(KeyCode::Power.to_key(), Some(Key::POWER));
    }

    #[test]
    fn keycode_extended_function_keys() {
        assert_eq!(KeyCode::F25.to_key(), Some(Key::F25));
        assert_eq!(KeyCode::F35.to_key(), Some(Key::F35));
    }

    #[test]
    fn keycode_browser_keys() {
        assert_eq!(KeyCode::BrowserBack.to_key(), Some(Key::BROWSER_BACK));
        assert_eq!(KeyCode::BrowserForward.to_key(), Some(Key::BROWSER_FORWARD));
        assert_eq!(KeyCode::BrowserHome.to_key(), Some(Key::BROWSER_HOME));
        assert_eq!(KeyCode::BrowserRefresh.to_key(), Some(Key::BROWSER_REFRESH));
        assert_eq!(KeyCode::BrowserSearch.to_key(), Some(Key::BROWSER_SEARCH));
        assert_eq!(KeyCode::BrowserStop.to_key(), Some(Key::BROWSER_STOP));
        assert_eq!(
            KeyCode::BrowserFavorites.to_key(),
            Some(Key::BROWSER_FAVORITES)
        );
    }

    #[test]
    fn keycode_clipboard_and_editing() {
        assert_eq!(KeyCode::Copy.to_key(), Some(Key::COPY));
        assert_eq!(KeyCode::Cut.to_key(), Some(Key::CUT));
        assert_eq!(KeyCode::Paste.to_key(), Some(Key::PASTE));
        assert_eq!(KeyCode::Undo.to_key(), Some(Key::UNDO));
        assert_eq!(KeyCode::Find.to_key(), Some(Key::FIND));
        assert_eq!(KeyCode::Help.to_key(), Some(Key::HELP));
        assert_eq!(KeyCode::Open.to_key(), Some(Key::OPEN));
        assert_eq!(KeyCode::Select.to_key(), Some(Key::SELECT));
        assert_eq!(KeyCode::Again.to_key(), Some(Key::AGAIN));
        assert_eq!(KeyCode::Props.to_key(), Some(Key::PROPS));
        assert_eq!(KeyCode::Abort.to_key(), Some(Key::ABORT));
    }

    #[test]
    fn keycode_system_keys_extended() {
        assert_eq!(KeyCode::Sleep.to_key(), Some(Key::SLEEP));
        assert_eq!(KeyCode::WakeUp.to_key(), Some(Key::WAKE_UP));
        assert_eq!(KeyCode::Eject.to_key(), Some(Key::EJECT));
        assert_eq!(KeyCode::Resume.to_key(), Some(Key::RESUME));
        assert_eq!(KeyCode::Suspend.to_key(), Some(Key::SUSPEND));
    }

    #[test]
    fn keycode_extended_numpad() {
        assert_eq!(KeyCode::NumpadEqual.to_key(), Some(Key::NUMPAD_EQUAL));
        assert_eq!(KeyCode::NumpadComma.to_key(), Some(Key::NUMPAD_COMMA));
        assert_eq!(
            KeyCode::NumpadBackspace.to_key(),
            Some(Key::NUMPAD_BACKSPACE)
        );
        assert_eq!(KeyCode::NumpadClear.to_key(), Some(Key::NUMPAD_CLEAR));
        assert_eq!(
            KeyCode::NumpadClearEntry.to_key(),
            Some(Key::NUMPAD_CLEAR_ENTRY)
        );
        assert_eq!(KeyCode::NumpadHash.to_key(), Some(Key::NUMPAD_HASH));
        assert_eq!(
            KeyCode::NumpadParenLeft.to_key(),
            Some(Key::NUMPAD_PAREN_LEFT)
        );
        assert_eq!(
            KeyCode::NumpadParenRight.to_key(),
            Some(Key::NUMPAD_PAREN_RIGHT)
        );
        assert_eq!(KeyCode::NumpadStar.to_key(), Some(Key::NUMPAD_STAR));
    }

    #[test]
    fn keycode_international_and_cjk() {
        assert_eq!(KeyCode::IntlBackslash.to_key(), Some(Key::INTL_BACKSLASH));
        assert_eq!(KeyCode::IntlRo.to_key(), Some(Key::INTL_RO));
        assert_eq!(KeyCode::IntlYen.to_key(), Some(Key::INTL_YEN));
        assert_eq!(KeyCode::Convert.to_key(), Some(Key::CONVERT));
        assert_eq!(KeyCode::NonConvert.to_key(), Some(Key::NON_CONVERT));
        assert_eq!(KeyCode::KanaMode.to_key(), Some(Key::KANA_MODE));
        assert_eq!(KeyCode::Hiragana.to_key(), Some(Key::HIRAGANA));
        assert_eq!(KeyCode::Katakana.to_key(), Some(Key::KATAKANA));
        assert_eq!(KeyCode::Lang1.to_key(), Some(Key::LANG1));
        assert_eq!(KeyCode::Lang2.to_key(), Some(Key::LANG2));
        assert_eq!(KeyCode::Lang3.to_key(), Some(Key::LANG3));
        assert_eq!(KeyCode::Lang4.to_key(), Some(Key::LANG4));
        assert_eq!(KeyCode::Lang5.to_key(), Some(Key::LANG5));
    }

    #[test]
    fn keycode_fn_and_legacy() {
        assert_eq!(KeyCode::Fn.to_key(), Some(Key::FN));
        assert_eq!(KeyCode::FnLock.to_key(), Some(Key::FN_LOCK));
        assert_eq!(KeyCode::Hyper.to_key(), Some(Key::HYPER));
        assert_eq!(KeyCode::Turbo.to_key(), Some(Key::TURBO));
    }

    #[test]
    fn keycode_launch_keys() {
        assert_eq!(KeyCode::LaunchApp1.to_key(), Some(Key::LAUNCH_APP1));
        assert_eq!(KeyCode::LaunchApp2.to_key(), Some(Key::LAUNCH_APP2));
        assert_eq!(KeyCode::LaunchMail.to_key(), Some(Key::LAUNCH_MAIL));
        assert_eq!(KeyCode::MediaSelect.to_key(), Some(Key::MEDIA_SELECT));
    }

    // WinitKeyExt — PhysicalKey

    #[test]
    fn physical_key_code_to_key() {
        let physical = PhysicalKey::Code(KeyCode::KeyA);
        assert_eq!(physical.to_key(), Some(Key::A));
    }

    #[test]
    fn physical_key_unidentified_returns_none() {
        let physical = PhysicalKey::Unidentified(NativeKeyCode::Unidentified);
        assert_eq!(physical.to_key(), None);
    }

    // WinitModifiersExt

    #[test]
    fn empty_modifiers() {
        assert_eq!(
            ModifiersState::empty().to_modifiers(),
            Vec::<Modifier>::new()
        );
    }

    #[test]
    fn single_modifiers() {
        assert_eq!(ModifiersState::CONTROL.to_modifiers(), vec![Modifier::Ctrl]);
        assert_eq!(ModifiersState::SHIFT.to_modifiers(), vec![Modifier::Shift]);
        assert_eq!(ModifiersState::ALT.to_modifiers(), vec![Modifier::Alt]);
        assert_eq!(ModifiersState::SUPER.to_modifiers(), vec![Modifier::Super]);
    }

    #[test]
    fn combined_modifiers() {
        let mods = ModifiersState::CONTROL | ModifiersState::SHIFT;
        assert_eq!(mods.to_modifiers(), vec![Modifier::Ctrl, Modifier::Shift]);
    }

    #[test]
    fn all_modifiers() {
        let mods = ModifiersState::CONTROL
            | ModifiersState::SHIFT
            | ModifiersState::ALT
            | ModifiersState::SUPER;
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

    // physical_key_to_hotkey (exercises the same logic as WinitEventExt)

    #[test]
    fn simple_key_to_hotkey() {
        let hotkey =
            physical_key_to_hotkey(PhysicalKey::Code(KeyCode::KeyC), ModifiersState::empty());
        assert_eq!(hotkey, Some(Hotkey::new(Key::C)));
    }

    #[test]
    fn key_with_ctrl_to_hotkey() {
        let hotkey =
            physical_key_to_hotkey(PhysicalKey::Code(KeyCode::KeyC), ModifiersState::CONTROL);
        assert_eq!(hotkey, Some(Hotkey::new(Key::C).modifier(Modifier::Ctrl)));
    }

    #[test]
    fn key_with_multiple_modifiers_to_hotkey() {
        let mods = ModifiersState::CONTROL | ModifiersState::SHIFT;
        let hotkey = physical_key_to_hotkey(PhysicalKey::Code(KeyCode::KeyA), mods);
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
    fn unidentified_key_to_hotkey_returns_none() {
        let hotkey = physical_key_to_hotkey(
            PhysicalKey::Unidentified(NativeKeyCode::Unidentified),
            ModifiersState::empty(),
        );
        assert_eq!(hotkey, None);
    }

    #[test]
    fn modifier_key_strips_self() {
        // Pressing ShiftLeft — winit reports SHIFT in ModifiersState.
        // Hotkey should be just "ShiftLeft", not "Shift+ShiftLeft".
        let hotkey =
            physical_key_to_hotkey(PhysicalKey::Code(KeyCode::ShiftLeft), ModifiersState::SHIFT);
        assert_eq!(hotkey, Some(Hotkey::new(Key::SHIFT_LEFT)));
    }

    #[test]
    fn modifier_key_keeps_other_modifiers() {
        // Pressing ControlLeft while Shift is already held
        let mods = ModifiersState::SHIFT | ModifiersState::CONTROL;
        let hotkey = physical_key_to_hotkey(PhysicalKey::Code(KeyCode::ControlLeft), mods);
        assert_eq!(
            hotkey,
            Some(Hotkey::new(Key::CONTROL_LEFT).modifier(Modifier::Shift))
        );
    }

    #[test]
    fn ctrl_shift_f5_to_hotkey() {
        let mods = ModifiersState::CONTROL | ModifiersState::SHIFT;
        let hotkey = physical_key_to_hotkey(PhysicalKey::Code(KeyCode::F5), mods);
        assert_eq!(
            hotkey,
            Some(
                Hotkey::new(Key::F5)
                    .modifier(Modifier::Ctrl)
                    .modifier(Modifier::Shift)
            )
        );
    }

    #[test]
    fn space_to_hotkey() {
        let hotkey =
            physical_key_to_hotkey(PhysicalKey::Code(KeyCode::Space), ModifiersState::empty());
        assert_eq!(hotkey, Some(Hotkey::new(Key::SPACE)));
    }

    #[test]
    fn super_key_strips_self() {
        // Pressing SuperLeft — winit reports SUPER in ModifiersState.
        let hotkey =
            physical_key_to_hotkey(PhysicalKey::Code(KeyCode::SuperLeft), ModifiersState::SUPER);
        assert_eq!(hotkey, Some(Hotkey::new(Key::META_LEFT)));
    }
}
