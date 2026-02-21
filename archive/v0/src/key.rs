use std::fmt;

// SMELL: Key and Modifier are roughly the same, just different buckets
// surely a Trait or a higher level enum type or something could help here?

/// A keyboard key, independent of any specific input backend.
///
/// This enum covers the standard set of keyboard keys. It intentionally
/// does not include modifier keys — use [`Modifier`] for those.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Key {
    // Letters
    A,
    B,
    C,
    D,
    E,
    F,
    G,
    H,
    I,
    J,
    K,
    L,
    M,
    N,
    O,
    P,
    Q,
    R,
    S,
    T,
    U,
    V,
    W,
    X,
    Y,
    Z,

    // Top-row numbers
    Num0,
    Num1,
    Num2,
    Num3,
    Num4,
    Num5,
    Num6,
    Num7,
    Num8,
    Num9,

    // Function keys
    F1,
    F2,
    F3,
    F4,
    F5,
    F6,
    F7,
    F8,
    F9,
    F10,
    F11,
    F12,
    F13,
    F14,
    F15,
    F16,
    F17,
    F18,
    F19,
    F20,
    F21,
    F22,
    F23,
    F24,

    // Special keys
    Enter,
    Escape,
    Space,
    Tab,
    Delete,
    Backspace,
    Insert,
    CapsLock,

    // Navigation
    Home,
    End,
    PageUp,
    PageDown,
    Up,
    Down,
    Left,
    Right,

    // Punctuation / symbols
    Minus,
    Equal,
    LeftBracket,
    RightBracket,
    Backslash,
    Semicolon,
    Apostrophe,
    Grave,
    Comma,
    Period,
    Slash,

    // Numpad
    Numpad0,
    Numpad1,
    Numpad2,
    Numpad3,
    Numpad4,
    Numpad5,
    Numpad6,
    Numpad7,
    Numpad8,
    Numpad9,
    NumpadDot,
    NumpadPlus,
    NumpadMinus,
    NumpadMultiply,
    NumpadDivide,
    NumpadEnter,
}

/// A modifier key, independent of left/right physical position.
///
/// All modifiers are logical — `Ctrl` matches both the left and right
/// physical Ctrl keys. Left/right distinction is handled internally
/// by each backend.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Modifier {
    Ctrl,
    Shift,
    Alt,
    Super,
}

// SMELL: same thing duplicated? Trait? macro?
impl fmt::Display for Key {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl fmt::Display for Modifier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl Key {
    /// Human-readable name for display and serialization.
    #[must_use]
    #[allow(clippy::too_many_lines)]
    pub fn as_str(self) -> &'static str {
        match self {
            Key::A => "A",
            Key::B => "B",
            Key::C => "C",
            Key::D => "D",
            Key::E => "E",
            Key::F => "F",
            Key::G => "G",
            Key::H => "H",
            Key::I => "I",
            Key::J => "J",
            Key::K => "K",
            Key::L => "L",
            Key::M => "M",
            Key::N => "N",
            Key::O => "O",
            Key::P => "P",
            Key::Q => "Q",
            Key::R => "R",
            Key::S => "S",
            Key::T => "T",
            Key::U => "U",
            Key::V => "V",
            Key::W => "W",
            Key::X => "X",
            Key::Y => "Y",
            Key::Z => "Z",
            Key::Num0 => "0",
            Key::Num1 => "1",
            Key::Num2 => "2",
            Key::Num3 => "3",
            Key::Num4 => "4",
            Key::Num5 => "5",
            Key::Num6 => "6",
            Key::Num7 => "7",
            Key::Num8 => "8",
            Key::Num9 => "9",
            Key::F1 => "F1",
            Key::F2 => "F2",
            Key::F3 => "F3",
            Key::F4 => "F4",
            Key::F5 => "F5",
            Key::F6 => "F6",
            Key::F7 => "F7",
            Key::F8 => "F8",
            Key::F9 => "F9",
            Key::F10 => "F10",
            Key::F11 => "F11",
            Key::F12 => "F12",
            Key::F13 => "F13",
            Key::F14 => "F14",
            Key::F15 => "F15",
            Key::F16 => "F16",
            Key::F17 => "F17",
            Key::F18 => "F18",
            Key::F19 => "F19",
            Key::F20 => "F20",
            Key::F21 => "F21",
            Key::F22 => "F22",
            Key::F23 => "F23",
            Key::F24 => "F24",
            Key::Enter => "Enter",
            Key::Escape => "Esc",
            Key::Space => "Space",
            Key::Tab => "Tab",
            Key::Delete => "Delete",
            Key::Backspace => "Backspace",
            Key::Insert => "Insert",
            Key::CapsLock => "CapsLock",
            Key::Home => "Home",
            Key::End => "End",
            Key::PageUp => "PageUp",
            Key::PageDown => "PageDown",
            Key::Up => "Up",
            Key::Down => "Down",
            Key::Left => "Left",
            Key::Right => "Right",
            Key::Minus => "Minus",
            Key::Equal => "Equal",
            Key::LeftBracket => "LeftBracket",
            Key::RightBracket => "RightBracket",
            Key::Backslash => "Backslash",
            Key::Semicolon => "Semicolon",
            Key::Apostrophe => "Apostrophe",
            Key::Grave => "Grave",
            Key::Comma => "Comma",
            Key::Period => "Dot",
            Key::Slash => "Slash",
            Key::Numpad0 => "Numpad0",
            Key::Numpad1 => "Numpad1",
            Key::Numpad2 => "Numpad2",
            Key::Numpad3 => "Numpad3",
            Key::Numpad4 => "Numpad4",
            Key::Numpad5 => "Numpad5",
            Key::Numpad6 => "Numpad6",
            Key::Numpad7 => "Numpad7",
            Key::Numpad8 => "Numpad8",
            Key::Numpad9 => "Numpad9",
            Key::NumpadDot => "NumpadDot",
            Key::NumpadPlus => "NumpadPlus",
            Key::NumpadMinus => "NumpadMinus",
            Key::NumpadMultiply => "NumpadAsterisk",
            Key::NumpadDivide => "NumpadSlash",
            Key::NumpadEnter => "NumpadEnter",
        }
    }

    /// Convert from an evdev key code. Returns `None` for unrecognized codes
    /// or modifier keys (use [`Modifier::from_evdev`] for those).
    // SMELL: why not a impl from?
    #[must_use]
    #[allow(clippy::too_many_lines)]
    pub(crate) fn from_evdev(code: evdev::KeyCode) -> Option<Self> {
        use evdev::KeyCode;
        match code {
            KeyCode::KEY_A => Some(Key::A),
            KeyCode::KEY_B => Some(Key::B),
            KeyCode::KEY_C => Some(Key::C),
            KeyCode::KEY_D => Some(Key::D),
            KeyCode::KEY_E => Some(Key::E),
            KeyCode::KEY_F => Some(Key::F),
            KeyCode::KEY_G => Some(Key::G),
            KeyCode::KEY_H => Some(Key::H),
            KeyCode::KEY_I => Some(Key::I),
            KeyCode::KEY_J => Some(Key::J),
            KeyCode::KEY_K => Some(Key::K),
            KeyCode::KEY_L => Some(Key::L),
            KeyCode::KEY_M => Some(Key::M),
            KeyCode::KEY_N => Some(Key::N),
            KeyCode::KEY_O => Some(Key::O),
            KeyCode::KEY_P => Some(Key::P),
            KeyCode::KEY_Q => Some(Key::Q),
            KeyCode::KEY_R => Some(Key::R),
            KeyCode::KEY_S => Some(Key::S),
            KeyCode::KEY_T => Some(Key::T),
            KeyCode::KEY_U => Some(Key::U),
            KeyCode::KEY_V => Some(Key::V),
            KeyCode::KEY_W => Some(Key::W),
            KeyCode::KEY_X => Some(Key::X),
            KeyCode::KEY_Y => Some(Key::Y),
            KeyCode::KEY_Z => Some(Key::Z),
            KeyCode::KEY_0 => Some(Key::Num0),
            KeyCode::KEY_1 => Some(Key::Num1),
            KeyCode::KEY_2 => Some(Key::Num2),
            KeyCode::KEY_3 => Some(Key::Num3),
            KeyCode::KEY_4 => Some(Key::Num4),
            KeyCode::KEY_5 => Some(Key::Num5),
            KeyCode::KEY_6 => Some(Key::Num6),
            KeyCode::KEY_7 => Some(Key::Num7),
            KeyCode::KEY_8 => Some(Key::Num8),
            KeyCode::KEY_9 => Some(Key::Num9),
            KeyCode::KEY_F1 => Some(Key::F1),
            KeyCode::KEY_F2 => Some(Key::F2),
            KeyCode::KEY_F3 => Some(Key::F3),
            KeyCode::KEY_F4 => Some(Key::F4),
            KeyCode::KEY_F5 => Some(Key::F5),
            KeyCode::KEY_F6 => Some(Key::F6),
            KeyCode::KEY_F7 => Some(Key::F7),
            KeyCode::KEY_F8 => Some(Key::F8),
            KeyCode::KEY_F9 => Some(Key::F9),
            KeyCode::KEY_F10 => Some(Key::F10),
            KeyCode::KEY_F11 => Some(Key::F11),
            KeyCode::KEY_F12 => Some(Key::F12),
            KeyCode::KEY_F13 => Some(Key::F13),
            KeyCode::KEY_F14 => Some(Key::F14),
            KeyCode::KEY_F15 => Some(Key::F15),
            KeyCode::KEY_F16 => Some(Key::F16),
            KeyCode::KEY_F17 => Some(Key::F17),
            KeyCode::KEY_F18 => Some(Key::F18),
            KeyCode::KEY_F19 => Some(Key::F19),
            KeyCode::KEY_F20 => Some(Key::F20),
            KeyCode::KEY_F21 => Some(Key::F21),
            KeyCode::KEY_F22 => Some(Key::F22),
            KeyCode::KEY_F23 => Some(Key::F23),
            KeyCode::KEY_F24 => Some(Key::F24),
            KeyCode::KEY_ENTER => Some(Key::Enter),
            KeyCode::KEY_ESC => Some(Key::Escape),
            KeyCode::KEY_SPACE => Some(Key::Space),
            KeyCode::KEY_TAB => Some(Key::Tab),
            KeyCode::KEY_DELETE => Some(Key::Delete),
            KeyCode::KEY_BACKSPACE => Some(Key::Backspace),
            KeyCode::KEY_INSERT => Some(Key::Insert),
            KeyCode::KEY_CAPSLOCK => Some(Key::CapsLock),
            KeyCode::KEY_HOME => Some(Key::Home),
            KeyCode::KEY_END => Some(Key::End),
            KeyCode::KEY_PAGEUP => Some(Key::PageUp),
            KeyCode::KEY_PAGEDOWN => Some(Key::PageDown),
            KeyCode::KEY_UP => Some(Key::Up),
            KeyCode::KEY_DOWN => Some(Key::Down),
            KeyCode::KEY_LEFT => Some(Key::Left),
            KeyCode::KEY_RIGHT => Some(Key::Right),
            KeyCode::KEY_MINUS => Some(Key::Minus),
            KeyCode::KEY_EQUAL => Some(Key::Equal),
            KeyCode::KEY_LEFTBRACE => Some(Key::LeftBracket),
            KeyCode::KEY_RIGHTBRACE => Some(Key::RightBracket),
            KeyCode::KEY_BACKSLASH => Some(Key::Backslash),
            KeyCode::KEY_SEMICOLON => Some(Key::Semicolon),
            KeyCode::KEY_APOSTROPHE => Some(Key::Apostrophe),
            KeyCode::KEY_GRAVE => Some(Key::Grave),
            KeyCode::KEY_COMMA => Some(Key::Comma),
            KeyCode::KEY_DOT => Some(Key::Period),
            KeyCode::KEY_SLASH => Some(Key::Slash),
            KeyCode::KEY_KP0 => Some(Key::Numpad0),
            KeyCode::KEY_KP1 => Some(Key::Numpad1),
            KeyCode::KEY_KP2 => Some(Key::Numpad2),
            KeyCode::KEY_KP3 => Some(Key::Numpad3),
            KeyCode::KEY_KP4 => Some(Key::Numpad4),
            KeyCode::KEY_KP5 => Some(Key::Numpad5),
            KeyCode::KEY_KP6 => Some(Key::Numpad6),
            KeyCode::KEY_KP7 => Some(Key::Numpad7),
            KeyCode::KEY_KP8 => Some(Key::Numpad8),
            KeyCode::KEY_KP9 => Some(Key::Numpad9),
            KeyCode::KEY_KPDOT => Some(Key::NumpadDot),
            KeyCode::KEY_KPPLUS => Some(Key::NumpadPlus),
            KeyCode::KEY_KPMINUS => Some(Key::NumpadMinus),
            KeyCode::KEY_KPASTERISK => Some(Key::NumpadMultiply),
            KeyCode::KEY_KPSLASH => Some(Key::NumpadDivide),
            KeyCode::KEY_KPENTER => Some(Key::NumpadEnter),
            _ => None,
        }
    }

    /// Convert to the corresponding evdev key code.
    // SMELL: why not a impl to?
    #[must_use]
    #[allow(clippy::too_many_lines)]
    pub(crate) fn to_evdev(self) -> evdev::KeyCode {
        use evdev::KeyCode;
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
        }
    }
}

impl Modifier {
    /// Human-readable name for display and serialization.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Modifier::Ctrl => "Ctrl",
            Modifier::Shift => "Shift",
            Modifier::Alt => "Alt",
            Modifier::Super => "Super",
        }
    }

    /// Convert from an evdev key code. Recognizes both left and right variants.
    // SMELL: why not a impl from?
    #[must_use]
    pub(crate) fn from_evdev(code: evdev::KeyCode) -> Option<Self> {
        use evdev::KeyCode;
        match code {
            KeyCode::KEY_LEFTCTRL | KeyCode::KEY_RIGHTCTRL => Some(Modifier::Ctrl),
            KeyCode::KEY_LEFTSHIFT | KeyCode::KEY_RIGHTSHIFT => Some(Modifier::Shift),
            KeyCode::KEY_LEFTALT | KeyCode::KEY_RIGHTALT => Some(Modifier::Alt),
            KeyCode::KEY_LEFTMETA | KeyCode::KEY_RIGHTMETA => Some(Modifier::Super),
            _ => None,
        }
    }

    /// Convert to the canonical (left-side) evdev key code.
    // SMELL: why not a impl from?
    #[must_use]
    pub(crate) fn to_evdev(self) -> evdev::KeyCode {
        use evdev::KeyCode;
        match self {
            Modifier::Ctrl => KeyCode::KEY_LEFTCTRL,
            Modifier::Shift => KeyCode::KEY_LEFTSHIFT,
            Modifier::Alt => KeyCode::KEY_LEFTALT,
            Modifier::Super => KeyCode::KEY_LEFTMETA,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_evdev_round_trip() {
        let keys = [
            Key::A,
            Key::Z,
            Key::Num0,
            Key::F1,
            Key::F24,
            Key::Enter,
            Key::CapsLock,
            Key::Numpad0,
            Key::NumpadEnter,
            Key::Period,
        ];

        for key in keys {
            let evdev_code = key.to_evdev();
            let round_tripped = Key::from_evdev(evdev_code);
            assert_eq!(round_tripped, Some(key), "round-trip failed for {key:?}");
        }
    }

    #[test]
    fn modifier_evdev_round_trip() {
        for modifier in [
            Modifier::Ctrl,
            Modifier::Shift,
            Modifier::Alt,
            Modifier::Super,
        ] {
            let evdev_code = modifier.to_evdev();
            let round_tripped = Modifier::from_evdev(evdev_code);
            assert_eq!(
                round_tripped,
                Some(modifier),
                "round-trip failed for {modifier:?}"
            );
        }
    }

    #[test]
    fn modifier_from_evdev_maps_both_sides() {
        use evdev::KeyCode;
        assert_eq!(
            Modifier::from_evdev(KeyCode::KEY_LEFTCTRL),
            Some(Modifier::Ctrl)
        );
        assert_eq!(
            Modifier::from_evdev(KeyCode::KEY_RIGHTCTRL),
            Some(Modifier::Ctrl)
        );
        assert_eq!(
            Modifier::from_evdev(KeyCode::KEY_LEFTSHIFT),
            Some(Modifier::Shift)
        );
        assert_eq!(
            Modifier::from_evdev(KeyCode::KEY_RIGHTSHIFT),
            Some(Modifier::Shift)
        );
        assert_eq!(
            Modifier::from_evdev(KeyCode::KEY_LEFTALT),
            Some(Modifier::Alt)
        );
        assert_eq!(
            Modifier::from_evdev(KeyCode::KEY_RIGHTALT),
            Some(Modifier::Alt)
        );
        assert_eq!(
            Modifier::from_evdev(KeyCode::KEY_LEFTMETA),
            Some(Modifier::Super)
        );
        assert_eq!(
            Modifier::from_evdev(KeyCode::KEY_RIGHTMETA),
            Some(Modifier::Super)
        );
    }

    #[test]
    fn non_modifier_key_returns_none() {
        assert!(Modifier::from_evdev(evdev::KeyCode::KEY_A).is_none());
    }

    #[test]
    fn modifier_key_returns_none_from_key() {
        assert!(Key::from_evdev(evdev::KeyCode::KEY_LEFTCTRL).is_none());
    }

    #[test]
    fn unknown_evdev_key_returns_none() {
        assert!(Key::from_evdev(evdev::KeyCode::KEY_VOLUMEUP).is_none());
    }
}
