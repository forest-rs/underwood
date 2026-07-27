# Underwood / Parley retained comparison — 2026-07-27

## Verdict

The current retained stack passes its asymptotic repeat and query claims, but
fails the memory, edit-latency, allocation, and churn comparison badly enough
to block completion of `und-oh0.13.17`.

This is not a claim that Underwood and Parley expose identical products.
Underwood additionally retains revision-safe source mapping, capability
facades, multi-selection/navigation topology, IME-facing geometry, immutable
publication, and region-aware scene data. The comparison asks whether those
capabilities earn their measured price. The answer for the current
representation is no.

Design-0021 and `und-oh0.13.17.11` own the blocking correction.

## Reproduction

The checked-in `benches/residency-compare` crate uses the exact high-level
Parley revision already pinned by the workspace. Both engines receive:

- embedded Roboto Flex and Noto Kufi Arabic;
- an embedded-only Fontique catalog and the same Arabic fallback;
- the same four-item Latin/Arabic corpus;
- 15-unit text at a 180-unit wrapping width;
- the same source strings and 64/1,000 retained-item scales.

Build and run:

```sh
cargo build --release -p underwood_residency_compare
benches/residency-compare/profile-timing.sh \
    target/release/underwood_residency_compare 21
benches/residency-compare/profile-live-memory.sh \
    target/release/underwood_residency_compare 3
benches/residency-compare/profile-allocations.sh \
    target/release/underwood_residency_compare
```

The measurements below were recorded on macOS 26.5.2, arm64. Timing rows are
21-sample medians. Live heap is the exact `heap` observation and was stable
across all three samples; process footprint varied slightly and is not
substituted for heap bytes. Allocation rows are differences between independent
`malloc_history -allEvents` processes and therefore retain small cross-process
runtime noise.

Font blob bytes are 1,788,992 in both engines. Source bytes are 27,250 at the
1,000-item scale. Underwood's font baseline is 1,820,512 live heap bytes;
Parley's is 1,818,080.

## Timing

Nanoseconds are per represented item for repeat/churn and per operation for
edit/query:

| Operation | Scale | Underwood median | Parley median | Underwood range |
|---|---:|---:|---:|---:|
| exact repeat | 64 | 125 | 192 | 121–128 |
| exact repeat | 1,000 | 165 | 189 | 161–173 |
| localized edit, default adapter budget | 64 | 22,679 | 3,045 | 22,027–23,461 |
| localized edit, default adapter budget | 1,000 | 22,520 | 3,026 | 22,260–23,463 |
| localized edit, warm adapter facts | 64 | 22,103 | — | 21,803–26,534 |
| localized edit, warm adapter facts | 1,000 | 22,213 | — | 21,882–22,777 |
| cold churn | 64 | 18,708 | 6,716 | 17,798–20,885 |
| cold churn | 1,000 | 14,261 | 4,842 | 13,893–14,742 |
| exact hit | 64 units | 46 | 13 | 41–55 |
| closest hit | 64 units | 60 | 17 | 52–65 |
| byte position | 64 units | 27 | 14 | 26–37 |
| exact hit | 1,000 units | 61 | 152 | 55–71 |
| closest hit | 1,000 units | 77 | 204 | 65–82 |
| byte position | 1,000 units | 38 | 176 | 37–40 |

### What passes

- Exact repeat is allocation-free in the matched profiler and its time remains
  flat per item.
- Localized edit time is flat from 64 to 1,000 unchanged siblings. The
  persistent document/scene structures therefore earn the O(change) law.
- The line-local hit correction changes the 1,000-unit queries from linear
  scans to indexed lookups. Underwood is faster than Parley at that scale.

### What fails

- A fixed 22.5-microsecond edit is about 7.4× the matched Parley rebuild.
- Cold creation is 2.8–2.9× slower.
- Keeping adapter facts warm does not materially improve the text-edit path.
  Those facts serve reflow/style/capability reuse and must be judged on those
  workloads rather than credited to typing.
- Underwood's smaller 64-unit query has a higher constant factor. The
  1,000-unit scaling result is strong, but the compact redesign must not add
  more query ceremony.

## Live retained heap

Values below subtract the corresponding engine's font baseline:

| Retained shape | Scale | Underwood | Parley comparison | Ratio |
|---|---:|---:|---:|---:|
| display labels / ordinary layouts | 64 | 566,976 B | 263,872 B | 2.15× |
| editable labels, adapter facts evicted / ordinary layouts | 64 | 1,352,384 B | 263,872 B | 5.13× |
| editable labels, warm adapter facts / ordinary layouts | 64 | 2,727,616 B | 263,872 B | 10.34× |
| display labels / ordinary layouts | 1,000 | 7,994,272 B | 3,378,240 B | 2.37× |
| editable labels, adapter facts evicted / ordinary layouts | 1,000 | 20,266,272 B | 3,378,240 B | 6.00× |
| editable labels, warm adapter facts / ordinary layouts | 1,000 | 41,973,152 B | 3,378,240 B | 12.42× |
| display document / one flat layout | 1,000 | 5,739,088 B | 2,874,240 B | 2.00× |
| one editor + 999 display siblings, adapter facts evicted / flat layout | 1,000 | 5,746,272 B | 2,874,240 B | 2.00× |
| one editor + 999 display siblings, warm adapter facts / flat layout | 1,000 | 21,225,696 B | 2,874,240 B | 7.38× |
| churn with 64 retained / churn with 64 retained | 1,000 created | 646,832 B | 291,008 B | 2.22× |

The sparse scene capability policy works: adding one editable paragraph to a
1,000-paragraph display document costs only 7,184 live bytes when adapter
facts are evicted. The 15.5 MB jump in the warm case is adapter-wide reusable
formation state, not accidental promotion of scene sidecars.

That distinction is useful but not exculpatory:

- the default editable-label scene is still 6× Parley;
- warm adapter state doubles that already excessive result;
- display and bounded churn are both more than 2× Parley.

Underwood's deterministic accounting reports these 1,000-label categories:

| Category | Display | Editable | Editable + warm adapter |
|---|---:|---:|---:|
| scene handle including spine | 2,558,000 B | 13,766,000 B | 13,766,000 B |
| engine scene-cache charge | 3,254,250 B | 14,462,250 B | 14,462,250 B |
| layout | 1,326,000 B | 1,326,000 B | 1,326,000 B |
| paint | 984,000 B | 984,000 B | 984,000 B |
| sources | 0 B | 320,000 B | 320,000 B |
| hit testing | 0 B | 3,720,000 B | 3,720,000 B |
| selection | 0 B | 2,240,000 B | 2,240,000 B |
| navigation | 0 B | 4,928,000 B | 4,928,000 B |
| adapter facts | 0 B | 0 B | 17,125,750 B |

Scene handles and cache charges share paragraph segments and must not be added
as if they were independent allocations. The category table instead explains
the retained shape: editable interaction is 10.9 MB of repeated hit, caret,
and movement observations, while the optional adapter form is another
17.1 MB.

## Allocation events

These rows report requested allocation events over the whole cold construction
or one changed paragraph. They are not live heap:

| Scenario | Scale | Underwood calls / bytes | Parley calls / bytes |
|---|---:|---:|---:|
| cold display labels | 64 | 6,934 / 1,537,612 | 863 / 281,962 |
| cold editable labels, adapter facts evicted | 64 | 8,646 / 2,920,269 | 863 / 281,962 |
| cold editable labels, warm adapter facts | 64 | 8,655 / 3,002,090 | 863 / 281,962 |
| one edit, adapter facts evicted | 64 | 136 / 47,040 | 3 / 1,147 |
| one edit, warm adapter facts | 64 | 132 / 45,104 | 3 / 1,147 |
| cold display labels | 1,000 | 105,753 / 22,960,448 | 10,691 / 3,390,654 |
| cold editable labels, adapter facts evicted | 1,000 | 132,505–132,506 / 44,568,545–44,572,897 | 10,691 / 3,390,654 |
| cold editable labels, warm adapter facts | 1,000 | 132,667–132,669 / 46,041,174–46,041,270 | 10,691 / 3,390,654 |
| one edit, adapter facts evicted | 1,000 | 135 / 41,536–45,760 | 3 / 1,147 |
| one edit, warm adapter facts | 1,000 | 130–135 / 47,696–55,216 | 3 / 1,147 |

The edit call count is flat with sibling count and therefore satisfies
O(change). It is still roughly 43× Parley's allocation-call count.

Exact-repeat and immutable-query subtractions range from zero to two calls in
independent processes, including occasional negative byte deltas. The focused
same-path profiler records no operation-owned allocation; this comparison
therefore labels the residual as cross-process runtime noise rather than
silently clamping it to zero.

`malloc_history -allByCount` on the warm 1,000-editable-label process gives
direct attribution:

- 6,272,000 bytes in adapter cursor-movement vector growth;
- 5,376,000 bytes in the corresponding scene geometry collection;
- 2,300,000 bytes in adapter glyph lowering;
- 2,816,000, 2,688,000, 1,184,000, and 1,088,000-byte scene geometry
  allocation classes;
- thousands of per-line, per-run, sidecar owner, source-map, spine, and
  one-paragraph document allocations.

The first two entries are the clearest duplication: the adapter materializes a
complete source-aware cursor graph and the scene materializes another complete
source-aware movement form.

## Required correction

The evidence selects structural deletion over incremental tuning:

1. one flat portable paragraph artifact shared directly by adapter output and
   scene traversal;
2. indexed position/unit/edge topology instead of repeated hit/caret/movement
   records;
3. no final `PreparedParagraph` retained in optional adapter formation state;
4. core-owned paint binding over adapter-supplied source coverage;
5. a compact single-paragraph `TextBlock` source state feeding the same engine;
6. one validation at the public trust boundary plus generation-keyed
   preflight;
7. engine-owned scratch only after duplicate persistent forms are removed.

Design-0021 sets blocking targets of at most 1.5× Parley display residency,
2× Parley default editable residency and edit latency, 16 allocation calls,
and 8 KiB requested bytes per edit. The current epic is not complete until
those gates pass or a separately approved design replaces them.

## Design-0021 progress checkpoint

The baseline above records the problem that selected Design-0021. The compact
artifact work subsequently removed duplicate movement forms, per-glyph paint
owners, deep scene publication, inline diagnostic payloads, and the deep
formation key.

The current exact 1,000-label live-heap deltas are 3,611,328 bytes for
display-only Underwood and 4,035,328 bytes for editable Underwood, against
3,378,240 bytes for Parley: 1.07× and 1.19× respectively. The ordinary
residency gates now pass with room to spare. Optional warm adapter retention
is still 4.07× and remains disabled by default.

The epic is not complete: localized edit is still about 3.2× Parley and makes
88 allocation calls, so it misses both the 2× latency gate and the 16-call
gate. This checkpoint supersedes the baseline ratios for current-state
decisions without rewriting the historical measurements that motivated the
design.

## Non-claims

- Parley's private vector capacities are unavailable; the harness does not
  invent deterministic Parley byte accounting.
- Process footprint is not presented as live heap.
- Allocator-requested bytes are not presented as retained memory.
- The current query win does not imply every Underwood interaction operation
  is faster; visual/logical movement and selection need to join the next
  matrix.
- The comparison does not measure vertical writing, pixel snapping, inline
  boxes, or renderer residency.
