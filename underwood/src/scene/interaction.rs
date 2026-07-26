// Copyright 2026 the Underwood Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Immutable scene interaction over committed and projected text.
//!
//! This module owns hit testing, cursor movement, and selection geometry; it
//! explicitly does not own visual record representation or scene preparation.

use super::*;
use crate::adapter::{ClusterBoundary, ClusterWhitespace};

/// Immutable renderer-neutral scene for one generated composition epoch.
#[derive(Clone, Debug)]
pub struct CompositionScene {
    pub(super) document: crate::DocumentId,
    pub(super) revision: DocumentRevision,
    pub(super) composition: CompositionId,
    pub(super) epoch: crate::CompositionEpoch,
    pub(super) paint: PaintTable,
    pub(super) core: Arc<SceneCore>,
}

impl CompositionScene {
    /// Returns exact intrinsic metrics for this projected scene.
    #[must_use]
    pub fn metrics(&self) -> TextMetrics {
        self.core.metrics
    }

    /// Returns the immutable document identity below the transient projection.
    #[must_use]
    pub const fn document(&self) -> crate::DocumentId {
        self.document
    }

    /// Returns the immutable base revision below the transient projection.
    #[must_use]
    pub const fn revision(&self) -> DocumentRevision {
        self.revision
    }

    /// Returns the native composition identity.
    #[must_use]
    pub const fn composition(&self) -> CompositionId {
        self.composition
    }

    /// Returns the exact transient epoch represented by this scene.
    #[must_use]
    pub const fn epoch(&self) -> crate::CompositionEpoch {
        self.epoch
    }

    /// Returns visual lines in flow order.
    #[must_use]
    pub fn lines(&self) -> ProjectedSceneLines<'_> {
        ProjectedSceneLines::new(self.revision, &self.core.spine)
    }

    /// Returns one visual line by its global flow-order index.
    #[must_use]
    pub fn line(&self, index: usize) -> Option<ProjectedSceneLineView<'_>> {
        self.lines().get(index)
    }

    /// Returns the number of visual lines.
    #[must_use]
    pub fn line_count(&self) -> usize {
        self.core.spine.summary().lines
    }

    /// Returns paint-homogeneous projected glyph fragments.
    #[must_use]
    pub fn fragments(&self) -> ProjectedSceneFragments<'_> {
        ProjectedSceneFragments::new(self.revision, &self.core.spine)
    }

    /// Returns one projected glyph fragment by its global visual index.
    #[must_use]
    pub fn fragment(&self, index: usize) -> Option<ProjectedSceneFragmentView<'_>> {
        self.fragments().get(index)
    }

    /// Returns the number of projected paint fragments.
    #[must_use]
    pub fn fragment_count(&self) -> usize {
        self.core.spine.summary().fragments
    }

    /// Returns immutable paint values referenced by fragment slots.
    #[must_use]
    pub const fn paint(&self) -> &PaintTable {
        &self.paint
    }

    /// Iterates semantic fragments in document order.
    pub fn semantics(&self) -> SceneSemantics<'_> {
        SceneSemantics::new(self.revision, &self.core.spine)
    }

    /// Returns the exact projected interaction unit under a point.
    #[must_use]
    pub fn hit_test(
        &self,
        point: Point,
    ) -> Option<TextHit<ProjectedTextRange, ProjectedTextPosition>> {
        if self.core.spine.is_normal_flow() {
            let positioned = self.core.spine.positioned_segment_at_block(point.y)?;
            return positioned
                .segment
                .geometry
                .clusters
                .iter()
                .find(|cluster| {
                    let origin = Vec2::new(0.0, positioned.position.block_origin);
                    (cluster.bounds + origin).contains(point)
                })
                .map(|cluster| cached_projected_hit(cluster, positioned, self.revision, point));
        }
        self.positioned_clusters()
            .find_map(|(positioned, cluster)| {
                let origin = Vec2::new(0.0, positioned.position.block_origin);
                (cluster.bounds + origin)
                    .contains(point)
                    .then(|| cached_projected_hit(cluster, positioned, self.revision, point))
            })
    }

    /// Returns the closest projected interaction-unit side for native point queries.
    #[must_use]
    pub fn hit_test_closest(
        &self,
        point: Point,
    ) -> Option<TextHit<ProjectedTextRange, ProjectedTextPosition>> {
        if self.core.spine.is_normal_flow() {
            let positioned = self.core.spine.positioned_segment_at_block(point.y)?;
            return closest_cluster(positioned, point)
                .map(|cluster| cached_projected_hit(cluster, positioned, self.revision, point));
        }
        let mut closest: Option<(PositionedSegment<'_>, &CachedCluster, f64, f64)> = None;
        for (positioned, cluster) in self.positioned_clusters() {
            let bounds = cluster.bounds + Vec2::new(0.0, positioned.position.block_origin);
            let (block_distance, inline_distance) = distance_to_rect_axes(point, bounds);
            if closest.is_none_or(|(_, _, current_block, current_inline)| {
                block_distance < current_block
                    || (block_distance == current_block && inline_distance < current_inline)
            }) {
                closest = Some((positioned, cluster, block_distance, inline_distance));
            }
        }
        closest.map(|(positioned, cluster, _, _)| {
            cached_projected_hit(cluster, positioned, self.revision, point)
        })
    }

    /// Resolves exact scene geometry for one projected caret position.
    #[must_use]
    pub fn caret(
        &self,
        position: &ProjectedTextPosition,
    ) -> Option<SceneCaret<ProjectedTextPosition>> {
        if let ProjectedTextPosition::Snapshot(position) = position {
            let positioned = positioned_snapshot_segment(&self.core.spine, position.text())?;
            return positioned.segment.geometry.carets.iter().find_map(|caret| {
                let candidate = projected_position(caret.position, self.revision);
                (candidate == ProjectedTextPosition::Snapshot(*position)).then(|| SceneCaret {
                    position: candidate,
                    bounds: caret.bounds + Vec2::new(0.0, positioned.position.block_origin),
                })
            });
        }
        self.positioned_carets().find_map(|(positioned, caret)| {
            let candidate = projected_position(caret.position, self.revision);
            (candidate == *position).then(|| SceneCaret {
                position: candidate,
                bounds: caret.bounds + Vec2::new(0.0, positioned.position.block_origin),
            })
        })
    }

    /// Moves one position through the adapter-produced interaction map.
    #[must_use]
    pub fn move_position(
        &self,
        position: &ProjectedTextPosition,
        movement: TextMovement,
    ) -> Option<ProjectedTextPosition> {
        let record = match position {
            ProjectedTextPosition::Snapshot(position) => {
                let positioned = positioned_snapshot_segment(&self.core.spine, position.text())?;
                positioned
                    .segment
                    .geometry
                    .movements
                    .iter()
                    .find(|record| {
                        projected_position(record.position, self.revision)
                            == ProjectedTextPosition::Snapshot(*position)
                    })?
            }
            ProjectedTextPosition::Composition(_) => self
                .positioned_movements()
                .map(|(_, record)| record)
                .find(|record| projected_position(record.position, self.revision) == *position)?,
        };
        let step = match movement {
            TextMovement::PreviousVisual => record.previous_visual.as_ref(),
            TextMovement::NextVisual => record.next_visual.as_ref(),
            TextMovement::PreviousLogical => record.previous_logical.as_ref(),
            TextMovement::NextLogical => record.next_logical.as_ref(),
        }?;
        Some(projected_position(step.target, self.revision))
    }

    /// Resolves highlight rectangles for the selected range inside preedit.
    pub fn composition_selection_geometry(
        &self,
        session: &CompositionSession,
    ) -> Result<Vec<SceneCompositionRect>, CompositionError> {
        self.validate_composition_session(session)?;
        let Some(selection) = session.selection() else {
            return Ok(Vec::new());
        };
        Ok(self.composition_range_geometry(selection))
    }

    /// Resolves visual rectangles covering the complete generated preedit.
    ///
    /// This is the renderer-neutral marked-text geometry. Native hosts can use
    /// it for underlines or backgrounds without approximating the preedit from
    /// glyph ink. The supplied session must name this exact composition epoch.
    pub fn composition_geometry(
        &self,
        session: &CompositionSession,
    ) -> Result<Vec<SceneCompositionRect>, CompositionError> {
        self.validate_composition_session(session)?;
        let end = u32::try_from(session.text().len())
            .map_err(|_| CompositionError::new(CompositionErrorKind::InvalidPreeditRange))?;
        Ok(self.composition_range_geometry(0..end))
    }

    fn validate_composition_session(
        &self,
        session: &CompositionSession,
    ) -> Result<(), CompositionError> {
        if session.document() != self.document
            || session.base_revision() != self.revision
            || session.id() != self.composition
            || session.epoch() != self.epoch
        {
            return Err(CompositionError::new(CompositionErrorKind::WrongSnapshot));
        }
        Ok(())
    }

    fn composition_range_geometry(&self, bytes: Range<u32>) -> Vec<SceneCompositionRect> {
        let mut geometry: Vec<SceneCompositionRect> = Vec::new();
        for (positioned, cluster) in self.positioned_clusters() {
            if !cluster.sources.iter().any(|source| {
                matches!(source, LocalRange::Composition { id, epoch, bytes: source }
                    if *id == self.composition
                        && *epoch == self.epoch
                        && source.start < bytes.end
                        && bytes.start < source.end)
            }) {
                continue;
            }
            let line = positioned.position.line_base + cluster.line;
            let bounds = cluster.bounds + Vec2::new(0.0, positioned.position.block_origin);
            if let Some(previous) = geometry.last_mut()
                && previous.line == line
                && previous.bidi_level == cluster.bidi_level
                && nearly_equal(previous.bounds.x1, bounds.x0)
            {
                previous.bounds.x1 = bounds.x1;
            } else {
                geometry.push(SceneCompositionRect {
                    line,
                    bounds,
                    bidi_level: cluster.bidi_level,
                });
            }
        }
        geometry
    }

    pub(crate) fn range_geometry(&self, range: &ProjectedTextRange) -> Vec<(usize, Rect)> {
        self.positioned_clusters()
            .filter(|(_, cluster)| {
                cluster
                    .sources
                    .iter()
                    .any(|source| local_range_overlaps_projected(source, range, self.revision))
            })
            .map(|(positioned, cluster)| {
                (
                    positioned.position.line_base + cluster.line,
                    cluster.bounds + Vec2::new(0.0, positioned.position.block_origin),
                )
            })
            .collect()
    }

    fn positioned_clusters(&self) -> impl Iterator<Item = (PositionedSegment<'_>, &CachedCluster)> {
        self.core.spine.segments().flat_map(|positioned| {
            positioned
                .segment
                .geometry
                .clusters
                .iter()
                .map(move |cluster| (positioned, cluster))
        })
    }

    fn positioned_carets(&self) -> impl Iterator<Item = (PositionedSegment<'_>, &CachedCaret)> {
        self.core.spine.segments().flat_map(|positioned| {
            positioned
                .segment
                .geometry
                .carets
                .iter()
                .map(move |caret| (positioned, caret))
        })
    }

    fn positioned_movements(
        &self,
    ) -> impl Iterator<Item = (PositionedSegment<'_>, &CachedCursorMovement)> {
        self.core.spine.segments().flat_map(|positioned| {
            positioned
                .segment
                .geometry
                .movements
                .iter()
                .map(move |movement| (positioned, movement))
        })
    }
}

/// Immutable renderer-neutral text scene.
#[derive(Clone, Debug)]
pub struct TextScene {
    pub(super) document: crate::DocumentId,
    pub(super) revision: DocumentRevision,
    pub(super) paint: PaintTable,
    pub(super) core: Arc<SceneCore>,
}

#[derive(Debug)]
pub(super) struct SceneCore {
    pub(super) paragraph_count: usize,
    pub(super) spine: SceneSpine,
    pub(super) metrics: TextMetrics,
    pub(super) region: Option<SceneRegionBinding>,
}

impl TextScene {
    /// Returns exact intrinsic metrics for this scene.
    #[must_use]
    pub fn metrics(&self) -> TextMetrics {
        self.core.metrics
    }

    /// Returns the document identity represented by this scene.
    #[must_use]
    pub const fn document(&self) -> crate::DocumentId {
        self.document
    }

    /// Returns the exact immutable snapshot revision represented by this scene.
    #[must_use]
    pub const fn revision(&self) -> DocumentRevision {
        self.revision
    }

    /// Starts one native composition over the current primary insertion point.
    ///
    /// Native composition protocols expose one marked region. A sole logical
    /// selection becomes that replacement target. If the scene has several
    /// independent selections, or the primary visual selection has several
    /// disjoint logical ranges, the host-visible set is explicitly normalized
    /// to one collapsed primary extent before composition starts. Callers can
    /// observe that normalization through [`CompositionStart::selection_changed`].
    pub fn begin_composition(
        &self,
        selections: &SnapshotTextSelectionSet,
        id: CompositionId,
    ) -> Result<CompositionStart, CompositionError> {
        if selections.document() != self.document || selections.revision() != self.revision {
            return Err(CompositionError::new(CompositionErrorKind::WrongSnapshot));
        }
        let Some(primary) = selections.primary() else {
            return Err(CompositionError::new(
                CompositionErrorKind::EmptySelectionSet,
            ));
        };
        let normalized = if selections.selections().len() == 1 && primary.ranges().len() == 1 {
            self.selection_set([primary.clone()])
        } else {
            self.collapsed_selection(primary.extent())
                .and_then(|selection| self.selection_set([selection]))
        }
        .map_err(|_| CompositionError::new(CompositionErrorKind::WrongSnapshot))?;
        let selection_changed = &normalized != selections;
        Ok(CompositionStart::new(
            CompositionSession::new(id, normalized.clone()),
            normalized,
            selection_changed,
        ))
    }

    /// Returns an empty selection set bound to this scene revision.
    #[must_use]
    pub fn empty_selection_set(&self) -> SnapshotTextSelectionSet {
        SnapshotTextSelectionSet::new(self.document, self.revision, Vec::new())
    }

    /// Creates one collapsed selection at an exact scene position.
    pub fn collapsed_selection(
        &self,
        position: &SnapshotTextPosition,
    ) -> Result<SnapshotTextSelection, SelectionError> {
        self.validate_position(position)?;
        Ok(SnapshotTextSelection::new(
            *position,
            *position,
            TextSelectionMode::Logical,
            alloc::vec![SnapshotTextRange::new(
                self.revision,
                position.text(),
                position.byte()..position.byte(),
            )],
        ))
    }

    /// Creates one logical or visual selection between two exact positions.
    ///
    /// A visual selection follows adapter-owned caret transitions and can
    /// expose several noncontiguous logical ranges across bidi boundaries.
    pub fn selection(
        &self,
        anchor: &SnapshotTextPosition,
        extent: &SnapshotTextPosition,
        mode: TextSelectionMode,
    ) -> Result<SnapshotTextSelection, SelectionError> {
        self.validate_position(anchor)?;
        self.validate_position(extent)?;
        let ranges = match mode {
            TextSelectionMode::Logical => self.logical_ranges(anchor, extent)?,
            TextSelectionMode::Visual => self.visual_ranges(anchor, extent)?,
        };
        Ok(SnapshotTextSelection::new(*anchor, *extent, mode, ranges))
    }

    /// Validates and collects independent selections for this scene.
    pub fn selection_set(
        &self,
        selections: impl IntoIterator<Item = SnapshotTextSelection>,
    ) -> Result<SnapshotTextSelectionSet, SelectionError> {
        let selections: Vec<_> = selections.into_iter().collect();
        for selection in &selections {
            let expected =
                self.selection(selection.anchor(), selection.extent(), selection.mode())?;
            if expected.ranges() != selection.ranges() {
                return Err(SelectionError::new(SelectionErrorKind::UnknownPosition));
            }
        }
        validate_independent_selections(&selections)?;
        Ok(SnapshotTextSelectionSet::new(
            self.document,
            self.revision,
            selections,
        ))
    }

    /// Moves every independent selection through the exact scene cursor map.
    ///
    /// When `extend` is true, each anchor is retained and the extent is moved.
    /// Otherwise a noncollapsed selection first collapses toward the requested
    /// direction and a collapsed selection advances by one interaction unit.
    pub fn move_selections(
        &self,
        selections: &SnapshotTextSelectionSet,
        movement: TextMovement,
        extend: bool,
    ) -> Result<SnapshotTextSelectionSet, SelectionError> {
        if selections.document() != self.document || selections.revision() != self.revision {
            return Err(SelectionError::new(SelectionErrorKind::WrongSnapshot));
        }
        let mode = movement_mode(movement);
        let mut moved = Vec::with_capacity(selections.selections().len());
        for selection in selections.selections() {
            let next = if !extend && !selection.is_collapsed() {
                self.collapse_for_movement(selection, movement)?
            } else {
                let extent = self
                    .cursor_step(selection.extent(), movement)?
                    .map_or(*selection.extent(), |step| step.target);
                if extend {
                    self.selection(selection.anchor(), &extent, mode)?
                } else {
                    self.collapsed_selection(&extent)?
                }
            };
            moved.push(next);
        }
        self.selection_set(moved)
    }

    /// Resolves visual highlight rectangles for a complete selection set.
    pub fn selection_geometry(
        &self,
        selections: &SnapshotTextSelectionSet,
    ) -> Result<Vec<SceneSelectionRect>, SelectionError> {
        if selections.document() != self.document || selections.revision() != self.revision {
            return Err(SelectionError::new(SelectionErrorKind::WrongSnapshot));
        }
        let mut geometry: Vec<SceneSelectionRect> = Vec::new();
        for (selection_index, selection) in selections.selections().iter().enumerate() {
            for (positioned, cluster) in self.positioned_clusters() {
                let Some((range_index, _)) =
                    selection.ranges().iter().enumerate().find(|(_, range)| {
                        cluster.sources.iter().any(|source| {
                            ranges_overlap(range, &materialize_range(source, self.revision))
                        })
                    })
                else {
                    continue;
                };
                let line = positioned.position.line_base + cluster.line;
                let bounds = cluster.bounds + Vec2::new(0.0, positioned.position.block_origin);
                if let Some(previous) = geometry.last_mut()
                    && previous.selection == selection_index
                    && previous.range == range_index
                    && previous.line == line
                    && previous.bidi_level == cluster.bidi_level
                    && nearly_equal(previous.bounds.x1, bounds.x0)
                {
                    previous.bounds.x1 = bounds.x1;
                } else {
                    geometry.push(SceneSelectionRect {
                        selection: selection_index,
                        range: range_index,
                        line,
                        bounds,
                        bidi_level: cluster.bidi_level,
                    });
                }
            }
        }
        Ok(geometry)
    }

    /// Returns visual lines in flow order.
    #[must_use]
    pub fn lines(&self) -> SceneLines<'_> {
        SceneLines::new(self.revision, &self.core.spine)
    }

    /// Returns one visual line by global index.
    #[must_use]
    pub fn line(&self, index: usize) -> Option<SceneLineView<'_>> {
        self.lines().get(index)
    }

    /// Returns the number of visual lines.
    #[must_use]
    pub fn line_count(&self) -> usize {
        self.core.spine.summary().lines
    }

    /// Returns paint-homogeneous glyph fragments in visual order.
    #[must_use]
    pub fn fragments(&self) -> SceneFragments<'_> {
        SceneFragments::new(self.revision, &self.core.spine)
    }

    /// Returns one paint-homogeneous fragment by global visual index.
    #[must_use]
    pub fn fragment(&self, index: usize) -> Option<SceneFragmentView<'_>> {
        self.fragments().get(index)
    }

    /// Returns the number of paint fragments.
    #[must_use]
    pub fn fragment_count(&self) -> usize {
        self.core.spine.summary().fragments
    }

    /// Returns immutable paint values referenced by fragment slots.
    #[must_use]
    pub const fn paint(&self) -> &PaintTable {
        &self.paint
    }

    /// Iterates semantic fragments in document order.
    pub fn semantics(&self) -> SceneSemantics<'_> {
        SceneSemantics::new(self.revision, &self.core.spine)
    }

    /// Returns an exact interaction-unit hit under a scene-space point.
    ///
    /// Unlike selection hit testing, this does not clamp points outside unit
    /// geometry to the nearest line edge.
    #[must_use]
    pub fn hit_test(&self, point: Point) -> Option<TextHit> {
        if self.core.spine.is_normal_flow() {
            let positioned = self.core.spine.positioned_segment_at_block(point.y)?;
            return positioned
                .segment
                .geometry
                .clusters
                .iter()
                .find(|cluster| {
                    let origin = Vec2::new(0.0, positioned.position.block_origin);
                    (cluster.bounds + origin).contains(point)
                })
                .map(|cluster| cached_snapshot_hit(cluster, positioned, self.revision, point));
        }
        self.positioned_clusters()
            .find_map(|(positioned, cluster)| {
                let origin = Vec2::new(0.0, positioned.position.block_origin);
                (cluster.bounds + origin)
                    .contains(point)
                    .then(|| cached_snapshot_hit(cluster, positioned, self.revision, point))
            })
    }

    /// Returns the closest interaction-unit side for pointer selection.
    ///
    /// This includes whitespace and empty editable text which may have no
    /// painted glyph fragment.
    #[must_use]
    pub fn hit_test_closest(&self, point: Point) -> Option<TextHit> {
        if self.core.spine.is_normal_flow() {
            let positioned = self.core.spine.positioned_segment_at_block(point.y)?;
            return closest_cluster(positioned, point)
                .map(|cluster| cached_snapshot_hit(cluster, positioned, self.revision, point));
        }
        let mut closest: Option<(PositionedSegment<'_>, &CachedCluster, f64, f64)> = None;
        for (positioned, cluster) in self.positioned_clusters() {
            let bounds = cluster.bounds + Vec2::new(0.0, positioned.position.block_origin);
            let (block_distance, inline_distance) = distance_to_rect_axes(point, bounds);
            if closest.is_none_or(|(_, _, current_block, current_inline)| {
                block_distance < current_block
                    || (block_distance == current_block && inline_distance < current_inline)
            }) {
                closest = Some((positioned, cluster, block_distance, inline_distance));
            }
        }
        closest.map(|(positioned, cluster, _, _)| {
            cached_snapshot_hit(cluster, positioned, self.revision, point)
        })
    }

    /// Resolves exact scene-space caret geometry for a snapshot position.
    ///
    /// Returns `None` for a stale revision, foreign text leaf, invalid
    /// affinity, or a valid snapshot position not represented by this scene.
    #[must_use]
    pub fn caret(&self, position: &SnapshotTextPosition) -> Option<SceneCaret> {
        if position.revision() != self.revision {
            return None;
        }
        let positioned = positioned_snapshot_segment(&self.core.spine, position.text())?;
        positioned.segment.geometry.carets.iter().find_map(|caret| {
            let candidate = materialize_position(caret.position, self.revision);
            (candidate == *position).then(|| SceneCaret {
                position: candidate,
                bounds: caret.bounds + Vec2::new(0.0, positioned.position.block_origin),
            })
        })
    }

    /// Returns the first logical caret position in the complete scene.
    ///
    /// This follows Underwood's cross-paragraph movement graph rather than
    /// treating every paragraph-local start as a document start.
    #[must_use]
    pub fn start_position(&self) -> Option<SnapshotTextPosition> {
        let first = self.core.spine.positioned_movement(0)?;
        first
            .position
            .segment
            .geometry
            .movements
            .iter()
            .filter(|movement| movement.previous_logical.is_none())
            .map(|movement| materialize_position(movement.position, self.revision))
            .min_by_key(logical_position_key)
    }

    /// Returns the final logical caret position in the complete scene.
    ///
    /// This follows Underwood's cross-paragraph movement graph rather than
    /// treating every paragraph-local end as a document end.
    #[must_use]
    pub fn end_position(&self) -> Option<SnapshotTextPosition> {
        let last = self
            .core
            .spine
            .summary()
            .movements
            .checked_sub(1)
            .and_then(|index| self.core.spine.positioned_movement(index))?;
        last.position
            .segment
            .geometry
            .movements
            .iter()
            .filter(|movement| movement.next_logical.is_none())
            .map(|movement| materialize_position(movement.position, self.revision))
            .max_by_key(logical_position_key)
    }

    /// Resolves a represented caret at one leaf-local UTF-8 boundary.
    ///
    /// This never fabricates a caret at an interior UTF-8 byte or at a
    /// semantic leaf seam inside one shaped grapheme. At a soft wrap or bidi
    /// discontinuity more than one affinity can share the byte boundary. The
    /// leaf start prefers downstream affinity and every other boundary
    /// prefers upstream affinity, then falls back to either represented stop.
    #[must_use]
    pub fn position_at(&self, text: TextId, byte: u32) -> Option<SnapshotTextPosition> {
        if text.document != self.document {
            return None;
        }
        let positioned = positioned_snapshot_segment(&self.core.spine, text)?;
        let preferred = if byte == 0 {
            TextAffinity::Downstream
        } else {
            TextAffinity::Upstream
        };
        let mut fallback = None;
        for movement in &positioned.segment.geometry.movements {
            let position = materialize_position(movement.position, self.revision);
            if position.text() == text && position.byte() == byte {
                if position.affinity() == preferred {
                    return Some(position);
                }
                fallback = Some(position);
            }
        }
        fallback
    }

    /// Returns the preceding logical word start, or the scene start.
    ///
    /// Word starts come from the paragraph adapter's retained Unicode
    /// analysis. A stale, foreign, or unrepresented position returns `None`.
    #[must_use]
    pub fn previous_word_position(
        &self,
        position: &SnapshotTextPosition,
    ) -> Option<SnapshotTextPosition> {
        self.word_position(position, false)
    }

    /// Returns the following logical word start, or the scene end.
    ///
    /// Word starts come from the paragraph adapter's retained Unicode
    /// analysis. A stale, foreign, or unrepresented position returns `None`.
    #[must_use]
    pub fn next_word_position(
        &self,
        position: &SnapshotTextPosition,
    ) -> Option<SnapshotTextPosition> {
        self.word_position(position, true)
    }

    fn validate_position(&self, position: &SnapshotTextPosition) -> Result<(), SelectionError> {
        if position.revision() != self.revision || position.text().document != self.document {
            return Err(SelectionError::new(SelectionErrorKind::WrongSnapshot));
        }
        let Some(positioned) = positioned_snapshot_segment(&self.core.spine, position.text())
        else {
            return Err(SelectionError::new(SelectionErrorKind::UnknownPosition));
        };
        if positioned
            .segment
            .geometry
            .movements
            .iter()
            .any(|movement| materialize_position(movement.position, self.revision) == *position)
        {
            Ok(())
        } else {
            Err(SelectionError::new(SelectionErrorKind::UnknownPosition))
        }
    }

    fn word_position(
        &self,
        position: &SnapshotTextPosition,
        forward: bool,
    ) -> Option<SnapshotTextPosition> {
        self.validate_position(position).ok()?;
        let current = logical_position_key(position);
        let candidates = self
            .positioned_clusters()
            .map(|(_, cluster)| cluster)
            .filter(|cluster| {
                cluster.boundary != ClusterBoundary::None
                    && cluster.whitespace == ClusterWhitespace::None
            })
            .filter_map(|cluster| {
                let source = cluster.sources.first()?;
                let source = materialize_range(source, self.revision);
                let key = logical_text_key(source.text(), source.bytes().start);
                Some((key, source.text(), source.bytes().start))
            })
            .filter(|(key, _, _)| {
                if forward {
                    *key > current
                } else {
                    *key < current
                }
            });
        if forward {
            candidates
                .min_by_key(|(key, _, _)| *key)
                .and_then(|(_, text, byte)| self.position_at(text, byte))
                .or_else(|| self.end_position())
        } else {
            candidates
                .max_by_key(|(key, _, _)| *key)
                .and_then(|(_, text, byte)| self.position_at(text, byte))
                .or_else(|| self.start_position())
        }
    }

    fn logical_ranges(
        &self,
        anchor: &SnapshotTextPosition,
        extent: &SnapshotTextPosition,
    ) -> Result<Vec<SnapshotTextRange>, SelectionError> {
        let anchor_text = self.text_rank(anchor.text())?;
        let extent_text = self.text_rank(extent.text())?;
        let ordering = (anchor_text, anchor.byte()).cmp(&(extent_text, extent.byte()));
        let (start, start_text, end, end_text) = if ordering.is_gt() {
            (extent, extent_text, anchor, anchor_text)
        } else {
            (anchor, anchor_text, extent, extent_text)
        };
        if start_text == end_text && start.byte() == end.byte() {
            return Ok(alloc::vec![SnapshotTextRange::new(
                self.revision,
                extent.text(),
                extent.byte()..extent.byte(),
            )]);
        }
        let mut ranges = Vec::new();
        for index in start_text..=end_text {
            let positioned = self
                .core
                .spine
                .positioned_text(index)
                .expect("validated text rank remains represented");
            let text = materialize_range(
                &positioned.position.segment.geometry.texts[positioned.local],
                self.revision,
            );
            let bytes = if start_text == end_text {
                start.byte()..end.byte()
            } else if index == start_text {
                start.byte()..text.bytes().end
            } else if index == end_text {
                0..end.byte()
            } else {
                text.bytes()
            };
            if !bytes.is_empty() {
                ranges.push(SnapshotTextRange::new(self.revision, text.text(), bytes));
            }
        }
        if ranges.is_empty() {
            ranges.push(SnapshotTextRange::new(
                self.revision,
                extent.text(),
                extent.byte()..extent.byte(),
            ));
        }
        Ok(ranges)
    }

    fn visual_ranges(
        &self,
        anchor: &SnapshotTextPosition,
        extent: &SnapshotTextPosition,
    ) -> Result<Vec<SnapshotTextRange>, SelectionError> {
        if anchor == extent {
            return Ok(alloc::vec![SnapshotTextRange::new(
                self.revision,
                extent.text(),
                extent.byte()..extent.byte(),
            )]);
        }
        let mut ranges = None;
        for (start, end, movement) in [
            (anchor, extent, TextMovement::NextVisual),
            (anchor, extent, TextMovement::PreviousVisual),
            (extent, anchor, TextMovement::NextVisual),
            (extent, anchor, TextMovement::PreviousVisual),
        ] {
            if let Some(found) = self.walk_visual_ranges(start, end, movement)? {
                ranges = Some(found);
                break;
            }
        }
        let Some(mut ranges) = ranges else {
            return Err(SelectionError::new(
                SelectionErrorKind::DisconnectedMovement,
            ));
        };
        canonicalize_ranges(&mut ranges);
        if ranges.is_empty() {
            ranges.push(SnapshotTextRange::new(
                self.revision,
                extent.text(),
                extent.byte()..extent.byte(),
            ));
        }
        Ok(ranges)
    }

    fn walk_visual_ranges(
        &self,
        start: &SnapshotTextPosition,
        end: &SnapshotTextPosition,
        movement: TextMovement,
    ) -> Result<Option<Vec<SnapshotTextRange>>, SelectionError> {
        let mut position = *start;
        let mut ranges = Vec::new();
        for _ in 0..=self.core.spine.summary().movements {
            if position == *end {
                return Ok(Some(ranges));
            }
            let Some(step) = self.cursor_step(&position, movement)? else {
                return Ok(None);
            };
            if let Some(source) = step.source {
                ranges.extend(source.sources().iter().cloned());
            }
            position = step.target;
        }
        Ok(None)
    }

    fn cursor_step(
        &self,
        position: &SnapshotTextPosition,
        movement: TextMovement,
    ) -> Result<Option<SceneCursorStep>, SelectionError> {
        self.validate_position(position)?;
        let positioned = positioned_snapshot_segment(&self.core.spine, position.text())
            .ok_or_else(|| SelectionError::new(SelectionErrorKind::UnknownPosition))?;
        let record = positioned
            .segment
            .geometry
            .movements
            .iter()
            .find(|record| materialize_position(record.position, self.revision) == *position)
            .ok_or_else(|| SelectionError::new(SelectionErrorKind::UnknownPosition))?;
        let step = match movement {
            TextMovement::PreviousVisual => record.previous_visual.as_ref(),
            TextMovement::NextVisual => record.next_visual.as_ref(),
            TextMovement::PreviousLogical => record.previous_logical.as_ref(),
            TextMovement::NextLogical => record.next_logical.as_ref(),
        };
        let step = materialize_cursor_step(step, self.revision);
        Ok(step.or_else(|| self.adjacent_paragraph_step(position, movement)))
    }

    fn adjacent_paragraph_step(
        &self,
        position: &SnapshotTextPosition,
        movement: TextMovement,
    ) -> Option<SceneCursorStep> {
        let previous = matches!(
            movement,
            TextMovement::PreviousVisual | TextMovement::PreviousLogical
        );
        let current = positioned_snapshot_segment(&self.core.spine, position.text())?;
        let global = if previous {
            current.position.movement_base.checked_sub(1)?
        } else {
            current
                .position
                .movement_base
                .saturating_add(current.segment.geometry.movements.len())
        };
        let adjacent = self.core.spine.positioned_movement(global)?;
        let mut candidates = adjacent
            .position
            .segment
            .geometry
            .movements
            .iter()
            .filter(|record| match movement {
                TextMovement::PreviousVisual => record.next_visual.is_none(),
                TextMovement::NextVisual => record.previous_visual.is_none(),
                TextMovement::PreviousLogical => record.next_logical.is_none(),
                TextMovement::NextLogical => record.previous_logical.is_none(),
            });
        let target = materialize_position(candidates.next()?.position, self.revision);
        if candidates.next().is_some() {
            return None;
        }
        Some(SceneCursorStep {
            target,
            source: None,
        })
    }

    fn collapse_for_movement(
        &self,
        selection: &SnapshotTextSelection,
        movement: TextMovement,
    ) -> Result<SnapshotTextSelection, SelectionError> {
        let anchor = *selection.anchor();
        let extent = *selection.extent();
        let choose_anchor = match movement {
            TextMovement::PreviousVisual | TextMovement::NextVisual => {
                let anchor_before = self.visual_ordering(&anchor, &extent)?.is_lt();
                matches!(movement, TextMovement::PreviousVisual) == anchor_before
            }
            TextMovement::PreviousLogical | TextMovement::NextLogical => {
                let anchor_before = self.compare_positions(&anchor, &extent)?.is_lt();
                matches!(movement, TextMovement::PreviousLogical) == anchor_before
            }
        };
        self.collapsed_selection(if choose_anchor { &anchor } else { &extent })
    }

    fn visual_ordering(
        &self,
        first: &SnapshotTextPosition,
        second: &SnapshotTextPosition,
    ) -> Result<core::cmp::Ordering, SelectionError> {
        if first == second {
            return Ok(core::cmp::Ordering::Equal);
        }
        if self.can_reach_visual(first, second, TextMovement::NextVisual)? {
            return Ok(core::cmp::Ordering::Less);
        }
        if self.can_reach_visual(first, second, TextMovement::PreviousVisual)? {
            return Ok(core::cmp::Ordering::Greater);
        }
        if self.can_reach_visual(second, first, TextMovement::NextVisual)? {
            return Ok(core::cmp::Ordering::Greater);
        }
        if self.can_reach_visual(second, first, TextMovement::PreviousVisual)? {
            return Ok(core::cmp::Ordering::Less);
        }
        Err(SelectionError::new(
            SelectionErrorKind::DisconnectedMovement,
        ))
    }

    fn can_reach_visual(
        &self,
        start: &SnapshotTextPosition,
        end: &SnapshotTextPosition,
        movement: TextMovement,
    ) -> Result<bool, SelectionError> {
        let mut position = *start;
        for _ in 0..=self.core.spine.summary().movements {
            if position == *end {
                return Ok(true);
            }
            let Some(step) = self.cursor_step(&position, movement)? else {
                return Ok(false);
            };
            position = step.target;
        }
        Ok(false)
    }

    fn compare_positions(
        &self,
        first: &SnapshotTextPosition,
        second: &SnapshotTextPosition,
    ) -> Result<core::cmp::Ordering, SelectionError> {
        Ok((self.text_rank(first.text())?, first.byte())
            .cmp(&(self.text_rank(second.text())?, second.byte())))
    }

    fn text_rank(&self, text: TextId) -> Result<usize, SelectionError> {
        let positioned = positioned_snapshot_segment(&self.core.spine, text)
            .ok_or_else(|| SelectionError::new(SelectionErrorKind::UnknownPosition))?;
        positioned
            .segment
            .geometry
            .texts
            .iter()
            .position(|range| {
                matches!(range, LocalRange::Snapshot { text: candidate, .. } if *candidate == text)
            })
            .map(|local| positioned.position.text_base.saturating_add(local))
            .ok_or_else(|| SelectionError::new(SelectionErrorKind::UnknownPosition))
    }

    pub(crate) fn range_geometry(&self, range: &SnapshotTextRange) -> Vec<(usize, Rect)> {
        self.positioned_clusters()
            .filter(|(_, cluster)| {
                cluster
                    .sources
                    .iter()
                    .any(|source| ranges_overlap(range, &materialize_range(source, self.revision)))
            })
            .map(|(positioned, cluster)| {
                (
                    positioned.position.line_base + cluster.line,
                    cluster.bounds + Vec2::new(0.0, positioned.position.block_origin),
                )
            })
            .collect()
    }

    fn positioned_clusters(&self) -> impl Iterator<Item = (PositionedSegment<'_>, &CachedCluster)> {
        self.core.spine.segments().flat_map(|positioned| {
            positioned
                .segment
                .geometry
                .clusters
                .iter()
                .map(move |cluster| (positioned, cluster))
        })
    }
}

fn logical_position_key(position: &SnapshotTextPosition) -> (u32, u32, u32) {
    logical_text_key(position.text(), position.byte())
}

fn logical_text_key(text: TextId, byte: u32) -> (u32, u32, u32) {
    (text.paragraph, text.index, byte)
}

fn local_range_overlaps_projected(
    local: &LocalRange,
    projected: &ProjectedTextRange,
    revision: DocumentRevision,
) -> bool {
    projected
        .sources()
        .iter()
        .any(|source| match (local, source) {
            (LocalRange::Snapshot { text, bytes }, ProjectedTextSource::Snapshot(projected)) => {
                projected.revision() == revision
                    && projected.text() == *text
                    && bytes.start < projected.bytes().end
                    && projected.bytes().start < bytes.end
            }
            (
                LocalRange::Composition { id, epoch, bytes },
                ProjectedTextSource::Composition(projected),
            ) => {
                projected.id() == *id
                    && projected.epoch() == *epoch
                    && bytes.start < projected.bytes().end
                    && projected.bytes().start < bytes.end
            }
            _ => false,
        })
}

fn movement_mode(movement: TextMovement) -> TextSelectionMode {
    match movement {
        TextMovement::PreviousVisual | TextMovement::NextVisual => TextSelectionMode::Visual,
        TextMovement::PreviousLogical | TextMovement::NextLogical => TextSelectionMode::Logical,
    }
}

fn canonicalize_ranges(ranges: &mut Vec<SnapshotTextRange>) {
    ranges.sort_by_key(|range| {
        (
            range.text().paragraph,
            range.text().index,
            range.bytes().start,
        )
    });
    let mut canonical: Vec<SnapshotTextRange> = Vec::with_capacity(ranges.len());
    for range in ranges.drain(..) {
        if let Some(previous) = canonical.last_mut()
            && previous.text() == range.text()
            && previous.bytes().end >= range.bytes().start
        {
            let start = previous.bytes().start;
            let end = previous.bytes().end.max(range.bytes().end);
            *previous = SnapshotTextRange::new(previous.revision(), previous.text(), start..end);
        } else {
            canonical.push(range);
        }
    }
    *ranges = canonical;
}

fn validate_independent_selections(
    selections: &[SnapshotTextSelection],
) -> Result<(), SelectionError> {
    for (index, selection) in selections.iter().enumerate() {
        for other in &selections[..index] {
            for range in selection.ranges() {
                for other_range in other.ranges() {
                    if ranges_conflict(range, other_range) {
                        return Err(SelectionError::new(
                            SelectionErrorKind::OverlappingSelections,
                        ));
                    }
                }
            }
        }
    }
    Ok(())
}

fn ranges_conflict(first: &SnapshotTextRange, second: &SnapshotTextRange) -> bool {
    if first.text() != second.text() {
        return false;
    }
    let first = first.bytes();
    let second = second.bytes();
    if first.is_empty() && second.is_empty() {
        first.start == second.start
    } else if first.is_empty() {
        second.start <= first.start && first.start <= second.end
    } else if second.is_empty() {
        first.start <= second.start && second.start <= first.end
    } else {
        first.start < second.end && second.start < first.end
    }
}

fn ranges_overlap(first: &SnapshotTextRange, second: &SnapshotTextRange) -> bool {
    if first.text() != second.text() {
        return false;
    }
    let first = first.bytes();
    let second = second.bytes();
    first.start < second.end && second.start < first.end
}

fn nearly_equal(first: f64, second: f64) -> bool {
    (first - second).abs() <= f64::max(1.0, first.abs().max(second.abs())) * 1.0e-9
}

fn positioned_snapshot_segment(spine: &SceneSpine, text: TextId) -> Option<PositionedSegment<'_>> {
    let positioned = spine.positioned_segment(usize::try_from(text.paragraph).ok()?)?;
    (positioned.segment.paragraph.document == text.document
        && positioned.segment.paragraph.index == text.paragraph)
        .then_some(positioned)
}

fn closest_cluster<'a>(
    positioned: PositionedSegment<'a>,
    point: Point,
) -> Option<&'a CachedCluster> {
    positioned
        .segment
        .geometry
        .clusters
        .iter()
        .min_by(|first, second| {
            let origin = Vec2::new(0.0, positioned.position.block_origin);
            let first = distance_to_rect_axes(point, first.bounds + origin);
            let second = distance_to_rect_axes(point, second.bounds + origin);
            first
                .0
                .total_cmp(&second.0)
                .then_with(|| first.1.total_cmp(&second.1))
        })
}

fn cached_snapshot_hit(
    cluster: &CachedCluster,
    positioned: PositionedSegment<'_>,
    revision: DocumentRevision,
    point: Point,
) -> TextHit {
    let origin = Vec2::new(0.0, positioned.position.block_origin);
    let bounds = cluster.bounds + origin;
    let midpoint = bounds.x0 + bounds.width() * 0.5;
    let semantic_id = cluster
        .hit_slices
        .iter()
        .filter(|slice| slice.x0 < slice.x1)
        .min_by(|first, second| {
            distance_to_interval(point.x, first.x0, first.x1)
                .total_cmp(&distance_to_interval(point.x, second.x0, second.x1))
        })
        .map_or(cluster.semantic_id, |slice| slice.semantic_id);
    TextHit {
        source: materialize_snapshot_unit(&cluster.sources, revision),
        position: materialize_position(
            if point.x <= midpoint {
                cluster.left
            } else {
                cluster.right
            },
            revision,
        ),
        semantic_id,
        bidi_level: cluster.bidi_level,
    }
}

fn cached_projected_hit(
    cluster: &CachedCluster,
    positioned: PositionedSegment<'_>,
    revision: DocumentRevision,
    point: Point,
) -> TextHit<ProjectedTextRange, ProjectedTextPosition> {
    let origin = Vec2::new(0.0, positioned.position.block_origin);
    let bounds = cluster.bounds + origin;
    let midpoint = bounds.x0 + bounds.width() * 0.5;
    let semantic_id = cluster
        .hit_slices
        .iter()
        .filter(|slice| slice.x0 < slice.x1)
        .min_by(|first, second| {
            distance_to_interval(point.x, first.x0, first.x1)
                .total_cmp(&distance_to_interval(point.x, second.x0, second.x1))
        })
        .map_or(cluster.semantic_id, |slice| slice.semantic_id);
    TextHit {
        source: projected_range(&cluster.sources, revision),
        position: projected_position(
            if point.x <= midpoint {
                cluster.left
            } else {
                cluster.right
            },
            revision,
        ),
        semantic_id,
        bidi_level: cluster.bidi_level,
    }
}

#[derive(Clone, Debug)]
pub(super) struct SceneCursorStep<Source = SnapshotTextUnit, Position = SnapshotTextPosition> {
    pub(super) target: Position,
    pub(super) source: Option<Source>,
}

fn distance_to_rect_axes(point: Point, bounds: Rect) -> (f64, f64) {
    let inline = if point.x < bounds.x0 {
        bounds.x0 - point.x
    } else if point.x > bounds.x1 {
        point.x - bounds.x1
    } else {
        0.0
    };
    let block = if point.y < bounds.y0 {
        bounds.y0 - point.y
    } else if point.y > bounds.y1 {
        point.y - bounds.y1
    } else {
        0.0
    };
    (block, inline)
}

fn distance_to_interval(value: f64, start: f64, end: f64) -> f64 {
    if value < start {
        start - value
    } else if value > end {
        value - end
    } else {
        0.0
    }
}
