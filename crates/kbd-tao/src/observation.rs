use kbd::key_state::KeyTransition;
use kbd::observation::KeyboardObservation;
use kbd::observation::LogicalKey;
use kbd::observation::LogicalKeyValue;
use kbd::observation::NamedKey;
use tao::event::ElementState;
use tao::keyboard::Key;
use tao::keyboard::KeyCode;
use tao::keyboard::ModifiersState;

use crate::TaoKeyExt;
use crate::TaoModifiersExt;

pub(super) fn from_parts(
    physical: KeyCode,
    logical: &Key<'_>,
    modifiers: ModifiersState,
    state: ElementState,
    repeat: bool,
) -> Option<KeyboardObservation> {
    Some(KeyboardObservation {
        physical: match physical {
            KeyCode::Plus => None,
            _ => physical.to_key(),
        },
        logical: logical_key(logical),
        modifiers: modifiers.to_modifiers(),
        transition: match (state, repeat) {
            (ElementState::Released, _) => KeyTransition::Release,
            (ElementState::Pressed, true) => KeyTransition::Repeat,
            (ElementState::Pressed, false) => KeyTransition::Press,
            _ => return None,
        },
    })
}

#[allow(deprecated)] // Preserve explicitly reported legacy Hyper.
#[allow(clippy::too_many_lines)] // Complete named-key correspondence table.
fn logical_key(key: &Key<'_>) -> Option<LogicalKey> {
    macro_rules! named {
        ($($name:ident),* $(,)?) => {
            match key {
                $(Key::$name => Some(NamedKey::$name.into()),)*
                Key::Character(text) => Some(LogicalKeyValue::Character((*text).into()).into()),
                Key::Dead(_) => Some(NamedKey::Dead.into()),
                Key::Super => Some(NamedKey::Meta.into()),
                Key::Space => Some(LogicalKeyValue::Character(" ".into()).into()),
                _ => None,
            }
        };
    }
    named!(
        Alt,
        AltGraph,
        CapsLock,
        Control,
        Fn,
        FnLock,
        NumLock,
        ScrollLock,
        Shift,
        Symbol,
        SymbolLock,
        Hyper,
        Enter,
        Tab,
        ArrowDown,
        ArrowLeft,
        ArrowRight,
        ArrowUp,
        End,
        Home,
        PageDown,
        PageUp,
        Backspace,
        Clear,
        Copy,
        CrSel,
        Cut,
        Delete,
        EraseEof,
        ExSel,
        Insert,
        Paste,
        Redo,
        Undo,
        Accept,
        Again,
        Attn,
        Cancel,
        ContextMenu,
        Escape,
        Execute,
        Find,
        Help,
        Pause,
        Play,
        Props,
        Select,
        ZoomIn,
        ZoomOut,
        BrightnessDown,
        BrightnessUp,
        Eject,
        LogOff,
        Power,
        PowerOff,
        PrintScreen,
        Hibernate,
        Standby,
        WakeUp,
        AllCandidates,
        Alphanumeric,
        CodeInput,
        Compose,
        Convert,
        FinalMode,
        GroupFirst,
        GroupLast,
        GroupNext,
        GroupPrevious,
        ModeChange,
        NextCandidate,
        NonConvert,
        PreviousCandidate,
        Process,
        SingleCandidate,
        HangulMode,
        HanjaMode,
        JunjaMode,
        Eisu,
        Hankaku,
        Hiragana,
        HiraganaKatakana,
        KanaMode,
        KanjiMode,
        Katakana,
        Romaji,
        Zenkaku,
        ZenkakuHankaku,
        Soft1,
        Soft2,
        Soft3,
        Soft4,
        ChannelDown,
        ChannelUp,
        Close,
        MailForward,
        MailReply,
        MailSend,
        MediaClose,
        MediaFastForward,
        MediaPause,
        MediaPlay,
        MediaPlayPause,
        MediaRecord,
        MediaRewind,
        MediaStop,
        MediaTrackNext,
        MediaTrackPrevious,
        New,
        Open,
        Print,
        Save,
        SpellCheck,
        Key11,
        Key12,
        AudioBalanceLeft,
        AudioBalanceRight,
        AudioBassBoostDown,
        AudioBassBoostToggle,
        AudioBassBoostUp,
        AudioFaderFront,
        AudioFaderRear,
        AudioSurroundModeNext,
        AudioTrebleDown,
        AudioTrebleUp,
        AudioVolumeDown,
        AudioVolumeUp,
        AudioVolumeMute,
        MicrophoneToggle,
        MicrophoneVolumeDown,
        MicrophoneVolumeUp,
        MicrophoneVolumeMute,
        SpeechCorrectionList,
        SpeechInputToggle,
        LaunchApplication1,
        LaunchApplication2,
        LaunchCalendar,
        LaunchContacts,
        LaunchMail,
        LaunchMediaPlayer,
        LaunchMusicPlayer,
        LaunchPhone,
        LaunchScreenSaver,
        LaunchSpreadsheet,
        LaunchWebBrowser,
        LaunchWebCam,
        LaunchWordProcessor,
        BrowserBack,
        BrowserFavorites,
        BrowserForward,
        BrowserHome,
        BrowserRefresh,
        BrowserSearch,
        BrowserStop,
        AppSwitch,
        Call,
        Camera,
        CameraFocus,
        EndCall,
        GoBack,
        GoHome,
        HeadsetHook,
        LastNumberRedial,
        Notification,
        MannerMode,
        VoiceDial,
        TV,
        TV3DMode,
        TVAntennaCable,
        TVAudioDescription,
        TVAudioDescriptionMixDown,
        TVAudioDescriptionMixUp,
        TVContentsMenu,
        TVDataService,
        TVInput,
        TVInputComponent1,
        TVInputComponent2,
        TVInputComposite1,
        TVInputComposite2,
        TVInputHDMI1,
        TVInputHDMI2,
        TVInputHDMI3,
        TVInputHDMI4,
        TVInputVGA1,
        TVMediaContext,
        TVNetwork,
        TVNumberEntry,
        TVPower,
        TVRadioService,
        TVSatellite,
        TVSatelliteBS,
        TVSatelliteCS,
        TVSatelliteToggle,
        TVTerrestrialAnalog,
        TVTerrestrialDigital,
        TVTimer,
        AVRInput,
        AVRPower,
        ColorF0Red,
        ColorF1Green,
        ColorF2Yellow,
        ColorF3Blue,
        ColorF4Grey,
        ColorF5Brown,
        ClosedCaptionToggle,
        Dimmer,
        DisplaySwap,
        DVR,
        Exit,
        FavoriteClear0,
        FavoriteClear1,
        FavoriteClear2,
        FavoriteClear3,
        FavoriteRecall0,
        FavoriteRecall1,
        FavoriteRecall2,
        FavoriteRecall3,
        FavoriteStore0,
        FavoriteStore1,
        FavoriteStore2,
        FavoriteStore3,
        Guide,
        GuideNextDay,
        GuidePreviousDay,
        Info,
        InstantReplay,
        Link,
        ListProgram,
        LiveContent,
        Lock,
        MediaApps,
        MediaAudioTrack,
        MediaLast,
        MediaSkipBackward,
        MediaSkipForward,
        MediaStepBackward,
        MediaStepForward,
        MediaTopMenu,
        NavigateIn,
        NavigateNext,
        NavigateOut,
        NavigatePrevious,
        NextFavoriteChannel,
        NextUserProfile,
        OnDemand,
        Pairing,
        PinPDown,
        PinPMove,
        PinPToggle,
        PinPUp,
        PlaySpeedDown,
        PlaySpeedReset,
        PlaySpeedUp,
        RandomToggle,
        RcLowBattery,
        RecordSpeedNext,
        RfBypass,
        ScanChannelsToggle,
        ScreenModeNext,
        Settings,
        SplitScreenToggle,
        STBInput,
        STBPower,
        Subtitle,
        Teletext,
        VideoModeNext,
        Wink,
        ZoomToggle,
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
        F25,
        F26,
        F27,
        F28,
        F29,
        F30,
        F31,
        F32,
        F33,
        F34,
        F35,
    )
}

#[cfg(test)]
mod tests {
    use kbd::hotkey::ModifierSet;
    use kbd::key::Key as Physical;
    use tao::keyboard::NativeKeyCode;

    use super::*;

    #[test]
    fn exact_strings_and_dead_keys_are_not_physical_positions() {
        for text in ["a", "A", "@", "é", "e\u{301}", "👩‍💻", "Enter"] {
            let source = Key::Character(text);
            let observed = from_parts(
                KeyCode::KeyQ,
                &source,
                ModifiersState::SHIFT,
                ElementState::Pressed,
                false,
            )
            .unwrap();
            assert_eq!(observed.physical, Some(Physical::Q));
            assert_eq!(
                observed.logical,
                Some(LogicalKeyValue::Character(text.into()).into())
            );
            assert_eq!(observed.modifiers, ModifierSet::SHIFT);
        }
        for accent in [Some('´'), None] {
            let source = Key::Dead(accent);
            assert_eq!(logical_key(&source), Some(NamedKey::Dead.into()));
        }
        for (source, expected) in [
            (Key::Enter, NamedKey::Enter),
            (Key::Super, NamedKey::Meta),
            (Key::AltGraph, NamedKey::AltGraph),
            (Key::Fn, NamedKey::Fn),
            (Key::F35, NamedKey::F35),
        ] {
            assert_eq!(logical_key(&source), Some(expected.into()));
        }
        assert_eq!(
            logical_key(&Key::Space),
            Some(LogicalKeyValue::Character(" ".into()).into())
        );
    }

    #[test]
    fn plus_and_unknown_physical_are_not_equal() {
        for physical in [
            KeyCode::Plus,
            KeyCode::Unidentified(NativeKeyCode::Unidentified),
        ] {
            let observed = from_parts(
                physical,
                &Key::Character("+"),
                ModifiersState::empty(),
                ElementState::Pressed,
                false,
            )
            .unwrap();
            assert_eq!(observed.physical, None);
            assert_eq!(
                observed.logical,
                Some(LogicalKeyValue::Character("+".into()).into())
            );
        }
        assert_eq!(
            from_parts(
                KeyCode::Equal,
                &Key::Character("+"),
                ModifiersState::SHIFT,
                ElementState::Pressed,
                false
            )
            .unwrap()
            .physical,
            Some(Physical::EQUAL)
        );
        // Compatibility deliberately retains the old alias.
        assert_eq!(KeyCode::Plus.to_key(), Some(Physical::EQUAL));
    }

    #[test]
    fn transitions_and_modifier_triggers() {
        for (state, repeat, expected) in [
            (ElementState::Pressed, false, KeyTransition::Press),
            (ElementState::Pressed, true, KeyTransition::Repeat),
            (ElementState::Released, true, KeyTransition::Release),
        ] {
            let observed = from_parts(
                KeyCode::ControlRight,
                &Key::Control,
                ModifiersState::CONTROL,
                state,
                repeat,
            )
            .unwrap();
            assert_eq!(observed.physical, Some(Physical::CONTROL_RIGHT));
            assert_eq!(observed.transition, expected);
            assert_eq!(observed.modifiers, ModifierSet::CTRL);
        }
        let observed = from_parts(
            KeyCode::NumpadEnter,
            &Key::Unidentified(NativeKeyCode::Unidentified),
            ModifiersState::empty(),
            ElementState::Released,
            false,
        )
        .unwrap();
        assert_eq!(observed.physical, Some(Physical::NUMPAD_ENTER));
        assert_eq!(observed.logical, None);
    }
}
