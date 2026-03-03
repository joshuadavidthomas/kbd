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
/// use kbd::key::Key;
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

/// Key constants grouped by category.
///
/// All constants use the W3C UI Events specification names. The list covers
/// the full 104/105-key PC layout, function keys through F35, media and
/// browser keys, numpad, international input, and system keys.
#[allow(missing_docs)]
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

    // Extended function keys
    pub const F25: Self = Self(Code::F25);
    pub const F26: Self = Self(Code::F26);
    pub const F27: Self = Self(Code::F27);
    pub const F28: Self = Self(Code::F28);
    pub const F29: Self = Self(Code::F29);
    pub const F30: Self = Self(Code::F30);
    pub const F31: Self = Self(Code::F31);
    pub const F32: Self = Self(Code::F32);
    pub const F33: Self = Self(Code::F33);
    pub const F34: Self = Self(Code::F34);
    pub const F35: Self = Self(Code::F35);

    // Browser keys
    pub const BROWSER_BACK: Self = Self(Code::BrowserBack);
    pub const BROWSER_FAVORITES: Self = Self(Code::BrowserFavorites);
    pub const BROWSER_FORWARD: Self = Self(Code::BrowserForward);
    pub const BROWSER_HOME: Self = Self(Code::BrowserHome);
    pub const BROWSER_REFRESH: Self = Self(Code::BrowserRefresh);
    pub const BROWSER_SEARCH: Self = Self(Code::BrowserSearch);
    pub const BROWSER_STOP: Self = Self(Code::BrowserStop);

    // Extended media keys
    pub const MEDIA_FAST_FORWARD: Self = Self(Code::MediaFastForward);
    pub const MEDIA_PAUSE: Self = Self(Code::MediaPause);
    pub const MEDIA_PLAY: Self = Self(Code::MediaPlay);
    pub const MEDIA_RECORD: Self = Self(Code::MediaRecord);
    pub const MEDIA_REWIND: Self = Self(Code::MediaRewind);
    pub const MEDIA_SELECT: Self = Self(Code::MediaSelect);
    pub const MICROPHONE_MUTE_TOGGLE: Self = Self(Code::MicrophoneMuteToggle);

    // System keys
    pub const PRINT_SCREEN: Self = Self(Code::PrintScreen);
    pub const SCROLL_LOCK: Self = Self(Code::ScrollLock);
    pub const PAUSE: Self = Self(Code::Pause);
    pub const NUM_LOCK: Self = Self(Code::NumLock);
    pub const CONTEXT_MENU: Self = Self(Code::ContextMenu);
    pub const POWER: Self = Self(Code::Power);
    pub const SLEEP: Self = Self(Code::Sleep);
    pub const WAKE_UP: Self = Self(Code::WakeUp);
    pub const EJECT: Self = Self(Code::Eject);
    pub const BRIGHTNESS_DOWN: Self = Self(Code::BrightnessDown);
    pub const BRIGHTNESS_UP: Self = Self(Code::BrightnessUp);
    pub const DISPLAY_TOGGLE_INT_EXT: Self = Self(Code::DisplayToggleIntExt);
    pub const PRIVACY_SCREEN_TOGGLE: Self = Self(Code::PrivacyScreenToggle);
    pub const KEYBOARD_BACKLIGHT_TOGGLE: Self = Self(Code::KeyboardBacklightToggle);

    // Fn keys
    pub const FN: Self = Self(Code::Fn);
    pub const FN_LOCK: Self = Self(Code::FnLock);

    // Clipboard / editing keys
    pub const COPY: Self = Self(Code::Copy);
    pub const CUT: Self = Self(Code::Cut);
    pub const PASTE: Self = Self(Code::Paste);
    pub const UNDO: Self = Self(Code::Undo);
    pub const FIND: Self = Self(Code::Find);
    pub const HELP: Self = Self(Code::Help);
    pub const OPEN: Self = Self(Code::Open);
    pub const SELECT: Self = Self(Code::Select);
    pub const AGAIN: Self = Self(Code::Again);
    pub const PROPS: Self = Self(Code::Props);
    pub const ABORT: Self = Self(Code::Abort);
    pub const RESUME: Self = Self(Code::Resume);
    pub const SUSPEND: Self = Self(Code::Suspend);

    // Legacy / niche modifier keys (deprecated in W3C spec, but real hardware exists)
    #[allow(deprecated)]
    pub const HYPER: Self = Self(Code::Hyper);
    /// The legacy `Super` key code in the W3C spec (distinct from `MetaLeft`/`MetaRight`).
    /// Displays as `"SuperKey"` to avoid conflict with the `"Super"` modifier alias
    /// which resolves to `MetaLeft`.
    #[allow(deprecated)]
    pub const SUPER_KEY: Self = Self(Code::Super);
    #[allow(deprecated)]
    pub const TURBO: Self = Self(Code::Turbo);

    // CJK input keys
    pub const CONVERT: Self = Self(Code::Convert);
    pub const NON_CONVERT: Self = Self(Code::NonConvert);
    pub const KANA_MODE: Self = Self(Code::KanaMode);
    pub const HIRAGANA: Self = Self(Code::Hiragana);
    pub const KATAKANA: Self = Self(Code::Katakana);
    pub const KEYBOARD_LAYOUT_SELECT: Self = Self(Code::KeyboardLayoutSelect);

    // Language keys
    pub const LANG1: Self = Self(Code::Lang1);
    pub const LANG2: Self = Self(Code::Lang2);
    pub const LANG3: Self = Self(Code::Lang3);
    pub const LANG4: Self = Self(Code::Lang4);
    pub const LANG5: Self = Self(Code::Lang5);

    // International keys
    pub const INTL_BACKSLASH: Self = Self(Code::IntlBackslash);
    pub const INTL_RO: Self = Self(Code::IntlRo);
    pub const INTL_YEN: Self = Self(Code::IntlYen);

    // App launch keys
    pub const LAUNCH_APP1: Self = Self(Code::LaunchApp1);
    pub const LAUNCH_APP2: Self = Self(Code::LaunchApp2);
    pub const LAUNCH_ASSISTANT: Self = Self(Code::LaunchAssistant);
    pub const LAUNCH_CONTROL_PANEL: Self = Self(Code::LaunchControlPanel);
    pub const LAUNCH_MAIL: Self = Self(Code::LaunchMail);
    pub const LAUNCH_SCREEN_SAVER: Self = Self(Code::LaunchScreenSaver);

    // Mail keys
    pub const MAIL_FORWARD: Self = Self(Code::MailForward);
    pub const MAIL_REPLY: Self = Self(Code::MailReply);
    pub const MAIL_SEND: Self = Self(Code::MailSend);

    // Extended numpad
    pub const NUMPAD_BACKSPACE: Self = Self(Code::NumpadBackspace);
    pub const NUMPAD_CLEAR: Self = Self(Code::NumpadClear);
    pub const NUMPAD_CLEAR_ENTRY: Self = Self(Code::NumpadClearEntry);
    pub const NUMPAD_COMMA: Self = Self(Code::NumpadComma);
    pub const NUMPAD_EQUAL: Self = Self(Code::NumpadEqual);
    pub const NUMPAD_HASH: Self = Self(Code::NumpadHash);
    pub const NUMPAD_MEMORY_ADD: Self = Self(Code::NumpadMemoryAdd);
    pub const NUMPAD_MEMORY_CLEAR: Self = Self(Code::NumpadMemoryClear);
    pub const NUMPAD_MEMORY_RECALL: Self = Self(Code::NumpadMemoryRecall);
    pub const NUMPAD_MEMORY_STORE: Self = Self(Code::NumpadMemoryStore);
    pub const NUMPAD_MEMORY_SUBTRACT: Self = Self(Code::NumpadMemorySubtract);
    pub const NUMPAD_PAREN_LEFT: Self = Self(Code::NumpadParenLeft);
    pub const NUMPAD_PAREN_RIGHT: Self = Self(Code::NumpadParenRight);
    pub const NUMPAD_STAR: Self = Self(Code::NumpadStar);

    // Misc system/app keys
    pub const SELECT_TASK: Self = Self(Code::SelectTask);
    pub const SHOW_ALL_WINDOWS: Self = Self(Code::ShowAllWindows);
    pub const ZOOM_TOGGLE: Self = Self(Code::ZoomToggle);

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
            // Disambiguate legacy Code::Super from MetaLeft "Super" modifier alias
            #[allow(deprecated)]
            Code::Super => "SuperKey",
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
            25 => Some(Key::F25),
            26 => Some(Key::F26),
            27 => Some(Key::F27),
            28 => Some(Key::F28),
            29 => Some(Key::F29),
            30 => Some(Key::F30),
            31 => Some(Key::F31),
            32 => Some(Key::F32),
            33 => Some(Key::F33),
            34 => Some(Key::F34),
            35 => Some(Key::F35),
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
        "SLEEP" => Some(Key::SLEEP),
        "WAKEUP" => Some(Key::WAKE_UP),
        "EJECT" => Some(Key::EJECT),
        "BRIGHTNESSDOWN" | "BRIGHTDOWN" => Some(Key::BRIGHTNESS_DOWN),
        "BRIGHTNESSUP" | "BRIGHTUP" => Some(Key::BRIGHTNESS_UP),
        "DISPLAYTOGGLEINTEXT" => Some(Key::DISPLAY_TOGGLE_INT_EXT),
        "PRIVACYSCREENTOGGLE" => Some(Key::PRIVACY_SCREEN_TOGGLE),
        "KEYBOARDBACKLIGHTTOGGLE" | "KBDBACKLIGHT" => Some(Key::KEYBOARD_BACKLIGHT_TOGGLE),
        // Browser keys
        "BROWSERBACK" => Some(Key::BROWSER_BACK),
        "BROWSERFAVORITES" | "BOOKMARKS" => Some(Key::BROWSER_FAVORITES),
        "BROWSERFORWARD" => Some(Key::BROWSER_FORWARD),
        "BROWSERHOME" => Some(Key::BROWSER_HOME),
        "BROWSERREFRESH" => Some(Key::BROWSER_REFRESH),
        "BROWSERSEARCH" => Some(Key::BROWSER_SEARCH),
        "BROWSERSTOP" => Some(Key::BROWSER_STOP),
        // Extended media keys
        "MEDIAPLAY" => Some(Key::MEDIA_PLAY),
        "MEDIAPAUSE" => Some(Key::MEDIA_PAUSE),
        "MEDIAFASTFORWARD" | "MEDIAFF" => Some(Key::MEDIA_FAST_FORWARD),
        "MEDIAREWIND" | "MEDIARW" => Some(Key::MEDIA_REWIND),
        "MEDIARECORD" => Some(Key::MEDIA_RECORD),
        "MEDIASELECT" => Some(Key::MEDIA_SELECT),
        "MICROPHONEMUTETOGGLE" | "MICMUTE" => Some(Key::MICROPHONE_MUTE_TOGGLE),
        // Fn keys
        "FN" => Some(Key::FN),
        "FNLOCK" => Some(Key::FN_LOCK),
        // Clipboard / editing
        "COPY" => Some(Key::COPY),
        "CUT" => Some(Key::CUT),
        "PASTE" => Some(Key::PASTE),
        "UNDO" => Some(Key::UNDO),
        "FIND" => Some(Key::FIND),
        "HELP" => Some(Key::HELP),
        "OPEN" => Some(Key::OPEN),
        "SELECT" => Some(Key::SELECT),
        "AGAIN" | "REDO" => Some(Key::AGAIN),
        "PROPS" | "PROPERTIES" => Some(Key::PROPS),
        "ABORT" | "CANCEL" => Some(Key::ABORT),
        "RESUME" => Some(Key::RESUME),
        "SUSPEND" => Some(Key::SUSPEND),
        // Legacy / niche
        "HYPER" => Some(Key::HYPER),
        "SUPERKEY" => Some(Key::SUPER_KEY),
        "TURBO" => Some(Key::TURBO),
        // CJK input keys
        "CONVERT" | "HENKAN" => Some(Key::CONVERT),
        "NONCONVERT" | "MUHENKAN" => Some(Key::NON_CONVERT),
        "KANAMODE" | "KANA" => Some(Key::KANA_MODE),
        "HIRAGANA" => Some(Key::HIRAGANA),
        "KATAKANA" => Some(Key::KATAKANA),
        "KEYBOARDLAYOUTSELECT" => Some(Key::KEYBOARD_LAYOUT_SELECT),
        // Language keys
        "LANG1" | "HANGUL" => Some(Key::LANG1),
        "LANG2" | "HANJA" => Some(Key::LANG2),
        "LANG3" => Some(Key::LANG3),
        "LANG4" => Some(Key::LANG4),
        "LANG5" => Some(Key::LANG5),
        // International keys
        "INTLBACKSLASH" => Some(Key::INTL_BACKSLASH),
        "INTLRO" => Some(Key::INTL_RO),
        "INTLYEN" => Some(Key::INTL_YEN),
        // App launch keys
        "LAUNCHAPP1" => Some(Key::LAUNCH_APP1),
        "LAUNCHAPP2" | "CALCULATOR" => Some(Key::LAUNCH_APP2),
        "LAUNCHASSISTANT" => Some(Key::LAUNCH_ASSISTANT),
        "LAUNCHCONTROLPANEL" => Some(Key::LAUNCH_CONTROL_PANEL),
        "LAUNCHMAIL" | "MAIL" => Some(Key::LAUNCH_MAIL),
        "LAUNCHSCREENSAVER" => Some(Key::LAUNCH_SCREEN_SAVER),
        // Mail keys
        "MAILFORWARD" => Some(Key::MAIL_FORWARD),
        "MAILREPLY" => Some(Key::MAIL_REPLY),
        "MAILSEND" => Some(Key::MAIL_SEND),
        // Extended numpad
        "NUMPADEQUAL" | "KPEQUAL" => Some(Key::NUMPAD_EQUAL),
        "NUMPADCOMMA" | "KPCOMMA" => Some(Key::NUMPAD_COMMA),
        "NUMPADBACKSPACE" | "KPBACKSPACE" => Some(Key::NUMPAD_BACKSPACE),
        "NUMPADCLEAR" | "KPCLEAR" => Some(Key::NUMPAD_CLEAR),
        "NUMPADCLEARENTRY" | "KPCLEARENTRY" => Some(Key::NUMPAD_CLEAR_ENTRY),
        "NUMPADHASH" | "KPHASH" => Some(Key::NUMPAD_HASH),
        "NUMPADMEMORYADD" | "KPMEMORYADD" => Some(Key::NUMPAD_MEMORY_ADD),
        "NUMPADMEMORYCLEAR" | "KPMEMORYCLEAR" => Some(Key::NUMPAD_MEMORY_CLEAR),
        "NUMPADMEMORYRECALL" | "KPMEMORYRECALL" => Some(Key::NUMPAD_MEMORY_RECALL),
        "NUMPADMEMORYSTORE" | "KPMEMORYSTORE" => Some(Key::NUMPAD_MEMORY_STORE),
        "NUMPADMEMORYSUBTRACT" | "KPMEMORYSUBTRACT" => Some(Key::NUMPAD_MEMORY_SUBTRACT),
        "NUMPADPARENLEFT" | "KPPARENLEFT" => Some(Key::NUMPAD_PAREN_LEFT),
        "NUMPADPARENRIGHT" | "KPPARENRIGHT" => Some(Key::NUMPAD_PAREN_RIGHT),
        "NUMPADSTAR" | "KPSTAR" => Some(Key::NUMPAD_STAR),
        // Misc system/app keys
        "SELECTTASK" => Some(Key::SELECT_TASK),
        "SHOWALLWINDOWS" | "EXPOSE" => Some(Key::SHOW_ALL_WINDOWS),
        "ZOOMTOGGLE" => Some(Key::ZOOM_TOGGLE),
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

/// A canonical modifier key (Ctrl, Shift, Alt, Super).
///
/// Left and right physical variants are canonicalized — both `ControlLeft`
/// and `ControlRight` map to `Modifier::Ctrl`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Modifier {
    /// The Control modifier (left or right).
    Ctrl,
    /// The Shift modifier (left or right).
    Shift,
    /// The Alt modifier (left or right).
    Alt,
    /// The Super/Meta/Win modifier (left or right).
    Super,
}

impl Modifier {
    /// Human-readable name for this modifier.
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

    /// Return the left and right physical [`Key`] variants for this modifier.
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

/// A key combined with zero or more modifiers.
///
/// Hotkeys are the matching unit — `"Ctrl+C"`, `"Shift+F5"`, or just `"Escape"`.
/// Parse from strings with [`str::parse`] or build programmatically with
/// [`Hotkey::new`] and [`Hotkey::modifier`].
///
/// ```
/// use kbd::key::{Hotkey, Key, Modifier};
///
/// // From a string
/// let hotkey: Hotkey = "Ctrl+Shift+A".parse().unwrap();
///
/// // Programmatic
/// let hotkey = Hotkey::new(Key::A).modifier(Modifier::Ctrl).modifier(Modifier::Shift);
/// ```
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

    /// Create a hotkey from a key and a list of modifiers. Modifiers are
    /// sorted and deduplicated.
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

    /// The non-modifier key in this hotkey.
    #[must_use]
    pub fn key(&self) -> Key {
        self.key
    }

    /// The modifiers required for this hotkey (sorted, deduplicated).
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

/// A multi-step hotkey sequence like `"Ctrl+K, Ctrl+C"`.
///
/// Sequences are comma-separated hotkeys. Each step must be pressed
/// in order for the sequence to match.
///
/// ```
/// use kbd::key::HotkeySequence;
///
/// let seq: HotkeySequence = "Ctrl+K, Ctrl+C".parse().unwrap();
/// assert_eq!(seq.steps().len(), 2);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct HotkeySequence {
    steps: Vec<Hotkey>,
}

impl HotkeySequence {
    /// Create a sequence from a non-empty list of hotkeys.
    pub fn new(steps: Vec<Hotkey>) -> Result<Self, ParseHotkeyError> {
        if steps.is_empty() {
            return Err(ParseHotkeyError::Empty);
        }

        Ok(Self { steps })
    }

    /// The individual hotkey steps in this sequence.
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

/// Error returned when parsing a hotkey or key from a string fails.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ParseHotkeyError {
    /// The input string was empty.
    #[error("hotkey string is empty")]
    Empty,
    /// A segment between `+` separators was empty (e.g., `"Ctrl++A"`).
    #[error("hotkey contains an empty token")]
    EmptySegment,
    /// A token could not be recognized as a key or modifier.
    #[error("unknown hotkey token: {0}")]
    UnknownToken(String),
    /// The hotkey contained only modifiers with no trigger key.
    #[error("hotkey is missing a non-modifier key")]
    MissingKey,
    /// The hotkey contained more than one non-modifier key.
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
        // All Code variants now have Key constants, so test with Unidentified
        let key = Key::UNIDENTIFIED;
        assert_eq!(key.to_string(), "Unidentified");
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

    #[test]
    fn extended_function_keys_exist() {
        assert_eq!(Key::F25, Key(Code::F25));
        assert_eq!(Key::F35, Key(Code::F35));
    }

    #[test]
    fn extended_function_keys_parse() {
        assert_eq!("F25".parse::<Key>().unwrap(), Key::F25);
        assert_eq!("F30".parse::<Key>().unwrap(), Key::F30);
        assert_eq!("F35".parse::<Key>().unwrap(), Key::F35);
    }

    #[test]
    fn browser_keys_exist_and_round_trip() {
        let browser_keys = [
            Key::BROWSER_BACK,
            Key::BROWSER_FORWARD,
            Key::BROWSER_HOME,
            Key::BROWSER_REFRESH,
            Key::BROWSER_SEARCH,
            Key::BROWSER_STOP,
            Key::BROWSER_FAVORITES,
        ];
        for key in browser_keys {
            let s = key.to_string();
            let parsed: Key = s.parse().unwrap();
            assert_eq!(parsed, key, "round-trip failed for {s}");
        }
    }

    #[test]
    fn extended_media_keys_exist_and_round_trip() {
        let media_keys = [
            Key::MEDIA_PLAY,
            Key::MEDIA_PAUSE,
            Key::MEDIA_FAST_FORWARD,
            Key::MEDIA_REWIND,
            Key::MEDIA_RECORD,
            Key::MEDIA_SELECT,
        ];
        for key in media_keys {
            let s = key.to_string();
            let parsed: Key = s.parse().unwrap();
            assert_eq!(parsed, key, "round-trip failed for {s}");
        }
    }

    #[test]
    fn system_keys_exist_and_round_trip() {
        let system_keys = [
            Key::SLEEP,
            Key::WAKE_UP,
            Key::EJECT,
            Key::BRIGHTNESS_DOWN,
            Key::BRIGHTNESS_UP,
        ];
        for key in system_keys {
            let s = key.to_string();
            let parsed: Key = s.parse().unwrap();
            assert_eq!(parsed, key, "round-trip failed for {s}");
        }
    }

    #[test]
    fn clipboard_keys_exist_and_round_trip() {
        let clipboard_keys = [Key::COPY, Key::CUT, Key::PASTE];
        for key in clipboard_keys {
            let s = key.to_string();
            let parsed: Key = s.parse().unwrap();
            assert_eq!(parsed, key, "round-trip failed for {s}");
        }
    }

    #[test]
    fn international_keys_exist_and_round_trip() {
        let intl_keys = [Key::INTL_BACKSLASH, Key::INTL_RO, Key::INTL_YEN];
        for key in intl_keys {
            let s = key.to_string();
            let parsed: Key = s.parse().unwrap();
            assert_eq!(parsed, key, "round-trip failed for {s}");
        }
    }

    #[test]
    fn cjk_keys_exist() {
        assert_eq!(Key::CONVERT, Key(Code::Convert));
        assert_eq!(Key::NON_CONVERT, Key(Code::NonConvert));
        assert_eq!(Key::KANA_MODE, Key(Code::KanaMode));
        assert_eq!(Key::HIRAGANA, Key(Code::Hiragana));
        assert_eq!(Key::KATAKANA, Key(Code::Katakana));
    }

    #[test]
    fn extended_numpad_keys_exist() {
        assert_eq!(Key::NUMPAD_EQUAL, Key(Code::NumpadEqual));
        assert_eq!(Key::NUMPAD_COMMA, Key(Code::NumpadComma));
        assert_eq!(Key::NUMPAD_BACKSPACE, Key(Code::NumpadBackspace));
        assert_eq!(Key::NUMPAD_PAREN_LEFT, Key(Code::NumpadParenLeft));
        assert_eq!(Key::NUMPAD_PAREN_RIGHT, Key(Code::NumpadParenRight));
    }

    #[test]
    fn fn_keys_exist() {
        assert_eq!(Key::FN, Key(Code::Fn));
        assert_eq!(Key::FN_LOCK, Key(Code::FnLock));
    }

    #[test]
    fn launch_keys_exist() {
        assert_eq!(Key::LAUNCH_APP1, Key(Code::LaunchApp1));
        assert_eq!(Key::LAUNCH_APP2, Key(Code::LaunchApp2));
        assert_eq!(Key::LAUNCH_MAIL, Key(Code::LaunchMail));
    }

    #[test]
    #[allow(deprecated)]
    fn legacy_keys_exist() {
        assert_eq!(Key::HYPER, Key(Code::Hyper));
        assert_eq!(Key::AGAIN, Key(Code::Again));
        assert_eq!(Key::PROPS, Key(Code::Props));
        assert_eq!(Key::UNDO, Key(Code::Undo));
        assert_eq!(Key::FIND, Key(Code::Find));
        assert_eq!(Key::HELP, Key(Code::Help));
    }

    #[test]
    fn parse_aliases_for_new_keys() {
        // Browser keys
        assert_eq!("BrowserBack".parse::<Key>().unwrap(), Key::BROWSER_BACK);
        // Clipboard
        assert_eq!("Copy".parse::<Key>().unwrap(), Key::COPY);
        assert_eq!("Cut".parse::<Key>().unwrap(), Key::CUT);
        assert_eq!("Paste".parse::<Key>().unwrap(), Key::PASTE);
        // System
        assert_eq!("Sleep".parse::<Key>().unwrap(), Key::SLEEP);
        assert_eq!("WakeUp".parse::<Key>().unwrap(), Key::WAKE_UP);
        assert_eq!("Eject".parse::<Key>().unwrap(), Key::EJECT);
        // Media
        assert_eq!("MediaPlay".parse::<Key>().unwrap(), Key::MEDIA_PLAY);
        assert_eq!("MediaPause".parse::<Key>().unwrap(), Key::MEDIA_PAUSE);
        // Fn
        assert_eq!("Fn".parse::<Key>().unwrap(), Key::FN);
        assert_eq!("FnLock".parse::<Key>().unwrap(), Key::FN_LOCK);
    }

    #[test]
    fn all_new_constants_round_trip_display_parse() {
        let new_keys = [
            Key::F25,
            Key::F26,
            Key::F27,
            Key::F28,
            Key::F29,
            Key::F30,
            Key::F31,
            Key::F32,
            Key::F33,
            Key::F34,
            Key::F35,
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
            Key::SLEEP,
            Key::WAKE_UP,
            Key::EJECT,
            Key::BRIGHTNESS_DOWN,
            Key::BRIGHTNESS_UP,
            Key::FN,
            Key::FN_LOCK,
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
            Key::RESUME,
            Key::SUSPEND,
            Key::HYPER,
            Key::SUPER_KEY,
            Key::TURBO,
            Key::CONVERT,
            Key::NON_CONVERT,
            Key::KANA_MODE,
            Key::HIRAGANA,
            Key::KATAKANA,
            Key::LANG1,
            Key::LANG2,
            Key::LANG3,
            Key::LANG4,
            Key::LANG5,
            Key::INTL_BACKSLASH,
            Key::INTL_RO,
            Key::INTL_YEN,
            Key::LAUNCH_APP1,
            Key::LAUNCH_APP2,
            Key::LAUNCH_MAIL,
            Key::LAUNCH_ASSISTANT,
            Key::LAUNCH_CONTROL_PANEL,
            Key::LAUNCH_SCREEN_SAVER,
            Key::MAIL_FORWARD,
            Key::MAIL_REPLY,
            Key::MAIL_SEND,
            Key::NUMPAD_EQUAL,
            Key::NUMPAD_COMMA,
            Key::NUMPAD_BACKSPACE,
            Key::NUMPAD_CLEAR,
            Key::NUMPAD_CLEAR_ENTRY,
            Key::NUMPAD_HASH,
            Key::NUMPAD_MEMORY_ADD,
            Key::NUMPAD_MEMORY_CLEAR,
            Key::NUMPAD_MEMORY_RECALL,
            Key::NUMPAD_MEMORY_STORE,
            Key::NUMPAD_MEMORY_SUBTRACT,
            Key::NUMPAD_PAREN_LEFT,
            Key::NUMPAD_PAREN_RIGHT,
            Key::NUMPAD_STAR,
            Key::DISPLAY_TOGGLE_INT_EXT,
            Key::KEYBOARD_BACKLIGHT_TOGGLE,
            Key::KEYBOARD_LAYOUT_SELECT,
            Key::PRIVACY_SCREEN_TOGGLE,
            Key::SELECT_TASK,
            Key::SHOW_ALL_WINDOWS,
            Key::ZOOM_TOGGLE,
        ];
        for key in new_keys {
            let s = key.to_string();
            let parsed: Key = s.parse().unwrap();
            assert_eq!(parsed, key, "round-trip failed for {s}");
        }
    }
}
