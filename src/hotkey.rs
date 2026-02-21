use std::fmt;
use std::str::FromStr;

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

use crate::key::Key;
use crate::key::Modifier;

/// A single key + modifier combination such as `Ctrl+Shift+A`.
///
/// Construct via [`Hotkey::new`] or parse from a string:
///
/// ```
/// use keybound::{Hotkey, Key, Modifier};
///
/// let hotkey = "Ctrl+Shift+A".parse::<Hotkey>().unwrap();
/// assert_eq!(hotkey.key(), Key::A);
/// assert_eq!(hotkey.modifiers(), &[Modifier::Ctrl, Modifier::Shift]);
/// ```
///
/// Modifier aliases `Control`, `Meta`, `Win`, and `Windows` are accepted
/// when parsing. The [`Display`](std::fmt::Display) impl produces a
/// canonical representation that round-trips through [`FromStr`](std::str::FromStr).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hotkey {
    key: Key,
    modifiers: Vec<Modifier>,
}

/// An ordered sequence of [`Hotkey`] steps, such as `Ctrl+K, Ctrl+C`.
///
/// Sequences must contain at least two steps. Parse from a comma-separated
/// string or build programmatically:
///
/// ```
/// use keybound::HotkeySequence;
///
/// let seq: HotkeySequence = "Ctrl+K, Ctrl+C".parse().unwrap();
/// assert_eq!(seq.steps().len(), 2);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HotkeySequence {
    steps: Vec<Hotkey>,
}

/// Errors produced when parsing a [`Hotkey`] or [`HotkeySequence`] from a string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseHotkeyError {
    /// The input string was empty.
    Empty,
    /// A segment between `+` separators was empty (e.g. `"Ctrl++A"`).
    EmptySegment,
    /// A token could not be recognized as a key or modifier.
    UnknownToken(String),
    /// No non-modifier key was found (e.g. `"Ctrl+Shift"`).
    MissingKey,
    /// More than one non-modifier key was found (e.g. `"A+B"`).
    MultipleKeys,
}

impl Hotkey {
    /// Create a hotkey from a target key and a list of modifiers.
    ///
    /// Modifiers are deduplicated and sorted into a canonical order so that
    /// `Hotkey::new(Key::A, vec![Modifier::Shift, Modifier::Ctrl])` equals
    /// `Hotkey::new(Key::A, vec![Modifier::Ctrl, Modifier::Shift])`.
    #[must_use]
    pub fn new(key: Key, mut modifiers: Vec<Modifier>) -> Self {
        modifiers.sort();
        modifiers.dedup();
        Self { key, modifiers }
    }

    /// Returns the target (non-modifier) key.
    #[must_use]
    pub fn key(&self) -> Key {
        self.key
    }

    /// Returns the modifier keys in canonical sorted order.
    #[must_use]
    pub fn modifiers(&self) -> &[Modifier] {
        &self.modifiers
    }
}

impl HotkeySequence {
    /// Create a sequence from a list of [`Hotkey`] steps.
    ///
    /// Returns [`ParseHotkeyError::Empty`] if `steps` is empty. Note that
    /// [`HotkeyManager::register_sequence`](crate::HotkeyManager::register_sequence)
    /// additionally requires at least two steps.
    pub fn new(steps: Vec<Hotkey>) -> Result<Self, ParseHotkeyError> {
        if steps.is_empty() {
            return Err(ParseHotkeyError::Empty);
        }

        Ok(Self { steps })
    }

    /// Returns the ordered steps of the sequence.
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
        let mut parts: Vec<&str> = self.modifiers.iter().map(|m| m.as_str()).collect();
        parts.push(self.key.as_str());
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

fn parse_modifier(token: &str) -> Option<Modifier> {
    match token.to_ascii_lowercase().as_str() {
        "ctrl" | "control" => Some(Modifier::Ctrl),
        "shift" => Some(Modifier::Shift),
        "alt" => Some(Modifier::Alt),
        "super" | "meta" | "win" | "windows" => Some(Modifier::Super),
        _ => None,
    }
}

#[allow(clippy::too_many_lines)]
fn parse_key(token: &str) -> Option<Key> {
    let upper = token.to_ascii_uppercase();
    match upper.as_str() {
        "RETURN" | "ENTER" => Some(Key::Enter),
        "ESC" | "ESCAPE" => Some(Key::Escape),
        "SPACE" => Some(Key::Space),
        "TAB" => Some(Key::Tab),
        "DELETE" | "DEL" => Some(Key::Delete),
        "BACKSPACE" | "BS" => Some(Key::Backspace),
        "INSERT" | "INS" => Some(Key::Insert),
        "CAPSLOCK" => Some(Key::CapsLock),
        "HOME" => Some(Key::Home),
        "END" => Some(Key::End),
        "PAGEUP" | "PGUP" => Some(Key::PageUp),
        "PAGEDOWN" | "PGDOWN" | "PGDN" => Some(Key::PageDown),
        "LEFT" => Some(Key::Left),
        "RIGHT" => Some(Key::Right),
        "UP" => Some(Key::Up),
        "DOWN" => Some(Key::Down),
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
        "NUMPADASTERISK" | "NUMPADMULTIPLY" | "KPASTERISK" => Some(Key::NumpadMultiply),
        "NUMPADSLASH" | "KPSLASH" => Some(Key::NumpadDivide),
        "NUMPADENTER" | "KPENTER" => Some(Key::NumpadEnter),
        "F1" => Some(Key::F1),
        "F2" => Some(Key::F2),
        "F3" => Some(Key::F3),
        "F4" => Some(Key::F4),
        "F5" => Some(Key::F5),
        "F6" => Some(Key::F6),
        "F7" => Some(Key::F7),
        "F8" => Some(Key::F8),
        "F9" => Some(Key::F9),
        "F10" => Some(Key::F10),
        "F11" => Some(Key::F11),
        "F12" => Some(Key::F12),
        "F13" => Some(Key::F13),
        "F14" => Some(Key::F14),
        "F15" => Some(Key::F15),
        "F16" => Some(Key::F16),
        "F17" => Some(Key::F17),
        "F18" => Some(Key::F18),
        "F19" => Some(Key::F19),
        "F20" => Some(Key::F20),
        "F21" => Some(Key::F21),
        "F22" => Some(Key::F22),
        "F23" => Some(Key::F23),
        "F24" => Some(Key::F24),
        _ if upper.len() == 1 => match upper.chars().next().unwrap() {
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
        },
        _ => None,
    }
}
