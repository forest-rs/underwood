# Reusable text-tools campaign review — 2026-07-25

## Summary judgment

Design-0014 is ready for protected landing. One public path now performs
source-complete projection, Parley Engine analysis and shaping, reusable line
formation, exact region flow, accepted-line adjustment, portable scene
materialization, interaction, and renderer adaptation. The aggregate proof is
**measured**: its workloads, work ledgers, allocation observations, region
transcripts, snapshots, and PDF artifact are reproducible.

This is not a claim of complete CSS Text conformance, universal justification,
viewer-independent PDF extraction, pixel-snapped presentation, or O(change)
scene publication. Those boundaries are explicit below and tracked outside
this campaign.

## Better than Parley, and not

Underwood does not replace Parley's font selection, Unicode analysis, or
shaping. It uses Fontique and `parley_engine` for those jobs. The useful
comparison is therefore Underwood's document/preparation/scene architecture
against high-level Parley's all-owning `Layout`, not Underwood against Parley
as a complete text stack.

| Concern | Underwood advantage | Parley advantage |
|---|---|---|
| Authored source | Projection preserves revision-bound authored provenance through whitespace collapse, generated composition, semantics, interaction, and PDF | Direct string and style builders are simpler when source and presented text are identical |
| Layout topology | One resumable slot protocol composes rectangles, floats, exclusions, columns, and continuation without teaching the line former page policy | `Layout` is a mature, convenient unit for ordinary paragraph breaking and repeated rebreaking/alignment |
| Retained work | Paragraph-local invalidation, bounded cross-identity reuse, and an explicit work trace distinguish shaping, formation, adjustment, and paint | A built `Layout` can rebreak and realign without rebuilding shaping; Underwood still deep-publishes document-scale scene output |
| Editing identity | Immutable document revisions, generated-source IME epochs, exact semantic positions, and multiple visual bidi selections survive across one portable scene contract | Parley already has a broader high-level editing façade: word, line, and document movement, inline-box-aware cursors, and AccessKit conversions |
| Output | Renderer-neutral scene records feed imaging, native interaction, diagnostics, and Krilla PDF from the same adjusted geometry | Parley exposes straightforward positioned runs and inline boxes and already carries decoration and quantization behavior Underwood lacks |
| Architecture | Reusable projection, formation, region, and adjustment stages can be tested or upstreamed without an Underwood document | Fewer layers and less ceremony make Parley the better default when a consumer simply needs a rich paragraph layout |
| Evidence | Deterministic work ledgers, region transcripts, allocation wind tunnels, snapshots, and explicit proof states make performance and non-claims inspectable | Parley's longer-lived implementation, users, examples, and editing surface provide maturity Underwood has not yet earned |

Underwood is better for the project only where these extra invariants are
required: source-complete native documents, retained multi-paragraph work,
arbitrary region flow, portable scenes, and exact revisioned interaction. It
is currently worse for inline objects, decorations, accessibility integration,
pixel-grid presentation, complete editor convenience, implementation maturity,
and steady-state document-scale allocation cost.

The architectural bet is not that every distinction must remain downstream.
Identity-free algorithms that genuinely depend only on public Parley Engine
facts should move upstream when their ownership is clear. Underwood should
retain the document revision, semantic projection, region orchestration,
portable-scene, and host-protocol boundaries that high-level Parley does not
try to own.

## Must fix

All Must findings discovered during final review are resolved:

1. Finite visible columns could exhaust `RegionFlow` and surface
   `PreparationErrorKind::InvalidOutput` as a fatal showcase host error.
   Visible columns now remain bounded while a separate off-page continuation
   accepts overflow. A long-page regression and responsive width sweep protect
   the result.
2. Japanese, Chinese, and Korean specimens previously shared one
   Japanese-derived proof subset. The living page now bundles pinned official
   Noto Sans CJK JP, SC, and KR subsets, selects them by authored language, and
   verifies resolved font identity and non-`.notdef` output.
3. ADR-0005 still described migration and proof updates as pending after the
   implementation existed. Its migration, proof impact, and Design-0014
   outcome now describe the landed boundary and name the aggregate evidence.
4. Historical proof prose repeated the retired catch-all preparation
   metaphor. Current code, normative architecture, proof prose, examples,
   benchmarks, and diagnostics now use the actual stage or artifact name.
   Append-only Beads history remains historical audit evidence.

Good catch: direct visual review exposed both the missing float gutter and the
unbounded column before the product proof was allowed to stand.

## Should fix

No unresolved Should finding blocks this campaign. The following work is
important but belongs to a separately fenced claim:

- `und-oh0.13.17` owns the measured O(document) retained lifecycle and its
  O(change) correction.
- `und-oh0.13.15` owns the bounded CSS Text semantic and differential profile.
- `und-oh0.5.2` owns Arabic kashida/tatweel justification; CJK adjustment also
  remains script-specific future work.
- `und-oh0.9.3` owns mixed-bidi PDF extraction and viewer conformance.
- `und-oh0.2.11` owns dictionary-quality CJK word-navigation evidence blocked
  by the currently merged boundary fact.
- `und-oh0.15` owns presentation-scale quantization and pixel-grid coherence;
  high-level Parley's vertical quantization is not inherited through
  `parley_engine`.

## Could improve

- Add vertical writing, discretionary hyphenation, inline objects, pagination,
  and scroll/virtualization policy only behind their own product consumers.
- Add native accessibility projection without moving host callback policy into
  Underwood.
- Prototype the `und-oh0.15` presentation-scale quantization contract so
  painted text, carets, selections, hit geometry, and semantic geometry move
  together. Canonical layout must not be silently rounded.
- Upstream the independently useful line-formation pieces once their Parley
  Engine home is agreed.

## Execution graph

```text
authored snapshot + styles + region request
                    |
                    v
 source-complete ProjectedText
                    |
                    v
 Parley Engine analysis / font selection / shaping
                    |
                    v
 reusable candidate former <---- restore / retry
                    |                    ^
                    v                    |
         caller-owned RegionFlow -------+
                    |
                    v
 alignment / Western-space adjustment
                    |
                    v
 identity-bound TextScene + trace
             +------+------+
             |             |
             v             v
    imaging / PDF      hit / edit / IME
```

The reusable tools never receive an Underwood document, widget, renderer, or
high-level Parley `Layout`. Underwood orchestration binds their immutable facts
to revision, semantic, paint, interaction, and placement identity.

## Real, mirage, and not yet proven

| Claim | Judgment | Evidence or gap |
|---|---|---|
| Whitespace collapse remains editable and source-complete | Real | Projection corpus plus public-scene selection and deletion traps |
| Formation can restore and retry across exact slots | Real | Candidate corpus, height rejection, replayable region transcripts |
| Floats, exclusions, and columns are one region protocol | Real | Region wind tunnel and living page |
| Alignment moves all spatial observations together | Real | Geometry, caret, hit, selection, semantics, visual, and PDF comparisons |
| Identical labels can share eligible preparation | Real | Opt-in, byte-budgeted identity-free cache and churn wind tunnel |
| The path is retained and therefore O(change) | Mirage | 1,000-paragraph repeat still performs 666,167 allocations and requests 163,571,752 bytes |
| CJK line breaking equals full locale typography | Not yet proven | Named break corpus exists; locale tailoring and CJK adjustment do not |
| Western justification generalizes to Arabic or CJK | Mirage | Only U+0020 expansion is implemented and claimed |
| Portable PDF geometry guarantees viewer extraction | Not yet proven | Run/glyph identity is preserved; mixed-bidi viewer corpus remains open |
| Layout is pixel-snapped | Not yet implemented | Canonical coordinates and renderer inputs remain fractional |

The most dangerous remaining gap is the retained-lifecycle mirage. Expensive
analysis, shaping, and formation reuse is real, but unchanged values are still
rebuilt and deep-compared and scene records are deeply republished. A caller
could mistake the calm retained API for document-scale O(change) behavior.
The measured baseline and correction laws therefore live in the P0 sibling
campaign `und-oh0.13.17`.

## Scenario trace

1. A `TextBlock` or document snapshot authors semantic leaves and computed
   styles without constructing shaping or line-layout internals.
2. Projection presents preserved or collapsed text while retaining complete
   authored provenance.
3. The shared Fontique catalog and Parley Engine produce shaped facts; exact
   duplicates may reuse identity-free preparation within an explicit budget.
4. The candidate former proposes a line. Region policy offers one exact slot;
   height or line-final width failure restores the checkpoint and retries.
5. Alignment or Western justification emits adjustment side data. Geometry,
   interaction, semantics, and PDF consume the same adjusted coordinates.
6. Edits and IME composition return revision-correct selections and invalidate
   only the text-preparation stages whose inputs changed, while the trace says
   what work was executed or reused.

## Failure modes and one-line gaps

- **Projection:** complete mappings are real; the full CSS whitespace and
  transform matrix is not.
- **Shaping:** Parley Engine remains the only shaper; high-level Parley layout
  behavior is not inherited automatically.
- **Flow:** continuation prevents fatal exhaustion; scrolling and pagination
  are host/product work.
- **Adjustment:** Western spaces expand; Arabic and CJK use distinct,
  unimplemented strategies.
- **Interaction:** grapheme and logical word facts are retained; dictionary
  word quality is limited by upstream boundary representation.
- **Caching:** cross-identity sharing is bounded; document publication is not
  yet O(change).
- **Rendering:** output is renderer-neutral; device-scale quantization and
  font-raster hinting policy are absent.
- **PDF:** geometry and glyph identity are real; selection/extraction behavior
  across viewers remains a conformance project.

## Suggested tests

- Make the 1,000-paragraph exact-repeat and one-byte-edit workloads acceptance
  tests for `und-oh0.13.17`, including O(1) repeat publication and sublinear
  scene-spine update laws.
- Add a device-scale differential corpus before defining pixel snapping:
  fractional origins, mixed font sizes, bidi carets, selections, hit tests,
  underlines, and scale changes must share one quantization view.
- Run the mixed-bidi PDF corpus through Preview/PDFKit, Acrobat, Chromium, and
  Poppler before promoting viewer conformance.
- Expand Chromium-recorded CSS Text differentials under Design-0015 without
  treating browser compatibility output as the Unicode oracle.
- Give Arabic kashida and CJK inter-character adjustment independent
  typography corpora and proof states.

## Validation status

The final local matrix passes formatting, denied-warning all-target/all-feature
Clippy, workspace tests, rustdoc with warnings denied, Rust 1.88, bare-metal
`no_std`, WebAssembly, Taplo, typos, repository policy, dependency-universe
inspection, Beads lint and cycle detection, deterministic showcase snapshot,
and release PDF generation.

Protected remote checks remain the landing gate. Their pull request and run
identifiers are recorded here before the epic is closed.
