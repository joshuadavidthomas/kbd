use super::Dispatcher;
use super::MatchedBindingRef;
use super::sequence::RegisteredSequenceBinding;
use super::sequence::SequenceBindingRef;
use crate::action::Action;
use crate::binding::BindingId;
use crate::binding::BindingOptions;
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
    /// if a binding for the same hotkey already exists in the standard
    /// precedence tier.
    pub fn register(
        &mut self,
        hotkey: impl HotkeyInput,
        action: impl Into<Action>,
    ) -> Result<BindingId, crate::error::Error> {
        self.register_with_options(hotkey, action, BindingOptions::default())
    }

    /// Register a binding with explicit [`BindingOptions`]. Returns the assigned [`BindingId`].
    ///
    /// Use this when you want binding metadata like descriptions, provenance,
    /// or overlay visibility without constructing a low-level
    /// [`RegisteredBinding`] yourself.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Parse`](crate::error::Error::Parse) when string
    /// input conversion fails, or
    /// [`Error::AlreadyRegistered`](crate::error::Error::AlreadyRegistered)
    /// if a binding for the same hotkey already exists in the same precedence
    /// tier.
    pub fn register_with_options(
        &mut self,
        hotkey: impl HotkeyInput,
        action: impl Into<Action>,
        options: BindingOptions,
    ) -> Result<BindingId, crate::error::Error> {
        let id = BindingId::new();
        let hotkey = hotkey.into_hotkey()?;
        let binding = RegisteredBinding::new(id, hotkey, action.into()).with_options(options);
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
    /// if a binding for the same hotkey already exists in the same precedence
    /// tier and device scope. Bindings with different device filters (or one
    /// with a filter and one without) can coexist for the same hotkey and tier.
    pub fn register_binding(
        &mut self,
        binding: RegisteredBinding,
    ) -> Result<(), crate::error::Error> {
        let id = binding.id();
        let hotkey = binding.hotkey().clone();
        let new_tier = binding.options().precedence_tier();
        let new_device = binding.options().device();

        if self.bindings_by_id.contains_key(&id) || self.sequence_bindings_by_id.contains_key(&id) {
            return Err(crate::error::Error::AlreadyRegistered);
        }

        let ids_for_hotkey = self.binding_ids_by_hotkey.entry(hotkey).or_default();
        if ids_for_hotkey.iter().any(|existing_id| {
            self.bindings_by_id
                .get(existing_id)
                .is_some_and(|existing| {
                    existing.options().precedence_tier() == new_tier
                        && existing.options().device() == new_device
                })
        }) {
            return Err(crate::error::Error::AlreadyRegistered);
        }

        let insert_at = ids_for_hotkey
            .iter()
            .position(|existing_id| {
                self.bindings_by_id
                    .get(existing_id)
                    .is_some_and(|existing| existing.options().precedence_tier() > new_tier)
            })
            .unwrap_or(ids_for_hotkey.len());

        ids_for_hotkey.insert(insert_at, id);
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
        self.throttle_tracker.remove_global(id);
        if let Some(binding) = self.bindings_by_id.remove(&id) {
            let hotkey = binding.hotkey().clone();
            let remove_hotkey_entry =
                if let Some(ids_for_hotkey) = self.binding_ids_by_hotkey.get_mut(&hotkey) {
                    ids_for_hotkey.retain(|existing_id| *existing_id != id);
                    ids_for_hotkey.is_empty()
                } else {
                    false
                };

            if remove_hotkey_entry {
                self.binding_ids_by_hotkey.remove(&hotkey);
            }
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

        let result = dispatcher.register_sequence("Ctrl+K, Ctrl+Nope", Action::Suppress);

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
    fn register_with_options_returns_id_and_stores_metadata() {
        let mut dispatcher = Dispatcher::new();
        let id = dispatcher
            .register_with_options(
                Hotkey::new(Key::C),
                Action::Suppress,
                crate::binding::BindingOptions::default()
                    .with_description("Copy")
                    .with_source("user"),
            )
            .unwrap();

        let binding = dispatcher
            .bindings_for_key(&Hotkey::new(Key::C))
            .expect("binding should be queryable after registration");
        assert_eq!(binding.description.as_deref(), Some("Copy"));
        assert_eq!(
            binding
                .source
                .as_ref()
                .map(crate::binding::BindingSource::as_str),
            Some("user")
        );

        dispatcher.unregister(id);
        assert!(!dispatcher.is_registered(&Hotkey::new(Key::C)));
    }

    #[test]
    fn register_with_options_rejects_same_standard_tier_hotkey() {
        let mut dispatcher = Dispatcher::new();
        dispatcher
            .register_with_options(
                Hotkey::new(Key::A),
                Action::Suppress,
                crate::binding::BindingOptions::default().with_source("plugin"),
            )
            .unwrap();

        let result = dispatcher.register(Hotkey::new(Key::A), Action::Suppress);
        assert!(matches!(
            result,
            Err(crate::error::Error::AlreadyRegistered)
        ));
    }

    #[test]
    fn register_reports_parse_error_for_invalid_string() {
        let mut dispatcher = Dispatcher::new();
        let result = dispatcher.register("Ctrl+Nope", Action::Suppress);
        assert!(matches!(result, Err(crate::error::Error::Parse(_))));
    }

    #[test]
    fn register_allows_device_filtered_and_global_for_same_hotkey() {
        let mut dispatcher = Dispatcher::new();

        // Device-filtered binding
        dispatcher
            .register_with_options(
                Hotkey::new(Key::A),
                Action::Suppress,
                crate::binding::BindingOptions::default()
                    .with_device(crate::device::DeviceFilter::name_contains("StreamDeck")),
            )
            .unwrap();

        // Global binding for same hotkey — should succeed
        let result = dispatcher.register(Hotkey::new(Key::A), Action::Suppress);
        assert!(result.is_ok());
    }

    #[test]
    fn register_rejects_duplicate_device_filter_for_same_hotkey() {
        let mut dispatcher = Dispatcher::new();

        dispatcher
            .register_with_options(
                Hotkey::new(Key::A),
                Action::Suppress,
                crate::binding::BindingOptions::default()
                    .with_device(crate::device::DeviceFilter::name_contains("StreamDeck")),
            )
            .unwrap();

        // Same device filter, same hotkey, same tier — rejected
        let result = dispatcher.register_with_options(
            Hotkey::new(Key::A),
            Action::Suppress,
            crate::binding::BindingOptions::default()
                .with_device(crate::device::DeviceFilter::name_contains("StreamDeck")),
        );
        assert!(matches!(
            result,
            Err(crate::error::Error::AlreadyRegistered)
        ));
    }

    #[test]
    fn register_allows_different_device_filters_for_same_hotkey() {
        let mut dispatcher = Dispatcher::new();

        dispatcher
            .register_with_options(
                Hotkey::new(Key::A),
                Action::Suppress,
                crate::binding::BindingOptions::default()
                    .with_device(crate::device::DeviceFilter::name_contains("StreamDeck")),
            )
            .unwrap();

        // Different device filter, same hotkey, same tier — allowed
        let result = dispatcher.register_with_options(
            Hotkey::new(Key::A),
            Action::Suppress,
            crate::binding::BindingOptions::default()
                .with_device(crate::device::DeviceFilter::usb(0x1234, 0x5678)),
        );
        assert!(result.is_ok());
    }
}
