use std::collections::HashMap;

use super::Dispatcher;
use super::sequence;
use super::sequence::RegisteredSequenceBinding;
use super::sequence::SequencePrefixKind;
use crate::hotkey::Hotkey;
use crate::hotkey::Modifier;
use crate::introspection::ActiveLayerInfo;
use crate::introspection::BindingInfo;
use crate::introspection::BindingLocation;
use crate::introspection::ConflictInfo;
use crate::introspection::ShadowedStatus;
use crate::layer::LayerName;
use crate::layer::StoredLayer;
use crate::layer::UnmatchedKeys;

enum SequenceQueryDecision {
    None,
    SingleStep(Hotkey),
    MultiStep,
}

impl Dispatcher {
    /// Return a snapshot of all registered bindings with their status.
    #[must_use]
    pub fn list_bindings(&self) -> Vec<BindingInfo> {
        // Build a map of hotkey → claiming layer name for active layers.
        // Walk top-down so the topmost layer claiming a hotkey "wins".
        let mut claimed_by: HashMap<&Hotkey, &LayerName> = HashMap::new();
        for entry in self.layer_stack.iter().rev() {
            if let Some(stored) = self.layers.get(&entry.name) {
                for binding in &stored.bindings {
                    claimed_by.entry(&binding.hotkey).or_insert(&entry.name);
                }
            }
        }

        let mut results = Vec::new();

        // Global bindings
        for binding in self.bindings_by_id.values() {
            let shadowed = if let Some(&layer_name) = claimed_by.get(binding.hotkey()) {
                ShadowedStatus::ShadowedBy(layer_name.clone())
            } else {
                ShadowedStatus::Active
            };

            results.push(BindingInfo {
                hotkey: binding.hotkey().clone(),
                description: binding.options().description().map(Box::from),
                location: BindingLocation::Global,
                shadowed,
                overlay_visibility: binding.options().overlay_visibility(),
            });
        }

        // Layer bindings (all defined layers, active or not)
        for (layer_name, stored) in &self.layers {
            let is_active = self.layer_stack.iter().any(|e| &e.name == layer_name);

            for binding in &stored.bindings {
                let shadowed = if !is_active {
                    ShadowedStatus::Inactive
                } else if let Some(&claiming_layer) = claimed_by.get(&binding.hotkey) {
                    if claiming_layer == layer_name {
                        ShadowedStatus::Active
                    } else {
                        ShadowedStatus::ShadowedBy(claiming_layer.clone())
                    }
                } else {
                    ShadowedStatus::Active
                };

                results.push(BindingInfo {
                    hotkey: binding.hotkey.clone(),
                    description: None,
                    location: BindingLocation::Layer(layer_name.clone()),
                    shadowed,
                    overlay_visibility: crate::binding::OverlayVisibility::Visible,
                });
            }
        }

        results
    }

    /// Query what would fire if this hotkey were pressed now.
    ///
    /// Considers the current layer stack. Returns `None` if nothing
    /// would match (including swallow-layer suppression and multi-step
    /// sequence prefixes that would enter a pending state).
    #[must_use]
    pub fn bindings_for_key(&self, hotkey: &Hotkey) -> Option<BindingInfo> {
        // Modifier-only keys never fire bindings in the real dispatcher,
        // so they can't match here either.
        if Modifier::from_key(hotkey.key()).is_some() {
            return None;
        }

        // Walk layer stack top-down, same as the dispatcher.
        // Sequence bindings are checked before immediate hotkeys.
        for entry in self.layer_stack.iter().rev() {
            if let Some(stored) = self.layers.get(&entry.name) {
                match Self::probe_layer_sequence_prefix(stored, hotkey) {
                    SequenceQueryDecision::None => {}
                    SequenceQueryDecision::SingleStep(matched_hotkey) => {
                        return Some(BindingInfo {
                            hotkey: matched_hotkey,
                            description: None,
                            location: BindingLocation::Layer(entry.name.clone()),
                            shadowed: ShadowedStatus::Active,
                            overlay_visibility: crate::binding::OverlayVisibility::Visible,
                        });
                    }
                    SequenceQueryDecision::MultiStep => {
                        return None;
                    }
                }

                for binding in &stored.bindings {
                    if binding.hotkey == *hotkey {
                        return Some(BindingInfo {
                            hotkey: binding.hotkey.clone(),
                            description: None,
                            location: BindingLocation::Layer(entry.name.clone()),
                            shadowed: ShadowedStatus::Active,
                            overlay_visibility: crate::binding::OverlayVisibility::Visible,
                        });
                    }
                }

                // Swallow layers block all unmatched keys from reaching
                // lower layers and globals — matches the real dispatcher.
                if matches!(stored.options.unmatched(), UnmatchedKeys::Swallow) {
                    return None;
                }
            }
        }

        // Global sequences are checked before global hotkeys, matching process().
        let mut global_sequences: Vec<_> = self
            .sequence_bindings_by_id
            .values()
            .filter(|binding| {
                !matches!(
                    sequence::classify_sequence_prefix(&binding.sequence, hotkey),
                    SequencePrefixKind::None
                )
            })
            .collect();
        global_sequences.sort_by_key(|binding| binding.id.as_u64());

        match Self::probe_global_sequence_prefix(&global_sequences, hotkey) {
            SequenceQueryDecision::None => {}
            SequenceQueryDecision::SingleStep(matched_hotkey) => {
                return Some(BindingInfo {
                    hotkey: matched_hotkey,
                    description: None,
                    location: BindingLocation::Global,
                    shadowed: ShadowedStatus::Active,
                    overlay_visibility: crate::binding::OverlayVisibility::Visible,
                });
            }
            SequenceQueryDecision::MultiStep => {
                return None;
            }
        }

        // Fall through to global immediate bindings.
        if let Some(&id) = self.binding_ids_by_hotkey.get(hotkey)
            && let Some(binding) = self.bindings_by_id.get(&id)
        {
            return Some(BindingInfo {
                hotkey: binding.hotkey().clone(),
                description: binding.options().description().map(Box::from),
                location: BindingLocation::Global,
                shadowed: ShadowedStatus::Active,
                overlay_visibility: binding.options().overlay_visibility(),
            });
        }

        None
    }

    fn probe_layer_sequence_prefix(stored: &StoredLayer, hotkey: &Hotkey) -> SequenceQueryDecision {
        let mut probe = SequenceQueryDecision::None;

        for sequence_binding in &stored.sequence_bindings {
            match sequence::classify_sequence_prefix(&sequence_binding.sequence, hotkey) {
                SequencePrefixKind::None => {}
                SequencePrefixKind::SingleStep => {
                    probe = SequenceQueryDecision::SingleStep(
                        sequence_binding.sequence.steps()[0].clone(),
                    );
                    break;
                }
                SequencePrefixKind::MultiStep => {
                    if matches!(probe, SequenceQueryDecision::None) {
                        probe = SequenceQueryDecision::MultiStep;
                    }
                }
            }
        }

        probe
    }

    fn probe_global_sequence_prefix(
        global_sequences: &[&RegisteredSequenceBinding],
        hotkey: &Hotkey,
    ) -> SequenceQueryDecision {
        let mut probe = SequenceQueryDecision::None;

        for sequence_binding in global_sequences {
            match sequence::classify_sequence_prefix(&sequence_binding.sequence, hotkey) {
                SequencePrefixKind::None => {}
                SequencePrefixKind::SingleStep => {
                    probe = SequenceQueryDecision::SingleStep(
                        sequence_binding.sequence.steps()[0].clone(),
                    );
                    break;
                }
                SequencePrefixKind::MultiStep => {
                    if matches!(probe, SequenceQueryDecision::None) {
                        probe = SequenceQueryDecision::MultiStep;
                    }
                }
            }
        }

        probe
    }

    /// Return the current layer stack (bottom to top).
    #[must_use]
    pub fn active_layers(&self) -> Vec<ActiveLayerInfo> {
        self.layer_stack
            .iter()
            .filter_map(|entry| {
                self.layers.get(&entry.name).map(|stored| ActiveLayerInfo {
                    name: entry.name.clone(),
                    description: stored.options.description().map(Box::from),
                    binding_count: stored.bindings.len() + stored.sequence_bindings.len(),
                })
            })
            .collect()
    }

    /// Return bindings shadowed by higher-priority layers.
    #[must_use]
    pub fn conflicts(&self) -> Vec<ConflictInfo> {
        let all_bindings = self.list_bindings();
        let mut conflicts = Vec::new();

        for shadowed in &all_bindings {
            if let ShadowedStatus::ShadowedBy(ref shadowing_layer) = shadowed.shadowed
                && let Some(shadowing) = all_bindings.iter().find(|b| {
                    b.hotkey == shadowed.hotkey
                        && matches!(&b.location, BindingLocation::Layer(name) if name == shadowing_layer)
                        && matches!(b.shadowed, ShadowedStatus::Active)
                })
            {
                conflicts.push(ConflictInfo {
                    hotkey: shadowed.hotkey.clone(),
                    shadowed_binding: shadowed.clone(),
                    shadowing_binding: shadowing.clone(),
                });
            }
        }

        conflicts
    }
}

#[cfg(test)]
mod tests {
    use crate::action::Action;
    use crate::dispatcher::Dispatcher;
    use crate::hotkey::Hotkey;
    use crate::hotkey::HotkeySequence;
    use crate::hotkey::Modifier;
    use crate::introspection::BindingLocation;
    use crate::introspection::ShadowedStatus;
    use crate::key::Key;
    use crate::layer::Layer;

    #[test]
    fn list_bindings_marks_inactive_layer_bindings() {
        let mut dispatcher = Dispatcher::new();
        dispatcher
            .define_layer(Layer::new("nav").bind(Key::H, Action::Suppress))
            .unwrap();
        // Layer defined but not pushed

        let bindings = dispatcher.list_bindings();
        assert_eq!(bindings.len(), 1);
        assert_eq!(bindings[0].shadowed, ShadowedStatus::Inactive);
    }

    #[test]
    fn bindings_for_key_returns_none_when_no_match() {
        let mut dispatcher = Dispatcher::new();
        dispatcher
            .register(
                Hotkey::new(Key::C).modifier(Modifier::Ctrl),
                Action::Suppress,
            )
            .unwrap();

        let result = dispatcher.bindings_for_key(&Hotkey::new(Key::V).modifier(Modifier::Ctrl));
        assert!(result.is_none());
    }

    #[test]
    fn bindings_for_key_returns_none_for_pending_sequence_prefix() {
        let mut dispatcher = Dispatcher::new();
        dispatcher
            .register(
                Hotkey::new(Key::K).modifier(Modifier::Ctrl),
                Action::Suppress,
            )
            .unwrap();
        dispatcher
            .register_sequence(
                "Ctrl+K, Ctrl+C".parse::<HotkeySequence>().unwrap(),
                Action::Suppress,
            )
            .unwrap();

        // Real dispatch enters sequence pending state here, so no immediate
        // binding action would fire.
        let result = dispatcher.bindings_for_key(&Hotkey::new(Key::K).modifier(Modifier::Ctrl));
        assert!(result.is_none());
    }

    #[test]
    fn bindings_for_key_reports_single_step_sequence_match() {
        let mut dispatcher = Dispatcher::new();
        dispatcher
            .register_sequence(
                "Ctrl+K".parse::<HotkeySequence>().unwrap(),
                Action::Suppress,
            )
            .unwrap();

        let result = dispatcher
            .bindings_for_key(&Hotkey::new(Key::K).modifier(Modifier::Ctrl))
            .expect("single-step sequence should match immediately");

        assert_eq!(result.hotkey, Hotkey::new(Key::K).modifier(Modifier::Ctrl));
        assert_eq!(result.location, BindingLocation::Global);
    }

    #[test]
    fn bindings_for_key_respects_swallow_layer() {
        let mut dispatcher = Dispatcher::new();
        dispatcher
            .register(Hotkey::new(Key::X), Action::Suppress)
            .unwrap();
        dispatcher
            .define_layer(Layer::new("modal").bind(Key::H, Action::Suppress).swallow())
            .unwrap();
        dispatcher.push_layer("modal").unwrap();

        // X not in swallow layer → blocked from reaching global
        let result = dispatcher.bindings_for_key(&Hotkey::new(Key::X));
        assert!(result.is_none());
    }

    #[test]
    fn bindings_for_key_returns_none_for_modifier_key() {
        let mut dispatcher = Dispatcher::new();
        dispatcher
            .register(Hotkey::new(Key::CONTROL_LEFT), Action::Suppress)
            .unwrap();

        let result = dispatcher.bindings_for_key(&Hotkey::new(Key::CONTROL_LEFT));
        assert!(result.is_none());
    }

    #[test]
    fn conflicts_empty_when_no_overlaps() {
        let mut dispatcher = Dispatcher::new();
        dispatcher
            .register(
                Hotkey::new(Key::C).modifier(Modifier::Ctrl),
                Action::Suppress,
            )
            .unwrap();
        assert!(dispatcher.conflicts().is_empty());
    }

    #[test]
    fn conflicts_empty_when_layer_not_active() {
        let mut dispatcher = Dispatcher::new();
        dispatcher
            .register(Hotkey::new(Key::H), Action::Suppress)
            .unwrap();
        dispatcher
            .define_layer(Layer::new("nav").bind(Key::H, Action::Suppress))
            .unwrap();
        // Layer defined but not pushed — no conflict

        assert!(dispatcher.conflicts().is_empty());
    }
}
