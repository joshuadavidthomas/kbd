#![cfg_attr(docsrs, feature(doc_cfg))]

//! Logical keyboard observations from caller-owned XKB state.
//!
//! [`enrich`] replaces logical evidence without altering physical identity,
//! physical modifiers, or transition. It neither updates XKB state nor dispatches
//! events. The caller supplies the authoritative keymap/state and an explicit
//! [`ModifierMapping`]; no default layout or desktop state is inferred.
//!
//! Direct-device owners should query before `State::update_key`, balance real
//! presses/releases, and not update on repeats. Clients of a state authority
//! should instead apply its complete `State::update_mask` notifications; do not
//! mix the two update models. The caller owns initial state, ordering, device
//! aggregation, resynchronization and keymap replacement. On lost input, use
//! the dispatcher's cancellation/reset APIs as well as resynchronizing XKB.
//!
//! Consumption is reported using XKB mode. Logical matching remains exact unless
//! a binding explicitly opts into `kbd::policy::LogicalMatchPolicy::Consumed`.
//! Sequences remain exact. No compose state, committed text, or synthesis is
//! handled here. Link-time and runtime `libxkbcommon` are required.
//!
//! See the crate README for the modifier mapping and ownership contracts.

mod logical;

use kbd::hotkey::Modifier;
use kbd::hotkey::ModifierSet;
use kbd::observation::KeyboardObservation;
use kbd::observation::LogicalModifiers;
use kbd::observation::ModifierObservation;
use kbd::observation::ModifierState;
/// The exact binding used by this adapter, for callers constructing XKB state.
pub use xkbcommon::xkb;

/// Explicit semantic interpretation of a keymap's eight real modifier bits.
///
/// Masks are XKB real-modifier masks, **not** [`ModifierSet`] bits or virtual
/// modifier indices. The caller must derive them from its authoritative keymap
/// and replace this mapping when that keymap changes. No Alt/Super/AltGraph
/// assignments are assumed. `u8` excludes XKB's virtual modifier bits.
///
/// Omitted semantic modifiers remain unknown. A zero mask explicitly asserts
/// that a semantic modifier is known inactive. Multiple entries for one semantic
/// modifier are combined; all its active real bits must be consumed before the
/// semantic modifier can be reported consumed. If inactive, all its mapped bits
/// must be consumed. This preserves XKB's potential consumption of inactive flags.
#[derive(Debug, Clone, Copy)]
pub struct ModifierMapping<'a> {
    /// Pairs of semantic modifiers and real XKB masks (any bit activates the flag).
    pub modifiers: &'a [(Modifier, u8)],
    /// Real modifiers explicitly irrelevant to shortcut matching, e.g. lock bits.
    /// Unmapped active bits outside this mask set `extra_active`, blocking matches.
    /// Ignoring a bit does not suppress any explicit mapping of that bit above.
    pub ignored: u8,
}

impl ModifierMapping<'_> {
    fn resolve(self, active: xkb::ModMask, consumed: Option<xkb::ModMask>) -> LogicalModifiers {
        let mut known = ModifierSet::NONE;
        let mut semantic_active = ModifierSet::NONE;
        let mut semantic_consumed = ModifierSet::NONE;
        let mut mapped = 0;
        for modifier in ModifierSet::ALL_MODIFIERS {
            let mut mask = 0;
            for &(semantic, real) in self.modifiers {
                if semantic == modifier {
                    known = known.with(modifier);
                    mask |= u32::from(real);
                }
            }
            mapped |= mask;
            let active_bits = active & mask;
            if active_bits != 0 {
                semantic_active = semantic_active.with(modifier);
            }
            let relevant = if active_bits == 0 { mask } else { active_bits };
            if relevant != 0 && consumed.is_some_and(|bits| relevant & bits == relevant) {
                semantic_consumed = semantic_consumed.with(modifier);
            }
        }
        LogicalModifiers {
            state: ModifierState::new(semantic_active, known)
                .with_extra_active(active & !(mapped | u32::from(self.ignored)) != 0),
            consumed: consumed.map(|_| semantic_consumed),
        }
    }
}

/// Convert a Linux evdev code to an XKB keycode for an evdev-compatible keymap.
///
/// Apply this offset once, not to an X11/XKB keycode already in XKB numbering.
/// This does not infer a physical [`kbd::key::Key`] or validate keymap membership.
#[must_use]
pub fn evdev_keycode(code: u16) -> xkb::Keycode {
    xkb::Keycode::new(u32::from(code) + 8)
}

/// Enrich an observation from the caller's current authoritative XKB state.
///
/// Physical/backend facts and transition are preserved. Existing logical facts
/// are replaced, including clearing the logical identity for missing, unknown,
/// or multiple symbols. A missing key has unknown consumption, not known-empty.
/// No case folding or text/compose processing occurs. The state is never updated.
///
/// `keycode` must already be in XKB numbering; use [`evdev_keycode`] only for an
/// evdev-compatible map. [`ModifierMapping`] must describe this state's keymap.
///
/// ```
/// use kbd::dispatcher::Dispatcher;
/// use kbd::observation::KeyboardObservation;
/// use kbd_xkb::{enrich, evdev_keycode, xkb, ModifierMapping};
///
/// fn on_evdev_event(
///     dispatcher: &mut Dispatcher,
///     state: &xkb::State,
///     mapping: ModifierMapping<'_>,
///     code: u16,
///     mut observation: KeyboardObservation,
/// ) {
///     enrich(&mut observation, state, evdev_keycode(code), mapping);
///     let result = dispatcher.process_event(&observation);
///     // Handle the one result; state updates and native text stay with the caller.
/// }
/// ```
pub fn enrich(
    observation: &mut KeyboardObservation,
    state: &xkb::State,
    keycode: xkb::Keycode,
    mapping: ModifierMapping<'_>,
) {
    let physical = observation.physical_modifiers();
    observation.logical = logical::from_keysym(state.key_get_one_sym(keycode));
    let consumed = (state.key_get_layout(keycode) != xkb::LAYOUT_INVALID)
        .then(|| state.key_get_consumed_mods(keycode));
    observation.modifier_observation = Some(ModifierObservation {
        physical,
        logical: Some(mapping.resolve(state.serialize_mods(xkb::STATE_MODS_EFFECTIVE), consumed)),
    });
}
