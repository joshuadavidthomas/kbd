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
