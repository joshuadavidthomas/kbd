use kbd::observation::LogicalKey;
use kbd::observation::LogicalKeyValue;
use kbd::observation::NamedKey;
use xkbcommon::xkb;
use xkbcommon::xkb::keysyms;

// Named symbols precede Unicode conversion: e.g. Return is Enter, not control
// text. Keep the table explicit; unknown non-Unicode symbols remain absent.
#[allow(clippy::too_many_lines)]
pub(super) fn from_keysym(symbol: xkb::Keysym) -> Option<LogicalKey> {
    macro_rules! named {
        ($($source:ident => $target:ident),* $(,)?) => {
            match symbol.raw() {
                $(keysyms::$source => return Some(NamedKey::$target.into()),)*
                _ => {}
            }
        };
    }
    named!(
        KEY_Return => Enter,
        KEY_KP_Enter => Enter,
        KEY_Tab => Tab,
        KEY_KP_Tab => Tab,
        KEY_ISO_Left_Tab => Tab,
        KEY_BackSpace => Backspace,
        KEY_Escape => Escape,
        KEY_Delete => Delete,
        KEY_KP_Delete => Delete,
        KEY_Insert => Insert,
        KEY_KP_Insert => Insert,
        KEY_Home => Home,
        KEY_KP_Home => Home,
        KEY_End => End,
        KEY_KP_End => End,
        KEY_Left => ArrowLeft,
        KEY_KP_Left => ArrowLeft,
        KEY_Right => ArrowRight,
        KEY_KP_Right => ArrowRight,
        KEY_Up => ArrowUp,
        KEY_KP_Up => ArrowUp,
        KEY_Down => ArrowDown,
        KEY_KP_Down => ArrowDown,
        KEY_Page_Up => PageUp,
        KEY_KP_Page_Up => PageUp,
        KEY_Page_Down => PageDown,
        KEY_KP_Page_Down => PageDown,
        KEY_Clear => Clear,
        KEY_KP_Begin => Clear,
        KEY_Pause => Pause,
        KEY_Print => PrintScreen,
        KEY_Menu => ContextMenu,
        KEY_Help => Help,
        KEY_Cancel => Cancel,
        KEY_Undo => Undo,
        KEY_Redo => Redo,
        KEY_Find => Find,
        KEY_Select => Select,
        KEY_Execute => Execute,
        KEY_Shift_L => Shift,
        KEY_Shift_R => Shift,
        KEY_Control_L => Control,
        KEY_Control_R => Control,
        KEY_Alt_L => Alt,
        KEY_Alt_R => Alt,
        KEY_Meta_L => Meta,
        KEY_Meta_R => Meta,
        KEY_Super_L => Meta,
        KEY_Super_R => Meta,
        KEY_ISO_Level3_Shift => AltGraph,
        KEY_ISO_Level3_Latch => AltGraph,
        KEY_ISO_Level3_Lock => AltGraph,
        KEY_Mode_switch => ModeChange,
        KEY_Caps_Lock => CapsLock,
        KEY_Num_Lock => NumLock,
        KEY_Scroll_Lock => ScrollLock,
        KEY_F1 => F1,
        KEY_KP_F1 => F1,
        KEY_F2 => F2,
        KEY_KP_F2 => F2,
        KEY_F3 => F3,
        KEY_KP_F3 => F3,
        KEY_F4 => F4,
        KEY_KP_F4 => F4,
        KEY_F5 => F5,
        KEY_F6 => F6,
        KEY_F7 => F7,
        KEY_F8 => F8,
        KEY_F9 => F9,
        KEY_F10 => F10,
        KEY_F11 => F11,
        KEY_F12 => F12,
        KEY_F13 => F13,
        KEY_F14 => F14,
        KEY_F15 => F15,
        KEY_F16 => F16,
        KEY_F17 => F17,
        KEY_F18 => F18,
        KEY_F19 => F19,
        KEY_F20 => F20,
        KEY_F21 => F21,
        KEY_F22 => F22,
        KEY_F23 => F23,
        KEY_F24 => F24,
        KEY_F25 => F25,
        KEY_F26 => F26,
        KEY_F27 => F27,
        KEY_F28 => F28,
        KEY_F29 => F29,
        KEY_F30 => F30,
        KEY_F31 => F31,
        KEY_F32 => F32,
        KEY_F33 => F33,
        KEY_F34 => F34,
        KEY_F35 => F35,
        KEY_XF86AudioLowerVolume => AudioVolumeDown,
        KEY_XF86AudioRaiseVolume => AudioVolumeUp,
        KEY_XF86AudioMute => AudioVolumeMute,
        KEY_XF86AudioPlay => MediaPlayPause,
        KEY_XF86AudioPause => MediaPause,
        KEY_XF86AudioStop => MediaStop,
        KEY_XF86AudioPrev => MediaTrackPrevious,
        KEY_XF86AudioNext => MediaTrackNext,
        KEY_XF86AudioRecord => MediaRecord,
        KEY_XF86Back => BrowserBack,
        KEY_XF86Forward => BrowserForward,
        KEY_XF86Refresh => BrowserRefresh,
        KEY_XF86HomePage => BrowserHome,
        KEY_XF86Search => BrowserSearch,
        KEY_XF86Mail => LaunchMail,
        KEY_XF86MonBrightnessUp => BrightnessUp,
        KEY_XF86MonBrightnessDown => BrightnessDown,
        KEY_XF86PowerOff => PowerOff,
        KEY_XF86Sleep => Standby,
        KEY_XF86WakeUp => WakeUp,
    );
    match symbol.raw() {
        // Contiguous dead-symbol ranges defined by XKB, without composing text.
        keysyms::KEY_dead_grave..=keysyms::KEY_dead_currency
        | keysyms::KEY_dead_a..=keysyms::KEY_dead_greek
        | keysyms::KEY_dead_lowline..=keysyms::KEY_dead_longsolidusoverlay => {
            Some(NamedKey::Dead.into())
        }
        _ => {
            let text = xkb::keysym_to_utf8(symbol);
            (!text.is_empty() && !text.chars().any(char::is_control))
                .then(|| LogicalKeyValue::Character(text).into())
        }
    }
}
