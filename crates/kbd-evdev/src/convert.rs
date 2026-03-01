//! Extension traits for converting between evdev key codes and `kbd` key types.
//!
//! These are extension traits rather than `From`/`Into` impls because of the
//! orphan rule: neither `evdev::KeyCode` nor `kbd::Key` is defined in
//! this crate, so we cannot implement foreign traits for foreign types.
//!
//! # Usage
//!
//! ```rust
//! use evdev::KeyCode;
//! use kbd::Key;
//! use kbd_evdev::KeyCodeExt;
//!
//! let key: Key = KeyCode::KEY_A.to_key();
//! assert_eq!(key, Key::A);
//! ```

use evdev::KeyCode;
use kbd::Key;

/// Extension trait on `evdev::KeyCode` for converting to `kbd::Key`.
pub trait KeyCodeExt {
    /// Convert this evdev key code to a `Key`.
    ///
    /// Returns `Key::UNIDENTIFIED` for key codes that don't have a mapping.
    fn to_key(self) -> Key;
}

/// Extension trait on `kbd::Key` for converting to `evdev::KeyCode`.
pub trait EvdevKeyExt {
    /// Convert this key to an evdev `KeyCode`.
    ///
    /// `Key::UNIDENTIFIED` maps to `KeyCode::KEY_UNKNOWN`.
    fn to_key_code(self) -> KeyCode;
}

impl KeyCodeExt for KeyCode {
    #[allow(clippy::too_many_lines)]
    fn to_key(self) -> Key {
        match self {
            KeyCode::KEY_A => Key::A,
            KeyCode::KEY_B => Key::B,
            KeyCode::KEY_C => Key::C,
            KeyCode::KEY_D => Key::D,
            KeyCode::KEY_E => Key::E,
            KeyCode::KEY_F => Key::F,
            KeyCode::KEY_G => Key::G,
            KeyCode::KEY_H => Key::H,
            KeyCode::KEY_I => Key::I,
            KeyCode::KEY_J => Key::J,
            KeyCode::KEY_K => Key::K,
            KeyCode::KEY_L => Key::L,
            KeyCode::KEY_M => Key::M,
            KeyCode::KEY_N => Key::N,
            KeyCode::KEY_O => Key::O,
            KeyCode::KEY_P => Key::P,
            KeyCode::KEY_Q => Key::Q,
            KeyCode::KEY_R => Key::R,
            KeyCode::KEY_S => Key::S,
            KeyCode::KEY_T => Key::T,
            KeyCode::KEY_U => Key::U,
            KeyCode::KEY_V => Key::V,
            KeyCode::KEY_W => Key::W,
            KeyCode::KEY_X => Key::X,
            KeyCode::KEY_Y => Key::Y,
            KeyCode::KEY_Z => Key::Z,
            KeyCode::KEY_0 => Key::DIGIT0,
            KeyCode::KEY_1 => Key::DIGIT1,
            KeyCode::KEY_2 => Key::DIGIT2,
            KeyCode::KEY_3 => Key::DIGIT3,
            KeyCode::KEY_4 => Key::DIGIT4,
            KeyCode::KEY_5 => Key::DIGIT5,
            KeyCode::KEY_6 => Key::DIGIT6,
            KeyCode::KEY_7 => Key::DIGIT7,
            KeyCode::KEY_8 => Key::DIGIT8,
            KeyCode::KEY_9 => Key::DIGIT9,
            KeyCode::KEY_F1 => Key::F1,
            KeyCode::KEY_F2 => Key::F2,
            KeyCode::KEY_F3 => Key::F3,
            KeyCode::KEY_F4 => Key::F4,
            KeyCode::KEY_F5 => Key::F5,
            KeyCode::KEY_F6 => Key::F6,
            KeyCode::KEY_F7 => Key::F7,
            KeyCode::KEY_F8 => Key::F8,
            KeyCode::KEY_F9 => Key::F9,
            KeyCode::KEY_F10 => Key::F10,
            KeyCode::KEY_F11 => Key::F11,
            KeyCode::KEY_F12 => Key::F12,
            KeyCode::KEY_F13 => Key::F13,
            KeyCode::KEY_F14 => Key::F14,
            KeyCode::KEY_F15 => Key::F15,
            KeyCode::KEY_F16 => Key::F16,
            KeyCode::KEY_F17 => Key::F17,
            KeyCode::KEY_F18 => Key::F18,
            KeyCode::KEY_F19 => Key::F19,
            KeyCode::KEY_F20 => Key::F20,
            KeyCode::KEY_F21 => Key::F21,
            KeyCode::KEY_F22 => Key::F22,
            KeyCode::KEY_F23 => Key::F23,
            KeyCode::KEY_F24 => Key::F24,
            KeyCode::KEY_ENTER => Key::ENTER,
            KeyCode::KEY_ESC => Key::ESCAPE,
            KeyCode::KEY_SPACE => Key::SPACE,
            KeyCode::KEY_TAB => Key::TAB,
            KeyCode::KEY_DELETE => Key::DELETE,
            KeyCode::KEY_BACKSPACE => Key::BACKSPACE,
            KeyCode::KEY_INSERT => Key::INSERT,
            KeyCode::KEY_CAPSLOCK => Key::CAPS_LOCK,
            KeyCode::KEY_HOME => Key::HOME,
            KeyCode::KEY_END => Key::END,
            KeyCode::KEY_PAGEUP => Key::PAGE_UP,
            KeyCode::KEY_PAGEDOWN => Key::PAGE_DOWN,
            KeyCode::KEY_UP => Key::ARROW_UP,
            KeyCode::KEY_DOWN => Key::ARROW_DOWN,
            KeyCode::KEY_LEFT => Key::ARROW_LEFT,
            KeyCode::KEY_RIGHT => Key::ARROW_RIGHT,
            KeyCode::KEY_MINUS => Key::MINUS,
            KeyCode::KEY_EQUAL => Key::EQUAL,
            KeyCode::KEY_LEFTBRACE => Key::BRACKET_LEFT,
            KeyCode::KEY_RIGHTBRACE => Key::BRACKET_RIGHT,
            KeyCode::KEY_BACKSLASH => Key::BACKSLASH,
            KeyCode::KEY_SEMICOLON => Key::SEMICOLON,
            KeyCode::KEY_APOSTROPHE => Key::QUOTE,
            KeyCode::KEY_GRAVE => Key::BACKQUOTE,
            KeyCode::KEY_COMMA => Key::COMMA,
            KeyCode::KEY_DOT => Key::PERIOD,
            KeyCode::KEY_SLASH => Key::SLASH,
            KeyCode::KEY_KP0 => Key::NUMPAD0,
            KeyCode::KEY_KP1 => Key::NUMPAD1,
            KeyCode::KEY_KP2 => Key::NUMPAD2,
            KeyCode::KEY_KP3 => Key::NUMPAD3,
            KeyCode::KEY_KP4 => Key::NUMPAD4,
            KeyCode::KEY_KP5 => Key::NUMPAD5,
            KeyCode::KEY_KP6 => Key::NUMPAD6,
            KeyCode::KEY_KP7 => Key::NUMPAD7,
            KeyCode::KEY_KP8 => Key::NUMPAD8,
            KeyCode::KEY_KP9 => Key::NUMPAD9,
            KeyCode::KEY_KPDOT => Key::NUMPAD_DECIMAL,
            KeyCode::KEY_KPPLUS => Key::NUMPAD_ADD,
            KeyCode::KEY_KPMINUS => Key::NUMPAD_SUBTRACT,
            KeyCode::KEY_KPASTERISK => Key::NUMPAD_MULTIPLY,
            KeyCode::KEY_KPSLASH => Key::NUMPAD_DIVIDE,
            KeyCode::KEY_KPENTER => Key::NUMPAD_ENTER,
            KeyCode::KEY_LEFTCTRL => Key::CONTROL_LEFT,
            KeyCode::KEY_RIGHTCTRL => Key::CONTROL_RIGHT,
            KeyCode::KEY_LEFTSHIFT => Key::SHIFT_LEFT,
            KeyCode::KEY_RIGHTSHIFT => Key::SHIFT_RIGHT,
            KeyCode::KEY_LEFTALT => Key::ALT_LEFT,
            KeyCode::KEY_RIGHTALT => Key::ALT_RIGHT,
            KeyCode::KEY_LEFTMETA => Key::META_LEFT,
            KeyCode::KEY_RIGHTMETA => Key::META_RIGHT,
            KeyCode::KEY_VOLUMEUP => Key::AUDIO_VOLUME_UP,
            KeyCode::KEY_VOLUMEDOWN => Key::AUDIO_VOLUME_DOWN,
            KeyCode::KEY_MUTE => Key::AUDIO_VOLUME_MUTE,
            KeyCode::KEY_PLAYPAUSE => Key::MEDIA_PLAY_PAUSE,
            KeyCode::KEY_STOPCD => Key::MEDIA_STOP,
            KeyCode::KEY_NEXTSONG => Key::MEDIA_TRACK_NEXT,
            KeyCode::KEY_PREVIOUSSONG => Key::MEDIA_TRACK_PREVIOUS,
            KeyCode::KEY_SYSRQ => Key::PRINT_SCREEN,
            KeyCode::KEY_SCROLLLOCK => Key::SCROLL_LOCK,
            KeyCode::KEY_PAUSE => Key::PAUSE,
            KeyCode::KEY_NUMLOCK => Key::NUM_LOCK,
            KeyCode::KEY_COMPOSE => Key::CONTEXT_MENU,
            KeyCode::KEY_POWER => Key::POWER,
            KeyCode::KEY_SLEEP => Key::SLEEP,
            KeyCode::KEY_WAKEUP => Key::WAKE_UP,
            KeyCode::KEY_EJECTCD => Key::EJECT,
            KeyCode::KEY_BRIGHTNESSDOWN => Key::BRIGHTNESS_DOWN,
            KeyCode::KEY_BRIGHTNESSUP => Key::BRIGHTNESS_UP,
            // Browser keys
            KeyCode::KEY_BACK => Key::BROWSER_BACK,
            KeyCode::KEY_FORWARD => Key::BROWSER_FORWARD,
            KeyCode::KEY_HOMEPAGE => Key::BROWSER_HOME,
            KeyCode::KEY_REFRESH => Key::BROWSER_REFRESH,
            KeyCode::KEY_SEARCH => Key::BROWSER_SEARCH,
            KeyCode::KEY_STOP => Key::BROWSER_STOP,
            KeyCode::KEY_BOOKMARKS => Key::BROWSER_FAVORITES,
            // Extended media
            KeyCode::KEY_PLAY => Key::MEDIA_PLAY,
            KeyCode::KEY_PAUSECD => Key::MEDIA_PAUSE,
            KeyCode::KEY_FASTFORWARD => Key::MEDIA_FAST_FORWARD,
            KeyCode::KEY_REWIND => Key::MEDIA_REWIND,
            KeyCode::KEY_RECORD => Key::MEDIA_RECORD,
            KeyCode::KEY_MEDIA => Key::MEDIA_SELECT,
            KeyCode::KEY_MICMUTE => Key::MICROPHONE_MUTE_TOGGLE,
            // Clipboard / editing
            KeyCode::KEY_COPY => Key::COPY,
            KeyCode::KEY_CUT => Key::CUT,
            KeyCode::KEY_PASTE => Key::PASTE,
            KeyCode::KEY_UNDO => Key::UNDO,
            KeyCode::KEY_FIND => Key::FIND,
            KeyCode::KEY_HELP => Key::HELP,
            KeyCode::KEY_OPEN => Key::OPEN,
            KeyCode::KEY_SELECT => Key::SELECT,
            KeyCode::KEY_AGAIN => Key::AGAIN,
            KeyCode::KEY_PROPS => Key::PROPS,
            KeyCode::KEY_CANCEL => Key::ABORT,
            KeyCode::KEY_SUSPEND => Key::SUSPEND,
            KeyCode::KEY_FRONT => Key::RESUME,
            // CJK / international
            KeyCode::KEY_HENKAN => Key::CONVERT,
            KeyCode::KEY_MUHENKAN => Key::NON_CONVERT,
            KeyCode::KEY_KATAKANAHIRAGANA => Key::KANA_MODE,
            KeyCode::KEY_HIRAGANA => Key::HIRAGANA,
            KeyCode::KEY_KATAKANA => Key::KATAKANA,
            KeyCode::KEY_HANGEUL => Key::LANG1,
            KeyCode::KEY_HANJA => Key::LANG2,
            KeyCode::KEY_102ND => Key::INTL_BACKSLASH,
            KeyCode::KEY_RO => Key::INTL_RO,
            KeyCode::KEY_YEN => Key::INTL_YEN,
            // App launch
            KeyCode::KEY_PROG1 => Key::LAUNCH_APP1,
            KeyCode::KEY_CALC => Key::LAUNCH_APP2,
            KeyCode::KEY_MAIL => Key::LAUNCH_MAIL,
            // Fn
            KeyCode::KEY_FN => Key::FN,
            // Extended numpad
            KeyCode::KEY_KPEQUAL => Key::NUMPAD_EQUAL,
            KeyCode::KEY_KPCOMMA => Key::NUMPAD_COMMA,
            KeyCode::KEY_KPLEFTPAREN => Key::NUMPAD_PAREN_LEFT,
            KeyCode::KEY_KPRIGHTPAREN => Key::NUMPAD_PAREN_RIGHT,
            _ => Key::UNIDENTIFIED,
        }
    }
}

impl EvdevKeyExt for Key {
    #[allow(clippy::too_many_lines)]
    fn to_key_code(self) -> KeyCode {
        match self {
            Key::A => KeyCode::KEY_A,
            Key::B => KeyCode::KEY_B,
            Key::C => KeyCode::KEY_C,
            Key::D => KeyCode::KEY_D,
            Key::E => KeyCode::KEY_E,
            Key::F => KeyCode::KEY_F,
            Key::G => KeyCode::KEY_G,
            Key::H => KeyCode::KEY_H,
            Key::I => KeyCode::KEY_I,
            Key::J => KeyCode::KEY_J,
            Key::K => KeyCode::KEY_K,
            Key::L => KeyCode::KEY_L,
            Key::M => KeyCode::KEY_M,
            Key::N => KeyCode::KEY_N,
            Key::O => KeyCode::KEY_O,
            Key::P => KeyCode::KEY_P,
            Key::Q => KeyCode::KEY_Q,
            Key::R => KeyCode::KEY_R,
            Key::S => KeyCode::KEY_S,
            Key::T => KeyCode::KEY_T,
            Key::U => KeyCode::KEY_U,
            Key::V => KeyCode::KEY_V,
            Key::W => KeyCode::KEY_W,
            Key::X => KeyCode::KEY_X,
            Key::Y => KeyCode::KEY_Y,
            Key::Z => KeyCode::KEY_Z,
            Key::DIGIT0 => KeyCode::KEY_0,
            Key::DIGIT1 => KeyCode::KEY_1,
            Key::DIGIT2 => KeyCode::KEY_2,
            Key::DIGIT3 => KeyCode::KEY_3,
            Key::DIGIT4 => KeyCode::KEY_4,
            Key::DIGIT5 => KeyCode::KEY_5,
            Key::DIGIT6 => KeyCode::KEY_6,
            Key::DIGIT7 => KeyCode::KEY_7,
            Key::DIGIT8 => KeyCode::KEY_8,
            Key::DIGIT9 => KeyCode::KEY_9,
            Key::F1 => KeyCode::KEY_F1,
            Key::F2 => KeyCode::KEY_F2,
            Key::F3 => KeyCode::KEY_F3,
            Key::F4 => KeyCode::KEY_F4,
            Key::F5 => KeyCode::KEY_F5,
            Key::F6 => KeyCode::KEY_F6,
            Key::F7 => KeyCode::KEY_F7,
            Key::F8 => KeyCode::KEY_F8,
            Key::F9 => KeyCode::KEY_F9,
            Key::F10 => KeyCode::KEY_F10,
            Key::F11 => KeyCode::KEY_F11,
            Key::F12 => KeyCode::KEY_F12,
            Key::F13 => KeyCode::KEY_F13,
            Key::F14 => KeyCode::KEY_F14,
            Key::F15 => KeyCode::KEY_F15,
            Key::F16 => KeyCode::KEY_F16,
            Key::F17 => KeyCode::KEY_F17,
            Key::F18 => KeyCode::KEY_F18,
            Key::F19 => KeyCode::KEY_F19,
            Key::F20 => KeyCode::KEY_F20,
            Key::F21 => KeyCode::KEY_F21,
            Key::F22 => KeyCode::KEY_F22,
            Key::F23 => KeyCode::KEY_F23,
            Key::F24 => KeyCode::KEY_F24,
            Key::ENTER => KeyCode::KEY_ENTER,
            Key::ESCAPE => KeyCode::KEY_ESC,
            Key::SPACE => KeyCode::KEY_SPACE,
            Key::TAB => KeyCode::KEY_TAB,
            Key::DELETE => KeyCode::KEY_DELETE,
            Key::BACKSPACE => KeyCode::KEY_BACKSPACE,
            Key::INSERT => KeyCode::KEY_INSERT,
            Key::CAPS_LOCK => KeyCode::KEY_CAPSLOCK,
            Key::HOME => KeyCode::KEY_HOME,
            Key::END => KeyCode::KEY_END,
            Key::PAGE_UP => KeyCode::KEY_PAGEUP,
            Key::PAGE_DOWN => KeyCode::KEY_PAGEDOWN,
            Key::ARROW_UP => KeyCode::KEY_UP,
            Key::ARROW_DOWN => KeyCode::KEY_DOWN,
            Key::ARROW_LEFT => KeyCode::KEY_LEFT,
            Key::ARROW_RIGHT => KeyCode::KEY_RIGHT,
            Key::MINUS => KeyCode::KEY_MINUS,
            Key::EQUAL => KeyCode::KEY_EQUAL,
            Key::BRACKET_LEFT => KeyCode::KEY_LEFTBRACE,
            Key::BRACKET_RIGHT => KeyCode::KEY_RIGHTBRACE,
            Key::BACKSLASH => KeyCode::KEY_BACKSLASH,
            Key::SEMICOLON => KeyCode::KEY_SEMICOLON,
            Key::QUOTE => KeyCode::KEY_APOSTROPHE,
            Key::BACKQUOTE => KeyCode::KEY_GRAVE,
            Key::COMMA => KeyCode::KEY_COMMA,
            Key::PERIOD => KeyCode::KEY_DOT,
            Key::SLASH => KeyCode::KEY_SLASH,
            Key::NUMPAD0 => KeyCode::KEY_KP0,
            Key::NUMPAD1 => KeyCode::KEY_KP1,
            Key::NUMPAD2 => KeyCode::KEY_KP2,
            Key::NUMPAD3 => KeyCode::KEY_KP3,
            Key::NUMPAD4 => KeyCode::KEY_KP4,
            Key::NUMPAD5 => KeyCode::KEY_KP5,
            Key::NUMPAD6 => KeyCode::KEY_KP6,
            Key::NUMPAD7 => KeyCode::KEY_KP7,
            Key::NUMPAD8 => KeyCode::KEY_KP8,
            Key::NUMPAD9 => KeyCode::KEY_KP9,
            Key::NUMPAD_DECIMAL => KeyCode::KEY_KPDOT,
            Key::NUMPAD_ADD => KeyCode::KEY_KPPLUS,
            Key::NUMPAD_SUBTRACT => KeyCode::KEY_KPMINUS,
            Key::NUMPAD_MULTIPLY => KeyCode::KEY_KPASTERISK,
            Key::NUMPAD_DIVIDE => KeyCode::KEY_KPSLASH,
            Key::NUMPAD_ENTER => KeyCode::KEY_KPENTER,
            Key::CONTROL_LEFT => KeyCode::KEY_LEFTCTRL,
            Key::CONTROL_RIGHT => KeyCode::KEY_RIGHTCTRL,
            Key::SHIFT_LEFT => KeyCode::KEY_LEFTSHIFT,
            Key::SHIFT_RIGHT => KeyCode::KEY_RIGHTSHIFT,
            Key::ALT_LEFT => KeyCode::KEY_LEFTALT,
            Key::ALT_RIGHT => KeyCode::KEY_RIGHTALT,
            Key::META_LEFT => KeyCode::KEY_LEFTMETA,
            Key::META_RIGHT => KeyCode::KEY_RIGHTMETA,
            Key::AUDIO_VOLUME_UP => KeyCode::KEY_VOLUMEUP,
            Key::AUDIO_VOLUME_DOWN => KeyCode::KEY_VOLUMEDOWN,
            Key::AUDIO_VOLUME_MUTE => KeyCode::KEY_MUTE,
            Key::MEDIA_PLAY_PAUSE => KeyCode::KEY_PLAYPAUSE,
            Key::MEDIA_STOP => KeyCode::KEY_STOPCD,
            Key::MEDIA_TRACK_NEXT => KeyCode::KEY_NEXTSONG,
            Key::MEDIA_TRACK_PREVIOUS => KeyCode::KEY_PREVIOUSSONG,
            Key::PRINT_SCREEN => KeyCode::KEY_SYSRQ,
            Key::SCROLL_LOCK => KeyCode::KEY_SCROLLLOCK,
            Key::PAUSE => KeyCode::KEY_PAUSE,
            Key::NUM_LOCK => KeyCode::KEY_NUMLOCK,
            Key::CONTEXT_MENU => KeyCode::KEY_COMPOSE,
            Key::POWER => KeyCode::KEY_POWER,
            Key::SLEEP => KeyCode::KEY_SLEEP,
            Key::WAKE_UP => KeyCode::KEY_WAKEUP,
            Key::EJECT => KeyCode::KEY_EJECTCD,
            Key::BRIGHTNESS_DOWN => KeyCode::KEY_BRIGHTNESSDOWN,
            Key::BRIGHTNESS_UP => KeyCode::KEY_BRIGHTNESSUP,
            Key::BROWSER_BACK => KeyCode::KEY_BACK,
            Key::BROWSER_FORWARD => KeyCode::KEY_FORWARD,
            Key::BROWSER_HOME => KeyCode::KEY_HOMEPAGE,
            Key::BROWSER_REFRESH => KeyCode::KEY_REFRESH,
            Key::BROWSER_SEARCH => KeyCode::KEY_SEARCH,
            Key::BROWSER_STOP => KeyCode::KEY_STOP,
            Key::BROWSER_FAVORITES => KeyCode::KEY_BOOKMARKS,
            Key::MEDIA_PLAY => KeyCode::KEY_PLAY,
            Key::MEDIA_PAUSE => KeyCode::KEY_PAUSECD,
            Key::MEDIA_FAST_FORWARD => KeyCode::KEY_FASTFORWARD,
            Key::MEDIA_REWIND => KeyCode::KEY_REWIND,
            Key::MEDIA_RECORD => KeyCode::KEY_RECORD,
            Key::MEDIA_SELECT => KeyCode::KEY_MEDIA,
            Key::MICROPHONE_MUTE_TOGGLE => KeyCode::KEY_MICMUTE,
            Key::COPY => KeyCode::KEY_COPY,
            Key::CUT => KeyCode::KEY_CUT,
            Key::PASTE => KeyCode::KEY_PASTE,
            Key::UNDO => KeyCode::KEY_UNDO,
            Key::FIND => KeyCode::KEY_FIND,
            Key::HELP => KeyCode::KEY_HELP,
            Key::OPEN => KeyCode::KEY_OPEN,
            Key::SELECT => KeyCode::KEY_SELECT,
            Key::AGAIN => KeyCode::KEY_AGAIN,
            Key::PROPS => KeyCode::KEY_PROPS,
            Key::ABORT => KeyCode::KEY_CANCEL,
            Key::SUSPEND => KeyCode::KEY_SUSPEND,
            Key::RESUME => KeyCode::KEY_FRONT,
            Key::CONVERT => KeyCode::KEY_HENKAN,
            Key::NON_CONVERT => KeyCode::KEY_MUHENKAN,
            Key::KANA_MODE => KeyCode::KEY_KATAKANAHIRAGANA,
            Key::HIRAGANA => KeyCode::KEY_HIRAGANA,
            Key::KATAKANA => KeyCode::KEY_KATAKANA,
            Key::LANG1 => KeyCode::KEY_HANGEUL,
            Key::LANG2 => KeyCode::KEY_HANJA,
            Key::INTL_BACKSLASH => KeyCode::KEY_102ND,
            Key::INTL_RO => KeyCode::KEY_RO,
            Key::INTL_YEN => KeyCode::KEY_YEN,
            Key::LAUNCH_APP1 => KeyCode::KEY_PROG1,
            Key::LAUNCH_APP2 => KeyCode::KEY_CALC,
            Key::LAUNCH_MAIL => KeyCode::KEY_MAIL,
            Key::FN => KeyCode::KEY_FN,
            Key::NUMPAD_EQUAL => KeyCode::KEY_KPEQUAL,
            Key::NUMPAD_COMMA => KeyCode::KEY_KPCOMMA,
            Key::NUMPAD_PAREN_LEFT => KeyCode::KEY_KPLEFTPAREN,
            Key::NUMPAD_PAREN_RIGHT => KeyCode::KEY_KPRIGHTPAREN,
            // Key wraps keyboard_types::Code which is #[non_exhaustive].
            // Codes without a known evdev mapping (including Key::UNIDENTIFIED)
            // fall back to KEY_UNKNOWN.
            _ => KeyCode::KEY_UNKNOWN,
        }
    }
}

#[cfg(test)]
mod tests {
    use evdev::KeyCode;
    use kbd::Key;

    use super::EvdevKeyExt;
    use super::KeyCodeExt;

    #[test]
    fn keycode_to_key_round_trip() {
        for key in [
            Key::A,
            Key::Z,
            Key::F24,
            Key::ENTER,
            Key::CAPS_LOCK,
            Key::NUMPAD_ENTER,
            Key::CONTROL_LEFT,
            Key::META_RIGHT,
        ] {
            let code = key.to_key_code();
            let parsed = code.to_key();
            assert_eq!(parsed, key, "round-trip failed for {key:?}");
        }
    }

    #[test]
    fn unmapped_keycode_maps_to_unknown() {
        // Use a key code that has no Key:: constant or mapping
        let key = KeyCode::KEY_PROG2.to_key();
        assert_eq!(key, Key::UNIDENTIFIED);
    }

    #[test]
    fn media_keys_round_trip() {
        for key in [
            Key::AUDIO_VOLUME_UP,
            Key::AUDIO_VOLUME_DOWN,
            Key::AUDIO_VOLUME_MUTE,
            Key::MEDIA_PLAY_PAUSE,
            Key::MEDIA_STOP,
            Key::MEDIA_TRACK_NEXT,
            Key::MEDIA_TRACK_PREVIOUS,
        ] {
            let code = key.to_key_code();
            let parsed = code.to_key();
            assert_eq!(parsed, key, "round-trip failed for {key:?}");
        }
    }

    #[test]
    fn system_keys_round_trip() {
        for key in [
            Key::PRINT_SCREEN,
            Key::SCROLL_LOCK,
            Key::PAUSE,
            Key::NUM_LOCK,
            Key::CONTEXT_MENU,
            Key::POWER,
        ] {
            let code = key.to_key_code();
            let parsed = code.to_key();
            assert_eq!(parsed, key, "round-trip failed for {key:?}");
        }
    }

    #[test]
    fn extended_keys_round_trip() {
        let extended_keys = [
            Key::SLEEP,
            Key::WAKE_UP,
            Key::EJECT,
            Key::BRIGHTNESS_DOWN,
            Key::BRIGHTNESS_UP,
            Key::BROWSER_BACK,
            Key::BROWSER_FORWARD,
            Key::BROWSER_HOME,
            Key::BROWSER_REFRESH,
            Key::BROWSER_SEARCH,
            Key::BROWSER_STOP,
            Key::BROWSER_FAVORITES,
            Key::MEDIA_PLAY,
            Key::MEDIA_PAUSE,
            Key::MEDIA_FAST_FORWARD,
            Key::MEDIA_REWIND,
            Key::MEDIA_RECORD,
            Key::MEDIA_SELECT,
            Key::MICROPHONE_MUTE_TOGGLE,
            Key::COPY,
            Key::CUT,
            Key::PASTE,
            Key::UNDO,
            Key::FIND,
            Key::HELP,
            Key::OPEN,
            Key::SELECT,
            Key::AGAIN,
            Key::PROPS,
            Key::ABORT,
            Key::SUSPEND,
            Key::RESUME,
            Key::CONVERT,
            Key::NON_CONVERT,
            Key::KANA_MODE,
            Key::HIRAGANA,
            Key::KATAKANA,
            Key::LANG1,
            Key::LANG2,
            Key::INTL_BACKSLASH,
            Key::INTL_RO,
            Key::INTL_YEN,
            Key::LAUNCH_APP1,
            Key::LAUNCH_APP2,
            Key::LAUNCH_MAIL,
            Key::FN,
            Key::NUMPAD_EQUAL,
            Key::NUMPAD_COMMA,
            Key::NUMPAD_PAREN_LEFT,
            Key::NUMPAD_PAREN_RIGHT,
        ];
        for key in extended_keys {
            let code = key.to_key_code();
            let parsed = code.to_key();
            assert_eq!(parsed, key, "round-trip failed for {key:?}");
        }
    }

    #[test]
    fn unknown_key_maps_to_key_unknown() {
        let code = Key::UNIDENTIFIED.to_key_code();
        assert_eq!(code, KeyCode::KEY_UNKNOWN);
    }

    #[test]
    fn all_letters_round_trip() {
        let letters = [
            Key::A,
            Key::B,
            Key::C,
            Key::D,
            Key::E,
            Key::F,
            Key::G,
            Key::H,
            Key::I,
            Key::J,
            Key::K,
            Key::L,
            Key::M,
            Key::N,
            Key::O,
            Key::P,
            Key::Q,
            Key::R,
            Key::S,
            Key::T,
            Key::U,
            Key::V,
            Key::W,
            Key::X,
            Key::Y,
            Key::Z,
        ];
        for key in letters {
            let code = key.to_key_code();
            let parsed = code.to_key();
            assert_eq!(parsed, key, "round-trip failed for {key:?}");
        }
    }

    #[test]
    fn all_modifiers_round_trip() {
        let modifiers = [
            Key::CONTROL_LEFT,
            Key::CONTROL_RIGHT,
            Key::SHIFT_LEFT,
            Key::SHIFT_RIGHT,
            Key::ALT_LEFT,
            Key::ALT_RIGHT,
            Key::META_LEFT,
            Key::META_RIGHT,
        ];
        for key in modifiers {
            let code = key.to_key_code();
            let parsed = code.to_key();
            assert_eq!(parsed, key, "round-trip failed for {key:?}");
        }
    }
}
