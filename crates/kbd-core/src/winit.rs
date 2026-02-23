//! Conversions between [`Key`] and [`winit::keyboard::KeyCode`] (`keyboard_types::Code`).
//!
//! Enabled by the `winit` feature flag. Since winit re-exports `keyboard_types::Code`
//! as `winit::keyboard::KeyCode`, these conversions work transparently for winit users.
//!
//! Any framework that uses `keyboard_types::Code` (winit, iced, etc.) gets these
//! conversions for free. Other windowing frameworks (Smithay keysyms, etc.) can be
//! added via additional feature flags on demand.
//!
//! # Example
//!
//! ```
//! use kbd_core::Key;
//! use keyboard_types::Code;
//!
//! let key = Key::from(Code::KeyA);
//! assert_eq!(key, Key::A);
//!
//! let code = Code::from(key);
//! assert_eq!(code, Code::KeyA);
//! ```

use keyboard_types::Code;

use crate::Key;

impl From<Code> for Key {
    #[allow(clippy::too_many_lines)]
    fn from(code: Code) -> Self {
        match code {
            Code::KeyA => Self::A,
            Code::KeyB => Self::B,
            Code::KeyC => Self::C,
            Code::KeyD => Self::D,
            Code::KeyE => Self::E,
            Code::KeyF => Self::F,
            Code::KeyG => Self::G,
            Code::KeyH => Self::H,
            Code::KeyI => Self::I,
            Code::KeyJ => Self::J,
            Code::KeyK => Self::K,
            Code::KeyL => Self::L,
            Code::KeyM => Self::M,
            Code::KeyN => Self::N,
            Code::KeyO => Self::O,
            Code::KeyP => Self::P,
            Code::KeyQ => Self::Q,
            Code::KeyR => Self::R,
            Code::KeyS => Self::S,
            Code::KeyT => Self::T,
            Code::KeyU => Self::U,
            Code::KeyV => Self::V,
            Code::KeyW => Self::W,
            Code::KeyX => Self::X,
            Code::KeyY => Self::Y,
            Code::KeyZ => Self::Z,
            Code::Digit0 => Self::Num0,
            Code::Digit1 => Self::Num1,
            Code::Digit2 => Self::Num2,
            Code::Digit3 => Self::Num3,
            Code::Digit4 => Self::Num4,
            Code::Digit5 => Self::Num5,
            Code::Digit6 => Self::Num6,
            Code::Digit7 => Self::Num7,
            Code::Digit8 => Self::Num8,
            Code::Digit9 => Self::Num9,
            Code::F1 => Self::F1,
            Code::F2 => Self::F2,
            Code::F3 => Self::F3,
            Code::F4 => Self::F4,
            Code::F5 => Self::F5,
            Code::F6 => Self::F6,
            Code::F7 => Self::F7,
            Code::F8 => Self::F8,
            Code::F9 => Self::F9,
            Code::F10 => Self::F10,
            Code::F11 => Self::F11,
            Code::F12 => Self::F12,
            Code::F13 => Self::F13,
            Code::F14 => Self::F14,
            Code::F15 => Self::F15,
            Code::F16 => Self::F16,
            Code::F17 => Self::F17,
            Code::F18 => Self::F18,
            Code::F19 => Self::F19,
            Code::F20 => Self::F20,
            Code::F21 => Self::F21,
            Code::F22 => Self::F22,
            Code::F23 => Self::F23,
            Code::F24 => Self::F24,
            Code::Enter => Self::Enter,
            Code::Escape => Self::Escape,
            Code::Space => Self::Space,
            Code::Tab => Self::Tab,
            Code::Delete => Self::Delete,
            Code::Backspace => Self::Backspace,
            Code::Insert => Self::Insert,
            Code::CapsLock => Self::CapsLock,
            Code::Home => Self::Home,
            Code::End => Self::End,
            Code::PageUp => Self::PageUp,
            Code::PageDown => Self::PageDown,
            Code::ArrowUp => Self::Up,
            Code::ArrowDown => Self::Down,
            Code::ArrowLeft => Self::Left,
            Code::ArrowRight => Self::Right,
            Code::Minus => Self::Minus,
            Code::Equal => Self::Equal,
            Code::BracketLeft => Self::LeftBracket,
            Code::BracketRight => Self::RightBracket,
            Code::Backslash => Self::Backslash,
            Code::Semicolon => Self::Semicolon,
            Code::Quote => Self::Apostrophe,
            Code::Backquote => Self::Grave,
            Code::Comma => Self::Comma,
            Code::Period => Self::Period,
            Code::Slash => Self::Slash,
            Code::Numpad0 => Self::Numpad0,
            Code::Numpad1 => Self::Numpad1,
            Code::Numpad2 => Self::Numpad2,
            Code::Numpad3 => Self::Numpad3,
            Code::Numpad4 => Self::Numpad4,
            Code::Numpad5 => Self::Numpad5,
            Code::Numpad6 => Self::Numpad6,
            Code::Numpad7 => Self::Numpad7,
            Code::Numpad8 => Self::Numpad8,
            Code::Numpad9 => Self::Numpad9,
            Code::NumpadDecimal => Self::NumpadDot,
            Code::NumpadAdd => Self::NumpadPlus,
            Code::NumpadSubtract => Self::NumpadMinus,
            Code::NumpadMultiply => Self::NumpadMultiply,
            Code::NumpadDivide => Self::NumpadDivide,
            Code::NumpadEnter => Self::NumpadEnter,
            Code::ControlLeft => Self::LeftCtrl,
            Code::ControlRight => Self::RightCtrl,
            Code::ShiftLeft => Self::LeftShift,
            Code::ShiftRight => Self::RightShift,
            Code::AltLeft => Self::LeftAlt,
            Code::AltRight => Self::RightAlt,
            Code::MetaLeft => Self::LeftSuper,
            Code::MetaRight => Self::RightSuper,
            _ => Self::Unknown,
        }
    }
}

impl From<Key> for Code {
    #[allow(clippy::too_many_lines)]
    fn from(key: Key) -> Self {
        match key {
            Key::A => Self::KeyA,
            Key::B => Self::KeyB,
            Key::C => Self::KeyC,
            Key::D => Self::KeyD,
            Key::E => Self::KeyE,
            Key::F => Self::KeyF,
            Key::G => Self::KeyG,
            Key::H => Self::KeyH,
            Key::I => Self::KeyI,
            Key::J => Self::KeyJ,
            Key::K => Self::KeyK,
            Key::L => Self::KeyL,
            Key::M => Self::KeyM,
            Key::N => Self::KeyN,
            Key::O => Self::KeyO,
            Key::P => Self::KeyP,
            Key::Q => Self::KeyQ,
            Key::R => Self::KeyR,
            Key::S => Self::KeyS,
            Key::T => Self::KeyT,
            Key::U => Self::KeyU,
            Key::V => Self::KeyV,
            Key::W => Self::KeyW,
            Key::X => Self::KeyX,
            Key::Y => Self::KeyY,
            Key::Z => Self::KeyZ,
            Key::Num0 => Self::Digit0,
            Key::Num1 => Self::Digit1,
            Key::Num2 => Self::Digit2,
            Key::Num3 => Self::Digit3,
            Key::Num4 => Self::Digit4,
            Key::Num5 => Self::Digit5,
            Key::Num6 => Self::Digit6,
            Key::Num7 => Self::Digit7,
            Key::Num8 => Self::Digit8,
            Key::Num9 => Self::Digit9,
            Key::F1 => Self::F1,
            Key::F2 => Self::F2,
            Key::F3 => Self::F3,
            Key::F4 => Self::F4,
            Key::F5 => Self::F5,
            Key::F6 => Self::F6,
            Key::F7 => Self::F7,
            Key::F8 => Self::F8,
            Key::F9 => Self::F9,
            Key::F10 => Self::F10,
            Key::F11 => Self::F11,
            Key::F12 => Self::F12,
            Key::F13 => Self::F13,
            Key::F14 => Self::F14,
            Key::F15 => Self::F15,
            Key::F16 => Self::F16,
            Key::F17 => Self::F17,
            Key::F18 => Self::F18,
            Key::F19 => Self::F19,
            Key::F20 => Self::F20,
            Key::F21 => Self::F21,
            Key::F22 => Self::F22,
            Key::F23 => Self::F23,
            Key::F24 => Self::F24,
            Key::Enter => Self::Enter,
            Key::Escape => Self::Escape,
            Key::Space => Self::Space,
            Key::Tab => Self::Tab,
            Key::Delete => Self::Delete,
            Key::Backspace => Self::Backspace,
            Key::Insert => Self::Insert,
            Key::CapsLock => Self::CapsLock,
            Key::Home => Self::Home,
            Key::End => Self::End,
            Key::PageUp => Self::PageUp,
            Key::PageDown => Self::PageDown,
            Key::Up => Self::ArrowUp,
            Key::Down => Self::ArrowDown,
            Key::Left => Self::ArrowLeft,
            Key::Right => Self::ArrowRight,
            Key::Minus => Self::Minus,
            Key::Equal => Self::Equal,
            Key::LeftBracket => Self::BracketLeft,
            Key::RightBracket => Self::BracketRight,
            Key::Backslash => Self::Backslash,
            Key::Semicolon => Self::Semicolon,
            Key::Apostrophe => Self::Quote,
            Key::Grave => Self::Backquote,
            Key::Comma => Self::Comma,
            Key::Period => Self::Period,
            Key::Slash => Self::Slash,
            Key::Numpad0 => Self::Numpad0,
            Key::Numpad1 => Self::Numpad1,
            Key::Numpad2 => Self::Numpad2,
            Key::Numpad3 => Self::Numpad3,
            Key::Numpad4 => Self::Numpad4,
            Key::Numpad5 => Self::Numpad5,
            Key::Numpad6 => Self::Numpad6,
            Key::Numpad7 => Self::Numpad7,
            Key::Numpad8 => Self::Numpad8,
            Key::Numpad9 => Self::Numpad9,
            Key::NumpadDot => Self::NumpadDecimal,
            Key::NumpadPlus => Self::NumpadAdd,
            Key::NumpadMinus => Self::NumpadSubtract,
            Key::NumpadMultiply => Self::NumpadMultiply,
            Key::NumpadDivide => Self::NumpadDivide,
            Key::NumpadEnter => Self::NumpadEnter,
            Key::LeftCtrl => Self::ControlLeft,
            Key::RightCtrl => Self::ControlRight,
            Key::LeftShift => Self::ShiftLeft,
            Key::RightShift => Self::ShiftRight,
            Key::LeftAlt => Self::AltLeft,
            Key::RightAlt => Self::AltRight,
            Key::LeftSuper => Self::MetaLeft,
            Key::RightSuper => Self::MetaRight,
            Key::Unknown => Self::Unidentified,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn letter_keys_round_trip() {
        for (code, key) in [
            (Code::KeyA, Key::A),
            (Code::KeyB, Key::B),
            (Code::KeyC, Key::C),
            (Code::KeyM, Key::M),
            (Code::KeyZ, Key::Z),
        ] {
            assert_eq!(Key::from(code), key, "Code→Key failed for {code:?}");
            assert_eq!(Code::from(key), code, "Key→Code failed for {key:?}");
        }
    }

    #[test]
    fn digit_keys_round_trip() {
        for (code, key) in [
            (Code::Digit0, Key::Num0),
            (Code::Digit5, Key::Num5),
            (Code::Digit9, Key::Num9),
        ] {
            assert_eq!(Key::from(code), key, "Code→Key failed for {code:?}");
            assert_eq!(Code::from(key), code, "Key→Code failed for {key:?}");
        }
    }

    #[test]
    fn function_keys_round_trip() {
        for (code, key) in [
            (Code::F1, Key::F1),
            (Code::F12, Key::F12),
            (Code::F24, Key::F24),
        ] {
            assert_eq!(Key::from(code), key, "Code→Key failed for {code:?}");
            assert_eq!(Code::from(key), code, "Key→Code failed for {key:?}");
        }
    }

    #[test]
    fn navigation_keys_round_trip() {
        for (code, key) in [
            (Code::ArrowUp, Key::Up),
            (Code::ArrowDown, Key::Down),
            (Code::ArrowLeft, Key::Left),
            (Code::ArrowRight, Key::Right),
            (Code::Home, Key::Home),
            (Code::End, Key::End),
            (Code::PageUp, Key::PageUp),
            (Code::PageDown, Key::PageDown),
        ] {
            assert_eq!(Key::from(code), key, "Code→Key failed for {code:?}");
            assert_eq!(Code::from(key), code, "Key→Code failed for {key:?}");
        }
    }

    #[test]
    fn modifier_keys_round_trip() {
        for (code, key) in [
            (Code::ControlLeft, Key::LeftCtrl),
            (Code::ControlRight, Key::RightCtrl),
            (Code::ShiftLeft, Key::LeftShift),
            (Code::ShiftRight, Key::RightShift),
            (Code::AltLeft, Key::LeftAlt),
            (Code::AltRight, Key::RightAlt),
            (Code::MetaLeft, Key::LeftSuper),
            (Code::MetaRight, Key::RightSuper),
        ] {
            assert_eq!(Key::from(code), key, "Code→Key failed for {code:?}");
            assert_eq!(Code::from(key), code, "Key→Code failed for {key:?}");
        }
    }

    #[test]
    fn numpad_keys_round_trip() {
        for (code, key) in [
            (Code::Numpad0, Key::Numpad0),
            (Code::Numpad9, Key::Numpad9),
            (Code::NumpadAdd, Key::NumpadPlus),
            (Code::NumpadSubtract, Key::NumpadMinus),
            (Code::NumpadMultiply, Key::NumpadMultiply),
            (Code::NumpadDivide, Key::NumpadDivide),
            (Code::NumpadDecimal, Key::NumpadDot),
            (Code::NumpadEnter, Key::NumpadEnter),
        ] {
            assert_eq!(Key::from(code), key, "Code→Key failed for {code:?}");
            assert_eq!(Code::from(key), code, "Key→Code failed for {key:?}");
        }
    }

    #[test]
    fn punctuation_keys_round_trip() {
        for (code, key) in [
            (Code::Minus, Key::Minus),
            (Code::Equal, Key::Equal),
            (Code::BracketLeft, Key::LeftBracket),
            (Code::BracketRight, Key::RightBracket),
            (Code::Backslash, Key::Backslash),
            (Code::Semicolon, Key::Semicolon),
            (Code::Quote, Key::Apostrophe),
            (Code::Backquote, Key::Grave),
            (Code::Comma, Key::Comma),
            (Code::Period, Key::Period),
            (Code::Slash, Key::Slash),
        ] {
            assert_eq!(Key::from(code), key, "Code→Key failed for {code:?}");
            assert_eq!(Code::from(key), code, "Key→Code failed for {key:?}");
        }
    }

    #[test]
    fn special_keys_round_trip() {
        for (code, key) in [
            (Code::Enter, Key::Enter),
            (Code::Escape, Key::Escape),
            (Code::Space, Key::Space),
            (Code::Tab, Key::Tab),
            (Code::Delete, Key::Delete),
            (Code::Backspace, Key::Backspace),
            (Code::Insert, Key::Insert),
            (Code::CapsLock, Key::CapsLock),
        ] {
            assert_eq!(Key::from(code), key, "Code→Key failed for {code:?}");
            assert_eq!(Code::from(key), code, "Key→Code failed for {key:?}");
        }
    }

    #[test]
    fn unknown_code_maps_to_unknown_key() {
        // keyboard_types::Code has many keys without a kbd-core equivalent
        assert_eq!(Key::from(Code::PrintScreen), Key::Unknown);
        assert_eq!(Key::from(Code::ScrollLock), Key::Unknown);
        assert_eq!(Key::from(Code::Pause), Key::Unknown);
        assert_eq!(Key::from(Code::NumLock), Key::Unknown);
        assert_eq!(Key::from(Code::ContextMenu), Key::Unknown);
    }

    #[test]
    fn unknown_key_maps_to_unidentified_code() {
        assert_eq!(Code::from(Key::Unknown), Code::Unidentified);
    }
}
