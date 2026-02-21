use std::collections::HashMap;

use crate::error::Error;
use crate::key::Key;
use crate::key::Modifier;
use crate::manager::attach_hotkey_events;
use crate::manager::normalize_modifiers;
use crate::manager::HotkeyKey;
use crate::manager::HotkeyOptions;
use crate::manager::HotkeyRegistration;

use super::controller::ModeController;

pub struct ModeBuilder {
    pub(crate) bindings: HashMap<HotkeyKey, HotkeyRegistration>,
    controller: ModeController,
}

impl ModeBuilder {
    pub(crate) fn new(controller: ModeController) -> Self {
        Self {
            bindings: HashMap::new(),
            controller,
        }
    }

    pub fn register<F>(
        &mut self,
        key: Key,
        modifiers: &[Modifier],
        callback: F,
    ) -> Result<(), Error>
    where
        F: Fn() + Send + Sync + 'static,
    {
        self.register_with_options(key, modifiers, HotkeyOptions::new(), callback)
    }

    pub fn register_with_options<F>(
        &mut self,
        key: Key,
        modifiers: &[Modifier],
        options: HotkeyOptions,
        callback: F,
    ) -> Result<(), Error>
    where
        F: Fn() + Send + Sync + 'static,
    {
        let hotkey_key = (key, normalize_modifiers(modifiers));

        if self.bindings.contains_key(&hotkey_key) {
            return Err(Error::AlreadyRegistered {
                key: hotkey_key.0,
                modifiers: hotkey_key.1,
            });
        }

        let press_timing = options.press_timing_config();
        let callbacks = attach_hotkey_events(
            options.build_callbacks(callback),
            &hotkey_key,
            &self.controller.registry.event_hub,
            press_timing,
        );

        let registration = HotkeyRegistration { callbacks };

        self.bindings.insert(hotkey_key, registration);
        Ok(())
    }

    #[must_use]
    pub fn mode_controller(&self) -> ModeController {
        self.controller.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mode::registry::ModeRegistry;

    #[test]
    fn mode_builder_collects_bindings() {
        let registry = ModeRegistry::new();
        let controller = ModeController::new(registry);
        let mut builder = ModeBuilder::new(controller);

        builder.register(Key::H, &[], || {}).unwrap();
        builder.register(Key::J, &[], || {}).unwrap();

        assert_eq!(builder.bindings.len(), 2);
    }

    #[test]
    fn mode_builder_rejects_duplicate_binding() {
        let registry = ModeRegistry::new();
        let controller = ModeController::new(registry);
        let mut builder = ModeBuilder::new(controller);

        builder.register(Key::H, &[], || {}).unwrap();

        let err = builder.register(Key::H, &[], || {}).err().unwrap();

        assert!(matches!(err, Error::AlreadyRegistered { .. }));
    }
}
