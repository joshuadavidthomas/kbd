use std::collections::HashMap;
use std::time::Instant;

use super::options::ModeDefinition;
use crate::manager::HotkeyKey;
use crate::manager::HotkeyRegistration;

struct ActiveMode {
    name: String,
    last_activity: Instant,
}

#[derive(Default)]
pub(crate) struct ModeStack {
    layers: Vec<ActiveMode>,
}

impl ModeStack {
    pub(crate) fn push(&mut self, name: String, now: Instant) {
        self.layers.push(ActiveMode {
            name,
            last_activity: now,
        });
    }

    pub(crate) fn pop(&mut self) -> Option<String> {
        self.layers.pop().map(|am| am.name)
    }

    pub(crate) fn top(&self) -> Option<&str> {
        self.layers.last().map(|am| am.name.as_str())
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.layers.is_empty()
    }

    pub(crate) fn touch_top(&mut self, now: Instant) {
        if let Some(top) = self.layers.last_mut() {
            top.last_activity = now;
        }
    }

    #[cfg(test)]
    pub(crate) fn depth(&self) -> usize {
        self.layers.len()
    }

    pub(crate) fn remove_topmost(&mut self, name: &str) -> bool {
        if let Some(index) = self.layers.iter().rposition(|am| am.name == name) {
            self.layers.remove(index);
            true
        } else {
            false
        }
    }

    pub(crate) fn clear(&mut self) {
        self.layers.clear();
    }
}

pub(crate) enum ModeLookupResult {
    Matched {
        mode_name: String,
        registration: HotkeyRegistration,
        oneshot: bool,
    },
    Swallowed,
    PassThrough,
}

pub(crate) fn lookup_hotkey_in_modes(
    key: &HotkeyKey,
    mode_stack: &ModeStack,
    definitions: &HashMap<String, ModeDefinition>,
) -> ModeLookupResult {
    for layer in mode_stack.layers.iter().rev() {
        if let Some(definition) = definitions.get(&layer.name) {
            if let Some(registration) = definition.bindings.get(key) {
                return ModeLookupResult::Matched {
                    mode_name: layer.name.clone(),
                    registration: registration.clone(),
                    oneshot: definition.options.oneshot,
                };
            }
            if definition.options.swallow {
                return ModeLookupResult::Swallowed;
            }
        }
    }
    ModeLookupResult::PassThrough
}

pub(crate) fn pop_timed_out_modes(
    mode_stack: &mut ModeStack,
    definitions: &HashMap<String, ModeDefinition>,
    now: Instant,
) -> Vec<String> {
    let mut popped = Vec::new();

    loop {
        let should_pop = mode_stack.layers.last().and_then(|top| {
            definitions.get(&top.name).and_then(|def| {
                def.options
                    .timeout
                    .map(|timeout| now.duration_since(top.last_activity) >= timeout)
            })
        });

        match should_pop {
            Some(true) => {
                popped.push(mode_stack.pop().unwrap());
            }
            _ => break,
        }
    }

    popped
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::AtomicUsize;
    use std::sync::atomic::Ordering;
    use std::sync::Arc;
    use std::time::Duration;

    use super::*;
    use crate::key::Key;
    use crate::mode::options::ModeOptions;
    use crate::mode::tests::make_definition;
    use crate::mode::tests::make_registration;

    #[test]
    fn mode_stack_push_and_pop() {
        let mut stack = ModeStack::default();
        let t0 = Instant::now();

        assert!(stack.is_empty());

        stack.push("resize".to_string(), t0);
        assert!(!stack.is_empty());
        assert_eq!(stack.top(), Some("resize"));

        let popped = stack.pop();
        assert_eq!(popped, Some("resize".to_string()));
        assert!(stack.is_empty());
    }

    #[test]
    fn mode_stack_top_shows_most_recent() {
        let mut stack = ModeStack::default();
        let t0 = Instant::now();

        stack.push("base".to_string(), t0);
        stack.push("overlay".to_string(), t0);

        assert_eq!(stack.top(), Some("overlay"));
        assert_eq!(stack.depth(), 2);

        stack.pop();
        assert_eq!(stack.top(), Some("base"));
    }

    #[test]
    fn mode_stack_pop_empty_returns_none() {
        let mut stack = ModeStack::default();
        assert_eq!(stack.pop(), None);
    }

    #[test]
    fn mode_stack_remove_topmost_targets_named_mode() {
        let mut stack = ModeStack::default();
        let t0 = Instant::now();

        stack.push("bottom".to_string(), t0);
        stack.push("middle".to_string(), t0);
        stack.push("top".to_string(), t0);

        assert!(stack.remove_topmost("middle"));
        assert_eq!(stack.depth(), 2);
        assert_eq!(stack.top(), Some("top"));

        stack.pop();
        assert_eq!(stack.top(), Some("bottom"));
    }

    #[test]
    fn mode_stack_remove_topmost_returns_false_for_unknown() {
        let mut stack = ModeStack::default();
        let t0 = Instant::now();
        stack.push("only".to_string(), t0);

        assert!(!stack.remove_topmost("nonexistent"));
        assert_eq!(stack.depth(), 1);
    }

    #[test]
    fn mode_stack_touch_top_updates_activity() {
        let mut stack = ModeStack::default();
        let t0 = Instant::now();
        let t1 = t0 + Duration::from_millis(100);

        stack.push("test".to_string(), t0);
        stack.touch_top(t1);

        assert_eq!(stack.layers.last().unwrap().last_activity, t1);
    }

    #[test]
    fn lookup_empty_stack_passes_through() {
        let stack = ModeStack::default();
        let definitions = HashMap::new();
        let key = (Key::H, vec![]);

        assert!(matches!(
            lookup_hotkey_in_modes(&key, &stack, &definitions),
            ModeLookupResult::PassThrough
        ));
    }

    #[test]
    fn lookup_finds_binding_in_active_mode() {
        let mut stack = ModeStack::default();
        let t0 = Instant::now();
        stack.push("resize".to_string(), t0);

        let counter = Arc::new(AtomicUsize::new(0));
        let key = (Key::H, vec![]);

        let mut definitions = HashMap::new();
        definitions.insert(
            "resize".to_string(),
            make_definition(
                ModeOptions::new(),
                vec![(key.clone(), make_registration(counter.clone()))],
            ),
        );

        let result = lookup_hotkey_in_modes(&key, &stack, &definitions);
        match result {
            ModeLookupResult::Matched {
                mode_name,
                registration,
                oneshot,
            } => {
                assert_eq!(mode_name, "resize");
                assert!(!oneshot);
                (registration.callbacks.on_press)();
                assert_eq!(counter.load(Ordering::SeqCst), 1);
            }
            _ => panic!("expected Matched"),
        }
    }

    #[test]
    fn lookup_prefers_top_mode_binding() {
        let mut stack = ModeStack::default();
        let t0 = Instant::now();
        stack.push("base".to_string(), t0);
        stack.push("overlay".to_string(), t0);

        let base_counter = Arc::new(AtomicUsize::new(0));
        let overlay_counter = Arc::new(AtomicUsize::new(0));
        let key = (Key::H, vec![]);

        let mut definitions = HashMap::new();
        definitions.insert(
            "base".to_string(),
            make_definition(
                ModeOptions::new(),
                vec![(key.clone(), make_registration(base_counter.clone()))],
            ),
        );
        definitions.insert(
            "overlay".to_string(),
            make_definition(
                ModeOptions::new(),
                vec![(key.clone(), make_registration(overlay_counter.clone()))],
            ),
        );

        let result = lookup_hotkey_in_modes(&key, &stack, &definitions);
        match result {
            ModeLookupResult::Matched {
                mode_name,
                registration,
                ..
            } => {
                assert_eq!(mode_name, "overlay");
                (registration.callbacks.on_press)();
                assert_eq!(overlay_counter.load(Ordering::SeqCst), 1);
                assert_eq!(base_counter.load(Ordering::SeqCst), 0);
            }
            _ => panic!("expected Matched from overlay"),
        }
    }

    #[test]
    fn lookup_same_key_in_different_modes_no_conflict() {
        let mut stack = ModeStack::default();
        let t0 = Instant::now();
        stack.push("mode_a".to_string(), t0);

        let counter_a = Arc::new(AtomicUsize::new(0));
        let counter_b = Arc::new(AtomicUsize::new(0));
        let key = (Key::F, vec![]);

        let mut definitions = HashMap::new();
        definitions.insert(
            "mode_a".to_string(),
            make_definition(
                ModeOptions::new(),
                vec![(key.clone(), make_registration(counter_a.clone()))],
            ),
        );
        definitions.insert(
            "mode_b".to_string(),
            make_definition(
                ModeOptions::new(),
                vec![(key.clone(), make_registration(counter_b.clone()))],
            ),
        );

        if let ModeLookupResult::Matched { registration, .. } =
            lookup_hotkey_in_modes(&key, &stack, &definitions)
        {
            (registration.callbacks.on_press)();
        }
        assert_eq!(counter_a.load(Ordering::SeqCst), 1);
        assert_eq!(counter_b.load(Ordering::SeqCst), 0);

        stack.pop();
        stack.push("mode_b".to_string(), t0);

        if let ModeLookupResult::Matched { registration, .. } =
            lookup_hotkey_in_modes(&key, &stack, &definitions)
        {
            (registration.callbacks.on_press)();
        }
        assert_eq!(counter_a.load(Ordering::SeqCst), 1);
        assert_eq!(counter_b.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn lookup_swallow_suppresses_unmatched_key() {
        let mut stack = ModeStack::default();
        let t0 = Instant::now();
        stack.push("swallow_mode".to_string(), t0);

        let bound_key = (Key::H, vec![]);
        let unbound_key = (Key::J, vec![]);

        let mut definitions = HashMap::new();
        definitions.insert(
            "swallow_mode".to_string(),
            make_definition(
                ModeOptions::new().swallow(),
                vec![(
                    bound_key.clone(),
                    make_registration(Arc::new(AtomicUsize::new(0))),
                )],
            ),
        );

        assert!(matches!(
            lookup_hotkey_in_modes(&unbound_key, &stack, &definitions),
            ModeLookupResult::Swallowed
        ));

        assert!(matches!(
            lookup_hotkey_in_modes(&bound_key, &stack, &definitions),
            ModeLookupResult::Matched { .. }
        ));
    }

    #[test]
    fn lookup_falls_through_without_swallow() {
        let mut stack = ModeStack::default();
        let t0 = Instant::now();
        stack.push("normal_mode".to_string(), t0);

        let unbound_key = (Key::Z, vec![]);

        let mut definitions = HashMap::new();
        definitions.insert(
            "normal_mode".to_string(),
            make_definition(ModeOptions::new(), vec![]),
        );

        assert!(matches!(
            lookup_hotkey_in_modes(&unbound_key, &stack, &definitions),
            ModeLookupResult::PassThrough
        ));
    }

    #[test]
    fn swallow_blocks_lower_mode_lookup() {
        let mut stack = ModeStack::default();
        let t0 = Instant::now();
        stack.push("base".to_string(), t0);
        stack.push("swallow_layer".to_string(), t0);

        let key = (Key::A, vec![]);

        let mut definitions = HashMap::new();
        definitions.insert(
            "base".to_string(),
            make_definition(
                ModeOptions::new(),
                vec![(
                    key.clone(),
                    make_registration(Arc::new(AtomicUsize::new(0))),
                )],
            ),
        );
        definitions.insert(
            "swallow_layer".to_string(),
            make_definition(ModeOptions::new().swallow(), vec![]),
        );

        assert!(matches!(
            lookup_hotkey_in_modes(&key, &stack, &definitions),
            ModeLookupResult::Swallowed
        ));
    }

    #[test]
    fn timeout_pops_expired_top_mode() {
        let mut stack = ModeStack::default();
        let t0 = Instant::now();
        stack.push("timed".to_string(), t0);

        let mut definitions = HashMap::new();
        definitions.insert(
            "timed".to_string(),
            make_definition(
                ModeOptions::new().timeout(Duration::from_millis(100)),
                vec![],
            ),
        );

        let popped = pop_timed_out_modes(&mut stack, &definitions, t0 + Duration::from_millis(50));
        assert!(popped.is_empty());
        assert!(!stack.is_empty());

        let popped = pop_timed_out_modes(&mut stack, &definitions, t0 + Duration::from_millis(150));
        assert_eq!(popped, vec!["timed".to_string()]);
        assert!(stack.is_empty());
    }

    #[test]
    fn timeout_cascades_through_stack() {
        let mut stack = ModeStack::default();
        let t0 = Instant::now();
        stack.push("bottom".to_string(), t0);
        stack.push("top".to_string(), t0);

        let mut definitions = HashMap::new();
        definitions.insert(
            "bottom".to_string(),
            make_definition(
                ModeOptions::new().timeout(Duration::from_millis(100)),
                vec![],
            ),
        );
        definitions.insert(
            "top".to_string(),
            make_definition(
                ModeOptions::new().timeout(Duration::from_millis(50)),
                vec![],
            ),
        );

        let popped = pop_timed_out_modes(&mut stack, &definitions, t0 + Duration::from_millis(150));
        assert_eq!(popped, vec!["top".to_string(), "bottom".to_string()]);
        assert!(stack.is_empty());
    }

    #[test]
    fn touch_top_resets_timeout() {
        let mut stack = ModeStack::default();
        let t0 = Instant::now();
        stack.push("timed".to_string(), t0);

        let mut definitions = HashMap::new();
        definitions.insert(
            "timed".to_string(),
            make_definition(
                ModeOptions::new().timeout(Duration::from_millis(100)),
                vec![],
            ),
        );

        stack.touch_top(t0 + Duration::from_millis(80));

        let popped = pop_timed_out_modes(&mut stack, &definitions, t0 + Duration::from_millis(150));
        assert!(popped.is_empty());

        let popped = pop_timed_out_modes(&mut stack, &definitions, t0 + Duration::from_millis(200));
        assert_eq!(popped, vec!["timed".to_string()]);
    }
}
