# Visual-proof Lynx and Rook review — 2026-07-21

- **Scope:** `examples/visual-proof`, its CPU snapshot, and repository claims
- **Review modes:** Lynx adversarial correctness; Rook real-versus-mirage audit
- **Snapshot:** 1600 × 1000 RGBA8, PNG SHA-256
  `a523c21aa890dfb575fa545ee53e9f2a2a9822583a129eeb283f8dd15863e51b`
- **Unsafe watch:** no `unsafe` in Underwood-owned Rust
- **Remote gate:** GitHub Actions run `29825978976`; all eight jobs passed,
  including exact snapshot reproduction on Linux, macOS, and Windows
- **Result:** all review findings are resolved; the final three-OS matrix earns
  the deterministic CPU snapshot claim

## Lynx review

### Summary judgment

The example is a real external integration consumer. It uses public Underwood
document, adapter, scene, source, semantic, hit, caret, and work-report APIs;
records imaging commands; and asks `imaging_vello_cpu` for the committed pixels.
Renderer and PNG dependencies remain outside both production crates.

### Must — resolved

1. **The fallback label initially inferred too much from script and bidi data.**
   The proof now compares the selected fragment's exact font bytes with the
   bundled Noto Kufi fixture before displaying the fallback claim. The same
   check binds the Latin hero to Roboto Flex.
2. **“Two source clips” initially counted only repeated glyph identity and
   position.** The evidence now also requires different paint slots, different
   snapshot source ranges, and different clip rectangles for the repeated
   glyph observation.
3. **The imaging boundary narrowed scene coordinates from `f64` to `f32`
   silently.** A documented adapter check now rejects non-finite or out-of-range
   values before the unavoidable backend conversion.
4. **A pretty PNG alone would not be a regression test.** The crate reruns the
   full public path, checks the semantic evidence, decodes the committed PNG,
   and requires exact RGBA equality.
5. **The original diagnostic caret escaped its evidence region and crossed the
   poster header.** The proof now intersects caret geometry with the focused
   split-ligature clip. It outlines only the two fragments that share the glyph,
   using a distinct violet diagnostic color instead of competing paint colors.
6. **An isolated Arabic line showed RTL shaping but not mixed-direction
   behavior.** One paragraph now places an Arabic RTL run between Latin LTR
   runs and asserts even and odd bidi levels plus both exact font resources.
7. **Unexplained ornament and generic slogans consumed space without adding
   evidence.** The decorative target was removed. The slogans were replaced by
   `ffi`, `fi`, `ff`, and `fl` specimens, each required to shape from one source
   token to exactly one glyph before the evidence caption is constructed.

Good catch: the font-resource comparison turns “real fallback” from a plausible
caption into an executable fact.

### Should

- Keep the adapter example-local until more consumers establish a reusable
  renderer contract and fallible error surface.
- Keep exact snapshot equality in every supported host job so any future
  cross-platform drift fails at the renderer boundary.

### Could

- Add more scripts only when each one tests a named fallback or shaping risk;
  visual variety alone is not conformance evidence.
- Add a deliberately corrupted snapshot test if the acceptance workflow later
  grows more complex than a direct exact comparison.

### Suggested tests

- Exact RGBA comparison with the committed CPU snapshot.
- Distinct source, paint, and clip evidence for one shared ligature glyph.
- Exact selected-font resource for Latin and Arabic fragments.
- Even Latin and odd Arabic bidi observations in one mixed-direction paragraph.
- One-glyph substitution for each displayed default ligature specimen.
- Local-edit reshaping, sibling reuse, and paint-only negative-work assertions.

## Rook audit

### Mirage risks

- **Mirage:** this is not a production renderer package. It is one deliberately
  local adapter in an unpublished example crate.
- **Mirage:** the poster's labels are independently shaped Underwood scenes;
  they do not imply that the first-slice style system supports a full poster
  document with heterogeneous font sizes.
- **Mirage:** one Latin/Arabic snapshot is executable evidence, not renderer or
  international-text conformance.
- **Mirage:** the Roboto Flex resource is genuinely variable, but this first
  public style path cannot select variation axes or toggle OpenType features.
  The poster claims only the default ligature substitutions it executes.

### Real strengths

- **Real:** all visible typography is made from glyph IDs and font instances
  produced by the checked-in Underwood-to-Parley path.
- **Real:** the split `ffi` color boundary is rendered from two clips over one
  shared shaped glyph without reshaping at the style boundary.
- **Real:** the mixed-direction line is guarded by exact Latin and Arabic font
  resources, script tags, and even/odd bidi-level checks.
- **Real:** each displayed default ligature token is asserted to produce one
  shaped glyph through the same public path.
- **Real:** the displayed `1 / 1 / 0` work story is formatted from the actual
  edit, retained-sibling, and paint-only `WorkReport` values.
- **Real:** the snapshot test compares rendered pixels, not a synthetic scene,
  command count, or non-empty buffer.

### Most dangerous gap

The original high-consequence uncertainty was exact CPU pixel identity across
operating systems. GitHub Actions run `29825978976` closed that gap for the
final specimen-driven image on Linux, macOS, and Windows. The remaining limit
is scope: one poster proves this path and these fonts, not general renderer
conformance.
