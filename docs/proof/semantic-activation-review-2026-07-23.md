<!-- Copyright 2026 the Underwood Authors -->
<!-- SPDX-License-Identifier: Apache-2.0 OR MIT -->

# Semantic activation proof review — 2026-07-23

- **Scope:** exact semantic hover, press, cancellation, pointer affordance, and
  host-directed activation in the native showcase
- **Design:** Design-0009, Slice D
- **Bead:** `und-oh0.10.2.5`
- **Unsafe watch:** no `unsafe` added
- **Dependency watch:** no dependency added
- **Local gate:** complete
- **Remote gate:** head run `29981440158` and merge-group run `29981766554`
  passed all eight jobs
- **Landed:** PR #17, squash commit `7f61221`

## Overview

The goal is to prove that the same shaped-cluster map used by editing can route
an application action without moving link or navigation policy into Underwood.
The non-goals are a permanent link role, URL parsing, browser navigation,
visited state, accessibility actions, and a reusable widget API.

**Fence:** The showcase owns action association and host activation; Underwood
owns only exact revision-bound semantic identity and explicitly does not own
URLs, navigation, or a permanent link schema.

## First read: concept and example

One authored inline leaf reads “Explore the source on GitHub” followed by
Arabic text. It is one semantic node, shapes through both LTR and RTL runs, and
wraps at narrow widths. After a committed scene is prepared, the showcase maps
that leaf's `TextId` to the scene's `SemanticId` and associates an external URL
request with it.

Pointer policy calls exact `TextScene::hit_test`; it never calls
`hit_test_closest` for activation. Hover changes only the leaf's paint and asks
the native host for its link pointer. A plain press is held before editor caret
policy. Release on the same semantic identity and scene revision yields one
host action; release elsewhere or after a revision change cancels. Movement
beyond the click threshold transfers the original exact position into visual
selection, allowing a drag that begins on the link to select its wrapped Latin
and Arabic runs. The native host records a visible receipt for the URL request
without launching a browser.

```text
shaped cluster hit
       |
       v
revision + SemanticId -> showcase ActionRegistry
                              |
                              v
                   hover / press / release
                       |             |
                       v             v
                paint + cursor   host URL receipt
```

## Glossary

- **Exact hit:** a point contained by a prepared shaping cluster.
- **Semantic identity:** the opaque document-node identity returned by the hit.
- **Action registry:** the showcase-local mapping from semantic identity to an
  application action.
- **Activation receipt:** evidence that the native host accepted the action
  request; it is not browser navigation.
- **Action visual:** idle, hovered, or pressed paint identity for the authored
  leaf.

## Second read: invariants and edge cases

1. The registry is bound to one committed `DocumentRevision`. A registry from
   another revision resolves no action.
2. A binding must resolve to a semantic fragment in the prepared scene, and two
   actions may not silently claim the same semantic node.
3. Only an unmodified primary-button press starts action policy. Shift- or
   Alt-modified presses continue through editor selection policy.
4. A press stores semantic identity, action value, scene revision, exact text
   position, and pointer origin. Release must match the first three and remain
   within the click threshold.
5. Moving outside while pressed removes the pressed paint and reports that
   release will cancel. Moving beyond the click threshold becomes a visual text
   drag from the exact pressed position; it does not manufacture a broad
   semantic rectangle.
6. A non-action exact text hit retains the text pointer and ordinary editor
   behavior. Empty space retains the arrow pointer.
7. Hover and press change a paint slot only. They publish no document revision
   and perform no analysis, font selection, shaping, flow, or geometry work.
8. The host receives an example-owned `VisitUrl` value. No production crate
   contains a URL, action registry, Winit cursor, or activation callback.

## Executable evidence

`wrapped_bidi_semantic_action_activates_from_exact_hits` scans actual cluster
interiors in a 300-unit scene. It requires more than one visual line and both
even and odd bidi levels, activates exact hits in both visual regions, and
observes one action per click with no caret. Its follow-up preparation reports
zero shaping and flow with non-zero paint work.

`semantic_action_cancels_outside_and_non_action_text_still_edits` presses the
action, releases over the editor paragraph, and observes no activation. A
second ordinary click reaches editor policy and creates a selection.

`dragging_from_action_text_selects_instead_of_activating` begins on the link's
Latin cluster, crosses the threshold into its wrapped Arabic run, and proves
that Arabic selection geometry appears with no host action. The core
`visual_selection_uses_the_reciprocal_caret_path` regression proves that this
selection remains valid in both drag directions when bidi affinity aliases
expose only the reciprocal traversal.

`host_accepts_semantic_activation_and_pointer_affordance` proves the native
boundary preserves both the URL-shaped request and the link pointer request.

The focused showcase suite passes 25 tests. Release-mode native inspection at
1100 by 800 confirms that the actionable leaf is visibly distinct, wraps, and
shows the expected mixed English/Arabic order without changing the ten-block
document structure.

## Adversarial review result

The correctness review found two must-fix overclaims before landing. First, the
editor status said the host had activated an action before the host had received
it; it now says that activation was requested, while the host alone records the
receipt. Second, holding every action press made the authored text impossible
to select from that direction and exposed a reciprocal wrapped-bidi caret path.
The click/drag transfer and direction-independent core selection regressions
close both failures.

The real-versus-mirage audit's most dangerous gap remains browser navigation:
the executable proof stops at a typed host receipt. The docs and window title
say exactly that, and no production crate contains a URL, action registry,
cursor icon, or navigation callback. No unsafe or dependency finding remains.

## Real-versus-mirage boundary

**Real:** exact cluster containment, revision-bound semantic lookup, deferred
click-versus-drag policy, same-node release validation, paint-only retained
work, native pointer selection, and host receipt are executable.

**Not claimed:** the receipt does not open a browser. The URL is an inert
showcase value, the registry is rebuilt from one known authored leaf, and no
general action model or permanent semantic link role exists. Calling this a
browser-capable link widget would be a mirage.

## Landed result

PR #17 landed as `7f61221`. Local formatting, Taplo, headers, spelling,
metadata/policy, workspace clippy and tests, rustdoc, Rust 1.92 MSRV,
bare-metal, and WebAssembly gates pass; the same eight-job matrix passed on the
final head and again in the merge queue. The discovered multi-source extended-
grapheme work remains separately gated as `und-oh0.10.2.6`.
