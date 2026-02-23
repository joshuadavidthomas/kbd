//! Extension traits for converting between evdev key codes and `kbd-core` key types.
//!
//! These are extension traits rather than `From`/`Into` impls because of the
//! orphan rule: neither `evdev::KeyCode` nor `kbd_core::Key` is defined in
//! this crate, so we cannot implement foreign traits for foreign types.
//!
//! # Usage
//!
//! ```rust
//! use evdev::KeyCode;
//! use kbd_core::Key;
//! use kbd_evdev::KeyCodeExt;
//!
//! let key: Key = KeyCode::KEY_A.to_key();
//! assert_eq!(key, Key::A);
//! ```

use evdev::KeyCode;
use kbd_core::Key;

/// Extension trait on `evdev::KeyCode` for converting to `kbd_core::Key`.
pub trait KeyCodeExt {
    /// Convert this evdev key code to a `Key`.
    ///
    /// Returns `Key::Unknown` for key codes that don't have a mapping.
    fn to_key(self) -> Key;
}

/// Extension trait on `kbd_core::Key` for converting to `evdev::KeyCode`.
pub trait EvdevKeyExt {
    /// Convert this key to an evdev `KeyCode`.
    ///
    /// `Key::Unknown` maps to `KeyCode::KEY_UNKNOWN`.
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
            KeyCode::KEY_0 => Key::Num0,
            KeyCode::KEY_1 => Key::Num1,
            KeyCode::KEY_2 => Key::Num2,
            KeyCode::KEY_3 => Key::Num3,
            KeyCode::KEY_4 => Key::Num4,
            KeyCode::KEY_5 => Key::Num5,
            KeyCode::KEY_6 => Key::Num6,
            KeyCode::KEY_7 => Key::Num7,
            KeyCode::KEY_8 => Key::Num8,
            KeyCode::KEY_9 => Key::Num9,
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
            KeyCode::KEY_ENTER => Key::Enter,
            KeyCode::KEY_ESC => Key::Escape,
            KeyCode::KEY_SPACE => Key::Space,
            KeyCode::KEY_TAB => Key::Tab,
            KeyCode::KEY_DELETE => Key::Delete,
            KeyCode::KEY_BACKSPACE => Key::Backspace,
            KeyCode::KEY_INSERT => Key::Insert,
            KeyCode::KEY_CAPSLOCK => Key::CapsLock,
            KeyCode::KEY_HOME => Key::Home,
            KeyCode::KEY_END => Key::End,
            KeyCode::KEY_PAGEUP => Key::PageUp,
            KeyCode::KEY_PAGEDOWN => Key::PageDown,
            KeyCode::KEY_UP => Key::Up,
            KeyCode::KEY_DOWN => Key::Down,
            KeyCode::KEY_LEFT => Key::Left,
            KeyCode::KEY_RIGHT => Key::Right,
            KeyCode::KEY_MINUS => Key::Minus,
            KeyCode::KEY_EQUAL => Key::Equal,
            KeyCode::KEY_LEFTBRACE => Key::LeftBracket,
            KeyCode::KEY_RIGHTBRACE => Key::RightBracket,
            KeyCode::KEY_BACKSLASH => Key::Backslash,
            KeyCode::KEY_SEMICOLON => Key::Semicolon,
            KeyCode::KEY_APOSTROPHE => Key::Apostrophe,
            KeyCode::KEY_GRAVE => Key::Grave,
            KeyCode::KEY_COMMA => Key::Comma,
            KeyCode::KEY_DOT => Key::Period,
            KeyCode::KEY_SLASH => Key::Slash,
            KeyCode::KEY_KP0 => Key::Numpad0,
            KeyCode::KEY_KP1 => Key::Numpad1,
            KeyCode::KEY_KP2 => Key::Numpad2,
            KeyCode::KEY_KP3 => Key::Numpad3,
            KeyCode::KEY_KP4 => Key::Numpad4,
            KeyCode::KEY_KP5 => Key::Numpad5,
            KeyCode::KEY_KP6 => Key::Numpad6,
            KeyCode::KEY_KP7 => Key::Numpad7,
            KeyCode::KEY_KP8 => Key::Numpad8,
            KeyCode::KEY_KP9 => Key::Numpad9,
            KeyCode::KEY_KPDOT => Key::NumpadDot,
            KeyCode::KEY_KPPLUS => Key::NumpadPlus,
            KeyCode::KEY_KPMINUS => Key::NumpadMinus,
            KeyCode::KEY_KPASTERISK => Key::NumpadMultiply,
            KeyCode::KEY_KPSLASH => Key::NumpadDivide,
            KeyCode::KEY_KPENTER => Key::NumpadEnter,
            KeyCode::KEY_LEFTCTRL => Key::LeftCtrl,
            KeyCode::KEY_RIGHTCTRL => Key::RightCtrl,
            KeyCode::KEY_LEFTSHIFT => Key::LeftShift,
            KeyCode::KEY_RIGHTSHIFT => Key::RightShift,
            KeyCode::KEY_LEFTALT => Key::LeftAlt,
            KeyCode::KEY_RIGHTALT => Key::RightAlt,
            KeyCode::KEY_LEFTMETA => Key::LeftSuper,
            KeyCode::KEY_RIGHTMETA => Key::RightSuper,
            _ => Key::Unknown,
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
            Key::Num0 => KeyCode::KEY_0,
            Key::Num1 => KeyCode::KEY_1,
            Key::Num2 => KeyCode::KEY_2,
            Key::Num3 => KeyCode::KEY_3,
            Key::Num4 => KeyCode::KEY_4,
            Key::Num5 => KeyCode::KEY_5,
            Key::Num6 => KeyCode::KEY_6,
            Key::Num7 => KeyCode::KEY_7,
            Key::Num8 => KeyCode::KEY_8,
            Key::Num9 => KeyCode::KEY_9,
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
            Key::Enter => KeyCode::KEY_ENTER,
            Key::Escape => KeyCode::KEY_ESC,
            Key::Space => KeyCode::KEY_SPACE,
            Key::Tab => KeyCode::KEY_TAB,
            Key::Delete => KeyCode::KEY_DELETE,
            Key::Backspace => KeyCode::KEY_BACKSPACE,
            Key::Insert => KeyCode::KEY_INSERT,
            Key::CapsLock => KeyCode::KEY_CAPSLOCK,
            Key::Home => KeyCode::KEY_HOME,
            Key::End => KeyCode::KEY_END,
            Key::PageUp => KeyCode::KEY_PAGEUP,
            Key::PageDown => KeyCode::KEY_PAGEDOWN,
            Key::Up => KeyCode::KEY_UP,
            Key::Down => KeyCode::KEY_DOWN,
            Key::Left => KeyCode::KEY_LEFT,
            Key::Right => KeyCode::KEY_RIGHT,
            Key::Minus => KeyCode::KEY_MINUS,
            Key::Equal => KeyCode::KEY_EQUAL,
            Key::LeftBracket => KeyCode::KEY_LEFTBRACE,
            Key::RightBracket => KeyCode::KEY_RIGHTBRACE,
            Key::Backslash => KeyCode::KEY_BACKSLASH,
            Key::Semicolon => KeyCode::KEY_SEMICOLON,
            Key::Apostrophe => KeyCode::KEY_APOSTROPHE,
            Key::Grave => KeyCode::KEY_GRAVE,
            Key::Comma => KeyCode::KEY_COMMA,
            Key::Period => KeyCode::KEY_DOT,
            Key::Slash => KeyCode::KEY_SLASH,
            Key::Numpad0 => KeyCode::KEY_KP0,
            Key::Numpad1 => KeyCode::KEY_KP1,
            Key::Numpad2 => KeyCode::KEY_KP2,
            Key::Numpad3 => KeyCode::KEY_KP3,
            Key::Numpad4 => KeyCode::KEY_KP4,
            Key::Numpad5 => KeyCode::KEY_KP5,
            Key::Numpad6 => KeyCode::KEY_KP6,
            Key::Numpad7 => KeyCode::KEY_KP7,
            Key::Numpad8 => KeyCode::KEY_KP8,
            Key::Numpad9 => KeyCode::KEY_KP9,
            Key::NumpadDot => KeyCode::KEY_KPDOT,
            Key::NumpadPlus => KeyCode::KEY_KPPLUS,
            Key::NumpadMinus => KeyCode::KEY_KPMINUS,
            Key::NumpadMultiply => KeyCode::KEY_KPASTERISK,
            Key::NumpadDivide => KeyCode::KEY_KPSLASH,
            Key::NumpadEnter => KeyCode::KEY_KPENTER,
            Key::LeftCtrl => KeyCode::KEY_LEFTCTRL,
            Key::RightCtrl => KeyCode::KEY_RIGHTCTRL,
            Key::LeftShift => KeyCode::KEY_LEFTSHIFT,
            Key::RightShift => KeyCode::KEY_RIGHTSHIFT,
            Key::LeftAlt => KeyCode::KEY_LEFTALT,
            Key::RightAlt => KeyCode::KEY_RIGHTALT,
            Key::LeftSuper => KeyCode::KEY_LEFTMETA,
            Key::RightSuper => KeyCode::KEY_RIGHTMETA,
            Key::Unknown => KeyCode::KEY_UNKNOWN,
        }
    }
}

#[cfg(test)]
mod tests {
    use evdev::KeyCode;
    use kbd_core::Key;

    use super::EvdevKeyExt;
    use super::KeyCodeExt;

    #[test]
    fn keycode_to_key_round_trip() {
        for key in [
            Key::A,
            Key::Z,
            Key::F24,
            Key::Enter,
            Key::CapsLock,
            Key::NumpadEnter,
            Key::LeftCtrl,
            Key::RightSuper,
        ] {
            let code = key.to_key_code();
            let parsed = code.to_key();
            assert_eq!(parsed, key, "round-trip failed for {key:?}");
        }
    }

    #[test]
    fn unknown_keycode_maps_to_unknown() {
        let key = KeyCode::KEY_VOLUMEUP.to_key();
        assert_eq!(key, Key::Unknown);
    }

    #[test]
    fn unknown_key_maps_to_key_unknown() {
        let code = Key::Unknown.to_key_code();
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
            Key::LeftCtrl,
            Key::RightCtrl,
            Key::LeftShift,
            Key::RightShift,
            Key::LeftAlt,
            Key::RightAlt,
            Key::LeftSuper,
            Key::RightSuper,
        ];
        for key in modifiers {
            let code = key.to_key_code();
            let parsed = code.to_key();
            assert_eq!(parsed, key, "round-trip failed for {key:?}");
        }
    }
}
