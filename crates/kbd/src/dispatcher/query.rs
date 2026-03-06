use std::collections::HashMap;

use super::Dispatcher;
use super::resolve;
use super::resolve::LayerMatch;
use super::resolve::SequencePrefixMatch;
use crate::hotkey::Hotkey;
use crate::hotkey::Modifier;
use crate::introspection::ActiveLayerInfo;
use crate::introspection::BindingInfo;
use crate::introspection::BindingLocation;
use crate::introspection::ConflictInfo;
use crate::introspection::ShadowedStatus;
use crate::layer::LayerName;
use crate::layer::UnmatchedKeys;

impl Dispatcher {
    /// Return a snapshot of all registered immediate bindings with their status.
    ///
    /// Sequence bindings are queried through [`bindings_for_key`](Self::bindings_for_key)
    /// and [`pending_sequence`](crate::dispatcher::Dispatcher::pending_sequence).
    ///
    /// Results are returned in a deterministic order: global bindings are
    /// grouped by hotkey and then by precedence tier, followed by layer
    /// bindings ordered by layer name while preserving each layer's binding
    /// declaration order.
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

        let mut global_hotkeys: Vec<_> = self.binding_ids_by_hotkey.keys().collect();
        global_hotkeys.sort_by_cached_key(std::string::ToString::to_string);

        for hotkey in global_hotkeys {
            let Some(ids_for_hotkey) = self.binding_ids_by_hotkey.get(hotkey) else {
                continue;
            };

            for id in ids_for_hotkey {
                let Some(binding) = self.bindings_by_id.get(id) else {
                    continue;
                };

                let shadowed = if let Some(&layer_name) = claimed_by.get(binding.hotkey()) {
                    ShadowedStatus::ShadowedBy(layer_name.clone())
                } else if self.active_global_binding_id(binding.hotkey()) == Some(binding.id()) {
                    ShadowedStatus::Active
                } else {
                    ShadowedStatus::ShadowedByGlobal
                };

                results.push(BindingInfo {
                    hotkey: binding.hotkey().clone(),
                    description: binding.options().description().map(Box::from),
                    source: binding.options().source().cloned(),
                    location: BindingLocation::Global,
                    shadowed,
                    overlay_visibility: binding.options().overlay_visibility(),
                });
            }
        }

        let mut layer_names: Vec<_> = self.layers.keys().cloned().collect();
        layer_names.sort_by(|left, right| left.as_str().cmp(right.as_str()));

        for layer_name in layer_names {
            let Some(stored) = self.layers.get(&layer_name) else {
                continue;
            };
            let is_active = self
                .layer_stack
                .iter()
                .any(|entry| entry.name == layer_name);

            for binding in &stored.bindings {
                let shadowed = if !is_active {
                    ShadowedStatus::Inactive
                } else if let Some(&claiming_layer) = claimed_by.get(&binding.hotkey) {
                    if claiming_layer == &layer_name {
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
                    source: None,
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
        // classify_layer checks sequences before immediate hotkeys.
        for entry in self.layer_stack.iter().rev() {
            if let Some(stored) = self.layers.get(&entry.name) {
                let layer_match = resolve::classify_layer(stored, hotkey);
                match layer_match {
                    LayerMatch::SingleStepSequence { index } => {
                        let sb = &stored.sequence_bindings[index];
                        return Some(BindingInfo {
                            hotkey: sb.sequence.steps()[0].clone(),
                            description: None,
                            source: None,
                            location: BindingLocation::Layer(entry.name.clone()),
                            shadowed: ShadowedStatus::Active,
                            overlay_visibility: crate::binding::OverlayVisibility::Visible,
                        });
                    }
                    LayerMatch::MultiStepSequences { .. } => {
                        return None;
                    }
                    LayerMatch::Immediate { index } => {
                        let lb = &stored.bindings[index];
                        return Some(BindingInfo {
                            hotkey: lb.hotkey.clone(),
                            description: None,
                            source: None,
                            location: BindingLocation::Layer(entry.name.clone()),
                            shadowed: ShadowedStatus::Active,
                            overlay_visibility: crate::binding::OverlayVisibility::Visible,
                        });
                    }
                    LayerMatch::None => {
                        // Swallow layers block all unmatched keys from reaching
                        // lower layers and globals — matches the real dispatcher.
                        if matches!(stored.options.unmatched(), UnmatchedKeys::Swallow) {
                            return None;
                        }
                    }
                }
            }
        }

        // Global sequences are checked before global hotkeys, matching process().
        let global_seqs = self.sorted_global_sequences();
        let prefix_match =
            resolve::classify_sequence_prefixes(global_seqs.iter().map(|b| &b.sequence), hotkey);

        match prefix_match {
            SequencePrefixMatch::SingleStep { index } => {
                let binding = global_seqs[index];
                return Some(BindingInfo {
                    hotkey: binding.sequence.steps()[0].clone(),
                    description: None,
                    source: None,
                    location: BindingLocation::Global,
                    shadowed: ShadowedStatus::Active,
                    overlay_visibility: crate::binding::OverlayVisibility::Visible,
                });
            }
            SequencePrefixMatch::MultiStep { .. } => {
                return None;
            }
            SequencePrefixMatch::None => {}
        }

        // Fall through to global immediate bindings.
        if let Some(id) = self.active_global_binding_id(hotkey)
            && let Some(binding) = self.bindings_by_id.get(&id)
        {
            return Some(BindingInfo {
                hotkey: binding.hotkey().clone(),
                description: binding.options().description().map(Box::from),
                source: binding.options().source().cloned(),
                location: BindingLocation::Global,
                shadowed: ShadowedStatus::Active,
                overlay_visibility: binding.options().overlay_visibility(),
            });
        }

        None
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
            let shadowing = match &shadowed.shadowed {
                ShadowedStatus::ShadowedBy(shadowing_layer) => all_bindings.iter().find(|binding| {
                    binding.hotkey == shadowed.hotkey
                        && matches!(&binding.location, BindingLocation::Layer(name) if name == shadowing_layer)
                        && matches!(binding.shadowed, ShadowedStatus::Active)
                }),
                ShadowedStatus::ShadowedByGlobal => all_bindings.iter().find(|binding| {
                    binding.hotkey == shadowed.hotkey
                        && binding.location == BindingLocation::Global
                        && matches!(binding.shadowed, ShadowedStatus::Active)
                }),
                ShadowedStatus::Active | ShadowedStatus::Inactive => None,
            };

            if let Some(shadowing) = shadowing {
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
            .define_layer(Layer::new("nav").bind(Key::H, Action::Suppress).unwrap())
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
            .define_layer(
                Layer::new("modal")
                    .bind(Key::H, Action::Suppress)
                    .unwrap()
                    .swallow(),
            )
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
            .define_layer(Layer::new("nav").bind(Key::H, Action::Suppress).unwrap())
            .unwrap();
        // Layer defined but not pushed — no conflict

        assert!(dispatcher.conflicts().is_empty());
    }

    #[test]
    fn user_source_overrides_default_source_for_same_hotkey() {
        let mut dispatcher = Dispatcher::new();
        let hotkey = Hotkey::new(Key::C);

        dispatcher
            .register_binding(
                crate::binding::RegisteredBinding::new(
                    crate::binding::BindingId::new(),
                    hotkey.clone(),
                    Action::Suppress,
                )
                .with_options(crate::binding::BindingOptions::default().with_source("default")),
            )
            .unwrap();

        dispatcher
            .register_binding(
                crate::binding::RegisteredBinding::new(
                    crate::binding::BindingId::new(),
                    hotkey.clone(),
                    Action::Suppress,
                )
                .with_options(crate::binding::BindingOptions::default().with_source("user")),
            )
            .unwrap();

        let active = dispatcher
            .bindings_for_key(&hotkey)
            .expect("winning binding should be queryable");
        assert_eq!(active.location, BindingLocation::Global);
        assert_eq!(
            active
                .source
                .as_ref()
                .map(crate::binding::BindingSource::as_str),
            Some("user")
        );

        let bindings = dispatcher.list_bindings();
        assert_eq!(bindings.len(), 2);
        assert!(bindings.iter().any(|binding| {
            binding
                .source
                .as_ref()
                .map(crate::binding::BindingSource::as_str)
                == Some("user")
                && binding.shadowed == ShadowedStatus::Active
        }));
        assert!(bindings.iter().any(|binding| {
            binding
                .source
                .as_ref()
                .map(crate::binding::BindingSource::as_str)
                == Some("default")
                && binding.shadowed == ShadowedStatus::ShadowedByGlobal
        }));

        let conflicts = dispatcher.conflicts();
        assert_eq!(conflicts.len(), 1);
        assert_eq!(
            conflicts[0]
                .shadowing_binding
                .source
                .as_ref()
                .map(crate::binding::BindingSource::as_str),
            Some("user")
        );
        assert_eq!(
            conflicts[0]
                .shadowed_binding
                .source
                .as_ref()
                .map(crate::binding::BindingSource::as_str),
            Some("default")
        );
    }

    #[test]
    fn global_precedence_restores_in_tier_order() {
        let mut dispatcher = Dispatcher::new();
        let hotkey = Hotkey::new(Key::V);

        let default_id = crate::binding::BindingId::new();
        dispatcher
            .register_binding(
                crate::binding::RegisteredBinding::new(
                    default_id,
                    hotkey.clone(),
                    Action::Suppress,
                )
                .with_options(crate::binding::BindingOptions::default().with_source("DEFAULT")),
            )
            .unwrap();

        let plugin_id = crate::binding::BindingId::new();
        dispatcher
            .register_binding(
                crate::binding::RegisteredBinding::new(plugin_id, hotkey.clone(), Action::Suppress)
                    .with_options(crate::binding::BindingOptions::default().with_source("plugin")),
            )
            .unwrap();

        let user_id = crate::binding::BindingId::new();
        dispatcher
            .register_binding(
                crate::binding::RegisteredBinding::new(user_id, hotkey.clone(), Action::Suppress)
                    .with_options(crate::binding::BindingOptions::default().with_source("user")),
            )
            .unwrap();

        let active = dispatcher
            .bindings_for_key(&hotkey)
            .expect("user binding should win over standard and default tiers");
        assert_eq!(
            active
                .source
                .as_ref()
                .map(crate::binding::BindingSource::as_str),
            Some("user")
        );
        assert_eq!(dispatcher.conflicts().len(), 2);

        dispatcher.unregister(user_id);

        let promoted = dispatcher
            .bindings_for_key(&hotkey)
            .expect("standard-tier binding should be promoted after user removal");
        assert_eq!(
            promoted
                .source
                .as_ref()
                .map(crate::binding::BindingSource::as_str),
            Some("plugin")
        );
        assert_eq!(dispatcher.conflicts().len(), 1);

        dispatcher.unregister(plugin_id);

        let restored = dispatcher
            .bindings_for_key(&hotkey)
            .expect("default-tier binding should be restored last");
        assert_eq!(
            restored
                .source
                .as_ref()
                .map(crate::binding::BindingSource::as_str),
            Some("DEFAULT")
        );
        assert!(dispatcher.conflicts().is_empty());

        dispatcher.unregister(default_id);
        assert!(dispatcher.bindings_for_key(&hotkey).is_none());
    }

    #[test]
    fn list_bindings_orders_globals_by_hotkey_then_precedence() {
        let mut dispatcher = Dispatcher::new();

        dispatcher
            .register_binding(
                crate::binding::RegisteredBinding::new(
                    crate::binding::BindingId::new(),
                    Hotkey::new(Key::V),
                    Action::Suppress,
                )
                .with_options(crate::binding::BindingOptions::default().with_source("user")),
            )
            .unwrap();
        dispatcher
            .register_binding(crate::binding::RegisteredBinding::new(
                crate::binding::BindingId::new(),
                Hotkey::new(Key::A),
                Action::Suppress,
            ))
            .unwrap();
        dispatcher
            .register_binding(
                crate::binding::RegisteredBinding::new(
                    crate::binding::BindingId::new(),
                    Hotkey::new(Key::V),
                    Action::Suppress,
                )
                .with_options(crate::binding::BindingOptions::default().with_source("default")),
            )
            .unwrap();
        dispatcher
            .register_binding(
                crate::binding::RegisteredBinding::new(
                    crate::binding::BindingId::new(),
                    Hotkey::new(Key::V),
                    Action::Suppress,
                )
                .with_options(crate::binding::BindingOptions::default().with_source("plugin")),
            )
            .unwrap();

        let summary: Vec<_> = dispatcher
            .list_bindings()
            .into_iter()
            .map(|binding| {
                (
                    binding.hotkey.to_string(),
                    binding.source.map(|source| source.to_string()),
                )
            })
            .collect();

        assert_eq!(
            summary,
            vec![
                ("A".to_string(), None),
                ("V".to_string(), Some("default".to_string())),
                ("V".to_string(), Some("plugin".to_string())),
                ("V".to_string(), Some("user".to_string())),
            ]
        );
    }

    #[test]
    fn list_bindings_orders_layers_by_name_and_preserves_layer_declaration_order() {
        let mut dispatcher = Dispatcher::new();
        dispatcher
            .define_layer(
                Layer::new("zeta")
                    .bind(Key::Z, Action::Suppress)
                    .unwrap()
                    .bind(Key::Y, Action::Suppress)
                    .unwrap(),
            )
            .unwrap();
        dispatcher
            .define_layer(
                Layer::new("alpha")
                    .bind(Key::B, Action::Suppress)
                    .unwrap()
                    .bind(Key::A, Action::Suppress)
                    .unwrap(),
            )
            .unwrap();

        let summary: Vec<_> = dispatcher
            .list_bindings()
            .into_iter()
            .map(|binding| {
                let BindingLocation::Layer(name) = binding.location else {
                    panic!("expected only layer bindings in this test");
                };
                (name.to_string(), binding.hotkey.to_string())
            })
            .collect();

        assert_eq!(
            summary,
            vec![
                ("alpha".to_string(), "B".to_string()),
                ("alpha".to_string(), "A".to_string()),
                ("zeta".to_string(), "Z".to_string()),
                ("zeta".to_string(), "Y".to_string()),
            ]
        );
    }
}
