// Copyright 2026 the Underwood Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Immutable scene interaction over committed and projected text.
//!
//! This module owns hit testing, cursor movement, and selection geometry; it
//! explicitly does not own visual record representation or scene preparation.

use super::*;
use crate::adapter::{ClusterBoundary, ClusterWhitespace};

/// Immutable renderer-neutral scene with a typed provenance model.
#[derive(Clone, Debug)]
pub struct Scene<T = SnapshotTextRange, Identity = ()> {
    pub(super) document: crate::DocumentId,
    pub(super) revision: DocumentRevision,
    pub(super) paint: PaintTable,
    pub(super) requested: SceneFeaturePolicy,
    pub(super) core: Arc<SceneCore>,
    pub(super) identity: Identity,
    pub(super) source: core::marker::PhantomData<fn() -> T>,
}

/// Immutable renderer-neutral text scene.
pub type TextScene = Scene;

/// Immutable renderer-neutral scene for one generated composition epoch.
pub type CompositionScene = Scene<ProjectedTextSource, (CompositionId, crate::CompositionEpoch)>;

impl<T, Identity> Scene<T, Identity> {
    pub(super) const fn new(
        document: crate::DocumentId,
        revision: DocumentRevision,
        paint: PaintTable,
        requested: SceneFeaturePolicy,
        core: Arc<SceneCore>,
        identity: Identity,
    ) -> Self {
        Self {
            document,
            revision,
            paint,
            requested,
            core,
            identity,
            source: core::marker::PhantomData,
        }
    }

    /// Returns exact intrinsic metrics for this scene.
    #[must_use]
    pub fn metrics(&self) -> TextMetrics {
        self.core.metrics
    }

    /// Returns the immutable document identity represented by this scene.
    #[must_use]
    pub const fn document(&self) -> crate::DocumentId {
        self.document
    }

    /// Returns the immutable snapshot revision below this scene.
    #[must_use]
    pub const fn revision(&self) -> DocumentRevision {
        self.revision
    }

    /// Returns the exact capability policy requested for this scene handle.
    #[must_use]
    pub const fn requested_features(&self) -> &SceneFeaturePolicy {
        &self.requested
    }

    /// Returns the capability policy physically resident in this scene.
    #[must_use]
    pub fn resident_features(&self) -> &SceneFeaturePolicy {
        &self.core.resident
    }

    /// Returns aggregate deterministic prepared-scene residency.
    #[must_use]
    pub fn residency(&self) -> SceneResidency {
        SceneResidency::from_spine(&self.core.spine)
    }

    /// Iterates requested and resident capabilities and paragraph byte charges.
    #[must_use]
    pub fn paragraph_residencies(
        &self,
    ) -> impl ExactSizeIterator<Item = ParagraphSceneResidency> + Clone + '_ {
        paragraph_residencies(&self.requested, &self.core.spine)
    }

    /// Returns semantic structure when every represented paragraph retained it.
    pub fn semantics(&self) -> Result<SceneSemantics<'_>, MissingSceneCapability> {
        require_scene_features(
            &self.core,
            &self.requested,
            SceneFeatures::DISPLAY.with_semantics(),
        )?;
        Ok(SceneSemantics::new(self.revision, &self.core.spine))
    }

    /// Returns visual lines in flow order.
    #[must_use]
    pub fn lines(&self) -> SceneLines<'_, T> {
        SceneLines::new(self.revision, &self.core, &self.requested)
    }

    /// Returns one visual line by its global flow-order index.
    #[must_use]
    pub fn line(&self, index: usize) -> Option<SceneLineView<'_, T>> {
        self.core
            .spine
            .positioned_line(index)
            .map(|line| SceneLineView::new(self.revision, &self.requested, line))
    }

    /// Returns the number of visual lines.
    #[must_use]
    pub fn line_count(&self) -> usize {
        self.core.spine.summary().lines
    }

    /// Returns paint-homogeneous glyph fragments.
    #[must_use]
    pub fn fragments(&self) -> SceneFragments<'_, T> {
        SceneFragments::new(self.revision, &self.core, &self.requested)
    }

    /// Returns one glyph fragment by its global visual index.
    #[must_use]
    pub fn fragment(&self, index: usize) -> Option<SceneFragmentView<'_, T>> {
        self.core
            .spine
            .positioned_fragment(index)
            .map(|fragment| SceneFragmentView::new(self.revision, &self.requested, fragment))
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
}

impl Scene<ProjectedTextSource, (CompositionId, crate::CompositionEpoch)> {
    /// Returns the native composition identity.
    #[must_use]
    pub const fn composition(&self) -> CompositionId {
        self.identity.0
    }

    /// Returns the exact transient epoch represented by this scene.
    #[must_use]
    pub const fn epoch(&self) -> crate::CompositionEpoch {
        self.identity.1
    }

    /// Returns exact point interaction over paragraphs that retained it.
    pub fn interaction(&self) -> Result<ProjectedSceneInteraction<'_>, MissingSceneCapability> {
        require_any_scene_features(
            &self.core,
            &self.requested,
            SceneFeatures::DISPLAY.with_hit_testing(),
        )?;
        Ok(ProjectedSceneInteraction::new(self))
    }

    /// Returns complete native composition interaction and geometry access.
    pub fn editing(&self) -> Result<ProjectedSceneEditing<'_>, MissingSceneCapability> {
        require_any_scene_features(&self.core, &self.requested, SceneFeatures::EDITABLE)?;
        Ok(ProjectedSceneEditing::new(self))
    }

    /// Returns the exact projected interaction unit under a point.
    #[must_use]
    pub(crate) fn hit_test(
        &self,
        point: Point,
    ) -> Option<TextHit<ProjectedTextUnitView<'_>, ProjectedTextPosition>> {
        let (positioned, cluster) =
            positioned_hit_cluster(&self.core.spine, point, HitMode::Exact)?;
        Some(cached_projected_hit(
            cluster,
            positioned,
            self.revision,
            point,
        ))
    }

    /// Returns the closest projected interaction-unit side for native point queries.
    #[must_use]
    pub(crate) fn hit_test_closest(
        &self,
        point: Point,
    ) -> Option<TextHit<ProjectedTextUnitView<'_>, ProjectedTextPosition>> {
        let (positioned, cluster) =
            positioned_hit_cluster(&self.core.spine, point, HitMode::Closest)?;
        Some(cached_projected_hit(
            cluster,
            positioned,
            self.revision,
            point,
        ))
    }

    /// Resolves exact scene geometry for one projected caret position.
    #[must_use]
    pub(crate) fn caret(
        &self,
        position: &ProjectedTextPosition,
    ) -> Option<SceneCaret<ProjectedTextPosition>> {
        let (positioned, source_map, source) =
            positioned_projected_source(&self.core.spine, self.revision, position)?;
        let movement = cursor_movement_at(&positioned.segment.geometry, source)?;
        let bounds = positioned.segment.geometry.caret_bounds(movement)?;
        Some(SceneCaret {
            position: projected_position(source_map, source, self.revision),
            bounds: bounds + Vec2::new(0.0, positioned.position.block_origin),
        })
    }

    /// Moves one position through the adapter-produced interaction map.
    #[must_use]
    pub(crate) fn move_position(
        &self,
        position: &ProjectedTextPosition,
        movement: TextMovement,
    ) -> Option<ProjectedTextPosition> {
        let (positioned, source_map, source) =
            positioned_projected_source(&self.core.spine, self.revision, position)?;
        let record = cursor_movement_at(&positioned.segment.geometry, source)?;
        let step = match movement {
            TextMovement::PreviousVisual => record.previous_visual(),
            TextMovement::NextVisual => record.next_visual(),
            TextMovement::PreviousLogical => record.previous_logical(),
            TextMovement::NextLogical => record.next_logical(),
        }?;
        let target = step.target;
        Some(projected_position(
            source_map,
            SourcePosition::new(target.offset(), target.affinity()),
            self.revision,
        ))
    }

    /// Resolves highlight rectangles for the selected range inside preedit.
    pub(crate) fn composition_selection_geometry(
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
    pub(crate) fn composition_geometry(
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
            || session.id() != self.composition()
            || session.epoch() != self.epoch()
        {
            return Err(CompositionError::new(CompositionErrorKind::WrongSnapshot));
        }
        Ok(())
    }

    fn composition_range_geometry(&self, bytes: Range<u32>) -> Vec<SceneCompositionRect> {
        let mut geometry: Vec<SceneCompositionRect> = Vec::new();
        for (positioned, cluster) in self.positioned_clusters() {
            let Some(source_map) = positioned.segment.geometry.source_map.as_ref() else {
                continue;
            };
            if !source_map.ranges_for_span(cluster.source).any(|source| {
                matches!(source, LocalRange::Composition { id, epoch, bytes: source }
                    if id == self.composition()
                        && epoch == self.epoch()
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
            .filter(|(positioned, cluster)| {
                positioned
                    .segment
                    .geometry
                    .source_map
                    .as_ref()
                    .is_some_and(|source_map| {
                        source_map.ranges_for_span(cluster.source).any(|source| {
                            local_range_overlaps_projected(&source, range, self.revision)
                        })
                    })
            })
            .map(|(positioned, cluster)| {
                (
                    positioned.position.line_base + cluster.line,
                    cluster.bounds + Vec2::new(0.0, positioned.position.block_origin),
                )
            })
            .collect()
    }

    fn positioned_clusters(
        &self,
    ) -> impl Iterator<Item = (PositionedSegment<'_>, CachedCluster<'_>)> {
        self.core.spine.segments().flat_map(|positioned| {
            positioned
                .segment
                .geometry
                .hit_clusters()
                .map(move |cluster| (positioned, cluster))
        })
    }
}

#[derive(Debug)]
pub(super) struct SceneCore {
    pub(super) paragraph_count: usize,
    pub(super) spine: SceneSpine,
    pub(super) metrics: TextMetrics,
    pub(super) region: Option<SceneRegionBinding>,
    pub(super) resident: SceneFeaturePolicy,
    pub(super) resident_union: SceneFeatures,
    pub(super) resident_intersection: SceneFeatures,
}

impl Scene {
    /// Returns exact point interaction over paragraphs that retained it.
    pub fn interaction(&self) -> Result<SceneInteraction<'_>, MissingSceneCapability> {
        require_any_scene_features(
            &self.core,
            &self.requested,
            SceneFeatures::DISPLAY.with_hit_testing(),
        )?;
        Ok(SceneInteraction::new(self))
    }

    /// Returns complete selection, navigation, and native-input access.
    pub fn editing(&self) -> Result<SceneEditing<'_>, MissingSceneCapability> {
        require_any_scene_features(&self.core, &self.requested, SceneFeatures::EDITABLE)?;
        Ok(SceneEditing::new(self))
    }

    /// Returns selection construction and geometry access.
    pub fn selection(&self) -> Result<SceneSelection<'_>, MissingSceneCapability> {
        require_any_scene_features(&self.core, &self.requested, SceneFeatures::SELECTABLE)?;
        Ok(SceneSelection::new(self))
    }

    /// Starts one native composition over the current primary insertion point.
    ///
    /// Native composition protocols expose one marked region. A sole logical
    /// selection becomes that replacement target. If the scene has several
    /// independent selections, or the primary visual selection has several
    /// disjoint logical ranges, the host-visible set is explicitly normalized
    /// to one collapsed primary extent before composition starts. Callers can
    /// observe that normalization through [`CompositionStart::selection_changed`].
    pub(crate) fn begin_composition(
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
    pub(crate) fn empty_selection_set(&self) -> SnapshotTextSelectionSet {
        SnapshotTextSelectionSet::new(self.document, self.revision, Vec::new())
    }

    /// Creates one collapsed selection at an exact scene position.
    pub(crate) fn collapsed_selection(
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
    pub(crate) fn selection_between(
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
    pub(crate) fn selection_set(
        &self,
        selections: impl IntoIterator<Item = SnapshotTextSelection>,
    ) -> Result<SnapshotTextSelectionSet, SelectionError> {
        let selections: Vec<_> = selections.into_iter().collect();
        self.validate_selections(&selections)?;
        Ok(SnapshotTextSelectionSet::new(
            self.document,
            self.revision,
            selections,
        ))
    }

    fn validate_selections(
        &self,
        selections: &[SnapshotTextSelection],
    ) -> Result<(), SelectionError> {
        for selection in selections {
            let expected =
                self.selection_between(selection.anchor(), selection.extent(), selection.mode())?;
            if expected.ranges() != selection.ranges() {
                return Err(SelectionError::new(SelectionErrorKind::UnknownPosition));
            }
        }
        validate_independent_selections(selections)
    }

    /// Moves every independent selection through the exact scene cursor map.
    ///
    /// When `extend` is true, each anchor is retained and the extent is moved.
    /// Otherwise a noncollapsed selection first collapses toward the requested
    /// direction and a collapsed selection advances by one interaction unit.
    pub(crate) fn move_selections(
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
                    self.selection_between(selection.anchor(), &extent, mode)?
                } else {
                    self.collapsed_selection(&extent)?
                }
            };
            moved.push(next);
        }
        self.selection_set(moved)
    }

    /// Resolves visual highlight rectangles for a complete selection set.
    pub(crate) fn selection_geometry(
        &self,
        selections: &SnapshotTextSelectionSet,
    ) -> Result<Vec<SceneSelectionRect>, SelectionError> {
        if selections.document() != self.document || selections.revision() != self.revision {
            return Err(SelectionError::new(SelectionErrorKind::WrongSnapshot));
        }
        self.validate_selections(selections.selections())?;
        let mut geometry: Vec<SceneSelectionRect> = Vec::new();
        for (selection_index, selection) in selections.selections().iter().enumerate() {
            for (positioned, cluster) in self.positioned_clusters() {
                let source_map = positioned
                    .segment
                    .geometry
                    .source_map
                    .as_ref()
                    .expect("selection capability retains a paragraph source map");
                let Some((range_index, _)) =
                    selection.ranges().iter().enumerate().find(|(_, range)| {
                        source_map.ranges_for_span(cluster.source).any(|source| {
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

    /// Returns an exact interaction-unit hit under a scene-space point.
    ///
    /// Unlike selection hit testing, this does not clamp points outside unit
    /// geometry to the nearest line edge.
    #[must_use]
    pub(crate) fn hit_test(&self, point: Point) -> Option<TextHit<SnapshotTextUnitView<'_>>> {
        let (positioned, cluster) =
            positioned_hit_cluster(&self.core.spine, point, HitMode::Exact)?;
        Some(cached_snapshot_hit(
            cluster,
            positioned,
            self.revision,
            point,
        ))
    }

    /// Returns the closest interaction-unit side for pointer selection.
    ///
    /// This includes whitespace and empty editable text which may have no
    /// painted glyph fragment.
    #[must_use]
    pub(crate) fn hit_test_closest(
        &self,
        point: Point,
    ) -> Option<TextHit<SnapshotTextUnitView<'_>>> {
        let (positioned, cluster) =
            positioned_hit_cluster(&self.core.spine, point, HitMode::Closest)?;
        Some(cached_snapshot_hit(
            cluster,
            positioned,
            self.revision,
            point,
        ))
    }

    /// Resolves exact scene-space caret geometry for a snapshot position.
    ///
    /// Returns `None` for a stale revision, foreign text leaf, invalid
    /// affinity, or a valid snapshot position not represented by this scene.
    #[must_use]
    pub(crate) fn caret(&self, position: &SnapshotTextPosition) -> Option<SceneCaret> {
        if position.revision() != self.revision {
            return None;
        }
        let positioned = positioned_snapshot_segment(&self.core.spine, position.text())?;
        let source_map = positioned.segment.geometry.source_map.as_ref()?;
        let source = source_map.source_position_for_local(LocalPosition::Snapshot {
            text: position.text(),
            byte: position.byte(),
            affinity: position.affinity(),
        })?;
        let movement = cursor_movement_at(&positioned.segment.geometry, source)?;
        let bounds = positioned.segment.geometry.caret_bounds(movement)?;
        Some(SceneCaret {
            position: materialize_position(source_map, source, self.revision),
            bounds: bounds + Vec2::new(0.0, positioned.position.block_origin),
        })
    }

    /// Returns the first logical caret retained by this scene's feature policy.
    ///
    /// This traverses retained paragraphs in scene order. A sparse policy can
    /// therefore return a position after the document's authored start.
    #[must_use]
    pub(crate) fn start_position(&self) -> Option<SnapshotTextPosition> {
        let first = self.core.spine.positioned_movement(0)?;
        let source_map = first.position.segment.geometry.source_map.as_ref()?;
        let movement = first.position.segment.geometry.cursor()?.start()?;
        let position = movement.position();
        Some(materialize_position(
            source_map,
            SourcePosition::new(position.offset(), position.affinity()),
            self.revision,
        ))
    }

    /// Returns the final logical caret retained by this scene's feature policy.
    ///
    /// This traverses retained paragraphs in scene order. A sparse policy can
    /// therefore return a position before the document's authored end.
    #[must_use]
    pub(crate) fn end_position(&self) -> Option<SnapshotTextPosition> {
        let last = self
            .core
            .spine
            .summary()
            .movements
            .checked_sub(1)
            .and_then(|index| self.core.spine.positioned_movement(index))?;
        let source_map = last.position.segment.geometry.source_map.as_ref()?;
        let movement = last.position.segment.geometry.cursor()?.end()?;
        let position = movement.position();
        Some(materialize_position(
            source_map,
            SourcePosition::new(position.offset(), position.affinity()),
            self.revision,
        ))
    }

    /// Resolves a represented caret at one leaf-local UTF-8 boundary.
    ///
    /// This never fabricates a caret at an interior UTF-8 byte or at a
    /// semantic leaf seam inside one shaped grapheme. At a soft wrap or bidi
    /// discontinuity more than one affinity can share the byte boundary. The
    /// leaf start prefers downstream affinity and every other boundary
    /// prefers upstream affinity, then falls back to either represented stop.
    #[must_use]
    pub(crate) fn position_at(&self, text: TextId, byte: u32) -> Option<SnapshotTextPosition> {
        if text.document != self.document {
            return None;
        }
        let positioned = positioned_snapshot_segment(&self.core.spine, text)?;
        let preferred = if byte == 0 {
            TextAffinity::Downstream
        } else {
            TextAffinity::Upstream
        };
        let source_map = positioned.segment.geometry.source_map.as_ref()?;
        for affinity in [
            preferred,
            match preferred {
                TextAffinity::Upstream => TextAffinity::Downstream,
                TextAffinity::Downstream => TextAffinity::Upstream,
            },
        ] {
            let Some(source) = source_map.source_position_for_local(LocalPosition::Snapshot {
                text,
                byte,
                affinity,
            }) else {
                continue;
            };
            if let Some(movement) = cursor_movement_at(&positioned.segment.geometry, source) {
                let position = movement.position();
                return Some(materialize_position(
                    source_map,
                    SourcePosition::new(position.offset(), position.affinity()),
                    self.revision,
                ));
            }
        }
        None
    }

    /// Returns the preceding logical word start, or the scene start.
    ///
    /// Word starts come from the paragraph adapter's retained Unicode
    /// analysis. A stale, foreign, or unrepresented position returns `None`.
    #[must_use]
    pub(crate) fn previous_word_position(
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
    pub(crate) fn next_word_position(
        &self,
        position: &SnapshotTextPosition,
    ) -> Option<SnapshotTextPosition> {
        self.word_position(position, true)
    }

    fn validate_position(&self, position: &SnapshotTextPosition) -> Result<(), SelectionError> {
        self.movement_record(position).map(|_| ())
    }

    fn movement_record<'a>(
        &'a self,
        position: &SnapshotTextPosition,
    ) -> Result<
        (
            PositionedSegment<'a>,
            &'a ParagraphSourceMap,
            CursorMovement<'a>,
        ),
        SelectionError,
    > {
        if position.revision() != self.revision || position.text().document != self.document {
            return Err(SelectionError::new(SelectionErrorKind::WrongSnapshot));
        }
        let Some(positioned) = positioned_snapshot_segment(&self.core.spine, position.text())
        else {
            return Err(SelectionError::new(SelectionErrorKind::UnknownPosition));
        };
        let source_map = positioned
            .segment
            .geometry
            .source_map
            .as_ref()
            .ok_or_else(|| SelectionError::new(SelectionErrorKind::UnknownPosition))?;
        let source = source_map
            .source_position_for_local(LocalPosition::Snapshot {
                text: position.text(),
                byte: position.byte(),
                affinity: position.affinity(),
            })
            .ok_or_else(|| SelectionError::new(SelectionErrorKind::UnknownPosition))?;
        let movement = cursor_movement_at(&positioned.segment.geometry, source)
            .ok_or_else(|| SelectionError::new(SelectionErrorKind::UnknownPosition))?;
        Ok((positioned, source_map, movement))
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
            .filter(|cluster| {
                cluster.1.boundary != ClusterBoundary::None
                    && cluster.1.whitespace == ClusterWhitespace::None
            })
            .filter_map(|(positioned, cluster)| {
                let source_map = positioned.segment.geometry.source_map.as_ref()?;
                let source = source_map.ranges_for_span(cluster.source).next()?;
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
            let source_map = positioned
                .position
                .segment
                .geometry
                .source_map
                .as_ref()
                .expect("source capability retains a paragraph source map");
            let text = materialize_range(
                source_map
                    .leaf_range(positioned.local)
                    .expect("positioned text indexes a retained source leaf"),
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
        let (_, source_map, record) = self.movement_record(position)?;
        let step = match movement {
            TextMovement::PreviousVisual => record.previous_visual(),
            TextMovement::NextVisual => record.next_visual(),
            TextMovement::PreviousLogical => record.previous_logical(),
            TextMovement::NextLogical => record.next_logical(),
        };
        let step = materialize_cursor_step(source_map, step, self.revision);
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
                .saturating_add(current.segment.geometry.movement_count())
        };
        let adjacent = self.core.spine.positioned_movement(global)?;
        let source_map = adjacent.position.segment.geometry.source_map.as_ref()?;
        let cursor = adjacent.position.segment.geometry.cursor()?;
        let target = match movement {
            TextMovement::PreviousVisual => cursor.last_visual(),
            TextMovement::NextVisual => cursor.first_visual(),
            TextMovement::PreviousLogical => cursor.end(),
            TextMovement::NextLogical => cursor.start(),
        }?
        .position();
        let target = materialize_position(
            source_map,
            SourcePosition::new(target.offset(), target.affinity()),
            self.revision,
        );
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
            .source_map
            .as_ref()
            .and_then(|source_map| source_map.leaf_index_for_text(text))
            .map(|local| positioned.position.text_base.saturating_add(local))
            .ok_or_else(|| SelectionError::new(SelectionErrorKind::UnknownPosition))
    }

    pub(crate) fn range_geometry(&self, range: &SnapshotTextRange) -> Vec<(usize, Rect)> {
        self.positioned_clusters()
            .filter(|(positioned, cluster)| {
                positioned
                    .segment
                    .geometry
                    .source_map
                    .as_ref()
                    .is_some_and(|source_map| {
                        source_map.ranges_for_span(cluster.source).any(|source| {
                            ranges_overlap(range, &materialize_range(source, self.revision))
                        })
                    })
            })
            .map(|(positioned, cluster)| {
                (
                    positioned.position.line_base + cluster.line,
                    cluster.bounds + Vec2::new(0.0, positioned.position.block_origin),
                )
            })
            .collect()
    }

    fn positioned_clusters(
        &self,
    ) -> impl Iterator<Item = (PositionedSegment<'_>, CachedCluster<'_>)> {
        self.core.spine.segments().flat_map(|positioned| {
            positioned
                .segment
                .geometry
                .hit_clusters()
                .map(move |cluster| (positioned, cluster))
        })
    }
}

fn require_any_scene_features(
    core: &SceneCore,
    requested: &SceneFeaturePolicy,
    required: SceneFeatures,
) -> Result<(), MissingSceneCapability> {
    let summary = core.spine.summary();
    if summary.paragraphs == 0 {
        return requested
            .default_features()
            .contains(required)
            .then_some(())
            .ok_or_else(|| {
                MissingSceneCapability::new(
                    None,
                    required,
                    requested.default_features(),
                    requested.default_features(),
                )
            });
    }
    debug_assert!(
        !required.has_semantics(),
        "any-paragraph sessions use the linear interaction capability branch"
    );
    if core.resident_union.contains(required) {
        return Ok(());
    }

    let mut missing = None;
    for positioned in core.spine.segments() {
        let paragraph = positioned.segment.paragraph;
        let resident = positioned.segment.geometry.features;
        if resident.contains(required) {
            return Ok(());
        }
        if missing.is_none() {
            missing = Some(MissingSceneCapability::new(
                Some(paragraph),
                required,
                requested.features_for(paragraph),
                resident,
            ));
        }
    }
    Err(missing.unwrap_or_else(|| {
        MissingSceneCapability::new(
            None,
            required,
            requested.default_features(),
            requested.default_features(),
        )
    }))
}

fn require_scene_features(
    core: &SceneCore,
    requested: &SceneFeaturePolicy,
    required: SceneFeatures,
) -> Result<(), MissingSceneCapability> {
    let summary = core.spine.summary();
    if summary.paragraphs == 0 || core.resident_intersection.contains(required) {
        return Ok(());
    }

    for positioned in core.spine.segments() {
        let paragraph = positioned.segment.paragraph;
        let resident = positioned.segment.geometry.features;
        if !resident.contains(required) {
            return Err(MissingSceneCapability::new(
                Some(paragraph),
                required,
                requested.features_for(paragraph),
                resident,
            ));
        }
    }
    Ok(())
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
        .sources
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

/// Physical-to-logical axes for Underwood's currently supported writing mode.
///
/// Interaction indexing consumes only this mapping. The logical-axis
/// readiness design reserves a paragraph writing mode to select another
/// mapping when vertical formation is real; until then the engine accepts only
/// horizontal top-to-bottom text.
const HORIZONTAL_AXES: TextAxes = TextAxes;

#[derive(Clone, Copy)]
struct TextAxes;

#[derive(Clone, Copy)]
struct LogicalPoint {
    inline: f64,
    block: f64,
}

#[derive(Clone, Copy)]
struct LogicalRect {
    inline_start: f64,
    inline_end: f64,
    block_start: f64,
    block_end: f64,
}

impl TextAxes {
    const fn scene_point(self, point: Point) -> LogicalPoint {
        LogicalPoint {
            inline: point.x,
            block: point.y,
        }
    }

    fn local_point(self, positioned: PositionedSegment<'_>, point: Point) -> LogicalPoint {
        let mut logical = self.scene_point(point);
        logical.block -= positioned.position.block_origin;
        logical
    }

    const fn rect(self, bounds: Rect) -> LogicalRect {
        LogicalRect {
            inline_start: bounds.x0,
            inline_end: bounds.x1,
            block_start: bounds.y0,
            block_end: bounds.y1,
        }
    }
}

impl LogicalRect {
    const fn contains(self, point: LogicalPoint) -> bool {
        self.inline_start <= point.inline
            && point.inline <= self.inline_end
            && self.block_start <= point.block
            && point.block <= self.block_end
    }
}

#[derive(Clone, Copy)]
enum HitMode {
    Exact,
    Closest,
}

fn positioned_hit_cluster(
    spine: &SceneSpine,
    point: Point,
    mode: HitMode,
) -> Option<(PositionedSegment<'_>, CachedCluster<'_>)> {
    if spine.is_normal_flow() {
        let positioned =
            spine.positioned_segment_at_block(HORIZONTAL_AXES.scene_point(point).block)?;
        let cluster = match mode {
            HitMode::Exact => hit_cluster(positioned, point, true),
            HitMode::Closest => closest_cluster(positioned, point, true),
        }?;
        return Some((positioned, cluster));
    }
    match mode {
        HitMode::Exact => spine.segments().find_map(|positioned| {
            hit_cluster(positioned, point, false).map(|cluster| (positioned, cluster))
        }),
        HitMode::Closest => {
            let mut closest: Option<(PositionedSegment<'_>, CachedCluster<'_>, f64, f64)> = None;
            for positioned in spine.segments() {
                let Some(cluster) = closest_cluster(positioned, point, false) else {
                    continue;
                };
                let local = HORIZONTAL_AXES.local_point(positioned, point);
                let bounds = HORIZONTAL_AXES.rect(cluster.bounds);
                let (block_distance, inline_distance) = distance_to_rect_axes(local, bounds);
                if closest.is_none_or(|(_, _, current_block, current_inline)| {
                    block_distance < current_block
                        || block_distance == current_block && inline_distance < current_inline
                }) {
                    closest = Some((positioned, cluster, block_distance, inline_distance));
                }
            }
            closest.map(|(positioned, cluster, _, _)| (positioned, cluster))
        }
    }
}

fn positioned_projected_source<'a>(
    spine: &'a SceneSpine,
    revision: DocumentRevision,
    position: &ProjectedTextPosition,
) -> Option<(
    PositionedSegment<'a>,
    &'a ParagraphSourceMap,
    SourcePosition,
)> {
    if let ProjectedTextPosition::Snapshot(position) = position {
        if position.revision() != revision {
            return None;
        }
        let positioned = positioned_snapshot_segment(spine, position.text())?;
        let source_map = positioned.segment.geometry.source_map.as_ref()?;
        let source = source_map.source_position_for_local(LocalPosition::Snapshot {
            text: position.text(),
            byte: position.byte(),
            affinity: position.affinity(),
        })?;
        return Some((positioned, source_map, source));
    }
    let ProjectedTextPosition::Composition(position) = position else {
        unreachable!("snapshot positions return above")
    };
    spine.segments().find_map(|positioned| {
        let source_map = positioned.segment.geometry.source_map.as_ref()?;
        let source = source_map.source_position_for_local(LocalPosition::Composition {
            id: position.id(),
            epoch: position.epoch(),
            byte: position.byte(),
            affinity: position.affinity(),
        })?;
        Some((positioned, source_map, source))
    })
}

fn hit_cluster(
    positioned: PositionedSegment<'_>,
    point: Point,
    normal_flow: bool,
) -> Option<CachedCluster<'_>> {
    let geometry = &positioned.segment.geometry;
    let local = HORIZONTAL_AXES.local_point(positioned, point);
    if geometry.lines.is_empty() {
        if !HORIZONTAL_AXES.rect(geometry.empty_bounds).contains(local) {
            return None;
        }
        return geometry.exact_hit_cluster(0, local.inline);
    }
    if normal_flow {
        let line = geometry
            .lines
            .partition_point(|line| HORIZONTAL_AXES.rect(line.bounds).block_end < local.block);
        let bounds = HORIZONTAL_AXES.rect(geometry.lines.get(line)?.bounds);
        if local.block < bounds.block_start || local.block > bounds.block_end {
            return None;
        }
        return geometry.exact_hit_cluster(line, local.inline);
    }
    geometry
        .lines
        .iter()
        .enumerate()
        .find_map(|(line, bounds)| {
            let bounds = HORIZONTAL_AXES.rect(bounds.bounds);
            (bounds.block_start <= local.block && local.block <= bounds.block_end)
                .then(|| geometry.exact_hit_cluster(line, local.inline))
                .flatten()
        })
}

fn closest_cluster<'a>(
    positioned: PositionedSegment<'a>,
    point: Point,
    normal_flow: bool,
) -> Option<CachedCluster<'a>> {
    let geometry = &positioned.segment.geometry;
    let local = HORIZONTAL_AXES.local_point(positioned, point);
    if geometry.lines.is_empty() {
        return geometry.closest_hit_cluster(0, local.inline);
    }
    if normal_flow {
        let next = geometry
            .lines
            .partition_point(|line| HORIZONTAL_AXES.rect(line.bounds).block_end < local.block);
        let mut lines = [
            next.checked_sub(1),
            (next < geometry.lines.len()).then_some(next),
        ]
        .into_iter()
        .flatten();
        let first = lines.next()?;
        let line = lines.fold(first, |closest, candidate| {
            let closest_bounds = HORIZONTAL_AXES.rect(geometry.lines[closest].bounds);
            let closest_distance = distance_to_interval(
                local.block,
                closest_bounds.block_start,
                closest_bounds.block_end,
            );
            let candidate_bounds = HORIZONTAL_AXES.rect(geometry.lines[candidate].bounds);
            let candidate_distance = distance_to_interval(
                local.block,
                candidate_bounds.block_start,
                candidate_bounds.block_end,
            );
            if candidate_distance < closest_distance {
                candidate
            } else {
                closest
            }
        });
        return geometry.closest_hit_cluster(line, local.inline);
    }
    let mut closest: Option<(CachedCluster<'_>, f64, f64)> = None;
    for (line, bounds) in geometry.lines.iter().enumerate() {
        let bounds = HORIZONTAL_AXES.rect(bounds.bounds);
        let block_distance =
            distance_to_interval(local.block, bounds.block_start, bounds.block_end);
        if closest.is_some_and(|(_, current, _)| block_distance > current) {
            continue;
        }
        let Some(cluster) = geometry.closest_hit_cluster(line, local.inline) else {
            continue;
        };
        let cluster_bounds = HORIZONTAL_AXES.rect(cluster.bounds);
        let inline_distance = distance_to_interval(
            local.inline,
            cluster_bounds.inline_start,
            cluster_bounds.inline_end,
        );
        if closest.is_none_or(|(_, current_block, current_inline)| {
            block_distance < current_block
                || block_distance == current_block && inline_distance < current_inline
        }) {
            closest = Some((cluster, block_distance, inline_distance));
        }
    }
    closest.map(|(cluster, _, _)| cluster)
}

fn cursor_movement_at<'a>(
    geometry: &'a CachedGeometry,
    position: SourcePosition,
) -> Option<CursorMovement<'a>> {
    geometry
        .cursor()?
        .get(PreparedClusterSide::new(position.offset, position.affinity))
}

fn cached_snapshot_hit<'a>(
    cluster: CachedCluster<'a>,
    positioned: PositionedSegment<'a>,
    revision: DocumentRevision,
    point: Point,
) -> TextHit<SnapshotTextUnitView<'a>> {
    let source_map = positioned
        .segment
        .geometry
        .source_map
        .as_ref()
        .expect("hit-testing capability retains a paragraph source map");
    let (position, semantic_id) = cached_hit_facts(cluster, positioned, point);
    TextHit {
        source: SnapshotTextUnitView::new(revision, source_map, cluster.source),
        position: materialize_position(source_map, position, revision),
        semantic_id,
        bidi_level: cluster.bidi_level,
    }
}

fn cached_projected_hit<'a>(
    cluster: CachedCluster<'a>,
    positioned: PositionedSegment<'a>,
    revision: DocumentRevision,
    point: Point,
) -> TextHit<ProjectedTextUnitView<'a>, ProjectedTextPosition> {
    let source_map = positioned
        .segment
        .geometry
        .source_map
        .as_ref()
        .expect("hit-testing capability retains a paragraph source map");
    let (position, semantic_id) = cached_hit_facts(cluster, positioned, point);
    TextHit {
        source: ProjectedTextUnitView::new(revision, source_map, cluster.source),
        position: projected_position(source_map, position, revision),
        semantic_id,
        bidi_level: cluster.bidi_level,
    }
}

fn cached_hit_facts(
    cluster: CachedCluster<'_>,
    positioned: PositionedSegment<'_>,
    point: Point,
) -> (SourcePosition, SemanticId) {
    let point = HORIZONTAL_AXES.local_point(positioned, point);
    let bounds = HORIZONTAL_AXES.rect(cluster.bounds);
    let midpoint = bounds.inline_start + (bounds.inline_end - bounds.inline_start) * 0.5;
    let source_map = positioned
        .segment
        .geometry
        .source_map
        .as_ref()
        .expect("hit-testing capability retains source provenance");
    let semantic = cluster.prepared.map_or(cluster.semantic_id, |unit| {
        let mut inline = bounds.inline_start;
        unit.slices()
            .iter()
            .enumerate()
            .filter_map(|(index, slice)| {
                let mut end = inline + slice.advance();
                if index + 1 == unit.slices().len() {
                    end = bounds.inline_end;
                }
                let start = inline;
                inline = end;
                (start < end).then(|| {
                    let source = SourceSpan::from(slice.source());
                    let semantic = source_map
                        .semantic_for_span(source)
                        .expect("validated hit slices retain semantic ownership");
                    (semantic, distance_to_interval(point.inline, start, end))
                })
            })
            .min_by(|(_, first), (_, second)| first.total_cmp(second))
            .map_or(cluster.semantic_id, |(semantic, _)| semantic)
    });
    let position = if point.inline <= midpoint {
        cluster.left
    } else {
        cluster.right
    };
    (position, semantic)
}

#[derive(Clone, Debug)]
pub(super) struct SceneCursorStep<Source = SnapshotTextUnit, Position = SnapshotTextPosition> {
    pub(super) target: Position,
    pub(super) source: Option<Source>,
}

fn distance_to_rect_axes(point: LogicalPoint, bounds: LogicalRect) -> (f64, f64) {
    let inline = if point.inline < bounds.inline_start {
        bounds.inline_start - point.inline
    } else if point.inline > bounds.inline_end {
        point.inline - bounds.inline_end
    } else {
        0.0
    };
    let block = if point.block < bounds.block_start {
        bounds.block_start - point.block
    } else if point.block > bounds.block_end {
        point.block - bounds.block_end
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
