use std::collections::HashMap;

use super::Dispatcher;
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
    /// would match (including swallow-layer suppression).
    #[must_use]
    pub fn bindings_for_key(&self, hotkey: &Hotkey) -> Option<BindingInfo> {
        // Modifier-only keys never fire bindings in the real dispatcher,
        // so they can't match here either.
        if Modifier::from_key(hotkey.key()).is_some() {
            return None;
        }

        // Walk layer stack top-down, same as the dispatcher
        for entry in self.layer_stack.iter().rev() {
            if let Some(stored) = self.layers.get(&entry.name) {
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

        // Fall through to global bindings
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

    /// Return the current layer stack (bottom to top).
    #[must_use]
    pub fn active_layers(&self) -> Vec<ActiveLayerInfo> {
        self.layer_stack
            .iter()
            .filter_map(|entry| {
                self.layers.get(&entry.name).map(|stored| ActiveLayerInfo {
                    name: entry.name.clone(),
                    description: stored.options.description().map(Box::from),
                    binding_count: stored.bindings.len(),
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
    use crate::binding::BindingId;
    use crate::binding::BindingOptions;
    use crate::binding::OverlayVisibility;
    use crate::binding::RegisteredBinding;
    use crate::dispatcher::Dispatcher;
    use crate::hotkey::Hotkey;
    use crate::hotkey::Modifier;
    use crate::introspection::BindingLocation;
    use crate::introspection::ShadowedStatus;
    use crate::key::Key;
    use crate::layer::Layer;
    use crate::layer::LayerName;

    #[test]
    fn list_bindings_returns_global_bindings() {
        let mut dispatcher = Dispatcher::new();
        dispatcher
            .register_binding(
                RegisteredBinding::new(
                    BindingId::new(),
                    Hotkey::new(Key::C).modifier(Modifier::Ctrl),
                    Action::Suppress,
                )
                .with_options(BindingOptions::default().with_description("Copy")),
            )
            .unwrap();

        let bindings = dispatcher.list_bindings();
        assert_eq!(bindings.len(), 1);
        assert_eq!(bindings[0].description.as_deref(), Some("Copy"));
        assert_eq!(bindings[0].location, BindingLocation::Global);
        assert_eq!(bindings[0].shadowed, ShadowedStatus::Active);
    }

    #[test]
    fn list_bindings_includes_layer_bindings() {
        let mut dispatcher = Dispatcher::new();
        dispatcher
            .define_layer(
                Layer::new("nav")
                    .bind(Key::H, Action::Suppress)
                    .bind(Key::J, Action::Suppress),
            )
            .unwrap();

        let bindings = dispatcher.list_bindings();
        let layer_bindings: Vec<_> = bindings
            .iter()
            .filter(|b| matches!(b.location, BindingLocation::Layer(_)))
            .collect();
        assert_eq!(layer_bindings.len(), 2);
    }

    #[test]
    fn list_bindings_detects_shadowed_global() {
        let mut dispatcher = Dispatcher::new();
        dispatcher
            .register(Hotkey::new(Key::H), Action::Suppress)
            .unwrap();
        dispatcher
            .define_layer(Layer::new("nav").bind(Key::H, Action::Suppress))
            .unwrap();
        dispatcher.push_layer("nav").unwrap();

        let bindings = dispatcher.list_bindings();
        let global_h = bindings
            .iter()
            .find(|b| b.hotkey == Hotkey::new(Key::H) && b.location == BindingLocation::Global)
            .expect("should find global H");
        assert_eq!(
            global_h.shadowed,
            ShadowedStatus::ShadowedBy(LayerName::from("nav"))
        );
    }

    #[test]
    fn list_bindings_preserves_overlay_visibility() {
        let mut dispatcher = Dispatcher::new();
        dispatcher
            .register_binding(
                RegisteredBinding::new(
                    BindingId::new(),
                    Hotkey::new(Key::C).modifier(Modifier::Ctrl),
                    Action::Suppress,
                )
                .with_options(
                    BindingOptions::default().with_overlay_visibility(OverlayVisibility::Hidden),
                ),
            )
            .unwrap();

        let bindings = dispatcher.list_bindings();
        assert_eq!(bindings[0].overlay_visibility, OverlayVisibility::Hidden);
    }

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
    fn bindings_for_key_returns_matching_binding() {
        let mut dispatcher = Dispatcher::new();
        dispatcher
            .register_binding(
                RegisteredBinding::new(
                    BindingId::new(),
                    Hotkey::new(Key::C).modifier(Modifier::Ctrl),
                    Action::Suppress,
                )
                .with_options(BindingOptions::default().with_description("Copy")),
            )
            .unwrap();

        let result = dispatcher.bindings_for_key(&Hotkey::new(Key::C).modifier(Modifier::Ctrl));
        assert!(result.is_some());
        assert_eq!(result.unwrap().description.as_deref(), Some("Copy"));
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
    fn bindings_for_key_respects_layer_stack() {
        let mut dispatcher = Dispatcher::new();
        dispatcher
            .register(Hotkey::new(Key::H), Action::Suppress)
            .unwrap();
        dispatcher
            .define_layer(Layer::new("nav").bind(Key::H, Action::Suppress))
            .unwrap();
        dispatcher.push_layer("nav").unwrap();

        let result = dispatcher.bindings_for_key(&Hotkey::new(Key::H));
        assert!(result.is_some());
        assert_eq!(
            result.unwrap().location,
            BindingLocation::Layer(LayerName::from("nav"))
        );
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
    fn active_layers_reflects_stack() {
        let mut dispatcher = Dispatcher::new();
        dispatcher
            .define_layer(
                Layer::new("layer1")
                    .bind(Key::H, Action::Suppress)
                    .description("First"),
            )
            .unwrap();
        dispatcher
            .define_layer(
                Layer::new("layer2")
                    .bind(Key::J, Action::Suppress)
                    .bind(Key::K, Action::Suppress)
                    .description("Second"),
            )
            .unwrap();
        dispatcher.push_layer("layer1").unwrap();
        dispatcher.push_layer("layer2").unwrap();

        let active = dispatcher.active_layers();
        assert_eq!(active.len(), 2);
        assert_eq!(active[0].name.as_str(), "layer1");
        assert_eq!(active[0].description.as_deref(), Some("First"));
        assert_eq!(active[0].binding_count, 1);
        assert_eq!(active[1].name.as_str(), "layer2");
        assert_eq!(active[1].description.as_deref(), Some("Second"));
        assert_eq!(active[1].binding_count, 2);
    }

    #[test]
    fn active_layers_empty_when_none_pushed() {
        let mut dispatcher = Dispatcher::new();
        dispatcher
            .define_layer(Layer::new("nav").bind(Key::H, Action::Suppress))
            .unwrap();

        assert!(dispatcher.active_layers().is_empty());
    }

    #[test]
    fn conflicts_detects_layer_shadowing_global() {
        let mut dispatcher = Dispatcher::new();
        dispatcher
            .register(Hotkey::new(Key::H), Action::Suppress)
            .unwrap();
        dispatcher
            .define_layer(Layer::new("nav").bind(Key::H, Action::Suppress))
            .unwrap();
        dispatcher.push_layer("nav").unwrap();

        let conflicts = dispatcher.conflicts();
        assert_eq!(conflicts.len(), 1);
        assert_eq!(conflicts[0].hotkey, Hotkey::new(Key::H));
        assert_eq!(
            conflicts[0].shadowed_binding.location,
            BindingLocation::Global
        );
        assert_eq!(
            conflicts[0].shadowing_binding.location,
            BindingLocation::Layer(LayerName::from("nav"))
        );
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
