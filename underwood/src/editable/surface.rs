// Copyright 2026 the Underwood Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Authored focused-surface scope and immutable base text.
//!
//! This module owns which semantic leaves enter a host surface; it explicitly
//! does not own encoding conversion or bound scene queries.

use super::*;

/// One caller-authored element of a focused editable-surface scope.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum EditableSurfaceElement {
    /// Expose one semantic text leaf exactly once.
    Text(TextId),
    /// Insert read-only surface text between semantic leaves.
    Separator(String),
}

impl EditableSurfaceElement {
    /// Creates a semantic text element.
    #[must_use]
    pub const fn text(text: TextId) -> Self {
        Self::Text(text)
    }

    /// Creates a read-only flattening separator.
    #[must_use]
    pub fn separator(text: impl Into<String>) -> Self {
        Self::Separator(text.into())
    }
}

/// Offset encoding requested by a native text protocol adapter.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum SurfaceTextEncoding {
    /// UTF-8 byte offsets used by Underwood internally.
    Utf8,
    /// UTF-16 code-unit offsets used by Apple APIs, Android, and Windows TSF.
    Utf16,
    /// Unicode scalar-value offsets used by some Android operations.
    UnicodeScalars,
}

/// One independent selection expressed in focused-surface UTF-8 offsets.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EditableSurfaceSelection {
    pub(super) ranges: Arc<[Range<u32>]>,
}

impl EditableSurfaceSelection {
    /// Returns logically ordered ranges belonging to this insertion point.
    #[must_use]
    pub fn ranges(&self) -> &[Range<u32>] {
        &self.ranges
    }
}

/// Immutable choice of semantic text exposed to one native text-input client.
///
/// The caller explicitly supplies both leaves and read-only separators, so a
/// platform offset can never silently mean a global document offset.
#[derive(Clone, Debug)]
pub struct EditableSurface {
    pub(super) snapshot: DocumentSnapshot,
    pub(super) text: String,
    pub(super) spans: Vec<BaseSurfaceSpan>,
}

impl EditableSurface {
    /// Flattens the requested semantic scope against one immutable snapshot.
    pub fn new(
        snapshot: &DocumentSnapshot,
        elements: impl IntoIterator<Item = EditableSurfaceElement>,
    ) -> Result<Self, SurfaceError> {
        let mut text = String::new();
        let mut spans = Vec::new();
        let mut seen = Vec::new();
        for element in elements {
            let start = surface_len(&text)?;
            match element {
                EditableSurfaceElement::Text(id) => {
                    if seen.contains(&id) {
                        return Err(SurfaceError::new(SurfaceErrorKind::DuplicateText));
                    }
                    let value = snapshot
                        .text(id)
                        .ok_or_else(|| SurfaceError::new(SurfaceErrorKind::UnknownText))?;
                    seen.push(id);
                    text.push_str(value);
                    let end = surface_len(&text)?;
                    spans.push(BaseSurfaceSpan {
                        surface: start..end,
                        source: BaseSurfaceSource::Snapshot(id),
                    });
                }
                EditableSurfaceElement::Separator(value) => {
                    text.push_str(&value);
                    let end = surface_len(&text)?;
                    spans.push(BaseSurfaceSpan {
                        surface: start..end,
                        source: BaseSurfaceSource::Separator,
                    });
                }
            }
        }
        Ok(Self {
            snapshot: snapshot.clone(),
            text,
            spans,
        })
    }

    /// Returns the document identity owning this focused scope.
    #[must_use]
    pub fn document(&self) -> DocumentId {
        self.snapshot.id()
    }

    /// Returns the immutable document revision owning this focused scope.
    #[must_use]
    pub fn revision(&self) -> DocumentRevision {
        self.snapshot.revision()
    }

    /// Returns committed flattened UTF-8 text before any composition projection.
    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Binds committed text, selections, geometry, and hit testing atomically.
    pub fn bind<'a>(
        &'a self,
        scene: &'a TextScene,
        selections: &SnapshotTextSelectionSet,
    ) -> Result<EditableSurfaceSnapshot<'a>, SurfaceError> {
        if scene.document() != self.document()
            || scene.revision() != self.revision()
            || selections.document() != self.document()
            || selections.revision() != self.revision()
        {
            return Err(SurfaceError::new(SurfaceErrorKind::WrongSnapshot));
        }
        let spans = self
            .spans
            .iter()
            .map(|span| match span.source {
                BaseSurfaceSource::Snapshot(text) => BoundSurfaceSpan {
                    surface: span.surface.clone(),
                    source: BoundSurfaceSource::Snapshot {
                        text,
                        bytes: 0..(span.surface.end - span.surface.start),
                    },
                },
                BaseSurfaceSource::Separator => BoundSurfaceSpan {
                    surface: span.surface.clone(),
                    source: BoundSurfaceSource::Separator,
                },
            })
            .collect();
        let selections = map_selections(self, selections)?;
        let host_selection = selections
            .first()
            .and_then(|selection| one_range(selection.ranges()));
        Ok(EditableSurfaceSnapshot {
            surface: self,
            text: self.text.clone(),
            spans,
            selections,
            host_selection,
            marked_range: None,
            composition: None,
            scene: BoundScene::Committed(scene),
        })
    }

    /// Binds one exact composition epoch to its projected text and geometry.
    pub fn bind_composition<'a>(
        &'a self,
        scene: &'a CompositionScene,
        session: &CompositionSession,
    ) -> Result<EditableSurfaceSnapshot<'a>, SurfaceError> {
        if scene.document() != self.document()
            || scene.revision() != self.revision()
            || scene.composition() != session.id()
            || scene.epoch() != session.epoch()
            || session.base_revision() != self.revision()
        {
            return Err(SurfaceError::new(SurfaceErrorKind::WrongSnapshot));
        }
        let target = session
            .target_text()
            .ok_or_else(|| SurfaceError::new(SurfaceErrorKind::UnsupportedSelection))?;
        let target_ranges = session.replacement_ranges();
        let mut text = String::new();
        let mut spans = Vec::new();
        let mut marked_range = None;
        for base in &self.spans {
            match base.source {
                BaseSurfaceSource::Separator => {
                    let value = self.slice(base.surface.clone())?;
                    append_bound_span(&mut text, &mut spans, value, BoundSurfaceSource::Separator)?;
                }
                BaseSurfaceSource::Snapshot(id) if id != target => {
                    let value = self.slice(base.surface.clone())?;
                    append_bound_span(
                        &mut text,
                        &mut spans,
                        value,
                        BoundSurfaceSource::Snapshot {
                            text: id,
                            bytes: 0..surface_len(value)?,
                        },
                    )?;
                }
                BaseSurfaceSource::Snapshot(id) => {
                    let value = self.slice(base.surface.clone())?;
                    let mut source = 0_u32;
                    for (index, range) in target_ranges.iter().enumerate() {
                        let bytes = range.bytes();
                        if bytes.start < source
                            || value
                                .get(bytes.start as usize..bytes.end as usize)
                                .is_none()
                        {
                            return Err(SurfaceError::new(SurfaceErrorKind::InvalidRange));
                        }
                        if source < bytes.start {
                            let retained = value
                                .get(source as usize..bytes.start as usize)
                                .ok_or_else(|| SurfaceError::new(SurfaceErrorKind::InvalidRange))?;
                            append_bound_span(
                                &mut text,
                                &mut spans,
                                retained,
                                BoundSurfaceSource::Snapshot {
                                    text: id,
                                    bytes: source..bytes.start,
                                },
                            )?;
                        }
                        if index == 0 {
                            let start = surface_len(&text)?;
                            append_bound_span(
                                &mut text,
                                &mut spans,
                                session.text(),
                                BoundSurfaceSource::Composition {
                                    id: session.id(),
                                    epoch: session.epoch(),
                                    bytes: 0..surface_len(session.text())?,
                                },
                            )?;
                            marked_range = Some(start..surface_len(&text)?);
                        }
                        source = bytes.end;
                    }
                    let end = surface_len(value)?;
                    if source < end {
                        let retained = value
                            .get(source as usize..)
                            .ok_or_else(|| SurfaceError::new(SurfaceErrorKind::InvalidRange))?;
                        append_bound_span(
                            &mut text,
                            &mut spans,
                            retained,
                            BoundSurfaceSource::Snapshot {
                                text: id,
                                bytes: source..end,
                            },
                        )?;
                    }
                }
            }
        }
        let marked_range =
            marked_range.ok_or_else(|| SurfaceError::new(SurfaceErrorKind::UnmappedRange))?;
        let host_selection = if let Some(selection) = session.selection() {
            Some(
                marked_range
                    .start
                    .checked_add(selection.start)
                    .ok_or_else(|| SurfaceError::new(SurfaceErrorKind::InvalidRange))?
                    ..marked_range
                        .start
                        .checked_add(selection.end)
                        .ok_or_else(|| SurfaceError::new(SurfaceErrorKind::InvalidRange))?,
            )
        } else {
            Some(marked_range.end..marked_range.end)
        };
        let selections = alloc::vec![EditableSurfaceSelection {
            ranges: Arc::from([host_selection
                .clone()
                .expect("selection was just constructed")]),
        }];
        Ok(EditableSurfaceSnapshot {
            surface: self,
            text,
            spans,
            selections,
            host_selection,
            marked_range: Some(marked_range),
            composition: Some((session.id(), session.epoch())),
            scene: BoundScene::Composition(scene),
        })
    }

    pub(super) fn slice(&self, bytes: Range<u32>) -> Result<&str, SurfaceError> {
        self.text
            .get(bytes.start as usize..bytes.end as usize)
            .ok_or_else(|| SurfaceError::new(SurfaceErrorKind::InvalidRange))
    }

    pub(super) fn surface_range(
        &self,
        range: &SnapshotTextRange,
    ) -> Result<Range<u32>, SurfaceError> {
        if range.revision() != self.revision() {
            return Err(SurfaceError::new(SurfaceErrorKind::WrongSnapshot));
        }
        let bytes = range.bytes();
        let span = self
            .spans
            .iter()
            .find(|span| matches!(span.source, BaseSurfaceSource::Snapshot(text) if text == range.text()))
            .ok_or_else(|| SurfaceError::new(SurfaceErrorKind::UnmappedRange))?;
        if bytes.end > span.surface.end - span.surface.start {
            return Err(SurfaceError::new(SurfaceErrorKind::InvalidRange));
        }
        Ok((span.surface.start + bytes.start)..(span.surface.start + bytes.end))
    }
}
#[derive(Clone, Debug)]
pub(super) struct BaseSurfaceSpan {
    pub(super) surface: Range<u32>,
    pub(super) source: BaseSurfaceSource,
}

#[derive(Clone, Copy, Debug)]
pub(super) enum BaseSurfaceSource {
    Snapshot(TextId),
    Separator,
}
