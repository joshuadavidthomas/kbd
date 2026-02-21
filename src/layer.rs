//! [`Layer`] — a named collection of bindings, stackable.
//!
//! Layers are the organizational unit. When active, a layer's bindings
//! participate in matching. Layers stack: most recently activated is
//! checked first. Global bindings act as an always-active base layer.
//!
//! Replaces the v0 mode system (6 types: `ModeOptions`, `ModeDefinition`,
//! `ModeBuilder`, `ModeRegistry`, `ModeController`, mode dispatch module)
//! with two concepts: `Layer` (the definition) and layer operations on
//! the manager (the control).
//!
//! Layer is a builder — construct with `Layer::new("name")`, add bindings
//! with `.bind()`, configure with `.oneshot()` / `.swallow()` / `.timeout()`,
//! then hand to `manager.define_layer(layer)`.
//!
//! # Reference
//!
//! Prior art: `archive/v0/src/mode/` (6 files),
//! `reference/keyd/src/keyboard.h` (layer struct with keymap[256])

// TODO: Layer struct with builder pattern
// TODO: LayerOptions (oneshot depth, swallow unmatched, timeout)
// TODO: LayerId newtype

/// Placeholder — see module docs.
pub struct Layer;

/// Placeholder — see module docs.
pub struct LayerOptions;
