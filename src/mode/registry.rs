use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;

use crate::events::EventHub;

use super::options::ModeDefinition;
use super::stack::ModeStack;

#[derive(Clone)]
pub(crate) struct ModeRegistry {
    pub(crate) definitions: Arc<Mutex<HashMap<String, ModeDefinition>>>,
    pub(crate) stack: Arc<Mutex<ModeStack>>,
    pub(crate) event_hub: EventHub,
}

impl ModeRegistry {
    pub(crate) fn new() -> Self {
        Self::with_event_hub(EventHub::new())
    }

    pub(crate) fn with_event_hub(event_hub: EventHub) -> Self {
        Self {
            definitions: Arc::new(Mutex::new(HashMap::new())),
            stack: Arc::new(Mutex::new(ModeStack::default())),
            event_hub,
        }
    }
}

impl Default for ModeRegistry {
    fn default() -> Self {
        Self::new()
    }
}
