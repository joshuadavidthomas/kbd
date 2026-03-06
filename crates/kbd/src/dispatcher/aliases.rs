use std::collections::HashMap;
use std::collections::hash_map::Entry;

use super::Dispatcher;
use crate::binding::BindingId;
use crate::hotkey::Hotkey;
use crate::hotkey::HotkeySequence;
use crate::hotkey::Modifier;
use crate::hotkey::ModifierAlias;
use crate::hotkey::ModifierAliases;

impl Dispatcher {
    /// Define or reassign a modifier alias.
    ///
    /// Aliases let users define abstract modifier names like `"Mod"` that
    /// resolve to concrete modifiers at match time. This enables portable
    /// bindings — a tiling WM can define `"Mod"` as `Super` and let users
    /// rebind it to `Alt` without changing any hotkey definitions.
    ///
    /// Alias names are stored case-insensitively. Defining `"Mod"` and
    /// `"mod"` refers to the same alias.
    ///
    /// Reassigning an existing alias updates resolution for all bindings
    /// that use it — no re-registration needed.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidAliasTarget`](crate::error::Error::InvalidAliasTarget)
    /// if `target` is [`Modifier::Alias`] — alias chaining is not supported.
    /// Returns [`Error::AliasConflict`](crate::error::Error::AliasConflict)
    /// if the new alias mapping would make any global bindings ambiguous.
    pub fn define_modifier_alias(
        &mut self,
        name: impl Into<String>,
        target: Modifier,
    ) -> Result<(), crate::error::Error> {
        if matches!(target, Modifier::Alias(_)) {
            return Err(crate::error::Error::InvalidAliasTarget);
        }

        let mut modifier_aliases = self.modifier_aliases.clone();
        modifier_aliases.insert(ModifierAlias::new(name), target);
        let binding_ids_by_resolved_hotkey =
            self.rebuild_binding_ids_by_resolved_hotkey(&modifier_aliases)?;

        self.modifier_aliases = modifier_aliases;
        self.binding_ids_by_resolved_hotkey = binding_ids_by_resolved_hotkey;
        Ok(())
    }

    /// Resolve all modifiers in a hotkey, returning a new hotkey with
    /// concrete modifiers only.
    ///
    /// Returns `None` if any alias is undefined.
    pub(crate) fn resolve_hotkey(&self, hotkey: &Hotkey) -> Option<Hotkey> {
        resolve_hotkey_with_aliases(hotkey, &self.modifier_aliases)
    }

    /// Rebuild the alias-resolved lookup table for global bindings.
    ///
    /// Called whenever an alias is defined or reassigned. Returns a new
    /// alias-resolved lookup table if the alias configuration is conflict-free.
    fn rebuild_binding_ids_by_resolved_hotkey(
        &self,
        aliases: &ModifierAliases,
    ) -> Result<HashMap<Hotkey, BindingId>, crate::error::Error> {
        let mut binding_ids_by_resolved_hotkey = HashMap::new();
        let mut effective_ids = HashMap::new();

        for binding in self.bindings_by_id.values() {
            let Some(resolved) = resolve_hotkey_with_aliases(binding.hotkey(), aliases) else {
                continue;
            };

            match effective_ids.entry(resolved.clone()) {
                Entry::Vacant(entry) => {
                    entry.insert(binding.id());
                }
                Entry::Occupied(entry) if *entry.get() != binding.id() => {
                    return Err(crate::error::Error::AliasConflict);
                }
                Entry::Occupied(_) => {}
            }

            if has_alias_modifiers(binding.hotkey()) {
                binding_ids_by_resolved_hotkey.insert(resolved, binding.id());
            }
        }

        let mut effective_sequence_ids = HashMap::new();
        for binding in self.sequence_bindings_by_id.values() {
            let Some(resolved) = resolve_sequence_with_aliases(&binding.sequence, aliases) else {
                continue;
            };

            match effective_sequence_ids.entry(resolved) {
                Entry::Vacant(entry) => {
                    entry.insert(binding.id);
                }
                Entry::Occupied(entry) if *entry.get() != binding.id => {
                    return Err(crate::error::Error::AliasConflict);
                }
                Entry::Occupied(_) => {}
            }
        }

        Ok(binding_ids_by_resolved_hotkey)
    }
}

pub(crate) fn resolve_hotkey_with_aliases(
    hotkey: &Hotkey,
    aliases: &ModifierAliases,
) -> Option<Hotkey> {
    if !has_alias_modifiers(hotkey) {
        return Some(hotkey.clone());
    }

    let mut resolved_modifiers = Vec::with_capacity(hotkey.modifiers().len());
    for modifier in hotkey.modifiers() {
        let resolved = match modifier {
            Modifier::Alias(alias) => aliases.get(alias).cloned()?,
            concrete => concrete.clone(),
        };
        resolved_modifiers.push(resolved);
    }
    Some(Hotkey::with_modifiers(hotkey.key(), resolved_modifiers))
}

pub(crate) fn resolve_sequence_with_aliases(
    sequence: &HotkeySequence,
    aliases: &ModifierAliases,
) -> Option<HotkeySequence> {
    if sequence
        .steps()
        .iter()
        .all(|step| !has_alias_modifiers(step))
    {
        return Some(sequence.clone());
    }

    let mut resolved_steps = Vec::with_capacity(sequence.steps().len());
    for step in sequence.steps() {
        resolved_steps.push(resolve_hotkey_with_aliases(step, aliases)?);
    }
    Some(HotkeySequence::new(resolved_steps).expect("sequence bindings are never empty"))
}

/// Check whether a hotkey contains any modifier aliases.
pub(crate) fn has_alias_modifiers(hotkey: &Hotkey) -> bool {
    hotkey
        .modifiers()
        .iter()
        .any(|m| matches!(m, Modifier::Alias(_)))
}

/// Check whether a hotkey matches another after alias resolution.
///
/// `binding_hotkey` may contain aliases; `event_hotkey` is concrete.
/// `aliases` maps lowercase alias names to concrete modifiers.
pub(crate) fn hotkeys_match_with_aliases(
    binding_hotkey: &Hotkey,
    event_hotkey: &Hotkey,
    aliases: &ModifierAliases,
) -> bool {
    resolve_hotkey_with_aliases(binding_hotkey, aliases).as_ref() == Some(event_hotkey)
}

#[cfg(test)]
mod tests {
    use crate::action::Action;
    use crate::dispatcher::Dispatcher;
    use crate::dispatcher::MatchResult;
    use crate::hotkey::Hotkey;
    use crate::hotkey::Modifier;
    use crate::hotkey::ModifierAlias;
    use crate::key::Key;
    use crate::key_state::KeyTransition;
    use crate::layer::Layer;

    #[test]
    fn programmatic_aliased_hotkey_matches_after_alias_definition() {
        let mut dispatcher = Dispatcher::new();
        dispatcher
            .define_modifier_alias("Mod", Modifier::Super)
            .unwrap();
        dispatcher
            .register(
                Hotkey::new(Key::T).modifier(Modifier::Alias(ModifierAlias::new("Mod"))),
                Action::Suppress,
            )
            .unwrap();

        let result = dispatcher.process(
            &Hotkey::new(Key::T).modifier(Modifier::Super),
            KeyTransition::Press,
        );
        assert!(matches!(result, MatchResult::Matched { .. }));
    }

    #[test]
    fn parse_hotkey_with_alias_modifier() {
        let hotkey: Hotkey = "Mod+T".parse().unwrap();
        assert_eq!(hotkey.key(), Key::T);
        assert_eq!(
            hotkey.modifiers(),
            &[Modifier::Alias(ModifierAlias::new("Mod"))]
        );
    }

    #[test]
    fn parse_hotkey_with_alias_and_concrete_modifier() {
        let hotkey: Hotkey = "Ctrl+Mod+A".parse().unwrap();
        assert_eq!(hotkey.key(), Key::A);
        assert!(hotkey.modifiers().contains(&Modifier::Ctrl));
        assert!(
            hotkey
                .modifiers()
                .contains(&Modifier::Alias(ModifierAlias::new("Mod")))
        );
    }

    #[test]
    fn parsed_aliased_hotkey_resolves_during_matching() {
        let mut dispatcher = Dispatcher::new();
        dispatcher
            .define_modifier_alias("Mod", Modifier::Super)
            .unwrap();

        dispatcher.register("Mod+T", Action::Suppress).unwrap();

        let result = dispatcher.process(
            &Hotkey::new(Key::T).modifier(Modifier::Super),
            KeyTransition::Press,
        );
        assert!(matches!(result, MatchResult::Matched { .. }));
    }

    #[test]
    fn alias_reassignment_updates_matching() {
        let mut dispatcher = Dispatcher::new();
        dispatcher
            .define_modifier_alias("Mod", Modifier::Super)
            .unwrap();
        dispatcher.register("Mod+T", Action::Suppress).unwrap();

        let result = dispatcher.process(
            &Hotkey::new(Key::T).modifier(Modifier::Super),
            KeyTransition::Press,
        );
        assert!(matches!(result, MatchResult::Matched { .. }));

        dispatcher
            .define_modifier_alias("Mod", Modifier::Alt)
            .unwrap();

        let result = dispatcher.process(
            &Hotkey::new(Key::T).modifier(Modifier::Super),
            KeyTransition::Press,
        );
        assert!(matches!(result, MatchResult::NoMatch));

        let result = dispatcher.process(
            &Hotkey::new(Key::T).modifier(Modifier::Alt),
            KeyTransition::Press,
        );
        assert!(matches!(result, MatchResult::Matched { .. }));
    }

    #[test]
    fn unknown_alias_does_not_match() {
        let mut dispatcher = Dispatcher::new();
        dispatcher.register("Mod+T", Action::Suppress).unwrap();

        let result = dispatcher.process(
            &Hotkey::new(Key::T).modifier(Modifier::Super),
            KeyTransition::Press,
        );
        assert!(matches!(result, MatchResult::NoMatch));

        let result = dispatcher.process(
            &Hotkey::new(Key::T).modifier(Modifier::Alt),
            KeyTransition::Press,
        );
        assert!(matches!(result, MatchResult::NoMatch));
    }

    #[test]
    fn alias_works_in_layer_bindings() {
        let mut dispatcher = Dispatcher::new();
        dispatcher
            .define_modifier_alias("Mod", Modifier::Super)
            .unwrap();

        let hotkey: Hotkey = "Mod+H".parse().unwrap();
        let layer = Layer::new("nav").bind(hotkey, Action::Suppress).unwrap();
        dispatcher.define_layer(layer).unwrap();
        dispatcher.push_layer("nav").unwrap();

        let result = dispatcher.process(
            &Hotkey::new(Key::H).modifier(Modifier::Super),
            KeyTransition::Press,
        );
        assert!(matches!(result, MatchResult::Matched { .. }));
    }

    #[test]
    fn modifier_alias_as_str_returns_name() {
        let alias = ModifierAlias::new("Mod");
        assert_eq!(alias.as_str(), "Mod");
    }

    #[test]
    fn modifier_alias_as_str_preserves_original_case() {
        let alias = ModifierAlias::new("MyMod");
        assert_eq!(alias.as_str(), "MyMod");
    }

    #[test]
    fn hotkey_display_preserves_alias_name() {
        let hotkey = Hotkey::new(Key::T).modifier(Modifier::Alias(ModifierAlias::new("Mod")));
        assert_eq!(hotkey.to_string(), "Mod+T");
    }

    #[test]
    fn concrete_bindings_are_unaffected_by_alias_definitions() {
        let mut dispatcher = Dispatcher::new();
        dispatcher
            .define_modifier_alias("Mod", Modifier::Super)
            .unwrap();

        dispatcher
            .register(
                Hotkey::new(Key::C).modifier(Modifier::Ctrl),
                Action::Suppress,
            )
            .unwrap();

        let result = dispatcher.process(
            &Hotkey::new(Key::C).modifier(Modifier::Ctrl),
            KeyTransition::Press,
        );
        assert!(matches!(result, MatchResult::Matched { .. }));
    }

    #[test]
    fn alias_combined_with_concrete_modifiers() {
        let mut dispatcher = Dispatcher::new();
        dispatcher
            .define_modifier_alias("Mod", Modifier::Super)
            .unwrap();

        dispatcher.register("Ctrl+Mod+A", Action::Suppress).unwrap();

        let result = dispatcher.process(
            &Hotkey::new(Key::A)
                .modifier(Modifier::Ctrl)
                .modifier(Modifier::Super),
            KeyTransition::Press,
        );
        assert!(matches!(result, MatchResult::Matched { .. }));
    }

    #[test]
    fn introspection_shows_alias_bindings() {
        let mut dispatcher = Dispatcher::new();
        dispatcher
            .define_modifier_alias("Mod", Modifier::Super)
            .unwrap();
        dispatcher.register("Mod+T", Action::Suppress).unwrap();

        let bindings = dispatcher.list_bindings();
        assert_eq!(bindings.len(), 1);
        assert!(
            bindings[0]
                .hotkey
                .modifiers()
                .contains(&Modifier::Alias(ModifierAlias::new("Mod")))
        );
    }

    #[test]
    fn alias_reassignment_updates_active_layer_binding_matching() {
        let mut dispatcher = Dispatcher::new();
        dispatcher
            .define_modifier_alias("Mod", Modifier::Super)
            .unwrap();

        let hotkey: Hotkey = "Mod+S".parse().unwrap();
        let layer = Layer::new("edit").bind(hotkey, Action::Suppress).unwrap();
        dispatcher.define_layer(layer).unwrap();
        dispatcher.push_layer("edit").unwrap();

        let result = dispatcher.process(
            &Hotkey::new(Key::S).modifier(Modifier::Super),
            KeyTransition::Press,
        );
        assert!(matches!(result, MatchResult::Matched { .. }));

        dispatcher
            .define_modifier_alias("Mod", Modifier::Alt)
            .unwrap();

        let result = dispatcher.process(
            &Hotkey::new(Key::S).modifier(Modifier::Super),
            KeyTransition::Press,
        );
        assert!(
            matches!(result, MatchResult::NoMatch),
            "Super+S should not match after alias reassigned to Alt"
        );

        let result = dispatcher.process(
            &Hotkey::new(Key::S).modifier(Modifier::Alt),
            KeyTransition::Press,
        );
        assert!(matches!(result, MatchResult::Matched { .. }));
    }

    #[test]
    fn define_modifier_alias_rejects_alias_target() {
        let mut dispatcher = Dispatcher::new();
        let result =
            dispatcher.define_modifier_alias("Mod", Modifier::Alias(ModifierAlias::new("Other")));
        assert!(matches!(
            result,
            Err(crate::error::Error::InvalidAliasTarget)
        ));
    }

    #[test]
    fn define_modifier_alias_rejects_conflicting_global_bindings() {
        let mut dispatcher = Dispatcher::new();
        dispatcher.register("Mod+T", Action::Suppress).unwrap();
        dispatcher
            .register(
                Hotkey::new(Key::T).modifier(Modifier::Super),
                Action::Suppress,
            )
            .unwrap();

        let result = dispatcher.define_modifier_alias("Mod", Modifier::Super);
        assert!(matches!(result, Err(crate::error::Error::AliasConflict)));

        let mod_binding: Hotkey = "Mod+T".parse().unwrap();
        assert!(dispatcher.is_registered(&mod_binding));
        assert!(dispatcher.is_registered(&Hotkey::new(Key::T).modifier(Modifier::Super)));
    }

    #[test]
    fn failed_alias_reassignment_keeps_previous_mapping() {
        let mut dispatcher = Dispatcher::new();
        dispatcher
            .define_modifier_alias("Mod", Modifier::Super)
            .unwrap();
        dispatcher.register("Mod+T", Action::Suppress).unwrap();
        dispatcher
            .register(
                Hotkey::new(Key::T).modifier(Modifier::Alt),
                Action::Suppress,
            )
            .unwrap();

        let result = dispatcher.define_modifier_alias("Mod", Modifier::Alt);
        assert!(matches!(result, Err(crate::error::Error::AliasConflict)));

        let result = dispatcher.process(
            &Hotkey::new(Key::T).modifier(Modifier::Super),
            KeyTransition::Press,
        );
        assert!(matches!(result, MatchResult::Matched { .. }));

        let result = dispatcher.process(
            &Hotkey::new(Key::T).modifier(Modifier::Alt),
            KeyTransition::Press,
        );
        assert!(matches!(result, MatchResult::Matched { .. }));
    }

    #[test]
    fn alias_reassignment_rejects_conflicting_sequences() {
        let mut dispatcher = Dispatcher::new();
        dispatcher
            .register_sequence("Mod+K, Ctrl+C", Action::Suppress)
            .unwrap();
        dispatcher
            .register_sequence("Super+K, Ctrl+C", Action::Suppress)
            .unwrap();

        let result = dispatcher.define_modifier_alias("Mod", Modifier::Super);
        assert!(matches!(result, Err(crate::error::Error::AliasConflict)));
    }

    #[test]
    fn alias_works_in_global_sequences() {
        let mut dispatcher = Dispatcher::new();
        dispatcher
            .define_modifier_alias("Mod", Modifier::Super)
            .unwrap();
        dispatcher
            .register_sequence("Mod+K, Ctrl+C", Action::Suppress)
            .unwrap();

        let result = dispatcher.process(
            &Hotkey::new(Key::K).modifier(Modifier::Super),
            KeyTransition::Press,
        );
        assert!(matches!(
            result,
            MatchResult::Pending {
                steps_matched: 1,
                steps_remaining: 1,
            }
        ));

        let result = dispatcher.process(
            &Hotkey::new(Key::C).modifier(Modifier::Ctrl),
            KeyTransition::Press,
        );
        assert!(matches!(result, MatchResult::Matched { .. }));
    }

    #[test]
    fn alias_works_in_layer_sequences() {
        let mut dispatcher = Dispatcher::new();
        dispatcher
            .define_modifier_alias("Mod", Modifier::Super)
            .unwrap();
        let layer = Layer::new("nav")
            .bind_sequence("Mod+K, Ctrl+C", Action::Suppress)
            .unwrap();
        dispatcher.define_layer(layer).unwrap();
        dispatcher.push_layer("nav").unwrap();

        let result = dispatcher.process(
            &Hotkey::new(Key::K).modifier(Modifier::Super),
            KeyTransition::Press,
        );
        assert!(matches!(
            result,
            MatchResult::Pending {
                steps_matched: 1,
                steps_remaining: 1,
            }
        ));

        let result = dispatcher.process(
            &Hotkey::new(Key::C).modifier(Modifier::Ctrl),
            KeyTransition::Press,
        );
        assert!(matches!(result, MatchResult::Matched { .. }));
    }
}
