// Copyright 2026 the Underwood Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Deterministic compatibility trace for the two native IME protocol families.

use underwood::{
    Brush, Color, CompositionId, CompositionSession, CompositionUpdate, ComputedInlineStyle,
    Document, DocumentId, EditableSurface, EditableSurfaceElement, FiniteWidth, FontFamily,
    InlineFlowStyle, InlineRole, LayoutEngine, PaintSlot, PaintTable, ParagraphRole, Point,
    SceneRequest, Script, ShapingStyle, SnapshotTextSelectionSet, StyleMap, SurfaceTextEncoding,
};
use underwood_parley::{Font, FontSet, ParleyParagraphEngine, TextData};

const LATIN_FONT: &[u8] =
    include_bytes!("../../../examples/headless/fonts/RobotoFlex-VariableFont.ttf");
const ARABIC_FONT: &[u8] =
    include_bytes!("../../../examples/headless/fonts/NotoKufiArabic-Regular.otf");

#[derive(Debug)]
struct FeedAdapter {
    session: CompositionSession,
}

impl FeedAdapter {
    fn preedit(&mut self, text: &str, selection: core::ops::Range<u32>) -> Result<(), AnyError> {
        let expected = self.session.epoch();
        self.session.update(
            expected,
            CompositionUpdate::new(text).with_selection(selection),
        )?;
        Ok(())
    }
}

#[derive(Debug)]
struct Fixture {
    document: Document,
    first_text: underwood::TextId,
    styles: StyleMap,
    paint: PaintTable,
    layout: LayoutEngine,
}

type AnyError = Box<dyn std::error::Error>;

fn main() -> Result<(), AnyError> {
    let mut fixture = fixture()?;
    let snapshot = fixture.document.snapshot();
    let request = SceneRequest::new(FiniteWidth::new(640.0)?, &fixture.styles, &fixture.paint);
    let committed = fixture.layout.prepare(&snapshot, &request)?;
    let scene = committed.scene();
    let primary = *scene
        .hit_test_closest(Point::new(-100.0, scene.lines()[0].bounds().center().y))
        .expect("first paragraph has a primary insertion point")
        .position();
    let secondary = *scene
        .hit_test_closest(Point::new(10_000.0, scene.lines()[2].bounds().center().y))
        .expect("last paragraph has a secondary insertion point")
        .position();
    let selections = scene.selection_set([
        scene.collapsed_selection(&primary)?,
        scene.collapsed_selection(&secondary)?,
    ])?;
    let start =
        scene.begin_composition(&selections, CompositionId::from_bytes(*b"ime-compat-epoch"))?;
    assert!(
        start.selection_changed(),
        "native composition must report multi-selection normalization"
    );
    assert_eq!(
        start.selections().selections().len(),
        1,
        "one native marked region requires one normalized insertion point"
    );
    let scope = EditableSurface::new(
        &snapshot,
        [EditableSurfaceElement::text(fixture.first_text)],
    )?;
    let host_base = scope.bind(scene, start.selections())?;
    let explicit_replacement = host_base.replacement_selection(0..5)?;
    assert_eq!(
        explicit_replacement
            .primary()
            .expect("the explicit host range must produce one insertion point")
            .ranges()[0]
            .bytes(),
        0..5,
        "host mutation ranges must map back to validated semantic source"
    );
    println!(
        "feed.begin base_revision={:?} selections={} normalized={} changed={}",
        snapshot.revision(),
        selections.selections().len(),
        start.selections().selections().len(),
        start.selection_changed()
    );
    println!(
        "host.replace surface=0..5 selections={} source=0..5",
        explicit_replacement.selections().len()
    );

    let mut feed = FeedAdapter {
        session: start.into_session(),
    };
    feed.preedit("مرحبا", 10..10)?;
    let projected = fixture
        .layout
        .prepare_composition(&snapshot, &request, &feed.session)?;
    assert_eq!(
        projected.work().shape().paragraphs(),
        1,
        "only the composition paragraph may reshape"
    );
    assert_eq!(
        projected.work().reused_paragraphs(),
        2,
        "both unaffected paragraphs must remain retained"
    );
    println!(
        "feed.preedit epoch={} shape={} geometry={} reused={} committed_revision={:?}",
        feed.session.epoch().get(),
        projected.work().shape().paragraphs(),
        projected.work().geometry().paragraphs(),
        projected.work().reused_paragraphs(),
        snapshot.revision()
    );

    let host = scope.bind_composition(projected.scene(), &feed.session)?;
    let marked = host.marked_range().expect("preedit has a marked range");
    let utf16 = host.range_in_encoding(marked.clone(), SurfaceTextEncoding::Utf16)?;
    let caret = host.caret_rect().expect("host caret geometry is present");
    let first_rect = host
        .first_rect_for_range(marked.clone())?
        .expect("host marked-text geometry is present");
    let hit = host
        .offset_for_point(first_rect.center())
        .expect("host point query maps to a surface offset");
    assert_eq!(
        host.text_for_range(marked.clone())?,
        "مرحبا",
        "host text query must read generated marked text"
    );
    assert_eq!(utf16, 0..5, "Arabic scalars each occupy one UTF-16 unit");
    assert!(
        (marked.start..=marked.end).contains(&hit),
        "point query must map into the marked surface range"
    );
    println!(
        "host.snapshot epoch={} text={:?} selection={:?} marked={:?} marked_utf16={:?}",
        feed.session.epoch().get(),
        host.text(),
        host.host_selection(),
        marked,
        utf16
    );
    println!(
        "host.geometry caret=({:.2},{:.2},{:.2},{:.2}) first=({:.2},{:.2},{:.2},{:.2}) hit={}",
        caret.x0,
        caret.y0,
        caret.x1,
        caret.y1,
        first_rect.x0,
        first_rect.y0,
        first_rect.x1,
        first_rect.y1,
        hit
    );

    feed.preedit("مرحبا", 0..0)?;
    let selection_only = fixture
        .layout
        .prepare_composition(&snapshot, &request, &feed.session)?;
    assert_eq!(
        selection_only.work().shape().paragraphs(),
        0,
        "moving only the preedit selection must not reshape"
    );
    assert_eq!(
        selection_only.work().geometry().paragraphs(),
        0,
        "moving only the preedit selection must retain geometry"
    );
    println!(
        "feed.selection epoch={} shape={} geometry={} reused={}",
        feed.session.epoch().get(),
        selection_only.work().shape().paragraphs(),
        selection_only.work().geometry().paragraphs(),
        selection_only.work().reused_paragraphs()
    );

    let cancelled_selection: SnapshotTextSelectionSet = feed.session.clone().cancel();
    let cancelled = fixture.layout.prepare(&snapshot, &request)?;
    assert_eq!(
        cancelled.work().shape().paragraphs(),
        0,
        "cancel must reveal committed shaping without recomputation"
    );
    assert_eq!(
        cancelled.work().reused_paragraphs(),
        3,
        "cancel must reveal every committed paragraph from cache"
    );
    println!(
        "feed.cancel publications=0 shape={} geometry={} reused={} selection_count={}",
        cancelled.work().shape().paragraphs(),
        cancelled.work().geometry().paragraphs(),
        cancelled.work().reused_paragraphs(),
        cancelled_selection.selections().len()
    );

    let publication = feed.session.commit(&mut fixture.document, "مرحبا")?;
    assert_eq!(
        publication.publication().changes().paragraphs().len(),
        1,
        "commit must publish one affected paragraph exactly once"
    );
    let committed_update = fixture
        .layout
        .prepare(publication.publication().snapshot(), &request)?;
    assert_eq!(
        committed_update.work().reused_paragraphs(),
        2,
        "commit must retain both unaffected siblings"
    );
    println!(
        "feed.commit revision={:?} changed={} shape={} geometry={} reused={}",
        publication.publication().snapshot().revision(),
        publication.publication().changes().paragraphs().len(),
        committed_update.work().shape().paragraphs(),
        committed_update.work().geometry().paragraphs(),
        committed_update.work().reused_paragraphs()
    );
    Ok(())
}

fn fixture() -> Result<Fixture, AnyError> {
    let mut document = Document::new(DocumentId::from_bytes(*b"ime-compat-proof"));
    let mut edit = document.edit();
    let first = edit.append_paragraph(ParagraphRole::BODY)?;
    let first_text = edit.append_text(first, InlineRole::TEXT, "alpha")?;
    let middle = edit.append_paragraph(ParagraphRole::BODY)?;
    edit.append_text(middle, InlineRole::TEXT, "bravo")?;
    let last = edit.append_paragraph(ParagraphRole::BODY)?;
    edit.append_text(last, InlineRole::TEXT, "charlie")?;
    edit.commit()?;

    let style = ComputedInlineStyle::new(
        ShapingStyle::new(FontFamily::named("Roboto Flex"), 20.0)?,
        InlineFlowStyle::default(),
        PaintSlot::new(0),
    );
    let styles = StyleMap::new(style);
    let paint = PaintTable::from_brushes([Brush::Solid(Color::BLACK)]);
    let fonts = FontSet::try_from_fonts([
        Font::from_bytes("latin", LATIN_FONT)?,
        Font::from_bytes("arabic", ARABIC_FONT)?,
    ])?
    .with_fallbacks(Script::from_bytes(*b"Arab"), None, ["Noto Kufi Arabic"])?;
    let layout = LayoutEngine::new(ParleyParagraphEngine::new(
        TextData::compiled_minimal(),
        fonts,
    )?);
    Ok(Fixture {
        document,
        first_text,
        styles,
        paint,
        layout,
    })
}
