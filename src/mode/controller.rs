use std::time::Instant;

use super::registry::ModeRegistry;
use crate::events::HotkeyEvent;

#[derive(Clone)]
pub struct ModeController {
    pub(super) registry: ModeRegistry,
}

impl ModeController {
    pub(crate) fn new(registry: ModeRegistry) -> Self {
        Self { registry }
    }

    /// # Panics
    ///
    /// Panics if the internal definitions or stack lock is poisoned.
    pub fn push(&self, name: &str) {
        let has_definition = self.registry.definitions.lock().unwrap().contains_key(name);

        if !has_definition {
            tracing::warn!("Attempted to push undefined mode: {name}");
            return;
        }

        let now = Instant::now();
        let mode_change_event = {
            let mut stack = self.registry.stack.lock().unwrap();
            let before = stack.top().map(str::to_string);
            stack.push(name.to_string(), now);
            let after = stack.top().map(str::to_string);
            (before != after).then_some(HotkeyEvent::ModeChanged(after))
        };

        if let Some(event) = mode_change_event {
            self.registry.event_hub.emit(&event);
        }
    }

    /// # Panics
    ///
    /// Panics if the internal stack lock is poisoned.
    pub fn pop(&self) -> Option<String> {
        let (popped, mode_change_event) = {
            let mut stack = self.registry.stack.lock().unwrap();
            let before = stack.top().map(str::to_string);
            let popped = stack.pop();
            let after = stack.top().map(str::to_string);
            let mode_change_event =
                (popped.is_some() && before != after).then_some(HotkeyEvent::ModeChanged(after));
            (popped, mode_change_event)
        };

        if let Some(event) = mode_change_event {
            self.registry.event_hub.emit(&event);
        }

        popped
    }

    /// # Panics
    ///
    /// Panics if the internal stack lock is poisoned.
    pub fn active_mode(&self) -> Option<String> {
        self.registry.stack.lock().unwrap().top().map(String::from)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mode::options::ModeOptions;
    use crate::mode::tests::make_definition;

    #[test]
    fn mode_controller_push_pop_roundtrip() {
        let registry = ModeRegistry::new();
        registry.definitions.lock().unwrap().insert(
            "test".to_string(),
            make_definition(ModeOptions::new(), vec![]),
        );

        let controller = ModeController::new(registry);

        assert!(controller.active_mode().is_none());

        controller.push("test");
        assert_eq!(controller.active_mode(), Some("test".to_string()));

        let popped = controller.pop();
        assert_eq!(popped, Some("test".to_string()));
        assert!(controller.active_mode().is_none());
    }

    #[test]
    fn mode_controller_push_undefined_is_noop() {
        let registry = ModeRegistry::new();
        let controller = ModeController::new(registry);

        controller.push("nonexistent");
        assert!(controller.active_mode().is_none());
    }
}
