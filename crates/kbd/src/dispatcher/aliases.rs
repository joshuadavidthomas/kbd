use std::collections::HashMap;

use super::Dispatcher;
use crate::hotkey::Hotkey;
use crate::hotkey::Modifier;

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
    pub fn define_modifier_alias(
        &mut self,
        name: impl Into<String>,
        target: Modifier,
    ) -> Result<(), crate::error::Error> {
        if matches!(target, Modifier::Alias(_)) {
            return Err(crate::error::Error::InvalidAliasTarget);
        }
        let name = name.into();
        self.modifier_aliases
            .insert(name.to_ascii_lowercase(), target);
        self.rebuild_alias_resolved_ids();
        Ok(())
    }

    /// Resolve a single modifier: if it's an alias, look it up; otherwise return as-is.
    ///
    /// Returns `None` if the alias is undefined (no mapping exists).
    pub(crate) fn resolve_modifier(&self, modifier: &Modifier) -> Option<Modifier> {
        match modifier {
            Modifier::Alias(alias) => self
                .modifier_aliases
                .get(&alias.as_str().to_ascii_lowercase())
                .cloned(),
            concrete => Some(concrete.clone()),
        }
    }

    /// Resolve all modifiers in a hotkey, returning a new hotkey with
    /// concrete modifiers only.
    ///
    /// Returns `None` if any alias is undefined.
    pub(crate) fn resolve_hotkey(&self, hotkey: &Hotkey) -> Option<Hotkey> {
        if !hotkey
            .modifiers()
            .iter()
            .any(|m| matches!(m, Modifier::Alias(_)))
        {
            return Some(hotkey.clone());
        }

        let mut resolved_modifiers = Vec::with_capacity(hotkey.modifiers().len());
        for modifier in hotkey.modifiers() {
            match self.resolve_modifier(modifier) {
                Some(concrete) => resolved_modifiers.push(concrete),
                None => return None,
            }
        }
        Some(Hotkey::with_modifiers(hotkey.key(), resolved_modifiers))
    }

    /// Rebuild the alias-resolved lookup table for global bindings.
    ///
    /// Called whenever an alias is defined or reassigned. Iterates all
    /// global bindings that contain aliases and (re-)inserts their
    /// resolved forms into the lookup table.
    fn rebuild_alias_resolved_ids(&mut self) {
        self.alias_resolved_ids.clear();

        let entries: Vec<_> = self
            .bindings_by_id
            .iter()
            .filter(|(_, binding)| has_alias_modifiers(binding.hotkey()))
            .map(|(_, binding)| (binding.id(), binding.hotkey().clone()))
            .collect();

        for (id, hotkey) in entries {
            if let Some(resolved) = self.resolve_hotkey(&hotkey) {
                self.alias_resolved_ids.insert(resolved, id);
            }
        }
    }
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
    aliases: &HashMap<String, Modifier>,
) -> bool {
    if binding_hotkey.key() != event_hotkey.key() {
        return false;
    }

    // If the binding has no aliases, use direct comparison
    if !has_alias_modifiers(binding_hotkey) {
        return binding_hotkey.modifiers() == event_hotkey.modifiers();
    }

    // Resolve aliases in the binding's modifiers
    let mut resolved: Vec<Modifier> = Vec::with_capacity(binding_hotkey.modifiers().len());
    for modifier in binding_hotkey.modifiers() {
        match modifier {
            Modifier::Alias(alias) => {
                if let Some(concrete) = aliases.get(&alias.as_str().to_ascii_lowercase()) {
                    resolved.push(concrete.clone());
                } else {
                    // Unknown alias — can't match
                    return false;
                }
            }
            concrete => resolved.push(concrete.clone()),
        }
    }
    resolved.sort();
    resolved.dedup();

    resolved == event_hotkey.modifiers()
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
    fn define_modifier_alias_and_match() {
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
    fn alias_resolution_during_matching_not_parsing() {
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
    fn alias_defined_on_dispatcher_directly() {
        let mut dispatcher = Dispatcher::new();
        dispatcher
            .define_modifier_alias("Leader", Modifier::Alt)
            .unwrap();

        dispatcher.register("Leader+X", Action::Suppress).unwrap();

        let result = dispatcher.process(
            &Hotkey::new(Key::X).modifier(Modifier::Alt),
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
    fn modifier_alias_display() {
        let alias = ModifierAlias::new("Mod");
        assert_eq!(alias.as_str(), "Mod");
    }

    #[test]
    fn modifier_alias_preserves_case() {
        let alias = ModifierAlias::new("MyMod");
        assert_eq!(alias.as_str(), "MyMod");
    }

    #[test]
    fn hotkey_display_with_alias() {
        let hotkey = Hotkey::new(Key::T).modifier(Modifier::Alias(ModifierAlias::new("Mod")));
        let display = hotkey.to_string();
        assert!(display.contains("Mod"));
        assert!(display.contains('T'));
    }

    #[test]
    fn concrete_modifier_bindings_still_work_with_aliases_defined() {
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
    fn alias_reassignment_updates_layer_matching() {
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
}
