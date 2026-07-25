# Design 0012: Retained text blocks and intrinsic layout

**Status:** Accepted for implementation on 2026-07-24

**Bead:** `und-oh0.5.3`

## Decision

Underwood will add a retained `TextBlock` façade for low-ceremony single-
paragraph text. A block uses the same document identities, immutable snapshots,
paragraph projection, `ParagraphFormation` implementation, retained caches,
source mapping, style partitions, and `TextScene` as a document.

Intrinsic sizing and cache lifetime are shared engine capabilities. They are
not label-only fast paths.

## Fence

`TextBlock` owns retained single-paragraph content and hides document editing
ceremony; it explicitly does not own widget vocabulary, host behavior, font
policy, a second source model, a second shaping path, or a label-specific
cache.

`LayoutEngine` owns coordinated geometry-cache lifetime and propagates release
and eviction into its paragraph backend; it explicitly does not prescribe a UI
widget lifetime or silently retain entries beyond its configured budget.

## Required invariants

1. A block and a document with equivalent content, styles, paint, and
   constraint execute the same paragraph adapter and produce equivalent line,
   glyph, source, and metric facts.
2. Common computed styles can be constructed once and borrowed by arbitrarily
   many block requests. Cloning a retained style never copies family strings,
   feature arrays, or variation arrays.
3. `MinContent`, `MaxContent`, and constrained wrapping are explicit formation
   modes. Intrinsic modes never use infinity or a very large finite width.
4. Max-content honors mandatory breaks and no soft breaks. Min-content commits
   every legal soft-break opportunity and executes break-sensitive reshaping.
5. Text metrics use actual line advances, not line hit-area padding or the
   requested width.
6. An empty block has size `(0, resolved line height)` and no first or last
   text baseline. Hosts may apply their documented non-text baseline fallback.
7. Geometry and backend preparation caches are looked up by indexed stable
   paragraph identity.
8. Explicit release and budget eviction remove all retained layers for an
   identity when no retained geometry still depends on it.
9. Cache diagnostics distinguish hits, misses, explicit releases, budget
   evictions, current entries, peak entries, and backend entries.
10. Foundational crates remain `no_std + alloc`; this work adds no production
    dependency and no `unsafe`.

## Rejected alternatives

### Separate label preparation

A separate simple-text engine would duplicate font selection, fallback,
shaping, and invalidation behavior. Equivalent text could resolve differently
depending on the presenting control.

### A new generic block provenance model

Generalizing `TextScene` source identity before measuring the cost of a
one-paragraph document would expand every renderer and interaction API. The
first block façade instead retains the existing exact source model.

### Hiding a synthetic infinite width

Infinity is invalid in the current contract and a large finite sentinel can
still wrap, contaminate cache identity, overflow arithmetic, and misrepresent
max-content. Intrinsic modes are explicit backend inputs.

### A global style registry

Process- or engine-global style identifiers introduce another lifetime and
eviction system. Immutable styles already share their owned variable-sized
data. Block requests borrow caller-owned styles, and the wind tunnel will
measure the remaining small-value/table cost before another cache is added.

## Public call sites

### Plain retained block

```rust
let style = ComputedInlineStyle::new(shaping, flow, PaintSlot::new(0));
let paint = PaintTable::from_brushes([brush]);
let mut block = TextBlock::plain(DocumentId::from_bytes(*b"save-label-00001"), "Save")?;

let output = layout.prepare_block(
    &block.snapshot(),
    &BlockRequest::new(TextConstraint::MaxContent, &style, &paint),
)?;

assert_eq!(output.scene().metrics().size().width, 37.0);

block.set_text("Open")?;
```

Several blocks may borrow the same `style` and `paint`. `TextBlock::plain`
constructs its paragraph and source leaf once; `set_text` publishes one atomic
revision without exposing an edit transaction.

### Document preparation

```rust
let request = SceneRequest::new(
    TextConstraint::Wrap(FiniteWidth::new(420.0)?),
    &styles,
    &paint,
);
let output = layout.prepare(&snapshot, &request)?;
```

Documents and blocks share `TextConstraint` and `TextMetrics`.

### Engine construction and cache lifetime

```rust
let paragraphs = ParleyParagraphEngine::new(fonts);
let mut layout = LayoutEngine::new(paragraphs, CacheBudget::new(4_096));

// Called when a document or retained block is permanently discarded.
layout.release_document(block.id());

let cache = layout.cache_diagnostics();
assert!(cache.current_entries() <= cache.budget());
```

A zero-entry budget is valid and means that results are materialized into the
owned output but not retained after preparation.

## Public surface

The intended first surface is:

```rust
pub struct TextBlock { /* one retained document paragraph */ }
pub struct TextBlockSnapshot { /* cheap exact revision */ }

impl TextBlock {
    pub fn plain(id: DocumentId, text: &str) -> Result<Self, EditError>;
    pub fn snapshot(&self) -> TextBlockSnapshot;
    pub fn set_text(&mut self, text: &str) -> Result<(), EditError>;
    pub fn id(&self) -> DocumentId;
}

pub enum TextConstraint {
    MinContent,
    MaxContent,
    Wrap(FiniteWidth),
}

pub struct BlockRequest<'a> { /* borrowed constraint/style/paint */ }
pub struct TextMetrics { /* exact size and optional baselines */ }
pub struct CacheBudget { /* maximum retained geometry entries */ }
pub struct CacheDiagnostics { /* cumulative work and resident state */ }

impl LayoutEngine {
    pub fn new(
        paragraphs: impl ParagraphFormation + 'static,
        budget: CacheBudget,
    ) -> Self;

    pub fn prepare_block(
        &mut self,
        snapshot: &TextBlockSnapshot,
        request: &BlockRequest<'_>,
    ) -> Result<SceneOutput, SceneError>;

    pub fn release_document(&mut self, document: DocumentId);
    pub fn clear_cache(&mut self);
    pub fn cache_diagnostics(&self) -> CacheDiagnostics;
}
```

Exact accessor names may be tightened during implementation, but ownership,
constraint semantics, cache coordination, and output equivalence are fixed.

## Formation mechanics

`ParagraphConstraints` carries the explicit constraint mode into
`underwood_parley`.

- `Wrap(width)` preserves the existing greedy legal-break behavior.
- `MaxContent` walks to a mandatory break or paragraph end without considering
  soft opportunities.
- `MinContent` selects the first legal opportunity after the line start. An
  unbreakable segment may exceed every nominal width and remains one line.
- Every regular break passes through the existing bounded
  `apply_break`/`apply_concat` repair loop. Intrinsic measurement therefore
  observes the same joining and ligature behavior as constrained layout.

The constraint mode participates in the formation cache key. Changing only the
constraint may reform lines but never repeats Unicode analysis, itemization,
font selection, or initial shaping.

## Metrics

`TextScene::metrics()` reports:

- `Size`: maximum actual line advance and total block-axis extent;
- first baseline: the first prepared text line's scene-space baseline;
- last baseline: the last prepared text line's scene-space baseline.

Line bounds remain usable hit/presentation geometry. Metrics do not infer
advance from the current one-unit minimum line rectangle. Paint changes do not
change metrics.

## Cache lifecycle

Committed and composition geometry entries use indexed paragraph identity and
carry a monotonically increasing recency value. A coordinated eviction index
selects the least recently used retained entry without linear cache lookup.

After an output owns its materialized scene:

1. entries beyond `CacheBudget` are removed;
2. when no committed or composition geometry remains for a paragraph,
   `ParagraphFormation::release` is called;
3. `underwood_parley` removes the matching paragraph preparation entry;
4. diagnostics record budget eviction separately from explicit release.

`release_document` removes every paragraph belonging to that document.
`clear_cache` removes all geometry and backend preparation. Custom stateless
paragraph implementations may use the default no-op release hook; retained
implementations report their resident entry count.

## Wind-tunnel proof

The separate `benches/labels` crate uses only public APIs and bundled audited
fonts. Its deterministic assertions precede timing:

1. **Stable unique:** prepare thousands of distinct blocks twice; the second
   pass performs no analysis, selection, shaping, flow, or geometry.
2. **Repeated identical:** distinct identities sharing one text and style
   expose identity-local shaping cost honestly.
3. **Localized edit:** changing one block reshapes exactly one paragraph.
4. **Constraint churn:** alternate max-content and constrained width; only
   flow/geometry repeat.
5. **Create/destroy churn:** explicitly release discarded blocks and exceed a
   small budget; current geometry and backend entries remain bounded.
6. **Shared style:** all blocks borrow one computed style and paint table; no
   workload reconstructs family, feature, or variation inputs.

Wall-time reports name the profile, machine-local nature, iteration count, and
per-operation duration. They do not substitute for work and cache assertions.

## Placeholder cleanup and migration

This campaign deliberately breaks the pre-release API:

| Before | After |
| --- | --- |
| `TextData::compiled_minimal()` | removed |
| `ParleyParagraphEngine::new(data, fonts)?` | `ParleyParagraphEngine::new(fonts)` |
| `LayoutEngine::new(paragraphs)` | `LayoutEngine::new(paragraphs, budget)` |
| `SceneRequest::new(width, styles, paint)` | `SceneRequest::new(TextConstraint::Wrap(width), styles, paint)` |
| `ParagraphConstraints::max_inline_advance()` | `ParagraphConstraints::text()` and match `TextConstraint` |

Retained `ParagraphFormation` implementations should also override the new
`release`, `clear`, and `retained_entries` lifecycle methods. Stateless
implementations remain source-compatible through the default no-op hooks.

Future text-data provisioning must implement ADR-0003's real immutable resource
identity. It will not revive the empty placeholder for compatibility.

## Completion

The slice is complete when the public façade and migration are documented, the
intrinsic and cache laws have deterministic multilingual tests, the label wind
tunnel publishes its work/cache/timing evidence, all existing callers migrate,
and formatting, denied-warning Clippy, tests, rustdoc, MSRV, no_std, policy,
Beads, and protected remote CI are green.
