//! Physical press ownership and silent lifecycle cancellation regressions.

use std::time::Duration;

use kbd::action::Action;
use kbd::dispatcher::Dispatcher;
use kbd::dispatcher::MatchResult;
use kbd::hotkey::Hotkey;
use kbd::hotkey::ModifierSet;
use kbd::key::Key;
use kbd::key_state::HeldKey;
use kbd::key_state::KeyTransition;
use kbd::observation::KeyboardObservation;
use kbd::observation::LogicalKeyValue;
use kbd::sequence::SequenceOptions;
use kbd::tap_hold::TapHoldOptions;

fn event(key: Key, transition: KeyTransition) -> KeyboardObservation {
    KeyboardObservation::from_hotkey(Hotkey::new(key), transition)
}

fn register(dispatcher: &mut Dispatcher) -> kbd::binding::BindingId {
    dispatcher
        .register_tap_hold(
            Key::A,
            Action::EmitHotkey(Key::T.into()),
            Action::EmitHotkey(Key::H.into()),
            TapHoldOptions::default().with_threshold(Duration::from_secs(60)),
        )
        .unwrap()
}

#[test]
fn same_physical_key_has_independent_source_and_generation() {
    let mut d = Dispatcher::new();
    register(&mut d);
    let press = event(Key::A, KeyTransition::Press);
    d.process_event_from_source(&press, 10);
    d.process_event_from_source(&press, 11); // distinct press interrupts source 10
    let pending = d.pending_timeouts();
    assert_eq!(pending.len(), 1);
    assert_eq!(
        pending[0].tap_hold_identity(),
        Some(HeldKey {
            source: Some(10),
            key: Key::A
        })
    );
    assert!(
        matches!(d.match_pending_timeout(&pending[0]), Some(MatchResult::Matched {
        action: Action::EmitHotkey(h), ..
    }) if h.key() == Key::H)
    );

    // Cancellation on source 10 must not cancel source 11's pending tap.
    d.cancel_source(Some(10));
    assert!(d.match_pending_timeout(&pending[0]).is_none());
    let mut release = event(Key::A, KeyTransition::Release);
    release.logical = Some(LogicalKeyValue::Character("q".into()).into());
    release.modifiers = ModifierSet::SHIFT;
    assert!(
        matches!(d.process_event_from_source(&release, 11), MatchResult::Matched {
        action: Action::EmitHotkey(h), ..
    } if h.key() == Key::T)
    );

    // Reusing the same source/key cannot validate an old timeout token.
    d.process_event_from_source(&press, 10);
    assert!(d.match_pending_timeout(&pending[0]).is_none());
    d.process_event_from_source(&event(Key::B, KeyTransition::Press), 10);
    let next = d.pending_timeouts();
    d.process_event_from_source(&event(Key::A, KeyTransition::Release), 10);
    d.process_event_from_source(&press, 10);
    assert!(d.match_pending_timeout(&next[0]).is_none());
}

#[test]
fn reset_and_unregister_are_cancellation_not_taps() {
    let mut d = Dispatcher::new();
    let id = register(&mut d);
    d.process_event_from_source(&event(Key::A, KeyTransition::Press), 7);
    d.reset_input();
    d.reset_input();
    assert!(d.pending_timeouts().is_empty());
    assert!(matches!(
        d.process_event_from_source(&event(Key::A, KeyTransition::Release), 7),
        MatchResult::Ignored
    ));
    d.process_event_from_source(&event(Key::A, KeyTransition::Press), 7);
    d.process_event_from_source(&event(Key::B, KeyTransition::Press), 7);
    let pending = d.pending_timeouts();
    d.unregister(id);
    register(&mut d);
    assert!(d.match_pending_timeout(&pending[0]).is_none());
    assert!(matches!(
        d.process_event_from_source(&event(Key::A, KeyTransition::Release), 7),
        MatchResult::Ignored
    ));
}

#[test]
fn full_reset_revokes_collected_sequence_fallback() {
    let mut d = Dispatcher::new();
    d.register(Key::A, Action::EmitHotkey(Key::T.into()))
        .unwrap();
    d.register_sequence_pattern(
        "A, B".parse().unwrap(),
        Action::Suppress,
        SequenceOptions::default().with_timeout(Duration::ZERO),
    )
    .unwrap();
    d.process_event(&event(Key::A, KeyTransition::Press));
    let pending = d.pending_timeouts();
    assert_eq!(pending.len(), 1);
    d.reset_input();
    assert!(d.match_pending_timeout(&pending[0]).is_none());
}

#[test]
fn logical_only_input_never_enrolls_a_held_key() {
    let mut d = Dispatcher::new();
    register(&mut d);
    let mut input = event(Key::A, KeyTransition::Press);
    input.physical = None;
    input.logical = Some(LogicalKeyValue::Character("a".into()).into());
    d.process_event_from_source(&input, 10);
    assert!(d.next_timeout_deadline().is_none());
}

#[test]
fn sequence_cancellation_revokes_fallback_but_preserves_tap_hold_tokens() {
    let mut d = Dispatcher::new();
    d.register(Key::B, Action::Suppress).unwrap();
    d.register_sequence_pattern(
        "B, C".parse().unwrap(),
        Action::Suppress,
        SequenceOptions::default().with_timeout(Duration::ZERO),
    )
    .unwrap();
    d.process_event(&event(Key::B, KeyTransition::Press));
    let sequence = d.pending_timeouts();
    assert_eq!(sequence.len(), 1);
    d.cancel_pending_sequence();
    assert!(d.match_pending_timeout(&sequence[0]).is_none());

    register(&mut d);
    d.process_event_from_source(&event(Key::A, KeyTransition::Press), 7);
    d.process_event_from_source(&event(Key::D, KeyTransition::Press), 7);
    let tap_hold = d.pending_timeouts();
    assert_eq!(tap_hold.len(), 1);
    d.cancel_pending_sequence();
    assert!(
        matches!(d.match_pending_timeout(&tap_hold[0]), Some(MatchResult::Matched {
        action: Action::EmitHotkey(h), ..
    }) if h.key() == Key::H)
    );
}
