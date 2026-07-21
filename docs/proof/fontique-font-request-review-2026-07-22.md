# Fontique font-request Lynx and Rook review — 2026-07-22

- **Scope:** Design-0004, computed font requests, the Fontique catalog and
  selection adapter, resolved scene evidence, headless proof, benchmark caller,
  and CPU poster
- **Review modes:** Lynx adversarial correctness; Rook real-versus-mirage audit
- **Fontique/Parley revision:** `45da4a90248b1600277a4294b70d8bfde5ca8e97`
- **Snapshot:** 1600 × 1000 RGBA8, PNG SHA-256
  `5e0d94cb093a176ca53d422d26ec5d691d8a4c559dbe372817cc25ffcf7057db`
- **Unsafe watch:** no `unsafe` in Underwood-owned Rust; Fontique is built
  without system-font features and every deterministic catalog disables system
  discovery
- **Result:** all Must findings are resolved locally; the reusable
  Fontique-to-Parley cluster-selector seam remains explicitly tracked by
  `und-oh0.2.6`
- **Performance:** same-machine product measurements and the retained-path
  correction are recorded in
  `docs/proof/semantic-scene-benchmark-2026-07-22.md`

## Lynx review

### Summary judgment

The branch replaces the ordered-font shortcut with real Fontique matching
without moving a matcher into Underwood. The public request is validated,
owned Parlance vocabulary; the adapter's caller-supplied catalog is
deterministic; selected resources, synthesis, and final normalized coordinates
survive into `TextScene`; and font-selection work has its own observable stage.

### Must fix — resolved

1. **Unsupported fallback keys initially disappeared silently.** Fontique's
   `Collection::set_fallbacks` returns `false` for untracked script/language
   pairs. `FontSet::with_fallbacks` now converts that result into
   `AdapterErrorKind::UnsupportedFallback`, with a focused test. Unknown family
   names likewise fail as `UnknownFamily` during catalog construction.
2. **A known family without cluster coverage must not become accidental
   `.notdef` success.** The cluster callback accepts only Fontique candidates
   that improve coverage (`Keep`) or complete it (`Complete`); a Roboto-only
   Arabic proof now reaches `PreparationErrorKind::MissingFont`.
3. **A missing-family proof must assert the stable diagnostic, not merely any
   error.** The public headless path now requires
   `SceneError::preparation() == Some(MissingFont)` for both absent-family and
   absent-coverage cases.
4. **All query-selectable faces must satisfy the metrics invariant used by
   scene scaling.** Catalog construction now validates `units_per_em` for every
   face returned by Fontique registration, not only face zero, before the
   private copy path relies on that invariant.
5. **`selection()` is ambiguous in a document engine.** The work-report API is
   now `font_selection()`, and its records are explicitly selected clusters,
   not editor selections or Fontique candidate scans.

Good catch: rejecting a non-covering but otherwise valid primary font is what
turns the fallback proof from “some glyphs appeared” into actual coverage
evidence.

### Should fix

- Keep the local cluster callback conformance-locked to high-level Parley until
  `und-oh0.2.6` supplies a reusable upstream seam. Emoji-family injection and
  richer partial-coverage cases are intentionally not claimed by this slice.
- Retain the structural qualifier on family canonicalization. Underwood parses
  CSS source into owned single/list vocabulary but does not duplicate
  Fontique's matcher-equivalent family-name normalization.
- Continue reporting selection records as cluster resolutions. Candidate scan
  counts and cache internals would require an upstream Fontique observation
  seam and are not implied by `WorkReport`.

### Could improve

- Add resolved family/face metadata only when a diagnostic consumer proves
  that exact bytes, face index, request, and synthesis are insufficient.
- Add deterministic system-font integration only with generation identity and
  collection-change invalidation; do not weaken the checked-in proof to depend
  on host state.

### Suggested tests

- Structured family parsing, ownership, duplicate rejection, and finite
  weight/width/style validation — present.
- Named Roboto and named Noto resource selection — present.
- Missing primary followed by configured `Arab`/`ar` fallback — present.
- Known primary with no cluster coverage and no fallback — present.
- Weight/width synthesis plus explicit-axis last-wins precedence — present.
- Synthetic skew retention and execution by the proof renderer — present.
- Font-request-only invalidation: zero analysis, one itemization/selection/shape
  paragraph, one reused sibling — present.
- Unknown generic family and unsupported fallback configuration — present.
- Empty, combining-mark, partial-coverage, and emoji clusters — deferred to
  `und-oh0.2.6`'s shared conformance corpus.

### Unsafe Watch

Underwood-owned source contains no `unsafe`. The pinned Fontique dependency
permits platform FFI internally, but this workspace disables its system-font
feature and constructs collections with `system_fonts: false`. Cross-target
`no_std` checks cover both production crates.

## Rook audit

### Summary judgment

This is a real resolver integration, not a renamed ordered list. The strongest
evidence is the combination of exact retained bytes, different raw synthesis
requests, equal final coordinates after an explicit-axis override, stable
missing-font diagnostics, and paragraph-local negative-work counters. The
architecture prose and executable path now agree on who owns matching.

### Mirage risks

- **Mirage:** `FontSet` is a deterministic caller-supplied memory catalog, not
  platform font discovery, live database mutation, or a complete application
  fallback policy.
- **Mirage:** `font_selection().records()` counts clusters for which a font was
  selected. It does not expose candidate scans, collection-cache hits, or the
  cost of loading a source.
- **Mirage:** retained `embolden` and `skew` are portable suggestions, not a
  promise that every renderer executes them. The current CPU proof executes
  skew and explicitly does not claim embolden fidelity.
- **Mirage:** structural family canonicalization does not make differently
  cased or otherwise matcher-equivalent names share an Underwood cache key.
- **Mirage:** the local cluster callback is not a new Underwood font-matching
  subsystem, but it is still one implementation of semantics also present in
  high-level Parley and can drift until `und-oh0.2.6` is resolved.

### Real strengths

- **Real:** family, weight, width, and style participate in computed shaping
  identity while text-only analysis remains reusable.
- **Real:** Fontique receives the complete attributes and itemized
  script/language key, owns candidate order and synthesis, and returns the
  exact blob/index consumed by Parley Core.
- **Real:** one public document directly selects both bundled families, reaches
  Noto Kufi from an absent primary through `Arab`/`ar`, and retains synthetic
  oblique evidence.
- **Real:** Fontique's synthesized axes are observable independently of the
  final normalized coordinates; the headless proof demonstrates that an
  explicit `wght` overrides a different synthesized `wght`.
- **Real:** the checked-in poster visibly exhibits Fontique-selected weight and
  width instances and renders the supported synthetic skew through `imaging`
  and `imaging_vello_cpu`.
- **Real:** all core types remain `no_std + alloc`; no production or development
  dependency was added by the campaign.
- **Real:** an initially unexplained 20–28% retained-path regression was reduced
  to a measured 6–10% total delta by removing cache-hit key clones and making
  absent synthesis evidence allocation-free.

### Most dangerous gap

The approximately forty-line `select_font` callback in `underwood_parley`
must stay behaviorally aligned with high-level Parley's private `FontSelector`.
It is narrow glue required by the current Parley Core callback, but future
emoji injection or partial-coverage changes could diverge silently. Bead
`und-oh0.2.6` owns a reusable upstream primitive or a common conformance corpus
and an explicit removal gate.

### Suggested tests

- Run the new Fontique selector cases on every host matrix with only checked-in
  fonts and exact resource assertions.
- Extend `und-oh0.2.6` with composed/decomposed clusters, combining marks,
  partial coverage, variation selectors, and emoji-family injection.
- Preserve an exact pixel snapshot for the synthetic-skew specimen, while
  keeping semantic assertions primary so a stable but incorrect picture cannot
  pass alone.
