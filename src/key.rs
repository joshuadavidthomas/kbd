//! Key types: [`Key`], [`Modifier`], [`Hotkey`], [`HotkeySequence`].
//!
//! Single source of truth for all key-related logic: the key enum, modifier
//! convenience type, hotkey combinations, string parsing (`FromStr`),
//! display formatting, and evdev conversions (`From`/`Into`).

use std::fmt;
use std::str::FromStr;

use evdev::KeyCode;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Key {
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
    Enter,
    Escape,
    Space,
    Tab,
    Delete,
    Backspace,
    Insert,
    CapsLock,
    Home,
    End,
    PageUp,
    PageDown,
    Up,
    Down,
    Left,
    Right,
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
    LeftCtrl,
    RightCtrl,
    LeftShift,
    RightShift,
    LeftAlt,
    RightAlt,
    LeftSuper,
    RightSuper,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Modifier {
    Ctrl,
    Shift,
    Alt,
    Super,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Hotkey {
    key: Key,
    modifiers: Vec<Modifier>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct HotkeySequence {
    steps: Vec<Hotkey>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseHotkeyError {
    Empty,
    EmptySegment,
    UnknownToken(String),
    MissingKey,
    MultipleKeys,
}

impl Key {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        key_name(self)
    }
}

impl Modifier {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Ctrl => "Ctrl",
            Self::Shift => "Shift",
            Self::Alt => "Alt",
            Self::Super => "Super",
        }
    }

    #[must_use]
    pub(crate) const fn from_key(key: Key) -> Option<Self> {
        match key {
            Key::LeftCtrl | Key::RightCtrl => Some(Self::Ctrl),
            Key::LeftShift | Key::RightShift => Some(Self::Shift),
            Key::LeftAlt | Key::RightAlt => Some(Self::Alt),
            Key::LeftSuper | Key::RightSuper => Some(Self::Super),
            _ => None,
        }
    }

    #[must_use]
    pub const fn keys(self) -> (Key, Key) {
        match self {
            Self::Ctrl => (Key::LeftCtrl, Key::RightCtrl),
            Self::Shift => (Key::LeftShift, Key::RightShift),
            Self::Alt => (Key::LeftAlt, Key::RightAlt),
            Self::Super => (Key::LeftSuper, Key::RightSuper),
        }
    }
}

impl Hotkey {
    #[must_use]
    pub fn new(key: Key, mut modifiers: Vec<Modifier>) -> Self {
        modifiers.sort();
        modifiers.dedup();
        Self { key, modifiers }
    }

    #[must_use]
    pub const fn key(&self) -> Key {
        self.key
    }

    #[must_use]
    pub fn modifiers(&self) -> &[Modifier] {
        &self.modifiers
    }
}

impl HotkeySequence {
    pub fn new(steps: Vec<Hotkey>) -> Result<Self, ParseHotkeyError> {
        if steps.is_empty() {
            return Err(ParseHotkeyError::Empty);
        }

        Ok(Self { steps })
    }

    #[must_use]
    pub fn steps(&self) -> &[Hotkey] {
        &self.steps
    }
}

impl From<Modifier> for Key {
    fn from(value: Modifier) -> Self {
        value.keys().0
    }
}

impl TryFrom<Key> for Modifier {
    type Error = Key;

    fn try_from(value: Key) -> Result<Self, Self::Error> {
        Self::from_key(value).ok_or(value)
    }
}

macro_rules! impl_display_via_as_str {
    ($($ty:ty),+ $(,)?) => {
        $(
            impl fmt::Display for $ty {
                fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                    f.write_str(self.as_str())
                }
            }
        )+
    };
}

impl_display_via_as_str!(Key, Modifier);

impl fmt::Display for Hotkey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for modifier in &self.modifiers {
            write!(f, "{modifier}+")?;
        }

        write!(f, "{}", self.key)
    }
}

impl fmt::Display for HotkeySequence {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (index, step) in self.steps.iter().enumerate() {
            if index > 0 {
                write!(f, ", ")?;
            }
            write!(f, "{step}")?;
        }

        Ok(())
    }
}

impl fmt::Display for ParseHotkeyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => write!(f, "hotkey string is empty"),
            Self::EmptySegment => write!(f, "hotkey contains an empty token"),
            Self::UnknownToken(token) => write!(f, "unknown hotkey token: {token}"),
            Self::MissingKey => write!(f, "hotkey is missing a non-modifier key"),
            Self::MultipleKeys => write!(f, "hotkey has multiple non-modifier keys"),
        }
    }
}

impl std::error::Error for ParseHotkeyError {}

impl FromStr for Modifier {
    type Err = ParseHotkeyError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let token = s.trim();
        let lower = token.to_ascii_lowercase();

        if let Some(modifier) = match lower.as_str() {
            "ctrl" | "control" => Some(Self::Ctrl),
            "shift" => Some(Self::Shift),
            "alt" => Some(Self::Alt),
            "super" | "meta" | "win" | "windows" => Some(Self::Super),
            _ => None,
        } {
            return Ok(modifier);
        }

        token
            .parse::<Key>()
            .ok()
            .and_then(Self::from_key)
            .ok_or_else(|| ParseHotkeyError::UnknownToken(token.to_string()))
    }
}

impl FromStr for Key {
    type Err = ParseHotkeyError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        parse_key_token(s).ok_or_else(|| ParseHotkeyError::UnknownToken(s.trim().to_string()))
    }
}

impl FromStr for Hotkey {
    type Err = ParseHotkeyError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let trimmed = s.trim();
        if trimmed.is_empty() {
            return Err(ParseHotkeyError::Empty);
        }

        let mut key = None;
        let mut modifiers = Vec::new();
        let mut last_modifier_key = None;

        for segment in trimmed.split('+') {
            let token = segment.trim();
            if token.is_empty() {
                return Err(ParseHotkeyError::EmptySegment);
            }

            let parsed_key = token
                .parse::<Key>()
                .map_err(|_| ParseHotkeyError::UnknownToken(token.to_string()))?;

            if let Some(modifier) = Modifier::from_key(parsed_key) {
                modifiers.push(modifier);
                last_modifier_key = Some(parsed_key);
                continue;
            }

            if key.replace(parsed_key).is_some() {
                return Err(ParseHotkeyError::MultipleKeys);
            }
        }

        let key = if let Some(key) = key {
            key
        } else {
            let key = last_modifier_key.ok_or(ParseHotkeyError::MissingKey)?;
            modifiers.pop();
            key
        };

        Ok(Self::new(key, modifiers))
    }
}

impl FromStr for HotkeySequence {
    type Err = ParseHotkeyError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut steps = Vec::new();
        for segment in s.split(',') {
            steps.push(segment.trim().parse::<Hotkey>()?);
        }

        Self::new(steps)
    }
}

impl From<KeyCode> for Key {
    #[allow(clippy::too_many_lines)]
    fn from(value: KeyCode) -> Self {
        match value {
            KeyCode::KEY_A => Self::A,
            KeyCode::KEY_B => Self::B,
            KeyCode::KEY_C => Self::C,
            KeyCode::KEY_D => Self::D,
            KeyCode::KEY_E => Self::E,
            KeyCode::KEY_F => Self::F,
            KeyCode::KEY_G => Self::G,
            KeyCode::KEY_H => Self::H,
            KeyCode::KEY_I => Self::I,
            KeyCode::KEY_J => Self::J,
            KeyCode::KEY_K => Self::K,
            KeyCode::KEY_L => Self::L,
            KeyCode::KEY_M => Self::M,
            KeyCode::KEY_N => Self::N,
            KeyCode::KEY_O => Self::O,
            KeyCode::KEY_P => Self::P,
            KeyCode::KEY_Q => Self::Q,
            KeyCode::KEY_R => Self::R,
            KeyCode::KEY_S => Self::S,
            KeyCode::KEY_T => Self::T,
            KeyCode::KEY_U => Self::U,
            KeyCode::KEY_V => Self::V,
            KeyCode::KEY_W => Self::W,
            KeyCode::KEY_X => Self::X,
            KeyCode::KEY_Y => Self::Y,
            KeyCode::KEY_Z => Self::Z,
            KeyCode::KEY_0 => Self::Num0,
            KeyCode::KEY_1 => Self::Num1,
            KeyCode::KEY_2 => Self::Num2,
            KeyCode::KEY_3 => Self::Num3,
            KeyCode::KEY_4 => Self::Num4,
            KeyCode::KEY_5 => Self::Num5,
            KeyCode::KEY_6 => Self::Num6,
            KeyCode::KEY_7 => Self::Num7,
            KeyCode::KEY_8 => Self::Num8,
            KeyCode::KEY_9 => Self::Num9,
            KeyCode::KEY_F1 => Self::F1,
            KeyCode::KEY_F2 => Self::F2,
            KeyCode::KEY_F3 => Self::F3,
            KeyCode::KEY_F4 => Self::F4,
            KeyCode::KEY_F5 => Self::F5,
            KeyCode::KEY_F6 => Self::F6,
            KeyCode::KEY_F7 => Self::F7,
            KeyCode::KEY_F8 => Self::F8,
            KeyCode::KEY_F9 => Self::F9,
            KeyCode::KEY_F10 => Self::F10,
            KeyCode::KEY_F11 => Self::F11,
            KeyCode::KEY_F12 => Self::F12,
            KeyCode::KEY_F13 => Self::F13,
            KeyCode::KEY_F14 => Self::F14,
            KeyCode::KEY_F15 => Self::F15,
            KeyCode::KEY_F16 => Self::F16,
            KeyCode::KEY_F17 => Self::F17,
            KeyCode::KEY_F18 => Self::F18,
            KeyCode::KEY_F19 => Self::F19,
            KeyCode::KEY_F20 => Self::F20,
            KeyCode::KEY_F21 => Self::F21,
            KeyCode::KEY_F22 => Self::F22,
            KeyCode::KEY_F23 => Self::F23,
            KeyCode::KEY_F24 => Self::F24,
            KeyCode::KEY_ENTER => Self::Enter,
            KeyCode::KEY_ESC => Self::Escape,
            KeyCode::KEY_SPACE => Self::Space,
            KeyCode::KEY_TAB => Self::Tab,
            KeyCode::KEY_DELETE => Self::Delete,
            KeyCode::KEY_BACKSPACE => Self::Backspace,
            KeyCode::KEY_INSERT => Self::Insert,
            KeyCode::KEY_CAPSLOCK => Self::CapsLock,
            KeyCode::KEY_HOME => Self::Home,
            KeyCode::KEY_END => Self::End,
            KeyCode::KEY_PAGEUP => Self::PageUp,
            KeyCode::KEY_PAGEDOWN => Self::PageDown,
            KeyCode::KEY_UP => Self::Up,
            KeyCode::KEY_DOWN => Self::Down,
            KeyCode::KEY_LEFT => Self::Left,
            KeyCode::KEY_RIGHT => Self::Right,
            KeyCode::KEY_MINUS => Self::Minus,
            KeyCode::KEY_EQUAL => Self::Equal,
            KeyCode::KEY_LEFTBRACE => Self::LeftBracket,
            KeyCode::KEY_RIGHTBRACE => Self::RightBracket,
            KeyCode::KEY_BACKSLASH => Self::Backslash,
            KeyCode::KEY_SEMICOLON => Self::Semicolon,
            KeyCode::KEY_APOSTROPHE => Self::Apostrophe,
            KeyCode::KEY_GRAVE => Self::Grave,
            KeyCode::KEY_COMMA => Self::Comma,
            KeyCode::KEY_DOT => Self::Period,
            KeyCode::KEY_SLASH => Self::Slash,
            KeyCode::KEY_KP0 => Self::Numpad0,
            KeyCode::KEY_KP1 => Self::Numpad1,
            KeyCode::KEY_KP2 => Self::Numpad2,
            KeyCode::KEY_KP3 => Self::Numpad3,
            KeyCode::KEY_KP4 => Self::Numpad4,
            KeyCode::KEY_KP5 => Self::Numpad5,
            KeyCode::KEY_KP6 => Self::Numpad6,
            KeyCode::KEY_KP7 => Self::Numpad7,
            KeyCode::KEY_KP8 => Self::Numpad8,
            KeyCode::KEY_KP9 => Self::Numpad9,
            KeyCode::KEY_KPDOT => Self::NumpadDot,
            KeyCode::KEY_KPPLUS => Self::NumpadPlus,
            KeyCode::KEY_KPMINUS => Self::NumpadMinus,
            KeyCode::KEY_KPASTERISK => Self::NumpadMultiply,
            KeyCode::KEY_KPSLASH => Self::NumpadDivide,
            KeyCode::KEY_KPENTER => Self::NumpadEnter,
            KeyCode::KEY_LEFTCTRL => Self::LeftCtrl,
            KeyCode::KEY_RIGHTCTRL => Self::RightCtrl,
            KeyCode::KEY_LEFTSHIFT => Self::LeftShift,
            KeyCode::KEY_RIGHTSHIFT => Self::RightShift,
            KeyCode::KEY_LEFTALT => Self::LeftAlt,
            KeyCode::KEY_RIGHTALT => Self::RightAlt,
            KeyCode::KEY_LEFTMETA => Self::LeftSuper,
            KeyCode::KEY_RIGHTMETA => Self::RightSuper,
            _ => Self::Unknown,
        }
    }
}

impl From<Key> for KeyCode {
    #[allow(clippy::too_many_lines)]
    fn from(value: Key) -> Self {
        match value {
            Key::A => Self::KEY_A,
            Key::B => Self::KEY_B,
            Key::C => Self::KEY_C,
            Key::D => Self::KEY_D,
            Key::E => Self::KEY_E,
            Key::F => Self::KEY_F,
            Key::G => Self::KEY_G,
            Key::H => Self::KEY_H,
            Key::I => Self::KEY_I,
            Key::J => Self::KEY_J,
            Key::K => Self::KEY_K,
            Key::L => Self::KEY_L,
            Key::M => Self::KEY_M,
            Key::N => Self::KEY_N,
            Key::O => Self::KEY_O,
            Key::P => Self::KEY_P,
            Key::Q => Self::KEY_Q,
            Key::R => Self::KEY_R,
            Key::S => Self::KEY_S,
            Key::T => Self::KEY_T,
            Key::U => Self::KEY_U,
            Key::V => Self::KEY_V,
            Key::W => Self::KEY_W,
            Key::X => Self::KEY_X,
            Key::Y => Self::KEY_Y,
            Key::Z => Self::KEY_Z,
            Key::Num0 => Self::KEY_0,
            Key::Num1 => Self::KEY_1,
            Key::Num2 => Self::KEY_2,
            Key::Num3 => Self::KEY_3,
            Key::Num4 => Self::KEY_4,
            Key::Num5 => Self::KEY_5,
            Key::Num6 => Self::KEY_6,
            Key::Num7 => Self::KEY_7,
            Key::Num8 => Self::KEY_8,
            Key::Num9 => Self::KEY_9,
            Key::F1 => Self::KEY_F1,
            Key::F2 => Self::KEY_F2,
            Key::F3 => Self::KEY_F3,
            Key::F4 => Self::KEY_F4,
            Key::F5 => Self::KEY_F5,
            Key::F6 => Self::KEY_F6,
            Key::F7 => Self::KEY_F7,
            Key::F8 => Self::KEY_F8,
            Key::F9 => Self::KEY_F9,
            Key::F10 => Self::KEY_F10,
            Key::F11 => Self::KEY_F11,
            Key::F12 => Self::KEY_F12,
            Key::F13 => Self::KEY_F13,
            Key::F14 => Self::KEY_F14,
            Key::F15 => Self::KEY_F15,
            Key::F16 => Self::KEY_F16,
            Key::F17 => Self::KEY_F17,
            Key::F18 => Self::KEY_F18,
            Key::F19 => Self::KEY_F19,
            Key::F20 => Self::KEY_F20,
            Key::F21 => Self::KEY_F21,
            Key::F22 => Self::KEY_F22,
            Key::F23 => Self::KEY_F23,
            Key::F24 => Self::KEY_F24,
            Key::Enter => Self::KEY_ENTER,
            Key::Escape => Self::KEY_ESC,
            Key::Space => Self::KEY_SPACE,
            Key::Tab => Self::KEY_TAB,
            Key::Delete => Self::KEY_DELETE,
            Key::Backspace => Self::KEY_BACKSPACE,
            Key::Insert => Self::KEY_INSERT,
            Key::CapsLock => Self::KEY_CAPSLOCK,
            Key::Home => Self::KEY_HOME,
            Key::End => Self::KEY_END,
            Key::PageUp => Self::KEY_PAGEUP,
            Key::PageDown => Self::KEY_PAGEDOWN,
            Key::Up => Self::KEY_UP,
            Key::Down => Self::KEY_DOWN,
            Key::Left => Self::KEY_LEFT,
            Key::Right => Self::KEY_RIGHT,
            Key::Minus => Self::KEY_MINUS,
            Key::Equal => Self::KEY_EQUAL,
            Key::LeftBracket => Self::KEY_LEFTBRACE,
            Key::RightBracket => Self::KEY_RIGHTBRACE,
            Key::Backslash => Self::KEY_BACKSLASH,
            Key::Semicolon => Self::KEY_SEMICOLON,
            Key::Apostrophe => Self::KEY_APOSTROPHE,
            Key::Grave => Self::KEY_GRAVE,
            Key::Comma => Self::KEY_COMMA,
            Key::Period => Self::KEY_DOT,
            Key::Slash => Self::KEY_SLASH,
            Key::Numpad0 => Self::KEY_KP0,
            Key::Numpad1 => Self::KEY_KP1,
            Key::Numpad2 => Self::KEY_KP2,
            Key::Numpad3 => Self::KEY_KP3,
            Key::Numpad4 => Self::KEY_KP4,
            Key::Numpad5 => Self::KEY_KP5,
            Key::Numpad6 => Self::KEY_KP6,
            Key::Numpad7 => Self::KEY_KP7,
            Key::Numpad8 => Self::KEY_KP8,
            Key::Numpad9 => Self::KEY_KP9,
            Key::NumpadDot => Self::KEY_KPDOT,
            Key::NumpadPlus => Self::KEY_KPPLUS,
            Key::NumpadMinus => Self::KEY_KPMINUS,
            Key::NumpadMultiply => Self::KEY_KPASTERISK,
            Key::NumpadDivide => Self::KEY_KPSLASH,
            Key::NumpadEnter => Self::KEY_KPENTER,
            Key::LeftCtrl => Self::KEY_LEFTCTRL,
            Key::RightCtrl => Self::KEY_RIGHTCTRL,
            Key::LeftShift => Self::KEY_LEFTSHIFT,
            Key::RightShift => Self::KEY_RIGHTSHIFT,
            Key::LeftAlt => Self::KEY_LEFTALT,
            Key::RightAlt => Self::KEY_RIGHTALT,
            Key::LeftSuper => Self::KEY_LEFTMETA,
            Key::RightSuper => Self::KEY_RIGHTMETA,
            Key::Unknown => Self::KEY_UNKNOWN,
        }
    }
}

#[allow(clippy::too_many_lines)]
const fn key_name(key: Key) -> &'static str {
    match key {
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
        Key::Escape => "Escape",
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
        Key::Period => "Period",
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
        Key::NumpadMultiply => "NumpadMultiply",
        Key::NumpadDivide => "NumpadDivide",
        Key::NumpadEnter => "NumpadEnter",
        Key::LeftCtrl => "LeftCtrl",
        Key::RightCtrl => "RightCtrl",
        Key::LeftShift => "LeftShift",
        Key::RightShift => "RightShift",
        Key::LeftAlt => "LeftAlt",
        Key::RightAlt => "RightAlt",
        Key::LeftSuper => "LeftSuper",
        Key::RightSuper => "RightSuper",
        Key::Unknown => "Unknown",
    }
}

#[allow(clippy::too_many_lines)]
fn parse_key_token(token: &str) -> Option<Key> {
    let trimmed = token.trim();
    if trimmed.is_empty() {
        return None;
    }

    if trimmed.len() == 1 {
        let ch = trimmed.chars().next()?.to_ascii_uppercase();
        return match ch {
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
            '0' => Some(Key::Num0),
            '1' => Some(Key::Num1),
            '2' => Some(Key::Num2),
            '3' => Some(Key::Num3),
            '4' => Some(Key::Num4),
            '5' => Some(Key::Num5),
            '6' => Some(Key::Num6),
            '7' => Some(Key::Num7),
            '8' => Some(Key::Num8),
            '9' => Some(Key::Num9),
            '-' => Some(Key::Minus),
            '=' => Some(Key::Equal),
            '[' => Some(Key::LeftBracket),
            ']' => Some(Key::RightBracket),
            '\\' => Some(Key::Backslash),
            ';' => Some(Key::Semicolon),
            '\'' => Some(Key::Apostrophe),
            '`' => Some(Key::Grave),
            ',' => Some(Key::Comma),
            '.' => Some(Key::Period),
            '/' => Some(Key::Slash),
            _ => None,
        };
    }

    let upper = trimmed.to_ascii_uppercase();

    if let Some(function_number) = upper.strip_prefix('F') {
        if let Ok(number) = function_number.parse::<u8>() {
            return match number {
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
                _ => None,
            };
        }
    }

    match upper.as_str() {
        "RETURN" | "ENTER" => Some(Key::Enter),
        "ESC" | "ESCAPE" => Some(Key::Escape),
        "SPACE" => Some(Key::Space),
        "TAB" => Some(Key::Tab),
        "DEL" | "DELETE" => Some(Key::Delete),
        "BS" | "BACKSPACE" => Some(Key::Backspace),
        "INS" | "INSERT" => Some(Key::Insert),
        "CAPSLOCK" => Some(Key::CapsLock),
        "HOME" => Some(Key::Home),
        "END" => Some(Key::End),
        "PAGEUP" | "PGUP" => Some(Key::PageUp),
        "PAGEDOWN" | "PGDN" => Some(Key::PageDown),
        "UP" => Some(Key::Up),
        "DOWN" => Some(Key::Down),
        "LEFT" => Some(Key::Left),
        "RIGHT" => Some(Key::Right),
        "MINUS" | "DASH" => Some(Key::Minus),
        "EQUAL" | "PLUS" => Some(Key::Equal),
        "LEFTBRACKET" | "LBRACKET" => Some(Key::LeftBracket),
        "RIGHTBRACKET" | "RBRACKET" => Some(Key::RightBracket),
        "BACKSLASH" | "PIPE" => Some(Key::Backslash),
        "SEMICOLON" => Some(Key::Semicolon),
        "APOSTROPHE" | "QUOTE" => Some(Key::Apostrophe),
        "GRAVE" | "BACKTICK" => Some(Key::Grave),
        "COMMA" => Some(Key::Comma),
        "PERIOD" | "DOT" => Some(Key::Period),
        "SLASH" => Some(Key::Slash),
        "NUMPAD0" | "KP0" => Some(Key::Numpad0),
        "NUMPAD1" | "KP1" => Some(Key::Numpad1),
        "NUMPAD2" | "KP2" => Some(Key::Numpad2),
        "NUMPAD3" | "KP3" => Some(Key::Numpad3),
        "NUMPAD4" | "KP4" => Some(Key::Numpad4),
        "NUMPAD5" | "KP5" => Some(Key::Numpad5),
        "NUMPAD6" | "KP6" => Some(Key::Numpad6),
        "NUMPAD7" | "KP7" => Some(Key::Numpad7),
        "NUMPAD8" | "KP8" => Some(Key::Numpad8),
        "NUMPAD9" | "KP9" => Some(Key::Numpad9),
        "NUMPADDOT" | "KPDOT" => Some(Key::NumpadDot),
        "NUMPADPLUS" | "KPPLUS" => Some(Key::NumpadPlus),
        "NUMPADMINUS" | "KPMINUS" => Some(Key::NumpadMinus),
        "NUMPADMULTIPLY" | "NUMPADASTERISK" | "KPASTERISK" => Some(Key::NumpadMultiply),
        "NUMPADDIVIDE" | "NUMPADSLASH" | "KPSLASH" => Some(Key::NumpadDivide),
        "NUMPADENTER" | "KPENTER" => Some(Key::NumpadEnter),
        "CTRL" | "CONTROL" | "LEFTCTRL" | "LCTRL" => Some(Key::LeftCtrl),
        "RIGHTCTRL" | "RCTRL" => Some(Key::RightCtrl),
        "SHIFT" | "LEFTSHIFT" | "LSHIFT" => Some(Key::LeftShift),
        "RIGHTSHIFT" | "RSHIFT" => Some(Key::RightShift),
        "ALT" | "LEFTALT" | "LALT" => Some(Key::LeftAlt),
        "RIGHTALT" | "RALT" => Some(Key::RightAlt),
        "SUPER" | "META" | "WIN" | "WINDOWS" | "LEFTSUPER" | "LSUPER" | "LEFTMETA" | "LMETA" => {
            Some(Key::LeftSuper)
        }
        "RIGHTSUPER" | "RSUPER" | "RIGHTMETA" | "RMETA" => Some(Key::RightSuper),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_aliases_case_insensitive() {
        let hotkey = "ctrl+Win+return".parse::<Hotkey>().unwrap();
        assert_eq!(hotkey.key(), Key::Enter);
        assert_eq!(hotkey.modifiers(), &[Modifier::Ctrl, Modifier::Super]);
    }

    #[test]
    fn hotkey_new_sorts_and_dedups_modifiers() {
        let hotkey = Hotkey::new(Key::A, vec![Modifier::Alt, Modifier::Ctrl, Modifier::Alt]);
        assert_eq!(hotkey.modifiers(), &[Modifier::Ctrl, Modifier::Alt]);
    }

    #[test]
    fn modifier_canonicalizes_left_and_right_keys() {
        assert_eq!(Modifier::from_key(Key::LeftCtrl), Some(Modifier::Ctrl));
        assert_eq!(Modifier::from_key(Key::RightCtrl), Some(Modifier::Ctrl));
        assert_eq!(Modifier::from_key(Key::LeftShift), Some(Modifier::Shift));
        assert_eq!(Modifier::from_key(Key::RightShift), Some(Modifier::Shift));
        assert_eq!(Modifier::from_key(Key::A), None);
    }

    #[test]
    fn keycode_round_trip_preserves_supported_keys() {
        for key in [
            Key::A,
            Key::F24,
            Key::Enter,
            Key::CapsLock,
            Key::NumpadEnter,
            Key::LeftCtrl,
            Key::RightSuper,
        ] {
            let code: KeyCode = key.into();
            let parsed = Key::from(code);
            assert_eq!(parsed, key);
        }
    }

    #[test]
    fn unknown_keycode_maps_to_unknown() {
        let key = Key::from(KeyCode::KEY_VOLUMEUP);
        assert_eq!(key, Key::Unknown);
    }

    #[test]
    fn sequence_display_round_trips() {
        let sequence = "Ctrl+K, Ctrl+C".parse::<HotkeySequence>().unwrap();
        let round_trip = sequence.to_string().parse::<HotkeySequence>().unwrap();
        assert_eq!(round_trip, sequence);
    }
}
