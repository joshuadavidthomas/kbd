use std::collections::HashMap;
use std::time::Instant;

use crate::events::HotkeyEvent;
use crate::key::Key;
use crate::manager::Callback;
use crate::manager::HotkeyKey;
use crate::manager::HotkeyRegistration;
use crate::manager::SequenceId;
use crate::manager::SequenceRegistration;

// SMELL: bool fields everywhere

#[derive(Clone)]
pub(crate) struct ActiveSequence {
    pub(crate) id: SequenceId,
    pub(crate) next_step_index: usize,
    pub(crate) deadline: Instant,
}

#[derive(Clone)]
pub(crate) struct PendingStandalone {
    pub(crate) key: HotkeyKey,
    pub(crate) pressed_at: Instant,
    pub(crate) released_at: Option<Instant>,
    pub(crate) deadline: Instant,
    pub(crate) press_dispatched: bool,
}

#[derive(Default)]
pub(crate) struct SequenceRuntime {
    pub(crate) active_sequences: Vec<ActiveSequence>,
    pub(crate) pending_standalone: Option<PendingStandalone>,
    pub(crate) deferred_release_callbacks: HashMap<Key, Callback>,
}

pub(crate) struct SequenceDispatch {
    pub(crate) callbacks: Vec<Callback>,
    pub(crate) synthetic_keys: Vec<HotkeyKey>,
    pub(crate) step_events: Vec<HotkeyEvent>,
    pub(crate) suppress_current_key_press: bool,
}

impl SequenceDispatch {
    pub(crate) fn empty() -> Self {
        Self {
            callbacks: Vec::new(),
            synthetic_keys: Vec::new(),
            step_events: Vec::new(),
            suppress_current_key_press: false,
        }
    }
}

impl SequenceRuntime {
    pub(crate) fn on_tick(
        &mut self,
        now: Instant,
        registrations: &HashMap<HotkeyKey, HotkeyRegistration>,
        sequence_registrations: &HashMap<SequenceId, SequenceRegistration>,
    ) -> SequenceDispatch {
        let mut callbacks = Vec::new();
        let mut synthetic_keys = Vec::new();

        if let Some(pending) = self.pending_standalone.as_mut() {
            if now >= pending.deadline {
                let mut should_clear_pending = true;

                if let Some(registration) = registrations.get(&pending.key) {
                    let hold_satisfied = registration.callbacks.min_hold.is_none_or(|min_hold| {
                        let held_for = pending.released_at.map_or_else(
                            || now.duration_since(pending.pressed_at),
                            |released_at| released_at.duration_since(pending.pressed_at),
                        );
                        held_for >= min_hold
                    });

                    if !pending.press_dispatched {
                        if hold_satisfied {
                            callbacks.push(registration.callbacks.on_press.clone());
                            pending.press_dispatched = true;
                        } else if pending.released_at.is_none() {
                            if let Some(min_hold) = registration.callbacks.min_hold {
                                pending.deadline = pending.pressed_at + min_hold;
                                should_clear_pending = false;
                            }
                        }
                    }

                    if pending.press_dispatched {
                        if pending.released_at.is_some() {
                            if let Some(on_release) = &registration.callbacks.on_release {
                                callbacks.push(on_release.clone());
                            }
                            should_clear_pending = true;
                        } else {
                            should_clear_pending = !registration.callbacks.wait_for_release;
                            if should_clear_pending {
                                if let Some(on_release) = &registration.callbacks.on_release {
                                    self.deferred_release_callbacks
                                        .insert(pending.key.0, on_release.clone());
                                }
                            }
                        }
                    }
                }

                if should_clear_pending {
                    self.pending_standalone = None;
                }
            }
        }

        let mut retained = Vec::with_capacity(self.active_sequences.len());
        for active in self.active_sequences.drain(..) {
            if now < active.deadline {
                retained.push(active);
                continue;
            }

            if let Some(registration) = sequence_registrations.get(&active.id) {
                if let Some(timeout_fallback) = &registration.timeout_fallback {
                    synthetic_keys.push(timeout_fallback.clone());
                }
            }
        }

        self.active_sequences = retained;

        SequenceDispatch {
            callbacks,
            synthetic_keys,
            step_events: Vec::new(),
            suppress_current_key_press: false,
        }
    }

    pub(crate) fn on_key_press(
        &mut self,
        key: HotkeyKey,
        now: Instant,
        registrations: &HashMap<HotkeyKey, HotkeyRegistration>,
        sequence_registrations: &HashMap<SequenceId, SequenceRegistration>,
    ) -> SequenceDispatch {
        self.deferred_release_callbacks.remove(&key.0);

        if self
            .pending_standalone
            .as_ref()
            .is_some_and(|pending| !pending.press_dispatched)
        {
            self.pending_standalone = None;
        }

        self.active_sequences.retain(|active| {
            sequence_registrations
                .get(&active.id)
                .is_some_and(|registration| registration.abort_key != key.0)
        });

        let mut callbacks = Vec::new();
        let mut step_events = Vec::new();
        let mut retained = Vec::with_capacity(self.active_sequences.len());
        let mut matched_existing_sequence = false;

        for mut active in self.active_sequences.drain(..) {
            let Some(registration) = sequence_registrations.get(&active.id) else {
                continue;
            };

            if registration
                .steps
                .get(active.next_step_index)
                .is_some_and(|expected| *expected == key)
            {
                matched_existing_sequence = true;

                if active.next_step_index + 1 == registration.steps.len() {
                    callbacks.push(registration.callback.clone());
                } else {
                    active.next_step_index += 1;
                    active.deadline = now + registration.timeout;
                    step_events.push(HotkeyEvent::SequenceStep {
                        id: active.id,
                        step: active.next_step_index,
                        total: registration.steps.len(),
                    });
                    retained.push(active);
                }
            }
        }

        self.active_sequences = retained;

        let mut started_sequences: Vec<ActiveSequence> = Vec::new();
        let mut earliest_deadline = None;
        for (id, registration) in sequence_registrations {
            if registration
                .steps
                .first()
                .is_some_and(|first_step| *first_step == key)
            {
                earliest_deadline = Some(
                    earliest_deadline.map_or(now + registration.timeout, |current: Instant| {
                        current.min(now + registration.timeout)
                    }),
                );

                started_sequences.push(ActiveSequence {
                    id: *id,
                    next_step_index: 1,
                    deadline: now + registration.timeout,
                });
                step_events.push(HotkeyEvent::SequenceStep {
                    id: *id,
                    step: 1,
                    total: registration.steps.len(),
                });
            }
        }

        let mut suppress_current_key_press = matched_existing_sequence;

        if !started_sequences.is_empty() {
            self.active_sequences.extend(started_sequences);

            if registrations.contains_key(&key) {
                suppress_current_key_press = true;
                if let Some(deadline) = earliest_deadline {
                    let can_replace_pending = self
                        .pending_standalone
                        .as_ref()
                        .is_none_or(|pending| !pending.press_dispatched);

                    if can_replace_pending {
                        self.pending_standalone = Some(PendingStandalone {
                            key,
                            pressed_at: now,
                            released_at: None,
                            deadline,
                            press_dispatched: false,
                        });
                    }
                }
            }
        }

        SequenceDispatch {
            callbacks,
            synthetic_keys: Vec::new(),
            step_events,
            suppress_current_key_press,
        }
    }

    pub(crate) fn on_key_release(&mut self, key: Key, now: Instant) -> Vec<Callback> {
        if let Some(pending) = self.pending_standalone.as_mut() {
            if pending.key.0 == key && pending.released_at.is_none() {
                pending.released_at = Some(now);
            }
        }

        self.deferred_release_callbacks
            .remove(&key)
            .into_iter()
            .collect()
    }
}
