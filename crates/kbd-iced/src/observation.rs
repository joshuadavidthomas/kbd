use iced_core::keyboard::Event;
use iced_core::keyboard::Key;
use iced_core::keyboard::key;
use kbd::hotkey::Hotkey;
use kbd::hotkey::Modifier;
use kbd::hotkey::ModifierSet;
use kbd::key_state::KeyTransition;
use kbd::observation::KeyboardObservation;
use kbd::observation::LogicalKey;
use kbd::observation::LogicalKeyValue;
use kbd::observation::ModifierObservation;
use kbd::observation::ModifierState;
use kbd::observation::NamedKey;

use crate::IcedKeyExt;
use crate::IcedModifiersExt;
use crate::private;

/// Convert an iced keyboard [`Event`] to a `kbd` [`Hotkey`].
///
/// Uses the physical key from the event for layout-independent matching.
/// Returns `None` for `ModifiersChanged` events (no key trigger) and
/// for events with unidentified physical keys.
///
/// When the key is itself a modifier (e.g., `ControlLeft`), the
/// corresponding modifier flag is stripped from the modifiers — iced
/// includes the pressed modifier key in its own modifier state, but
/// `kbd` treats the key as the trigger, not as a modifier of itself.
/// This trait is sealed and cannot be implemented outside this crate.
pub trait IcedEventExt: private::Sealed {
    /// Convert this keyboard event to a [`Hotkey`], or `None` if unmappable.
    ///
    /// # Examples
    ///
    /// ```
    /// use iced_core::keyboard::{Event, Location, Modifiers, key};
    /// use kbd::hotkey::{Hotkey, Modifier};
    /// use kbd::key::Key;
    /// use kbd_iced::IcedEventExt;
    ///
    /// let event = Event::KeyPressed {
    ///     key: iced_core::keyboard::Key::Unidentified,
    ///     modified_key: iced_core::keyboard::Key::Unidentified,
    ///     physical_key: key::Physical::Code(key::Code::KeyS),
    ///     location: Location::Standard,
    ///     modifiers: Modifiers::CTRL,
    ///     text: None,
    ///     repeat: false,
    /// };
    /// assert_eq!(
    ///     event.to_hotkey(),
    ///     Some(Hotkey::new(Key::S).modifier(Modifier::Ctrl)),
    /// );
    /// ```
    #[must_use]
    fn to_hotkey(&self) -> Option<Hotkey>;

    /// Observe independent identities, reported modifiers and transition.
    ///
    /// Logical identity comes from `modified_key`, not modifier-stripped `key`
    /// or produced text. Character strings retain exact case and Unicode.
    /// Generic physical `Meta` is omitted rather than assigned a side. Modifier
    /// triggers retain their reported modifiers, unlike legacy `to_hotkey`.
    /// Returns `None` only for `ModifiersChanged`; unknown identities stay absent.
    ///
    /// Keep the event for location, the unmodified key and press text. Iced
    /// releases carry no text/repeat and iced has already erased dead-key detail.
    /// IME is a separate input-method stream. The four aggregate modifier flags
    /// do not convey complete knowledge, sides, `AltGr` or Fn state.
    #[must_use]
    fn to_observation(&self) -> Option<KeyboardObservation>;
}

impl IcedEventExt for Event {
    fn to_observation(&self) -> Option<KeyboardObservation> {
        let (physical, logical, modifiers, transition) = match self {
            Event::KeyPressed {
                physical_key,
                modified_key,
                modifiers,
                repeat,
                ..
            } => (
                physical_key,
                modified_key,
                modifiers,
                if *repeat {
                    KeyTransition::Repeat
                } else {
                    KeyTransition::Press
                },
            ),
            Event::KeyReleased {
                physical_key,
                modified_key,
                modifiers,
                ..
            } => (
                physical_key,
                modified_key,
                modifiers,
                KeyTransition::Release,
            ),
            Event::ModifiersChanged(_) => return None,
        };
        Some(KeyboardObservation {
            physical: match physical {
                key::Physical::Code(key::Code::Meta) => None,
                _ => physical.to_key(),
            },
            logical: match logical {
                Key::Character(text) => Some(LogicalKeyValue::Character(text.to_string()).into()),
                Key::Named(named) => named_key(*named),
                Key::Unidentified => None,
            },
            modifiers: modifiers.to_modifiers(),
            modifier_observation: Some(ModifierObservation {
                physical: ModifierState::new(modifiers.to_modifiers(), ModifierSet::STANDARD),
                logical: None,
            }),
            transition,
        })
    }

    fn to_hotkey(&self) -> Option<Hotkey> {
        let (physical_key, modifiers) = match self {
            Event::KeyPressed {
                physical_key,
                modifiers,
                ..
            }
            | Event::KeyReleased {
                physical_key,
                modifiers,
                ..
            } => (physical_key, modifiers),
            Event::ModifiersChanged(_) => return None,
        };

        let key = physical_key.to_key()?;
        let mut mods = modifiers.to_modifiers();

        // Strip the modifier that corresponds to the key itself.
        if let Some(self_modifier) = Modifier::from_key(key) {
            mods = mods.without(self_modifier);
        }

        Some(Hotkey::with_modifiers(key, mods))
    }
}

#[allow(deprecated)] // Preserve explicitly reported legacy Hyper.
#[allow(clippy::too_many_lines)] // Complete named-key correspondence table.
fn named_key(key: key::Named) -> Option<LogicalKey> {
    use key::Named as Source;

    macro_rules! named {
        ($($name:ident),* $(,)?) => {
            match key {
                $(Source::$name => Some(NamedKey::$name.into()),)*
                Source::Super | Source::Meta => Some(NamedKey::Meta.into()),
                Source::Space => Some(LogicalKeyValue::Character(" ".into()).into()),
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
    use iced_core::keyboard::Event;
    use iced_core::keyboard::Key;
    use iced_core::keyboard::Location;
    use iced_core::keyboard::Modifiers;
    use kbd::action::Action;
    use kbd::binding::BindingOptions;
    use kbd::dispatcher::Dispatcher;
    use kbd::dispatcher::MatchResult;
    use kbd::hotkey::ModifierSet;
    use kbd::key::Key as Physical;
    use kbd::key_state::KeyTransition;
    use kbd::observation::BindingPattern;

    use super::*;
    use crate::IcedEventExt;

    fn press(physical_key: key::Physical, modified_key: Key, repeat: bool) -> Event {
        Event::KeyPressed {
            key: Key::Character("unmodified".into()),
            modified_key,
            physical_key,
            location: Location::Standard,
            modifiers: Modifiers::SHIFT,
            text: Some("produced text".into()),
            repeat,
        }
    }

    #[test]
    fn uses_exact_modified_identity_not_unmodified_key_or_text() {
        for text in ["A", "a", "@", "é", "e\u{301}", "👩‍💻", "Enter"] {
            let source = press(
                key::Physical::Code(key::Code::KeyQ),
                Key::Character(text.into()),
                false,
            );
            let observed = source.to_observation().unwrap();
            assert_eq!(observed.physical, Some(Physical::Q));
            assert_eq!(
                observed.logical,
                Some(LogicalKeyValue::Character(text.into()).into())
            );
            assert_eq!(observed.modifiers, ModifierSet::SHIFT);
            assert_eq!(observed.transition, KeyTransition::Press);
            let mut dispatcher = Dispatcher::new();
            dispatcher
                .register_pattern(
                    BindingPattern::logical(
                        LogicalKeyValue::Character(text.into()),
                        ModifierSet::SHIFT,
                    ),
                    Action::Suppress,
                    BindingOptions::default(),
                )
                .unwrap();
            assert!(matches!(
                dispatcher.process_event(&observed),
                MatchResult::Matched { .. }
            ));
            assert!(
                matches!(source, Event::KeyPressed { text: Some(ref t), .. } if t == "produced text")
            );
        }
    }

    #[test]
    fn unknown_and_unsided_physical_keys_do_not_hide_logical_keys() {
        for physical in [
            key::Physical::Unidentified(key::NativeCode::Unidentified),
            key::Physical::Code(key::Code::Meta),
        ] {
            let observed = press(physical, Key::Named(key::Named::Fn), true)
                .to_observation()
                .unwrap();
            assert_eq!(observed.physical, None);
            assert_eq!(observed.logical, Some(NamedKey::Fn.into()));
            assert_eq!(observed.transition, KeyTransition::Repeat);
        }
        let observed = press(
            key::Physical::Code(key::Code::SuperLeft),
            Key::Unidentified,
            false,
        )
        .to_observation()
        .unwrap();
        assert_eq!(observed.physical, Some(Physical::META_LEFT));
        assert_eq!(observed.logical, None);
        assert_eq!(
            named_key(key::Named::Space),
            Some(LogicalKeyValue::Character(" ".into()).into())
        );
        assert_eq!(named_key(key::Named::Super), Some(NamedKey::Meta.into()));
        assert_eq!(named_key(key::Named::F35), Some(NamedKey::F35.into()));
    }

    #[test]
    fn releases_and_modifier_only_events() {
        let source = Event::KeyReleased {
            key: Key::Character("unmodified".into()),
            modified_key: Key::Named(key::Named::Shift),
            physical_key: key::Physical::Code(key::Code::ShiftRight),
            location: Location::Right,
            modifiers: Modifiers::SHIFT,
        };
        let observed = source.to_observation().unwrap();
        assert_eq!(observed.transition, KeyTransition::Release);
        assert_eq!(observed.physical, Some(Physical::SHIFT_RIGHT));
        assert_eq!(observed.logical, Some(NamedKey::Shift.into()));
        assert_eq!(observed.modifiers, ModifierSet::SHIFT);
        assert_eq!(observed.physical_modifiers().known(), ModifierSet::STANDARD);
        assert_eq!(observed.logical_modifiers().consumed, None);
        assert_eq!(
            source.to_hotkey().unwrap().modifier_set(),
            ModifierSet::NONE
        );
        assert_eq!(
            Event::ModifiersChanged(Modifiers::SHIFT).to_observation(),
            None
        );
    }
}
