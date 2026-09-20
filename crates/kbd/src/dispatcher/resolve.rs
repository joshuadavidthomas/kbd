use super::DeviceContext;
use crate::binding::Binding;
#[cfg(test)]
use crate::hotkey::Hotkey;
#[cfg(test)]
use crate::hotkey::HotkeySequence;
use crate::layer::StoredLayer;
use crate::observation::KeyboardObservation;
use crate::sequence::BindingSequence;

/// Result of classifying all sequence bindings within a scope against a hotkey.
///
/// Encodes the precedence rule: single-step sequences win over multi-step.
/// Indices refer to positions in the input iterator passed to
/// [`classify_observation_prefixes`].
///
/// Used directly by the global-bindings path where immediate hotkey lookup
/// is a separate `HashMap` operation. For layer scopes, prefer
/// [`classify_observation`] which combines sequence and immediate
/// classification into [`LayerMatch`].
#[derive(Debug, PartialEq)]
pub(super) enum SequencePrefixMatch {
    /// No sequences in this scope matched the hotkey as a prefix.
    None,
    /// A single-step sequence matched immediately (highest priority).
    SingleStep { index: usize },
    /// One or more multi-step sequences matched as prefixes (pending state).
    MultiStep { indices: Vec<usize> },
}

/// What matched for a hotkey within a single layer scope.
///
/// Combines sequence classification and immediate hotkey scanning into
/// a single result. Both the runtime path ([`Dispatcher::process`]) and
/// the query path ([`Dispatcher::bindings_for_key`]) match on this enum,
/// so adding a new match type (e.g., tap-hold) forces both paths to
/// handle it via exhaustive matching.
///
/// Indices for sequence variants refer to positions in
/// [`StoredLayer::sequence_bindings`]. The `Immediate` index refers to
/// a position in [`StoredLayer::bindings`].
#[derive(Debug, PartialEq)]
pub(super) enum LayerMatch {
    /// A single-step sequence matched immediately.
    SingleStepSequence { index: usize },
    /// Multi-step sequences entered pending state.
    /// `immediate_index` is set if an immediate hotkey also matches,
    /// enabling standalone fallback on sequence timeout.
    MultiStepSequences {
        indices: Vec<usize>,
        immediate_index: Option<usize>,
    },
    /// Only an immediate hotkey matched.
    Immediate { index: usize },
    /// Nothing matched.
    None,
}

/// Classify all sequence bindings in a scope against a hotkey.
///
/// Iterates the sequences, classifying each prefix. Returns the
/// highest-priority match: `SingleStep` wins over `MultiStep`, which
/// wins over `None`. For `MultiStep`, all matching indices are
/// collected so the runtime can start them as active sequences.
pub(super) fn classify_observation_prefixes<'a>(
    sequences: impl Iterator<Item = &'a BindingSequence>,
    event: &KeyboardObservation,
) -> SequencePrefixMatch {
    let mut matches: Vec<_> = sequences
        .enumerate()
        .filter(|(_, sequence)| sequence.steps()[0].matches(event))
        .collect();
    // Stable sorting preserves the scope's declaration/BindingId order for ties.
    // Only matching prefixes need ranking, not the entire sequence registry.
    matches.sort_by(|(_, left), (_, right)| left.domain_cmp(right));

    if let Some((index, _)) = matches
        .iter()
        .find(|(_, sequence)| sequence.steps().len() == 1)
    {
        SequencePrefixMatch::SingleStep { index: *index }
    } else if !matches.is_empty() {
        SequencePrefixMatch::MultiStep {
            indices: matches.into_iter().map(|(index, _)| index).collect(),
        }
    } else {
        SequencePrefixMatch::None
    }
}

/// Physical-only helper for the existing classification tests.
#[cfg(test)]
fn classify_layer(
    stored: &StoredLayer,
    hotkey: Hotkey,
    device: Option<&DeviceContext<'_>>,
) -> LayerMatch {
    classify_observation(
        stored,
        &KeyboardObservation::from_hotkey(hotkey, crate::key_state::KeyTransition::Press),
        device,
    )
}

#[cfg(test)]
fn classify_sequence_prefixes<'a>(
    sequences: impl Iterator<Item = &'a HotkeySequence>,
    hotkey: Hotkey,
) -> SequencePrefixMatch {
    let sequences: Vec<BindingSequence> = sequences.cloned().map(Into::into).collect();
    classify_observation_prefixes(
        sequences.iter(),
        &KeyboardObservation::from_hotkey(hotkey, crate::key_state::KeyTransition::Press),
    )
}

/// Classify a layer once for both identities: sequences before immediate
/// patterns, retaining the best immediate match for standalone fallback.
/// Runtime and query paths share this classification.
pub(super) fn classify_observation(
    stored: &StoredLayer,
    event: &KeyboardObservation,
    device: Option<&DeviceContext<'_>>,
) -> LayerMatch {
    let seq_match =
        classify_observation_prefixes(stored.sequence_bindings.iter().map(|b| &b.sequence), event);

    match seq_match {
        SequencePrefixMatch::SingleStep { index } => LayerMatch::SingleStepSequence { index },
        SequencePrefixMatch::MultiStep { indices } => {
            let immediate_index = find_immediate_in_layer(stored, event, device);
            LayerMatch::MultiStepSequences {
                indices,
                immediate_index,
            }
        }
        SequencePrefixMatch::None => match find_immediate_in_layer(stored, event, device) {
            Some(index) => LayerMatch::Immediate { index },
            None => LayerMatch::None,
        },
    }
}

/// Find the best immediate pattern in a layer.
/// Physical wins within the layer; declaration order breaks same-domain ties.
/// Source labels do not rank layer bindings (the existing layer contract).
///
/// When device context is provided, bindings with a device filter are only
/// considered if the device matches the filter. Per-device modifier isolation
/// applies: device-filtered bindings match against a device-specific hotkey.
///
/// Returns the index into `stored.bindings`.
fn find_immediate_in_layer(
    stored: &StoredLayer,
    event: &KeyboardObservation,
    device: Option<&DeviceContext<'_>>,
) -> Option<usize> {
    stored
        .bindings
        .iter()
        .enumerate()
        .filter(|(_, binding)| binding_matches_observation(binding, event, device))
        .min_by_key(|(index, binding)| (binding.hotkey().is_none(), *index))
        .map(|(index, _)| index)
}

/// Check whether a binding matches a hotkey, respecting device filters.
pub(super) fn binding_matches_observation(
    binding: &Binding,
    event: &KeyboardObservation,
    device: Option<&DeviceContext<'_>>,
) -> bool {
    let Some(filter) = binding.options().device() else {
        // No device filter — match against aggregate hotkey
        return binding.pattern().matches(event);
    };

    // Binding has a device filter — need device context
    let Some(ctx) = device else {
        // No device context available — can't match device-filtered bindings
        return false;
    };

    if !filter.matches(ctx.info()) {
        return false;
    }

    // Build device-specific hotkey for modifier isolation
    if let Some(device_mods) = ctx.device_modifiers() {
        let mut event = event.clone();
        event.modifiers = device_mods;
        binding.pattern().matches(&event)
    } else {
        // No device modifiers — use aggregate
        binding.pattern().matches(event)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action::Action;
    use crate::binding::BindingId;
    use crate::binding::SequenceBinding;
    use crate::hotkey::Hotkey;
    use crate::key::Key;
    use crate::layer::LayerOptions;
    use crate::layer::StoredLayer;
    use crate::sequence::SequenceOptions;

    fn single_step(key: Key) -> HotkeySequence {
        HotkeySequence::new(vec![Hotkey::new(key)]).unwrap()
    }

    fn two_step(first: Key, second: Key) -> HotkeySequence {
        HotkeySequence::new(vec![Hotkey::new(first), Hotkey::new(second)]).unwrap()
    }

    fn three_step(a: Key, b: Key, c: Key) -> HotkeySequence {
        HotkeySequence::new(vec![Hotkey::new(a), Hotkey::new(b), Hotkey::new(c)]).unwrap()
    }

    fn immediate(key: Key) -> Binding {
        Binding::new(BindingId::new(), Hotkey::new(key), Action::Suppress)
    }

    fn seq_binding(sequence: HotkeySequence) -> SequenceBinding {
        SequenceBinding::new(
            BindingId::new(),
            sequence,
            Action::Suppress,
            SequenceOptions::default(),
        )
    }

    fn layer(bindings: Vec<Binding>, sequence_bindings: Vec<SequenceBinding>) -> StoredLayer {
        StoredLayer {
            bindings,
            sequence_bindings,
            options: LayerOptions::default(),
        }
    }

    // classify_sequence_prefixes

    #[test]
    fn prefixes_empty_sequences_returns_none() {
        let seqs: Vec<HotkeySequence> = vec![];
        let result = classify_sequence_prefixes(seqs.iter(), Hotkey::new(Key::A));
        assert_eq!(result, SequencePrefixMatch::None);
    }

    #[test]
    fn prefixes_no_match_returns_none() {
        let seqs = [single_step(Key::B)];
        let result = classify_sequence_prefixes(seqs.iter(), Hotkey::new(Key::A));
        assert_eq!(result, SequencePrefixMatch::None);
    }

    #[test]
    fn prefixes_single_step_match() {
        let seqs = [single_step(Key::A)];
        let result = classify_sequence_prefixes(seqs.iter(), Hotkey::new(Key::A));
        assert_eq!(result, SequencePrefixMatch::SingleStep { index: 0 });
    }

    #[test]
    fn prefixes_single_step_returns_first_match_index() {
        // Non-matching sequence at index 0, matching at index 1
        let seqs = [single_step(Key::B), single_step(Key::A)];
        let result = classify_sequence_prefixes(seqs.iter(), Hotkey::new(Key::A));
        assert_eq!(result, SequencePrefixMatch::SingleStep { index: 1 });
    }

    #[test]
    fn prefixes_multi_step_match() {
        let seqs = [two_step(Key::A, Key::B)];
        let result = classify_sequence_prefixes(seqs.iter(), Hotkey::new(Key::A));
        assert_eq!(result, SequencePrefixMatch::MultiStep { indices: vec![0] });
    }

    #[test]
    fn prefixes_multiple_multi_step_matches_collected() {
        let seqs = [
            two_step(Key::A, Key::B),
            two_step(Key::A, Key::C),
            two_step(Key::X, Key::Y), // non-matching
        ];
        let result = classify_sequence_prefixes(seqs.iter(), Hotkey::new(Key::A));
        assert_eq!(
            result,
            SequencePrefixMatch::MultiStep {
                indices: vec![0, 1],
            }
        );
    }

    #[test]
    fn prefixes_single_step_wins_over_multi_step() {
        let seqs = [
            two_step(Key::A, Key::B),           // multi-step at index 0
            single_step(Key::A),                // single-step at index 1
            three_step(Key::A, Key::C, Key::D), // multi-step at index 2
        ];
        let result = classify_sequence_prefixes(seqs.iter(), Hotkey::new(Key::A));
        assert_eq!(result, SequencePrefixMatch::SingleStep { index: 1 });
    }

    #[test]
    fn prefixes_first_single_step_wins_when_multiple_match() {
        let seqs = [
            single_step(Key::A), // index 0
            single_step(Key::A), // index 1 (duplicate, ignored)
        ];
        let result = classify_sequence_prefixes(seqs.iter(), Hotkey::new(Key::A));
        assert_eq!(result, SequencePrefixMatch::SingleStep { index: 0 });
    }

    // classify_layer

    #[test]
    fn layer_no_bindings_returns_none() {
        let stored = layer(vec![], vec![]);
        let result = classify_layer(&stored, Hotkey::new(Key::A), None);
        assert_eq!(result, LayerMatch::None);
    }

    #[test]
    fn layer_no_match_returns_none() {
        let stored = layer(vec![immediate(Key::B)], vec![]);
        let result = classify_layer(&stored, Hotkey::new(Key::A), None);
        assert_eq!(result, LayerMatch::None);
    }

    #[test]
    fn layer_immediate_only() {
        let stored = layer(vec![immediate(Key::A)], vec![]);
        let result = classify_layer(&stored, Hotkey::new(Key::A), None);
        assert_eq!(result, LayerMatch::Immediate { index: 0 });
    }

    #[test]
    fn layer_immediate_returns_first_match_index() {
        let stored = layer(vec![immediate(Key::B), immediate(Key::A)], vec![]);
        let result = classify_layer(&stored, Hotkey::new(Key::A), None);
        assert_eq!(result, LayerMatch::Immediate { index: 1 });
    }

    #[test]
    fn layer_single_step_sequence() {
        let stored = layer(vec![], vec![seq_binding(single_step(Key::A))]);
        let result = classify_layer(&stored, Hotkey::new(Key::A), None);
        assert_eq!(result, LayerMatch::SingleStepSequence { index: 0 });
    }

    #[test]
    fn layer_single_step_sequence_wins_over_immediate() {
        let stored = layer(
            vec![immediate(Key::A)],
            vec![seq_binding(single_step(Key::A))],
        );
        let result = classify_layer(&stored, Hotkey::new(Key::A), None);
        assert_eq!(result, LayerMatch::SingleStepSequence { index: 0 });
    }

    #[test]
    fn layer_multi_step_without_immediate() {
        let stored = layer(vec![], vec![seq_binding(two_step(Key::A, Key::B))]);
        let result = classify_layer(&stored, Hotkey::new(Key::A), None);
        assert_eq!(
            result,
            LayerMatch::MultiStepSequences {
                indices: vec![0],
                immediate_index: None,
            }
        );
    }

    #[test]
    fn layer_multi_step_with_immediate_records_fallback() {
        let stored = layer(
            vec![immediate(Key::A)],
            vec![seq_binding(two_step(Key::A, Key::B))],
        );
        let result = classify_layer(&stored, Hotkey::new(Key::A), None);
        assert_eq!(
            result,
            LayerMatch::MultiStepSequences {
                indices: vec![0],
                immediate_index: Some(0),
            }
        );
    }

    #[test]
    fn layer_multi_step_immediate_index_reflects_position() {
        // Immediate for Key::A is at index 1 (Key::X is at index 0)
        let stored = layer(
            vec![immediate(Key::X), immediate(Key::A)],
            vec![seq_binding(two_step(Key::A, Key::B))],
        );
        let result = classify_layer(&stored, Hotkey::new(Key::A), None);
        assert_eq!(
            result,
            LayerMatch::MultiStepSequences {
                indices: vec![0],
                immediate_index: Some(1),
            }
        );
    }

    #[test]
    fn layer_single_step_sequence_wins_over_multi_step() {
        let stored = layer(
            vec![],
            vec![
                seq_binding(two_step(Key::A, Key::B)),
                seq_binding(single_step(Key::A)),
            ],
        );
        let result = classify_layer(&stored, Hotkey::new(Key::A), None);
        assert_eq!(result, LayerMatch::SingleStepSequence { index: 1 });
    }

    #[test]
    fn layer_single_step_sequence_wins_over_multi_step_and_immediate() {
        let stored = layer(
            vec![immediate(Key::A)],
            vec![
                seq_binding(two_step(Key::A, Key::C)),
                seq_binding(single_step(Key::A)),
            ],
        );
        let result = classify_layer(&stored, Hotkey::new(Key::A), None);
        assert_eq!(result, LayerMatch::SingleStepSequence { index: 1 });
    }

    #[test]
    fn layer_non_matching_bindings_skipped() {
        let stored = layer(
            vec![immediate(Key::X), immediate(Key::Y)],
            vec![
                seq_binding(two_step(Key::X, Key::Y)),
                seq_binding(single_step(Key::Z)),
            ],
        );
        let result = classify_layer(&stored, Hotkey::new(Key::A), None);
        assert_eq!(result, LayerMatch::None);
    }
}
