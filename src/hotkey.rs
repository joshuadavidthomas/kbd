use evdev::KeyCode;
use std::fmt;
use std::str::FromStr;

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
        modifiers.sort();
        modifiers.dedup();
        Self { key, modifiers }
    }

    pub fn key(&self) -> KeyCode {
        self.key
    }

    pub fn modifiers(&self) -> &[KeyCode] {
        &self.modifiers
    }
}

impl HotkeySequence {
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

        if steps.is_empty() {
            return Err(ParseHotkeyError::Empty);
        }

        Ok(Self { steps })
    }
}

impl fmt::Display for Hotkey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut parts: Vec<&str> = self.modifiers.iter().map(display_modifier).collect();
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

fn parse_key(token: &str) -> Option<KeyCode> {
    let upper = token.to_ascii_uppercase();
    match upper.as_str() {
        "RETURN" | "ENTER" => Some(KeyCode::KEY_ENTER),
        "ESC" | "ESCAPE" => Some(KeyCode::KEY_ESC),
        "SPACE" => Some(KeyCode::KEY_SPACE),
        "TAB" => Some(KeyCode::KEY_TAB),
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
            _ => None,
        },
        _ => None,
    }
}

fn display_modifier(modifier: &KeyCode) -> &'static str {
    match *modifier {
        KeyCode::KEY_LEFTCTRL | KeyCode::KEY_RIGHTCTRL => "Ctrl",
        KeyCode::KEY_LEFTSHIFT | KeyCode::KEY_RIGHTSHIFT => "Shift",
        KeyCode::KEY_LEFTALT | KeyCode::KEY_RIGHTALT => "Alt",
        KeyCode::KEY_LEFTMETA | KeyCode::KEY_RIGHTMETA => "Super",
        _ => "Unknown",
    }
}

fn display_key(key: KeyCode) -> &'static str {
    match key {
        KeyCode::KEY_ENTER => "Enter",
        KeyCode::KEY_ESC => "Esc",
        KeyCode::KEY_SPACE => "Space",
        KeyCode::KEY_TAB => "Tab",
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
