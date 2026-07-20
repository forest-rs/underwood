# Underwood

## A handover for a five-year text, document, editing, and composition platform

- **Status:** architectural north star and implementation handover
- **Working repository:** `forest-rs/underwood`
- **Working umbrella crate:** `underwood`
- **License:** Apache-2.0 OR MIT
- **Default platform posture:** `no_std + alloc`; optional `std`; desktop, mobile,
  WebAssembly, headless, and server-side consumers
- **Primary upstream text engine:** Parley and `parley_core`
- **Primary UI consumer:** Overstory
- **Primary rendering integration:** Imaging
- **Spearhead product:** the living agent document
- **Reference collaboration implementation:** Loro through `underwood_loro`
- **Proof discipline:** Specified → Executable → Measured → Conformant →
  Product-proven

---

## 1. Executive declaration

Underwood is not a rich-text widget, a styled-string container, a thin Parley adapter, or a convenient editor control.

Underwood is a renderer- and toolkit-independent document composition and editing platform for Rust. It turns semantic, revisioned documents into incrementally prepared text scenes. It uses Parley for Unicode analysis and shaping, but owns the larger machine: document structure, stable positions, layered annotations, style computation, source projection, incremental flow, editing transactions, inline objects, semantic mapping, and prepared presentation.

The ambition is deliberately large:

> The same coherent architecture must be capable of powering a button label, a form field, a million-line code editor, a collaborative design specification, a multilingual book, a living agent document containing recursive UI and nested sessions, and deterministic CPU/GPU/PDF output.

These are not separate engines sharing a few helper crates. They are policies and workloads over one platform.

Underwood should become the system that other Rust UI frameworks wish they had, the document engine that product teams cannot justify rebuilding, and the foundation upon which Overstory can offer text experiences that would otherwise require an entire company.

Most individual ideas in this design are proven rather than novel. Serious systems converge on projection, transactions, retained shaping, layered annotations, position semantics, and separated paint. Underwood’s novelty is the disciplined union: editor latency, document composition, semantic accessibility, recursive live UI, deterministic scenes, and capability-scoped operations in one embeddable Rust machine. The project must be intellectually honest about that. Reinventing known boundaries is not differentiation; making the union coherent and product-proven is.

An engine does not win because its architecture is admirable. It wins because a product people want makes the engine unavoidable. Underwood’s spearhead is therefore the **living agent document**: a streaming transcript that becomes a durable, editable, provenance-rich work artifact containing tool results, forms, code, tables, comments, and nested live UI. This is not a reduced destination. It is the workload that applies the earliest and most concentrated pressure to the five-year architecture.

This document does not propose an MVP that quietly freezes the wrong abstractions. It describes the whole machine. Implementation can proceed through coherent workstreams, but every public type, cache boundary, and ownership decision must track the complete destination from the beginning.

The core declaration is:

> **Parley shapes the text. Underwood makes it a document. Overstory makes it an experience.**

---

## 2. Why Underwood exists

Text systems tend to accrete in layers:

1. A UI toolkit needs labels and calls a shaper.
2. A text field adds a mutable string, selection, and IME state.
3. A text area adds wrapping and scrolling.
4. Rich text adds style spans.
5. Links add hit testing and semantics.
6. Inline objects add an escape hatch.
7. Large documents expose invalidation and memory failures.
8. Accessibility reveals that the display tree is not a semantic document.
9. Collaboration reveals that byte offsets are not durable positions.
10. Pagination, arbitrary regions, vertical text, or advanced typography reveal that “maximum width” was never a layout model.

At that point, the toolkit has several incompatible text stacks and no tractable route to a real document system.

Underwood begins at the other end. It assumes from the start that:

- text is semantic before it is visual;
- positions must survive edits;
- attributes exist in independent layers;
- shaping is only one stage of preparation;
- flow is more general than a rectangle;
- paint changes must not trigger shaping;
- width changes must not trigger Unicode analysis;
- accessibility is sourced from meaning, not reconstructed from glyphs;
- editors are transactional systems, not mutable strings with key handlers;
- prepared output must be reusable by GPU, CPU, PDF, tests, and remote consumers;
- simple labels must remain simple and fast despite the larger architecture.

This is not complexity for its own sake. It is the minimum set of separations needed to avoid repeatedly paying for accidental coupling.

---

## 3. Values and architectural tenets

### 3.1 Calm surfaces, powerful internals

The public API should feel smaller than the implementation.

An application should be able to write:

```rust
let document = Document::builder()
    .section(|section| {
        section.heading(1, |text| text.push("Underwood"));
        section.paragraph(|text| {
            text.push("One document model, ");
            text.span(Strong, |text| text.push("many experiences"));
            text.push(".");
        });
    })
    .finish();

let session = DocumentSession::new(document);
```

It should not be asked to understand itemization, bidi levels, paragraph projections, shaping workspaces, style-set provenance, glyph caches, or revision graphs.

Internally, Underwood may be sophisticated. Externally, it should present a small number of stable concepts with strong defaults.

### 3.2 Typed systems, not universal value bags

Do not build the document, style, command, or extension system around a universal `Value` enum.

Use:

- typed schema keys;
- typed payload access;
- typed style declarations;
- typed commands;
- a small closed algebra only where the domain is genuinely closed, such as primitive tree or text edits;
- erasure at narrow storage or dispatch boundaries, never as the authoring model.

Extensibility must not make ordinary code dynamically typed.

### 3.3 Mechanism and policy are separate

Underwood owns document and editing mechanisms. It does not own platform keyboard conventions, widget focus policy, theme selection, or clipboard APIs.

Parley owns text mechanics. It does not own document semantics, collaboration, or UI controls.

Overstory owns interaction policy. It does not own shaping, range maintenance, or document history.

Rendering backends own pixels. They do not own layout or semantics.

### 3.4 Retained, incremental, and observable

Every expensive stage produces a retained result with explicit identity and dependencies. There must be no hidden “rebuild it all” path masquerading as simplicity.

The engine must be able to explain:

- what changed;
- which stages were invalidated;
- why a paragraph reshaped;
- which cached value was reused;
- how much memory a document and its prepared resources consume;
- how long an edit took to become visible.

Spoor and Sightline should eventually make this pipeline inspectable in real time.

### 3.5 Renderer-independent by construction

Widgets do not paint text directly. Document layout does not emit WGPU commands. The prepared result is a renderer-neutral `TextScene`.

The same scene should be lowerable to:

- Imaging GPU paths;
- Imaging CPU paths;
- PDF or print output;
- deterministic test snapshots;
- serialized or remote presentation;
- accessibility geometry.

### 3.6 International text is foundational, not a compatibility pass

Arabic shaping, bidi, Indic scripts, CJK line breaking, emoji, language-specific segmentation, vertical text, ruby, fallback, and mixed-script content are not edge cases. Architectural decisions must be tested against them from the start.

### 3.7 Accessibility follows the semantic source

Glyph runs are not headings. Blue underlined paint is not a link. A visual span is not an accessibility node.

The document supplies meaning. Layout supplies geometry. Overstory supplies the platform accessibility bridge.

### 3.8 Agent-accessible, never agent-dependent

Underwood should be exceptionally legible to tools and agents:

- semantic document inspection;
- typed commands;
- transactional changes;
- provenance;
- stable locations;
- deterministic replay;
- rich diagnostics.

The platform must remain fully coherent and usable without an LLM.

### 3.9 Design for decades

Overstory and the surrounding platform are intended to outlive individual UI trends, renderers, and host integrations. Underwood must avoid baking in the current shape of Parley, AccessKit, WGPU, a particular editor behavior, or today’s fashionable document model.

### 3.10 Simplicity is the result of good boundaries

Underwood must not become a generalized database, DOM clone, ECS, render graph, task runtime, or arbitrary plugin host.

Its internal machine is built around three ideas:

1. **Immutable snapshots** describe semantic state.
2. **Deliberate position forms** give each range exactly the persistence it needs.
3. **Revisioned prepared resources** cache transformations of that state.

Everything else is a transformation, index, policy adapter, or view over those forms.

Stable anchors are one position form, not the universal currency. Sparse human-meaningful positions deserve durable identity; dense authored ranges deserve bulk tracking; dense derived ranges normally belong to one snapshot. This distinction is part of the foundation, not an optimization to discover later.

---

## 4. North stars

### 4.1 One platform, many experiences

The following are all clients of the same document, layout, and scene systems:

- `Text`: immutable, single-style label policy;
- `TextField`: single-line plain-document editing policy;
- `TextArea`: multiline plain-document editing policy;
- `TextView`: read-only structured document policy;
- `TextEditor`: editable structured document policy;
- code editor: specialized schema, commands, annotations, and virtualization policy;
- transcript viewer: streaming document and retention policy;
- living agent document: streaming operations, provenance, collaboration, and recursive tool/UI surfaces;
- page compositor: region chain and pagination policy;
- collaborative specification editor: operation transport and remote-presence policy.

They may have distinct calm public controls. They must not have distinct text engines.

### 4.2 Paint is not layout

Changing every foreground brush in a hundred-page document must perform zero Unicode analysis, zero shaping, and zero line breaking.

### 4.3 Width is not content

Resizing a window may change line breaks and downstream flow. It must not re-run text analysis or shaping except for bounded reshaping required by newly committed breaks.

### 4.4 A local edit is local

Editing one word in paragraph 4,000 must not reshape paragraph 3,999 or 4,001. It may reflow later pages if pagination changes, but those later paragraphs must retain analysis and shaping.

### 4.5 Meaning survives presentation

A link, heading, table cell, diagnostic, comment, or inline product reference retains its semantic identity through projection, shaping, line breaking, bidi reordering, clipping, virtualization, and rendering.

### 4.6 Text flows through the world

Text can flow through columns, around arbitrary `kurbo` shapes, past floats, between linked regions, across pages, and alongside inline live UI without requiring a parallel layout engine.

### 4.7 The scene is portable

One prepared document can render consistently through GPU, CPU, PDF, and headless test paths.

### 4.8 A document is an operation space

Human editing, undo, collaboration, macros, and agent changes all produce the same typed transactions. No privileged mutation path bypasses history, invariants, or invalidation.

### 4.9 Documents can contain experience recursively

A document may contain a live widget that contains another document session: a tool-result card with editable structured output, a comment thread in a float, a table cell with its own editor, or a form whose help and validation are documents. Focus, selection, accessibility, layout, and undo cross those boundaries only through explicit contracts. Self-embedding cycles and unbounded layout recursion are rejected.

This is the inversion Underwood and Overstory should own: applications no longer merely contain documents; documents can become the application shell, with UI as semantic content.

### 4.10 One spearhead tyrannizes product pressure

The living agent document is the flagship conformance client and the theory of adoption. It must force streaming append economics, virtualization, structured tool output, provenance, accessibility, collaboration, nested UI, semantic diff, and scene-delta delivery to become real together.

This is not an MVP or permission to deform the architecture around chat bubbles. The compositor and technical editor remain standing wind tunnels. The spearhead decides which complete capabilities receive product pressure first, so one usable thing reaches depth instead of every subsystem remaining impressive at twenty percent.

### 4.11 Small content stays inexpensive

The presence of this architecture must not make a button label expensive. Small immutable documents should collapse into compact representations and highly shared prepared resources.

---

## 5. Ownership and repository boundaries

### 5.1 The dependency direction

```text
Overstory ───────► Underwood ───────► Parley / parley_core
    │                 │
    │                 ├─────────────► Fontique
    │                 ├─────────────► Kurbo / Peniko
    │                 ├─────────────► AttributedText / StyledText
    │                 └─ optional ──► Loro reference collaboration adapter
    │
    ├────────────► Understory general UI primitives
    └────────────► Imaging / presenters

Underwood scene adapters ───────────► Imaging / parley_draw
```

Dependencies may be factored into adapter crates to avoid cycles. The conceptual ownership must remain clear even if temporary implementation details differ.

### 5.2 Parley owns text physics

The Parley repository should own universally applicable text machinery:

- generic text-access helpers and snapshot-local validated ranges used by Parley algorithms;
- `attributed_text`;
- `styled_text`;
- leaf typography types in `parlance` or related crates;
- Unicode analysis and bidi;
- itemization;
- shaping and fallback;
- retained `ShapedText`;
- reshaping around committed breaks;
- cluster, caret, hit-test, and visual-order paragraph primitives;
- generic inline-object boundaries and metrics;
- paragraph line-breaking mechanisms.

Parley should know that a shaping property changes at a position. It should not know that the span is a hyperlink or that its foreground came from an Overstory theme token.

### 5.3 Underwood owns the document platform

Underwood owns:

- semantic document snapshots;
- the concrete canonical text store and its three position forms;
- structured nodes and extension schemas;
- annotation layers;
- projections and source maps;
- document style sheets and computed styles;
- preparation caches and dependency keys;
- block, region, and page flow;
- document-level selections and transactions;
- undo and redo;
- inline-object protocol and placement;
- semantic-to-visual mapping;
- renderer-neutral `TextScene` output;
- collaboration operation semantics and the maintained Loro reference adapter;
- interchange document fragments.

### 5.4 Understory remains general UI infrastructure

Understory should not hide a document engine inside a growing family of incidental text crates.

It continues to own general-purpose facilities such as:

- property mechanisms;
- invalidation;
- shapes and geometry abstractions;
- selection mechanisms useful outside documents;
- presentation stores and generic prepared-resource references;
- animation, timing, and interaction primitives.

`understory_presentation` may refer to a generic prepared text resource key. It should not own Underwood documents or Parley layouts.

### 5.5 Overstory owns experience and host policy

Overstory owns:

- controls and templates;
- dependency-property and theme integration;
- focus and routed events;
- pointer gestures;
- platform keyboard maps;
- caret blinking and reveal policy;
- scrolling realization;
- clipboard and drag/drop hosts;
- IME host integration;
- context menus and link activation;
- widget-backed inline objects;
- recursive focus, selection, and accessibility stitching for nested document surfaces;
- AccessKit tree construction;
- scheduling and presentation-resource lifetime;
- development tools and live inspection.

### 5.6 Imaging and drawing integrations own pixels

Actual glyph rasterization, hinting, atlas management, CPU/GPU drawing, and PDF command emission do not belong in the semantic document or flow engines.

Underwood provides stable scene data and paint slots. Imaging, `parley_draw`, and presenter adapters turn those resources into output.

### 5.7 Seven prohibitions

1. No semantic document tree in Parley.
2. No Overstory element per text span.
3. No platform keybinding or host policy in Underwood.
4. No actual paint values embedded in shaping cache identity.
5. No open storage trait or per-span anchor resolution in a paragraph hot loop.
6. No collaboration abstraction declared complete without a maintained merge implementation and trace corpus.
7. No reentrant parent/child document layout or implicit cross-session undo.

---

## 6. Repository and crate topology

The repository is a workspace. Crate boundaries should correspond to genuine dependency or policy fences, not aesthetic decomposition.

```text
underwood/
├── underwood/                 # Calm public façade
├── underwood_core/            # IDs, revisions, anchors, common contracts
├── underwood_document/        # Snapshots, structure, layers, projections
├── underwood_data/            # Unicode/locale/hyphenation data capabilities
├── underwood_style/           # Specified/computed styles and style sheets
├── underwood_edit/            # Sessions, transactions, selections, history
├── underwood_layout/          # Analysis, shaping, breaking, regions, pages
├── underwood_scene/           # Renderer-neutral prepared text scenes
├── underwood_parley/          # Explicit Parley adaptation seam
├── underwood_loro/            # Blessed collaboration/conformance adapter
├── underwood_imaging/         # Optional Imaging lowering
├── underwood_pdf/             # Optional tagged PDF/PDF-UA lowering
├── underwood_interchange/     # Optional codecs and clipboard formats
├── underwood_test/            # Corpora, fixtures, differential harnesses
├── examples/
├── benches/
└── docs/
```

It may be wise to begin with fewer published crates while retaining these module boundaries. The architecture should not require a dozen public packages merely to display text.

### 6.1 Crate posture

All foundational crates:

- `#![no_std]`;
- `extern crate alloc`;
- optional `std` feature;
- no ambient filesystem, locale, font, clock, thread-pool, or platform assumptions;
- crate names use underscores;
- dependencies use `default-features = false` where practical;
- `kurbo` types are used for geometry;
- `hashbrown` is preferred when a hash map is genuinely needed;
- concrete public APIs are preferred over generic soup.

Host capabilities are supplied explicitly.

### 6.2 Repository governance and design discipline

Underwood’s breadth makes written architectural discipline essential.

- Important decisions are recorded in design documents or ADRs before they become folklore.
- Decisions that affect absent contributors are made asynchronously in writing.
- Review expectations distinguish correctness, architectural fit, API design, performance evidence, and editorial polish.
- Crate fences and dependency directions are checked in CI where practical.
- Every major subsystem names an owner, invariants, diagnostics, and a conformance corpus.
- Examples demonstrate real capabilities and compose the public API; they are not decorative screenshots hiding private shortcuts.
- Documentation explains why boundaries exist, not only how to call methods.
- Performance claims include workloads, measurement methodology, and retained artifacts.
- LLM assistance is welcome, but generated volume is never evidence of architectural completion. Depth, tests, interoperability, and measured behavior are the evidence.

The project should publish a short constitutional document derived from this handover. Changes to its core tenets require explicit discussion. This is not bureaucracy: it prevents a five-year platform from becoming the sum of locally convenient pull requests.

API stability should be honest. Before 1.0, make necessary corrections decisively and document migrations. After stability commitments, preserve calm façades while allowing internal representations and preparation strategies to evolve.

### 6.3 The surface-to-hands ratio is the primary existential risk

The architecture is achievable. The conformance surface can still kill the project. Correct bidi editing, platform IME behavior, CJK segmentation, table fragmentation, assistive-technology interoperability, font fallback, PDF embedding, and collaborative undo are years of corpus work and host-specific grind. Most person-hours must be expected to land there rather than in elegant core types.

A polished handover is not implementation progress. Every named capability carries a proof status:

```text
Specified → Executable → Measured → Conformant → Product-proven
```

- **Specified** means the contract and invariants exist in writing.
- **Executable** means a real public path runs without a private shortcut.
- **Measured** means budgets and retained-work evidence exist.
- **Conformant** means the relevant corpus, platforms, failure cases, and differential tests pass.
- **Product-proven** means the spearhead depends on it under sustained real use.

The repository maintains a proof ledger mapping every architectural promise to an owner, corpus, harness, current status, and blocking dependency. Roadmaps and announcements use these words literally. A screenshot cannot promote a subsystem from `Executable` to `Conformant`.

Forest-rs also has correlated ecosystem risk: Understory, Overstory, Imaging, Spoor, Sightline, storage, and Underwood can become interlocking keystones maintained by one person. Coherence is valuable; simultaneous incompleteness is not. Therefore:

- Underwood must remain headless-testable and useful without Overstory;
- Overstory is the flagship consumer, not the only possible consumer;
- adapters depend in one direction and pin known-compatible releases rather than requiring every repository at `main`;
- test fonts, synchronous schedulers, headless scenes, and fake object providers let Underwood reach conformance without waiting for the full stack;
- each cross-repository dependency has an owner, compatibility fixture, and degraded-mode story;
- a capability with no conformance owner is explicitly dormant rather than “in parallel.”

Bus factor is an architectural metric. Decision records, contributor maps, small reviewable crates, checked-in corpora, reproducible traces, and funding for maintenance are part of correctness because they determine whether difficult behavior remains correct after its original author moves on.

### 6.4 The theory of adoption is a product and an institution

Companies do not adopt engines because their separations are beautiful. HarfBuzz, Skia, and browser engines travel inside products people already need. The living agent document is Underwood’s vehicle: it should make the complete forest-rs stack desirable before Underwood is marketed as infrastructure.

The dual Apache-2.0/MIT license is a distribution policy, not a sustainability model. If the goal is broad influence and craft, downstream appropriation is success. If the goal is a durable team capable of company-grade conformance, the project needs a stewardship charter naming who funds maintenance and why.

This handover recommends a **funded commons anchored by the flagship product**. Underwood’s engine, formats, conformance corpora, and ordinary adapters remain Apache-2.0/MIT infrastructure. Recurring hosted collaboration/synchronization, certified export and conformance services, enterprise integration, and the living-agent product can fund a permanent text/host/accessibility team. Sponsorship or a later foundation can widen stewardship. The core does not become artificially crippled “open core,” and commercial users do not receive a private correctness tier unavailable to the commons.

Before major compatibility promises, the project records:

- confirm or explicitly overturn the recommended combination of broad influence, a sustainable independent institution, and a commercial flagship product;
- which parts are irrevocably common infrastructure and which services may fund them;
- how external contributors gain ownership rather than becoming drive-by patch suppliers;
- how security, Unicode, accessibility, and platform conformance maintenance is financed;
- which adoption artifacts exist: a wanted product, integration guides, stable crates, migration paths, benchmark corpora, and support expectations.

“Defang entire companies” should mean that independent teams can embed capabilities previously reserved for company-scale stacks. The proof is a shipped product and an adoptable engine, not the emotional reaction of incumbents.

---

## 7. The complete preparation pipeline

```text
DocumentSnapshot
    │
    ├─ semantic structure
    ├─ text storage snapshots
    └─ annotation-layer revisions
    ▼
ParagraphProjection
    │
    ├─ projected UTF-8
    ├─ source map
    ├─ inline-object markers
    └─ projected semantic ranges
    ▼
ComputedStyledText
    │
    ├─ shaping-style runs
    ├─ flow-style runs
    └─ paint-style runs
    ▼
Analysis
    │
    ├─ Unicode properties
    ├─ segmentation
    └─ bidi resolution
    ▼
Itemization
    │
    ├─ script and language runs
    ├─ shaping-style topology
    ├─ fallback candidates
    └─ inline-object boundaries
    ▼
ShapedText
    │
    ├─ glyphs and clusters
    ├─ font instances
    └─ logical/visual mappings
    ▼
UsedInlineValues
    │
    ├─ resolved spacing and baselines
    ├─ line-relative lengths
    └─ device-text configuration
    ▼
ParagraphBreakPlan
    │
    ├─ committed breaks
    ├─ bounded reshaping
    └─ line metrics
    ▼
UsedFlowValues
    │
    ├─ resolved block percentages
    ├─ region-relative offsets
    └─ table/fragmentation constraints
    ▼
FlowLayout
    │
    ├─ blocks
    ├─ regions and columns
    ├─ pages and exclusions
    └─ inline-object placements
    ▼
FlowCheckpoints + VirtualExtentMap
    │
    ├─ resumable region state
    ├─ estimated/measured extents
    └─ viewport correction deltas
    ▼
TextSceneGeometry + PaintTable + SemanticMap
    │
    ├─ GPU / CPU / PDF lowering
    ├─ hit testing and carets
    └─ accessibility geometry
```

Each arrow is an explicit transformation with:

- typed input;
- typed output;
- dependency key;
- invalidation summary;
- reusable workspace;
- instrumentation;
- deterministic behavior.

Analysis and itemization are separate because their invalidation is different: Unicode properties do not change when font weight changes, while fallback and shaping runs can. Used values are named because geometry-dependent resolution must not occur incidentally inside a breaker or renderer. Flow checkpoints are named because pagination and virtualization are retained computations, not a promise that an eager layout somehow becomes cheap.

No stage reaches backward into a widget tree or property store.

---

## 8. Core identity model

### 8.1 Immutable snapshots

Consumers read immutable snapshots. Mutable sessions produce new snapshots through transactions.

```rust
#[derive(Clone)]
pub struct DocumentSnapshot {
    id: DocumentId,
    revision: DocumentRevision,
    structure: StructureSnapshot,
    text: TextStoreSnapshot,
    layers: LayerSetSnapshot,
}

impl DocumentSnapshot {
    pub fn id(&self) -> DocumentId {
        self.id
    }

    pub fn revision(&self) -> DocumentRevision {
        self.revision
    }

    pub fn root(&self) -> NodeId {
        self.structure.root()
    }

    pub fn node(&self, id: NodeId) -> Option<NodeRef<'_>> {
        self.structure.node(id)
    }

    pub fn text(&self, id: TextNodeId) -> Option<TextSlice<'_>> {
        self.text.slice(id)
    }

    pub fn layer<L: AnnotationLayer>(&self) -> Option<LayerRef<'_, L>> {
        self.layers.typed::<L>()
    }
}
```

Snapshots make preparation safe to parallelize and make caches trustworthy. An engine never observes text changing underneath a range or `StyleId`.

### 8.2 Revisions are structured

A single monotonically increasing document revision is useful for ordering but too coarse for cache identity.

```rust
pub struct DocumentRevision(u64);

pub struct RevisionVector {
    pub structure: StructureRevision,
    pub text: TextRevision,
    pub authored_style: LayerRevision,
    pub semantics: LayerRevision,
    pub inline_objects: LayerRevision,
}
```

Paragraphs and layers should expose stable fingerprints or component revisions so unrelated edits do not poison cache reuse.

### 8.3 The position contract is constitutional

There is no universal range type. Underwood has three position forms because their economics and meanings are different.

#### Sparse durable anchors

Stable anchors are for sparse, persistent, human-meaningful positions: carets and selections, comments, bookmarks, collaboration presence, review threads, and application references that must survive edits.

```rust
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct StableAnchor {
    container: TextNodeId,
    token: AnchorToken,
    bias: AnchorBias,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AnchorBias {
    Before,
    After,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct StableRange {
    pub start: StableAnchor,
    pub end: StableAnchor,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DocumentPosition {
    pub anchor: StableAnchor,
    pub affinity: VisualAffinity,
}
```

Anchor creation, resolution, transformation, retention, and release are explicit operations with measured cost. An implementation may not allocate two permanent anchor records for every syntax token and diagnostic.

#### Dense authored tracked ranges

Authored styles and some semantic annotations can be dense and must persist, but millions of independent anchor tokens are the wrong representation. They live in a storage-integrated, bulk-updated range set.

```rust
pub struct TrackedRangeSet<V> {
    revision: LayerRevision,
    storage: Shared<TrackedRangeStorage<V>>,
}

pub struct TrackedRangeRef<'a, V> {
    pub range: core::ops::Range<u32>,
    pub edge_behavior: EdgeBehavior,
    pub value: &'a V,
}
```

The range set is queried against one text snapshot and transformed once per edit summary, using a run tree or another compact index. Its underlying representation is private. The contract is bulk iteration, logarithmic localized mutation, structural sharing, and no per-span anchor-resolution loop.

#### Dense derived snapshot ranges

Syntax, diagnostics, search results, spellcheck, and other computed layers normally use revision-bound offsets.

```rust
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct SnapshotRange {
    pub text_revision: TextRevision,
    pub container: TextNodeId,
    pub bytes: core::ops::Range<u32>,
}

pub enum DerivedRangeUpdate {
    MappedForward(Box<[SnapshotRange]>),
    Recompute(InvalidatedTextRegions),
}
```

A producer may cheaply map these ranges through an `EditSummary` when the mapping remains sound; otherwise it recomputes the affected window. A stale `SnapshotRange` cannot be resolved against another revision by accident. Revision mismatch is a type-checked or runtime-checked error, never silent offset reuse.

These are not three interchangeable implementations behind one trait. They are three semantic commitments visible in layer descriptors, source maps, diagnostics, and performance counters.

### 8.4 The default canonical text store is concrete

Underwood does not expose `Document<S: TextStorage>`. By default, its document façade uses a concrete persistent UTF-8 store optimized for paragraph iteration, localized replacement, snapshot sharing, bulk range transformation, and edit-summary production.

The store may internally specialize small contiguous text, chunked text, and very large append-heavy text. Those are sealed implementation strategies under one contract, not an open trait dispatched through projection, analysis, and shaping hot paths.

```rust
pub struct TextStoreSnapshot {
    revision: TextRevision,
    root: Shared<TextStoreRoot>,
}

pub struct EditSummary {
    pub before: TextRevision,
    pub after: TextRevision,
    pub replacements: Box<[TextReplacementSummary]>,
    pub affected_paragraphs: ParagraphInterval,
}
```

An external rope, piece table, or server model can remain authoritative through an adapter that publishes Underwood transactions and snapshots. It does not replace the inner-loop representation by default. If evidence demands another internal backend, it enters a sealed set and must pass the same corpus, cache, and cost tests.

Collaboration is the deliberate challenge to this stance. The Loro reference implementation must prototype both a mirrored mode—Loro operations publish into the canonical store—and an authoritative mode—an immutable Underwood preparation snapshot is projected directly from Loro identities and rich-text state. The CRDT-native ADR compares those modes before collaborative schemas stabilize. Whatever wins remains behind the concrete `Document` façade; `Document<S>` is not the prize.

This is deliberate asymmetry: interoperability exists at snapshot and transaction boundaries; the hottest internal paths remain concrete.

### 8.5 Revisioned resources

Prepared resources use stable identity plus revision.

```rust
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ResourceRef<K, R> {
    pub key: K,
    pub revision: R,
}
```

Presentation stores compare lightweight references. Resource registries own the larger immutable values.

---

## 9. Semantic document model

### 9.1 The model is structural but not a browser DOM

Underwood needs enough structure for editing, layout, accessibility, serialization, and extension. It does not need browser scripting semantics, CSS DOM mutation behavior, or a universal string-keyed attribute map.

Core structure includes:

- document root;
- sections;
- paragraphs;
- headings;
- block quotations;
- list containers and items;
- code blocks;
- tables, rows, and cells;
- thematic breaks;
- text leaves;
- inline semantic containers;
- hard and soft breaks;
- inline objects.

Applications must be able to define additional node schemas without weakening typed access.

### 9.2 Typed schemas

Avoid a giant permanently closed enum containing every possible payload.

```rust
pub trait NodeSchema: 'static {
    type Payload: Clone + core::fmt::Debug + Send + Sync + 'static;

    const ROLE: NodeRole;
    const CONTENT_MODEL: ContentModel;
}

pub struct Heading;

impl NodeSchema for Heading {
    type Payload = HeadingData;

    const ROLE: NodeRole = NodeRole::HEADING;
    const CONTENT_MODEL: ContentModel = ContentModel::Inline;
}

pub struct HeadingData {
    pub level: u8,
    pub outline_id: Option<OutlineId>,
}

let heading: NodeHandle<Heading> = document.node_typed(node_id)?;
let level = heading.payload().level;
```

Internally, the structure can store compact node records and typed side tables. Erasure is localized to the registry boundary.

### 9.3 Core node record sketch

```rust
struct NodeRecord {
    id: NodeId,
    role: NodeRole,
    parent: Option<NodeId>,
    first_child: Option<NodeId>,
    next_sibling: Option<NodeId>,
    payload: PayloadRef,
    content_revision: NodeRevision,
}
```

The tree should support compact traversal and structural sharing between snapshots. Do not commit prematurely to a general persistent-tree library if a purpose-built arena plus copy-on-write pages is simpler.

### 9.4 Text leaves and paragraphs

The unit of correct Unicode analysis and bidi resolution is effectively a paragraph. The semantic model may contain multiple inline text leaves, objects, and semantic spans within a paragraph. Projection flattens those into the string presented to Parley while retaining source mappings.

Paragraph identity must remain stable even when its internal text changes. Splitting or joining paragraphs is a structural transaction with explicit position mapping.

### 9.5 Document fragments

Copy, paste, drag/drop, undo payloads, and interchange need a portable fragment form:

```rust
pub struct DocumentFragment {
    pub structure: FragmentTree,
    pub text: FragmentTextStore,
    pub authored_styles: FragmentStyleLayer,
    pub semantics: FragmentSemanticLayer,
    pub attachments: FragmentAttachments,
}
```

A fragment can produce plain text, HTML, Markdown, RTF, or an application-specific representation without making any of those formats canonical.

---

## 10. Annotation layers

Rich documents contain overlapping concerns that must not be collapsed into one attribute stream.

### 10.1 Representative layers

```text
Authored style       persistent       affects shape / flow / paint
Semantics            persistent       affects accessibility / interaction
Links                persistent       affects interaction / semantics
Diagnostics          derived          affects overlay paint / commands
Syntax               derived          affects paint, occasionally shape
Search matches       transient        affects overlay paint
Spellcheck           derived          affects overlay paint / commands
IME composition      session-local    affects projection / overlay / caret
Local selection      session-local    affects overlay / accessibility
Remote presence      collaborative    affects overlay
Review comments      persistent       affects semantics / overlay
Provenance           persistent       affects tooling / policy
```

### 10.2 Layer contract

```rust
pub trait AnnotationLayer: 'static {
    type Value: Clone + core::fmt::Debug + 'static;
    type Range: LayerRange;

    const ROLE: AnnotationRole;
    const POSITIONING: PositioningClass;
    const EDIT_BEHAVIOR: EditBehavior;
    const INVALIDATION: InvalidationClass;
}

pub struct AnnotationSpan<R, V> {
    pub range: R,
    pub value: V,
}

pub enum PositioningClass {
    SparseDurable,
    DenseTracked,
    SnapshotLocal,
}
```

Each layer declares:

- how spans behave when text is inserted at an edge;
- whether adjacent equal spans coalesce;
- whether overlaps are allowed;
- whether ordering is significant;
- whether the layer participates in snapshots, sessions, or external derivation;
- which preparation stages it can invalidate.

The range type is part of the layer’s contract. Review comments may use `StableRange`; authored inline styles may use a `TrackedRangeSet`; syntax and diagnostics use `SnapshotRange`. Promoting a dense layer to stable anchors requires performance evidence and an ADR. Keeping a persistent human reference in snapshot-local offsets is a correctness bug.

### 10.3 Semantics are not paint

A link may receive a default style from a semantic style rule, but the `Link` annotation remains independent of that style. Removing the underline does not remove link behavior. Recoloring a link does not alter accessibility identity.

### 10.4 `attributed_text` integration

`attributed_text` is appropriate for compact snapshot-local span storage and segmentation. Underwood adds:

- an explicit position strategy per layer;
- layer identity;
- editing affinity;
- snapshot revision;
- projection into paragraph-local ranges;
- invalidation classification.

Do not force all layers into one generic attribute type merely because the underlying storage can represent it.

---

## 11. Transactions, operations, and history

### 11.1 All mutation is transactional

```rust
pub struct EditTransaction {
    pub id: TransactionId,
    pub base_revision: DocumentRevision,
    pub edits: Box<[PrimitiveEdit]>,
    pub selection_before: SelectionSet,
    pub selection_after: SelectionSet,
    pub metadata: TransactionMetadata,
}

pub struct TransactionMetadata {
    pub origin: EditOrigin,
    pub timestamp: Option<LogicalTimestamp>,
    pub undo_group: Option<UndoGroupId>,
    pub label: Option<SharedString>,
    pub provenance: Option<ProvenanceId>,
}
```

### 11.2 A small primitive edit algebra

A closed algebra is appropriate when it describes the irreducible mutations of Underwood’s own model.

```rust
pub enum PrimitiveEdit {
    ReplaceText(ReplaceText),
    SpliceChildren(SpliceChildren),
    SetNodePayload(SetNodePayload),
    ReplaceLayerSpans(ReplaceLayerSpans),
    AttachResource(AttachResource),
    DetachResource(DetachResource),
}
```

Higher-level commands compile into these primitives. The transaction records enough prior state or inverse information for reliable undo.

Extension payload changes remain typed at the authoring API and are erased only into a schema-identified storage operation.

### 11.3 Commands are policy-independent intentions

```rust
pub enum StandardEditCommand {
    InsertText(SharedString),
    DeleteSelection,
    DeleteWordBackward,
    MoveVisual(Direction),
    MoveWord(Direction),
    ExtendToLineBoundary(Direction),
    SplitParagraph,
    JoinWithPreviousParagraph,
    ToggleStrong,
    SetBlockRole(NodeRole),
}
```

This enumeration is illustrative, not a requirement that all application commands be built-ins. The edit engine exposes typed command traits and a standard vocabulary. Overstory maps platform gestures to commands.

### 11.4 Undo and redo

Undo is transaction inversion, not a side channel. Grouping policy is explicit and can account for:

- typing runs;
- IME commits;
- semantic formatting;
- structural changes;
- paste/drop;
- application commands;
- agent-originated edits.

In a single-writer session, inversion against the expected descendant revision may restore semantic equality. In a collaborative session, undo has different semantics: it is **selective undo of local intent**, expressed as a new compensating transaction against the current snapshot. It never rewinds shared history and never removes remote operations merely because they occurred after the local edit.

```rust
pub enum SelectiveUndoOutcome {
    Applied(EditTransaction),
    PartiallyApplied {
        transaction: EditTransaction,
        omissions: Box<[UndoOmission]>,
    },
    NoEffect(UndoNoEffect),
    Conflict(UndoConflict),
}
```

Undo records retain operation identities, affected semantic identities, inverse intent, and enough boundary context to map that intent through rebases. If remote work deleted or structurally replaced the target, the policy produces a deterministic partial result, no-op, or surfaced conflict; it does not resurrect or overwrite content silently. Redo likewise issues a new transaction that reasserts the local intent in the current document.

The exact mapping laws—insert versus concurrent insert, formatting over split ranges, node moves, schema changes, and local group ordering—require a pre-stability ADR and trace corpus. “The CRDT will handle it” is not a semantic definition.

### 11.5 Collaboration

Underwood does not hard-code one merge algorithm into every label, but it does ship a blessed collaboration implementation. Conditions without a working merge client do not validate position or undo semantics.

The maintained `underwood_loro` crate is the reference adapter and collaboration conformance client. Loro is the first choice because its current Rust system provides text CRDTs, rich-text marks with boundary-expansion configuration, persistent cursors, movable trees, collaborative undo behavior, version history, delta exchange, and shallow snapshots. It lives in the optional `std`-capable adapter tier; none of its public types cross the Underwood façade.

The collaboration contract requires:

- stable positions;
- immutable snapshots;
- explicit operations;
- deterministic application;
- schema-identified extensions;
- selection/presence layers;
- provenance;
- mapping across rebases.

`underwood_loro` must exercise concurrent text insertion/deletion, rich-style boundaries, node moves, comments, presence, streaming append, shallow history, compaction, persistent anchors, provenance, and selective undo. Every merge trace is checked for semantic convergence and for equivalent Underwood preparation snapshots.

Automerge is the first differential candidate because it has a mature local-first model and Peritext-inspired marks; server-authoritative and other CRDT adapters remain possible. A generalized collaboration trait is extracted only after a second maintained implementation demonstrates the shared contract. Until then, concrete APIs and trace formats are preferable to speculative neutrality.

Every adapter must declare whether it can preserve Underwood operation identities and selective-undo semantics. Unsupported mappings fail capability negotiation; they do not quietly degrade to snapshot rewind.

### 11.6 CRDT-native is a measured architecture question

Before collaborative document schemas stabilize, an ADR compares three concrete arrangements:

1. **Canonical-first:** Underwood owns text/tree/range state; Loro mirrors Underwood transactions and returns merged transactions.
2. **Loro-authoritative:** Loro text, tree, marks, and cursors own collaborative identity; Underwood publishes optimized immutable preparation snapshots from them.
3. **Hybrid:** collaborative documents retain CRDT identities in a sealed backend while small/non-collaborative documents use the compact canonical store.

The decision is made with product traces, not aesthetic preference. It measures label and small-document memory, append throughput, million-line iteration, range enumeration, snapshot publication, concurrent rich-text semantics, tree moves, selective undo, shallow loading, compaction, `no_std` impact, and the cost of maintaining two sources of truth.

A CRDT-native result does not make collaboration mandatory. A canonical-first result does not permit the adapter to paper over identity mismatches. The purpose of the experiment is to force the hardest identity questions into executable evidence before either representation calcifies.

### 11.7 The transaction log is the operation substrate

The transaction log is not merely an undo implementation. It is the common, replayable operation space for people, macros, importers, collaborators, and agents.

```rust
pub struct OperationAuthority {
    pub principal: PrincipalId,
    pub capabilities: CapabilitySet,
    pub scope: OperationScope,
    pub budget: OperationBudget,
    pub approval: ApprovalPolicy,
}

pub struct TransactionProposal {
    pub base_revision: DocumentRevision,
    pub commands: Box<[EncodedTypedCommand]>,
    pub authority: OperationAuthority,
    pub provenance: ProvenanceRecord,
}
```

Capabilities can restrict node subtrees, semantic roles, command families, resource access, destructive operations, and operation count. A proposal is resolved against a named snapshot, validated through the same schema and command machinery as interactive editing, and materialized as an inspectable transaction plus semantic/visual diff. Hosts may require approval before publication.

Replay records the command/operation versions, schema identities, relevant text-data manifest, and deterministic host inputs. An agent receives no privileged mutation API and cannot smuggle meaning through paint. MCP or another tool protocol belongs in an Overstory or server adapter; Underwood provides the capability-scoped transaction semantics that make such adapters trustworthy.

---

## 12. Selection and position model

### 12.1 Multi-selection is foundational

Even if a form field exposes one selection, the underlying engine should support a set.

```rust
pub struct SelectionSet {
    primary: SelectionId,
    selections: SmallVec<[DocumentSelection; 1]>,
}

pub struct DocumentSelection {
    pub id: SelectionId,
    pub anchor: DocumentPosition,
    pub focus: DocumentPosition,
    pub preferred_inline_position: Option<f64>,
    pub goal: SelectionGoal,
}
```

### 12.2 Logical and visual movement

Parley supplies paragraph-local cluster and visual-order mechanics. Underwood maps those through projection and across:

- inline objects;
- paragraph boundaries;
- blocks;
- columns;
- pages;
- folded or elided content;
- vertical writing modes.

### 12.3 Selection is not formatting

Selections, search matches, and remote presence are late overlay layers. They do not rewrite `StyledText`, document styles, or shaped layout.

---

## 13. Style architecture

### 13.1 Three value stages

Underwood distinguishes:

1. **Specified values**: authored declarations, inheritance markers, tokens, expressions.
2. **Computed values**: fully resolved, context-independent values suitable for interning.
3. **Used values**: geometry-dependent resolution such as percentages, line-relative offsets, or device-dependent decisions.

Used values are not permission to resolve something “somewhere in layout.” They are retained products at the closest dependency boundary:

```rust
pub struct UsedInlineValuesKey {
    pub computed_inline: InlineFlowStyleId,
    pub computed_block: ComputedBlockStyleId,
    pub containing_geometry: ContainingGeometryKey,
    pub writing_mode: WritingMode,
    pub device_text: DeviceTextConfigurationKey,
}

pub struct UsedInlineValues {
    pub letter_spacing: f32,
    pub word_spacing: f32,
    pub baseline_shift: f32,
    pub tabs: ResolvedTabStops,
}
```

Shape-sensitive used values, such as an explicit `opsz` variation derived from a size policy, are resolved before itemization and named in its key. Line-relative spacing and tabs are resolved before breaking. Block percentages, page-relative offsets, and fragmentation constraints form `UsedFlowValues` before flow placement. Paint-device values are resolved during scene lowering. The cache graph names each family; code review must reject incidental resolution inside an unrelated stage.

### 13.2 Typed declarations

```rust
pub enum Declaration<T> {
    Unspecified,
    Initial,
    Inherit,
    Value(T),
    Expression(TextExpressionId),
}

pub struct SpecifiedInlineStyle {
    pub font_family: Declaration<FontFamily<'static>>,
    pub font_size: Declaration<LengthExpr>,
    pub font_weight: Declaration<FontWeight>,
    pub font_style: Declaration<FontStyle>,
    pub language: Declaration<Option<Language>>,
    pub foreground: Declaration<BrushExpr>,
    pub underline: Declaration<DecorationExpr>,
}
```

This is typed unresolved data, not a universal value bag.

### 13.3 Computed partitions

```rust
pub struct ComputedInlineStyle {
    pub shaping: ShapingStyleId,
    pub inline_flow: InlineFlowStyleId,
    pub paint: PaintStyleId,
}

pub struct ShapingStyle {
    pub font_family: FontFamily<'static>,
    pub text_scale: TextScale,
    pub font_width: FontWidth,
    pub font_weight: FontWeight,
    pub font_style: FontStyle,
    pub variations: SharedSlice<FontVariation>,
    pub features: SharedSlice<FontFeature>,
    pub language: Option<Language>,
    pub tracking_topology: TrackingTopology,
    pub writing_mode: WritingMode,
}

pub struct InlineFlowStyle {
    pub line_height: LineHeight,
    pub word_break: WordBreak,
    pub overflow_wrap: OverflowWrap,
    pub hyphens: Hyphens,
    pub baseline_shift: f32,
    pub letter_spacing: f32,
    pub word_spacing: f32,
}

pub enum TrackingTopology {
    Normal,
    ExplicitTracking,
}

pub struct PaintStyle {
    pub foreground: Brush,
    pub foreground_transform: Option<Affine>,
    pub decorations: TextDecorations,
    pub stroke: Option<TextStroke>,
    pub shadows: SharedSlice<TextShadow>,
}
```

### 13.4 Block style

```rust
pub struct ComputedBlockStyle {
    pub alignment: TextAlign,
    pub base_direction: BaseDirection,
    pub indentation: Indentation,
    pub tabs: TabStops,
    pub spacing_before: f64,
    pub spacing_after: f64,
    pub keep_with_next: bool,
    pub widows: u16,
    pub orphans: u16,
    pub column_span: ColumnSpan,
}
```

### 13.5 Cascade sources

The document style resolver combines:

1. engine initial values;
2. containing-surface root style supplied by the toolkit;
3. document style sheets targeting roles/classes/attributes;
4. semantic styles such as links or code;
5. authored inline declarations;
6. interaction-state styles;
7. late overlays that do not belong in computed document style.

Overstory supplies theme tokens, property-expression values, pseudoclass state, and its inherited root properties through a resolver interface.

```rust
pub trait StyleEnvironment {
    fn root_style(&self) -> &ComputedRootStyle;
    fn resolve_expression<T: TextStyleValue>(
        &self,
        expression: TextExpressionId,
    ) -> Result<T, StyleError>;
    fn state(&self, semantic: SemanticId) -> SemanticState;
}
```

### 13.6 `styled_text` as computed run IR

Parley’s `styled_text` stores compact complete styles with separate layout and paint identity. Underwood should use it after cascade resolution, not as the authored document model.

Conceptually:

```rust
pub type PreparedStyledText<T> = styled_text::StyledText<
    T,
    ComputedLayoutStyle,
    PaintStyle,
>;

pub struct ComputedLayoutStyle {
    pub shaping: ShapingStyleId,
    pub inline_flow: InlineFlowStyleId,
}
```

This preserves `styled_text`’s useful layout/paint split while allowing Underwood to distinguish shaping from line-flow dependencies inside the layout payload.

### 13.7 Spacing and scale are designed cache boundaries

Letter and word spacing magnitudes are applied to advances after glyph shaping. Animating tracking must therefore update used inline values, breaking, and downstream geometry without keying `ShapeCache` on a raw `f32` every frame. If nonzero letter spacing changes optional-ligature behavior, itemization keys the discrete `TrackingTopology` predicate—not the magnitude. Explicit OpenType feature choices remain explicit and win according to a documented precedence rule.

Current Parley passes `font_size` into shaping and splits itemization on raw spacing values. Underwood must adapt to that behavior correctly today without making it the permanent architecture. Before the repository is founded, an ADR must test and decide whether retained glyph shaping can occur in font units so scale-only changes reuse glyph identity and update geometry. The ADR must account for:

- explicit versus automatic optical-size (`opsz`) variation;
- hinting and device-dependent advances;
- size-dependent fallback or font synthesis;
- color-font and bitmap strikes;
- backend numerical stability and line metrics.

The north star is cheap size animation and retained glyph identity. The rule is not “font size never affects shaping” until the conformance corpus proves it. Every exception becomes an explicit input key rather than an ambient reason to reshape.

---

## 14. Projection and source mapping

### 14.1 The shaped string is a projection

The semantic source may differ from displayed text because of:

- whitespace processing;
- password masking;
- generated list markers;
- case or display transformations;
- placeholders;
- inserted bidi controls;
- object replacement characters;
- IME preedit text in a session-local composition overlay;
- folded content;
- elision;
- generated labels;
- application-specific views.

Therefore source projection is a named retained stage.

```rust
pub struct ParagraphProjection {
    pub paragraph: ParagraphId,
    pub revision: ProjectionRevision,
    pub source_revision: TextRevision,
    pub text: SharedString,
    pub map: SourceMap,
    pub style_input: ProjectedStyleInput,
    pub inline_objects: SharedSlice<ProjectedInlineObject>,
    pub semantics: SharedSlice<ProjectedSemanticRange>,
}
```

### 14.2 Source map

```rust
pub struct SourceMap {
    segments: Box<[SourceMapSegment]>,
}

pub struct SourceMapSegment {
    pub projected: core::ops::Range<u32>,
    pub source: Option<SnapshotRange>,
    pub mapping: MappingKind,
}

pub enum MappingKind {
    Identity,
    Replaced,
    Generated,
    InlineObject(InlineObjectId),
    Elided,
    Composition(CompositionId),
}
```

Length-changing transformations require explicit mapping rules for cursor placement and editing. A password-mask projection, for example, cannot blindly map UTF-8 byte counts.

The source map belongs to one immutable snapshot, so its dense segments use `SnapshotRange`, not stable anchors. A caller that needs a durable position asks the document to create an anchor at a mapped boundary. Generated segments have no source range and carry an explicit attachment rule to a semantic node or neighboring source boundary.

### 14.3 Composition has its own epoch

The committed `ParagraphProjection` does not change during IME preedit. A session-local overlay composes with it under a separate identity:

```rust
pub struct CompositionProjection {
    pub base: ProjectionKey,
    pub composition: CompositionId,
    pub epoch: CompositionEpoch,
    pub text: SharedString,
    pub map: SourceMap,
    pub clauses: Box<[ProjectedCompositionClause]>,
}
```

Preedit churn invalidates composition itemization, shaping, breaking, and overlay geometry for the affected paragraph, but it does not evict the committed projection. Cancellation drops the overlay and reveals the still-cached committed preparation. Commit creates one document transaction and then advances the ordinary projection identity.

### 14.4 Projection is deterministic

Given the same snapshot, projection context, and relevant layer revisions, projection must produce the same text and source map. This is required for cache identity, replay, and collaboration diagnostics.

---

## 15. Parley integration and retained shaping

### 15.1 Use Parley’s emerging seams

Underwood should align with the movement of Parley toward:

- explicit whole-paragraph analysis;
- style-aware itemization predicates;
- caller-controlled font selection seams;
- retained `ShapedText`;
- bounded reshaping around line breaks;
- vertical orientation;
- first-class inline-object markers.

Underwood must not make `parley::Layout<Brush>` its permanent architectural center.

### 15.2 Explicit adapter

```rust
pub struct ParleyParagraphEngine {
    analyzer: parley_core::Analyzer,
    shaper: parley_core::Shaper,
    analysis_workspace: AnalysisWorkspace,
    shape_workspace: ShapeWorkspace,
}

pub trait ParagraphPreparation {
    fn analyze(
        &mut self,
        projection: &ParagraphProjection,
        styles: &PreparedStyleRuns,
    ) -> PreparedAnalysis;

    fn itemize(
        &mut self,
        projection: &ParagraphProjection,
        analysis: &PreparedAnalysis,
        styles: &PreparedStyleRuns,
        fonts: &mut dyn FontResolver,
    ) -> PreparedItemization;

    fn shape(
        &mut self,
        projection: &ParagraphProjection,
        analysis: &PreparedAnalysis,
        items: &PreparedItemization,
    ) -> PreparedShapedText;
}
```

### 15.3 Itemization splits only for shaping changes

Analysis is Unicode properties, segmentation, and bidi. It is independent of font family, weight, paint, and fallback. Itemization consumes analysis plus shaping-relevant style topology and font availability; it deserves its own retained key and instrumentation.

Paint boundaries must not split shaping items. Semantic boundaries must not split shaping items. Flow properties split only when Parley requires them during shaping or breaking. Raw letter- and word-spacing magnitudes do not split shaping items; only a documented discrete topology change, such as optional-ligature behavior under explicit tracking, may do so.

The itemizer receives an efficient predicate over the shaping-style topology.

### 15.4 Paint slots, not brushes in shaped identity

Actual brushes must not be embedded in shaping cache keys.

When drawing requires paint boundaries to be retained through layout, use stable boundary slots:

```rust
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct PaintSlot(u32);

pub struct PaintRun {
    pub projected_range: core::ops::Range<u32>,
    pub slot: PaintSlot,
}

pub struct PaintTable {
    pub revision: PaintRevision,
    pub values: Box<[PaintStyle]>,
}
```

Geometry may depend on paint-boundary topology if the renderer must divide glyph output at those boundaries, but it must not depend on the paint values. Replacing red with blue swaps the table, not the glyph geometry.

### 15.5 Ligatures across paint boundaries

Color changes inside text create a subtle requirement: preserving shaping across a paint-only boundary while drawing the correct visual portions of clusters or ligatures.

The design must explicitly choose and test behavior:

- preserve shaping and split drawing at cluster-safe boundaries;
- retain paint slots in shaped run metadata without changing shaping inputs;
- use bounded reshaping only when a backend cannot faithfully paint a shared glyph across the boundary;
- document behavior for style boundaries inside grapheme clusters or ligatures.

This cannot be left as an accidental consequence of `Layout<Brush>`.

### 15.6 The Parley contingency is designed, not improvised

“Do not fork text physics” means Underwood does not casually build a second shaper, bidi engine, or Unicode analyzer. It does not mean the platform waits indefinitely for every upstream seam.

`underwood_parley` is the only allowed adaptation boundary. It targets a pinned Parley revision and owns a conformance suite over analysis, itemization, shaping, breaks, vertical text, and inline objects. If a required capability is designed, demonstrated, and blocked upstream beyond an explicit review threshold, the project may carry a narrowly scoped patch stack or pinned fork there under these rules:

1. the gap and required semantics are recorded in an ADR;
2. an upstream issue or pull request exists when the change is generally applicable;
3. the patch is isolated behind Underwood-owned prepared types, never leaked through the public façade;
4. conformance runs against both upstream and patched backends where possible;
5. every divergence has an owner, upstreaming plan, and periodic removal review;
6. Underwood may implement a breaker over `parley_core` retained primitives if the high-level layout API cannot express region flow, but may not duplicate Unicode analysis or glyph shaping without a separate constitutional decision.

The escape hatch protects the mission without normalizing permanent divergence. Review bandwidth is a scheduling input; it is not allowed to become an accidental architectural veto.

### 15.7 Text data is an explicit capability

`no_std` and “no ambient locale” are incomplete unless the machine also owns how Unicode and language data enter it. Analysis, segmentation, bidi, line breaking, complex-script word boundaries, and hyphenation depend on versioned data.

Underwood takes an immutable data bundle at engine construction:

```rust
pub struct TextDataIdentity {
    pub unicode: UnicodeVersion,
    pub provider_format: TextDataFormatVersion,
    pub segmentation: DataSetId,
    pub line_break: DataSetId,
    pub bidi: DataSetId,
    pub locale_tailoring: Option<DataSetId>,
    pub hyphenation: Option<DataSetId>,
}

pub trait TextDataProvider {
    fn load_bundle(
        &self,
        request: &TextDataRequest,
    ) -> Result<Shared<TextDataBundle>, TextDataError>;
}
```

The provider is an initialization and data-loading seam, not virtual dispatch inside a character loop. `TextDataBundle` contains prepared immutable tables consumed concretely by the Parley adapter and Underwood breakers.

The project ships deliberate tiers:

- a minimal built-in tier with documented Unicode version and baseline segmentation/bidi/line-break behavior;
- feature-gated complex-script dictionaries and locale tailoring;
- host-supplied hyphenation and language packs;
- a full-data distribution for compositor and server use.

Each tier publishes compressed and resident-size budgets, especially for WebAssembly. Unsupported data is a negotiated capability with diagnostics, never silent “English enough” behavior.

`TextDataIdentity` participates in analysis, breaking, command-resolution, replay, and scene-protocol manifests wherever results depend on it. Upgrading Unicode or hyphenation data is an explicit environment revision that produces targeted invalidation. Primitive transactions remain replayable as edits; replaying higher-level commands such as “delete previous word” requires the recorded data identity or a declared migration.

The shape of this contract should resemble ICU4X’s explicit data provisioning, while the Parley adapter handles the concrete APIs Parley exposes. Current baked analysis data is a valid provider implementation, not the permanent capability ceiling.

---

## 16. Incremental cache graph

### 16.1 Cache stages

```text
CommittedProjectionCache
    key: paragraph identity + source/layer revisions + projection context

CompositionProjectionCache
    key: committed projection + composition id/epoch

StyleCache
    key: projected topology + style-sheet/environment revisions

AnalysisCache
    key: projected text + analysis options + text-data identity

ItemizationCache
    key: analysis + shaping-style topology + font generation

ShapeCache
    key: itemization + glyph-shaping inputs + backend configuration

UsedInlineValuesCache
    key: computed flow style + containing geometry + device-text configuration

BreakCache
    key: shaped text + used inline values + available intervals

UsedFlowValuesCache
    key: computed block style + region/page geometry + fragmentation context

TableConstraintCache
    key: table structure + intrinsic cell measurements + available measure

FlowCache
    key: block/table metrics + region chain + break plans + object metrics

FlowCheckpointCache
    key: flow input + preceding checkpoint + block interval

VirtualExtentCache
    key: flow revision + measured/estimated block extents

SceneGeometryCache
    key: flow result + geometry-affecting decoration topology

PaintTableCache
    key: computed paint values

SemanticMapCache
    key: document semantics + projection + flow geometry
```

### 16.2 Example dependency keys

```rust
pub struct ItemizationKey {
    pub projection: ProjectionKey,
    pub analysis: AnalysisKey,
    pub shaping_topology: StyleTopologyKey,
    pub fonts: FontGeneration,
}

pub struct ShapeKey {
    pub itemization: ItemizationKey,
    pub glyph_inputs: GlyphShapingInputsKey,
    pub backend: ShapingBackendKey,
}

pub struct BreakKey {
    pub shaped: ShapedTextKey,
    pub used_inline: UsedInlineValuesKey,
    pub intervals: AvailableIntervalsKey,
    pub breaker: BreakerConfigurationKey,
}
```

Do not use a global document revision where a paragraph fingerprint is sufficient.

### 16.3 Invalidation matrix

| Change | Projection | Style | Analysis | Itemize | Shape | Used | Break | Flow | Geometry | Paint | Semantics |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Foreground brush value | no | yes | no | no | no | no | no | no | no | yes | no |
| Underline color | no | yes | no | no | no | no | no | no | no | yes | no |
| Letter/word spacing magnitude | no | yes | no | no | no | yes | yes | yes | yes | no | geometry only |
| Tracking topology change | no | yes | no | yes | affected items | yes | yes | yes | yes | no | geometry only |
| Font weight | no | yes | **no** | yes | affected items | yes | yes | yes | yes | no | geometry only |
| Font size/scale | no | yes | no | explicit exceptions | backend ADR | yes | yes | yes | yes | no | geometry only |
| Paragraph width | no | no | no | no | no | yes | yes | yes | yes | no | geometry only |
| Text insertion | affected paragraph | affected runs | affected paragraph | affected items | affected items | affected runs | affected paragraph | from edit frontier | affected | affected | affected |
| Link target change | no | semantic rules only | no | no | no | no | no | no | no | rule-dependent | yes |
| Selection move | no | no | no | no | no | no | no | no | overlay only | overlay only | selection only |
| Search match update | no | no | no | no | no | no | no | no | overlay only | overlay only | no |
| IME preedit change | composition only | affected overlay | affected composition | affected composition | affected composition | affected composition | affected paragraph | local | overlay | overlay | composition only |
| Font loaded | no | no | no | affected candidates | affected items | affected runs | affected paragraphs | downstream | affected | no | geometry only |
| Text-data identity | no | no | affected paragraphs | downstream | downstream | downstream | downstream | downstream | affected | no | affected segmentation |
| Page size change | no | no | no | no | no | affected values | affected intervals | from checkpoint | affected | no | geometry only |
| Inline-object metrics | marker stable | no | no | no | no | affected object | affected lines | from checkpoint | affected | no | geometry only |

“Maybe” is not an admissible implementation state. The matrix may point to an explicit ADR or a named conditional key, but the engine must know whether a property participates in analysis, itemization, shaping, used-value resolution, breaking, flow, geometry, paint, or semantics. In particular, font weight never invalidates Unicode analysis.

### 16.4 Caches are bounded and inspectable

Every cache declares:

- memory accounting;
- eviction policy;
- shareability across sessions;
- revision lifetime;
- hit/miss/eviction counters;
- diagnostic key summaries;
- invalidation reasons.

No unbounded global cache.

---

## 17. Flow and composition

### 17.1 More than maximum width

The flow engine places block and inline content into a chain of regions. A region may be rectangular, shaped, transformed, paginated, or divided into columns.

```rust
pub struct FlowRegion {
    pub id: FlowRegionId,
    pub geometry: FlowGeometryId,
    pub transform: Affine,
    pub writing_mode: WritingMode,
    pub next: Option<FlowRegionId>,
}

pub trait FlowGeometry {
    fn block_extent(&self) -> f64;

    fn inline_intervals(
        &self,
        block_position: f64,
        band_height: f64,
        exclusions: &ExclusionSet,
        output: &mut Vec<InlineInterval>,
    );
}
```

This interface supports rectangles, rounded shapes, arbitrary paths, holes, and exclusions without forcing the line breaker to understand every geometry representation.

### 17.2 Region chains

```rust
pub struct FlowThread {
    pub regions: Box<[FlowRegion]>,
    pub overflow: OverflowDestination,
}
```

A thread can represent:

- a scrollable viewport;
- newspaper columns;
- linked design-canvas text frames;
- pages;
- a shaped label;
- a report body with headers, footers, and footnote regions.

### 17.3 Resumable flow is a retained primitive

Pagination is sequential, so “offscreen pages are cheap” requires a mechanism. Underwood retains compact flow cursors at deterministic boundaries.

```rust
pub struct FlowCheckpoint {
    pub version: FlowCheckpointFormatVersion,
    pub input: FlowInputKey,
    pub after_block: BlockCursor,
    pub region: FlowRegionId,
    pub region_cursor: RegionCursor,
    pub pending_floats: Box<[PendingFloatState]>,
    pub keeps: KeepConstraintState,
    pub footnotes: FootnoteCarryState,
    pub counters: GeneratedCounterState,
    pub extent_before: f64,
}
```

A checkpoint contains only the state necessary to resume flow; it does not retain all preceding geometry. It is serializable with a versioned private format so background pagination, remote preparation, and deterministic diagnostics can resume the same state.

Checkpoints occur at page/region boundaries and at adaptive block intervals in long scrolling flows. After an edit, flow starts at the nearest valid predecessor checkpoint, advances through the changed frontier, and stops when both output and checkpoint state converge with a retained successor. The system promises that it will not recompute the unaffected prefix. It does not falsely promise that inserting a page-one paragraph can never move every later page.

Checkpoint density is policy driven by measured resume cost and memory, but checkpoint semantics are engine-owned. Footnote carry, active exclusions, counters, keeps, and table-fragment state may not hide in an uncheckpointed local variable.

### 17.4 Virtual extents and scroll anchoring

The technical editor has the inverse problem: it must not flow a million lines merely to know the scroll range. Underwood maintains a virtual extent map whose entries disclose whether they are estimated or measured.

```rust
pub enum ExtentKnowledge {
    Estimated { confidence: ExtentConfidence },
    Measured { layout: FlowLayoutKey },
}

pub struct VirtualExtent {
    pub blocks: BlockInterval,
    pub block_extent: f64,
    pub knowledge: ExtentKnowledge,
}

pub struct ViewportCorrection {
    pub anchor: StableAnchor,
    pub old_scene_position: Point,
    pub new_scene_position: Point,
    pub recommended_scroll_delta: Vec2,
}
```

When measurement replaces an estimate above or across the viewport, Underwood reports the correction required to keep a chosen document anchor visually stable. Overstory chooses the anchor and policy—caret, first visible line, pointer target, or no correction—but never reconstructs the delta from private line data. Hit testing and accessibility expose estimated, realized, and unavailable states honestly.

### 17.5 Paragraph breaking and bounded reshaping

The flow engine asks the paragraph engine for candidate breaks given available inline intervals. Once it commits a break, Parley may need to reshape a bounded region around that break to sever joins or ligatures correctly.

Changing region width should preserve paragraph analysis and the initial shaped representation.

### 17.6 Block layout

Underwood owns document block flow, not general UI layout. It must handle:

- paragraph spacing and collapsing policy;
- indentation;
- list markers;
- keep-with-next;
- keep-together;
- widow and orphan constraints;
- tables and cell content regions;
- page/column breaks;
- footnote placement;
- floats and exclusions.

The block system should remain document-specific and should not evolve into a second Taffy or CSS box engine.

### 17.7 Tables are a first-class layout subsystem

Tables are not “blocks with rows.” Underwood gives them an explicit retained constraint and fragmentation pipeline:

```text
TableGrid
    ├─ validated row/column occupancy and spans
    ▼
IntrinsicCellMeasures
    ├─ min/max inline contributions
    ├─ fixed/percentage constraints
    └─ object and nested-table contributions
    ▼
TableConstraintSolution
    ├─ column measures
    ├─ row constraints
    └─ border model
    ▼
TableFragmentPlan
    ├─ row and cell fragmentation
    ├─ repeated headers/footers
    └─ per-fragment cell regions
```

The subsystem must define auto and fixed width resolution, spanning-cell constraint contribution, percentage interaction, border-collapse policy, row height, vertical alignment, nested tables, keep constraints, repeated headers, and breaking a cell across columns or pages. A cell’s content is a nested flow thread with a constrained region, but table constraint solving is not delegated to general block flow.

Intrinsic cell measurements and constraint solutions are cached separately. Editing one cell invalidates its intrinsic contribution and only the dependent column/row solution before downstream fragment flow. A table ADR and corpus are mandatory before the public table schema is stabilized; a simplified table algorithm must never masquerade as the final contract.

### 17.8 Advanced paragraph algorithms have explicit domains

The architecture must permit:

- greedy breaking;
- Knuth–Plass or other global optimization;
- language-aware hyphenation;
- justification strategies;
- balanced headings or captions;
- optical margin alignment;
- application-specific breakers.

Break algorithms consume retained shaping and flow intervals through an explicit trait. Their optimization domain is part of the request: one paragraph in fixed intervals, a balanced heading, or a bounded group of lines in one region.

Unconstrained Knuth–Plass across an arbitrary region chain with floats is not assumed tractable. Line widths depend on region placement; region placement depends on breaks; floats and footnotes can change both. Underwood therefore requires an ADR for the solver protocol and exposes diagnostics for the chosen domain, iteration count, fallback, and loss function. Exceptional typography is a real capability, not a magical `Optimal` enum variant.

### 17.9 Exclusions and objects use bounded convergence

Floats, exclusions, footnotes, tables, and width-dependent inline objects participate in a deterministic negotiation:

1. begin from retained intrinsic metrics and a candidate flow state;
2. compute intervals and candidate breaks within a declared optimization domain;
3. place objects and update exclusions/carry state;
4. repeat only while the declared state changes and the iteration budget remains;
5. publish the stable result or a structured fallback with `LayoutCycleReport`.

The protocol records every state transition. Each object type must declare whether its response is monotone, bounded, or arbitrary; arbitrary feedback receives the strictest budget. A solver may fall back to greedy breaking, defer a float, or pin a previous stable placement according to explicit policy. It may not spin, silently oscillate, or imply that paragraph-global optimality survived a changed region graph.

---

## 18. Inline objects

### 18.1 Object identity and projection

The semantic document contains an inline-object node. Projection emits an object replacement marker plus an `InlineObjectId` mapping.

```rust
pub struct ProjectedInlineObject {
    pub id: InlineObjectId,
    pub projected_range: core::ops::Range<u32>,
    pub role: InlineObjectRole,
}
```

### 18.2 Measurement protocol

```rust
pub trait InlineObjectProvider {
    fn measure(
        &mut self,
        id: InlineObjectId,
        constraints: InlineObjectConstraints,
    ) -> InlineObjectMetrics;

    fn presentation(&self, id: InlineObjectId) -> InlinePresentationRef;

    fn semantics(&self, id: InlineObjectId) -> InlineObjectSemantics;
}

pub struct InlineObjectMetrics {
    pub size: Size,
    pub baseline: Option<f64>,
    pub margins: Insets,
    pub break_behavior: InlineBreakBehavior,
}
```

### 18.3 Placement modes

- in-flow inline object;
- out-of-flow anchored object;
- float/exclusion object;
- custom object that yields control to the flow engine;
- replaced content with intrinsic size;
- live Overstory widget adapter.

Parley sees the shaping boundary and metrics. Underwood owns object identity, flow behavior, placement, and semantic mapping. Overstory may realize a widget and route events.

### 18.4 Feedback must converge

Inline widgets may depend on available width, while text flow depends on widget size. The engine needs an explicit bounded layout negotiation protocol with cycle diagnostics. Never permit an unbounded measure/reflow loop.

All inline-object feedback enters the convergence protocol in §17.9 and must be representable in a `FlowCheckpoint`. An object provider cannot retain hidden measurement state that changes layout without advancing its resource revision.

### 18.5 Recursive document surfaces

An inline or block object may host another Underwood document/session or an Overstory widget whose descendants include documents. This is a first-class recursive surface, not a lucky consequence of “custom objects.”

```rust
pub struct EmbeddedDocumentSurface {
    pub id: EmbeddedSurfaceId,
    pub document: DocumentHandle,
    pub session: Option<SessionHandle>,
    pub selection_boundary: SelectionBoundaryPolicy,
    pub undo_scope: UndoScope,
    pub semantics: EmbeddedSemanticBridge,
}

pub enum SelectionBoundaryPolicy {
    Atomic,
    Enterable,
    Traversable,
}

pub enum UndoScope {
    Isolated,
    Coordinated(TransactionCoordinatorId),
}
```

`Atomic` makes the child one selectable object from the parent. `Enterable` lets focus and selection enter the child but prevents one range from spanning both sessions. `Traversable` participates in document-order navigation and copy semantics through an explicit composite selection; it is supported only when both schemas declare a compatible bridge.

Nested preparation is never reentrant. The parent publishes constraints; the child prepares against an immutable snapshot and returns intrinsic metrics plus a nested scene reference; the parent then places that revision through the ordinary convergence protocol. An `EmbeddingPath` detects direct and indirect cycles, and hosts impose depth, work, scene, and accessibility budgets.

Undo is isolated by default. Coordinated cross-document commands use a transaction coordinator that validates all participant base revisions and publishes an atomic composite record or none. A child cannot silently join its parent’s typing group. Focus routing and platform accessibility-node stitching belong to Overstory; semantic parent/child identity, navigation order, nested scene geometry, and transaction scope belong to Underwood.

Renderers may retain a nested `TextScene` reference rather than flattening it eagerly. Scene deltas, hit testing, and accessibility carry the embedding path so identical child documents can be reused without losing instance identity.

---

## 19. Prepared `TextScene`

### 19.1 Scene separation

Split stable geometry from frequently changing paint and overlays.

```rust
pub struct TextScene {
    pub key: TextSceneKey,
    pub revision: TextSceneRevision,
    pub geometry: Shared<TextSceneGeometry>,
    pub paint: Shared<PaintTable>,
    pub semantics: Shared<SemanticMap>,
    pub overlays: Shared<OverlayScene>,
}
```

### 19.2 Geometry sketch

```rust
pub struct TextSceneGeometry {
    pub fragments: Box<[GlyphFragment]>,
    pub glyphs: Box<[SceneGlyph]>,
    pub positions: Box<[GlyphPosition]>,
    pub decorations: Box<[DecorationGeometry]>,
    pub inline_objects: Box<[PlacedInlineObject]>,
    pub lines: Box<[LineGeometry]>,
    pub hit_map: TextHitMap,
    pub caret_map: CaretMap,
}

pub struct GlyphFragment {
    pub id: SceneFragmentId,
    pub font: FontResourceKey,
    pub font_size: f32,
    pub variation_coords: CoordinateRange,
    pub glyphs: core::ops::Range<u32>,
    pub positions: core::ops::Range<u32>,
    pub paint: PaintSlot,
    pub transform: Affine,
    pub source: SemanticFragmentId,
}
```

### 19.3 Semantic map

```rust
pub struct SemanticMap {
    pub fragments: Box<[SemanticFragment]>,
}

pub struct SemanticFragment {
    pub semantic_id: SemanticId,
    pub source_range: Option<SnapshotRange>,
    pub geometry: FragmentGeometryRef,
    pub role: SemanticRole,
}
```

The semantic map enables accessibility, link hit testing, comments, selection geometry, and agent inspection without reverse-engineering draw commands.

### 19.4 Presentation integration

`understory_presentation` should retain a key and lightweight layout intent, not the complete scene:

```rust
pub struct PreparedTextPrimitive<TextKey = u64> {
    pub scene: TextKey,
    pub clip: Option<Rect>,
}
```

Overstory’s resource registry resolves the key and hands the scene to the presenter.

### 19.5 Scene deltas are a protocol, not a serialization afterthought

Stable fragment identities, immutable scene revisions, and separated resource tables make scene differences a natural wire format.

```rust
pub struct TextSceneDelta {
    pub protocol: SceneProtocolVersion,
    pub base: TextSceneRevision,
    pub target: TextSceneRevision,
    pub resources: Box<[ResourceDelta]>,
    pub geometry: Box<[GeometryDelta]>,
    pub paint: Box<[PaintDelta]>,
    pub semantics: Box<[SemanticDelta]>,
    pub overlays: Box<[OverlayDelta]>,
}
```

The delta protocol can support server-authoritative preparation to thin clients, collaborative previews, deterministic cross-machine snapshots, test artifact inspection, and low-bandwidth remote presentation. It transmits renderer-neutral glyph/resource identities and geometry, never a GPU atlas or backend command stream.

Every delta names its base revision and data/resource manifest. A receiver applies it atomically or requests a full checkpoint scene; it never guesses after a missing base. Resource blobs are content-addressed, bounded, and separately authorized. Geometry, paint, semantics, and overlays can advance independently when their dependency keys allow it.

This protocol is part of `underwood_scene`, with codecs in an optional crate. It is not allowed to constrain in-process table layouts to a frozen wire representation; versioned encoders translate from the live scene model.

---

## 20. Editing engine

### 20.1 `DocumentSession`

```rust
pub struct DocumentSession {
    snapshot: DocumentSnapshot,
    selection: SelectionSet,
    composition: Option<CompositionState>,
    typing_style: TypingStyle,
    history: EditHistory,
    generation: SessionGeneration,
}

impl DocumentSession {
    pub fn snapshot(&self) -> &DocumentSnapshot {
        &self.snapshot
    }

    pub fn selection(&self) -> &SelectionSet {
        &self.selection
    }

    pub fn execute<C: EditCommand>(
        &mut self,
        command: C,
        context: &mut EditContext<'_>,
    ) -> Result<EditOutcome, EditError> {
        command.execute(self, context)
    }

    pub fn apply(
        &mut self,
        transaction: EditTransaction,
    ) -> Result<EditOutcome, EditError> {
        self.apply_transaction(transaction)
    }
}
```

The session is toolkit-independent. It does not blink a caret, access the clipboard, or interpret keyboard modifiers.

### 20.2 IME composition

IME preedit is session state projected into the paragraph without prematurely mutating committed document content.

```rust
pub struct CompositionState {
    pub id: CompositionId,
    pub replacement: StableRange,
    pub preedit: SharedString,
    pub selection: core::ops::Range<u32>,
    pub clauses: Box<[CompositionClause]>,
}
```

The composition projection includes preedit text and clause styling under its own epoch. Commit produces one transaction. Cancellation drops the overlay and reuses the unchanged committed projection and preparation caches.

### 20.3 Editing policy

Underwood defines semantic commands and correct document transformations. Overstory owns mappings such as:

- macOS Option+Arrow versus Windows Ctrl+Arrow;
- Home/End behavior;
- single-line Enter behavior;
- password field restrictions;
- platform clipboard formats;
- focus and selection gestures;
- caret blink cadence.

### 20.4 Read-only and constrained sessions

Plain text fields and text areas are restrictions over the same session machinery:

- schema allows one paragraph or plain paragraphs;
- style edits disabled;
- inline objects disabled;
- command policy rejects structural operations;
- projection may mask text;
- validation/coercion supplied by Overstory.

Do not maintain a permanently separate plain-editor architecture merely because the public control is simpler.

### 20.5 Append-heavy sessions batch publication

Transcripts, logs, model output, and telemetry views must not publish one snapshot and remap anchors for every token. Underwood provides an append ingestion mechanism that coalesces source chunks, layer updates, and semantic events into atomic transactions under an explicit latency/size budget.

```rust
pub struct AppendBatchPolicy {
    pub max_latency: DurationBudget,
    pub max_utf8_bytes: u32,
    pub max_operations: u32,
    pub paragraph_flush: ParagraphFlushPolicy,
}

pub struct AppendBatch {
    pub text: SharedString,
    pub annotations: Box<[PendingAppendAnnotation]>,
    pub provenance: ProvenanceId,
}
```

The hot tail may use a mutable builder owned by the ingestion session; publication freezes structurally shared chunks into an immutable snapshot. One `EditSummary` maps sparse anchors and bulk tracked ranges for the entire batch. Derived token layers publish revision-local windows at the same or a later cadence.

Retention is equally explicit. A `StreamRetentionPolicy` defines hot text, retained snapshot/checkpoint horizons, and eviction behavior for anchors, selections, provenance, and accessibility. Dropping an old prefix is a typed structural transaction with a report of durable references preserved, relocated to an eviction boundary, or invalidated. No background cache may keep an ostensibly evicted transcript alive indefinitely.

Batching changes publication economics, not transaction semantics: every visible snapshot remains deterministic, replayable, and attributable.

---

## 21. Accessibility and semantic interaction

### 21.1 Division of responsibility

- Document: roles, hierarchy, language, links, labels, editable state, semantics.
- Underwood layout: geometry, logical/visual mapping, visible fragments, range bounds.
- Overstory: AccessKit nodes, platform actions, focus, virtualization policy.

### 21.2 Required capabilities

- headings and outline hierarchy;
- paragraphs and language changes;
- lists and list items;
- links and interactive inline objects;
- tables, rows, columns, and cells;
- code blocks;
- editable values and selections;
- text ranges and bounds;
- logical navigation through bidi content;
- virtualized semantic trees for huge documents;
- actions that compile to Underwood commands.

### 21.3 Accessibility is tested as architecture

Every reference document must have an expected semantic tree independent of rendering. Geometry tests verify mapping from semantic ranges to visual fragments.

### 21.4 Platform ceilings are reported, not abstracted away

AccessKit is the bridge, not proof that every assistive-technology stack behaves correctly. Huge virtualized documents, nested editable surfaces, mixed bidi selections, live streaming regions, tables, and scene-hosted controls require manual and automated testing across platform APIs, screen readers, magnifiers, switch access, and voice control.

Underwood and Overstory maintain a capability matrix that distinguishes semantic support, bridge support, platform exposure, and verified assistive-technology behavior. Virtualized nodes preserve stable focus and navigation identities, but realization strategy may differ by host. Known platform ceilings are documented with degraded behavior and reproducible traces rather than hidden behind a uniform trait.

---

## 22. Overstory integration

### 22.1 Public controls

```rust
Text::new("Run")

TextField::new(session_handle)
    .placeholder("Name")

TextArea::new(session_handle)
    .wrap(TextWrap::Soft)

TextView::new(document_handle)
    .style_sheet(article_styles)

TextEditor::new(session_handle)
    .class("design-spec")
    .inline_object_factory(factory)
```

These APIs are policies and integrations, not distinct engines.

### 22.2 Property integration

Overstory resolves inherited element properties into an Underwood root style. Document style sheets then cascade within the semantic document.

Overstory properties should not store a deeply mutable document value. They store a document/session handle whose published revision changes explicitly.

```rust
pub struct DocumentHandle {
    pub id: DocumentId,
    pub revision: DocumentRevision,
}
```

The UI subscribes to revision notifications and applies Underwood’s invalidation summary to Overstory channels.

### 22.3 Presentation

The widget writes a prepared text scene key into presentation. It never calls an Imaging painter from `Widget::paint` and never causes the presenter to read dependency properties.

### 22.4 Inline widgets

Overstory provides an `InlineObjectProvider` adapter that:

- realizes document object nodes as widgets;
- measures them through normal layout contracts;
- returns baselines and metrics to Underwood;
- maps Underwood placement into presentation nodes;
- routes focus, events, and accessibility.

The document model remains free of widget instances.

### 22.5 Devtools

Overstory devtools should expose:

- document and session revisions;
- paragraph preparation states;
- cache keys and invalidation reasons;
- style cascade traces for a document range;
- projection and source-map inspection;
- shaped runs, lines, regions, and pages;
- semantic fragments;
- flow checkpoints, estimated/measured extents, and convergence traces;
- text-data and Parley-backend identities;
- scene-delta and remote-resource state;
- edit transaction history;
- memory and latency metrics.

### 22.6 Recursive surface integration

Overstory realizes `EmbeddedDocumentSurface` as a focus scope and presentation subtree. It owns pointer transfer, tab navigation, caret reveal, hover, capture, platform focus, and AccessKit parent/child stitching. Underwood supplies boundary policy, composite document order, nested geometry, and coordinated transaction semantics.

The host must visibly distinguish selecting an embedded surface, entering it, and traversing through it. Copy, drag, delete, and accessibility actions compile to typed commands at the correct session boundary. An embedded editor cannot become an event-routing island or an accessibility subtree detached from the containing document.

### 22.7 Web text input is a release gate

“WebAssembly support” is not established by compiling the core to `wasm32`. A canvas-backed editing surface is credible only when a browser host passes real composition, `beforeinput`, selection, clipboard, focus, virtual keyboard, accessibility, and caret-geometry traces.

The Overstory web host may use a hidden textarea, contenteditable proxy, or another platform bridge, but the bridge is a replaceable host policy with an explicit synchronization protocol. It must handle composition replacement ranges, surrounding-text requests, mobile keyboard resize, browser zoom, bidi caret placement, and recovery when DOM and Underwood selection epochs diverge.

Web release claims name the tested browser/OS/input-method matrix and the accessibility limitations that remain. If a browser cannot expose a required capability reliably, the host reports a degraded mode; the engine does not bury the gap under a generic `wasm` feature.

---

## 23. Rendering integration

### 23.1 Imaging adapter

```rust
pub trait TextSceneRenderer {
    type Error;

    fn render(
        &mut self,
        scene: &TextScene,
        transform: Affine,
        clip: Option<Rect>,
    ) -> Result<(), Self::Error>;
}
```

Imaging lowering resolves:

- font resources;
- glyph outlines or cached images;
- hinting policy;
- paint slots;
- decorations;
- clipping;
- color glyphs;
- backend-specific batching.

### 23.2 GPU and CPU parity

The scene representation must not assume a GPU atlas. CPU rendering may use outlines, sparse strips, or raster caches. PDF may prefer glyph references and vector decoration geometry.

### 23.3 Device scale and hinting

Device-dependent preparation must be isolated from document shaping where possible. If hinted advances affect shaping or line layout, the dependency becomes explicit through a device/text rendering configuration key rather than ambient state.

### 23.4 Tagged PDF and PDF/UA are first-class output

`SemanticMap` plus projection source maps make genuinely accessible PDF an architectural consequence Underwood should claim. `underwood_pdf` lowers the semantic structure and paginated scene together to:

- a logical structure tree and reading order independent of paint order;
- marked-content associations from semantic fragments to page geometry;
- `ActualText` for ligatures, generated presentation, and approved projections;
- ToUnicode mappings derived from shaped clusters and source mapping;
- language metadata, headings, lists, tables, links, notes, and form semantics;
- alternate descriptions for non-text objects;
- explicit artifact marking for decorative or repeated presentation content.

The exporter consumes semantic structure, source maps, font resources, and page flow—not draw commands alone. Password masking, redaction, elision, private annotations, and generated content pass through an export policy so accessibility metadata never leaks concealed source text.

Font subsetting and embedding are governed by an explicit export capability that preserves license metadata and distinguishes embeddable, subsettable, outline-only, substitutable, and forbidden resources. The engine reports policy failures before export; it does not silently violate font rights or rasterize text in a way that destroys accessibility.

This is not “free PDF”: tagging order, table semantics, font embedding, conformance validation, and archival constraints require a dedicated corpus and validator loop. But the hard prerequisite—meaning joined to exact geometry and source text—already belongs to Underwood rather than being reverse-engineered after rendering. Accessible PDF and PDF/UA conformance are a named capability of the compositor reference application, not a someday adapter bullet.

---

## 24. Scheduling and concurrency

### 24.1 Immutable work inputs

Snapshots and preparation inputs are immutable, allowing paragraph work to occur off the UI thread.

### 24.2 Underwood does not own a task runtime

The engine exposes units of work and cancellation/revision checks. The host chooses the scheduler, whether `nx_taskpool`, an Overstory runtime, a server executor, or synchronous execution.

```rust
pub struct PreparationRequest {
    pub snapshot: DocumentSnapshot,
    pub viewport: ViewportSpec,
    pub regions: FlowThread,
    pub configuration: LayoutConfiguration,
}

pub trait PreparationScheduler {
    fn submit(&self, work: PreparationWork);
}
```

### 24.3 Deterministic publication

Parallel paragraph results may complete in any order, but scene publication and cache mutation must be deterministic. Results carry input revisions; stale results are discarded without corrupting current state.

### 24.4 Font contexts

Font discovery and source caches are shared through explicit resources. Mutable shaping contexts are worker-local or safely pooled. Loading a font advances `FontGeneration` and produces targeted invalidation.

---

## 25. Interchange and persistence

### 25.1 No accidental canonical external format

Underwood’s in-memory semantic model is not HTML, Markdown, RTF, OOXML, or a ProseMirror JSON tree. Each loses or imposes semantics inappropriate for some consumers.

### 25.2 Adapters

Optional adapters can support:

- plain text;
- HTML;
- Markdown;
- RTF;
- application-specific structured JSON;
- clipboard multi-flavor fragments;
- Tagged PDF/PDF-UA export through semantic, source-map, and scene/layout data;
- versioned `TextSceneDelta` codecs and full-scene checkpoints;
- code-editor protocols;
- collaborative model formats.

### 25.3 Internal serialization

If Underwood defines a durable binary snapshot format, it must be explicitly versioned, schema-aware, and forward-compatible. Do not serialize private arena layouts and mistake that for a format.

---

## 26. Diagnostics, observability, and failure

### 26.1 Errors are structured

```rust
pub enum PreparationError {
    InvalidDocument(InvalidDocumentError),
    Projection(ProjectionError),
    Style(StyleError),
    Font(FontError),
    InlineObject(InlineObjectError),
    NonConvergingLayout(LayoutCycleReport),
    Resource(ResourceError),
}
```

Expected absence, such as an unavailable optional font, should normally produce diagnostics and fallback rather than aborting the entire document.

### 26.2 Layout cycle reports

If inline measurement or region dependencies fail to converge, report:

- involved object and region IDs;
- iteration values;
- constraints and returned metrics;
- dependency path;
- last stable scene, if one exists.

### 26.3 Instrumentation vocabulary

At minimum:

- `underwood.project`;
- `underwood.style.resolve`;
- `underwood.analyze`;
- `underwood.itemize`;
- `underwood.shape`;
- `underwood.values.resolve`;
- `underwood.break`;
- `underwood.flow`;
- `underwood.flow.checkpoint`;
- `underwood.table.solve`;
- `underwood.scene.prepare`;
- `underwood.edit.apply`;
- `underwood.cache.hit/miss/evict`;
- `underwood.font.invalidate`.

Spoor events should carry document, paragraph, revision, and cache-stage identities without leaking document contents by default.

### 26.4 Security and privacy posture

Documents, fonts, text-data bundles, images, interchange payloads, scene deltas, and collaborative operations may be untrusted.

- Parsing and import adapters enforce depth, size, span-count, and resource limits.
- Invalid external structure produces diagnostics or rejected transactions, not corrupted snapshots.
- Preparation work has cancellation and budget hooks so adversarial content cannot monopolize a host.
- Font and image decoding remain behind hardened upstream/resource boundaries.
- Links and embedded commands are inert semantic values until a host policy activates them.
- Telemetry never records source text, replacement text, clipboard contents, link targets, or document metadata by default.
- Debug dumps containing content are explicit, scoped, and clearly marked.
- Collaboration adapters authenticate transport and validate operations before applying them to a session.
- Scene-delta receivers validate base revision, counts, ranges, transforms, resource hashes, and allocation budgets before atomic publication.
- Text-data providers authenticate or content-address external bundles and verify format/version compatibility before preparation.

---

## 27. Performance contracts

### 27.1 Metrics that matter

- edit-to-present latency;
- time per preparation stage;
- paragraphs analyzed/shaped/broken per update;
- glyphs reshaped around breaks;
- cache hit rates and evictions;
- memory per source character;
- memory per visible glyph;
- offscreen retained memory;
- projection/source-map overhead;
- transaction, anchor-update, and bulk-range-transform cost;
- flow resume distance and checkpoint convergence distance;
- estimated-to-measured extent error and viewport correction;
- append batch size, publication cadence, and retained tail memory;
- Loro merge-to-snapshot publication latency and changed-frontier size;
- recursive-surface negotiation count, depth, and nested-scene reuse;
- host input/composition event-to-visible latency;
- scene lowering and draw time;
- font fallback cost;
- accessibility realization cost.

### 27.2 Target behavior

Exact budgets should be established through measurement, but architectural expectations are non-negotiable:

- paint-only changes do no shaping;
- selection changes do no document style resolution;
- width changes do no Unicode analysis;
- local paragraph edits do not reshape unrelated paragraphs;
- invisible pages need not have fully materialized scene geometry because flow can resume from checkpoints;
- virtualized flows distinguish estimated from measured extents and report scroll-anchor correction;
- stable labels share prepared results;
- cache memory remains bounded;
- large documents can batch streaming publication and virtualize semantics and scenes;
- a historical edit during active streaming does not force tail-wide shaping or block append publication;
- a remote merge republishes only the semantically affected snapshot and preparation frontier;
- an unchanged nested document reuses its scene and intrinsic metrics when the parent changes paint or placement;
- dense derived layers do not pay per-span stable-anchor costs;
- paginated edits restart at a valid predecessor checkpoint and stop at a converged successor when one exists;
- operation cost scales with the actual affected frontier, with unavoidable downstream pagination propagation measured rather than denied.

### 27.3 Memory posture

Use:

- compact IDs and ranges;
- interning for repeated styles and fonts;
- structural sharing between snapshots;
- chunked/paged arenas where they improve locality;
- shared immutable strings and slices;
- reusable workspaces;
- topology keys instead of copied style payloads;
- explicit retention policies for offscreen scenes;
- compact flow checkpoints and coalesced virtual extents;
- separate hot-tail retention for append-heavy sessions.

Do not use tiny-object graphs where compact tables suffice.

---

## 28. Reference applications that hold the architecture accountable

Underwood should be developed against demanding reference applications, not a sequence of toy demos that validate only what already exists.

### 28.1 The spearhead: the living agent document

The flagship is not a chat transcript painted as bubbles. It is a durable workstream that accepts streaming human, agent, and tool operations and can crystallize them into an editable artifact without changing engines.

```text
Human input ─┐
Agent plans ─┼─► capability-scoped commands ─► Underwood transaction log
Tool streams ┤                                      │
Loro merges ─┘                                      ▼
                                          immutable document snapshots
                                                    │
                              ┌─────────────────────┼────────────────────┐
                              ▼                     ▼                    ▼
                       Overstory experience   TextScene deltas    archival/PDF output
                              │                     │
                              ▼                     ▼
                    native/web interaction   thin/remote clients
```

It contains:

- append-heavy conversational text with bounded retention and instant historical navigation;
- typed human/agent provenance, citations, review state, and semantic diffs;
- tool-output cards containing code, tables, images, forms, diagnostics, and nested editable documents;
- live plans and specifications that agents and people edit through the same capability-scoped commands;
- local-first collaboration through `underwood_loro`, presence, comments, and selective undo;
- folding, search, syntax, diagnostics, multiple selections, and million-line virtualization;
- keyboard, IME, clipboard, touch, accessibility, and browser-host conformance;
- renderer-neutral scenes, scene-delta streaming, archival replay, and accessible export.

The product exercises Underwood, Overstory, Understory’s general mechanisms, Imaging, observability, and retention as one wanted experience. It uses public crate contracts: no bespoke transcript model, renderer shortcut, hidden web editor, or agent mutation path is allowed to make the demo look complete.

Its defining interaction is transformation without export: a streamed exchange becomes a specification, form, report, or reviewable patch in place, while every operation remains attributable and selectively undoable. That is the product expression of “the document is an operation space.”

### 28.2 The living design specification

A collaborative product/design document containing:

- headings, lists, tables, and code;
- comments and review annotations;
- links and mentions;
- live inline Overstory controls and product references;
- remote selections;
- theme changes;
- Tagged PDF/PDF-UA export with verified reading order;
- semantic agent inspection and typed edits;
- accessibility structure.

This exercises the complete semantic/editing/presentation path.

### 28.3 The million-line technical editor

A code/log/transcript workload containing:

- a million lines;
- incremental syntax and diagnostic layers;
- multiple selections;
- folding and elision projections;
- streaming updates;
- batched publication and bounded tail retention;
- search matches;
- smooth viewport virtualization;
- estimated/measured extent disclosure and stable scroll anchoring;
- stable anchors under edits;
- low edit-to-present latency.

This prevents a beautiful book engine that cannot handle working software.

### 28.4 The multilingual compositor

A paginated document containing:

- Arabic, Indic, CJK, emoji, and mixed bidi;
- vertical text;
- ruby;
- columns;
- arbitrary exclusion shapes;
- floats and footnotes;
- spanning and fragmented tables with repeated headers;
- high-quality justification and hyphenation;
- CPU, GPU, and Tagged PDF/PDF-UA output;
- background pagination resumed from serialized flow checkpoints.

This prevents a fast code editor that cannot become a true composition system.

These applications are not separate products required before progress can occur. They are standing architectural wind tunnels with checked-in corpora and expected results.

---

## 29. Fitness tests and conformance

### 29.1 Invalidation tests

Tests should assert work not performed:

```rust
#[test]
fn changing_foreground_reuses_shape_and_flow() {
    let before = harness.prepare();
    harness.change_document_foreground(Color::BLUE);
    let after = harness.prepare();

    assert_eq!(after.stats.analysis_runs, 0);
    assert_eq!(after.stats.shape_runs, 0);
    assert_eq!(after.stats.break_runs, 0);
    assert!(after.stats.paint_updates > 0);
    assert_eq!(before.geometry_key, after.geometry_key);
}
```

### 29.2 Snapshot and position laws

- old snapshots never change;
- applying a transaction produces a new revision;
- sparse anchors transform deterministically;
- tracked authored ranges transform in bulk with the same edge semantics as the transaction;
- snapshot ranges reject revision mismatch;
- dense derived layers can map or recompute without allocating durable anchors;
- transaction inversion restores semantic equality;
- independent layer updates preserve unrelated layer identities;
- projection/source mapping round trips at valid cursor positions.

### 29.3 Differential and corpus testing

Maintain:

- Unicode and script corpora;
- browser/typography comparison cases where meaningful;
- Parley regression cases;
- editor command traces;
- IME traces from host platforms;
- pagination fixtures;
- fuzz tests for malformed transactions and ranges;
- snapshot replay tests;
- CPU/GPU/PDF visual comparisons with defined tolerances;
- selective-undo traces with remote inserts, deletes, formatting, moves, and schema changes;
- Loro convergence traces for rich-text marks, persistent cursors, tree moves, shallow snapshots, compaction, and streaming append;
- canonical-first versus Loro-authoritative publication benchmarks;
- recursive-surface traces for layout negotiation, selection boundaries, focus, accessibility, scene reuse, and coordinated undo;
- flow-checkpoint resume/convergence traces and serialization round trips;
- estimated-to-measured viewport anchoring traces;
- table constraint and fragmentation corpora;
- text-data identity and Unicode-version replay fixtures;
- Tagged PDF/PDF-UA structure, `ActualText`, ToUnicode, and reading-order validation;
- scene-delta loss/recovery, versioning, and cross-machine replay tests.

### 29.4 Platform matrix

- `no_std + alloc` checks;
- `wasm32v1-none` or the strongest practical constrained target;
- desktop Rust targets;
- WebAssembly browser integration;
- iOS/iPadOS host path;
- deterministic headless tests.

### 29.5 Host-edge conformance

The host matrix is maintained separately from the pure-engine matrix because platform success cannot be inferred from `no_std` tests:

- macOS, iOS/iPadOS, Windows, Linux, Android, and browser IME/composition traces;
- hardware and software keyboards, dead keys, candidate windows, dictation, handwriting, and replacement ranges where available;
- browser `beforeinput`, clipboard, focus, virtual keyboard, zoom, and hidden-input synchronization;
- AccessKit-to-platform bridge tests plus named assistive-technology smoke and regression runs;
- font discovery, fallback, license metadata, subsetting, embedding, and export-policy cases;
- scene-delta thin-client recovery under loss, reordering, resource eviction, and version skew.

Support claims identify which layer passed: engine, host bridge, platform API, or end-to-end assistive/input technology.

### 29.6 Proof ledger

CI publishes a machine-readable ledger for the major promises in this document:

```rust
pub struct CapabilityProof {
    pub capability: CapabilityId,
    pub status: ProofStatus,
    pub owner: Option<MaintainerId>,
    pub corpus: Box<[CorpusId]>,
    pub platforms: Box<[PlatformEvidence]>,
    pub budgets: Box<[MeasuredBudget]>,
    pub product_dependency: Option<ProductScenarioId>,
    pub last_verified: EvidenceRevision,
}
```

The ledger is not a vanity dashboard. It prevents specified surface from being confused with working surface, identifies ownerless conformance obligations, and lets product planning choose pressure without rewriting architectural truth.

---

## 30. What makes Underwood strategically exceptional

Many systems can display styled spans. Several can edit rich text. A few can paginate. Code editors can handle huge files. Browser engines can flow around shapes. Collaborative editors can merge operations. Design tools can embed objects.

### 30.1 The real competitor is the browser-shaped default

For nearly every Underwood workload, the practical incumbent is a browser or webview containing ProseMirror, Lexical, CodeMirror, Monaco, or bespoke DOM code. It is broadly available, inexpensive at the point of adoption, familiar to hiring markets, and good enough. Native platform stacks—TextKit 2, DirectWrite/RichEdit, and Android text—are deep but bounded to their platforms. Flutter’s paragraph stack and Qt Scribe cover cross-platform text. In Rust, cosmic-text, Parley/Xilem/Masonry, and specialized editor cores such as Zed’s cover important fronts without presenting this document platform. Typst demonstrates serious incremental composition but is oriented around authored/batch layout rather than a live general editing surface.

Underwood cannot win by checking more boxes on a comparison table. It must make a wanted product possible with materially better latency, determinism, recursion, semantics, accessibility, operation control, or deployment economics than the browser-shaped alternative. “Native Rust” is an implementation property, not a reason to switch.

The absence of a competitor spanning the full union is opportunity and warning. The union may be uniquely valuable, or it may be a surface everyone admires and nobody funds. The spearhead and stewardship charter exist to answer that with adoption evidence.

### 30.2 The union creates capabilities, not merely completeness

Underwood becomes exceptional by refusing to treat those as unrelated products:

1. **One semantic and transactional foundation** from labels to structured collaborative documents.
2. **Incremental preparation at every stage**, not merely cached final layouts.
3. **Retained shaping and bounded break reshaping** suitable for correct international typography.
4. **Arbitrary shape and region flow** integrated with Rust geometry rather than limited to rectangular boxes.
5. **Recursive live document surfaces** that let documents become application shells without infecting semantics with widget instances.
6. **Renderer-neutral text scenes** shared by GPU, CPU, PDF, testing, and remote presentation.
7. **Scene deltas as a versioned wire protocol** for thin clients, remote preview, and deterministic distributed preparation.
8. **Semantic accessibility and accessible PDF** rooted in the same document identities and exact geometry.
9. **Capability-scoped operations for agents and automation**, with provenance, approval, deterministic replay, and no privileged mutation path.
10. **A maintained Loro collaboration implementation and selective undo**, while preserving a concrete non-collaborative core and differential adapter boundary.
11. **Resumable pagination and honest virtual extents** spanning books and million-line editors.
12. **Versioned international-text data capabilities** suitable for full compositors and disciplined WebAssembly budgets.
13. **`no_std + alloc` core architecture** with explicit host capabilities.
14. **A calm Rust API** instead of a browser-sized public surface.

Together those enable:

- agent-native documents where human and agent operations are attributable, capability-scoped, reviewable, and selectively undoable;
- a document that is simultaneously a human artifact and a machine-queryable API;
- archival and contractual rendering with deterministic replay and byte-stable CI evidence;
- high-quality document scenes streamed to thin, embedded, low-power, and e-ink clients;
- interactive editorial publishing: print-grade flow containing live UI without shipping a browser;
- workstreams that transform from transcript to form, specification, report, or application in place.

### 30.3 The theory of victory

The living agent document carries the engine into the world. Underwood proves that experience cannot be built as coherently on incumbent stacks; Overstory makes it desirable; headless crates and adapters make the engine independently adoptable; the proof ledger makes the claims credible; the stewardship model keeps the conformance work alive.

The strategic goal is not to match a checklist called “rich text.” It is to ship a product whose operation model, recursive documents, determinism, and performance expose the limits of less coherent platforms—and then make the underlying engine calm enough for others to adopt.

---

## 31. Simplicity constraints: the beautifully designed machine

The ambition does not excuse architectural indulgence.

### 31.1 The three foundations

Return repeatedly to:

- snapshot;
- deliberate position form;
- prepared resource.

If a new abstraction cannot explain its role relative to those, it may not belong.

### 31.2 Concrete façade

Do not expose every storage, schema, layout, or cache parameter as a generic type on `Document`. Use concrete façade types and localized traits at substitution boundaries.

Bad:

```rust
Document<S, A, N, P, R, C, F, E>
```

Better:

```rust
Document
DocumentSnapshot
DocumentSession
LayoutEngine
TextScene
```

### 31.3 No speculative subsystem recreation

- Use Parley rather than writing another shaper; isolate any temporary upstream patch stack in `underwood_parley`.
- Use Fontique rather than owning platform font discovery.
- Use Kurbo rather than inventing geometry types.
- Use Peniko/Imaging rather than defining a private brush universe.
- Use `attributed_text` and `styled_text` where their contracts fit.
- Use external scheduling rather than owning a runtime.

### 31.4 No universal extension machinery

Add extensibility where a concrete second consumer demands it. Typed node schemas and annotation layers are necessary. A general plugin runtime inside the core is not.

### 31.5 No hidden mutation

Resources and snapshots do not mutate behind shared handles without publishing a revision. This is essential for invalidation, determinism, and concurrency.

### 31.6 No demo-driven architectural collapse

Do not choose `String`, one style, one rectangle, or one selection as the foundational representation because it makes the first screenshot easier. Provide optimized representations for those cases behind the final contracts.

---

## 32. Refusals and traps

Underwood should explicitly refuse:

- `RichText = String + Vec<Span>` as the public ceiling;
- byte offsets as durable document positions;
- stable anchors for every dense derived or authored span;
- an open `TextStorage` generic or trait object in document hot paths;
- one attribute layer for style, semantics, selection, diagnostics, and links;
- editor mutation that bypasses transactions;
- a widget per span;
- document semantics inferred from paint;
- actual brushes inside shaping cache identity;
- one global “layout dirty” bit;
- full document relayout after local edits;
- eager page-one-to-end pagination with no resumable flow state;
- virtual extents that conceal whether values are estimated or measured;
- a permanent dependence on `parley::Layout<Brush>`;
- renderer callbacks from document or widget code;
- presentation stores that read properties during paint;
- a private platform font-discovery path;
- ambient or unversioned Unicode, segmentation, locale, or hyphenation data;
- an engine-owned thread pool;
- a second general UI box-layout engine hidden inside document flow;
- a closed document-node enum that makes extensions second-class;
- “the CRDT handles it” as the definition of selective undo;
- a generalized collaboration trait with no maintained second implementation;
- CRDT-native versus canonical storage decided without competing product traces;
- table layout treated as ordinary block flow;
- a scene wire format made of backend drawing commands;
- agent mutation that bypasses capability checks, typed commands, or provenance;
- reentrant recursive-document layout or implicit cross-session selection/undo;
- `wasm32` compilation presented as proof of web editing, IME, or accessibility;
- an AccessKit mapping presented as proof of end-to-end assistive-technology behavior;
- generic APIs so abstract that ordinary use becomes archaeology;
- shipping a polished toy editor and declaring the architecture proven;
- treating every workstream as simultaneously active when no one owns its conformance;
- architecture prose, screenshots, or crate count used as substitutes for the proof ledger;
- assuming an engine will be adopted without a wanted product carrying it.

---

## 33. Architectural workstreams

This is not an MVP ladder, and it is not a license to run nine half-staffed queues. The workstreams are ownership maps for the complete machine. The living agent document selects the active cross-cutting capability campaign; that campaign is carried from `Specified` toward `Product-proven` across every affected crate before attention fragments again. The other wind tunnels keep the contracts honest without pretending one person can execute company-sized conformance fronts simultaneously.

### 33.1 Constitutional and repository work

- establish `forest-rs/underwood`;
- write crate fences and dependency rules;
- set `no_std + alloc` CI expectations;
- establish the four north-star reference corpora, with the living-agent corpus doubled as the product trace;
- define instrumentation and memory accounting conventions;
- publish the proof ledger and cross-repository dependency map;
- record the stewardship/adoption charter and conformance ownership;
- ratify the mandatory pre-repository ADRs in §35 before public types acquire folklore.

### 33.2 Upstream Parley alignment

- land and stabilize `styled_text`;
- design the `styled_text_parley` seam around shaping styles and paint slots;
- support retained `ShapedText`;
- expose break/concat reshaping appropriately;
- ensure inline objects and vertical text remain first-class;
- provide reusable workspaces and measurable cache keys;
- stop raw spacing magnitude from splitting shaping items;
- decide font-unit retained shaping and explicit scale exceptions;
- establish the pinned-backend and upstream-patch contingency.

### 33.3 Document and transaction foundation

- snapshot and revision model;
- typed node schemas;
- concrete canonical text store and edit summaries;
- sparse durable anchors;
- dense tracked authored ranges;
- revision-bound derived ranges;
- annotation layers;
- primitive edit algebra;
- transaction inversion and mapping;
- `underwood_loro` reference adapter and merge trace format;
- canonical-first and Loro-authoritative publication prototypes;
- document fragments.

### 33.4 Style and projection foundation

- specified/computed/used styles;
- shaping/flow/paint partitions;
- explicit analysis/itemization separation;
- topology-aware tracking and scale dependencies;
- style sheets and semantic rules;
- root environment adapter;
- projection and source maps;
- `StyledText` preparation;
- invalidation classification.

### 33.5 Layout and scene foundation

- analysis/itemization/shaping/used-value/break/flow cache graph;
- region geometry protocol;
- resumable flow checkpoints and virtual extent maps;
- block and paragraph flow;
- table constraints and fragmentation;
- bounded float/object/footnote convergence;
- inline object measurement;
- `TextScene` geometry/paint/semantic separation;
- hit and caret maps;
- renderer adapters.

### 33.6 Editing and experience foundation

- session and selection set;
- commands and typing style;
- IME composition;
- selective collaborative undo/redo;
- append batching and retention;
- maintained Loro collaboration and selective-undo integration;
- clipboard fragments;
- Overstory controls as policies;
- AccessKit mapping;
- widget-backed inline objects;
- recursive document surfaces, boundary selection, and coordinated undo.

### 33.7 Text-data and international conformance

- define `TextDataProvider`, bundle tiers, and content identities;
- integrate concrete bundles through `underwood_parley` without inner-loop dispatch;
- measure compressed and resident WebAssembly budgets;
- version Unicode, segmentation, locale tailoring, and hyphenation inputs;
- maintain multilingual segmentation, shaping, breaking, and replay corpora.

### 33.8 Agent operation substrate

- capability-scoped operation authorities and budgets;
- versioned typed-command encoding;
- proposal validation, semantic/visual diff, and approval;
- provenance and deterministic replay manifests;
- Overstory/MCP adapters that possess no privileged mutation path;
- hostile and accidental-agent-operation conformance traces.

### 33.9 Portable output and distributed scenes

- stable scene fragment/resource identities;
- versioned `TextSceneDelta` protocol and recovery checkpoints;
- thin-client and remote-preview reference consumer;
- Tagged PDF/PDF-UA exporter using semantics plus flow geometry;
- conformance, privacy, resource-budget, and cross-machine replay suites.

### 33.10 Spearhead product and host edges

- ship the living agent document entirely through public Underwood/Overstory contracts;
- stream and retain long work sessions while humans edit history and agents continue appending;
- render tool cards and nested document sessions with explicit focus, selection, accessibility, and undo boundaries;
- validate native and browser IME, clipboard, touch, focus, and assistive-technology paths;
- use semantic diffs, capability approval, provenance, collaboration, and selective undo in the product—not only fixtures;
- exercise scene-delta thin clients and deterministic archival replay;
- publish product latency, memory, accessibility, merge, and conformance evidence.

The workstreams are successful when one wanted product and the standing corpora prove a coherent architecture. An isolated miniature, a complete module with no host evidence, or nine subsystems at twenty percent does not satisfy the handover.

---

## 34. Decisions to preserve unless evidence overturns them

1. **Underwood is its own repository and platform.**
2. **Parley remains the shaping engine; any necessary pinned patch stack is isolated in `underwood_parley`, measured, and carried with an upstreaming obligation.**
3. **The semantic document is not `StyledText`.**
4. **`StyledText` is a computed, compact preparation IR.**
5. **Positions have three contracts: sparse durable anchors, dense tracked authored ranges, and revision-bound derived ranges.**
6. **Underwood’s default canonical text store is concrete; Loro-native authority is decided by measured prototype without exposing a generic `Document<S>`.**
7. **Annotations are layered by concern and declare their position strategy.**
8. **All mutation is transactional; collaborative undo is a compensating transaction that preserves remote work.**
9. **Projection and source mapping are explicit stages; IME composition has a separate epoch.**
10. **Analysis, itemization, shaping, used values, breaking, flow, geometry, paint, and semantics have distinct cache identities.**
11. **Unicode and language data are explicit, versioned capabilities included in dependent keys and replay manifests.**
12. **Flow regions are general geometry, and long flow is resumable through checkpoints and virtual extents.**
13. **Tables have a dedicated constraint and fragmentation subsystem.**
14. **Prepared output is a renderer-neutral `TextScene`; scene deltas are a versioned protocol.**
15. **Overstory controls are policies over Underwood sessions and documents, including recursive embedded sessions.**
16. **The simple label case is optimized behind the same conceptual contracts.**
17. **Accessibility begins at the semantic document and extends through Tagged PDF/PDF-UA output.**
18. **The transaction log is the capability-scoped substrate for human, collaborative, and agent operations.**
19. **The public façade remains concrete and calm.**
20. **The living agent document is the spearhead product and first product-proven conformance client.**
21. **`underwood_loro` is the maintained reference collaboration implementation; abstraction follows a second implementation.**
22. **Recursive document surfaces have explicit layout, selection, focus, accessibility, scene, and undo boundaries.**
23. **WebAssembly editing and accessibility are claimed only with end-to-end host evidence.**
24. **Every major capability reports `Specified`, `Executable`, `Measured`, `Conformant`, or `Product-proven` through a proof ledger.**
25. **Overstory is the flagship vehicle, while Underwood remains headless and independently adoptable.**
26. **Open licensing and project sustainability are separate decisions; company-grade conformance requires a stewardship model.**

---

## 35. Design records that must precede folklore

One product charter and four technical contracts are mandatory before the repository publishes foundational types. They do not postpone the mission; they prevent the first convenient implementation—or the first beautiful document—from becoming the five-year tax.

### 35.1 Mandatory before repository foundations

#### Charter-000: spearhead, proof, and stewardship

Name the living agent document as the flagship, the four reference corpora, the proof-status vocabulary, the work-in-progress discipline, the requirement to use public APIs, and the boundary between Overstory’s product and Underwood’s independently adoptable engine. The living-agent corpus is also the sustained product trace. Confirm or explicitly overturn the funded-commons/product model in §6.4 and name the initial maintenance/funding commitments. Architecture with no vehicle or conformance owner is not a complete charter.

#### ADR-0001: position and canonical storage contract

Ratify the three position forms in §8, edge/bias laws, lifecycle and resolution behavior, the concrete default text store, `EditSummary`, external-store adaptation boundary, and cost budgets. Include million-span syntax/diagnostic, dense authored-style, and Loro cursor/mark traces. Define the canonical-first versus CRDT-authoritative experiment without exposing `Document<S: TextStorage>` or choosing representation by ideology.

#### ADR-0002: resumable flow and virtual extent contract

Specify every serializable `FlowCheckpoint` field, checkpoint validity, predecessor restart, successor convergence, pagination scheduling, estimate production, measured-state publication, viewport-anchor corrections, and accessibility behavior for unrealized content. Include a long-book edit and a million-line sparse-measurement trace.

#### ADR-0003: text-data provisioning and identity

Specify the provider/bundle seam, minimal and full data tiers, Unicode and locale version identity, hyphenation ownership, data-dependent cache/replay keys, capability diagnostics, and compressed/resident WebAssembly budgets. There is no unversioned default hidden behind a feature flag.

#### ADR-0004: Parley boundary and contingency

Name the required retained analysis/itemization/shaping/break APIs, pinned revision policy, upstream contribution path, evidence and review threshold for carrying a patch, patch ownership/removal rules, and the conformance suite that prevents divergence. The ADR must preserve the ability to build Underwood’s breaker over `parley_core` primitives without leaking a fork through public APIs.

### 35.2 Mandatory before public API stabilization

- **Collaboration authority and Loro:** canonical-first, Loro-authoritative, and sealed-hybrid prototypes; cursor/anchor identity; rich-text boundary semantics; tree moves; shallow loading; compaction; selective undo; snapshot cost; `no_std` impact; and the evidence that chooses authority.
- **Tracking and scale:** exact itemization predicate for spacing, OpenType feature precedence, font-unit shaping evidence, automatic versus explicit `opsz`, hinting/device keys, and animation behavior.
- **Collaborative selective undo:** compensating-operation semantics under remote insert/delete/format/move/schema change, partial outcomes, redo, adapter capability negotiation, and trace corpus.
- **Tables:** grid validation, intrinsic sizing, span contribution, percentage and border rules, row/cell fragmentation, repeated headers, nested flow, and invalidation.
- **Flow convergence and optimization domains:** floats, exclusions, footnotes, width-dependent objects, solver budgets/fallbacks, and the boundary of Knuth–Plass-style optimization across region chains.
- **Scene protocol and tagged PDF:** stable resource/fragment identity, delta recovery/versioning, semantic reading order, `ActualText`, ToUnicode, export privacy, and conformance tooling.
- **Agent operation authority:** scope/capability algebra, approval, command versioning, provenance, deterministic replay, and host adapter boundary.
- **Recursive document surfaces:** layout negotiation, cycle/depth budgets, selection boundary policies, focus/accessibility stitching, nested scenes, copy semantics, and isolated/coordinated undo.
- **Host-edge support:** browser/native IME protocol, AccessKit realization and virtualization, support matrices, degraded modes, font rights/subsetting, and the proof required for platform claims.

### 35.3 Further records that remain real questions

#### Document and styles

- What is the minimal core content model that supports paragraphs, structured blocks, and extensions without becoming a DOM?
- How much structural persistence is useful before complexity exceeds the value of snapshot sharing?
- Which node payloads must be serializable, and how are application schemas identified?
- Does the style sheet use a deliberately small selector language, or consume a generalized Understory selector engine through an adapter?
- How are token/expression dependencies represented without coupling to Overstory?
- What is the exact rule for paint changes inside ligatures or grapheme clusters?

#### Layout, scenes, and rendering

- Which line-breaking API must Parley expose for arbitrary per-line intervals and bounded reshaping?
- How are multiple disjoint inline intervals on one visual line represented?
- Which advanced typography belongs upstream in Parley and which belongs in Underwood flow?
- What geometry belongs in `TextScene` versus backend caches?
- How are font resources named across process, serialized, and PDF contexts?

#### Editing and accessibility

- What primitive edit algebra is sufficient for extension schemas and robust inversion?
- How are typing attributes and document styles reconciled at boundaries?
- How are composition, remote edits, and local selections reconciled?
- Which editor commands require layout, and how do they operate when layout is partially virtualized?
- What semantic fragment API gives Overstory enough information without embedding AccessKit types in Underwood?
- How are huge document trees virtualized while retaining stable accessibility focus?
- How are generated projection content and source semantics exposed?

---

## 36. Initial API constellation

This sketch is intentionally cohesive rather than exhaustive.

```rust
// Construct semantic content.
let document = Document::builder()
    .section(|section| {
        section.heading(1, |text| text.push("Studio Specification"));
        section.paragraph(|text| {
            text.push("Configure the ");
            text.span(Strong, |text| text.push("Northstar Chair"));
            text.push(" using ");
            text.link(product_url, |text| text.push("this catalog"));
            text.push(".");
            text.inline_object(configuration_preview);
        });
    })
    .finish();

// Create a toolkit-independent editing session.
let mut session = DocumentSession::new(document);
session.execute(
    InsertText::new("New text"),
    &mut edit_context,
)?;

// Supply versioned host capabilities once; no ambient locale or font state.
let data = text_data_provider.load_bundle(&TextDataRequest::full())?;
let mut engine = LayoutEngine::builder()
    .text_data(data)
    .font_resolver(font_resolver)
    .build()?;

// Resolve styles and prepare layout through regions.
let request = PreparationRequest {
    snapshot: session.snapshot().clone(),
    viewport,
    regions: FlowThread::single(FlowRegion::shape(article_shape)),
    configuration,
};

let prepared = engine.prepare(request, &style_environment, &mut objects)?;

// Publish a lightweight scene reference.
presentation.text_scene = Some(prepared.scene_ref());
```

The façade does not expose Parley contexts, internal caches, or renderer types. Advanced consumers can access deliberate lower-level crates.

---

## 37. Current related work and alignment notes

As of July 2026:

- Parley’s `attributed_text` work provides generic text storage and attributed ranges.
- Parley PR #631 proposes `styled_text` with compact complete style spans and separate layout/paint interning.
- `styled_text_parley` is intended as a follow-up rather than part of the base `styled_text` PR.
- `parley_core` now contains low-level analysis/itemization/shaping work and is moving toward retained `ShapedText`.
- `parley_core::analysis` explicitly performs Unicode analysis before shaping and independently of fonts; font weight therefore never belongs in `AnalysisKey`.
- `parley_core::itemize` already exposes a caller split predicate for shaping-relevant style topology, validating itemization as a distinct retained boundary.
- Current high-level Parley shaping splits items on raw letter/word spacing and passes `font_size` through `ShapeOptions`; spacing is subsequently applied in run layout. Underwood must adapt correctly while pursuing the topology-aware spacing and font-unit-shaping ADRs above.
- Current Parley analysis uses ICU4X components and feature-gated complex-script segmentation data; Underwood’s provider contract must turn that concrete state into an explicit, versioned capability rather than assuming baked data forever.
- Loro’s current Rust repository provides a 1.x crate, Fugue text, rich-text marks with configurable expansion behavior, persistent cursors, movable trees, delta exchange, history/time travel, shallow snapshots, and collaborative undo tests. That is enough concrete surface to bless `underwood_loro` as the first collaboration conformance client.
- Loro remains an optional host-side integration rather than a foundational `no_std` dependency. Its exact cursor, mark, undo, compaction, and snapshot semantics must be tested against Underwood rather than inferred from feature names.
- Automerge remains the first differential candidate: its current core is Rust/Wasm, its marks are explicitly motivated by Peritext, and its project has a mature sync/persistence mission. Its own README currently describes the Rust API as low-level and under-documented, making it a less direct first adapter for a Rust-native product.
- Overstory’s current README describes the toolkit as early pre-release work and notes that CI is not yet present. The spearhead must therefore create product and conformance pressure without assuming Overstory’s control, host, or accessibility surfaces are already settled.
- Parley PR #634 explored reshaping across breaks, vertical writing modes, and first-class inline objects.
- Overstory currently has a plain `Text` control, a shared `Layout<Brush>` cache, and `TextInput` based on `PlainEditor`.
- `understory_presentation::TextPrimitive` currently contains `PlainTextPrimitive` and already anticipates a styled sibling.
- Overstory’s prepared-resource ticket correctly identifies the need to share text preparation between layout and paint, but should become a child of the Underwood architecture.
- Overstory’s `TextArea` roadmap should treat `TextArea` as a proving client of Underwood rather than the owner of multiline text machinery.

Relevant upstream references:

- <https://github.com/linebender/parley/pull/631>
- <https://github.com/linebender/parley/pull/634>
- <https://github.com/linebender/parley/pull/675>
- <https://github.com/linebender/parley/blob/main/parley_core/src/analysis.rs>
- <https://github.com/linebender/parley/blob/main/parley_core/src/itemize.rs>
- <https://github.com/linebender/parley/blob/main/parley/src/shape/mod.rs>
- <https://github.com/loro-dev/loro>
- <https://github.com/loro-dev/loro/blob/main/crates/loro/tests/contracts/text_richtext_advanced.rs>
- <https://github.com/loro-dev/loro/blob/main/crates/loro/tests/integration_test/undo_test.rs>
- <https://github.com/automerge/automerge>
- <https://github.com/automerge/automerge/blob/main/rust/automerge/src/marks.rs>
- <https://github.com/forest-rs/overstory/blob/main/overstory/src/text.rs>
- <https://github.com/forest-rs/overstory/blob/main/README.md>
- <https://github.com/forest-rs/overstory/blob/main/.tickets/ove-6cp5.md>
- <https://github.com/forest-rs/understory/blob/main/understory_presentation/src/primitive/text.rs>

---

## 38. Closing charge

Underwood should not be timid because text is difficult. Text is difficult whether the architecture admits it or not.

The opportunity is to build the machine once, with the right seams:

- semantics before glyphs;
- snapshots before mutation;
- the right position form before ranges escape;
- transactions before undo becomes a patchwork;
- a maintained merge implementation before collaboration becomes an abstraction;
- projection before generated content corrupts selection;
- itemization before font changes poison analysis identity;
- retained shaping before resizing becomes expensive;
- regions before rectangles become a prison;
- checkpoints before pagination becomes eager global work;
- explicit text data before Unicode versions become ambient state;
- scenes before renderers infect layout;
- scene deltas before remote presentation becomes screenshots;
- recursive boundaries before documents become application shells;
- accessibility before meaning must be inferred;
- host evidence before WebAssembly or platform support becomes marketing;
- operation authority before agents acquire a privileged side door;
- observability before invalidation becomes folklore;
- a wanted product before adoption is assumed;
- conformance ownership before specified surface expands.

With Parley becoming a stronger foundation, Kurbo and Imaging available as coherent geometry and rendering layers, Understory providing disciplined mechanisms, and Overstory providing the UI experience, this is achievable. It is not achievable by confusing architectural completeness with simultaneous implementation. The living agent document is the concentrated vehicle; the compositor, technical editor, and design specification are the wind tunnels that prevent it from narrowing the machine.

The majority of the work will be conformance: scripts, keyboards, assistive technologies, merge traces, browsers, font policy, broken files, and years of regression cases. That grind is not beneath the architecture. It is where the architecture becomes true.

The standard should be a beautifully designed machine: a small set of strong concepts, precise transformations, explicit ownership, and no magical mutation. It should be pleasant for a developer displaying a label, formidable for a team building an editor, and credible for applications that currently assume only browsers or proprietary platform frameworks can provide serious text.

Do not build a rich-text widget.

Build Underwood.
