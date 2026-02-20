use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use evdev::KeyCode;

use crate::error::Error;
use crate::events::{EventHub, HotkeyEvent};
use crate::manager::{
    attach_hotkey_events, normalize_modifiers, validate_hotkey_binding, ActiveHotkeyPress,
    Callback, HotkeyKey, HotkeyOptions, HotkeyRegistration, PressDispatchState, PressOrigin,
    RepeatBehavior,
};

// Mode options
#[derive(Clone, Default)]
pub struct ModeOptions {
    pub(crate) oneshot: bool,
    pub(crate) swallow: bool,
    pub(crate) timeout: Option<Duration>,
}

impl ModeOptions {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn oneshot(mut self) -> Self {
        self.oneshot = true;
        self
    }

    pub fn swallow(mut self) -> Self {
        self.swallow = true;
        self
    }

    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }
}

// Mode definition: options + bindings
pub(crate) struct ModeDefinition {
    pub(crate) options: ModeOptions,
    pub(crate) bindings: HashMap<HotkeyKey, HotkeyRegistration>,
}

// An active mode on the stack
struct ActiveMode {
    name: String,
    last_activity: Instant,
}

// Runtime mode stack
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

// Result of looking up a hotkey in the mode stack
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

pub(crate) enum ModeEventDispatch {
    PassThrough,
    Swallowed,
    Handled {
        callbacks: Vec<Callback>,
        passthrough: bool,
    },
}

pub(crate) fn dispatch_mode_key_event(
    hotkey_key: &HotkeyKey,
    value: i32,
    now: Instant,
    definitions: &HashMap<String, ModeDefinition>,
    stack: &mut ModeStack,
    active_presses: &mut HashMap<KeyCode, ActiveHotkeyPress>,
) -> ModeEventDispatch {
    match value {
        1 => dispatch_mode_press(hotkey_key, now, definitions, stack, active_presses),
        0 => dispatch_mode_release(hotkey_key.0, now, definitions, stack, active_presses),
        2 => dispatch_mode_repeat(hotkey_key.0, now, definitions, active_presses),
        _ => ModeEventDispatch::PassThrough,
    }
}

fn dispatch_mode_press(
    hotkey_key: &HotkeyKey,
    now: Instant,
    definitions: &HashMap<String, ModeDefinition>,
    stack: &mut ModeStack,
    active_presses: &mut HashMap<KeyCode, ActiveHotkeyPress>,
) -> ModeEventDispatch {
    if stack.is_empty() {
        return ModeEventDispatch::PassThrough;
    }

    match lookup_hotkey_in_modes(hotkey_key, stack, definitions) {
        ModeLookupResult::Matched {
            mode_name,
            registration,
            oneshot,
        } => {
            let press_dispatch_state = registration
                .callbacks
                .min_hold
                .map(|min_hold| {
                    if min_hold.is_zero() {
                        PressDispatchState::Dispatched
                    } else {
                        PressDispatchState::Pending
                    }
                })
                .unwrap_or(PressDispatchState::Dispatched);

            let passthrough = registration.callbacks.passthrough;

            active_presses.insert(
                hotkey_key.0,
                ActiveHotkeyPress {
                    registration_key: hotkey_key.clone(),
                    origin: PressOrigin::Mode(mode_name.clone()),
                    pressed_at: now,
                    press_dispatch_state,
                },
            );

            let mut callbacks = Vec::new();
            if press_dispatch_state == PressDispatchState::Dispatched {
                callbacks.push(registration.callbacks.on_press.clone());
            }

            if oneshot {
                stack.remove_topmost(&mode_name);
            }

            stack.touch_top(now);

            ModeEventDispatch::Handled {
                callbacks,
                passthrough,
            }
        }
        ModeLookupResult::Swallowed => {
            stack.touch_top(now);
            ModeEventDispatch::Swallowed
        }
        ModeLookupResult::PassThrough => ModeEventDispatch::PassThrough,
    }
}

fn dispatch_mode_release(
    key: KeyCode,
    now: Instant,
    definitions: &HashMap<String, ModeDefinition>,
    stack: &mut ModeStack,
    active_presses: &mut HashMap<KeyCode, ActiveHotkeyPress>,
) -> ModeEventDispatch {
    let Some(active) = active_presses.get(&key) else {
        return ModeEventDispatch::PassThrough;
    };
    let PressOrigin::Mode(ref mode_name) = active.origin else {
        return ModeEventDispatch::PassThrough;
    };
    let mode_name = mode_name.clone();

    let active = active_presses.remove(&key).unwrap();

    let Some(registration) = definitions
        .get(&mode_name)
        .and_then(|def| def.bindings.get(&active.registration_key))
    else {
        return ModeEventDispatch::Handled {
            callbacks: Vec::new(),
            passthrough: false,
        };
    };

    let passthrough = registration.callbacks.passthrough;
    let mut callbacks = Vec::new();

    if active.press_dispatch_state == PressDispatchState::Pending {
        if let Some(min_hold) = registration.callbacks.min_hold {
            if now.duration_since(active.pressed_at) >= min_hold {
                callbacks.push(registration.callbacks.on_press.clone());
            }
        }
    }

    if let Some(on_release) = &registration.callbacks.on_release {
        callbacks.push(on_release.clone());
    }

    if !stack.is_empty() {
        stack.touch_top(now);
    }

    ModeEventDispatch::Handled {
        callbacks,
        passthrough,
    }
}

fn dispatch_mode_repeat(
    key: KeyCode,
    now: Instant,
    definitions: &HashMap<String, ModeDefinition>,
    active_presses: &mut HashMap<KeyCode, ActiveHotkeyPress>,
) -> ModeEventDispatch {
    let is_mode_press = active_presses
        .get(&key)
        .is_some_and(|active| matches!(active.origin, PressOrigin::Mode(_)));

    if !is_mode_press {
        return ModeEventDispatch::PassThrough;
    }

    let active = active_presses.get_mut(&key).unwrap();
    let PressOrigin::Mode(ref mode_name) = active.origin else {
        unreachable!();
    };

    let Some(registration) = definitions
        .get(mode_name)
        .and_then(|def| def.bindings.get(&active.registration_key))
    else {
        return ModeEventDispatch::Handled {
            callbacks: Vec::new(),
            passthrough: false,
        };
    };

    let passthrough = registration.callbacks.passthrough;
    let hold_satisfied = registration
        .callbacks
        .min_hold
        .is_none_or(|min_hold| now.duration_since(active.pressed_at) >= min_hold);

    let mut callbacks = Vec::new();
    if registration.callbacks.repeat_behavior == RepeatBehavior::Trigger && hold_satisfied {
        callbacks.push(registration.callbacks.on_press.clone());
        active.press_dispatch_state = PressDispatchState::Dispatched;
    }

    ModeEventDispatch::Handled {
        callbacks,
        passthrough,
    }
}

/// Look up a registration's callbacks for an active press, checking mode definitions
/// when the press originated from a mode, device registrations when the press
/// originated from a device-specific hotkey, or global registrations otherwise.
pub(crate) fn find_callbacks_for_active_press<'a>(
    active: &ActiveHotkeyPress,
    global_registrations: &'a HashMap<HotkeyKey, HotkeyRegistration>,
    mode_definitions: &'a HashMap<String, ModeDefinition>,
    device_registrations: &'a HashMap<
        crate::manager::DeviceRegistrationId,
        crate::manager::DeviceHotkeyRegistration,
    >,
) -> Option<&'a crate::manager::HotkeyCallbacks> {
    match &active.origin {
        PressOrigin::Mode(mode_name) => mode_definitions
            .get(mode_name)
            .and_then(|def| def.bindings.get(&active.registration_key))
            .map(|reg| &reg.callbacks),
        PressOrigin::Device(device_reg_id) => device_registrations
            .get(device_reg_id)
            .map(|reg| &reg.callbacks),
        PressOrigin::Global => global_registrations
            .get(&active.registration_key)
            .map(|reg| &reg.callbacks),
    }
}

// Shared mode state between manager and listener
#[derive(Clone)]
pub(crate) struct ModeRegistry {
    pub(crate) definitions: Arc<Mutex<HashMap<String, ModeDefinition>>>,
    pub(crate) stack: Arc<Mutex<ModeStack>>,
    pub(crate) event_hub: EventHub,
}

impl ModeRegistry {
    pub(crate) fn new() -> Self {
        Self::with_event_hub(EventHub::default())
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

// Public mode controller for push/pop from callbacks
#[derive(Clone)]
pub struct ModeController {
    registry: ModeRegistry,
}

impl ModeController {
    pub(crate) fn new(registry: ModeRegistry) -> Self {
        Self { registry }
    }

    pub fn push(&self, name: &str) {
        let has_definition = self.registry.definitions.lock().unwrap().contains_key(name);

        if !has_definition {
            tracing::warn!("Attempted to push undefined mode: {name}");
            return;
        }

        let now = Instant::now();
        self.registry
            .stack
            .lock()
            .unwrap()
            .push(name.to_string(), now);
        self.registry
            .event_hub
            .emit(HotkeyEvent::ModeChanged(self.active_mode()));
    }

    pub fn pop(&self) -> Option<String> {
        let popped = self.registry.stack.lock().unwrap().pop();
        if popped.is_some() {
            self.registry
                .event_hub
                .emit(HotkeyEvent::ModeChanged(self.active_mode()));
        }
        popped
    }

    pub fn active_mode(&self) -> Option<String> {
        self.registry.stack.lock().unwrap().top().map(String::from)
    }
}

// Builder for defining mode bindings
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
        key: KeyCode,
        modifiers: &[KeyCode],
        callback: F,
    ) -> Result<(), Error>
    where
        F: Fn() + Send + Sync + 'static,
    {
        self.register_with_options(key, modifiers, HotkeyOptions::new(), callback)
    }

    pub fn register_with_options<F>(
        &mut self,
        key: KeyCode,
        modifiers: &[KeyCode],
        options: HotkeyOptions,
        callback: F,
    ) -> Result<(), Error>
    where
        F: Fn() + Send + Sync + 'static,
    {
        validate_hotkey_binding(key, modifiers)?;
        let hotkey_key = (key, normalize_modifiers(modifiers));

        if self.bindings.contains_key(&hotkey_key) {
            return Err(Error::AlreadyRegistered {
                key: hotkey_key.0,
                modifiers: hotkey_key.1,
            });
        }

        let callbacks = attach_hotkey_events(
            options.build_callbacks(callback),
            &hotkey_key,
            &self.controller.registry.event_hub,
        );

        let registration = HotkeyRegistration { callbacks };

        self.bindings.insert(hotkey_key, registration);
        Ok(())
    }

    pub fn mode_controller(&self) -> ModeController {
        self.controller.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manager::HotkeyCallbacks;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn make_registration(counter: Arc<AtomicUsize>) -> HotkeyRegistration {
        HotkeyRegistration {
            callbacks: HotkeyCallbacks {
                on_press: Arc::new(move || {
                    counter.fetch_add(1, Ordering::SeqCst);
                }),
                on_release: None,
                has_release_callback: false,
                min_hold: None,
                repeat_behavior: RepeatBehavior::Ignore,
                passthrough: false,
            },
        }
    }

    fn make_registration_with_release(
        press_counter: Arc<AtomicUsize>,
        release_counter: Arc<AtomicUsize>,
    ) -> HotkeyRegistration {
        let rc = release_counter;
        HotkeyRegistration {
            callbacks: HotkeyCallbacks {
                on_press: Arc::new(move || {
                    press_counter.fetch_add(1, Ordering::SeqCst);
                }),
                on_release: Some(Arc::new(move || {
                    rc.fetch_add(1, Ordering::SeqCst);
                })),
                has_release_callback: true,
                min_hold: None,
                repeat_behavior: RepeatBehavior::Ignore,
                passthrough: false,
            },
        }
    }

    fn make_definition(
        options: ModeOptions,
        bindings: Vec<(HotkeyKey, HotkeyRegistration)>,
    ) -> ModeDefinition {
        ModeDefinition {
            options,
            bindings: bindings.into_iter().collect(),
        }
    }

    fn dispatch_callbacks(callbacks: Vec<Callback>) {
        for cb in callbacks {
            cb();
        }
    }

    // ModeStack tests

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

    // ModeOptions tests

    #[test]
    fn mode_options_default_has_no_special_behavior() {
        let opts = ModeOptions::new();
        assert!(!opts.oneshot);
        assert!(!opts.swallow);
        assert!(opts.timeout.is_none());
    }

    #[test]
    fn mode_options_oneshot_swallow_timeout() {
        let opts = ModeOptions::new()
            .oneshot()
            .swallow()
            .timeout(Duration::from_secs(5));

        assert!(opts.oneshot);
        assert!(opts.swallow);
        assert_eq!(opts.timeout, Some(Duration::from_secs(5)));
    }

    // Lookup tests

    #[test]
    fn lookup_empty_stack_passes_through() {
        let stack = ModeStack::default();
        let definitions = HashMap::new();
        let key = (KeyCode::KEY_H, vec![]);

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
        let key = (KeyCode::KEY_H, vec![]);

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
        let key = (KeyCode::KEY_H, vec![]);

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
        let key = (KeyCode::KEY_F, vec![]);

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

        // mode_a is active, so its binding fires
        if let ModeLookupResult::Matched { registration, .. } =
            lookup_hotkey_in_modes(&key, &stack, &definitions)
        {
            (registration.callbacks.on_press)();
        }
        assert_eq!(counter_a.load(Ordering::SeqCst), 1);
        assert_eq!(counter_b.load(Ordering::SeqCst), 0);

        // Pop mode_a, push mode_b
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

        let bound_key = (KeyCode::KEY_H, vec![]);
        let unbound_key = (KeyCode::KEY_J, vec![]);

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

        // Unbound key is swallowed
        assert!(matches!(
            lookup_hotkey_in_modes(&unbound_key, &stack, &definitions),
            ModeLookupResult::Swallowed
        ));

        // Bound key is matched
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

        let unbound_key = (KeyCode::KEY_Z, vec![]);

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

        let key = (KeyCode::KEY_A, vec![]);

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

        // Even though base has KEY_A, swallow_layer consumes it
        assert!(matches!(
            lookup_hotkey_in_modes(&key, &stack, &definitions),
            ModeLookupResult::Swallowed
        ));
    }

    // Timeout tests

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

        // Touch at 80ms resets
        stack.touch_top(t0 + Duration::from_millis(80));

        // At 150ms from t0 (70ms from touch), not expired
        let popped = pop_timed_out_modes(&mut stack, &definitions, t0 + Duration::from_millis(150));
        assert!(popped.is_empty());

        // At 200ms from t0 (120ms from touch), expired
        let popped = pop_timed_out_modes(&mut stack, &definitions, t0 + Duration::from_millis(200));
        assert_eq!(popped, vec!["timed".to_string()]);
    }

    // Mode event dispatch tests

    #[test]
    fn mode_dispatch_press_fires_callback() {
        let mut stack = ModeStack::default();
        let t0 = Instant::now();
        stack.push("resize".to_string(), t0);

        let counter = Arc::new(AtomicUsize::new(0));
        let key = (KeyCode::KEY_H, vec![]);

        let mut definitions = HashMap::new();
        definitions.insert(
            "resize".to_string(),
            make_definition(
                ModeOptions::new(),
                vec![(key.clone(), make_registration(counter.clone()))],
            ),
        );

        let mut active_presses = HashMap::new();
        let dispatch =
            dispatch_mode_key_event(&key, 1, t0, &definitions, &mut stack, &mut active_presses);

        let ModeEventDispatch::Handled { callbacks, .. } = dispatch else {
            panic!("expected Handled");
        };
        dispatch_callbacks(callbacks);
        assert_eq!(counter.load(Ordering::SeqCst), 1);
        assert!(active_presses.contains_key(&KeyCode::KEY_H));
    }

    #[test]
    fn mode_dispatch_release_fires_release_callback() {
        let mut stack = ModeStack::default();
        let t0 = Instant::now();
        stack.push("test".to_string(), t0);

        let press_count = Arc::new(AtomicUsize::new(0));
        let release_count = Arc::new(AtomicUsize::new(0));
        let key = (KeyCode::KEY_H, vec![]);

        let mut definitions = HashMap::new();
        definitions.insert(
            "test".to_string(),
            make_definition(
                ModeOptions::new(),
                vec![(
                    key.clone(),
                    make_registration_with_release(press_count.clone(), release_count.clone()),
                )],
            ),
        );

        let mut active_presses = HashMap::new();

        // Press
        let press_dispatch =
            dispatch_mode_key_event(&key, 1, t0, &definitions, &mut stack, &mut active_presses);
        let ModeEventDispatch::Handled { callbacks, .. } = press_dispatch else {
            panic!("expected Handled");
        };
        dispatch_callbacks(callbacks);

        // Release
        let release_dispatch = dispatch_mode_key_event(
            &key,
            0,
            t0 + Duration::from_millis(10),
            &definitions,
            &mut stack,
            &mut active_presses,
        );
        let ModeEventDispatch::Handled { callbacks, .. } = release_dispatch else {
            panic!("expected Handled");
        };
        dispatch_callbacks(callbacks);

        assert_eq!(press_count.load(Ordering::SeqCst), 1);
        assert_eq!(release_count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn mode_dispatch_oneshot_pops_after_match() {
        let mut stack = ModeStack::default();
        let t0 = Instant::now();
        stack.push("oneshot_mode".to_string(), t0);

        let counter = Arc::new(AtomicUsize::new(0));
        let key = (KeyCode::KEY_F, vec![]);

        let mut definitions = HashMap::new();
        definitions.insert(
            "oneshot_mode".to_string(),
            make_definition(
                ModeOptions::new().oneshot(),
                vec![(key.clone(), make_registration(counter.clone()))],
            ),
        );

        let mut active_presses = HashMap::new();
        let dispatch =
            dispatch_mode_key_event(&key, 1, t0, &definitions, &mut stack, &mut active_presses);

        let ModeEventDispatch::Handled { callbacks, .. } = dispatch else {
            panic!("expected Handled");
        };
        dispatch_callbacks(callbacks);
        assert_eq!(counter.load(Ordering::SeqCst), 1);
        assert!(stack.is_empty());
    }

    #[test]
    fn mode_dispatch_oneshot_removes_matched_mode_not_top() {
        let mut stack = ModeStack::default();
        let t0 = Instant::now();

        // Push oneshot base, then a non-swallow overlay on top
        stack.push("base_oneshot".to_string(), t0);
        stack.push("overlay".to_string(), t0);

        let counter = Arc::new(AtomicUsize::new(0));
        let key = (KeyCode::KEY_G, vec![]);

        let mut definitions = HashMap::new();
        definitions.insert(
            "base_oneshot".to_string(),
            make_definition(
                ModeOptions::new().oneshot(),
                vec![(key.clone(), make_registration(counter.clone()))],
            ),
        );
        // overlay has no bindings and no swallow, so KEY_G falls through to base
        definitions.insert(
            "overlay".to_string(),
            make_definition(ModeOptions::new(), vec![]),
        );

        let mut active_presses = HashMap::new();
        let dispatch =
            dispatch_mode_key_event(&key, 1, t0, &definitions, &mut stack, &mut active_presses);

        let ModeEventDispatch::Handled { callbacks, .. } = dispatch else {
            panic!("expected Handled");
        };
        dispatch_callbacks(callbacks);
        assert_eq!(counter.load(Ordering::SeqCst), 1);

        // base_oneshot should be removed, overlay should remain
        assert_eq!(stack.depth(), 1);
        assert_eq!(stack.top(), Some("overlay"));
    }

    #[test]
    fn mode_dispatch_swallow_suppresses_unmatched() {
        let mut stack = ModeStack::default();
        let t0 = Instant::now();
        stack.push("swallow_mode".to_string(), t0);

        let mut definitions = HashMap::new();
        definitions.insert(
            "swallow_mode".to_string(),
            make_definition(ModeOptions::new().swallow(), vec![]),
        );

        let unbound_key = (KeyCode::KEY_Z, vec![]);
        let mut active_presses = HashMap::new();
        let dispatch = dispatch_mode_key_event(
            &unbound_key,
            1,
            t0,
            &definitions,
            &mut stack,
            &mut active_presses,
        );

        assert!(matches!(dispatch, ModeEventDispatch::Swallowed));
    }

    #[test]
    fn mode_dispatch_passes_through_when_no_modes_active() {
        let mut stack = ModeStack::default();
        let t0 = Instant::now();
        let key = (KeyCode::KEY_A, vec![]);
        let mut active_presses = HashMap::new();

        let dispatch = dispatch_mode_key_event(
            &key,
            1,
            t0,
            &HashMap::new(),
            &mut stack,
            &mut active_presses,
        );

        assert!(matches!(dispatch, ModeEventDispatch::PassThrough));
    }

    #[test]
    fn mode_dispatch_passes_through_for_unmatched_key_without_swallow() {
        let mut stack = ModeStack::default();
        let t0 = Instant::now();
        stack.push("partial_mode".to_string(), t0);

        let mut definitions = HashMap::new();
        definitions.insert(
            "partial_mode".to_string(),
            make_definition(ModeOptions::new(), vec![]),
        );

        let key = (KeyCode::KEY_Q, vec![]);
        let mut active_presses = HashMap::new();
        let dispatch =
            dispatch_mode_key_event(&key, 1, t0, &definitions, &mut stack, &mut active_presses);

        assert!(matches!(dispatch, ModeEventDispatch::PassThrough));
    }

    // ModeController tests

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

    // ModeBuilder tests

    #[test]
    fn mode_builder_collects_bindings() {
        let registry = ModeRegistry::new();
        let controller = ModeController::new(registry);
        let mut builder = ModeBuilder::new(controller);

        builder.register(KeyCode::KEY_H, &[], || {}).unwrap();
        builder.register(KeyCode::KEY_J, &[], || {}).unwrap();

        assert_eq!(builder.bindings.len(), 2);
    }

    #[test]
    fn mode_builder_rejects_duplicate_binding() {
        let registry = ModeRegistry::new();
        let controller = ModeController::new(registry);
        let mut builder = ModeBuilder::new(controller);

        builder.register(KeyCode::KEY_H, &[], || {}).unwrap();

        let err = builder.register(KeyCode::KEY_H, &[], || {}).err().unwrap();

        assert!(matches!(err, Error::AlreadyRegistered { .. }));
    }

    #[test]
    fn mode_builder_validates_hotkey_bindings() {
        let registry = ModeRegistry::new();
        let controller = ModeController::new(registry);
        let mut builder = ModeBuilder::new(controller);

        let err = builder
            .register(KeyCode::KEY_LEFTCTRL, &[], || {})
            .err()
            .unwrap();

        assert!(matches!(err, Error::InvalidHotkey(_)));
    }

    // find_registration_for_active_press tests

    #[test]
    fn find_callbacks_finds_global_press() {
        let counter = Arc::new(AtomicUsize::new(0));
        let key = (KeyCode::KEY_A, vec![KeyCode::KEY_LEFTCTRL]);

        let mut global = HashMap::new();
        global.insert(key.clone(), make_registration(counter.clone()));

        let active = ActiveHotkeyPress {
            registration_key: key,
            origin: PressOrigin::Global,
            pressed_at: Instant::now(),
            press_dispatch_state: PressDispatchState::Dispatched,
        };

        let no_modes = HashMap::new();
        let no_devices = HashMap::new();
        let callbacks = find_callbacks_for_active_press(&active, &global, &no_modes, &no_devices);
        assert!(callbacks.is_some());
        (callbacks.unwrap().on_press)();
        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn find_callbacks_finds_mode_press() {
        let counter = Arc::new(AtomicUsize::new(0));
        let key = (KeyCode::KEY_H, vec![]);

        let mut definitions = HashMap::new();
        definitions.insert(
            "resize".to_string(),
            make_definition(
                ModeOptions::new(),
                vec![(key.clone(), make_registration(counter.clone()))],
            ),
        );

        let active = ActiveHotkeyPress {
            registration_key: key,
            origin: PressOrigin::Mode("resize".to_string()),
            pressed_at: Instant::now(),
            press_dispatch_state: PressDispatchState::Dispatched,
        };

        let no_global = HashMap::new();
        let no_devices = HashMap::new();
        let callbacks =
            find_callbacks_for_active_press(&active, &no_global, &definitions, &no_devices);
        assert!(callbacks.is_some());
        (callbacks.unwrap().on_press)();
        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }
}
