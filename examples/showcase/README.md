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

Controls are shown in the window. Resize to reflow, press Space for a local text
edit, `P` for a paint-only change, `A` to animate the variable-font weight axis,
`G` for line and baseline evidence, and `R` to restore the initial document.

The crate is deliberately outside the production crates. It does not make
Underwood depend on a window toolkit or renderer.

Every visible glyph comes from one `DocumentSnapshot`. Heading roles are
preserved semantically, while this app deliberately keeps role-based block
styling, scrolling, and native accessibility projection outside the proof. A
viewport too short for the complete flow is reported as `CLIPPED` in the window
title instead of silently implying that scrolling already exists.
