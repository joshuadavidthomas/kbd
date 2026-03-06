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
    #[must_use]
    pub fn list_bindings(&self) -> Vec<BindingInfo> {
        // Build a map of effective hotkey → claiming layer name for active
        // layers. Walk top-down so the topmost layer claiming a hotkey wins.
        let mut claimed_by: HashMap<Hotkey, LayerName> = HashMap::new();
        for entry in self.layer_stack.iter().rev() {
            if let Some(stored) = self.layers.get(&entry.name) {
                for binding in &stored.bindings {
                    let Some(effective_hotkey) = self.resolve_hotkey(&binding.hotkey) else {
                        continue;
                    };
                    claimed_by
                        .entry(effective_hotkey)
                        .or_insert_with(|| entry.name.clone());
                }
            }
        }

        let mut results = Vec::new();

        // Global bindings
        for binding in self.bindings_by_id.values() {
            let shadowed = match self.resolve_hotkey(binding.hotkey()) {
                Some(effective_hotkey) => claimed_by
                    .get(&effective_hotkey)
                    .cloned()
                    .map_or(ShadowedStatus::Active, ShadowedStatus::ShadowedBy),
                None => ShadowedStatus::UnresolvedAlias,
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
                } else if let Some(effective_hotkey) = self.resolve_hotkey(&binding.hotkey) {
                    if let Some(claiming_layer) = claimed_by.get(&effective_hotkey) {
                        if claiming_layer == layer_name {
                            ShadowedStatus::Active
                        } else {
                            ShadowedStatus::ShadowedBy(claiming_layer.clone())
                        }
                    } else {
                        ShadowedStatus::Active
                    }
                } else {
                    ShadowedStatus::UnresolvedAlias
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
        // classify_layer checks sequences before immediate hotkeys.
        for entry in self.layer_stack.iter().rev() {
            if let Some(stored) = self.layers.get(&entry.name) {
                let layer_match = resolve::classify_layer(stored, hotkey, &self.modifier_aliases);
                match layer_match {
                    LayerMatch::SingleStepSequence { index } => {
                        let sb = &stored.sequence_bindings[index];
                        return Some(BindingInfo {
                            hotkey: sb.sequence.steps()[0].clone(),
                            description: None,
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
        let prefix_match = resolve::classify_sequence_prefixes(
            global_seqs.iter().map(|b| &b.sequence),
            hotkey,
            &self.modifier_aliases,
        );

        match prefix_match {
            SequencePrefixMatch::SingleStep { index } => {
                let binding = global_seqs[index];
                return Some(BindingInfo {
                    hotkey: binding.sequence.steps()[0].clone(),
                    description: None,
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
        // Check both direct and alias-resolved lookups, matching match_global_hotkey().
        let global_id = self
            .binding_ids_by_hotkey
            .get(hotkey)
            .or_else(|| self.alias_resolved_ids.get(hotkey));

        if let Some(&id) = global_id
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
            let Some(effective_hotkey) = self.resolve_hotkey(&shadowed.hotkey) else {
                continue;
            };

            if let ShadowedStatus::ShadowedBy(ref shadowing_layer) = shadowed.shadowed
                && let Some(shadowing) = all_bindings.iter().find(|binding| {
                    matches!(&binding.location, BindingLocation::Layer(name) if name == shadowing_layer)
                        && matches!(binding.shadowed, ShadowedStatus::Active)
                        && self.resolve_hotkey(&binding.hotkey).as_ref() == Some(&effective_hotkey)
                })
            {
                conflicts.push(ConflictInfo {
                    hotkey: effective_hotkey.clone(),
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
    fn bindings_for_key_finds_alias_resolved_global_binding() {
        let mut dispatcher = Dispatcher::new();
        dispatcher
            .define_modifier_alias("Mod", Modifier::Super)
            .unwrap();
        dispatcher.register("Mod+T", Action::Suppress).unwrap();

        // Query with the resolved concrete hotkey (Super+T)
        let result = dispatcher
            .bindings_for_key(&Hotkey::new(Key::T).modifier(Modifier::Super))
            .expect("alias-resolved global binding should be found");

        assert_eq!(result.location, BindingLocation::Global);
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
    fn list_bindings_marks_alias_layer_binding_as_shadowing_concrete_global() {
        let mut dispatcher = Dispatcher::new();
        dispatcher
            .define_modifier_alias("Mod", Modifier::Super)
            .unwrap();
        dispatcher
            .register(
                Hotkey::new(Key::T).modifier(Modifier::Super),
                Action::Suppress,
            )
            .unwrap();
        dispatcher
            .define_layer(Layer::new("nav").bind("Mod+T", Action::Suppress).unwrap())
            .unwrap();
        dispatcher.push_layer("nav").unwrap();

        let bindings = dispatcher.list_bindings();
        let global = bindings
            .iter()
            .find(|binding| binding.location == BindingLocation::Global)
            .expect("global binding should exist");
        let layer = bindings
            .iter()
            .find(|binding| binding.location == BindingLocation::Layer("nav".into()))
            .expect("layer binding should exist");

        assert!(matches!(global.shadowed, ShadowedStatus::ShadowedBy(_)));
        assert_eq!(layer.shadowed, ShadowedStatus::Active);
    }

    #[test]
    fn conflicts_report_alias_layer_binding_against_concrete_global() {
        let mut dispatcher = Dispatcher::new();
        dispatcher
            .define_modifier_alias("Mod", Modifier::Super)
            .unwrap();
        dispatcher
            .register(
                Hotkey::new(Key::T).modifier(Modifier::Super),
                Action::Suppress,
            )
            .unwrap();
        dispatcher
            .define_layer(Layer::new("nav").bind("Mod+T", Action::Suppress).unwrap())
            .unwrap();
        dispatcher.push_layer("nav").unwrap();

        let conflicts = dispatcher.conflicts();
        assert_eq!(conflicts.len(), 1);
        assert_eq!(
            conflicts[0].hotkey,
            Hotkey::new(Key::T).modifier(Modifier::Super)
        );
        assert_eq!(
            conflicts[0].shadowing_binding.location,
            BindingLocation::Layer("nav".into())
        );
    }

    #[test]
    fn bindings_for_key_finds_alias_resolved_layer_sequence() {
        let mut dispatcher = Dispatcher::new();
        dispatcher
            .define_modifier_alias("Mod", Modifier::Super)
            .unwrap();
        let layer = Layer::new("nav")
            .bind_sequence("Mod+K", Action::Suppress)
            .unwrap();
        dispatcher.define_layer(layer).unwrap();
        dispatcher.push_layer("nav").unwrap();

        let result = dispatcher
            .bindings_for_key(&Hotkey::new(Key::K).modifier(Modifier::Super))
            .expect("single-step aliased sequence should match");

        assert_eq!(result.location, BindingLocation::Layer("nav".into()));
        assert_eq!(result.hotkey, "Mod+K".parse::<Hotkey>().unwrap());
    }

    #[test]
    fn list_bindings_marks_global_binding_with_undefined_alias_as_unresolved() {
        let mut dispatcher = Dispatcher::new();
        dispatcher.register("Mod+T", Action::Suppress).unwrap();

        let bindings = dispatcher.list_bindings();
        assert_eq!(bindings.len(), 1);
        assert_eq!(bindings[0].shadowed, ShadowedStatus::UnresolvedAlias);
    }

    #[test]
    fn list_bindings_marks_active_layer_binding_with_undefined_alias_as_unresolved() {
        let mut dispatcher = Dispatcher::new();
        dispatcher
            .define_layer(Layer::new("nav").bind("Mod+T", Action::Suppress).unwrap())
            .unwrap();
        dispatcher.push_layer("nav").unwrap();

        let bindings = dispatcher.list_bindings();
        assert_eq!(bindings.len(), 1);
        assert_eq!(bindings[0].shadowed, ShadowedStatus::UnresolvedAlias);
    }
}
