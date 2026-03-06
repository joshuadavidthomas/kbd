use super::Dispatcher;
use super::MatchedBindingRef;
use super::sequence::RegisteredSequenceBinding;
use super::sequence::SequenceBindingRef;
use crate::action::Action;
use crate::binding::BindingId;
use crate::binding::RegisteredBinding;
use crate::hotkey::Hotkey;
use crate::hotkey::HotkeyInput;
use crate::hotkey::HotkeySequence;
use crate::sequence::SequenceInput;
use crate::sequence::SequenceOptions;

impl Dispatcher {
    /// Register a binding. Returns the assigned [`BindingId`].
    ///
    /// Accepts any type implementing [`HotkeyInput`]: a [`Hotkey`], a
    /// [`Key`](crate::key::Key), or a string (`&str` / `String`).
    ///
    /// # Errors
    ///
    /// Returns [`Error::Parse`](crate::error::Error::Parse) when string
    /// input conversion fails, or
    /// [`Error::AlreadyRegistered`](crate::error::Error::AlreadyRegistered)
    /// if a binding for the same hotkey exists.
    pub fn register(
        &mut self,
        hotkey: impl HotkeyInput,
        action: impl Into<Action>,
    ) -> Result<BindingId, crate::error::Error> {
        let id = BindingId::new();
        let hotkey = hotkey.into_hotkey()?;
        let binding = RegisteredBinding::new(id, hotkey, action.into());
        self.register_binding(binding)?;
        Ok(id)
    }

    /// Register a multi-step sequence binding with default sequence options.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Parse`](crate::error::Error::Parse) when sequence input
    /// conversion fails, or
    /// [`Error::AlreadyRegistered`](crate::error::Error::AlreadyRegistered)
    /// if a binding for the same sequence already exists.
    pub fn register_sequence(
        &mut self,
        sequence: impl SequenceInput,
        action: impl Into<Action>,
    ) -> Result<BindingId, crate::error::Error> {
        self.register_sequence_with_options(sequence, action, SequenceOptions::default())
    }

    /// Register a sequence with explicit sequence options.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Parse`](crate::error::Error::Parse) when sequence input
    /// conversion fails, or
    /// [`Error::AlreadyRegistered`](crate::error::Error::AlreadyRegistered)
    /// if a binding for the same sequence already exists.
    pub fn register_sequence_with_options(
        &mut self,
        sequence: impl SequenceInput,
        action: impl Into<Action>,
        options: SequenceOptions,
    ) -> Result<BindingId, crate::error::Error> {
        let id = BindingId::new();
        let sequence = sequence.into_sequence()?;
        self.register_sequence_binding_with_id(id, sequence, action.into(), options)?;
        Ok(id)
    }

    /// Register a sequence binding with a caller-provided binding ID.
    ///
    /// # Errors
    ///
    /// Returns [`Error::AlreadyRegistered`](crate::error::Error::AlreadyRegistered)
    /// if a binding for the same sequence already exists.
    pub(crate) fn register_sequence_binding_with_id(
        &mut self,
        id: BindingId,
        sequence: HotkeySequence,
        action: Action,
        options: SequenceOptions,
    ) -> Result<(), crate::error::Error> {
        let binding = RegisteredSequenceBinding::new(id, sequence, action, options);
        self.register_sequence_binding(binding)
    }

    /// Register a [`RegisteredBinding`] with full options control.
    ///
    /// # Errors
    ///
    /// Returns [`Error::AlreadyRegistered`](crate::error::Error::AlreadyRegistered)
    /// if a binding for the same hotkey exists.
    pub fn register_binding(
        &mut self,
        binding: RegisteredBinding,
    ) -> Result<(), crate::error::Error> {
        let id = binding.id();
        let hotkey = binding.hotkey().clone();
        let has_aliases = super::aliases::has_alias_modifiers(&hotkey);

        if self.bindings_by_id.contains_key(&id)
            || self.sequence_bindings_by_id.contains_key(&id)
            || self.binding_ids_by_hotkey.contains_key(&hotkey)
            || self.alias_resolved_ids.contains_key(&hotkey)
        {
            return Err(crate::error::Error::AlreadyRegistered);
        }

        // For aliased bindings, check the resolved form for conflicts and
        // insert into the resolved lookup table. If the alias is undefined,
        // the binding won't match until the alias is defined (at which point
        // rebuild_alias_resolved_ids will add it).
        if has_aliases {
            if let Some(resolved) = self.resolve_hotkey(&hotkey) {
                if self.binding_ids_by_hotkey.contains_key(&resolved)
                    || self.alias_resolved_ids.contains_key(&resolved)
                {
                    return Err(crate::error::Error::AlreadyRegistered);
                }
                self.alias_resolved_ids.insert(resolved, id);
            }
        }
        self.binding_ids_by_hotkey.insert(hotkey, id);

        self.bindings_by_id.insert(id, binding);
        Ok(())
    }

    fn register_sequence_binding(
        &mut self,
        binding: RegisteredSequenceBinding,
    ) -> Result<(), crate::error::Error> {
        let id = binding.id;
        let sequence = binding.sequence.clone();

        if self.sequence_bindings_by_id.contains_key(&id)
            || self.bindings_by_id.contains_key(&id)
            || self.sequence_ids_by_value.contains_key(&sequence)
        {
            return Err(crate::error::Error::AlreadyRegistered);
        }

        self.sequence_ids_by_value.insert(sequence, id);
        self.sequence_bindings_by_id.insert(id, binding);
        Ok(())
    }

    /// Unregister a binding by its [`BindingId`].
    pub fn unregister(&mut self, id: BindingId) {
        if let Some(binding) = self.bindings_by_id.remove(&id) {
            self.binding_ids_by_hotkey.remove(binding.hotkey());
            // Also remove from alias-resolved lookup
            self.alias_resolved_ids.retain(|_, v| *v != id);
        }

        if let Some(binding) = self.sequence_bindings_by_id.remove(&id) {
            self.sequence_ids_by_value.remove(&binding.sequence);
        }

        self.active_sequences
            .retain(|active| !matches!(active.binding_ref, SequenceBindingRef::Global(global_id) if global_id == id));

        if self.pending_standalone.as_ref().is_some_and(|pending| {
            matches!(
                pending.binding_ref,
                MatchedBindingRef::Global(global_id) | MatchedBindingRef::SequenceGlobal(global_id)
                    if global_id == id
            )
        }) {
            self.pending_standalone = None;
        }

        if self.active_sequences.is_empty() {
            self.pending_standalone = None;
        }
    }

    /// Check whether a hotkey has a registered global binding.
    ///
    /// For aliased bindings, checks the original (unresolved) hotkey.
    #[must_use]
    pub fn is_registered(&self, hotkey: &Hotkey) -> bool {
        self.binding_ids_by_hotkey.contains_key(hotkey)
    }
}

#[cfg(test)]
mod tests {
    use super::super::Dispatcher;
    use crate::action::Action;
    use crate::binding::BindingId;
    use crate::binding::RegisteredBinding;
    use crate::hotkey::Hotkey;
    use crate::hotkey::HotkeySequence;
    use crate::hotkey::Modifier;
    use crate::key::Key;
    use crate::sequence::SequenceOptions;

    #[test]
    fn register_returns_unique_id() {
        let mut dispatcher = Dispatcher::new();
        let id1 = dispatcher
            .register(Hotkey::new(Key::A), Action::Suppress)
            .unwrap();
        let id2 = dispatcher
            .register(Hotkey::new(Key::B), Action::Suppress)
            .unwrap();
        assert_ne!(id1, id2);
    }

    #[test]
    fn is_registered_reflects_state() {
        let mut dispatcher = Dispatcher::new();
        let hotkey = Hotkey::new(Key::C).modifier(Modifier::Ctrl);
        assert!(!dispatcher.is_registered(&hotkey));

        dispatcher
            .register(hotkey.clone(), Action::Suppress)
            .unwrap();
        assert!(dispatcher.is_registered(&hotkey));
    }

    #[test]
    fn register_sequence_accepts_typed_sequence() {
        let mut dispatcher = Dispatcher::new();
        let sequence = HotkeySequence::new(vec![
            Hotkey::new(Key::K).modifier(Modifier::Ctrl),
            Hotkey::new(Key::C).modifier(Modifier::Ctrl),
        ])
        .unwrap();

        let id = dispatcher
            .register_sequence(sequence, Action::Suppress)
            .unwrap();
        dispatcher.unregister(id);
    }

    #[test]
    fn register_sequence_accepts_string_input() {
        let mut dispatcher = Dispatcher::new();

        let id = dispatcher
            .register_sequence("Ctrl+K, Ctrl+C", Action::Suppress)
            .unwrap();

        dispatcher.unregister(id);
    }

    #[test]
    fn register_sequence_accepts_vec_hotkeys_input() {
        let mut dispatcher = Dispatcher::new();

        let id = dispatcher
            .register_sequence(
                vec![
                    Hotkey::new(Key::K).modifier(Modifier::Ctrl),
                    Hotkey::new(Key::C).modifier(Modifier::Ctrl),
                ],
                Action::Suppress,
            )
            .unwrap();

        dispatcher.unregister(id);
    }

    #[test]
    fn register_sequence_reports_parse_error_for_string_input() {
        let mut dispatcher = Dispatcher::new();

        let result = dispatcher.register_sequence("Ctrl+K, Ctrl+@@@", Action::Suppress);

        assert!(matches!(result, Err(crate::error::Error::Parse(_))));
    }

    #[test]
    fn registering_sequence_with_existing_hotkey_id_is_rejected() {
        let mut dispatcher = Dispatcher::new();
        let id = BindingId::new();

        dispatcher
            .register_binding(RegisteredBinding::new(
                id,
                Hotkey::new(Key::A),
                Action::Suppress,
            ))
            .unwrap();

        let result = dispatcher.register_sequence_binding_with_id(
            id,
            "Ctrl+K, Ctrl+C".parse::<HotkeySequence>().unwrap(),
            Action::Suppress,
            SequenceOptions::default(),
        );

        assert!(matches!(
            result,
            Err(crate::error::Error::AlreadyRegistered)
        ));
    }

    #[test]
    fn registering_hotkey_with_existing_sequence_id_is_rejected() {
        let mut dispatcher = Dispatcher::new();
        let id = BindingId::new();

        dispatcher
            .register_sequence_binding_with_id(
                id,
                "Ctrl+K, Ctrl+C".parse::<HotkeySequence>().unwrap(),
                Action::Suppress,
                SequenceOptions::default(),
            )
            .unwrap();

        let result = dispatcher.register_binding(RegisteredBinding::new(
            id,
            Hotkey::new(Key::A),
            Action::Suppress,
        ));

        assert!(matches!(
            result,
            Err(crate::error::Error::AlreadyRegistered)
        ));
    }

    #[test]
    fn register_accepts_string_input() {
        let mut dispatcher = Dispatcher::new();
        let id = dispatcher.register("Ctrl+A", Action::Suppress).unwrap();
        assert!(dispatcher.is_registered(&Hotkey::new(Key::A).modifier(Modifier::Ctrl)));
        dispatcher.unregister(id);
    }

    #[test]
    fn register_accepts_key_input() {
        let mut dispatcher = Dispatcher::new();
        let id = dispatcher.register(Key::ESCAPE, Action::Suppress).unwrap();
        assert!(dispatcher.is_registered(&Hotkey::new(Key::ESCAPE)));
        dispatcher.unregister(id);
    }

    #[test]
    fn register_reports_parse_error_for_invalid_string() {
        let mut dispatcher = Dispatcher::new();
        let result = dispatcher.register("Ctrl+@@@", Action::Suppress);
        assert!(matches!(result, Err(crate::error::Error::Parse(_))));
    }

    #[test]
    fn concrete_binding_conflicts_with_alias_resolved_form() {
        let mut dispatcher = Dispatcher::new();
        dispatcher
            .define_modifier_alias("Mod", Modifier::Super)
            .unwrap();

        // Register aliased Mod+T (resolves to Super+T)
        dispatcher.register("Mod+T", Action::Suppress).unwrap();

        // Registering concrete Super+T must be rejected — it conflicts
        // with the alias-resolved form
        let result = dispatcher.register(
            Hotkey::new(Key::T).modifier(Modifier::Super),
            Action::Suppress,
        );
        assert!(matches!(
            result,
            Err(crate::error::Error::AlreadyRegistered)
        ));
    }

    #[test]
    fn is_registered_finds_aliased_binding() {
        let mut dispatcher = Dispatcher::new();
        dispatcher
            .define_modifier_alias("Mod", Modifier::Super)
            .unwrap();
        dispatcher.register("Mod+T", Action::Suppress).unwrap();

        let aliased_hotkey = "Mod+T".parse::<Hotkey>().unwrap();
        assert!(dispatcher.is_registered(&aliased_hotkey));
    }
}
