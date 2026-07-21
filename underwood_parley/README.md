<!-- Copyright 2026 the Underwood Authors -->
<!-- SPDX-License-Identifier: Apache-2.0 OR MIT -->

# `underwood_parley`

`underwood_parley` is the pinned, `no_std + alloc` Parley Core adapter for
Underwood's pre-stable paragraph-preparation contract. It accepts only
caller-supplied font bytes and never enables system font discovery.

The adapter owns analysis and shaping scratch, retains Unicode analysis across
shaping-style changes, copies every shaped result into Underwood-owned records,
and preserves paint boundaries as source and clip metadata without making them
shaping inputs. Complete Underwood shaping runs supply font size, language,
OpenType features, and variable-font coordinates to the pinned `parley_core`
seam.
