# Design-0004: Fontique-backed font requests

- **Status:** Approved
- **Approved:** 2026-07-22 by Bruce Mitchener
- **Bead:** `und-oh0.4.3`
- **Authority:** `UNDERWOOD_HANDOVER.md` §§13.3, 13.6; Design-0002; Design-0003

## Goal

Make family, weight, width, and style real computed shaping inputs. One public
document must select bundled font resources and instances through Fontique,
exercise script-and-language-aware fallback, retain synthesis evidence, and
report the exact additional work caused by a font-request change.

## Fence

`underwood` owns the complete, validated, `no_std` font request, its identity,
and portable evidence about the resolved font instance; it explicitly does not
own font discovery, family matching, coverage matching, fallback policy,
synthesis decisions, or platform font access.

`underwood_parley` owns deterministic caller-supplied font registration and the
conversion from an Underwood request to a Fontique query. Fontique owns family,
attribute, fallback, coverage, and synthesis matching. Parley Core owns shaping
the selected instance. No Fontique or Parley engine type crosses the Underwood
facade.

## Invariants

1. Font family, weight, width, and style participate in shaping identity and
   never in Unicode-analysis identity.
2. Underwood reuses Parlance's `FontFamily`, `FontFamilyName`, `GenericFamily`,
   `FontWeight`, `FontWidth`, and `FontStyle` vocabulary. It does not implement
   a competing matcher or copy Fontique's database model.
3. Every stored family request is owned and structurally canonical: CSS source
   text is parsed at construction, empty lists and exact duplicate family
   entries are rejected, and source/single/list spellings converge. Underwood
   does not claim to canonicalize every matcher-equivalent family spelling;
   Fontique still owns family-name matching semantics.
4. Fontique queries receive the complete request plus the itemized script and
   language. The adapter performs only the cluster-coverage loop required to
   feed Parley Core.
5. Fontique's synthesized variation settings precede explicit
   `ShapingStyle::variations`; explicit coordinates therefore win for duplicate
   axes. This precedence is executable and documented.
6. The scene retains the exact selected resource, final normalized variation
   coordinates, and portable synthesis evidence. A renderer is not required to
   implement every synthetic paint effect merely because the scene records it.
7. A request change reuses unchanged Unicode analysis, reruns itemization and
   selection/shaping only for the affected paragraph, and leaves unrelated
   paragraphs reusable.
8. Missing requested families are skipped deterministically. A cluster with no
   covering primary, generic, or configured script/language fallback yields the
   existing `MissingFont` preparation error.
9. System font discovery is disabled in deterministic examples, tests, and
   benchmarks. Their results depend only on checked-in font bytes and explicit
   fallback configuration.
10. Core crates remain `no_std + alloc`, gain no dev-dependencies, and contain
    no `unsafe`.

## Options considered

### Expose Fontique request and collection types from `underwood`

This makes the resolver an architectural dependency of the document facade
and prevents another backend from consuming computed styles. Rejected.

### Build an Underwood-specific matcher over an ordered font set

This duplicates family, attribute, fallback, coverage, and synthesis behavior
that belongs in Fontique. The current ordered `FontSet` is exactly this
provisional shortcut and is replaced. Rejected.

### Parlance requests, Fontique execution in the adapter

Chosen. Parlance already supplies the backend-neutral computed vocabulary used
by Parley. Fontique remains behind `underwood_parley`, where caller-owned font
bytes become a deterministic collection and itemized runs become queries.

## Approved public direction

The constructor becomes explicit about the family request. This is an
intentional pre-stable break with all checked-in callers migrated together:

```rust
pub use parlance::{
    FontFamily, FontFamilyName, FontStyle, FontWeight, FontWidth, GenericFamily,
};

let body = ShapingStyle::new(FontFamily::named("Roboto Flex"), 16.0)?
    .with_font_weight(FontWeight::new(450.0))?
    .with_font_width(FontWidth::from_ratio(0.9))?
    .with_font_style(FontStyle::Normal)?;
```

`ShapingStyle` exposes read-only accessors and validated consuming setters for
the request fields. `FontFamily::Source` is accepted as authoring convenience
but parsed once at construction or replacement; the adapter never parses CSS.

`FontSet` changes from an ordered coverage list into a deterministic Fontique
catalog. `try_from_fonts` registers caller-supplied memory fonts with system
fonts disabled. Explicit builder operations configure named families for each
generic family and each `(script, optional language)` fallback key. Unknown
configuration names fail at catalog construction rather than disappearing
during shaping.

The adapter returns portable `FontSynthesis` evidence with every prepared and
scene run:

```rust
pub struct FontSynthesis;

impl FontSynthesis {
    pub fn variations(&self) -> &[FontVariation];
    pub fn embolden(&self) -> bool;
    pub fn skew_degrees(&self) -> Option<f32>;
}
```

This preserves Fontique's chosen variable-axis settings and renderer-facing
embolden/skew suggestions without exposing Fontique's `Synthesis` type.
Normalized coordinates remain separately available as proof of the settings
actually consumed by shaping. Absent synthesis is represented without an
allocation; non-empty evidence is shared across the fragments of a run.

## Selection and shaping sequence

```text
computed ShapingStyle
  -> paragraph-local shaping table
  -> underwood_parley Fontique Query
       families + weight + width + style
       item script + optional language fallback key
  -> selected blob/index + Fontique synthesis
  -> Parley Core FontInstance + ShapeOptions
       synthesized variations, then explicit variations
  -> prepared run
       exact FontData + synthesis + normalized coordinates
  -> TextScene fragment
```

Font selection is observable work. `PreparationWork` reports selected clusters,
and `WorkReport::font_selection` exposes a separate stage rather than hiding this new
cost in shaping. A cache hit reports no selection work.

## Executable proof

The change is not complete until the public path proves all of the following:

- named-family requests select both checked-in Roboto Flex and Noto Kufi Arabic
  resources in one document;
- weight and width requests select distinct Roboto Flex instances through
  Fontique synthesis;
- an explicit axis coordinate overrides Fontique's synthesized setting for the
  same axis;
- Arabic text can request a Latin primary family and reach Noto Kufi through a
  configured `Arab`/`ar` fallback;
- an unsupported oblique request on a static face retains synthetic-skew scene
  evidence, and the CPU proof renders the supported transform;
- a font-request-only edit reuses Unicode analysis while reporting selection,
  itemization, and shaping for the affected paragraph;
- a missing-family/no-covering-fallback case fails deterministically; and
- the headless example, benchmark, rustdoc, and visual proof all use the same
  public workflow.

## Deferred work

- Platform/system font discovery and live collection updates require their own
  determinism and invalidation contract.
- Font generation identity and external resource lifetime remain future scene
  resource concerns.
- Device-specific embolden fidelity is a renderer capability; the current CPU
  proof must not imply support if its released backend ignores that field.
- Font fallback policy beyond explicitly configured deterministic lists belongs
  with application/platform integration, not the Underwood core.

## Validation

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo check -p underwood --target x86_64-unknown-none
cargo check --workspace --all-targets --all-features --locked
cargo xtask proof
cargo xtask policy
cargo xtask text
bd doctor
bd export -o .beads/issues.jsonl
git diff --check
```
