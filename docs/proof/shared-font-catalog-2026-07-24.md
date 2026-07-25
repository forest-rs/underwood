<!-- Copyright 2026 the Underwood Authors -->
<!-- SPDX-License-Identifier: Apache-2.0 OR MIT -->

# Shared font catalog proof — 2026-07-24

## Judgment

`FontSet` now serves as the single application font-catalog snapshot expected
by the Overstory integration checkpoint. It supports empty, embedded-only,
system-only, and embedded-plus-system construction without introducing an
Underwood-owned UI builder or a second font abstraction.

The earlier exploratory API checkpoint was directionally useful but constructed
Fontique with sharing disabled. The production slice corrects that mismatch:
the explicit `underwood_parley/std` feature creates Fontique's synchronized
shared collection and source cache, and `system-fonts` implies `std`.

## Ownership and call site

```text
Overstory TextResourcesBuilder
    ├── chooses embedded fonts and platform discovery policy
    ├── completes generic-family and fallback configuration
    └── owns one FontSet snapshot
             ├── clone -> UI-local ParleyParagraphEngine
             ├── clone -> UI-local ParleyParagraphEngine
             └── clone -> UI-local ParleyParagraphEngine
```

Underwood owns catalog validation and consumption. Overstory remains free to
own builder errors, discovered-family presentation, and application policy.
Analyzer, shaper, query state, paragraph preparation, and layout caches remain
engine-local.

Configuration is a build-time operation. Generic-family and fallback mappings
must be complete before clones enter paragraph engines; runtime registration
and the cache-generation protocol it would require remain deliberately absent.

## Executable claims

Focused unit tests prove:

1. `FontSet::empty` has no registered families and does not discover the host.
2. Two embedded fixture fonts produce sorted, deduplicated, stable family names.
3. A system-only catalog is constructible when `system-fonts` is enabled, while
   platform family names remain absent from stable registered-family reporting.
4. A registration through one `std` clone becomes visible through another,
   proving Fontique collection sharing rather than catalog copying.
5. An embedded font has the same Fontique blob identity through both clones,
   proving that its bytes are not copied or re-registered.
6. A file-backed source loaded through one source-cache clone remains available
   through another after the source file is removed, proving shared cache
   backing rather than coincidental independent loads.

The default-feature check separately proves that `underwood_parley` remains a
`no_std + alloc` crate. In that mode Fontique does not offer synchronized shared
catalog/cache storage: catalog records clone locally, while registered bytes
remain shared blobs. This is the precise boundary behind “where Fontique
permits it,” not an undocumented `std` requirement in Underwood core.

## API and migration

The slice adds:

- `FontSet::empty()`;
- `FontSet::registered_family_names()`;
- the opt-in `underwood_parley/std` feature.

No existing call site changes. Native hosts already enabling `system-fonts`
automatically receive shared catalog/cache backing. Embedded-only native hosts
that distribute one catalog across engines should additionally enable `std`.
Deterministic and bare-metal hosts can retain the default feature set.

## Limits

- “Registered family” means application-supplied memory fonts, not the
  machine-dependent platform catalog.
- The public API does not support runtime font registration.
- System discovery occurs when the application resource owner invokes
  `with_system_fonts`; clones of that finished set reuse the resulting Fontique
  system snapshot.
- Sharing does not merge engine-local analyzer, shaper, query, or paragraph
  caches. That separation is intentional.
- The test observes Fontique's public sharing semantics rather than reaching
  into its private implementation.

No production dependency or `unsafe` code was added.
