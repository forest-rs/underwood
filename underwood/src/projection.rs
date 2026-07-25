// Copyright 2026 the Underwood Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Compact, source-complete text projection.
//!
//! Projection records how immutable authored UTF-8 becomes presentation UTF-8
//! without making document, style, shaping, paint, or widget policy part of the
//! transformation kernel.

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::fmt;
use core::ops::Range;
use core::slice;

use crate::TextAffinity;

/// Paragraph-stream whitespace processing performed before Unicode analysis.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum WhitespaceCollapse {
    /// Preserve every authored scalar exactly.
    #[default]
    Preserve,
    /// Replace each maximal run of CSS document whitespace with one ASCII space.
    ///
    /// This initial policy recognizes space, tab, line feed, carriage return,
    /// and form feed. It does not trim paragraph or line edges.
    Collapse,
}

/// Relationship between one authored interval and one presentation interval.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum ProjectionKind {
    /// Authored and presentation bytes are identical.
    Identity,
    /// A nonempty authored interval was replaced by nonempty presentation text.
    Replacement,
    /// A nonempty authored run was collapsed to a nonempty presentation unit.
    Collapsed,
    /// A nonempty authored interval has no presentation bytes.
    Omitted,
    /// Presentation bytes were inserted at one authored boundary.
    Inserted,
}

/// One validated, monotonic run in an authored-to-presentation projection.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProjectionSegment {
    kind: ProjectionKind,
    source: Range<u32>,
    projected: Range<u32>,
}

/// Iterator over complete ordered projection relation runs.
///
/// Identity projection yields one synthesized run without allocating storage.
#[derive(Clone, Debug)]
pub struct ProjectionSegments<'a> {
    identity: Option<ProjectionSegment>,
    runs: slice::Iter<'a, ProjectionSegment>,
}

impl Iterator for ProjectionSegments<'_> {
    type Item = ProjectionSegment;

    fn next(&mut self) -> Option<Self::Item> {
        self.identity.take().or_else(|| self.runs.next().cloned())
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.len();
        (len, Some(len))
    }
}

impl DoubleEndedIterator for ProjectionSegments<'_> {
    fn next_back(&mut self) -> Option<Self::Item> {
        self.runs
            .next_back()
            .cloned()
            .or_else(|| self.identity.take())
    }
}

impl ExactSizeIterator for ProjectionSegments<'_> {
    fn len(&self) -> usize {
        usize::from(self.identity.is_some()) + self.runs.len()
    }
}

impl core::iter::FusedIterator for ProjectionSegments<'_> {}

impl ProjectionSegment {
    /// Returns the relationship represented by this run.
    #[must_use]
    pub const fn kind(&self) -> ProjectionKind {
        self.kind
    }

    /// Returns the authored UTF-8 byte interval.
    ///
    /// Inserted runs return an empty interval at their authored insertion
    /// boundary.
    #[must_use]
    pub fn source(&self) -> Range<u32> {
        self.source.clone()
    }

    /// Returns the presentation UTF-8 byte interval.
    ///
    /// Omitted runs return an empty interval at their presentation boundary.
    #[must_use]
    pub fn projected(&self) -> Range<u32> {
        self.projected.clone()
    }
}

/// Stable category for projection construction or lookup failures.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum ProjectionErrorKind {
    /// Authored or presentation text exceeds the compact 32-bit range space.
    TextTooLong,
    /// A source operation was empty, reversed, out of order, or out of bounds.
    InvalidSourceRange,
    /// A source operation boundary was not a UTF-8 boundary.
    InvalidSourceBoundary,
    /// A transformation that requires output received an empty output string.
    EmptyProjectedText,
    /// Construction ended before every authored byte was related.
    IncompleteSource,
    /// A queried presentation range was reversed or out of bounds.
    InvalidProjectedRange,
    /// A queried presentation boundary was not a UTF-8 boundary.
    InvalidProjectedBoundary,
}

/// Concrete compact-projection error.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProjectionError {
    kind: ProjectionErrorKind,
}

impl ProjectionError {
    const fn new(kind: ProjectionErrorKind) -> Self {
        Self { kind }
    }

    /// Returns the stable failure category.
    #[must_use]
    pub const fn kind(&self) -> ProjectionErrorKind {
        self.kind
    }
}

impl fmt::Display for ProjectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "text projection failed: {:?}", self.kind)
    }
}

impl core::error::Error for ProjectionError {}

/// Authored text, presentation text, and a complete monotonic relation.
///
/// Identity projections retain only the authored string. Nonidentity
/// projections retain the authored string and one presentation string.
/// Relations are stored as runs rather than per-byte maps.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProjectedText {
    source: String,
    projected: Option<String>,
    segments: Vec<ProjectionSegment>,
}

impl ProjectedText {
    /// Creates an identity projection without duplicating `source`.
    pub fn identity(source: impl Into<String>) -> Result<Self, ProjectionError> {
        let source = source.into();
        text_len(&source)?;
        Ok(Self {
            source,
            projected: None,
            segments: Vec::new(),
        })
    }

    /// Applies one paragraph-stream whitespace policy.
    ///
    /// Collapse state crosses any style or semantic boundaries represented
    /// outside this kernel because processing sees one complete source string.
    pub fn from_whitespace(
        source: impl Into<String>,
        policy: WhitespaceCollapse,
    ) -> Result<Self, ProjectionError> {
        let source = source.into();
        if policy == WhitespaceCollapse::Preserve {
            return Self::identity(source);
        }

        let mut builder = ProjectionBuilder::new(source)?;
        while builder.source_cursor() < builder.source_len() {
            let start = builder.source_cursor();
            let (end, whitespace) = {
                let bytes = builder.source_text().as_bytes();
                let start = usize::try_from(start)
                    .map_err(|_| ProjectionError::new(ProjectionErrorKind::TextTooLong))?;
                let whitespace = is_document_whitespace(bytes[start]);
                let mut end = start + 1;
                while end < bytes.len() && is_document_whitespace(bytes[end]) == whitespace {
                    end += 1;
                }
                (
                    u32::try_from(end)
                        .map_err(|_| ProjectionError::new(ProjectionErrorKind::TextTooLong))?,
                    whitespace,
                )
            };
            if whitespace {
                let is_one_space =
                    end - start == 1 && builder.source_text().as_bytes()[start as usize] == b' ';
                if is_one_space {
                    builder.push_identity(end)?;
                } else {
                    builder.push_collapsed(end, " ")?;
                }
            } else {
                builder.push_identity(end)?;
            }
        }
        builder.finish()
    }

    /// Returns the immutable authored UTF-8.
    #[must_use]
    pub fn source_text(&self) -> &str {
        &self.source
    }

    /// Returns the UTF-8 consumed by analysis and shaping.
    #[must_use]
    pub fn text(&self) -> &str {
        self.projected.as_deref().unwrap_or(&self.source)
    }

    /// Returns the complete ordered relation runs.
    #[must_use]
    pub fn segments(&self) -> ProjectionSegments<'_> {
        let identity = (self.projected.is_none() && !self.source.is_empty()).then(|| {
            let len = self.source_len();
            ProjectionSegment {
                kind: ProjectionKind::Identity,
                source: 0..len,
                projected: 0..len,
            }
        });
        ProjectionSegments {
            identity,
            runs: self.segments.iter(),
        }
    }

    /// Returns whether presentation text is exactly the authored allocation.
    #[must_use]
    pub const fn is_identity(&self) -> bool {
        self.projected.is_none()
    }

    /// Maps one presentation boundary to an authored boundary.
    ///
    /// At the interior of a replacement or collapse, downstream affinity maps
    /// before the authored unit and upstream affinity maps after it. At an
    /// omission boundary, the two affinities select the two authored sides.
    pub fn source_position(
        &self,
        projected: u32,
        affinity: TextAffinity,
    ) -> Result<u32, ProjectionError> {
        self.validate_projected_position(projected)?;
        if self.is_identity() {
            return Ok(projected);
        }

        if let Some(segment) = self.segments.iter().find(|segment| {
            segment.projected.start < projected && projected < segment.projected.end
        }) {
            return Ok(match segment.kind {
                ProjectionKind::Identity => {
                    segment.source.start + (projected - segment.projected.start)
                }
                ProjectionKind::Replacement | ProjectionKind::Collapsed => match affinity {
                    TextAffinity::Upstream => segment.source.end,
                    TextAffinity::Downstream => segment.source.start,
                },
                ProjectionKind::Inserted => segment.source.start,
                ProjectionKind::Omitted => unreachable!("omitted runs have no interior"),
            });
        }

        Ok(match affinity {
            TextAffinity::Upstream => self
                .segments
                .iter()
                .rev()
                .find(|segment| !segment.projected.is_empty() && segment.projected.end <= projected)
                .map_or(0, |segment| segment.source.end),
            TextAffinity::Downstream => self
                .segments
                .iter()
                .find(|segment| {
                    !segment.projected.is_empty() && segment.projected.start >= projected
                })
                .map_or(self.source_len(), |segment| segment.source.start),
        })
    }

    /// Maps one authored boundary to a presentation boundary.
    ///
    /// At the interior of a replacement or collapse, downstream affinity maps
    /// before the presentation unit and upstream affinity maps after it.
    /// Inserted text lies between the two affinities at its authored boundary.
    pub fn projected_position(
        &self,
        source: u32,
        affinity: TextAffinity,
    ) -> Result<u32, ProjectionError> {
        self.validate_source_position(source)?;
        if self.is_identity() {
            return Ok(source);
        }

        let mut inserted = self
            .segments
            .iter()
            .filter(|segment| {
                segment.kind == ProjectionKind::Inserted && segment.source.start == source
            })
            .peekable();
        if inserted.peek().is_some() {
            return Ok(match affinity {
                TextAffinity::Upstream => inserted
                    .map(|segment| segment.projected.start)
                    .min()
                    .unwrap_or(0),
                TextAffinity::Downstream => inserted
                    .map(|segment| segment.projected.end)
                    .max()
                    .unwrap_or(0),
            });
        }

        if let Some(segment) = self
            .segments
            .iter()
            .find(|segment| segment.source.start < source && source < segment.source.end)
        {
            return Ok(match segment.kind {
                ProjectionKind::Identity => {
                    segment.projected.start + (source - segment.source.start)
                }
                ProjectionKind::Replacement | ProjectionKind::Collapsed => match affinity {
                    TextAffinity::Upstream => segment.projected.end,
                    TextAffinity::Downstream => segment.projected.start,
                },
                ProjectionKind::Omitted => segment.projected.start,
                ProjectionKind::Inserted => unreachable!("inserted runs have no source interior"),
            });
        }

        Ok(match affinity {
            TextAffinity::Upstream => self
                .segments
                .iter()
                .rev()
                .find(|segment| !segment.source.is_empty() && segment.source.end <= source)
                .map_or(0, |segment| segment.projected.end),
            TextAffinity::Downstream => self
                .segments
                .iter()
                .find(|segment| !segment.source.is_empty() && segment.source.start >= source)
                .map_or(self.projected_len(), |segment| segment.projected.start),
        })
    }

    /// Maps a nonempty presentation interval to its complete authored interval.
    ///
    /// A partial interval inside a replacement or collapse owns the complete
    /// authored transformation unit. An interval containing only inserted text
    /// maps to an empty authored interval at the insertion boundary.
    pub fn source_range(&self, projected: Range<u32>) -> Result<Range<u32>, ProjectionError> {
        self.validate_projected_range(&projected)?;
        if projected.is_empty() {
            let source = self.source_position(projected.start, TextAffinity::Upstream)?;
            return Ok(source..source);
        }
        let start = self.source_position(projected.start, TextAffinity::Downstream)?;
        let end = self.source_position(projected.end, TextAffinity::Upstream)?;
        Ok(start.min(end)..start.max(end))
    }

    /// Maps a nonempty authored interval to its complete presentation interval.
    ///
    /// A partial interval inside a replacement or collapse owns the complete
    /// presentation transformation unit. An omitted interval maps to an empty
    /// presentation interval.
    pub fn projected_range(&self, source: Range<u32>) -> Result<Range<u32>, ProjectionError> {
        self.validate_source_range(&source)?;
        if source.is_empty() {
            let projected = self.projected_position(source.start, TextAffinity::Upstream)?;
            return Ok(projected..projected);
        }
        let start = self.projected_position(source.start, TextAffinity::Downstream)?;
        let end = self.projected_position(source.end, TextAffinity::Upstream)?;
        Ok(start.min(end)..start.max(end))
    }

    /// Returns the authored owner of one presentation interval.
    ///
    /// Replaced, collapsed, and inserted presentation uses the first authored
    /// boundary contributing to the interval. Hosts can use this position to
    /// select style or semantic ownership while retaining the complete source
    /// interval separately.
    pub fn source_owner(&self, projected: Range<u32>) -> Result<u32, ProjectionError> {
        self.validate_projected_range(&projected)?;
        if projected.is_empty() {
            self.source_position(projected.start, TextAffinity::Upstream)
        } else {
            self.source_position(projected.start, TextAffinity::Downstream)
        }
    }

    fn source_len(&self) -> u32 {
        u32::try_from(self.source.len()).expect("validated projection length fits u32")
    }

    fn projected_len(&self) -> u32 {
        u32::try_from(self.text().len()).expect("validated projection length fits u32")
    }

    fn validate_source_position(&self, position: u32) -> Result<(), ProjectionError> {
        if position > self.source_len() {
            return Err(ProjectionError::new(
                ProjectionErrorKind::InvalidSourceRange,
            ));
        }
        if !self.source.is_char_boundary(position as usize) {
            return Err(ProjectionError::new(
                ProjectionErrorKind::InvalidSourceBoundary,
            ));
        }
        Ok(())
    }

    fn validate_projected_position(&self, position: u32) -> Result<(), ProjectionError> {
        if position > self.projected_len() {
            return Err(ProjectionError::new(
                ProjectionErrorKind::InvalidProjectedRange,
            ));
        }
        if !self.text().is_char_boundary(position as usize) {
            return Err(ProjectionError::new(
                ProjectionErrorKind::InvalidProjectedBoundary,
            ));
        }
        Ok(())
    }

    fn validate_source_range(&self, range: &Range<u32>) -> Result<(), ProjectionError> {
        if range.start > range.end || range.end > self.source_len() {
            return Err(ProjectionError::new(
                ProjectionErrorKind::InvalidSourceRange,
            ));
        }
        self.validate_source_position(range.start)?;
        self.validate_source_position(range.end)
    }

    fn validate_projected_range(&self, range: &Range<u32>) -> Result<(), ProjectionError> {
        if range.start > range.end || range.end > self.projected_len() {
            return Err(ProjectionError::new(
                ProjectionErrorKind::InvalidProjectedRange,
            ));
        }
        self.validate_projected_position(range.start)?;
        self.validate_projected_position(range.end)
    }
}

/// Monotonic builder for custom source-complete transformations.
///
/// Source-consuming operations advance from the previous authored boundary.
/// [`ProjectionBuilder::finish`] succeeds only after every authored byte has
/// been related. Output strings are copied only after the first nonidentity
/// operation.
#[derive(Debug)]
pub struct ProjectionBuilder {
    source: String,
    projected: Option<String>,
    segments: Vec<ProjectionSegment>,
    source_cursor: u32,
    projected_cursor: u32,
}

impl ProjectionBuilder {
    /// Starts a projection at authored byte zero.
    pub fn new(source: impl Into<String>) -> Result<Self, ProjectionError> {
        let source = source.into();
        text_len(&source)?;
        Ok(Self {
            source,
            projected: None,
            segments: Vec::new(),
            source_cursor: 0,
            projected_cursor: 0,
        })
    }

    /// Relates authored bytes through `source_end` identically.
    pub fn push_identity(&mut self, source_end: u32) -> Result<(), ProjectionError> {
        self.validate_source_end(source_end)?;
        let start = self.source_cursor;
        let delta = source_end - start;
        let projected_end = self
            .projected_cursor
            .checked_add(delta)
            .ok_or_else(|| ProjectionError::new(ProjectionErrorKind::TextTooLong))?;
        if let Some(projected) = &mut self.projected {
            let slice = self
                .source
                .get(start as usize..source_end as usize)
                .ok_or_else(|| ProjectionError::new(ProjectionErrorKind::InvalidSourceBoundary))?;
            projected.push_str(slice);
            self.append_segment(ProjectionSegment {
                kind: ProjectionKind::Identity,
                source: start..source_end,
                projected: self.projected_cursor..projected_end,
            });
        }
        self.source_cursor = source_end;
        self.projected_cursor = projected_end;
        Ok(())
    }

    /// Replaces authored bytes through `source_end` with nonempty `projected`.
    pub fn push_replacement(
        &mut self,
        source_end: u32,
        projected: &str,
    ) -> Result<(), ProjectionError> {
        self.push_transformed(ProjectionKind::Replacement, source_end, projected)
    }

    /// Collapses authored bytes through `source_end` to nonempty `projected`.
    pub fn push_collapsed(
        &mut self,
        source_end: u32,
        projected: &str,
    ) -> Result<(), ProjectionError> {
        self.push_transformed(ProjectionKind::Collapsed, source_end, projected)
    }

    /// Omits authored bytes through `source_end` from presentation.
    pub fn push_omitted(&mut self, source_end: u32) -> Result<(), ProjectionError> {
        self.validate_source_end(source_end)?;
        self.materialize_projected();
        let start = self.source_cursor;
        self.append_segment(ProjectionSegment {
            kind: ProjectionKind::Omitted,
            source: start..source_end,
            projected: self.projected_cursor..self.projected_cursor,
        });
        self.source_cursor = source_end;
        Ok(())
    }

    /// Inserts nonempty presentation text at the current authored boundary.
    pub fn push_inserted(&mut self, projected: &str) -> Result<(), ProjectionError> {
        if projected.is_empty() {
            return Err(ProjectionError::new(
                ProjectionErrorKind::EmptyProjectedText,
            ));
        }
        self.materialize_projected();
        let projected_len = text_len(projected)?;
        let projected_end = self
            .projected_cursor
            .checked_add(projected_len)
            .ok_or_else(|| ProjectionError::new(ProjectionErrorKind::TextTooLong))?;
        self.projected
            .as_mut()
            .expect("insertions materialize presentation text")
            .push_str(projected);
        self.append_segment(ProjectionSegment {
            kind: ProjectionKind::Inserted,
            source: self.source_cursor..self.source_cursor,
            projected: self.projected_cursor..projected_end,
        });
        self.projected_cursor = projected_end;
        Ok(())
    }

    /// Finishes after verifying complete authored coverage.
    pub fn finish(self) -> Result<ProjectedText, ProjectionError> {
        if self.source_cursor != self.source_len() {
            return Err(ProjectionError::new(ProjectionErrorKind::IncompleteSource));
        }
        Ok(ProjectedText {
            source: self.source,
            projected: self.projected,
            segments: self.segments,
        })
    }

    fn push_transformed(
        &mut self,
        kind: ProjectionKind,
        source_end: u32,
        projected: &str,
    ) -> Result<(), ProjectionError> {
        self.validate_source_end(source_end)?;
        if projected.is_empty() {
            return Err(ProjectionError::new(
                ProjectionErrorKind::EmptyProjectedText,
            ));
        }
        self.materialize_projected();
        let projected_len = text_len(projected)?;
        let projected_end = self
            .projected_cursor
            .checked_add(projected_len)
            .ok_or_else(|| ProjectionError::new(ProjectionErrorKind::TextTooLong))?;
        self.projected
            .as_mut()
            .expect("transforms materialize presentation text")
            .push_str(projected);
        self.append_segment(ProjectionSegment {
            kind,
            source: self.source_cursor..source_end,
            projected: self.projected_cursor..projected_end,
        });
        self.source_cursor = source_end;
        self.projected_cursor = projected_end;
        Ok(())
    }

    fn append_segment(&mut self, segment: ProjectionSegment) {
        if let Some(previous) = self.segments.last_mut()
            && previous.kind == segment.kind
            && matches!(
                segment.kind,
                ProjectionKind::Identity | ProjectionKind::Inserted | ProjectionKind::Omitted
            )
            && previous.source.end == segment.source.start
            && previous.projected.end == segment.projected.start
        {
            previous.source.end = segment.source.end;
            previous.projected.end = segment.projected.end;
            return;
        }
        self.segments.push(segment);
    }

    fn validate_source_end(&self, source_end: u32) -> Result<(), ProjectionError> {
        if source_end <= self.source_cursor || source_end > self.source_len() {
            return Err(ProjectionError::new(
                ProjectionErrorKind::InvalidSourceRange,
            ));
        }
        if !self.source.is_char_boundary(source_end as usize) {
            return Err(ProjectionError::new(
                ProjectionErrorKind::InvalidSourceBoundary,
            ));
        }
        Ok(())
    }

    fn materialize_projected(&mut self) {
        if self.projected.is_none() {
            self.projected = Some(self.source[..self.source_cursor as usize].to_string());
            if self.source_cursor > 0 {
                self.segments.push(ProjectionSegment {
                    kind: ProjectionKind::Identity,
                    source: 0..self.source_cursor,
                    projected: 0..self.projected_cursor,
                });
            }
        }
    }

    fn source_text(&self) -> &str {
        &self.source
    }

    const fn source_cursor(&self) -> u32 {
        self.source_cursor
    }

    fn source_len(&self) -> u32 {
        u32::try_from(self.source.len()).expect("validated projection length fits u32")
    }
}

fn text_len(text: &str) -> Result<u32, ProjectionError> {
    u32::try_from(text.len()).map_err(|_| ProjectionError::new(ProjectionErrorKind::TextTooLong))
}

const fn is_document_whitespace(byte: u8) -> bool {
    matches!(byte, b' ' | b'\t' | b'\n' | b'\r' | 0x0c)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_reuses_the_authored_string_and_one_run() {
        let projection =
            ProjectedText::from_whitespace("hello".to_string(), WhitespaceCollapse::Preserve)
                .expect("identity text is valid");
        assert!(projection.is_identity());
        assert_eq!(projection.source_text(), "hello");
        assert_eq!(projection.text(), "hello");
        assert_eq!(projection.segments().len(), 1);
        assert_eq!(
            projection
                .segments()
                .next()
                .expect("identity relation must be observable")
                .kind(),
            ProjectionKind::Identity
        );
    }

    #[test]
    fn whitespace_collapse_is_dense_and_does_not_treat_nbsp_as_space() {
        let projection =
            ProjectedText::from_whitespace("a \t\r\n\x0c b\u{a0}c", WhitespaceCollapse::Collapse)
                .expect("whitespace input is valid");
        assert_eq!(projection.text(), "a b\u{a0}c");
        assert_eq!(
            projection
                .segments()
                .map(|segment| segment.kind())
                .collect::<Vec<_>>(),
            [
                ProjectionKind::Identity,
                ProjectionKind::Collapsed,
                ProjectionKind::Identity,
            ]
        );
        let collapsed = projection
            .segments()
            .nth(1)
            .expect("collapsed relation must be observable");
        assert_eq!(collapsed.source(), 1..7);
        assert_eq!(collapsed.projected(), 1..2);
        assert_eq!(
            projection.source_range(1..2).expect("space maps back"),
            1..7
        );
    }

    #[test]
    fn builder_proves_every_relation_and_ambiguous_affinity() {
        let mut builder =
            ProjectionBuilder::new("aß  x!".to_string()).expect("fixture source is valid");
        builder.push_identity(1).expect("identity is valid");
        builder
            .push_replacement(3, "SS")
            .expect("one-to-many scalar replacement is valid");
        builder.push_collapsed(5, " ").expect("collapse is valid");
        builder.push_omitted(6).expect("omission is valid");
        builder
            .push_inserted("→")
            .expect("insertion is valid at the current boundary");
        builder.push_identity(7).expect("suffix identity is valid");
        let projection = builder.finish().expect("source is completely covered");

        assert_eq!(projection.text(), "aSS →!");
        assert_eq!(
            projection
                .segments()
                .map(|segment| segment.kind())
                .collect::<Vec<_>>(),
            [
                ProjectionKind::Identity,
                ProjectionKind::Replacement,
                ProjectionKind::Collapsed,
                ProjectionKind::Omitted,
                ProjectionKind::Inserted,
                ProjectionKind::Identity,
            ]
        );
        assert_eq!(projection.source_range(1..2).expect("first S maps"), 1..3);
        assert_eq!(projection.source_range(2..3).expect("second S maps"), 1..3);
        assert_eq!(projection.source_position(4, TextAffinity::Upstream), Ok(5));
        assert_eq!(
            projection.source_position(4, TextAffinity::Downstream),
            Ok(6)
        );
        assert_eq!(
            projection.projected_position(6, TextAffinity::Upstream),
            Ok(4)
        );
        assert_eq!(
            projection.projected_position(6, TextAffinity::Downstream),
            Ok(7)
        );
        assert_eq!(
            projection.source_range(4..7).expect("inserted arrow maps"),
            6..6
        );
    }

    #[test]
    fn invalid_boundaries_and_incomplete_coverage_are_rejected() {
        let mut builder = ProjectionBuilder::new("éx").expect("fixture source is valid");
        assert_eq!(
            builder
                .push_identity(1)
                .expect_err("mid-scalar source is invalid")
                .kind(),
            ProjectionErrorKind::InvalidSourceBoundary
        );
        builder.push_identity(2).expect("whole scalar is valid");
        assert_eq!(
            builder.finish().expect_err("suffix is uncovered").kind(),
            ProjectionErrorKind::IncompleteSource
        );

        let projection = ProjectedText::identity("é").expect("identity is valid");
        assert_eq!(
            projection
                .source_range(1..2)
                .expect_err("mid-scalar projection range is invalid")
                .kind(),
            ProjectionErrorKind::InvalidProjectedBoundary
        );
    }

    #[test]
    fn long_identity_and_dense_collapse_store_runs_not_byte_maps() {
        let identity =
            ProjectedText::identity("a".repeat(32_768)).expect("identity fixture is valid");
        assert_eq!(identity.segments().len(), 1);
        assert!(identity.is_identity());

        let collapse =
            ProjectedText::from_whitespace(" \t\r\n".repeat(8_192), WhitespaceCollapse::Collapse)
                .expect("dense collapse fixture is valid");
        assert_eq!(collapse.text(), " ");
        assert_eq!(collapse.segments().len(), 1);
        assert_eq!(
            collapse
                .segments()
                .next()
                .expect("collapse relation must be observable")
                .kind(),
            ProjectionKind::Collapsed
        );
    }
}
