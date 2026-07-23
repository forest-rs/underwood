// Copyright 2026 the Underwood Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Private retained-Parley adapter and conformance wind tunnel.

use std::fs;
use std::io;
use std::ops::Range;
use std::path::PathBuf;

use fontique::{Blob, Synthesis};
use parlance::{FontFeature, FontVariation};
use parley_core::{
    Analysis, AnalysisOptions, Analyzer, Boundary, FontInstance, ShapeOptions, ShapedText, Shaper,
    shape::ClusterData,
};

const PARLEY_REVISION: &str = "44d155e17a6dbf455c8b9133c2ae40955c9f2af2";
const CORPUS: &str = "office affinity — مرحبا بالعالم";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FontChoice {
    Latin,
    Arabic,
}

#[derive(Clone, Debug)]
struct FontAsset {
    choice: FontChoice,
    source_name: &'static str,
    source_digest: u64,
    instance: FontInstance,
}

#[derive(Clone, Debug)]
struct FontSet {
    latin: FontAsset,
    arabic: FontAsset,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ItemRecord {
    byte_range: Range<usize>,
    char_range: Range<usize>,
    bidi_level: u8,
    script: [u8; 4],
    font: FontChoice,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct GlyphRecord {
    id: u32,
    cluster: usize,
    source_bytes: Range<usize>,
    source_chars: Range<usize>,
    advance_bits: u32,
    x_offset_bits: u32,
    y_offset_bits: u32,
    paint_slots: Vec<u16>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct RunRecord {
    byte_range: Range<usize>,
    char_range: Range<usize>,
    bidi_level: u8,
    script: [u8; 4],
    font: FontChoice,
    font_digest: u64,
    normalized_coords: Vec<i16>,
    glyphs: Vec<GlyphRecord>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct PreparedObservation {
    analysis_digest: u64,
    item_digest: u64,
    physics_digest: u64,
    slot_digest: u64,
    items: Vec<ItemRecord>,
    runs: Vec<RunRecord>,
}

#[derive(Clone, Copy, Debug)]
struct ShapeConfig<'a> {
    font_size: f32,
    features: &'a [FontFeature],
    variations: &'a [FontVariation],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CurrentGap {
    VerticalShaping,
    CoreInlineObjects,
    TextDataIdentity,
}

fn current_gaps() -> &'static [CurrentGap] {
    &[
        CurrentGap::VerticalShaping,
        CurrentGap::CoreInlineObjects,
        CurrentGap::TextDataIdentity,
    ]
}

fn load_fonts() -> io::Result<FontSet> {
    Ok(FontSet {
        latin: load_font(
            FontChoice::Latin,
            "RobotoFlex-VariableFont.ttf",
            "RobotoFlex-VariableFont.ttf",
        )?,
        arabic: load_font(
            FontChoice::Arabic,
            "NotoKufiArabic-Regular.otf",
            "NotoKufiArabic-Regular.otf",
        )?,
    })
}

fn load_font(
    choice: FontChoice,
    file_name: &'static str,
    source_name: &'static str,
) -> io::Result<FontAsset> {
    let path = find_font(file_name)?;
    let bytes = fs::read(path)?;
    let source_digest = digest_bytes(&bytes);
    Ok(FontAsset {
        choice,
        source_name,
        source_digest,
        instance: FontInstance {
            font: parley_core::FontData::new(Blob::from(bytes), 0),
            synthesis: Synthesis::default(),
        },
    })
}

fn find_font(file_name: &str) -> io::Result<PathBuf> {
    for directory in parley_dev::font_dirs() {
        let candidate = directory.join(file_name);
        if candidate.is_file() {
            return Ok(candidate);
        }
    }
    Err(io::Error::new(
        io::ErrorKind::NotFound,
        format!("pinned parley_dev font `{file_name}` is missing"),
    ))
}

fn prepare(
    text: &str,
    fonts: &FontSet,
    paint_slots: &[u16],
    config: ShapeConfig<'_>,
) -> PreparedObservation {
    let char_count = text.chars().count();
    assert_eq!(
        paint_slots.len(),
        char_count,
        "paint slots must cover every source character"
    );

    let mut analyzer = Analyzer::new();
    let mut analysis = Analysis::new();
    analyzer.analyze(
        text,
        &AnalysisOptions {
            word_break: &[],
            line_break_override: None,
        },
        &mut analysis,
    );
    let analysis_digest = digest_analysis(&analysis);
    let mut shaper = Shaper::default();
    let mut shaped_text = ShapedText::new();
    let mut items = Vec::new();
    let mut runs = Vec::new();

    for item in analysis.itemize(text, |_| false) {
        let script = item.script.to_bytes();
        let font = if script == *b"Arab" {
            &fonts.arabic
        } else {
            &fonts.latin
        };
        items.push(ItemRecord {
            byte_range: item.range.byte_range.clone(),
            char_range: item.range.char_range.clone(),
            bidi_level: item.bidi_level,
            script,
            font: font.choice,
        });

        let appended = shaper.shape_item(
            text,
            &analysis,
            &item,
            &ShapeOptions {
                font_size: config.font_size,
                language: None,
                features: config.features,
                variations: config.variations,
                char_style_indices: paint_slots,
            },
            |_| Some(font.instance.clone()),
            &mut shaped_text,
        );
        runs.extend(
            appended.map(|run_index| copy_run(&shaped_text, run_index, script, font, paint_slots)),
        );
    }

    let item_digest = digest_items(&items);
    let physics_digest = digest_physics(&runs);
    let slot_digest = digest_slots(&runs);
    PreparedObservation {
        analysis_digest,
        item_digest,
        physics_digest,
        slot_digest,
        items,
        runs,
    }
}

fn copy_run(
    shaped_text: &ShapedText,
    run_index: usize,
    script: [u8; 4],
    font: &FontAsset,
    paint_slots: &[u16],
) -> RunRecord {
    let run = &shaped_text.runs()[run_index];
    assert_eq!(
        shaped_text.fonts()[run.font_index],
        font.instance,
        "retained run must name the exact selected font instance"
    );
    let clusters = &shaped_text.clusters()[run.clusters_range.clone()];
    let mut glyphs = Vec::with_capacity(run.glyphs_range.len());
    let mut retain_cluster = |index: usize| {
        let cluster = &clusters[index];
        if cluster.is_ligature_component() {
            return;
        }
        let (source_bytes, source_chars) = cluster_source(run, clusters, index);
        let mut slots: Vec<u16> = paint_slots[source_chars.clone()].to_vec();
        slots.sort_unstable();
        slots.dedup();
        if cluster.glyph_len == u8::MAX {
            glyphs.push(GlyphRecord {
                id: cluster.glyph_offset,
                cluster: source_chars.start,
                source_bytes,
                source_chars,
                advance_bits: cluster.advance.to_bits(),
                x_offset_bits: 0.0_f32.to_bits(),
                y_offset_bits: 0.0_f32.to_bits(),
                paint_slots: slots,
            });
            return;
        }
        let glyph_start = run.glyphs_range.start + cluster.glyph_offset as usize;
        for glyph in
            &shaped_text.glyphs()[glyph_start..glyph_start + usize::from(cluster.glyph_len)]
        {
            glyphs.push(GlyphRecord {
                id: glyph.id,
                cluster: source_chars.start,
                source_bytes: source_bytes.clone(),
                source_chars: source_chars.clone(),
                advance_bits: glyph.advance.to_bits(),
                x_offset_bits: glyph.x.to_bits(),
                y_offset_bits: glyph.y.to_bits(),
                paint_slots: slots.clone(),
            });
        }
    };
    if run.bidi_level & 1 == 1 {
        for index in (0..clusters.len()).rev() {
            retain_cluster(index);
        }
    } else {
        for index in 0..clusters.len() {
            retain_cluster(index);
        }
    }

    RunRecord {
        byte_range: run.range.byte_range.clone(),
        char_range: run.range.char_range.clone(),
        bidi_level: run.bidi_level,
        script,
        font: font.choice,
        font_digest: font.source_digest,
        normalized_coords: shaped_text.normalized_coords()[run.normalized_coords_range.clone()]
            .iter()
            .map(|coord| coord.to_bits())
            .collect(),
        glyphs,
    }
}

fn cluster_source(
    run: &parley_core::ShapedRun,
    clusters: &[ClusterData],
    index: usize,
) -> (Range<usize>, Range<usize>) {
    let cluster = &clusters[index];
    let mut byte_start = run.range.byte_range.start + usize::from(cluster.text_offset);
    let mut byte_end = byte_start + usize::from(cluster.text_len);
    let mut char_start = run.range.char_range.start + index;
    let mut char_end = char_start + 1;
    if cluster.is_ligature_start() {
        if run.bidi_level & 1 == 1 {
            for (component_index, component) in clusters[..index].iter().enumerate().rev() {
                if !component.is_ligature_component() {
                    break;
                }
                let start = run.range.byte_range.start + usize::from(component.text_offset);
                assert_eq!(
                    start + usize::from(component.text_len),
                    byte_start,
                    "RTL ligature components must be source adjacent"
                );
                byte_start = start;
                char_start = run.range.char_range.start + component_index;
            }
        } else {
            for (component_index, component) in clusters.iter().enumerate().skip(index + 1) {
                if !component.is_ligature_component() {
                    break;
                }
                let start = run.range.byte_range.start + usize::from(component.text_offset);
                assert_eq!(
                    start, byte_end,
                    "LTR ligature components must be source adjacent"
                );
                byte_end += usize::from(component.text_len);
                char_end = run.range.char_range.start + component_index + 1;
            }
        }
    }
    (byte_start..byte_end, char_start..char_end)
}

fn digest_analysis(analysis: &Analysis) -> u64 {
    let mut digest = Digest::new();
    digest.usize(analysis.char_info().len());
    digest.u8(analysis.paragraph_level());
    for info in analysis.char_info() {
        digest.u8(match info.boundary {
            Boundary::None => 0,
            Boundary::Word => 1,
            Boundary::Line => 2,
            Boundary::Mandatory => 3,
        });
        digest.u8(u8::from(info.is_emoji_or_pictograph()));
        digest.u8(u8::from(info.force_normalize()));
        digest.u8(u8::from(info.contributes_to_shaping()));
    }
    for level in analysis.bidi_levels() {
        digest.u8(*level);
    }
    digest.finish()
}

fn digest_items(items: &[ItemRecord]) -> u64 {
    let mut digest = Digest::new();
    for item in items {
        digest.range(&item.byte_range);
        digest.range(&item.char_range);
        digest.u8(item.bidi_level);
        digest.bytes(&item.script);
        digest.u8(font_choice_byte(item.font));
    }
    digest.finish()
}

fn digest_physics(runs: &[RunRecord]) -> u64 {
    let mut digest = Digest::new();
    for run in runs {
        digest.range(&run.byte_range);
        digest.range(&run.char_range);
        digest.u8(run.bidi_level);
        digest.bytes(&run.script);
        digest.u8(font_choice_byte(run.font));
        digest.u64(run.font_digest);
        for coord in &run.normalized_coords {
            digest.bytes(&coord.to_le_bytes());
        }
        for glyph in &run.glyphs {
            digest.u32(glyph.id);
            digest.usize(glyph.cluster);
            digest.range(&glyph.source_bytes);
            digest.range(&glyph.source_chars);
            digest.u32(glyph.advance_bits);
            digest.u32(glyph.x_offset_bits);
            digest.u32(glyph.y_offset_bits);
        }
    }
    digest.finish()
}

fn digest_slots(runs: &[RunRecord]) -> u64 {
    let mut digest = Digest::new();
    for glyph in runs.iter().flat_map(|run| &run.glyphs) {
        digest.range(&glyph.source_chars);
        for slot in &glyph.paint_slots {
            digest.bytes(&slot.to_le_bytes());
        }
    }
    digest.finish()
}

fn lower_paint(prepared: &PreparedObservation, paint_values: &[u32]) -> Option<u64> {
    let mut digest = Digest::new();
    for glyph in prepared.runs.iter().flat_map(|run| &run.glyphs) {
        digest.u32(glyph.id);
        for slot in &glyph.paint_slots {
            digest.u32(*paint_values.get(usize::from(*slot))?);
        }
    }
    Some(digest.finish())
}

const fn font_choice_byte(choice: FontChoice) -> u8 {
    match choice {
        FontChoice::Latin => 0,
        FontChoice::Arabic => 1,
    }
}

fn digest_bytes(bytes: &[u8]) -> u64 {
    let mut digest = Digest::new();
    digest.bytes(bytes);
    digest.finish()
}

#[derive(Clone, Copy, Debug)]
struct Digest(u64);

impl Digest {
    const fn new() -> Self {
        Self(0xcbf2_9ce4_8422_2325)
    }

    fn bytes(&mut self, bytes: &[u8]) {
        for byte in bytes {
            self.0 = (self.0 ^ u64::from(*byte)).wrapping_mul(0x0000_0100_0000_01b3);
        }
    }

    fn u8(&mut self, value: u8) {
        self.bytes(&[value]);
    }

    fn u32(&mut self, value: u32) {
        self.bytes(&value.to_le_bytes());
    }

    fn u64(&mut self, value: u64) {
        self.bytes(&value.to_le_bytes());
    }

    fn usize(&mut self, value: usize) {
        self.u64(u64::try_from(value).expect("trace values must fit u64"));
    }

    fn range(&mut self, value: &Range<usize>) {
        self.usize(value.start);
        self.usize(value.end);
    }

    const fn finish(self) -> u64 {
        self.0
    }
}

fn default_slots(text: &str) -> Vec<u16> {
    vec![0; text.chars().count()]
}

fn slots_with_paint_boundaries(text: &str) -> Vec<u16> {
    let mut slots = default_slots(text);
    let latin_i = text
        .chars()
        .position(|ch| ch == 'i')
        .expect("corpus must contain an fi ligature candidate");
    slots[latin_i] = 1;
    let arabic_start = text
        .chars()
        .position(|ch| ch == 'م')
        .expect("corpus must contain Arabic");
    slots[arabic_start + 1] = 2;
    slots
}

fn config<'a>(features: &'a [FontFeature], variations: &'a [FontVariation]) -> ShapeConfig<'a> {
    ShapeConfig {
        font_size: 16.0,
        features,
        variations,
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let fonts = load_fonts()?;
    let slots = slots_with_paint_boundaries(CORPUS);
    let prepared = prepare(CORPUS, &fonts, &slots, config(&[], &[]));
    let paint_digest = lower_paint(&prepared, &[0xff00_00ff, 0x00ff_00ff, 0x0000_ffff])
        .expect("all benchmark paint slots must resolve");
    let glyph_count: usize = prepared.runs.iter().map(|run| run.glyphs.len()).sum();

    println!(
        "parley={} analysis={:016x} items={:016x} physics={:016x} slots={:016x} paint={:016x} items_count={} runs={} glyphs={} gaps={}",
        PARLEY_REVISION,
        prepared.analysis_digest,
        prepared.item_digest,
        prepared.physics_digest,
        prepared.slot_digest,
        paint_digest,
        prepared.items.len(),
        prepared.runs.len(),
        glyph_count,
        current_gaps().len(),
    );
    println!(
        "fonts={}:{:016x},{}:{:016x}",
        fonts.latin.source_name,
        fonts.latin.source_digest,
        fonts.arabic.source_name,
        fonts.arabic.source_digest,
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use fontique::{Blob, Collection, CollectionOptions, SourceCache};
    use parlance::Tag;
    use parley::{
        BreakReason, FontContext, FontFamily, FontFamilyName, LayoutContext, LineHeight,
        StyleProperty,
    };

    use super::{
        CORPUS, CurrentGap, FontChoice, FontFeature, FontVariation, config, current_gaps,
        default_slots, find_font, load_fonts, lower_paint, prepare, slots_with_paint_boundaries,
    };

    const FEATURE_CORPUS: &str = "AV";

    #[derive(Clone, Debug)]
    struct OracleLine {
        source: std::ops::Range<usize>,
        reason: BreakReason,
        line_height: f32,
        baseline: f32,
        advance: f32,
        rtl_runs: Vec<bool>,
    }

    fn oracle_lines(text: &str, max_advance: f32, line_height: f32) -> Vec<OracleLine> {
        let mut collection = Collection::new(CollectionOptions {
            shared: false,
            system_fonts: false,
        });
        for file_name in ["RobotoFlex-VariableFont.ttf", "NotoKufiArabic-Regular.otf"] {
            let path = find_font(file_name).expect("pinned Parley oracle font must exist");
            let bytes = std::fs::read(path).expect("pinned Parley oracle font must be readable");
            collection.register_fonts(Blob::new(Arc::new(bytes)), None);
        }
        let mut font_context = FontContext {
            collection,
            source_cache: SourceCache::default(),
        };
        let mut layout_context: LayoutContext<[u8; 4]> = LayoutContext::new();
        let families = [
            FontFamilyName::named("Roboto Flex"),
            FontFamilyName::named("Noto Kufi Arabic"),
        ];
        let mut builder = layout_context.ranged_builder(&mut font_context, text, 1.0, false);
        builder.push_default(FontFamily::from(&families[..]));
        builder.push_default(StyleProperty::FontSize(20.0));
        builder.push_default(LineHeight::FontSizeRelative(line_height));
        let mut layout = builder.build(text);
        layout.break_all_lines(Some(max_advance));
        layout
            .lines()
            .map(|line| OracleLine {
                source: line.text_range(),
                reason: line.break_reason(),
                line_height: line.metrics().line_height,
                baseline: line.metrics().baseline,
                advance: line.metrics().advance,
                rtl_runs: line.runs().map(|run| run.is_rtl()).collect(),
            })
            .collect()
    }

    #[test]
    fn retained_shaped_text_produces_deterministic_observations() {
        let fonts = load_fonts().expect("pinned Parley fonts must load");
        let slots = default_slots(CORPUS);
        let first = prepare(CORPUS, &fonts, &slots, config(&[], &[]));
        let second = prepare(CORPUS, &fonts, &slots, config(&[], &[]));
        assert_eq!(first, second, "identical inputs must prepare identically");
        assert!(!first.items.is_empty(), "analysis must emit owned items");
        assert!(!first.runs.is_empty(), "shaping must emit owned runs");
        assert!(
            first.runs.iter().all(|run| !run.glyphs.is_empty()),
            "every retained text run must expose glyph observations"
        );
    }

    #[test]
    fn paint_values_and_boundaries_never_change_text_physics() {
        let fonts = load_fonts().expect("pinned Parley fonts must load");
        let flat = prepare(CORPUS, &fonts, &default_slots(CORPUS), config(&[], &[]));
        let divided = prepare(
            CORPUS,
            &fonts,
            &slots_with_paint_boundaries(CORPUS),
            config(&[], &[]),
        );

        assert_eq!(
            flat.analysis_digest, divided.analysis_digest,
            "paint topology must not invalidate analysis"
        );
        assert_eq!(
            flat.item_digest, divided.item_digest,
            "paint topology must not alter shaping itemization"
        );
        assert_eq!(
            flat.physics_digest, divided.physics_digest,
            "paint topology must not alter glyphs or advances"
        );
        assert_ne!(
            flat.slot_digest, divided.slot_digest,
            "paint-slot coverage must remain observable"
        );

        let latin_i = CORPUS
            .chars()
            .position(|ch| ch == 'i')
            .expect("corpus must contain an fi ligature candidate");
        let ligature = divided
            .runs
            .iter()
            .flat_map(|run| &run.glyphs)
            .find(|glyph| {
                glyph.source_chars.contains(&latin_i)
                    && glyph.source_chars.end - glyph.source_chars.start > 1
            })
            .expect("fixture must shape the paint boundary inside a ligature");
        assert_eq!(
            ligature.paint_slots,
            [0, 1],
            "one ligature glyph must retain both source paint slots"
        );
        assert!(
            divided
                .runs
                .iter()
                .flat_map(|run| &run.glyphs)
                .any(|glyph| glyph.paint_slots.contains(&2)),
            "Arabic cursive shaping must retain the second paint boundary"
        );

        let first_paint = lower_paint(&divided, &[0xff00_00ff, 0x00ff_00ff, 0x0000_ffff])
            .expect("all paint slots must resolve");
        let second_paint = lower_paint(&divided, &[0x1010_10ff, 0x2020_20ff, 0x3030_30ff])
            .expect("all paint slots must resolve");
        assert_ne!(
            first_paint, second_paint,
            "paint-table changes must affect only lowering"
        );
    }

    #[test]
    fn weight_and_kerning_changes_preserve_earlier_stages() {
        let fonts = load_fonts().expect("pinned Parley fonts must load");
        let slots = default_slots(FEATURE_CORPUS);
        let baseline = prepare(FEATURE_CORPUS, &fonts, &slots, config(&[], &[]));
        let weight = [FontVariation::new(Tag::from_bytes(*b"wght"), 700.0)];
        let weighted = prepare(FEATURE_CORPUS, &fonts, &slots, config(&[], &weight));
        let no_kerning = [FontFeature::new(Tag::from_bytes(*b"kern"), 0)];
        let unkerned = prepare(FEATURE_CORPUS, &fonts, &slots, config(&no_kerning, &[]));

        for candidate in [&weighted, &unkerned] {
            assert_eq!(
                baseline.analysis_digest, candidate.analysis_digest,
                "shaping values must not invalidate Unicode analysis"
            );
            assert_eq!(
                baseline.item_digest, candidate.item_digest,
                "constant shaping values must not change item topology"
            );
            assert_ne!(
                baseline.physics_digest, candidate.physics_digest,
                "weight and kerning settings must change shaped physics"
            );
        }
    }

    #[test]
    fn corpus_exercises_latin_ltr_and_arabic_rtl_items() {
        let fonts = load_fonts().expect("pinned Parley fonts must load");
        let prepared = prepare(CORPUS, &fonts, &default_slots(CORPUS), config(&[], &[]));

        assert!(
            prepared.items.iter().any(|item| {
                item.script == *b"Latn"
                    && item.bidi_level & 1 == 0
                    && item.font == FontChoice::Latin
            }),
            "corpus must exercise a Latin left-to-right item"
        );
        assert!(
            prepared.items.iter().any(|item| {
                item.script == *b"Arab"
                    && item.bidi_level & 1 == 1
                    && item.font == FontChoice::Arabic
            }),
            "corpus must exercise an Arabic right-to-left item"
        );
    }

    #[test]
    fn items_and_glyph_sources_stay_inside_the_semantic_text() {
        let fonts = load_fonts().expect("pinned Parley fonts must load");
        let prepared = prepare(
            CORPUS,
            &fonts,
            &slots_with_paint_boundaries(CORPUS),
            config(&[], &[]),
        );
        let mut cursor = 0;
        for item in &prepared.items {
            assert_eq!(
                item.byte_range.start, cursor,
                "items must tile source bytes without gaps"
            );
            cursor = item.byte_range.end;
        }
        assert_eq!(cursor, CORPUS.len(), "items must cover all source bytes");

        let source_chars = CORPUS.chars().count();
        for glyph in prepared.runs.iter().flat_map(|run| &run.glyphs) {
            assert!(
                glyph.source_bytes.start <= glyph.source_bytes.end
                    && glyph.source_bytes.end <= CORPUS.len(),
                "glyph byte coverage must remain inside the source"
            );
            assert!(
                glyph.source_chars.start < glyph.source_chars.end
                    && glyph.source_chars.end <= source_chars,
                "glyph character coverage must be nonempty and in range"
            );
            assert!(
                !glyph.paint_slots.is_empty(),
                "every copied glyph must retain paint-slot coverage"
            );
        }
    }

    #[test]
    fn unsupported_candidate_seams_remain_explicit_gaps() {
        assert_eq!(
            current_gaps(),
            &[
                CurrentGap::VerticalShaping,
                CurrentGap::CoreInlineObjects,
                CurrentGap::TextDataIdentity,
            ],
            "the wind tunnel must not impersonate absent upstream seams"
        );
    }

    #[test]
    fn high_level_oracle_wraps_only_at_legal_boundaries() {
        let text = "alpha beta gamma";
        let lines = oracle_lines(text, 72.0, 1.2);
        assert_eq!(lines.len(), 3, "oracle evidence: {lines:#?}");
        assert_eq!(lines[0].source.start, 0);
        assert_eq!(
            lines[1].source.start,
            text.find("beta").expect("beta is present")
        );
        assert_eq!(
            lines[2].source.start,
            text.find("gamma").expect("gamma is present")
        );
        assert_eq!(lines[0].reason, BreakReason::Regular);
        assert_eq!(lines[1].reason, BreakReason::Regular);
        assert_eq!(lines[2].reason, BreakReason::None);
    }

    #[test]
    fn high_level_oracle_coalesces_crlf_and_honors_mandatory_breaks() {
        let text = "a\r\nb\nc\u{2028}d\u{2029}e";
        let lines = oracle_lines(text, 1_000.0, 1.2);
        assert_eq!(lines.len(), 5, "oracle evidence: {lines:#?}");
        assert_eq!(
            lines
                .iter()
                .filter(|line| line.reason == BreakReason::Explicit)
                .count(),
            4,
            "CRLF must be one explicit break; LF, LS, and PS add one each"
        );
        assert_eq!(lines[0].source, 0..3, "CRLF stays on one source line");
        assert_eq!(
            lines.last().expect("final line exists").source.end,
            text.len()
        );
    }

    #[test]
    fn high_level_oracle_uses_real_line_metrics() {
        let lines = oracle_lines("Ag", 1_000.0, 1.5);
        assert_eq!(lines.len(), 1);
        let line = &lines[0];
        assert_eq!(line.line_height, 30.0);
        assert!(line.baseline > 0.0 && line.baseline < line.line_height);
        assert_ne!(
            line.baseline, 24.0,
            "baseline must not be the provisional 80% split"
        );
    }

    #[test]
    fn high_level_oracle_overflows_an_unbreakable_word() {
        let lines = oracle_lines("alphabet", 1.0, 1.2);
        assert_eq!(
            lines.len(),
            1,
            "a word without legal opportunities must not split: {lines:#?}"
        );
        assert!(
            lines[0].advance > 1.0,
            "unbreakable content overflows honestly"
        );
    }

    #[test]
    fn high_level_oracle_records_current_nbsp_divergence() {
        let text = "alpha\u{00a0}beta";
        let lines = oracle_lines(text, 1.0, 1.2);
        assert_eq!(lines.len(), 2, "oracle evidence changed: {lines:#?}");
        assert_eq!(
            lines[1].source.start,
            text.find("beta").expect("beta is present"),
            "pinned high-level Parley hangs NBSP then breaks after it; Underwood must not copy this policy"
        );
    }

    #[test]
    fn high_level_oracle_exposes_line_local_mixed_bidi() {
        let lines = oracle_lines("office مرحبا world", 1_000.0, 1.2);
        assert_eq!(lines.len(), 1);
        assert!(
            lines[0].rtl_runs.iter().any(|is_rtl| !is_rtl)
                && lines[0].rtl_runs.iter().any(|is_rtl| *is_rtl),
            "one line must contain both LTR and RTL runs: {lines:#?}"
        );
    }
}
