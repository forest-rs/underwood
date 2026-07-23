<!-- Copyright 2026 the Underwood Authors -->
<!-- SPDX-License-Identifier: Apache-2.0 OR MIT -->

# Underwood live showcase

This external app presents one real semantic Underwood document in a native,
resizable window. Window width becomes the document's finite inline constraint;
Underwood performs retained paragraph formation; `imaging` records the resulting
portable `TextScene`; `imaging_vello_cpu` rasterizes it; and `softbuffer` only
presents the final pixels.

Run it from the repository root:

```sh
cargo run --release -p underwood_showcase
```

Controls are shown in the window. Click to place an exact caret, drag for a
visual selection, Shift-click to extend it, and Alt-click to add an independent
caret. Typing, Enter, Backspace, Delete, and the left/right arrow keys execute
revision-checked edits and movement. Native Winit IME preedit is projected
without mutating the document and commits once. `F2` changes paint, `F3`
animates the variable-font weight axis, `F4` shows line evidence, and `F5`
restores the complete authored document.

The mixed English/Arabic “Explore the source” leaf is also actionable. Hover
uses exact shaped-cluster hits to change its paint and request the native link
pointer; press and release on the same semantic node sends a URL-shaped action
to the host. The proof host acknowledges that request in the title bar but does
not launch a browser. Moving beyond the click threshold transfers the original
cluster position into visual-selection policy, so dragging from the link selects
its wrapped Latin and Arabic text instead of activating it.

The editor paragraph deliberately mixes Latin, Arabic, an `ffi` ligature, and
a decomposed combining sequence. Selection geometry follows visual bidi order
without flattening disjoint logical ranges; independent carets publish one
atomic replacement transaction. The title keeps the last meaningful work
observation visible while reporting current preparation and rendering times
separately.

The crate is deliberately outside the production crates. It does not make
Underwood depend on a window toolkit or renderer.

Every visible glyph comes from one `DocumentSnapshot`. Heading roles are
preserved semantically, while this app deliberately keeps role-based block
styling, scrolling, and native accessibility projection outside the proof. A
viewport too short for the complete flow is reported as `CLIPPED` in the window
title instead of silently implying that scrolling already exists.
