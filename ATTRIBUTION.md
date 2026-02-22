# Attribution and Licensing Notes

keybound is licensed under MIT. This document tracks projects we studied
during design and their license compatibility, to ensure we stay clean.

## Reference implementations (code adaptation permitted)

These projects have MIT or MIT-compatible licenses. We may adapt code,
algorithms, or patterns from them — with appropriate copyright notices
preserved.

### keyd (MIT)

- **Repo**: https://github.com/rvaiya/keyd
- **Copyright**: © 2020 Raheman Vaiya
- **Local copy**: `reference/keyd/`
- **What we reference**: Layer/cache_entry model, chord state machine,
  tap-hold resolution strategies, keyboard event processing pipeline.
- **Status**: Primary reference for the layer system and key
  transformation engine (Phases 3, 5, 6).

### global-hotkey (Apache-2.0 OR MIT)

- **Repo**: https://github.com/tauri-apps/global-hotkey
- **Copyright**: © Tauri Programme within The Commons Conservancy
- **What we reference**: Hotkey parsing patterns, error type design,
  serde serialization approach, HotKey ID generation.
- **Status**: Reference for API ergonomics and string parsing (Phase 1,
  Phase 4.7).

### livesplit-hotkey (MIT OR Apache-2.0)

- **Repo**: https://github.com/LiveSplit/livesplit-core
  (`crates/livesplit-hotkey/`)
- **Copyright**: © Christopher Serr, Sergey Papushin, Cris Hall-Ramos
- **What we reference**: `ConsumePreference` enum design, evdev device
  discovery and permission checking, backend selection logic.
- **Status**: Reference for consume/observe model and backend fallback
  (Phase 4.5).

## Inspiration only — clean room (GPL, not compatible with MIT)

These projects informed our design decisions but their code MUST NOT be
copied or adapted. Any features inspired by these projects must be
implemented from scratch based on the *concepts*, not the code.

### Niri (GPL-3.0)

- **Repo**: https://github.com/YaLTeR/niri
- **Inspired**: Modifier alias concept (`COMPOSITOR` modifier), per-binding
  cooldown and repeat policy, hotkey overlay with visibility control,
  `allow-when-locked` / `allow-when-inhibited` binding options.
- **Relevant to**: Phases 3.4, 4.4, 4.8, 5.5.

### COSMIC (GPL-3.0)

- **Repos**: https://github.com/pop-os/cosmic-comp,
  https://github.com/pop-os/cosmic-settings-daemon
- **Inspired**: XKB keyboard layout awareness, modifier bridging between
  input systems, key repeat rate/delay configuration, compose key
  handling.
- **Relevant to**: Phase 4.9 (XKB layout awareness).

### Zed editor (GPL-3.0+ for editor crates)

- **Repo**: https://github.com/zed-industries/zed
- **Inspired**: Context-aware binding dispatch, binding provenance /
  source tracking (`KeybindSource`), conflict detection and resolution
  UI patterns, introspection API design, pending sequence state for UI
  feedback.
- **Relevant to**: Phases 3.5, 4.1, 4.10, 5.6.
- **Note**: Zed's GPUI framework is Apache-2.0 (see below), but the
  editor-specific crates (keymap_editor, settings) are GPL-3.0+. Treat
  all Zed code as GPL unless the specific crate's `Cargo.toml` says
  `Apache-2.0`.

### Zed GPUI (Apache-2.0)

- **Repo**: https://github.com/zed-industries/zed (`crates/gpui/`)
- **Copyright**: © Zed Industries, Inc.
- **Status**: Apache-2.0 is compatible with MIT for downstream use, but
  adapting code into an MIT project requires: (1) preserving the Apache
  LICENSE and any NOTICE file, (2) stating changes made. If we adapt any
  GPUI code, add the Apache-2.0 notice to the relevant source files.
- **What could be adapted**: Action trait design, keystroke parsing,
  binding matching algorithms.

## Rules

1. **MIT/Apache-2.0 sources**: May adapt code. Preserve original
   copyright notices in adapted files or in this document.
2. **GPL sources**: Clean room only. Read to understand the *concept*,
   close the code, implement from the concept. Do not have GPL source
   open while writing the implementation.
3. **Apache-2.0 sources**: May adapt, but include Apache LICENSE notice
   and state changes if we do.
4. **When in doubt**: Treat it as GPL and clean-room it.
