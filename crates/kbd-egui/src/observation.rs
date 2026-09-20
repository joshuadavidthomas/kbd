use egui::Event;
use egui::Key;
use kbd::key_state::KeyTransition;
use kbd::observation::KeyboardObservation;

use crate::EguiKeyExt;
use crate::EguiModifiersExt;

pub(super) fn convert(event: &Event) -> Option<KeyboardObservation> {
    let Event::Key {
        physical_key,
        pressed,
        repeat,
        modifiers,
        ..
    } = event
    else {
        return None;
    };
    Some(KeyboardObservation {
        physical: physical_key.and_then(|key| match key {
            // egui-winit merges distinct physical codes into these values.
            Key::Num0
            | Key::Num1
            | Key::Num2
            | Key::Num3
            | Key::Num4
            | Key::Num5
            | Key::Num6
            | Key::Num7
            | Key::Num8
            | Key::Num9
            | Key::Enter
            | Key::Slash
            | Key::Minus => None,
            _ => key.to_key(),
        }),
        // Even a named shortcut can originate from a physical fallback.
        logical: None,
        modifiers: modifiers.to_modifiers(),
        modifier_observation: Some(kbd::observation::ModifierObservation {
            // mac_cmd=true is positive evidence; false does not establish
            // Super's absence on non-Mac hosts. `command` is only an alias.
            physical: kbd::observation::ModifierState::new(
                modifiers.to_modifiers(),
                kbd::hotkey::ModifierSet::CTRL
                    .union(kbd::hotkey::ModifierSet::SHIFT)
                    .union(kbd::hotkey::ModifierSet::ALT),
            )
            .with_extra_active(modifiers.command && !modifiers.ctrl && !modifiers.mac_cmd),
            logical: None,
        }),
        transition: match (*pressed, *repeat) {
            (false, _) => KeyTransition::Release,
            (true, true) => KeyTransition::Repeat,
            (true, false) => KeyTransition::Press,
        },
    })
}

#[cfg(test)]
mod tests {
    use egui::ImeEvent;
    use egui::Modifiers;
    use kbd::hotkey::ModifierSet;
    use kbd::key::Key as Physical;

    use super::*;
    use crate::EguiEventExt;

    #[test]
    fn command_alias_does_not_establish_super_knowledge() {
        let mut source = event(Some(Key::A), true, false);
        if let Event::Key { modifiers, .. } = &mut source {
            modifiers.command = true;
        }
        let observed = source.to_observation().unwrap();
        assert!(
            !observed
                .physical_modifiers()
                .known()
                .contains(kbd::hotkey::Modifier::Super)
        );
        assert!(observed.physical_modifiers().active().is_empty());
        // An active alias with no concrete flag is still not an unmodified event.
        assert!(observed.physical_modifiers().extra_active());
        assert!(!kbd::observation::BindingPattern::Physical(Physical::A.into()).matches(&observed));
        assert!(observed.physical_hotkey().is_none());
        if let Event::Key { modifiers, .. } = &mut source {
            modifiers.mac_cmd = true;
        }
        let observed = source.to_observation().unwrap();
        assert!(
            observed
                .physical_modifiers()
                .known()
                .contains(kbd::hotkey::Modifier::Super)
        );
        assert_eq!(observed.physical_modifiers().active(), ModifierSet::SUPER);
    }

    fn event(physical_key: Option<Key>, pressed: bool, repeat: bool) -> Event {
        Event::Key {
            key: Key::A,
            physical_key,
            pressed,
            repeat,
            modifiers: Modifiers::NONE,
        }
    }

    #[test]
    fn physical_identity_never_comes_from_shortcut_labels() {
        let source = event(Some(Key::Q), true, false);
        let observed = source.to_observation().unwrap();
        assert_eq!(observed.physical, Some(Physical::Q));
        assert_eq!(observed.logical, None);
        assert_eq!(source.to_hotkey().unwrap().key(), Physical::A);
        assert_eq!(
            event(None, true, false).to_observation().unwrap().physical,
            None
        );
    }

    #[test]
    fn merged_and_shifted_positions_remain_unknown() {
        for key in [
            Key::Num0,
            Key::Num1,
            Key::Num9,
            Key::Enter,
            Key::Slash,
            Key::Minus,
            Key::Plus,
            Key::Colon,
            Key::Questionmark,
        ] {
            let observed = event(Some(key), true, false).to_observation().unwrap();
            assert_eq!(observed.physical, None, "{key:?}");
            assert_eq!(observed.logical, None);
        }
        assert_eq!(
            event(Some(Key::Equals), true, false)
                .to_observation()
                .unwrap()
                .physical,
            Some(Physical::EQUAL)
        );
    }

    #[test]
    fn transition_and_modifier_aliases_are_not_invented() {
        for (pressed, repeat, expected) in [
            (true, false, KeyTransition::Press),
            (true, true, KeyTransition::Repeat),
            (false, true, KeyTransition::Release),
        ] {
            assert_eq!(
                event(None, pressed, repeat)
                    .to_observation()
                    .unwrap()
                    .transition,
                expected
            );
        }
        let source = Event::Key {
            key: Key::A,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers {
                command: true,
                ..Modifiers::NONE
            },
        };
        assert_eq!(
            source.to_observation().unwrap().modifiers,
            ModifierSet::NONE
        );
        let source = Event::Key {
            key: Key::A,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers {
                command: true,
                mac_cmd: true,
                ..Modifiers::NONE
            },
        };
        assert_eq!(
            source.to_observation().unwrap().modifiers,
            ModifierSet::SUPER
        );
    }

    #[test]
    fn text_and_ime_do_not_become_keyboard_events() {
        for source in [
            Event::Text("é".into()),
            Event::Paste("A".into()),
            Event::Ime(ImeEvent::Commit("👩‍💻".into())),
        ] {
            assert_eq!(source.to_observation(), None);
        }
    }
}
