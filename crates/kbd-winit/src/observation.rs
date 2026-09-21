use kbd::hotkey::ModifierSet;
use kbd::key_state::KeyTransition;
use kbd::observation::KeyboardObservation;
use kbd::observation::LogicalKey;
use kbd::observation::LogicalKeyValue;
use kbd::observation::ModifierObservation;
use kbd::observation::ModifierState;
use kbd::observation::NamedKey;
use winit::event::ElementState;
use winit::keyboard::Key;
use winit::keyboard::KeyCode;
use winit::keyboard::ModifiersState;
use winit::keyboard::PhysicalKey;

use crate::WinitKeyExt;
use crate::WinitModifiersExt;

/// Construct a [`KeyboardObservation`] from winit's independent input facts.
///
/// This trait is sealed and cannot be implemented outside this crate.
pub trait WinitObservationExt: crate::private::Sealed + Sized {
    /// Preserve the supplied identities, modifier state, and transition.
    /// Text, IME, location, and native unidentified-key details stay with the caller.
    #[must_use]
    fn from_winit(
        physical: PhysicalKey,
        logical: &Key,
        modifiers: ModifiersState,
        state: ElementState,
        repeat: bool,
    ) -> Self;
}

impl WinitObservationExt for KeyboardObservation {
    fn from_winit(
        physical: PhysicalKey,
        logical: &Key,
        modifiers: ModifiersState,
        state: ElementState,
        repeat: bool,
    ) -> KeyboardObservation {
        KeyboardObservation {
            // The legacy mapping assigns this unsided code to MetaLeft.
            physical: match physical {
                PhysicalKey::Code(KeyCode::Meta) => None,
                _ => physical.to_key(),
            },
            logical: logical_key(logical),
            modifiers: modifiers.to_modifiers(),
            modifier_observation: Some(ModifierObservation {
                physical: ModifierState::new(modifiers.to_modifiers(), ModifierSet::STANDARD),
                logical: None,
            }),
            transition: match (state, repeat) {
                (ElementState::Released, _) => KeyTransition::Release,
                (ElementState::Pressed, true) => KeyTransition::Repeat,
                (ElementState::Pressed, false) => KeyTransition::Press,
            },
        }
    }
}

fn logical_key(key: &Key) -> Option<LogicalKey> {
    match key {
        Key::Character(text) => Some(LogicalKeyValue::Character(text.to_string()).into()),
        Key::Dead(_) => Some(NamedKey::Dead.into()),
        Key::Named(named) => named_key(*named),
        Key::Unidentified(_) => None,
    }
}

#[allow(deprecated)] // Preserve explicitly reported legacy Hyper.
#[allow(clippy::too_many_lines)] // Complete named-key correspondence table.
fn named_key(key: winit::keyboard::NamedKey) -> Option<LogicalKey> {
    use winit::keyboard::NamedKey as Source;

    // Explicit W3C variant correspondence, not parsing Debug or character text.
    macro_rules! named {
        ($($name:ident),* $(,)?) => {
            match key {
                $(Source::$name => Some(NamedKey::$name.into()),)*
                Source::Super | Source::Meta => Some(NamedKey::Meta.into()),
                Source::Space => Some(LogicalKeyValue::Character(" ".into()).into()),
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
    use winit::keyboard::NamedKey as Named;
    use winit::keyboard::NativeKeyCode;

    use super::*;

    #[test]
    fn characters_and_physical_positions_are_independent() {
        for text in ["a", "A", "@", "é", "e\u{301}", "👩‍💻", "Enter"] {
            let logical = Key::Character(text.into());
            let observed = KeyboardObservation::from_winit(
                PhysicalKey::Code(KeyCode::KeyQ),
                &logical,
                ModifiersState::SHIFT,
                ElementState::Pressed,
                false,
            );
            assert_eq!(observed.physical, Some(Physical::Q));
            assert_eq!(
                observed.logical,
                Some(LogicalKeyValue::Character(text.into()).into())
            );
            assert_eq!(observed.modifiers, ModifierSet::SHIFT);
        }
    }

    #[test]
    fn unknown_physical_and_dead_details_do_not_fabricate_identity() {
        let unknown = PhysicalKey::Unidentified(NativeKeyCode::Xkb(248));
        let event = KeyboardObservation::from_winit(
            unknown,
            &Key::Character("λ".into()),
            ModifiersState::empty(),
            ElementState::Pressed,
            false,
        );
        assert_eq!(event.physical, None);
        assert_eq!(
            event.logical,
            Some(LogicalKeyValue::Character("λ".into()).into())
        );
        for accent in [Some('´'), None] {
            let source = Key::Dead(accent);
            assert_eq!(logical_key(&source), Some(NamedKey::Dead.into()));
        }
        for (source, expected) in [
            (Named::Enter, NamedKey::Enter),
            (Named::Super, NamedKey::Meta),
            (Named::AltGraph, NamedKey::AltGraph),
            (Named::Fn, NamedKey::Fn),
            (Named::F35, NamedKey::F35),
        ] {
            assert_eq!(logical_key(&Key::Named(source)), Some(expected.into()));
        }
        assert_eq!(
            logical_key(&Key::Named(Named::Space)),
            Some(LogicalKeyValue::Character(" ".into()).into())
        );
    }

    #[test]
    fn transitions_and_reported_modifiers_survive() {
        for (state, repeat, expected) in [
            (ElementState::Pressed, false, KeyTransition::Press),
            (ElementState::Pressed, true, KeyTransition::Repeat),
            (ElementState::Released, true, KeyTransition::Release),
        ] {
            let observed = KeyboardObservation::from_winit(
                PhysicalKey::Code(KeyCode::ShiftLeft),
                &Key::Named(Named::Shift),
                ModifiersState::SHIFT,
                state,
                repeat,
            );
            assert_eq!(observed.transition, expected);
            assert_eq!(observed.modifiers, ModifierSet::SHIFT);
            assert_eq!(observed.physical, Some(Physical::SHIFT_LEFT));
            assert_eq!(observed.physical_modifiers().known(), ModifierSet::STANDARD);
            assert_eq!(observed.logical_modifiers().consumed, None);
        }
        for (code, expected) in [
            (KeyCode::Meta, None),
            (KeyCode::SuperLeft, Some(Physical::META_LEFT)),
            (KeyCode::NumpadEnter, Some(Physical::NUMPAD_ENTER)),
        ] {
            assert_eq!(
                KeyboardObservation::from_winit(
                    PhysicalKey::Code(code),
                    &Key::Named(Named::Enter),
                    ModifiersState::empty(),
                    ElementState::Pressed,
                    false
                )
                .physical,
                expected
            );
        }
    }
}
