// Copyright 2026 the Underwood Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::boxed::Box;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::ops::Range;

use crate::adapter::{
    PaintRun, ParagraphInput, ParagraphPreparation, PreparationWork, PreparedGlyph,
    PreparedParagraph, PreparedRun, ShapingRun,
};
use crate::document::Paragraph;
use crate::{
    Affine, ComputedInlineStyle, DocumentRevision, DocumentSnapshot, FontData, InlineFlowStyle,
    InlineRole, PaintSlot, PaintTable, ParagraphId, ParagraphRole, Point, Rect, SceneError,
    SceneErrorKind, SceneRequest, SemanticId, TextId, Vec2,
};

/// Mutable owner of one paragraph adapter and its retained stage caches.
pub struct LayoutEngine {
    paragraphs: Box<dyn ParagraphPreparation>,
    cache: Vec<ParagraphCache>,
}

impl core::fmt::Debug for LayoutEngine {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("LayoutEngine")
            .field("cached_paragraphs", &self.cache.len())
            .finish_non_exhaustive()
    }
}

impl LayoutEngine {
    /// Creates an engine owning exactly one configured paragraph adapter.
    #[must_use]
    pub fn new(paragraphs: impl ParagraphPreparation + 'static) -> Self {
        Self {
            paragraphs: Box::new(paragraphs),
            cache: Vec::new(),
        }
    }

    /// Prepares an immutable scene without publishing partial results on failure.
    pub fn prepare(
        &mut self,
        snapshot: &DocumentSnapshot,
        request: &SceneRequest<'_>,
    ) -> Result<SceneOutput, SceneError> {
        validate_styles(snapshot, request)?;

        let mut work = WorkReport::default();
        let mut lines = Vec::new();
        let mut fragments = Vec::new();
        let mut semantics = Vec::new();
        let mut y_offset = 0.0;

        for paragraph in snapshot.paragraphs() {
            let projection = Projection::new(paragraph, request)?;
            let preparation_key = PreparationKey {
                version: paragraph.version,
                shaping_runs: projection.shaping_runs.clone(),
            };
            let cache_index = self
                .cache
                .iter()
                .position(|entry| entry.paragraph == paragraph.id);

            let needs_preparation = cache_index.is_none_or(|index| {
                self.cache[index].preparation_key != preparation_key
                    || self.cache[index].paint_runs != projection.paint_runs
            });
            let cache_index = if needs_preparation {
                let text_len = u32::try_from(projection.text.len()).map_err(|_| {
                    SceneError::for_paragraph(SceneErrorKind::SourceCoverage, paragraph.id)
                })?;
                let output = self
                    .paragraphs
                    .prepare(ParagraphInput::new(
                        paragraph.id,
                        &projection.text,
                        &projection.shaping_runs,
                        &projection.paint_runs,
                    ))
                    .map_err(|error| SceneError::from_preparation(paragraph.id, error.kind()))?;
                if output.paragraph().paragraph() != paragraph.id
                    || output.paragraph().text_len() != text_len
                {
                    return Err(SceneError::for_paragraph(
                        SceneErrorKind::SourceCoverage,
                        paragraph.id,
                    ));
                }
                validate_prepared(output.paragraph(), &projection)?;
                record_preparation_work(&mut work, output.work());
                let mut geometry = cache_index.and_then(|index| {
                    (output.work().shaped_runs() == 0
                        && self.cache[index].preparation_key == preparation_key)
                        .then(|| self.cache[index].geometry.take())
                        .flatten()
                });
                if let Some(cached) = geometry.as_mut() {
                    update_cached_paint(cached, output.paragraph(), &projection)?;
                }
                let entry = ParagraphCache {
                    paragraph: paragraph.id,
                    preparation_key,
                    paint_runs: projection.paint_runs.clone(),
                    prepared: output.into_paragraph(),
                    geometry,
                };
                if let Some(index) = cache_index {
                    self.cache[index] = entry;
                    index
                } else {
                    self.cache.push(entry);
                    self.cache.len() - 1
                }
            } else {
                work.reused_paragraphs += 1;
                cache_index.expect("a reusable cache index must exist")
            };

            let width_key = request.width.0.to_bits();
            if self.cache[cache_index]
                .geometry
                .as_ref()
                .is_none_or(|geometry| {
                    geometry.width != width_key
                        || geometry.inline_flow_runs != projection.inline_flow_runs
                })
            {
                let geometry = build_geometry(
                    &self.cache[cache_index].prepared,
                    &projection,
                    request.width.0,
                )?;
                work.flow.add_paragraph(geometry.lines.len());
                work.geometry.add_paragraph(geometry.fragments.len());
                self.cache[cache_index].geometry = Some(CachedGeometry {
                    width: width_key,
                    ..geometry
                });
            }

            let geometry = self.cache[cache_index]
                .geometry
                .as_ref()
                .expect("geometry was installed above");
            materialize_geometry(
                geometry,
                snapshot.revision(),
                y_offset,
                &mut lines,
                &mut fragments,
                &mut semantics,
            );
            y_offset += geometry.height;
        }

        work.paint = StageWork {
            paragraphs: snapshot.paragraphs().len(),
            records: fragments.len(),
        };
        Ok(SceneOutput {
            scene: TextScene {
                lines,
                fragments,
                paint: request.paint.clone(),
                semantics,
            },
            work,
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
struct PreparationKey {
    version: u64,
    shaping_runs: Vec<ShapingRun>,
}

#[derive(Clone, Debug)]
struct ParagraphCache {
    paragraph: ParagraphId,
    preparation_key: PreparationKey,
    paint_runs: Vec<PaintRun>,
    prepared: PreparedParagraph,
    geometry: Option<CachedGeometry>,
}

#[derive(Clone, Debug)]
struct Projection {
    paragraph: ParagraphId,
    text: alloc::string::String,
    spans: Vec<LeafSpan>,
    shaping_runs: Vec<ShapingRun>,
    inline_flow_runs: Vec<InlineFlowRun>,
    paint_runs: Vec<PaintRun>,
    default_style: ComputedInlineStyle,
    paragraph_semantic: SemanticId,
    paragraph_role: ParagraphRole,
}

impl Projection {
    fn new(paragraph: &Paragraph, request: &SceneRequest<'_>) -> Result<Self, SceneError> {
        let text = paragraph.projected_text();
        let mut spans = Vec::with_capacity(paragraph.leaves.len());
        let mut shaping_runs = Vec::with_capacity(paragraph.leaves.len());
        let mut inline_flow_runs = Vec::with_capacity(paragraph.leaves.len());
        let mut paint_runs = Vec::with_capacity(paragraph.leaves.len());
        let mut start = 0_u32;
        for leaf in &paragraph.leaves {
            let len = u32::try_from(leaf.text.len()).map_err(|_| {
                SceneError::for_paragraph(SceneErrorKind::SourceCoverage, paragraph.id)
            })?;
            let end = start.checked_add(len).ok_or_else(|| {
                SceneError::for_paragraph(SceneErrorKind::SourceCoverage, paragraph.id)
            })?;
            let style = request.styles.style_for(leaf.id);
            spans.push(LeafSpan {
                paragraph: start..end,
                text: leaf.id,
                role: leaf.role,
                semantic: leaf.semantic_id(),
            });
            if start != end {
                append_shaping_run(&mut shaping_runs, start..end, style.shaping());
                append_inline_flow_run(&mut inline_flow_runs, start..end, style.inline_flow());
                append_paint_run(&mut paint_runs, start..end, style.paint());
            }
            start = end;
        }
        Ok(Self {
            paragraph: paragraph.id,
            text,
            spans,
            shaping_runs,
            inline_flow_runs,
            paint_runs,
            default_style: request.styles.default_style().clone(),
            paragraph_semantic: paragraph.semantic_id(),
            paragraph_role: paragraph.role,
        })
    }

    fn local_range(&self, paragraph: Range<u32>) -> Result<LocalRange, SceneError> {
        let span = self
            .spans
            .iter()
            .find(|span| {
                paragraph.start >= span.paragraph.start && paragraph.end <= span.paragraph.end
            })
            .ok_or_else(|| {
                SceneError::for_source(
                    SceneErrorKind::SourceCoverage,
                    self.paragraph,
                    paragraph.clone(),
                )
            })?;
        Ok(LocalRange {
            text: span.text,
            bytes: (paragraph.start - span.paragraph.start)..(paragraph.end - span.paragraph.start),
        })
    }

    fn line_height(&self, source: Range<u32>, font_size: f32) -> f64 {
        let multiplier = self
            .inline_flow_runs
            .iter()
            .filter(|run| run.bytes.start < source.end && run.bytes.end > source.start)
            .map(|run| run.style.line_height().multiplier())
            .fold(0.0_f32, f32::max);
        let multiplier = if multiplier == 0.0 {
            self.default_style.inline_flow().line_height().multiplier()
        } else {
            multiplier
        };
        f64::from(font_size) * f64::from(multiplier)
    }
}

#[derive(Clone, Debug)]
struct LeafSpan {
    paragraph: Range<u32>,
    text: TextId,
    role: InlineRole,
    semantic: SemanticId,
}

#[derive(Clone, Debug, PartialEq)]
struct InlineFlowRun {
    bytes: Range<u32>,
    style: InlineFlowStyle,
}

fn append_shaping_run(runs: &mut Vec<ShapingRun>, bytes: Range<u32>, style: &crate::ShapingStyle) {
    if let Some(last) = runs.last_mut()
        && last.bytes().end == bytes.start
        && last.style() == style
    {
        let start = last.bytes().start;
        *last = ShapingRun::new(start..bytes.end, style.clone());
    } else {
        runs.push(ShapingRun::new(bytes, style.clone()));
    }
}

fn append_inline_flow_run(
    runs: &mut Vec<InlineFlowRun>,
    bytes: Range<u32>,
    style: InlineFlowStyle,
) {
    if let Some(last) = runs.last_mut()
        && last.bytes.end == bytes.start
        && last.style == style
    {
        last.bytes.end = bytes.end;
    } else {
        runs.push(InlineFlowRun { bytes, style });
    }
}

fn append_paint_run(runs: &mut Vec<PaintRun>, bytes: Range<u32>, slot: PaintSlot) {
    runs.push(PaintRun::new(bytes, slot));
}

fn validate_styles(
    snapshot: &DocumentSnapshot,
    request: &SceneRequest<'_>,
) -> Result<(), SceneError> {
    if request
        .styles
        .overrides()
        .iter()
        .any(|(text, _)| snapshot.text(*text).is_none())
    {
        return Err(SceneError::for_document(
            SceneErrorKind::InvalidStyle,
            snapshot.id(),
        ));
    }
    for paragraph in snapshot.paragraphs() {
        for leaf in &paragraph.leaves {
            if request
                .paint
                .brush(request.styles.style_for(leaf.id).paint())
                .is_none()
            {
                return Err(SceneError::for_paragraph(
                    SceneErrorKind::InvalidStyle,
                    paragraph.id,
                ));
            }
        }
    }
    Ok(())
}

fn validate_prepared(
    prepared: &PreparedParagraph,
    projection: &Projection,
) -> Result<(), SceneError> {
    for run in prepared.runs() {
        let source = run.source();
        if projection
            .text
            .get(source.start as usize..source.end as usize)
            .is_none()
        {
            return Err(SceneError::for_source(
                SceneErrorKind::SourceCoverage,
                prepared.paragraph(),
                source,
            ));
        }
        for glyph in run.glyphs() {
            let source = glyph.source();
            if projection
                .text
                .get(source.start as usize..source.end as usize)
                .is_none()
            {
                return Err(SceneError::for_source(
                    SceneErrorKind::SourceCoverage,
                    prepared.paragraph(),
                    source,
                ));
            }
            for segment in glyph.paint().segments() {
                let source = segment.source();
                if projection
                    .text
                    .get(source.start as usize..source.end as usize)
                    .is_none()
                {
                    return Err(SceneError::for_source(
                        SceneErrorKind::SourceCoverage,
                        prepared.paragraph(),
                        source,
                    ));
                }
                projection.local_range(source)?;
            }
        }
    }
    Ok(())
}

fn record_preparation_work(report: &mut WorkReport, work: PreparationWork) {
    if work.analyzed() {
        report.analysis.add_paragraph(1);
    }
    if work.itemized() {
        report.itemization.add_paragraph(1);
    }
    if work.shaped_runs() > 0 {
        report.shape.paragraphs += 1;
        report.shape.records += work.shaped_glyphs() as usize;
    }
}

#[derive(Clone, Debug)]
struct CachedGeometry {
    width: u64,
    inline_flow_runs: Vec<InlineFlowRun>,
    height: f64,
    lines: Vec<CachedLine>,
    fragments: Vec<CachedFragment>,
    semantics: Vec<CachedSemantic>,
}

#[derive(Clone, Debug)]
struct CachedLine {
    bounds: Rect,
    source: LocalRange,
}

#[derive(Clone, Debug)]
struct CachedFragment {
    id: SceneFragmentId,
    glyphs: Vec<CachedGlyph>,
    paint: PaintSlot,
    transform: Affine,
    source: LocalRange,
    bounds: Rect,
    clip: Rect,
    font: FontData,
    font_size: f32,
    normalized_coords: Arc<[i16]>,
    bidi_level: u8,
    script: [u8; 4],
}

#[derive(Clone, Debug)]
struct CachedGlyph {
    id: u32,
    position: Point,
    advance: Vec2,
    source: LocalRange,
}

#[derive(Clone, Debug)]
struct CachedSemantic {
    semantic_id: SemanticId,
    paragraph_role: Option<ParagraphRole>,
    inline_role: Option<InlineRole>,
    source: Option<LocalRange>,
    bounds: Rect,
}

#[derive(Clone, Debug)]
struct LocalRange {
    text: TextId,
    bytes: Range<u32>,
}

#[derive(Clone, Copy, Debug)]
struct PendingGlyph<'a> {
    run: &'a PreparedRun,
    glyph: &'a PreparedGlyph,
    x: f64,
}

fn build_geometry(
    prepared: &PreparedParagraph,
    projection: &Projection,
    width: f64,
) -> Result<CachedGeometry, SceneError> {
    let empty_line_height = f64::from(projection.default_style.shaping().font_size())
        * f64::from(
            projection
                .default_style
                .inline_flow()
                .line_height()
                .multiplier(),
        );
    let mut x = 0.0;
    let mut line_top = 0.0;
    let mut line_above = 0.0_f64;
    let mut line_below = 0.0_f64;
    let mut pending = Vec::new();
    let mut lines = Vec::new();
    let mut fragments = Vec::new();

    for run in prepared.runs() {
        for glyph in run.glyphs() {
            let advance = glyph.advance();
            let horizontal_advance = advance.x.abs();
            if !pending.is_empty() && x + horizontal_advance > width {
                flush_line(
                    prepared.paragraph(),
                    projection,
                    &pending,
                    line_top,
                    line_above,
                    line_below,
                    width,
                    &mut lines,
                    &mut fragments,
                )?;
                line_top += line_above + line_below;
                x = 0.0;
                line_above = 0.0;
                line_below = 0.0;
                pending.clear();
            }
            let line_height = projection.line_height(glyph.source(), run.font_size());
            line_above = line_above.max(line_height * 0.8);
            line_below = line_below.max(line_height * 0.2);
            pending.push(PendingGlyph { run, glyph, x });
            x += horizontal_advance;
        }
    }
    if !pending.is_empty() {
        flush_line(
            prepared.paragraph(),
            projection,
            &pending,
            line_top,
            line_above,
            line_below,
            width,
            &mut lines,
            &mut fragments,
        )?;
        line_top += line_above + line_below;
    }

    let mut semantics = Vec::new();
    if !projection.spans.is_empty()
        && let Some(first_line) = lines.first()
    {
        let bounds = lines
            .iter()
            .skip(1)
            .fold(first_line.bounds, |bounds, line| bounds.union(line.bounds));
        semantics.push(CachedSemantic {
            semantic_id: projection.paragraph_semantic,
            paragraph_role: Some(projection.paragraph_role),
            inline_role: None,
            source: None,
            bounds,
        });
    }
    for span in &projection.spans {
        if span.paragraph.is_empty() {
            continue;
        }
        let mut bounds: Option<Rect> = None;
        for fragment in &fragments {
            if fragment.source.text == span.text {
                bounds = Some(match bounds {
                    Some(current) => current.union(fragment.bounds),
                    None => fragment.bounds,
                });
            }
        }
        let source = LocalRange {
            text: span.text,
            bytes: 0..(span.paragraph.end - span.paragraph.start),
        };
        semantics.push(CachedSemantic {
            semantic_id: span.semantic,
            paragraph_role: None,
            inline_role: Some(span.role),
            source: Some(source),
            bounds: bounds.unwrap_or(Rect::new(0.0, 0.0, 0.0, empty_line_height)),
        });
    }

    Ok(CachedGeometry {
        width: width.to_bits(),
        inline_flow_runs: projection.inline_flow_runs.clone(),
        height: if fragments.is_empty() {
            empty_line_height
        } else {
            line_top
        },
        lines,
        fragments,
        semantics,
    })
}

fn flush_line(
    paragraph: ParagraphId,
    projection: &Projection,
    pending: &[PendingGlyph<'_>],
    line_top: f64,
    line_above: f64,
    line_below: f64,
    width: f64,
    lines: &mut Vec<CachedLine>,
    fragments: &mut Vec<CachedFragment>,
) -> Result<(), SceneError> {
    let baseline = line_top + line_above;
    let mut line_source = None;
    let mut right = 0.0_f64;
    for pending_glyph in pending {
        let run = pending_glyph.run;
        let glyph = pending_glyph.glyph;
        let position = Point::new(
            pending_glyph.x + glyph.offset().x,
            baseline - glyph.offset().y,
        );
        let normalized_coords: Arc<[i16]> = Arc::from(run.normalized_coords());
        for segment in glyph.paint().segments() {
            let source = projection.local_range(segment.source())?;
            let local_clip = segment.local_clip();
            let clip = Rect::new(
                position.x + local_clip.x0,
                position.y + local_clip.y0,
                position.x + local_clip.x1,
                position.y + local_clip.y1,
            );
            line_source.get_or_insert_with(|| source.clone());
            right = right.max(clip.x1);
            let id = SceneFragmentId(fragment_identity(paragraph, fragments.len()));
            fragments.push(CachedFragment {
                id,
                glyphs: alloc::vec![CachedGlyph {
                    id: glyph.id(),
                    position,
                    advance: glyph.advance(),
                    source: source.clone(),
                }],
                paint: segment.slot(),
                transform: Affine::IDENTITY,
                source,
                bounds: clip,
                clip,
                font: run.font().clone(),
                font_size: run.font_size(),
                normalized_coords: Arc::clone(&normalized_coords),
                bidi_level: run.bidi_level(),
                script: run.script(),
            });
        }
    }
    let source = line_source
        .ok_or_else(|| SceneError::for_paragraph(SceneErrorKind::SourceCoverage, paragraph))?;
    lines.push(CachedLine {
        bounds: Rect::new(
            0.0,
            line_top,
            right.max(1.0).min(width),
            line_top + line_above + line_below,
        ),
        source,
    });
    Ok(())
}

fn update_cached_paint(
    geometry: &mut CachedGeometry,
    prepared: &PreparedParagraph,
    projection: &Projection,
) -> Result<(), SceneError> {
    let mut fragments = geometry.fragments.iter_mut();
    for run in prepared.runs() {
        for glyph in run.glyphs() {
            for segment in glyph.paint().segments() {
                let source = projection.local_range(segment.source())?;
                let fragment = fragments.next().ok_or_else(|| {
                    SceneError::for_paragraph(SceneErrorKind::SourceCoverage, prepared.paragraph())
                })?;
                if fragment.source.text != source.text
                    || fragment.source.bytes != source.bytes
                    || fragment.glyphs[0].id != glyph.id()
                {
                    return Err(SceneError::for_paragraph(
                        SceneErrorKind::SourceCoverage,
                        prepared.paragraph(),
                    ));
                }
                fragment.paint = segment.slot();
            }
        }
    }
    if fragments.next().is_some() {
        return Err(SceneError::for_paragraph(
            SceneErrorKind::SourceCoverage,
            prepared.paragraph(),
        ));
    }
    Ok(())
}

fn fragment_identity(paragraph: ParagraphId, fragment: usize) -> u64 {
    let mut identity = 0xcbf2_9ce4_8422_2325_u64;
    for byte in paragraph.document.opaque_bytes() {
        identity = (identity ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01b3);
    }
    for byte in paragraph.index.to_le_bytes() {
        identity = (identity ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01b3);
    }
    for byte in u64::try_from(fragment).unwrap_or(u64::MAX).to_le_bytes() {
        identity = (identity ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01b3);
    }
    identity
}

fn materialize_geometry(
    geometry: &CachedGeometry,
    revision: DocumentRevision,
    y_offset: f64,
    lines: &mut Vec<SceneLine>,
    fragments: &mut Vec<SceneFragment>,
    semantics: &mut Vec<SemanticFragment>,
) {
    let translate = Vec2::new(0.0, y_offset);
    lines.extend(geometry.lines.iter().map(|line| SceneLine {
        bounds: line.bounds + translate,
        source: materialize_range(&line.source, revision),
    }));
    fragments.extend(geometry.fragments.iter().map(|fragment| {
        SceneFragment {
            id: fragment.id,
            glyphs: fragment
                .glyphs
                .iter()
                .map(|glyph| SceneGlyph {
                    id: glyph.id,
                    position: glyph.position + translate,
                    advance: glyph.advance,
                    source: materialize_range(&glyph.source, revision),
                })
                .collect(),
            paint: fragment.paint,
            transform: fragment.transform,
            source: Some(materialize_range(&fragment.source, revision)),
            bounds: fragment.bounds + translate,
            clip: fragment.clip + translate,
            font: fragment.font.clone(),
            font_size: fragment.font_size,
            normalized_coords: Arc::clone(&fragment.normalized_coords),
            bidi_level: fragment.bidi_level,
            script: fragment.script,
        }
    }));
    semantics.extend(geometry.semantics.iter().map(|semantic| {
        SemanticFragment {
            semantic_id: semantic.semantic_id,
            paragraph_role: semantic.paragraph_role,
            inline_role: semantic.inline_role,
            source: semantic
                .source
                .as_ref()
                .map(|source| materialize_range(source, revision)),
            bounds: semantic.bounds + translate,
        }
    }));
}

fn materialize_range(range: &LocalRange, revision: DocumentRevision) -> SnapshotTextRange {
    SnapshotTextRange {
        revision,
        text: range.text,
        bytes: range.bytes.clone(),
    }
}

/// Immutable prepared scene and exact work report.
#[derive(Clone, Debug)]
pub struct SceneOutput {
    scene: TextScene,
    work: WorkReport,
}

impl SceneOutput {
    /// Returns the prepared scene.
    #[must_use]
    pub const fn scene(&self) -> &TextScene {
        &self.scene
    }

    /// Returns actual work performed for this request.
    #[must_use]
    pub const fn work(&self) -> &WorkReport {
        &self.work
    }
}

/// Count of paragraphs and records processed by one stage.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct StageWork {
    paragraphs: usize,
    records: usize,
}

impl StageWork {
    fn add_paragraph(&mut self, records: usize) {
        self.paragraphs += 1;
        self.records += records;
    }

    /// Returns paragraphs processed rather than reused.
    #[must_use]
    pub const fn paragraphs(self) -> usize {
        self.paragraphs
    }

    /// Returns stage-specific records processed.
    #[must_use]
    pub const fn records(self) -> usize {
        self.records
    }
}

/// Exact stage work performed for one scene request.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WorkReport {
    analysis: StageWork,
    itemization: StageWork,
    shape: StageWork,
    flow: StageWork,
    geometry: StageWork,
    paint: StageWork,
    reused_paragraphs: usize,
}

impl WorkReport {
    /// Returns Unicode analysis work.
    #[must_use]
    pub const fn analysis(&self) -> StageWork {
        self.analysis
    }

    /// Returns shaping itemization work.
    #[must_use]
    pub const fn itemization(&self) -> StageWork {
        self.itemization
    }

    /// Returns shaping work.
    #[must_use]
    pub const fn shape(&self) -> StageWork {
        self.shape
    }

    /// Returns finite-width flow work.
    #[must_use]
    pub const fn flow(&self) -> StageWork {
        self.flow
    }

    /// Returns scene-geometry work.
    #[must_use]
    pub const fn geometry(&self) -> StageWork {
        self.geometry
    }

    /// Returns paint lowering work.
    #[must_use]
    pub const fn paint(&self) -> StageWork {
        self.paint
    }

    /// Returns paragraphs reused without calling the adapter.
    #[must_use]
    pub const fn reused_paragraphs(&self) -> usize {
        self.reused_paragraphs
    }
}

/// Immutable renderer-neutral text scene.
#[derive(Clone, Debug)]
pub struct TextScene {
    lines: Vec<SceneLine>,
    fragments: Vec<SceneFragment>,
    paint: PaintTable,
    semantics: Vec<SemanticFragment>,
}

impl TextScene {
    /// Returns visual lines in flow order.
    #[must_use]
    pub fn lines(&self) -> &[SceneLine] {
        &self.lines
    }

    /// Returns paint-homogeneous glyph fragments in visual order.
    #[must_use]
    pub fn fragments(&self) -> &[SceneFragment] {
        &self.fragments
    }

    /// Returns immutable paint values referenced by fragment slots.
    #[must_use]
    pub const fn paint(&self) -> &PaintTable {
        &self.paint
    }

    /// Iterates semantic fragments in document order.
    pub fn semantics(&self) -> impl Iterator<Item = &SemanticFragment> {
        self.semantics.iter()
    }

    /// Returns the source under a scene-space point.
    #[must_use]
    pub fn hit_test(&self, point: Point) -> Option<TextHit> {
        self.fragments
            .iter()
            .find(|fragment| fragment.bounds.contains(point))
            .and_then(|fragment| {
                fragment.source.clone().map(|source| TextHit {
                    source,
                    point,
                    line_height: fragment.bounds.height().max(1.0),
                })
            })
    }

    /// Produces caret geometry for a hit belonging to this scene revision.
    #[must_use]
    pub fn caret(&self, hit: &TextHit) -> SceneCaret {
        SceneCaret {
            source: hit.source.clone(),
            bounds: Rect::new(
                hit.point.x,
                hit.point.y - hit.line_height,
                hit.point.x + 1.0,
                hit.point.y,
            ),
        }
    }
}

/// One visual line.
#[derive(Clone, Debug)]
pub struct SceneLine {
    bounds: Rect,
    source: SnapshotTextRange,
}

impl SceneLine {
    /// Returns scene-space line bounds.
    #[must_use]
    pub const fn bounds(&self) -> Rect {
        self.bounds
    }

    /// Returns a snapshot-local source range represented by the line.
    #[must_use]
    pub const fn source(&self) -> &SnapshotTextRange {
        &self.source
    }
}

/// Opaque identity of a fragment within the current retained engine context.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SceneFragmentId(u64);

/// Paint-homogeneous shaped glyph fragment.
#[derive(Clone, Debug)]
pub struct SceneFragment {
    id: SceneFragmentId,
    glyphs: Vec<SceneGlyph>,
    paint: PaintSlot,
    transform: Affine,
    source: Option<SnapshotTextRange>,
    bounds: Rect,
    clip: Rect,
    font: FontData,
    font_size: f32,
    normalized_coords: Arc<[i16]>,
    bidi_level: u8,
    script: [u8; 4],
}

impl SceneFragment {
    /// Returns the retained fragment identity.
    #[must_use]
    pub const fn id(&self) -> SceneFragmentId {
        self.id
    }

    /// Returns shaped glyph observations.
    #[must_use]
    pub fn glyphs(&self) -> &[SceneGlyph] {
        &self.glyphs
    }

    /// Returns the paint slot.
    #[must_use]
    pub const fn paint(&self) -> PaintSlot {
        self.paint
    }

    /// Returns the fragment transform.
    #[must_use]
    pub const fn transform(&self) -> Affine {
        self.transform
    }

    /// Returns snapshot-local source when the fragment represents authored text.
    #[must_use]
    pub const fn source(&self) -> Option<&SnapshotTextRange> {
        self.source.as_ref()
    }

    /// Returns the exact font bytes and face index for these glyphs.
    #[must_use]
    pub const fn font(&self) -> &FontData {
        &self.font
    }

    /// Returns the scene-unit font size used to shape and position these glyphs.
    #[must_use]
    pub const fn font_size(&self) -> f32 {
        self.font_size
    }

    /// Returns normalized variation coordinates for the exact font instance.
    #[must_use]
    pub fn normalized_coords(&self) -> &[i16] {
        &self.normalized_coords
    }

    /// Returns the scene-space clip preserving this fragment's paint coverage.
    #[must_use]
    pub const fn clip(&self) -> Rect {
        self.clip
    }

    /// Returns the resolved Unicode bidi level for the shaped run.
    #[must_use]
    pub const fn bidi_level(&self) -> u8 {
        self.bidi_level
    }

    /// Returns the resolved ISO 15924 script tag for the shaped run.
    #[must_use]
    pub const fn script(&self) -> [u8; 4] {
        self.script
    }
}

/// One shaped glyph observation.
#[derive(Clone, Debug)]
pub struct SceneGlyph {
    id: u32,
    position: Point,
    advance: Vec2,
    source: SnapshotTextRange,
}

impl SceneGlyph {
    /// Returns the backend glyph identifier.
    #[must_use]
    pub const fn id(&self) -> u32 {
        self.id
    }

    /// Returns the glyph origin in scene coordinates.
    #[must_use]
    pub const fn position(&self) -> Point {
        self.position
    }

    /// Returns the shaped advance.
    #[must_use]
    pub const fn advance(&self) -> Vec2 {
        self.advance
    }

    /// Returns the snapshot-local source covered by this painted glyph observation.
    #[must_use]
    pub const fn source(&self) -> &SnapshotTextRange {
        &self.source
    }
}

/// Semantic observation with scene geometry.
#[derive(Clone, Debug)]
pub struct SemanticFragment {
    semantic_id: SemanticId,
    paragraph_role: Option<ParagraphRole>,
    inline_role: Option<InlineRole>,
    source: Option<SnapshotTextRange>,
    bounds: Rect,
}

impl SemanticFragment {
    /// Returns the source semantic identity.
    #[must_use]
    pub const fn semantic_id(&self) -> SemanticId {
        self.semantic_id
    }

    /// Returns the paragraph role, or `None` for an inline semantic fragment.
    #[must_use]
    pub const fn paragraph_role(&self) -> Option<ParagraphRole> {
        self.paragraph_role
    }

    /// Returns the inline role, or `None` for a block-level semantic fragment.
    #[must_use]
    pub const fn inline_role(&self) -> Option<InlineRole> {
        self.inline_role
    }

    /// Returns snapshot-local source when present.
    #[must_use]
    pub const fn source(&self) -> Option<&SnapshotTextRange> {
        self.source.as_ref()
    }

    /// Returns scene-space semantic bounds.
    #[must_use]
    pub const fn bounds(&self) -> Rect {
        self.bounds
    }
}

/// Result of scene-space hit testing.
#[derive(Clone, Debug)]
pub struct TextHit {
    source: SnapshotTextRange,
    point: Point,
    line_height: f64,
}

impl TextHit {
    /// Returns the snapshot-local source under the point.
    #[must_use]
    pub const fn source(&self) -> &SnapshotTextRange {
        &self.source
    }

    /// Returns the queried scene-space point.
    #[must_use]
    pub const fn point(&self) -> Point {
        self.point
    }
}

/// Scene-space caret derived from one text hit.
#[derive(Clone, Debug)]
pub struct SceneCaret {
    source: SnapshotTextRange,
    bounds: Rect,
}

impl SceneCaret {
    /// Returns the snapshot-local caret source.
    #[must_use]
    pub const fn source(&self) -> &SnapshotTextRange {
        &self.source
    }

    /// Returns scene-space caret bounds.
    #[must_use]
    pub const fn bounds(&self) -> Rect {
        self.bounds
    }
}

/// Dense source range valid only for one exact immutable snapshot revision.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SnapshotTextRange {
    revision: DocumentRevision,
    text: TextId,
    bytes: Range<u32>,
}

impl SnapshotTextRange {
    /// Returns the exact snapshot revision.
    #[must_use]
    pub const fn revision(&self) -> DocumentRevision {
        self.revision
    }

    /// Returns the text leaf identity.
    #[must_use]
    pub const fn text(&self) -> TextId {
        self.text
    }

    /// Returns the UTF-8 byte range within the text leaf.
    #[must_use]
    pub fn bytes(&self) -> Range<u32> {
        self.bytes.clone()
    }
}

#[cfg(test)]
mod tests {
    use alloc::vec;

    use peniko::Blob;

    use super::LayoutEngine;
    use crate::adapter::{
        GlyphPaintCoverage, GlyphPaintSegment, ParagraphInput, ParagraphPreparation,
        ParagraphPreparationOutput, PreparationError, PreparationWork, PreparedGlyph,
        PreparedParagraph, PreparedRun,
    };
    use crate::{
        Brush, Color, ComputedInlineStyle, Document, DocumentId, FiniteWidth, FontData,
        InlineFlowStyle, InlineRole, PaintSlot, PaintTable, ParagraphRole, Rect, SceneErrorKind,
        SceneRequest, ShapingStyle, StyleMap, Vec2,
    };

    #[derive(Debug)]
    struct EchoAdapter {
        split_utf8: bool,
    }

    impl ParagraphPreparation for EchoAdapter {
        fn prepare(
            &mut self,
            input: ParagraphInput<'_>,
        ) -> Result<ParagraphPreparationOutput, PreparationError> {
            let text_len = u32::try_from(input.text().len())
                .map_err(|_| PreparationError::invalid_output())?;
            let glyph_source = if self.split_utf8 {
                1..text_len
            } else {
                0..text_len
            };
            let segment = GlyphPaintSegment::new(
                glyph_source.clone(),
                input.paint_runs()[0].slot(),
                Rect::new(0., -16., 10., 4.),
            )?;
            let coverage = GlyphPaintCoverage::try_from_segments([segment])?;
            let glyph =
                PreparedGlyph::try_new(7, glyph_source, Vec2::new(10., 0.), Vec2::ZERO, coverage)?;
            let run = PreparedRun::try_new(
                0..text_len,
                0,
                *b"Latn",
                FontData::new(Blob::from(vec![0_u8]), 0),
                input.shaping_runs()[0].style().font_size(),
                [],
                [glyph],
            )?;
            let paragraph = PreparedParagraph::try_from_runs(input.paragraph(), text_len, [run])?;
            Ok(ParagraphPreparationOutput::new(
                paragraph,
                PreparationWork::new(true, true, 1, 1),
            ))
        }
    }

    #[test]
    fn layout_rejects_adapter_ranges_inside_a_utf8_scalar() {
        let (document, styles, paint) = one_leaf_document(*b"scene-test-doc01", "é");
        let mut layout = LayoutEngine::new(EchoAdapter { split_utf8: true });
        let request = SceneRequest::new(
            FiniteWidth::new(100.).expect("test width is valid"),
            &styles,
            &paint,
        );
        let error = layout
            .prepare(&document.snapshot(), &request)
            .expect_err("mid-scalar adapter source must be rejected");
        assert_eq!(
            error.kind(),
            SceneErrorKind::SourceCoverage,
            "invalid UTF-8 coverage must be a source-coverage error"
        );
        assert!(
            error.paragraph().is_some(),
            "source-coverage diagnostics must identify the paragraph"
        );
        assert_eq!(
            error.source(),
            Some(1..2),
            "source-coverage diagnostics must retain the invalid range"
        );
    }

    #[test]
    fn fragment_identity_is_distinct_across_documents() {
        let (first, first_styles, first_paint) = one_leaf_document(*b"scene-test-doc02", "a");
        let (second, second_styles, second_paint) = one_leaf_document(*b"scene-test-doc03", "b");
        let mut layout = LayoutEngine::new(EchoAdapter { split_utf8: false });
        let width = FiniteWidth::new(100.).expect("test width is valid");
        let first_request = SceneRequest::new(width, &first_styles, &first_paint);
        let first_scene = layout
            .prepare(&first.snapshot(), &first_request)
            .expect("first scene must prepare");
        let second_request = SceneRequest::new(width, &second_styles, &second_paint);
        let second_scene = layout
            .prepare(&second.snapshot(), &second_request)
            .expect("second scene must prepare");
        assert_ne!(
            first_scene.scene().fragments()[0].id(),
            second_scene.scene().fragments()[0].id(),
            "document identity must participate in retained fragment identity"
        );
    }

    fn one_leaf_document(identity: [u8; 16], text: &str) -> (Document, StyleMap, PaintTable) {
        let mut document = Document::new(DocumentId::from_bytes(identity));
        let mut edit = document.edit();
        let paragraph = edit
            .append_paragraph(ParagraphRole::BODY)
            .expect("test paragraph must append");
        edit.append_text(paragraph, InlineRole::TEXT, text)
            .expect("test text must append");
        edit.commit().expect("test document must commit");
        let styles = StyleMap::new(ComputedInlineStyle::new(
            ShapingStyle::new(16.).expect("test style must be valid"),
            InlineFlowStyle::default(),
            PaintSlot::new(0),
        ));
        let paint = PaintTable::from_brushes([Brush::Solid(Color::BLACK)]);
        (document, styles, paint)
    }
}
