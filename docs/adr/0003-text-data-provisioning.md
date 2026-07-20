# ADR-0003: Text-data provisioning and identity

- **Status:** Open — investigation and human ratification required
- **Bead:** `und-oh0.7.1`
- **Authority:** `UNDERWOOD_HANDOVER.md` §§15.7, 35.1

## Goal

Define how Unicode, segmentation, bidi, line-break, locale, and hyphenation
data enter Underwood, participate in cache and replay identity, and remain
viable for no_std and WebAssembly consumers.

## Non-goals

- Replacing Parley or ICU4X analysis algorithms.
- Standardizing host locale selection.
- Shipping every language capability in every binary.

## Fence

The text-data layer owns immutable prepared data bundles and their identity; it
explicitly does not own ambient locale selection or inner-loop algorithm
dispatch.

## Constitutional invariants

1. Text data is explicit and versioned.
2. Providers load immutable prepared bundles outside hot loops.
3. Results depending on data identity name it in cache and replay keys.
4. Unsupported capability is diagnosed, never silently approximated.
5. Minimal and full tiers publish compressed and resident budgets.
6. Primitive edits remain replayable; data-dependent commands name or migrate
   their environment.

## Options

### Baked minimal data plus host extensions

Keep baseline data compiled into Underwood and load optional bundles through a
provider. This gives a reliable floor but must not hide version identity.

### Fully provider-supplied data

Require the host to provide every bundle. This maximizes explicitness but makes
simple labels and ordinary applications harder to construct.

### Tiered distributions

Publish a documented minimal bundle, feature-gated complex-script and locale
packs, host hyphenation, and a full compositor distribution. This is the
handover recommendation and needs concrete size evidence.

## Required evidence

- Exact Parley and ICU4X data entry points at the pinned revision.
- Minimal, complex-script, locale-tailored, and full bundle inventories.
- Compressed and resident WebAssembly size measurements.
- Cache invalidation trace across a data identity upgrade.
- Replay trace for data-dependent editor commands.
- Unsupported-locale and unavailable-hyphenation diagnostics.
- Multilingual segmentation and line-break corpus coverage.

## Open semantic questions

- Which Unicode version fields and dataset hashes form identity.
- Whether minimal data is compiled in or distributed as an ordinary bundle.
- Hyphenation licensing, ownership, and fallback.
- Bundle compatibility and authentication.
- Host capability negotiation and degraded behavior.

## Decision

Pending evidence and human ratification.

## Migration

Data identity and provider format changes require explicit version migration.
No unversioned default is grandfathered.

## Proof impact

This decision gates `international-text-data`, data-dependent preparation and
editing commands, deterministic replay, and the first semantic-to-scene
campaign.
