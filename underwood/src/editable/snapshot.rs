// Copyright 2026 the Underwood Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Atomically bound scene, selection, encoding, and provenance mapping.
//!
//! This module owns host offset and geometry queries; it explicitly does not
//! choose the semantic text exposed by a focused surface.

use super::*;

/// One atomically bound native text-input snapshot.
#[derive(Debug)]
pub struct EditableSurfaceSnapshot<'a> {
    pub(super) surface: &'a EditableSurface,
    pub(super) text: String,
    pub(super) spans: Vec<BoundSurfaceSpan>,
    pub(super) selections: Vec<EditableSurfaceSelection>,
    pub(super) host_selection: Option<Range<u32>>,
    pub(super) marked_range: Option<Range<u32>>,
    pub(super) composition: Option<(CompositionId, CompositionEpoch)>,
    pub(super) scene: BoundScene<'a>,
}

impl EditableSurfaceSnapshot<'_> {
    /// Returns the exact document identity.
    #[must_use]
    pub fn document(&self) -> DocumentId {
        self.surface.document()
    }

    /// Returns the exact immutable base revision.
    #[must_use]
    pub fn revision(&self) -> DocumentRevision {
        self.surface.revision()
    }

    /// Returns the exact composition identity and epoch, when marked text exists.
    #[must_use]
    pub const fn composition(&self) -> Option<(CompositionId, CompositionEpoch)> {
        self.composition
    }

    /// Returns the complete flattened UTF-8 text visible to the native client.
    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Returns every independent selection without flattening visual bidi ranges.
    #[must_use]
    pub fn selections(&self) -> &[EditableSurfaceSelection] {
        &self.selections
    }

    /// Returns the single range representable by conventional native APIs.
    ///
    /// This is `None` for a committed primary visual selection with several
    /// logical ranges. Starting composition explicitly normalizes that case.
    #[must_use]
    pub fn host_selection(&self) -> Option<Range<u32>> {
        self.host_selection.clone()
    }

    /// Returns the generated marked-text range in surface UTF-8 offsets.
    #[must_use]
    pub fn marked_range(&self) -> Option<Range<u32>> {
        self.marked_range.clone()
    }

    /// Returns a validated substring for a synchronous native range query.
    pub fn text_for_range(&self, range: Range<u32>) -> Result<&str, SurfaceError> {
        self.text
            .get(range.start as usize..range.end as usize)
            .ok_or_else(|| SurfaceError::new(SurfaceErrorKind::InvalidRange))
    }

    /// Converts one UTF-8 range to the requested adapter encoding.
    pub fn range_in_encoding(
        &self,
        range: Range<u32>,
        encoding: SurfaceTextEncoding,
    ) -> Result<Range<u32>, SurfaceError> {
        if range.start > range.end {
            return Err(SurfaceError::new(SurfaceErrorKind::InvalidRange));
        }
        let prefix = self.text_for_range(0..range.start)?;
        let through = self.text_for_range(0..range.end)?;
        Ok(encoded_len(prefix, encoding)?..encoded_len(through, encoding)?)
    }

    /// Converts one adapter range into surface UTF-8 offsets.
    pub fn range_from_encoding(
        &self,
        range: Range<u32>,
        encoding: SurfaceTextEncoding,
    ) -> Result<Range<u32>, SurfaceError> {
        let start = byte_offset_for_encoded(&self.text, range.start, encoding)?;
        let end = byte_offset_for_encoded(&self.text, range.end, encoding)?;
        if start > end {
            return Err(SurfaceError::new(SurfaceErrorKind::InvalidRange));
        }
        Ok(start..end)
    }

    /// Maps a single authored surface range back to its semantic snapshot leaf.
    pub fn snapshot_range(&self, range: Range<u32>) -> Result<SnapshotTextRange, SurfaceError> {
        let source = self.single_source_range(range)?;
        let BoundSurfaceSource::Snapshot { text, bytes } = source else {
            return Err(SurfaceError::new(SurfaceErrorKind::UnmappedRange));
        };
        Ok(SnapshotTextRange::new(self.revision(), text, bytes))
    }

    /// Maps one native authored range into a validated logical replacement selection.
    ///
    /// The range must stay within one semantic text leaf in a committed surface
    /// snapshot. Generated marked text and read-only separators are rejected.
    /// This is the mutation-side counterpart of [`Self::snapshot_range`]: a
    /// host-driven protocol can convert its offset encoding, request this set,
    /// and pass it to [`crate::SceneEditing::begin_composition`] without constructing a
    /// snapshot position from a raw byte offset.
    pub fn replacement_selection(
        &self,
        range: Range<u32>,
    ) -> Result<SnapshotTextSelectionSet, SurfaceError> {
        let scene = match &self.scene {
            BoundScene::Committed(scene) => *scene,
            BoundScene::Composition(_) => {
                return Err(SurfaceError::new(SurfaceErrorKind::UnsupportedSelection));
            }
        };
        let source = self.snapshot_range(range)?;
        let bytes = source.bytes();
        let start = scene_position(
            scene,
            self.revision(),
            source.text(),
            bytes.start,
            TextAffinity::Downstream,
        )
        .ok_or_else(|| SurfaceError::new(SurfaceErrorKind::UnsupportedSelection))?;
        let selection = if bytes.is_empty() {
            scene.collapsed_selection(&start)
        } else {
            let end = scene_position(
                scene,
                self.revision(),
                source.text(),
                bytes.end,
                TextAffinity::Upstream,
            )
            .ok_or_else(|| SurfaceError::new(SurfaceErrorKind::UnsupportedSelection))?;
            scene.selection_between(&start, &end, TextSelectionMode::Logical)
        }
        .map_err(|_| SurfaceError::new(SurfaceErrorKind::UnsupportedSelection))?;
        scene
            .selection_set([selection])
            .map_err(|_| SurfaceError::new(SurfaceErrorKind::UnsupportedSelection))
    }

    /// Answers the primary insertion rectangle from this exact bound scene.
    #[must_use]
    pub fn caret_rect(&self) -> Option<Rect> {
        let selection = self.host_selection.as_ref()?;
        let position = self.source_position(selection.end, TextAffinity::Upstream)?;
        match (&self.scene, position) {
            (BoundScene::Committed(scene), BoundPosition::Snapshot(position)) => {
                scene.caret(&position).map(|caret| caret.bounds)
            }
            (BoundScene::Composition(scene), position) => {
                scene.caret(&position.projected()).map(|caret| caret.bounds)
            }
            _ => None,
        }
    }

    /// Answers the first scene rectangle covering a surface range.
    pub fn first_rect_for_range(&self, range: Range<u32>) -> Result<Option<Rect>, SurfaceError> {
        if range.is_empty() {
            let position = self
                .source_position(range.start, TextAffinity::Downstream)
                .ok_or_else(|| SurfaceError::new(SurfaceErrorKind::UnmappedRange))?;
            return Ok(match (&self.scene, position) {
                (BoundScene::Committed(scene), BoundPosition::Snapshot(position)) => {
                    scene.caret(&position).map(|caret| caret.bounds)
                }
                (BoundScene::Composition(scene), position) => {
                    scene.caret(&position.projected()).map(|caret| caret.bounds)
                }
                _ => None,
            });
        }
        let sources = self.source_ranges(range)?;
        let mut geometry = Vec::new();
        match &self.scene {
            BoundScene::Committed(scene) => {
                for source in sources {
                    let BoundSurfaceSource::Snapshot { text, bytes } = source else {
                        return Err(SurfaceError::new(SurfaceErrorKind::UnmappedRange));
                    };
                    geometry.extend(scene.range_geometry(&SnapshotTextRange::new(
                        self.revision(),
                        text,
                        bytes,
                    )));
                }
            }
            BoundScene::Composition(scene) => {
                if sources
                    .iter()
                    .any(|source| matches!(source, BoundSurfaceSource::Separator))
                {
                    return Err(SurfaceError::new(SurfaceErrorKind::UnmappedRange));
                }
                let sources = sources
                    .into_iter()
                    .filter_map(|source| source.projected(self.revision()))
                    .collect();
                geometry.extend(scene.range_geometry(&ProjectedTextRange::new(sources)));
            }
        }
        geometry.sort_by(|first, second| {
            first
                .0
                .cmp(&second.0)
                .then_with(|| first.1.y0.total_cmp(&second.1.y0))
                .then_with(|| first.1.x0.total_cmp(&second.1.x0))
        });
        Ok(geometry.first().map(|(_, bounds)| *bounds))
    }

    /// Maps a scene-space point back to this surface's UTF-8 offset.
    #[must_use]
    pub fn offset_for_point(&self, point: Point) -> Option<u32> {
        let position = match &self.scene {
            BoundScene::Committed(scene) => {
                BoundPosition::Snapshot(scene.hit_test_closest(point)?.position)
            }
            BoundScene::Composition(scene) => match scene.hit_test_closest(point)?.position {
                ProjectedTextPosition::Snapshot(position) => BoundPosition::Snapshot(position),
                ProjectedTextPosition::Composition(position) => {
                    BoundPosition::Composition(position)
                }
            },
        };
        self.surface_offset(position)
    }

    fn single_source_range(&self, range: Range<u32>) -> Result<BoundSurfaceSource, SurfaceError> {
        let sources = self.source_ranges(range)?;
        let [source] = sources.as_slice() else {
            return Err(SurfaceError::new(SurfaceErrorKind::UnmappedRange));
        };
        Ok(source.clone())
    }

    fn source_ranges(&self, range: Range<u32>) -> Result<Vec<BoundSurfaceSource>, SurfaceError> {
        if self
            .text
            .get(range.start as usize..range.end as usize)
            .is_none()
        {
            return Err(SurfaceError::new(SurfaceErrorKind::InvalidRange));
        }
        if range.is_empty() {
            return self
                .source_position(range.start, TextAffinity::Downstream)
                .map(|position| alloc::vec![position.empty_range()])
                .ok_or_else(|| SurfaceError::new(SurfaceErrorKind::UnmappedRange));
        }
        let mut covered = range.start;
        let mut sources = Vec::new();
        for span in &self.spans {
            let start = range.start.max(span.surface.start);
            let end = range.end.min(span.surface.end);
            if start >= end {
                continue;
            }
            if start != covered {
                return Err(SurfaceError::new(SurfaceErrorKind::UnmappedRange));
            }
            sources.push(span.slice(start, end));
            covered = end;
        }
        if covered != range.end {
            return Err(SurfaceError::new(SurfaceErrorKind::UnmappedRange));
        }
        Ok(sources)
    }

    fn source_position(&self, byte: u32, affinity: TextAffinity) -> Option<BoundPosition> {
        let mut candidates = self.spans.iter().filter(|span| {
            !matches!(span.source, BoundSurfaceSource::Separator)
                && match affinity {
                    TextAffinity::Upstream => span.surface.start < byte && byte <= span.surface.end,
                    TextAffinity::Downstream => {
                        span.surface.start <= byte && byte < span.surface.end
                    }
                }
        });
        let span = match affinity {
            TextAffinity::Upstream => candidates.next_back(),
            TextAffinity::Downstream => candidates.next(),
        }
        .or_else(|| {
            self.spans.iter().find(|span| {
                !matches!(span.source, BoundSurfaceSource::Separator)
                    && span.surface.start <= byte
                    && byte <= span.surface.end
            })
        })?;
        Some(span.position(byte, affinity, self.revision()))
    }

    fn surface_offset(&self, position: BoundPosition) -> Option<u32> {
        let affinity = position.affinity();
        let byte = position.byte();
        let matching = |span: &&BoundSurfaceSpan| span.source.matches_position(position);
        let span = match affinity {
            TextAffinity::Upstream => self
                .spans
                .iter()
                .filter(matching)
                .rev()
                .find(|span| span.source.start() < byte && byte <= span.source.end()),
            TextAffinity::Downstream => self
                .spans
                .iter()
                .filter(matching)
                .find(|span| span.source.start() <= byte && byte < span.source.end()),
        }
        .or_else(|| {
            self.spans
                .iter()
                .filter(matching)
                .find(|span| span.source.start() <= byte && byte <= span.source.end())
        })?;
        Some(span.surface.start + byte - span.source.start())
    }
}
#[derive(Clone, Debug)]
pub(super) struct BoundSurfaceSpan {
    pub(super) surface: Range<u32>,
    pub(super) source: BoundSurfaceSource,
}

impl BoundSurfaceSpan {
    fn slice(&self, start: u32, end: u32) -> BoundSurfaceSource {
        let relative_start = start - self.surface.start;
        let relative_end = end - self.surface.start;
        match &self.source {
            BoundSurfaceSource::Snapshot { text, bytes } => BoundSurfaceSource::Snapshot {
                text: *text,
                bytes: (bytes.start + relative_start)..(bytes.start + relative_end),
            },
            BoundSurfaceSource::Composition { id, epoch, bytes } => {
                BoundSurfaceSource::Composition {
                    id: *id,
                    epoch: *epoch,
                    bytes: (bytes.start + relative_start)..(bytes.start + relative_end),
                }
            }
            BoundSurfaceSource::Separator => BoundSurfaceSource::Separator,
        }
    }

    fn position(
        &self,
        byte: u32,
        affinity: TextAffinity,
        revision: DocumentRevision,
    ) -> BoundPosition {
        let relative = byte - self.surface.start;
        match &self.source {
            BoundSurfaceSource::Snapshot { text, bytes } => BoundPosition::Snapshot(
                SnapshotTextPosition::new(revision, *text, bytes.start + relative, affinity),
            ),
            BoundSurfaceSource::Composition { id, epoch, bytes } => BoundPosition::Composition(
                CompositionTextPosition::new(*id, *epoch, bytes.start + relative, affinity),
            ),
            BoundSurfaceSource::Separator => unreachable!("separators have no text position"),
        }
    }
}

#[derive(Clone, Debug)]
pub(super) enum BoundSurfaceSource {
    Snapshot {
        text: TextId,
        bytes: Range<u32>,
    },
    Composition {
        id: CompositionId,
        epoch: CompositionEpoch,
        bytes: Range<u32>,
    },
    Separator,
}

impl BoundSurfaceSource {
    fn projected(self, revision: DocumentRevision) -> Option<ProjectedTextSource> {
        match self {
            Self::Snapshot { text, bytes } => Some(ProjectedTextSource::Snapshot(
                SnapshotTextRange::new(revision, text, bytes),
            )),
            Self::Composition { id, epoch, bytes } => Some(ProjectedTextSource::Composition(
                CompositionTextRange::new(id, epoch, bytes),
            )),
            Self::Separator => None,
        }
    }

    const fn start(&self) -> u32 {
        match self {
            Self::Snapshot { bytes, .. } | Self::Composition { bytes, .. } => bytes.start,
            Self::Separator => 0,
        }
    }

    const fn end(&self) -> u32 {
        match self {
            Self::Snapshot { bytes, .. } | Self::Composition { bytes, .. } => bytes.end,
            Self::Separator => 0,
        }
    }

    fn matches_position(&self, position: BoundPosition) -> bool {
        match (self, position) {
            (Self::Snapshot { text, .. }, BoundPosition::Snapshot(position)) => {
                *text == position.text()
            }
            (Self::Composition { id, epoch, .. }, BoundPosition::Composition(position)) => {
                *id == position.id() && *epoch == position.epoch()
            }
            _ => false,
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum BoundPosition {
    Snapshot(SnapshotTextPosition),
    Composition(CompositionTextPosition),
}

impl BoundPosition {
    const fn byte(self) -> u32 {
        match self {
            Self::Snapshot(position) => position.byte(),
            Self::Composition(position) => position.byte(),
        }
    }

    const fn affinity(self) -> TextAffinity {
        match self {
            Self::Snapshot(position) => position.affinity(),
            Self::Composition(position) => position.affinity(),
        }
    }

    const fn projected(self) -> ProjectedTextPosition {
        match self {
            Self::Snapshot(position) => ProjectedTextPosition::Snapshot(position),
            Self::Composition(position) => ProjectedTextPosition::Composition(position),
        }
    }

    fn empty_range(self) -> BoundSurfaceSource {
        match self {
            Self::Snapshot(position) => BoundSurfaceSource::Snapshot {
                text: position.text(),
                bytes: position.byte()..position.byte(),
            },
            Self::Composition(position) => BoundSurfaceSource::Composition {
                id: position.id(),
                epoch: position.epoch(),
                bytes: position.byte()..position.byte(),
            },
        }
    }
}

#[derive(Debug)]
pub(super) enum BoundScene<'a> {
    Committed(&'a TextScene),
    Composition(&'a CompositionScene),
}

pub(super) fn append_bound_span(
    text: &mut String,
    spans: &mut Vec<BoundSurfaceSpan>,
    value: &str,
    source: BoundSurfaceSource,
) -> Result<(), SurfaceError> {
    let start = surface_len(text)?;
    text.push_str(value);
    let end = surface_len(text)?;
    spans.push(BoundSurfaceSpan {
        surface: start..end,
        source,
    });
    Ok(())
}

pub(super) fn map_selections(
    surface: &EditableSurface,
    selections: &SnapshotTextSelectionSet,
) -> Result<Vec<EditableSurfaceSelection>, SurfaceError> {
    selections
        .selections()
        .iter()
        .map(|selection| {
            selection
                .ranges()
                .iter()
                .map(|range| surface.surface_range(range))
                .collect::<Result<Vec<_>, _>>()
                .map(|ranges| EditableSurfaceSelection {
                    ranges: ranges.into(),
                })
        })
        .collect()
}

pub(super) fn one_range(ranges: &[Range<u32>]) -> Option<Range<u32>> {
    let [range] = ranges else {
        return None;
    };
    Some(range.clone())
}

fn scene_position(
    scene: &TextScene,
    revision: DocumentRevision,
    text: TextId,
    byte: u32,
    preferred: TextAffinity,
) -> Option<SnapshotTextPosition> {
    [preferred, opposite_affinity(preferred)]
        .into_iter()
        .map(|affinity| SnapshotTextPosition::new(revision, text, byte, affinity))
        .find(|position| scene.caret(position).is_some())
}

const fn opposite_affinity(affinity: TextAffinity) -> TextAffinity {
    match affinity {
        TextAffinity::Upstream => TextAffinity::Downstream,
        TextAffinity::Downstream => TextAffinity::Upstream,
    }
}

pub(super) fn surface_len(text: &str) -> Result<u32, SurfaceError> {
    u32::try_from(text.len()).map_err(|_| SurfaceError::new(SurfaceErrorKind::InvalidRange))
}

pub(super) fn encoded_len(text: &str, encoding: SurfaceTextEncoding) -> Result<u32, SurfaceError> {
    let len = match encoding {
        SurfaceTextEncoding::Utf8 => text.len(),
        SurfaceTextEncoding::Utf16 => text.encode_utf16().count(),
        SurfaceTextEncoding::UnicodeScalars => text.chars().count(),
    };
    u32::try_from(len).map_err(|_| SurfaceError::new(SurfaceErrorKind::InvalidRange))
}

pub(super) fn byte_offset_for_encoded(
    text: &str,
    offset: u32,
    encoding: SurfaceTextEncoding,
) -> Result<u32, SurfaceError> {
    if encoding == SurfaceTextEncoding::Utf8 {
        let offset = usize::try_from(offset)
            .map_err(|_| SurfaceError::new(SurfaceErrorKind::InvalidRange))?;
        return if offset <= text.len() && text.is_char_boundary(offset) {
            u32::try_from(offset).map_err(|_| SurfaceError::new(SurfaceErrorKind::InvalidRange))
        } else {
            Err(SurfaceError::new(SurfaceErrorKind::InvalidRange))
        };
    }
    if offset == 0 {
        return Ok(0);
    }
    let mut count = 0_u32;
    for (byte, character) in text.char_indices() {
        count = count
            .checked_add(match encoding {
                SurfaceTextEncoding::Utf16 => u32::try_from(character.len_utf16())
                    .map_err(|_| SurfaceError::new(SurfaceErrorKind::InvalidRange))?,
                SurfaceTextEncoding::UnicodeScalars => 1,
                SurfaceTextEncoding::Utf8 => unreachable!(),
            })
            .ok_or_else(|| SurfaceError::new(SurfaceErrorKind::InvalidRange))?;
        if count == offset {
            return u32::try_from(byte + character.len_utf8())
                .map_err(|_| SurfaceError::new(SurfaceErrorKind::InvalidRange));
        }
        if count > offset {
            return Err(SurfaceError::new(SurfaceErrorKind::InvalidRange));
        }
    }
    Err(SurfaceError::new(SurfaceErrorKind::InvalidRange))
}
