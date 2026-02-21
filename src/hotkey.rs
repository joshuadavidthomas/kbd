use std::fmt;
use std::str::FromStr;

use evdev::KeyCode;
#[cfg(feature = "serde")]
use serde::de::Error as _;
#[cfg(feature = "serde")]
use serde::Deserialize;
#[cfg(feature = "serde")]
use serde::Deserializer;
#[cfg(feature = "serde")]
use serde::Serialize;
#[cfg(feature = "serde")]
use serde::Serializer;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hotkey {
    key: KeyCode,
    modifiers: Vec<KeyCode>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
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

impl Hotkey {
    pub fn new(key: KeyCode, mut modifiers: Vec<KeyCode>) -> Self {
        modifiers = modifiers
            .into_iter()
            .map(canonical_modifier)
            .collect::<Vec<_>>();
        modifiers.sort();
        modifiers.dedup();
        Self { key, modifiers }
    }

    #[must_use]
    pub fn key(&self) -> KeyCode {
        self.key
    }

    #[must_use]
    pub fn modifiers(&self) -> &[KeyCode] {
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

impl FromStr for Hotkey {
    type Err = ParseHotkeyError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let trimmed = s.trim();
        if trimmed.is_empty() {
            return Err(ParseHotkeyError::Empty);
        }

        let mut key = None;
        let mut modifiers = Vec::new();

        for raw_part in trimmed.split('+') {
            let part = raw_part.trim();
            if part.is_empty() {
                return Err(ParseHotkeyError::EmptySegment);
            }

            if let Some(modifier) = parse_modifier(part) {
                modifiers.push(modifier);
                continue;
            }

            let parsed_key =
                parse_key(part).ok_or_else(|| ParseHotkeyError::UnknownToken(part.to_string()))?;
            if key.replace(parsed_key).is_some() {
                return Err(ParseHotkeyError::MultipleKeys);
            }
        }

        let key = key.ok_or(ParseHotkeyError::MissingKey)?;
        Ok(Hotkey::new(key, modifiers))
    }
}

impl FromStr for HotkeySequence {
    type Err = ParseHotkeyError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut steps = Vec::new();
        for step in s.split(',') {
            let parsed = step.trim().parse::<Hotkey>()?;
            steps.push(parsed);
        }

        HotkeySequence::new(steps)
    }
}

#[cfg(feature = "serde")]
impl Serialize for Hotkey {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

#[cfg(feature = "serde")]
impl<'de> Deserialize<'de> for Hotkey {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        value.parse::<Hotkey>().map_err(|err| {
            D::Error::custom(format!(
                "invalid hotkey \"{value}\": {err}. Expected format like Ctrl+Shift+A"
            ))
        })
    }
}

#[cfg(feature = "serde")]
impl Serialize for HotkeySequence {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

#[cfg(feature = "serde")]
impl<'de> Deserialize<'de> for HotkeySequence {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        value.parse::<HotkeySequence>().map_err(|err| {
            D::Error::custom(format!(
                "invalid hotkey sequence \"{value}\": {err}. Expected format like Ctrl+K, Ctrl+C"
            ))
        })
    }
}

impl fmt::Display for Hotkey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut parts: Vec<&str> = self
            .modifiers
            .iter()
            .copied()
            .map(display_modifier)
            .collect();
        parts.push(display_key(self.key));
        write!(f, "{}", parts.join("+"))
    }
}

impl fmt::Display for HotkeySequence {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let rendered: Vec<String> = self.steps.iter().map(ToString::to_string).collect();
        write!(f, "{}", rendered.join(", "))
    }
}

impl fmt::Display for ParseHotkeyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParseHotkeyError::Empty => write!(f, "hotkey string is empty"),
            ParseHotkeyError::EmptySegment => write!(f, "hotkey contains an empty token"),
            ParseHotkeyError::UnknownToken(token) => write!(f, "unknown hotkey token: {token}"),
            ParseHotkeyError::MissingKey => write!(f, "hotkey is missing a non-modifier key"),
            ParseHotkeyError::MultipleKeys => write!(f, "hotkey has multiple non-modifier keys"),
        }
    }
}

impl std::error::Error for ParseHotkeyError {}

fn parse_modifier(token: &str) -> Option<KeyCode> {
    match token.to_ascii_lowercase().as_str() {
        "ctrl" | "control" => Some(KeyCode::KEY_LEFTCTRL),
        "shift" => Some(KeyCode::KEY_LEFTSHIFT),
        "alt" => Some(KeyCode::KEY_LEFTALT),
        "super" | "meta" | "win" | "windows" => Some(KeyCode::KEY_LEFTMETA),
        _ => None,
    }
}

pub(crate) fn canonical_modifier(key: KeyCode) -> KeyCode {
    match key {
        KeyCode::KEY_LEFTCTRL | KeyCode::KEY_RIGHTCTRL => KeyCode::KEY_LEFTCTRL,
        KeyCode::KEY_LEFTALT | KeyCode::KEY_RIGHTALT => KeyCode::KEY_LEFTALT,
        KeyCode::KEY_LEFTSHIFT | KeyCode::KEY_RIGHTSHIFT => KeyCode::KEY_LEFTSHIFT,
        KeyCode::KEY_LEFTMETA | KeyCode::KEY_RIGHTMETA => KeyCode::KEY_LEFTMETA,
        _ => key,
    }
}

#[allow(clippy::too_many_lines)]
fn parse_key(token: &str) -> Option<KeyCode> {
    let upper = token.to_ascii_uppercase();
    match upper.as_str() {
        "RETURN" | "ENTER" => Some(KeyCode::KEY_ENTER),
        "ESC" | "ESCAPE" => Some(KeyCode::KEY_ESC),
        "SPACE" => Some(KeyCode::KEY_SPACE),
        "TAB" => Some(KeyCode::KEY_TAB),
        "DELETE" | "DEL" => Some(KeyCode::KEY_DELETE),
        "BACKSPACE" | "BS" => Some(KeyCode::KEY_BACKSPACE),
        "INSERT" | "INS" => Some(KeyCode::KEY_INSERT),
        "HOME" => Some(KeyCode::KEY_HOME),
        "END" => Some(KeyCode::KEY_END),
        "PAGEUP" | "PGUP" => Some(KeyCode::KEY_PAGEUP),
        "PAGEDOWN" | "PGDOWN" | "PGDN" => Some(KeyCode::KEY_PAGEDOWN),
        "LEFT" => Some(KeyCode::KEY_LEFT),
        "RIGHT" => Some(KeyCode::KEY_RIGHT),
        "UP" => Some(KeyCode::KEY_UP),
        "DOWN" => Some(KeyCode::KEY_DOWN),
        "MINUS" | "DASH" => Some(KeyCode::KEY_MINUS),
        "EQUAL" | "PLUS" => Some(KeyCode::KEY_EQUAL),
        "LEFTBRACKET" | "LBRACKET" => Some(KeyCode::KEY_LEFTBRACE),
        "RIGHTBRACKET" | "RBRACKET" => Some(KeyCode::KEY_RIGHTBRACE),
        "BACKSLASH" | "PIPE" => Some(KeyCode::KEY_BACKSLASH),
        "SEMICOLON" => Some(KeyCode::KEY_SEMICOLON),
        "APOSTROPHE" | "QUOTE" => Some(KeyCode::KEY_APOSTROPHE),
        "GRAVE" | "BACKTICK" => Some(KeyCode::KEY_GRAVE),
        "COMMA" => Some(KeyCode::KEY_COMMA),
        "PERIOD" | "DOT" => Some(KeyCode::KEY_DOT),
        "SLASH" => Some(KeyCode::KEY_SLASH),
        "NUMPAD0" | "KP0" => Some(KeyCode::KEY_KP0),
        "NUMPAD1" | "KP1" => Some(KeyCode::KEY_KP1),
        "NUMPAD2" | "KP2" => Some(KeyCode::KEY_KP2),
        "NUMPAD3" | "KP3" => Some(KeyCode::KEY_KP3),
        "NUMPAD4" | "KP4" => Some(KeyCode::KEY_KP4),
        "NUMPAD5" | "KP5" => Some(KeyCode::KEY_KP5),
        "NUMPAD6" | "KP6" => Some(KeyCode::KEY_KP6),
        "NUMPAD7" | "KP7" => Some(KeyCode::KEY_KP7),
        "NUMPAD8" | "KP8" => Some(KeyCode::KEY_KP8),
        "NUMPAD9" | "KP9" => Some(KeyCode::KEY_KP9),
        "NUMPADDOT" | "KPDOT" => Some(KeyCode::KEY_KPDOT),
        "NUMPADPLUS" | "KPPLUS" => Some(KeyCode::KEY_KPPLUS),
        "NUMPADMINUS" | "KPMINUS" => Some(KeyCode::KEY_KPMINUS),
        "NUMPADASTERISK" | "NUMPADMULTIPLY" | "KPASTERISK" => Some(KeyCode::KEY_KPASTERISK),
        "NUMPADSLASH" | "KPSLASH" => Some(KeyCode::KEY_KPSLASH),
        "NUMPADENTER" | "KPENTER" => Some(KeyCode::KEY_KPENTER),
        "F1" => Some(KeyCode::KEY_F1),
        "F2" => Some(KeyCode::KEY_F2),
        "F3" => Some(KeyCode::KEY_F3),
        "F4" => Some(KeyCode::KEY_F4),
        "F5" => Some(KeyCode::KEY_F5),
        "F6" => Some(KeyCode::KEY_F6),
        "F7" => Some(KeyCode::KEY_F7),
        "F8" => Some(KeyCode::KEY_F8),
        "F9" => Some(KeyCode::KEY_F9),
        "F10" => Some(KeyCode::KEY_F10),
        "F11" => Some(KeyCode::KEY_F11),
        "F12" => Some(KeyCode::KEY_F12),
        "F13" => Some(KeyCode::KEY_F13),
        "F14" => Some(KeyCode::KEY_F14),
        "F15" => Some(KeyCode::KEY_F15),
        "F16" => Some(KeyCode::KEY_F16),
        "F17" => Some(KeyCode::KEY_F17),
        "F18" => Some(KeyCode::KEY_F18),
        "F19" => Some(KeyCode::KEY_F19),
        "F20" => Some(KeyCode::KEY_F20),
        "F21" => Some(KeyCode::KEY_F21),
        "F22" => Some(KeyCode::KEY_F22),
        "F23" => Some(KeyCode::KEY_F23),
        "F24" => Some(KeyCode::KEY_F24),
        _ if upper.len() == 1 => match upper.chars().next().unwrap() {
            'A' => Some(KeyCode::KEY_A),
            'B' => Some(KeyCode::KEY_B),
            'C' => Some(KeyCode::KEY_C),
            'D' => Some(KeyCode::KEY_D),
            'E' => Some(KeyCode::KEY_E),
            'F' => Some(KeyCode::KEY_F),
            'G' => Some(KeyCode::KEY_G),
            'H' => Some(KeyCode::KEY_H),
            'I' => Some(KeyCode::KEY_I),
            'J' => Some(KeyCode::KEY_J),
            'K' => Some(KeyCode::KEY_K),
            'L' => Some(KeyCode::KEY_L),
            'M' => Some(KeyCode::KEY_M),
            'N' => Some(KeyCode::KEY_N),
            'O' => Some(KeyCode::KEY_O),
            'P' => Some(KeyCode::KEY_P),
            'Q' => Some(KeyCode::KEY_Q),
            'R' => Some(KeyCode::KEY_R),
            'S' => Some(KeyCode::KEY_S),
            'T' => Some(KeyCode::KEY_T),
            'U' => Some(KeyCode::KEY_U),
            'V' => Some(KeyCode::KEY_V),
            'W' => Some(KeyCode::KEY_W),
            'X' => Some(KeyCode::KEY_X),
            'Y' => Some(KeyCode::KEY_Y),
            'Z' => Some(KeyCode::KEY_Z),
            '0' => Some(KeyCode::KEY_0),
            '1' => Some(KeyCode::KEY_1),
            '2' => Some(KeyCode::KEY_2),
            '3' => Some(KeyCode::KEY_3),
            '4' => Some(KeyCode::KEY_4),
            '5' => Some(KeyCode::KEY_5),
            '6' => Some(KeyCode::KEY_6),
            '7' => Some(KeyCode::KEY_7),
            '8' => Some(KeyCode::KEY_8),
            '9' => Some(KeyCode::KEY_9),
            '-' => Some(KeyCode::KEY_MINUS),
            '=' => Some(KeyCode::KEY_EQUAL),
            '[' => Some(KeyCode::KEY_LEFTBRACE),
            ']' => Some(KeyCode::KEY_RIGHTBRACE),
            '\\' => Some(KeyCode::KEY_BACKSLASH),
            ';' => Some(KeyCode::KEY_SEMICOLON),
            '\'' => Some(KeyCode::KEY_APOSTROPHE),
            '`' => Some(KeyCode::KEY_GRAVE),
            ',' => Some(KeyCode::KEY_COMMA),
            '.' => Some(KeyCode::KEY_DOT),
            '/' => Some(KeyCode::KEY_SLASH),
            _ => None,
        },
        _ => None,
    }
}

fn display_modifier(modifier: KeyCode) -> &'static str {
    match modifier {
        KeyCode::KEY_LEFTCTRL | KeyCode::KEY_RIGHTCTRL => "Ctrl",
        KeyCode::KEY_LEFTSHIFT | KeyCode::KEY_RIGHTSHIFT => "Shift",
        KeyCode::KEY_LEFTALT | KeyCode::KEY_RIGHTALT => "Alt",
        KeyCode::KEY_LEFTMETA | KeyCode::KEY_RIGHTMETA => "Super",
        _ => "Unknown",
    }
}

#[allow(clippy::too_many_lines)]
fn display_key(key: KeyCode) -> &'static str {
    match key {
        KeyCode::KEY_ENTER => "Enter",
        KeyCode::KEY_KPENTER => "NumpadEnter",
        KeyCode::KEY_ESC => "Esc",
        KeyCode::KEY_SPACE => "Space",
        KeyCode::KEY_TAB => "Tab",
        KeyCode::KEY_DELETE => "Delete",
        KeyCode::KEY_BACKSPACE => "Backspace",
        KeyCode::KEY_INSERT => "Insert",
        KeyCode::KEY_HOME => "Home",
        KeyCode::KEY_END => "End",
        KeyCode::KEY_PAGEUP => "PageUp",
        KeyCode::KEY_PAGEDOWN => "PageDown",
        KeyCode::KEY_LEFT => "Left",
        KeyCode::KEY_RIGHT => "Right",
        KeyCode::KEY_UP => "Up",
        KeyCode::KEY_DOWN => "Down",
        KeyCode::KEY_MINUS => "Minus",
        KeyCode::KEY_EQUAL => "Equal",
        KeyCode::KEY_LEFTBRACE => "LeftBracket",
        KeyCode::KEY_RIGHTBRACE => "RightBracket",
        KeyCode::KEY_BACKSLASH => "Backslash",
        KeyCode::KEY_SEMICOLON => "Semicolon",
        KeyCode::KEY_APOSTROPHE => "Apostrophe",
        KeyCode::KEY_GRAVE => "Grave",
        KeyCode::KEY_COMMA => "Comma",
        KeyCode::KEY_DOT => "Dot",
        KeyCode::KEY_SLASH => "Slash",
        KeyCode::KEY_KP0 => "Numpad0",
        KeyCode::KEY_KP1 => "Numpad1",
        KeyCode::KEY_KP2 => "Numpad2",
        KeyCode::KEY_KP3 => "Numpad3",
        KeyCode::KEY_KP4 => "Numpad4",
        KeyCode::KEY_KP5 => "Numpad5",
        KeyCode::KEY_KP6 => "Numpad6",
        KeyCode::KEY_KP7 => "Numpad7",
        KeyCode::KEY_KP8 => "Numpad8",
        KeyCode::KEY_KP9 => "Numpad9",
        KeyCode::KEY_KPDOT => "NumpadDot",
        KeyCode::KEY_KPPLUS => "NumpadPlus",
        KeyCode::KEY_KPMINUS => "NumpadMinus",
        KeyCode::KEY_KPASTERISK => "NumpadAsterisk",
        KeyCode::KEY_KPSLASH => "NumpadSlash",
        KeyCode::KEY_F1 => "F1",
        KeyCode::KEY_F2 => "F2",
        KeyCode::KEY_F3 => "F3",
        KeyCode::KEY_F4 => "F4",
        KeyCode::KEY_F5 => "F5",
        KeyCode::KEY_F6 => "F6",
        KeyCode::KEY_F7 => "F7",
        KeyCode::KEY_F8 => "F8",
        KeyCode::KEY_F9 => "F9",
        KeyCode::KEY_F10 => "F10",
        KeyCode::KEY_F11 => "F11",
        KeyCode::KEY_F12 => "F12",
        KeyCode::KEY_F13 => "F13",
        KeyCode::KEY_F14 => "F14",
        KeyCode::KEY_F15 => "F15",
        KeyCode::KEY_F16 => "F16",
        KeyCode::KEY_F17 => "F17",
        KeyCode::KEY_F18 => "F18",
        KeyCode::KEY_F19 => "F19",
        KeyCode::KEY_F20 => "F20",
        KeyCode::KEY_F21 => "F21",
        KeyCode::KEY_F22 => "F22",
        KeyCode::KEY_F23 => "F23",
        KeyCode::KEY_F24 => "F24",
        KeyCode::KEY_A => "A",
        KeyCode::KEY_B => "B",
        KeyCode::KEY_C => "C",
        KeyCode::KEY_D => "D",
        KeyCode::KEY_E => "E",
        KeyCode::KEY_F => "F",
        KeyCode::KEY_G => "G",
        KeyCode::KEY_H => "H",
        KeyCode::KEY_I => "I",
        KeyCode::KEY_J => "J",
        KeyCode::KEY_K => "K",
        KeyCode::KEY_L => "L",
        KeyCode::KEY_M => "M",
        KeyCode::KEY_N => "N",
        KeyCode::KEY_O => "O",
        KeyCode::KEY_P => "P",
        KeyCode::KEY_Q => "Q",
        KeyCode::KEY_R => "R",
        KeyCode::KEY_S => "S",
        KeyCode::KEY_T => "T",
        KeyCode::KEY_U => "U",
        KeyCode::KEY_V => "V",
        KeyCode::KEY_W => "W",
        KeyCode::KEY_X => "X",
        KeyCode::KEY_Y => "Y",
        KeyCode::KEY_Z => "Z",
        KeyCode::KEY_0 => "0",
        KeyCode::KEY_1 => "1",
        KeyCode::KEY_2 => "2",
        KeyCode::KEY_3 => "3",
        KeyCode::KEY_4 => "4",
        KeyCode::KEY_5 => "5",
        KeyCode::KEY_6 => "6",
        KeyCode::KEY_7 => "7",
        KeyCode::KEY_8 => "8",
        KeyCode::KEY_9 => "9",
        _ => "Unknown",
    }
}
