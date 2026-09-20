use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyEventKind;
use crossterm::event::MediaKeyCode;
use crossterm::event::ModifierKeyCode;
use kbd::key_state::KeyTransition;
use kbd::observation::KeyboardObservation;
use kbd::observation::LogicalKey;
use kbd::observation::LogicalKeyValue;
use kbd::observation::NamedKey;

use crate::CrosstermModifiersExt;

pub(super) fn convert(event: &KeyEvent) -> KeyboardObservation {
    KeyboardObservation {
        physical: None,
        logical: logical_key(event.code),
        modifiers: event.modifiers.to_modifiers(),
        modifier_observation: Some(kbd::observation::ModifierObservation {
            physical: kbd::observation::ModifierState::new(
                event.modifiers.to_modifiers(),
                kbd::hotkey::ModifierSet::STANDARD,
            )
            .with_extra_active(event.modifiers.intersects(
                crossterm::event::KeyModifiers::HYPER | crossterm::event::KeyModifiers::META,
            )),
            logical: None,
        }),
        transition: match event.kind {
            KeyEventKind::Press => KeyTransition::Press,
            KeyEventKind::Repeat => KeyTransition::Repeat,
            KeyEventKind::Release => KeyTransition::Release,
        },
    }
}

#[allow(deprecated)] // Preserve an explicitly reported legacy Hyper key.
fn logical_key(key: KeyCode) -> Option<LogicalKey> {
    use NamedKey as N;
    let named = match key {
        KeyCode::Char(ch) => return Some(LogicalKeyValue::Character(ch.to_string()).into()),
        KeyCode::Backspace => N::Backspace,
        KeyCode::Enter => N::Enter,
        KeyCode::Left => N::ArrowLeft,
        KeyCode::Right => N::ArrowRight,
        KeyCode::Up => N::ArrowUp,
        KeyCode::Down => N::ArrowDown,
        KeyCode::Home => N::Home,
        KeyCode::End => N::End,
        KeyCode::PageUp => N::PageUp,
        KeyCode::PageDown => N::PageDown,
        KeyCode::Tab => N::Tab,
        KeyCode::Delete => N::Delete,
        KeyCode::Insert => N::Insert,
        KeyCode::Esc => N::Escape,
        KeyCode::CapsLock => N::CapsLock,
        KeyCode::ScrollLock => N::ScrollLock,
        KeyCode::NumLock => N::NumLock,
        KeyCode::PrintScreen => N::PrintScreen,
        KeyCode::Pause => N::Pause,
        KeyCode::Menu => N::ContextMenu,
        KeyCode::F(n) => function_key(n)?,
        KeyCode::Media(media) => match media {
            MediaKeyCode::Play => N::MediaPlay,
            MediaKeyCode::Pause => N::MediaPause,
            MediaKeyCode::PlayPause => N::MediaPlayPause,
            MediaKeyCode::Stop => N::MediaStop,
            MediaKeyCode::FastForward => N::MediaFastForward,
            MediaKeyCode::Rewind => N::MediaRewind,
            MediaKeyCode::TrackNext => N::MediaTrackNext,
            MediaKeyCode::TrackPrevious => N::MediaTrackPrevious,
            MediaKeyCode::Record => N::MediaRecord,
            MediaKeyCode::LowerVolume => N::AudioVolumeDown,
            MediaKeyCode::RaiseVolume => N::AudioVolumeUp,
            MediaKeyCode::MuteVolume => N::AudioVolumeMute,
            MediaKeyCode::Reverse => return None,
        },
        KeyCode::Modifier(modifier) => match modifier {
            ModifierKeyCode::LeftShift | ModifierKeyCode::RightShift => N::Shift,
            ModifierKeyCode::LeftControl | ModifierKeyCode::RightControl => N::Control,
            ModifierKeyCode::LeftAlt | ModifierKeyCode::RightAlt => N::Alt,
            ModifierKeyCode::LeftSuper
            | ModifierKeyCode::RightSuper
            | ModifierKeyCode::LeftMeta
            | ModifierKeyCode::RightMeta => N::Meta,
            ModifierKeyCode::LeftHyper | ModifierKeyCode::RightHyper => N::Hyper,
            ModifierKeyCode::IsoLevel3Shift => N::AltGraph,
            ModifierKeyCode::IsoLevel5Shift => return None,
        },
        KeyCode::BackTab | KeyCode::Null | KeyCode::KeypadBegin => return None,
    };
    Some(named.into())
}

fn function_key(number: u8) -> Option<NamedKey> {
    macro_rules! functions {
        ($($number:literal => $name:ident),* $(,)?) => {
            match number {
                $($number => Some(NamedKey::$name),)*
                _ => None,
            }
        };
    }
    functions!(
        1 => F1, 2 => F2, 3 => F3, 4 => F4, 5 => F5, 6 => F6, 7 => F7,
        8 => F8, 9 => F9, 10 => F10, 11 => F11, 12 => F12, 13 => F13,
        14 => F14, 15 => F15, 16 => F16, 17 => F17, 18 => F18, 19 => F19,
        20 => F20, 21 => F21, 22 => F22, 23 => F23, 24 => F24, 25 => F25,
        26 => F26, 27 => F27, 28 => F28, 29 => F29, 30 => F30, 31 => F31,
        32 => F32, 33 => F33, 34 => F34, 35 => F35,
    )
}

#[cfg(test)]
mod tests {
    use crossterm::event::KeyEventState;
    use crossterm::event::KeyModifiers;
    use kbd::action::Action;
    use kbd::binding::BindingOptions;
    use kbd::dispatcher::Dispatcher;
    use kbd::dispatcher::MatchResult;
    use kbd::hotkey::ModifierSet;
    use kbd::observation::BindingPattern;

    use super::*;
    use crate::CrosstermEventExt;

    #[test]
    fn unrepresentable_active_modifiers_do_not_match_plain_shortcuts() {
        for flags in [KeyModifiers::HYPER, KeyModifiers::META] {
            let source = KeyEvent::new(KeyCode::Char('a'), flags);
            let event = source.to_observation();
            let mut dispatcher = Dispatcher::new();
            dispatcher
                .register_pattern(
                    BindingPattern::logical(
                        LogicalKeyValue::Character("a".into()),
                        ModifierSet::NONE,
                    ),
                    Action::Suppress,
                    BindingOptions::default(),
                )
                .unwrap();
            assert!(event.physical_modifiers().extra_active());
            assert!(matches!(
                dispatcher.process_event(&event),
                MatchResult::NoMatch
            ));
            // The legacy lossy projection remains a compatibility API.
            assert_eq!(
                source.to_hotkey().unwrap().modifier_set(),
                ModifierSet::NONE
            );
        }
    }

    #[test]
    fn exact_characters_without_physical_inference_or_case_normalization() {
        for (ch, flags, expected) in [
            ('A', KeyModifiers::NONE, ModifierSet::NONE),
            ('a', KeyModifiers::SHIFT, ModifierSet::SHIFT),
            ('+', KeyModifiers::NONE, ModifierSet::NONE),
            ('!', KeyModifiers::NONE, ModifierSet::NONE),
            ('λ', KeyModifiers::NONE, ModifierSet::NONE),
        ] {
            let source = KeyEvent::new(KeyCode::Char(ch), flags);
            let observed = source.to_observation();
            assert_eq!(observed.physical, None);
            assert_eq!(
                observed.logical,
                Some(LogicalKeyValue::Character(ch.to_string()).into())
            );
            assert_eq!(observed.modifiers, expected);
        }
    }

    #[test]
    fn named_keys_and_unknown_codes() {
        for (code, expected) in [
            (KeyCode::Enter, NamedKey::Enter),
            (KeyCode::F(35), NamedKey::F35),
            (
                KeyCode::Modifier(ModifierKeyCode::IsoLevel3Shift),
                NamedKey::AltGraph,
            ),
            (
                KeyCode::Media(MediaKeyCode::PlayPause),
                NamedKey::MediaPlayPause,
            ),
        ] {
            assert_eq!(
                KeyEvent::new(code, KeyModifiers::NONE)
                    .to_observation()
                    .logical,
                Some(expected.into())
            );
        }
        for code in [
            KeyCode::F(0),
            KeyCode::F(36),
            KeyCode::BackTab,
            KeyCode::Null,
            KeyCode::KeypadBegin,
            KeyCode::Modifier(ModifierKeyCode::IsoLevel5Shift),
        ] {
            let observed = KeyEvent::new(code, KeyModifiers::NONE).to_observation();
            assert_eq!(observed.logical, None);
            assert_eq!(observed.physical, None);
        }
    }

    #[test]
    fn transitions_keypad_evidence_and_modifier_triggers() {
        for (kind, expected) in [
            (KeyEventKind::Press, KeyTransition::Press),
            (KeyEventKind::Repeat, KeyTransition::Repeat),
            (KeyEventKind::Release, KeyTransition::Release),
        ] {
            for state in [
                KeyEventState::NONE,
                KeyEventState::KEYPAD | KeyEventState::NUM_LOCK,
            ] {
                let source = KeyEvent {
                    code: KeyCode::Char('1'),
                    modifiers: KeyModifiers::NONE,
                    kind,
                    state,
                };
                let observed = source.to_observation();
                assert_eq!(observed.physical, None);
                assert_eq!(observed.transition, expected);
            }
        }
        let source = KeyEvent::new(
            KeyCode::Modifier(ModifierKeyCode::LeftShift),
            KeyModifiers::SHIFT,
        );
        assert_eq!(source.to_observation().modifiers, ModifierSet::SHIFT);
        assert_eq!(
            source.to_hotkey().unwrap().modifier_set(),
            ModifierSet::NONE
        );
    }

    #[test]
    fn logical_only_input_dispatches_without_changing_legacy_projection() {
        let mut dispatcher = Dispatcher::new();
        dispatcher
            .register_pattern(
                BindingPattern::logical(LogicalKeyValue::Character("+".into()), ModifierSet::NONE),
                Action::Suppress,
                BindingOptions::default(),
            )
            .unwrap();
        let source = KeyEvent::new(KeyCode::Char('+'), KeyModifiers::NONE);
        assert!(source.to_hotkey().is_none());
        assert!(matches!(
            dispatcher.process_event(&source.to_observation()),
            MatchResult::Matched { .. }
        ));
        let lower = KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE).to_observation();
        let upper = KeyEvent::new(KeyCode::Char('A'), KeyModifiers::NONE).to_observation();
        assert_ne!(lower.logical, upper.logical);
    }
}
