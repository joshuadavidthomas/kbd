use std::collections::HashMap;

use super::Dispatcher;
use super::sequence::RegisteredSequenceBinding;
use crate::hotkey::Hotkey;
use crate::hotkey::HotkeySequence;
use crate::hotkey::Modifier;
use crate::layer::StoredLayer;

/// Classification of a single sequence binding's first step against a hotkey.
enum SequencePrefixKind {
    /// The sequence's first step does not match the hotkey.
    None,
    /// The sequence is a single step and matches (immediate fire).
    SingleStep,
    /// The sequence has multiple steps and the first matches (enter pending).
    MultiStep,
}

/// Classify whether a sequence's first step matches a given hotkey.
fn classify_sequence_prefix(sequence: &HotkeySequence, hotkey: &Hotkey) -> SequencePrefixKind {
    if !sequence
        .steps()
        .first()
        .is_some_and(|first_step| first_step == hotkey)
    {
        return SequencePrefixKind::None;
    }

    if sequence.steps().len() == 1 {
        SequencePrefixKind::SingleStep
    } else {
        SequencePrefixKind::MultiStep
    }
}

/// Result of classifying all sequence bindings within a scope against a hotkey.
///
/// Encodes the precedence rule: single-step sequences win over multi-step.
/// Indices refer to positions in the input iterator passed to
/// [`classify_sequence_prefixes`].
///
/// Used directly by the global-bindings path where immediate hotkey lookup
/// is a separate `HashMap` operation. For layer scopes, prefer
/// [`classify_layer`] which combines sequence and immediate
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
pub(super) fn classify_sequence_prefixes<'a>(
    sequences: impl Iterator<Item = &'a HotkeySequence>,
    hotkey: &Hotkey,
) -> SequencePrefixMatch {
    let mut single_step_index: Option<usize> = None;
    let mut multi_step_indices: Vec<usize> = Vec::new();

    for (index, sequence) in sequences.enumerate() {
        match classify_sequence_prefix(sequence, hotkey) {
            SequencePrefixKind::None => {}
            SequencePrefixKind::SingleStep => {
                if single_step_index.is_none() {
                    single_step_index = Some(index);
                }
            }
            SequencePrefixKind::MultiStep => {
                multi_step_indices.push(index);
            }
        }
    }

    if let Some(index) = single_step_index {
        SequencePrefixMatch::SingleStep { index }
    } else if !multi_step_indices.is_empty() {
        SequencePrefixMatch::MultiStep {
            indices: multi_step_indices,
        }
    } else {
        SequencePrefixMatch::None
    }
}

/// Classify all bindings (sequences + immediate hotkeys) in a layer scope.
///
/// Applies the precedence rule: sequences are checked before immediate
/// hotkeys. When multi-step sequences match, the immediate hotkey index
/// is also recorded for standalone-fallback on sequence timeout.
///
/// Both the runtime and query paths call this function, ensuring
/// consistent classification. When a new match type is added (e.g.,
/// tap-hold), adding a variant to [`LayerMatch`] forces both paths
/// to handle it.
pub(super) fn classify_layer(
    stored: &StoredLayer,
    hotkey: &Hotkey,
    aliases: &HashMap<String, Modifier>,
) -> LayerMatch {
    let seq_match =
        classify_sequence_prefixes(stored.sequence_bindings.iter().map(|b| &b.sequence), hotkey);

    match seq_match {
        SequencePrefixMatch::SingleStep { index } => LayerMatch::SingleStepSequence { index },
        SequencePrefixMatch::MultiStep { indices } => {
            let immediate_index = find_immediate_in_layer(stored, hotkey, aliases);
            LayerMatch::MultiStepSequences {
                indices,
                immediate_index,
            }
        }
        SequencePrefixMatch::None => match find_immediate_in_layer(stored, hotkey, aliases) {
            Some(index) => LayerMatch::Immediate { index },
            None => LayerMatch::None,
        },
    }
}

/// Find the first immediate hotkey binding in a layer that matches a hotkey.
///
/// Returns the index into `stored.bindings`. Handles modifier alias
/// resolution for bindings that contain alias modifiers.
fn find_immediate_in_layer(
    stored: &StoredLayer,
    hotkey: &Hotkey,
    aliases: &HashMap<String, Modifier>,
) -> Option<usize> {
    stored.bindings.iter().position(|binding| {
        super::aliases::hotkeys_match_with_aliases(&binding.hotkey, hotkey, aliases)
    })
}

impl Dispatcher {
    /// Return all global sequence bindings sorted by ID for deterministic ordering.
    ///
    /// Both the runtime and query paths need globally-registered sequences in a
    /// consistent order. This helper centralises the filter-free sort so callers
    /// can pass the result straight to [`classify_sequence_prefixes`].
    pub(super) fn sorted_global_sequences(&self) -> Vec<&RegisteredSequenceBinding> {
        let mut seqs: Vec<_> = self.sequence_bindings_by_id.values().collect();
        seqs.sort_by_key(|b| b.id.as_u64());
        seqs
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;
    use crate::action::Action;
    use crate::binding::KeyPropagation;
    use crate::hotkey::Hotkey;
    use crate::hotkey::Modifier;
    use crate::key::Key;
    use crate::layer::LayerBinding;
    use crate::layer::LayerOptions;
    use crate::layer::LayerSequenceBinding;
    use crate::layer::StoredLayer;
    use crate::sequence::SequenceOptions;

    fn no_aliases() -> HashMap<String, Modifier> {
        HashMap::new()
    }

    fn single_step(key: Key) -> HotkeySequence {
        HotkeySequence::new(vec![Hotkey::new(key)]).unwrap()
    }

    fn two_step(first: Key, second: Key) -> HotkeySequence {
        HotkeySequence::new(vec![Hotkey::new(first), Hotkey::new(second)]).unwrap()
    }

    fn three_step(a: Key, b: Key, c: Key) -> HotkeySequence {
        HotkeySequence::new(vec![Hotkey::new(a), Hotkey::new(b), Hotkey::new(c)]).unwrap()
    }

    fn immediate(key: Key) -> LayerBinding {
        LayerBinding {
            hotkey: Hotkey::new(key),
            action: Action::Suppress,
            propagation: KeyPropagation::default(),
        }
    }

    fn seq_binding(sequence: HotkeySequence) -> LayerSequenceBinding {
        LayerSequenceBinding {
            sequence,
            action: Action::Suppress,
            propagation: KeyPropagation::default(),
            options: SequenceOptions::default(),
        }
    }

    fn layer(
        bindings: Vec<LayerBinding>,
        sequence_bindings: Vec<LayerSequenceBinding>,
    ) -> StoredLayer {
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
        let result = classify_sequence_prefixes(seqs.iter(), &Hotkey::new(Key::A));
        assert_eq!(result, SequencePrefixMatch::None);
    }

    #[test]
    fn prefixes_no_match_returns_none() {
        let seqs = [single_step(Key::B)];
        let result = classify_sequence_prefixes(seqs.iter(), &Hotkey::new(Key::A));
        assert_eq!(result, SequencePrefixMatch::None);
    }

    #[test]
    fn prefixes_single_step_match() {
        let seqs = [single_step(Key::A)];
        let result = classify_sequence_prefixes(seqs.iter(), &Hotkey::new(Key::A));
        assert_eq!(result, SequencePrefixMatch::SingleStep { index: 0 });
    }

    #[test]
    fn prefixes_single_step_returns_first_match_index() {
        // Non-matching sequence at index 0, matching at index 1
        let seqs = [single_step(Key::B), single_step(Key::A)];
        let result = classify_sequence_prefixes(seqs.iter(), &Hotkey::new(Key::A));
        assert_eq!(result, SequencePrefixMatch::SingleStep { index: 1 });
    }

    #[test]
    fn prefixes_multi_step_match() {
        let seqs = [two_step(Key::A, Key::B)];
        let result = classify_sequence_prefixes(seqs.iter(), &Hotkey::new(Key::A));
        assert_eq!(result, SequencePrefixMatch::MultiStep { indices: vec![0] });
    }

    #[test]
    fn prefixes_multiple_multi_step_matches_collected() {
        let seqs = [
            two_step(Key::A, Key::B),
            two_step(Key::A, Key::C),
            two_step(Key::X, Key::Y), // non-matching
        ];
        let result = classify_sequence_prefixes(seqs.iter(), &Hotkey::new(Key::A));
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
        let result = classify_sequence_prefixes(seqs.iter(), &Hotkey::new(Key::A));
        assert_eq!(result, SequencePrefixMatch::SingleStep { index: 1 });
    }

    #[test]
    fn prefixes_first_single_step_wins_when_multiple_match() {
        let seqs = [
            single_step(Key::A), // index 0
            single_step(Key::A), // index 1 (duplicate, ignored)
        ];
        let result = classify_sequence_prefixes(seqs.iter(), &Hotkey::new(Key::A));
        assert_eq!(result, SequencePrefixMatch::SingleStep { index: 0 });
    }

    // classify_layer

    #[test]
    fn layer_no_bindings_returns_none() {
        let stored = layer(vec![], vec![]);
        let result = classify_layer(&stored, &Hotkey::new(Key::A), &no_aliases());
        assert_eq!(result, LayerMatch::None);
    }

    #[test]
    fn layer_no_match_returns_none() {
        let stored = layer(vec![immediate(Key::B)], vec![]);
        let result = classify_layer(&stored, &Hotkey::new(Key::A), &no_aliases());
        assert_eq!(result, LayerMatch::None);
    }

    #[test]
    fn layer_immediate_only() {
        let stored = layer(vec![immediate(Key::A)], vec![]);
        let result = classify_layer(&stored, &Hotkey::new(Key::A), &no_aliases());
        assert_eq!(result, LayerMatch::Immediate { index: 0 });
    }

    #[test]
    fn layer_immediate_returns_first_match_index() {
        let stored = layer(vec![immediate(Key::B), immediate(Key::A)], vec![]);
        let result = classify_layer(&stored, &Hotkey::new(Key::A), &no_aliases());
        assert_eq!(result, LayerMatch::Immediate { index: 1 });
    }

    #[test]
    fn layer_single_step_sequence() {
        let stored = layer(vec![], vec![seq_binding(single_step(Key::A))]);
        let result = classify_layer(&stored, &Hotkey::new(Key::A), &no_aliases());
        assert_eq!(result, LayerMatch::SingleStepSequence { index: 0 });
    }

    #[test]
    fn layer_single_step_sequence_wins_over_immediate() {
        let stored = layer(
            vec![immediate(Key::A)],
            vec![seq_binding(single_step(Key::A))],
        );
        let result = classify_layer(&stored, &Hotkey::new(Key::A), &no_aliases());
        assert_eq!(result, LayerMatch::SingleStepSequence { index: 0 });
    }

    #[test]
    fn layer_multi_step_without_immediate() {
        let stored = layer(vec![], vec![seq_binding(two_step(Key::A, Key::B))]);
        let result = classify_layer(&stored, &Hotkey::new(Key::A), &no_aliases());
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
        let result = classify_layer(&stored, &Hotkey::new(Key::A), &no_aliases());
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
        let result = classify_layer(&stored, &Hotkey::new(Key::A), &no_aliases());
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
        let result = classify_layer(&stored, &Hotkey::new(Key::A), &no_aliases());
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
        let result = classify_layer(&stored, &Hotkey::new(Key::A), &no_aliases());
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
        let result = classify_layer(&stored, &Hotkey::new(Key::A), &no_aliases());
        assert_eq!(result, LayerMatch::None);
    }
}
