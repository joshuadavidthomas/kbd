use super::Dispatcher;
use super::sequence::RegisteredSequenceBinding;
use crate::hotkey::Hotkey;
use crate::hotkey::HotkeySequence;
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
/// [`classify_scope_sequences`].
///
/// Used directly by the global-bindings path where immediate hotkey lookup
/// is a separate `HashMap` operation. For layer scopes, prefer
/// [`classify_layer_scope`] which combines sequence and immediate
/// classification into [`ScopeMatch`].
pub(super) enum ScopeSequenceMatch {
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
pub(super) enum ScopeMatch {
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
pub(super) fn classify_scope_sequences<'a>(
    sequences: impl Iterator<Item = &'a HotkeySequence>,
    hotkey: &Hotkey,
) -> ScopeSequenceMatch {
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
        ScopeSequenceMatch::SingleStep { index }
    } else if !multi_step_indices.is_empty() {
        ScopeSequenceMatch::MultiStep {
            indices: multi_step_indices,
        }
    } else {
        ScopeSequenceMatch::None
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
/// tap-hold), adding a variant to [`ScopeMatch`] forces both paths
/// to handle it.
pub(super) fn classify_layer_scope(stored: &StoredLayer, hotkey: &Hotkey) -> ScopeMatch {
    let seq_match =
        classify_scope_sequences(stored.sequence_bindings.iter().map(|b| &b.sequence), hotkey);

    match seq_match {
        ScopeSequenceMatch::SingleStep { index } => ScopeMatch::SingleStepSequence { index },
        ScopeSequenceMatch::MultiStep { indices } => {
            let immediate_index = find_immediate_in_layer(stored, hotkey);
            ScopeMatch::MultiStepSequences {
                indices,
                immediate_index,
            }
        }
        ScopeSequenceMatch::None => match find_immediate_in_layer(stored, hotkey) {
            Some(index) => ScopeMatch::Immediate { index },
            None => ScopeMatch::None,
        },
    }
}

/// Find the first immediate hotkey binding in a layer that matches a hotkey.
///
/// Returns the index into `stored.bindings`.
fn find_immediate_in_layer(stored: &StoredLayer, hotkey: &Hotkey) -> Option<usize> {
    stored
        .bindings
        .iter()
        .position(|binding| binding.hotkey == *hotkey)
}

impl Dispatcher {
    /// Return all global sequence bindings sorted by ID for deterministic ordering.
    ///
    /// Both the runtime and query paths need globally-registered sequences in a
    /// consistent order. This helper centralises the filter-free sort so callers
    /// can pass the result straight to [`classify_scope_sequences`].
    pub(super) fn sorted_global_sequences(&self) -> Vec<&RegisteredSequenceBinding> {
        let mut seqs: Vec<_> = self.sequence_bindings_by_id.values().collect();
        seqs.sort_by_key(|b| b.id.as_u64());
        seqs
    }
}
