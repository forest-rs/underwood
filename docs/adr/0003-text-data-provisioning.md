# ADR-0003: Text-data provisioning and identity

- **Status:** Accepted
- **Accepted:** 2026-07-21 by Bruce Mitchener
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

## Evidence snapshot

The upstream audit on 2026-07-21 used Parley `main` at
[`45da4a90248b1600277a4294b70d8bfde5ca8e97`](https://github.com/linebender/parley/commit/45da4a90248b1600277a4294b70d8bfde5ca8e97).

The current entry points are compiled into `parley_core`:

- `parley_data` supplies a baked, locale-invariant composite property lookup;
- `icu_properties`, `icu_normalizer`, and `icu_segmenter` are built with
  `compiled_data`;
- `AnalysisDataSources::new()` constructs these sources internally;
- the `complex-scripts` feature switches word and line segmentation from the
  non-complex constructor to dictionary-backed constructors;
- `AnalysisDataSources` is currently public only as a documented temporary
  shaping escape hatch and has no bundle identity;
- Parley supplies no hyphenation dictionary or provider contract.

The Parley manifest declares ICU4X 2.1.x-compatible dependencies, while its
audited lockfile resolves the ICU4X code and data crates to 2.2.0. A crate
version alone is therefore not a sufficient replay identity.

The generated `parley_data` source is 93,659 bytes. Its three static tables
contain 2,176 `u8`, 13,136 `u16`, and 2,868 `u32` entries: 39,920 bytes of raw
table payload before object-format padding, dead-code elimination, compression,
or the additional ICU4X datasets. This is an inventory fact, not a WebAssembly
bundle-size claim.

The audited Parley crates are Apache-2.0 OR MIT. The audited ICU4X crates declare
Unicode-3.0. Future hyphenation and locale packs require their own per-artifact
license inventory; compatibility may not be inferred from the provider code's
license.

## Data entry-point assessment

| Need | Current seam | Assessment |
| --- | --- | --- |
| Composite script/category/grapheme/bidi flags | `parley_data::Properties::get` | Compact and tested, but baked and identity-free |
| Grapheme segmentation | ICU4X compiled constructor | Available; provider not injectable |
| Word and line segmentation | ICU4X compiled constructors | Available in baseline and dictionary modes |
| Bidi properties and mirroring | Parley composite data plus ICU4X maps | Available; identity split across sources |
| Canonical normalization | ICU4X compiled constructors | Available; identity not surfaced |
| Script short names | ICU4X property names | Available; identity not surfaced |
| Locale tailoring | No Underwood/Parley bundle seam | Gap |
| Hyphenation | No Parley data source | Gap |
| Bundle negotiation | None | Gap |
| Cache/replay identity | None | Constitutional gap |

The current public `AnalysisDataSources` shape is not an acceptable long-lived
provider API. It leaks upstream construction details while providing neither
an immutable manifest nor a capability negotiation result.

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

## Proposed distribution tiers

The tiers are capability sets, not Cargo feature names. A build may deliver a
tier as compiled data, an immutable external bundle, or both, but the manifest
and resulting identity must be the same.

| Tier | Required inventory | Explicitly absent |
| --- | --- | --- |
| `minimal` | Parley composite properties, normalization, grapheme segmentation, non-dictionary word/line segmentation, bidi/mirroring | Dictionary segmentation, locale tailoring, hyphenation |
| `complex-segmentation` | `minimal` plus dictionary word/line segmentation for the scripts supported by the audited ICU4X dataset | Hyphenation and product locale policy |
| `locale` | Named locale-tailoring packs and negotiation metadata | Unlisted locales and unlicensed dictionaries |
| `hyphenation` | Named language dictionaries, pattern version, provenance, and license | Silent algorithmic or character-break fallback |
| `compositor-full` | All project-supported segmentation, locale, and hyphenation packs plus conformance corpus manifest | Unbounded “whatever the host has” inputs |

`minimal` is a reliable construction floor, not a claim of correct complex
segmentation for every script. Requesting an absent capability returns a
structured diagnostic.

## Candidate identity

Every immutable bundle manifest must contain at least:

- bundle format major and minor;
- content digest over canonical manifest and payload bytes;
- Unicode version;
- CLDR version when locale data participates;
- generator name, version, and source revision;
- Parley data schema revision;
- ICU4X component and data-package versions;
- capability set and covered locale/language/script identifiers;
- segmentation and hyphenation algorithm identifiers;
- per-component license SPDX expression and provenance;
- endianness or canonical decoding declaration;
- compatibility range and required engine capabilities.

The content digest is the cache identity. Human-readable versions explain it
and support migration; they do not replace it. Authentication, if required by a
host, signs the digest and manifest without changing semantic identity.

Prepared analysis, itemization, shaping, line-break, data-dependent command,
and replay keys include the smallest applicable data identities. A paint-only
cache must not acquire a Unicode-data dependency merely because a global
environment object exists.

## Budget methodology

The future text-data wind tunnel must live in a separate top-level workspace
crate and must not add dev-dependencies to a core crate.

For every tier it will publish:

1. exact Rust toolchain, target, linker, optimization flags, Parley revision,
   ICU4X lock entries, and bundle digest;
2. raw `.wasm` bytes and bytes after the project's declared transport
   compression;
3. incremental bytes versus an empty harness and versus the preceding tier;
4. static linear-memory bytes after instantiation;
5. provider heap bytes immediately after load and after one warm multilingual
   corpus pass;
6. peak transient load bytes and load latency;
7. analysis throughput and allocations for the same corpus;
8. dead-code-elimination checks proving every advertised capability is
   exercised.

Compressed size is measured from the emitted artifact, not the generated Rust
source. Resident size includes decoded tables, indices, allocator overhead, and
provider-owned caches. Shared pages are reported separately rather than
subtracted opportunistically.

The proposed admission rule is:

- `minimal` may add at most 256 KiB compressed and 1 MiB resident to the empty
  WebAssembly harness;
- `complex-segmentation` may add at most a further 512 KiB compressed and 2 MiB
  resident;
- each optional locale or hyphenation pack publishes its own cap before
  admission;
- `compositor-full` has no inherited “unlimited” exemption: its measured cap is
  a ratification input;
- exceeding a cap requires an explicit size-versus-coverage decision, never a
  renamed tier.

These are proposed experiment gates, not measured claims. The first wind-tunnel
run may demonstrate that a cap should change, but it may not silently waive it.

## Replay and upgrade traces

Each trace header names source text digest, command schema, bundle identities,
font identities, locale request, and expected capability result.

Required cases:

1. Replay a word-delete and line-boundary command under the same identities;
   operation and resulting snapshot digests must match.
2. Replay under a newer data identity. A primitive byte-range edit remains
   valid; a data-dependent command must either migrate through a named rule or
   reject replay with both identities.
3. Upgrade one analysis bundle while paint and fonts remain unchanged. Only
   the data-dependent cache frontier invalidates.
4. Request Thai dictionary segmentation from `minimal`; receive
   `missing_capability`, never character fallback presented as full support.
5. Request an unavailable hyphenation language; receive
   `unsupported_language` with installed capabilities.
6. Load a bundle whose digest, format, or license manifest is invalid; publish
   no partial provider state.

Diagnostics include requested capability, locale/language, installed bundle
ids, engine compatibility, fallback if explicitly authorized, and remediation.

## Conformance inventory

The corpus must cover grapheme, word, line-break, normalization, bidi, emoji,
mixed-script, and dictionary-segmentation cases. Expected results name the
Unicode/CLDR/data identities that produced them. A corpus update and a data
upgrade are distinct reviewable changes.

Hyphenation adds language-specific license fixtures and break-opportunity
expectations. It cannot borrow the Unicode segmentation corpus as proof.

## Recommendation

Choose **tiered distributions** with an immutable manifest and content digest.
Use the current baked/compiled Parley path only as the first measured
`minimal` implementation. Before Underwood stabilizes prepared cache or replay
contracts, pursue an upstream Parley Core seam that accepts an immutable
data-source handle and exposes its identity.

Do not build a generalized mutable provider in the analysis hot loop. Loading,
verification, and capability negotiation happen before preparation; the inner
pipeline receives borrowed immutable prepared data.

## Open semantic questions

- Whether minimal data is compiled in or distributed as an ordinary bundle.
- Hyphenation licensing, ownership, and fallback.
- Host capability negotiation and degraded behavior.
- Whether the proposed 256 KiB/1 MiB and 512 KiB/2 MiB WebAssembly experiment
  caps are accepted.
- Who owns the first hyphenation corpus and license review.
- Whether authentication is required for local embedded bundles or only for
  remotely supplied bundles.

## Decision

Accepted on 2026-07-21.

Underwood adopts tiered immutable text-data distributions, content-digest
identity, explicit capability diagnostics, per-artifact licensing, replay-key
participation, and the proposed WebAssembly experiment gates. The current
Parley baked/compiled path is admitted only as the first measured `minimal`
implementation.

The provider encoding, authentication policy, hyphenation datasets, and
production dependency set remain unchosen. Each requires evidence and its
normal review gate.

## Migration

Data identity and provider format changes require explicit version migration.
No unversioned default is grandfathered.

## Proof impact

This decision gates `international-text-data`, data-dependent preparation and
editing commands, deterministic replay, and the first semantic-to-scene
campaign.
