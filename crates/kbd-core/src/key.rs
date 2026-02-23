//! Key types: [`Key`], [`Modifier`], [`Hotkey`], [`HotkeySequence`].
//!
//! `Key` is a newtype wrapping a W3C physical key code. Associated constants
//! (`Key::A`, `Key::ENTER`, `Key::CONTROL_LEFT`) are the primary API for
//! referring to specific keys. The inner representation is private — `Key`
//! is a domain boundary that insulates the rest of the crate from the
//! upstream `keyboard_types` dependency.
//!
//! Platform-specific conversions (e.g., evdev key codes) live in their
//! respective backend crates (`kbd-evdev`) as extension traits.

use std::collections::HashMap;
use std::fmt;
use std::str::FromStr;
use std::sync::LazyLock;
use std::sync::Mutex;

use keyboard_types::Code;

/// A physical key on the keyboard.
///
/// Use the associated constants to refer to specific keys:
///
/// ```
/// use kbd_core::Key;
///
/// let key = Key::A;
/// assert_eq!(key.to_string(), "A");
/// ```
///
/// The inner representation is private. `Key` is a domain boundary — the
/// rest of the crate works with `Key` values, not raw key codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct Key(Code);

impl Key {
    // Letters
    pub const A: Self = Self(Code::KeyA);
    pub const B: Self = Self(Code::KeyB);
    pub const C: Self = Self(Code::KeyC);
    pub const D: Self = Self(Code::KeyD);
    pub const E: Self = Self(Code::KeyE);
    pub const F: Self = Self(Code::KeyF);
    pub const G: Self = Self(Code::KeyG);
    pub const H: Self = Self(Code::KeyH);
    pub const I: Self = Self(Code::KeyI);
    pub const J: Self = Self(Code::KeyJ);
    pub const K: Self = Self(Code::KeyK);
    pub const L: Self = Self(Code::KeyL);
    pub const M: Self = Self(Code::KeyM);
    pub const N: Self = Self(Code::KeyN);
    pub const O: Self = Self(Code::KeyO);
    pub const P: Self = Self(Code::KeyP);
    pub const Q: Self = Self(Code::KeyQ);
    pub const R: Self = Self(Code::KeyR);
    pub const S: Self = Self(Code::KeyS);
    pub const T: Self = Self(Code::KeyT);
    pub const U: Self = Self(Code::KeyU);
    pub const V: Self = Self(Code::KeyV);
    pub const W: Self = Self(Code::KeyW);
    pub const X: Self = Self(Code::KeyX);
    pub const Y: Self = Self(Code::KeyY);
    pub const Z: Self = Self(Code::KeyZ);

    // Digits
    pub const DIGIT0: Self = Self(Code::Digit0);
    pub const DIGIT1: Self = Self(Code::Digit1);
    pub const DIGIT2: Self = Self(Code::Digit2);
    pub const DIGIT3: Self = Self(Code::Digit3);
    pub const DIGIT4: Self = Self(Code::Digit4);
    pub const DIGIT5: Self = Self(Code::Digit5);
    pub const DIGIT6: Self = Self(Code::Digit6);
    pub const DIGIT7: Self = Self(Code::Digit7);
    pub const DIGIT8: Self = Self(Code::Digit8);
    pub const DIGIT9: Self = Self(Code::Digit9);

    // Function keys
    pub const F1: Self = Self(Code::F1);
    pub const F2: Self = Self(Code::F2);
    pub const F3: Self = Self(Code::F3);
    pub const F4: Self = Self(Code::F4);
    pub const F5: Self = Self(Code::F5);
    pub const F6: Self = Self(Code::F6);
    pub const F7: Self = Self(Code::F7);
    pub const F8: Self = Self(Code::F8);
    pub const F9: Self = Self(Code::F9);
    pub const F10: Self = Self(Code::F10);
    pub const F11: Self = Self(Code::F11);
    pub const F12: Self = Self(Code::F12);
    pub const F13: Self = Self(Code::F13);
    pub const F14: Self = Self(Code::F14);
    pub const F15: Self = Self(Code::F15);
    pub const F16: Self = Self(Code::F16);
    pub const F17: Self = Self(Code::F17);
    pub const F18: Self = Self(Code::F18);
    pub const F19: Self = Self(Code::F19);
    pub const F20: Self = Self(Code::F20);
    pub const F21: Self = Self(Code::F21);
    pub const F22: Self = Self(Code::F22);
    pub const F23: Self = Self(Code::F23);
    pub const F24: Self = Self(Code::F24);

    // Navigation and editing
    pub const ENTER: Self = Self(Code::Enter);
    pub const ESCAPE: Self = Self(Code::Escape);
    pub const SPACE: Self = Self(Code::Space);
    pub const TAB: Self = Self(Code::Tab);
    pub const DELETE: Self = Self(Code::Delete);
    pub const BACKSPACE: Self = Self(Code::Backspace);
    pub const INSERT: Self = Self(Code::Insert);
    pub const CAPS_LOCK: Self = Self(Code::CapsLock);
    pub const HOME: Self = Self(Code::Home);
    pub const END: Self = Self(Code::End);
    pub const PAGE_UP: Self = Self(Code::PageUp);
    pub const PAGE_DOWN: Self = Self(Code::PageDown);
    pub const ARROW_UP: Self = Self(Code::ArrowUp);
    pub const ARROW_DOWN: Self = Self(Code::ArrowDown);
    pub const ARROW_LEFT: Self = Self(Code::ArrowLeft);
    pub const ARROW_RIGHT: Self = Self(Code::ArrowRight);

    // Punctuation
    pub const MINUS: Self = Self(Code::Minus);
    pub const EQUAL: Self = Self(Code::Equal);
    pub const BRACKET_LEFT: Self = Self(Code::BracketLeft);
    pub const BRACKET_RIGHT: Self = Self(Code::BracketRight);
    pub const BACKSLASH: Self = Self(Code::Backslash);
    pub const SEMICOLON: Self = Self(Code::Semicolon);
    pub const QUOTE: Self = Self(Code::Quote);
    pub const BACKQUOTE: Self = Self(Code::Backquote);
    pub const COMMA: Self = Self(Code::Comma);
    pub const PERIOD: Self = Self(Code::Period);
    pub const SLASH: Self = Self(Code::Slash);

    // Numpad
    pub const NUMPAD0: Self = Self(Code::Numpad0);
    pub const NUMPAD1: Self = Self(Code::Numpad1);
    pub const NUMPAD2: Self = Self(Code::Numpad2);
    pub const NUMPAD3: Self = Self(Code::Numpad3);
    pub const NUMPAD4: Self = Self(Code::Numpad4);
    pub const NUMPAD5: Self = Self(Code::Numpad5);
    pub const NUMPAD6: Self = Self(Code::Numpad6);
    pub const NUMPAD7: Self = Self(Code::Numpad7);
    pub const NUMPAD8: Self = Self(Code::Numpad8);
    pub const NUMPAD9: Self = Self(Code::Numpad9);
    pub const NUMPAD_DECIMAL: Self = Self(Code::NumpadDecimal);
    pub const NUMPAD_ADD: Self = Self(Code::NumpadAdd);
    pub const NUMPAD_SUBTRACT: Self = Self(Code::NumpadSubtract);
    pub const NUMPAD_MULTIPLY: Self = Self(Code::NumpadMultiply);
    pub const NUMPAD_DIVIDE: Self = Self(Code::NumpadDivide);
    pub const NUMPAD_ENTER: Self = Self(Code::NumpadEnter);

    // Modifiers
    pub const CONTROL_LEFT: Self = Self(Code::ControlLeft);
    pub const CONTROL_RIGHT: Self = Self(Code::ControlRight);
    pub const SHIFT_LEFT: Self = Self(Code::ShiftLeft);
    pub const SHIFT_RIGHT: Self = Self(Code::ShiftRight);
    pub const ALT_LEFT: Self = Self(Code::AltLeft);
    pub const ALT_RIGHT: Self = Self(Code::AltRight);
    pub const META_LEFT: Self = Self(Code::MetaLeft);
    pub const META_RIGHT: Self = Self(Code::MetaRight);

    // Misc
    pub const UNIDENTIFIED: Self = Self(Code::Unidentified);

    // Media keys
    pub const AUDIO_VOLUME_UP: Self = Self(Code::AudioVolumeUp);
    pub const AUDIO_VOLUME_DOWN: Self = Self(Code::AudioVolumeDown);
    pub const AUDIO_VOLUME_MUTE: Self = Self(Code::AudioVolumeMute);
    pub const MEDIA_PLAY_PAUSE: Self = Self(Code::MediaPlayPause);
    pub const MEDIA_STOP: Self = Self(Code::MediaStop);
    pub const MEDIA_TRACK_NEXT: Self = Self(Code::MediaTrackNext);
    pub const MEDIA_TRACK_PREVIOUS: Self = Self(Code::MediaTrackPrevious);

    // System keys
    pub const PRINT_SCREEN: Self = Self(Code::PrintScreen);
    pub const SCROLL_LOCK: Self = Self(Code::ScrollLock);
    pub const PAUSE: Self = Self(Code::Pause);
    pub const NUM_LOCK: Self = Self(Code::NumLock);
    pub const CONTEXT_MENU: Self = Self(Code::ContextMenu);
    pub const POWER: Self = Self(Code::Power);

    /// Human-friendly name for this key.
    ///
    /// Most keys use the W3C standard name (`"Enter"`, `"Space"`,
    /// `"ShiftLeft"`, `"PrintScreen"`). A small set of keys that have
    /// verbose W3C names get short overrides for config-file readability:
    ///
    /// - Letters: `"A"` not `"KeyA"`
    /// - Digits: `"0"` not `"Digit0"`
    /// - Arrows: `"Up"` not `"ArrowUp"`
    ///
    /// Parsing accepts both forms — `"A"` and `"KeyA"` both work.
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        // Short overrides for keys whose W3C names are too verbose.
        match self.0 {
            Code::KeyA => "A",
            Code::KeyB => "B",
            Code::KeyC => "C",
            Code::KeyD => "D",
            Code::KeyE => "E",
            Code::KeyF => "F",
            Code::KeyG => "G",
            Code::KeyH => "H",
            Code::KeyI => "I",
            Code::KeyJ => "J",
            Code::KeyK => "K",
            Code::KeyL => "L",
            Code::KeyM => "M",
            Code::KeyN => "N",
            Code::KeyO => "O",
            Code::KeyP => "P",
            Code::KeyQ => "Q",
            Code::KeyR => "R",
            Code::KeyS => "S",
            Code::KeyT => "T",
            Code::KeyU => "U",
            Code::KeyV => "V",
            Code::KeyW => "W",
            Code::KeyX => "X",
            Code::KeyY => "Y",
            Code::KeyZ => "Z",
            Code::Digit0 => "0",
            Code::Digit1 => "1",
            Code::Digit2 => "2",
            Code::Digit3 => "3",
            Code::Digit4 => "4",
            Code::Digit5 => "5",
            Code::Digit6 => "6",
            Code::Digit7 => "7",
            Code::Digit8 => "8",
            Code::Digit9 => "9",
            Code::ArrowUp => "Up",
            Code::ArrowDown => "Down",
            Code::ArrowLeft => "Left",
            Code::ArrowRight => "Right",
            // Everything else: delegate to Code's Display (W3C standard name).
            _ => {
                static CACHE: LazyLock<Mutex<HashMap<Code, &'static str>>> =
                    LazyLock::new(|| Mutex::new(HashMap::new()));

                let mut cache = CACHE
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                cache
                    .entry(self.0)
                    .or_insert_with(|| Box::leak(self.0.to_string().into_boxed_str()))
            }
        }
    }
}

impl From<Code> for Key {
    fn from(code: Code) -> Self {
        Self(code)
    }
}

impl From<Key> for Code {
    fn from(key: Key) -> Self {
        key.0
    }
}

impl fmt::Display for Key {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for Key {
    type Err = ParseHotkeyError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        parse_key_token(s).ok_or_else(|| ParseHotkeyError::UnknownToken(s.trim().to_string()))
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
            '0' => Some(Key::DIGIT0),
            '1' => Some(Key::DIGIT1),
            '2' => Some(Key::DIGIT2),
            '3' => Some(Key::DIGIT3),
            '4' => Some(Key::DIGIT4),
            '5' => Some(Key::DIGIT5),
            '6' => Some(Key::DIGIT6),
            '7' => Some(Key::DIGIT7),
            '8' => Some(Key::DIGIT8),
            '9' => Some(Key::DIGIT9),
            '-' => Some(Key::MINUS),
            '=' => Some(Key::EQUAL),
            '[' => Some(Key::BRACKET_LEFT),
            ']' => Some(Key::BRACKET_RIGHT),
            '\\' => Some(Key::BACKSLASH),
            ';' => Some(Key::SEMICOLON),
            '\'' => Some(Key::QUOTE),
            '`' => Some(Key::BACKQUOTE),
            ',' => Some(Key::COMMA),
            '.' => Some(Key::PERIOD),
            '/' => Some(Key::SLASH),
            _ => None,
        };
    }

    let upper = trimmed.to_ascii_uppercase();

    if let Some(function_number) = upper.strip_prefix('F')
        && let Ok(number) = function_number.parse::<u8>()
    {
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

    match upper.as_str() {
        "RETURN" | "ENTER" => Some(Key::ENTER),
        "ESC" | "ESCAPE" => Some(Key::ESCAPE),
        "SPACE" => Some(Key::SPACE),
        "TAB" => Some(Key::TAB),
        "DEL" | "DELETE" => Some(Key::DELETE),
        "BS" | "BACKSPACE" => Some(Key::BACKSPACE),
        "INS" | "INSERT" => Some(Key::INSERT),
        "CAPSLOCK" => Some(Key::CAPS_LOCK),
        "HOME" => Some(Key::HOME),
        "END" => Some(Key::END),
        "PAGEUP" | "PGUP" => Some(Key::PAGE_UP),
        "PAGEDOWN" | "PGDN" => Some(Key::PAGE_DOWN),
        "UP" => Some(Key::ARROW_UP),
        "DOWN" => Some(Key::ARROW_DOWN),
        "LEFT" => Some(Key::ARROW_LEFT),
        "RIGHT" => Some(Key::ARROW_RIGHT),
        "MINUS" | "DASH" => Some(Key::MINUS),
        "EQUAL" | "PLUS" => Some(Key::EQUAL),
        "LEFTBRACKET" | "LBRACKET" => Some(Key::BRACKET_LEFT),
        "RIGHTBRACKET" | "RBRACKET" => Some(Key::BRACKET_RIGHT),
        "BACKSLASH" | "PIPE" => Some(Key::BACKSLASH),
        "SEMICOLON" => Some(Key::SEMICOLON),
        "APOSTROPHE" | "QUOTE" => Some(Key::QUOTE),
        "GRAVE" | "BACKTICK" => Some(Key::BACKQUOTE),
        "COMMA" => Some(Key::COMMA),
        "PERIOD" | "DOT" => Some(Key::PERIOD),
        "SLASH" => Some(Key::SLASH),
        "NUMPAD0" | "KP0" => Some(Key::NUMPAD0),
        "NUMPAD1" | "KP1" => Some(Key::NUMPAD1),
        "NUMPAD2" | "KP2" => Some(Key::NUMPAD2),
        "NUMPAD3" | "KP3" => Some(Key::NUMPAD3),
        "NUMPAD4" | "KP4" => Some(Key::NUMPAD4),
        "NUMPAD5" | "KP5" => Some(Key::NUMPAD5),
        "NUMPAD6" | "KP6" => Some(Key::NUMPAD6),
        "NUMPAD7" | "KP7" => Some(Key::NUMPAD7),
        "NUMPAD8" | "KP8" => Some(Key::NUMPAD8),
        "NUMPAD9" | "KP9" => Some(Key::NUMPAD9),
        "NUMPADDOT" | "KPDOT" => Some(Key::NUMPAD_DECIMAL),
        "NUMPADPLUS" | "KPPLUS" => Some(Key::NUMPAD_ADD),
        "NUMPADMINUS" | "KPMINUS" => Some(Key::NUMPAD_SUBTRACT),
        "NUMPADMULTIPLY" | "NUMPADASTERISK" | "KPASTERISK" => Some(Key::NUMPAD_MULTIPLY),
        "NUMPADDIVIDE" | "NUMPADSLASH" | "KPSLASH" => Some(Key::NUMPAD_DIVIDE),
        "NUMPADENTER" | "KPENTER" => Some(Key::NUMPAD_ENTER),
        "CTRL" | "CONTROL" | "LEFTCTRL" | "LCTRL" => Some(Key::CONTROL_LEFT),
        "RIGHTCTRL" | "RCTRL" => Some(Key::CONTROL_RIGHT),
        "SHIFT" | "LEFTSHIFT" | "LSHIFT" => Some(Key::SHIFT_LEFT),
        "RIGHTSHIFT" | "RSHIFT" => Some(Key::SHIFT_RIGHT),
        "ALT" | "LEFTALT" | "LALT" => Some(Key::ALT_LEFT),
        "RIGHTALT" | "RALT" => Some(Key::ALT_RIGHT),
        "SUPER" | "META" | "WIN" | "WINDOWS" | "LEFTSUPER" | "LSUPER" | "LEFTMETA" | "LMETA" => {
            Some(Key::META_LEFT)
        }
        "RIGHTSUPER" | "RSUPER" | "RIGHTMETA" | "RMETA" => Some(Key::META_RIGHT),
        "VOLUMEUP" | "VOLUP" => Some(Key::AUDIO_VOLUME_UP),
        "VOLUMEDOWN" | "VOLDOWN" => Some(Key::AUDIO_VOLUME_DOWN),
        "VOLUMEMUTE" | "MUTE" => Some(Key::AUDIO_VOLUME_MUTE),
        "MEDIAPLAYPAUSE" | "PLAYPAUSE" => Some(Key::MEDIA_PLAY_PAUSE),
        "MEDIASTOP" => Some(Key::MEDIA_STOP),
        "MEDIANEXT" | "MEDIATRACKNEXT" => Some(Key::MEDIA_TRACK_NEXT),
        "MEDIAPREVIOUS" | "MEDIATRACKPREVIOUS" | "MEDIAPREV" => Some(Key::MEDIA_TRACK_PREVIOUS),
        "PRINTSCREEN" | "PRINT" | "PRTSC" | "SYSRQ" => Some(Key::PRINT_SCREEN),
        "SCROLLLOCK" => Some(Key::SCROLL_LOCK),
        "PAUSE" | "BREAK" => Some(Key::PAUSE),
        "NUMLOCK" => Some(Key::NUM_LOCK),
        "CONTEXTMENU" | "MENU" | "APPS" => Some(Key::CONTEXT_MENU),
        "POWER" => Some(Key::POWER),
        // Fallback: try the W3C standard name (PascalCase, case-sensitive).
        // This ensures round-tripping: as_str() outputs "KeyA", parse accepts "KeyA".
        _ => Code::from_str(trimmed).ok().map(Key::from),
    }
}

impl From<Modifier> for Key {
    fn from(value: Modifier) -> Self {
        value.keys().0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Modifier {
    Ctrl,
    Shift,
    Alt,
    Super,
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

    /// Check whether a key is a modifier key, returning the canonical modifier.
    ///
    /// Left/right variants canonicalize: both `ControlLeft` and `ControlRight`
    /// return `Some(Modifier::Ctrl)`.
    #[must_use]
    pub fn from_key(key: Key) -> Option<Self> {
        match key.0 {
            Code::ControlLeft | Code::ControlRight => Some(Self::Ctrl),
            Code::ShiftLeft | Code::ShiftRight => Some(Self::Shift),
            Code::AltLeft | Code::AltRight => Some(Self::Alt),
            Code::MetaLeft | Code::MetaRight => Some(Self::Super),
            _ => None,
        }
    }

    #[must_use]
    pub const fn keys(self) -> (Key, Key) {
        match self {
            Self::Ctrl => (Key::CONTROL_LEFT, Key::CONTROL_RIGHT),
            Self::Shift => (Key::SHIFT_LEFT, Key::SHIFT_RIGHT),
            Self::Alt => (Key::ALT_LEFT, Key::ALT_RIGHT),
            Self::Super => (Key::META_LEFT, Key::META_RIGHT),
        }
    }
}

impl fmt::Display for Modifier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

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

impl TryFrom<Key> for Modifier {
    type Error = Key;

    fn try_from(value: Key) -> Result<Self, Self::Error> {
        Self::from_key(value).ok_or(value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Hotkey {
    key: Key,
    modifiers: Vec<Modifier>,
}

impl Hotkey {
    /// Create a hotkey for a single key with no modifiers.
    #[must_use]
    pub fn new(key: Key) -> Self {
        Self {
            key,
            modifiers: Vec::new(),
        }
    }

    /// Create a hotkey from a key and a list of modifiers.
    #[must_use]
    pub fn with_modifiers(key: Key, mut modifiers: Vec<Modifier>) -> Self {
        modifiers.sort();
        modifiers.dedup();
        Self { key, modifiers }
    }

    /// Add a modifier to this hotkey.
    #[must_use]
    pub fn modifier(mut self, modifier: Modifier) -> Self {
        if !self.modifiers.contains(&modifier) {
            self.modifiers.push(modifier);
            self.modifiers.sort();
        }
        self
    }

    #[must_use]
    pub fn key(&self) -> Key {
        self.key
    }

    #[must_use]
    pub fn modifiers(&self) -> &[Modifier] {
        &self.modifiers
    }
}

impl From<Key> for Hotkey {
    fn from(key: Key) -> Self {
        Self::new(key)
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

        Ok(Self::with_modifiers(key, modifiers))
    }
}

impl fmt::Display for Hotkey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for modifier in &self.modifiers {
            write!(f, "{modifier}+")?;
        }

        write!(f, "{}", self.key)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct HotkeySequence {
    steps: Vec<Hotkey>,
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

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ParseHotkeyError {
    #[error("hotkey string is empty")]
    Empty,
    #[error("hotkey contains an empty token")]
    EmptySegment,
    #[error("unknown hotkey token: {0}")]
    UnknownToken(String),
    #[error("hotkey is missing a non-modifier key")]
    MissingKey,
    #[error("hotkey has multiple non-modifier keys")]
    MultipleKeys,
}

#[cfg(test)]
mod tests {
    use keyboard_types::Code;

    use super::*;

    #[test]
    fn parses_aliases_case_insensitive() {
        let hotkey = "ctrl+Win+return".parse::<Hotkey>().unwrap();
        assert_eq!(hotkey.key(), Key::ENTER);
        assert_eq!(hotkey.modifiers(), &[Modifier::Ctrl, Modifier::Super]);
    }

    #[test]
    fn hotkey_new_sorts_and_dedups_modifiers() {
        let hotkey =
            Hotkey::with_modifiers(Key::A, vec![Modifier::Alt, Modifier::Ctrl, Modifier::Alt]);
        assert_eq!(hotkey.modifiers(), &[Modifier::Ctrl, Modifier::Alt]);
    }

    #[test]
    fn modifier_canonicalizes_left_and_right_keys() {
        assert_eq!(Modifier::from_key(Key::CONTROL_LEFT), Some(Modifier::Ctrl));
        assert_eq!(Modifier::from_key(Key::CONTROL_RIGHT), Some(Modifier::Ctrl));
        assert_eq!(Modifier::from_key(Key::SHIFT_LEFT), Some(Modifier::Shift));
        assert_eq!(Modifier::from_key(Key::SHIFT_RIGHT), Some(Modifier::Shift));
        assert_eq!(Modifier::from_key(Key::A), None);
    }

    #[test]
    fn sequence_display_round_trips() {
        let sequence = "Ctrl+K, Ctrl+C".parse::<HotkeySequence>().unwrap();
        let round_trip = sequence.to_string().parse::<HotkeySequence>().unwrap();
        assert_eq!(round_trip, sequence);
    }

    #[test]
    fn key_constants_map_to_expected_codes() {
        assert_eq!(Key::A, Key(Code::KeyA));
        assert_eq!(Key::ENTER, Key(Code::Enter));
        assert_eq!(Key::CONTROL_LEFT, Key(Code::ControlLeft));
        assert_eq!(Key::DIGIT0, Key(Code::Digit0));
        assert_eq!(Key::ARROW_UP, Key(Code::ArrowUp));
    }

    #[test]
    fn key_from_code_round_trips() {
        let key = Key::from(Code::KeyA);
        assert_eq!(key, Key::A);

        let code = Code::from(key);
        assert_eq!(code, Code::KeyA);
    }

    #[test]
    fn key_display_short_overrides() {
        // Letters, digits, arrows get short names
        assert_eq!(Key::A.to_string(), "A");
        assert_eq!(Key::Z.to_string(), "Z");
        assert_eq!(Key::DIGIT0.to_string(), "0");
        assert_eq!(Key::DIGIT9.to_string(), "9");
        assert_eq!(Key::ARROW_UP.to_string(), "Up");
        assert_eq!(Key::ARROW_LEFT.to_string(), "Left");
    }

    #[test]
    fn key_display_delegates_to_w3c_for_rest() {
        // These use Code's Display directly — W3C standard names
        assert_eq!(Key::ENTER.to_string(), "Enter");
        assert_eq!(Key::ESCAPE.to_string(), "Escape");
        assert_eq!(Key::CONTROL_LEFT.to_string(), "ControlLeft");
        assert_eq!(Key::SHIFT_LEFT.to_string(), "ShiftLeft");
        assert_eq!(Key::META_LEFT.to_string(), "MetaLeft");
        assert_eq!(Key::AUDIO_VOLUME_UP.to_string(), "AudioVolumeUp");
        assert_eq!(Key::PRINT_SCREEN.to_string(), "PrintScreen");
        assert_eq!(Key::NUM_LOCK.to_string(), "NumLock");
        assert_eq!(Key::CONTEXT_MENU.to_string(), "ContextMenu");
    }

    #[test]
    fn key_display_for_code_without_constant() {
        let key = Key::from(Code::Again);
        assert_eq!(key.to_string(), "Again");
    }

    #[test]
    fn key_parse_round_trips_for_known_keys() {
        for key in [
            Key::A,
            Key::ENTER,
            Key::ESCAPE,
            Key::DIGIT0,
            Key::F1,
            Key::ARROW_UP,
            Key::NUMPAD0,
            Key::NUMPAD_ENTER,
            Key::AUDIO_VOLUME_UP,
            Key::PRINT_SCREEN,
            Key::SCROLL_LOCK,
            Key::PAUSE,
            Key::NUM_LOCK,
            Key::CONTEXT_MENU,
            Key::POWER,
        ] {
            let s = key.to_string();
            let parsed: Key = s.parse().unwrap();
            assert_eq!(parsed, key, "round-trip failed for {s}");
        }
    }
}
