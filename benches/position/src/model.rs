// Copyright 2026 the Underwood Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::mem::size_of;
use std::ops::Range;
use std::sync::Arc;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Revision(u64);

impl Revision {
    pub(crate) const INITIAL: Self = Self(0);

    pub(crate) const fn get(self) -> u64 {
        self.0
    }

    fn next(self) -> Self {
        Self(self.0.checked_add(1).expect("trace revision overflow"))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Bias {
    Before,
    After,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct AnchorToken(usize);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct AnchorState {
    offset: usize,
    bias: Bias,
    resolved: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct EdgeBehavior {
    pub(crate) start: Bias,
    pub(crate) end: Bias,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AuthoredSpan {
    pub(crate) range: Range<usize>,
    pub(crate) edges: EdgeBehavior,
    pub(crate) value: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SnapshotRange {
    pub(crate) revision: Revision,
    pub(crate) bytes: Range<usize>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct WorkCounters {
    pub(crate) anchors_visited: usize,
    pub(crate) anchors_resolved: usize,
    pub(crate) authored_spans_visited: usize,
    pub(crate) source_bytes_copied: usize,
    pub(crate) snapshot_records_visited: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct EditSummary {
    pub(crate) before: Revision,
    pub(crate) after: Revision,
    pub(crate) replaced: Range<usize>,
    pub(crate) inserted_bytes: usize,
    pub(crate) work: WorkCounters,
}

#[derive(Clone, Debug)]
pub(crate) struct Snapshot {
    revision: Revision,
    text: Arc<str>,
    authored: Arc<[AuthoredSpan]>,
}

impl Snapshot {
    pub(crate) fn revision(&self) -> Revision {
        self.revision
    }

    pub(crate) fn text(&self) -> &str {
        &self.text
    }

    pub(crate) fn authored(&self) -> &[AuthoredSpan] {
        &self.authored
    }

    pub(crate) fn shares_text_with(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.text, &other.text)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum ModelError {
    InvalidRange {
        range: Range<usize>,
        text_len: usize,
    },
    NonBoundary {
        offset: usize,
    },
    UnknownAnchor(AnchorToken),
    UnresolvedAnchor(AnchorToken),
    StaleRange {
        expected: Revision,
        actual: Revision,
    },
}

#[derive(Debug)]
pub(crate) struct CanonicalBaseline {
    revision: Revision,
    text: Arc<str>,
    anchors: Vec<AnchorState>,
    authored: Vec<AuthoredSpan>,
}

pub(crate) const fn anchor_record_bytes() -> usize {
    size_of::<AnchorState>()
}

pub(crate) const fn authored_span_bytes() -> usize {
    size_of::<AuthoredSpan>()
}

pub(crate) const fn derived_range_bytes() -> usize {
    size_of::<SnapshotRange>()
}

pub(crate) const fn baseline_inline_bytes() -> usize {
    size_of::<CanonicalBaseline>()
}

impl CanonicalBaseline {
    pub(crate) fn new(text: &str) -> Self {
        Self {
            revision: Revision::INITIAL,
            text: Arc::from(text),
            anchors: Vec::new(),
            authored: Vec::new(),
        }
    }

    pub(crate) fn text_len(&self) -> usize {
        self.text.len()
    }

    pub(crate) fn snapshot(&self) -> (Snapshot, WorkCounters) {
        let work = WorkCounters {
            snapshot_records_visited: self.authored.len(),
            ..WorkCounters::default()
        };
        (
            Snapshot {
                revision: self.revision,
                text: Arc::clone(&self.text),
                authored: Arc::from(self.authored.clone()),
            },
            work,
        )
    }

    pub(crate) fn create_anchor(
        &mut self,
        offset: usize,
        bias: Bias,
    ) -> Result<AnchorToken, ModelError> {
        self.validate_boundary(offset)?;
        let token = AnchorToken(self.anchors.len());
        self.anchors.push(AnchorState {
            offset,
            bias,
            resolved: true,
        });
        Ok(token)
    }

    pub(crate) fn resolve_anchor(&self, token: AnchorToken) -> Result<usize, ModelError> {
        let Some(anchor) = self.anchors.get(token.0) else {
            return Err(ModelError::UnknownAnchor(token));
        };
        if anchor.resolved {
            Ok(anchor.offset)
        } else {
            Err(ModelError::UnresolvedAnchor(token))
        }
    }

    pub(crate) fn replace_authored(&mut self, mut authored: Vec<AuthoredSpan>) {
        authored.sort_by_key(|span| (span.range.start, span.range.end, span.value));
        self.authored = authored;
    }

    pub(crate) fn authored(&self) -> &[AuthoredSpan] {
        &self.authored
    }

    pub(crate) fn snapshot_range(&self, bytes: Range<usize>) -> SnapshotRange {
        SnapshotRange {
            revision: self.revision,
            bytes,
        }
    }

    pub(crate) fn resolve_snapshot_range(&self, range: &SnapshotRange) -> Result<&str, ModelError> {
        if range.revision != self.revision {
            return Err(ModelError::StaleRange {
                expected: self.revision,
                actual: range.revision,
            });
        }
        self.validate_range(range.bytes.clone())?;
        Ok(&self.text[range.bytes.clone()])
    }

    pub(crate) fn replace(
        &mut self,
        replaced: Range<usize>,
        inserted: &str,
    ) -> Result<EditSummary, ModelError> {
        self.validate_range(replaced.clone())?;

        let before = self.revision;
        let mut next = String::with_capacity(
            self.text
                .len()
                .checked_sub(replaced.len())
                .and_then(|len| len.checked_add(inserted.len()))
                .expect("trace text length overflow"),
        );
        next.push_str(&self.text[..replaced.start]);
        next.push_str(inserted);
        next.push_str(&self.text[replaced.end..]);

        let mut work = WorkCounters {
            source_bytes_copied: next.len(),
            ..WorkCounters::default()
        };
        for anchor in &mut self.anchors {
            if anchor.resolved {
                anchor.offset = map_boundary(anchor.offset, anchor.bias, &replaced, inserted.len());
                work.anchors_visited += 1;
            }
        }
        for span in &mut self.authored {
            span.range.start = map_boundary(
                span.range.start,
                span.edges.start,
                &replaced,
                inserted.len(),
            );
            span.range.end =
                map_boundary(span.range.end, span.edges.end, &replaced, inserted.len());
            span.range.end = span.range.end.max(span.range.start);
            work.authored_spans_visited += 1;
        }
        self.authored
            .sort_by_key(|span| (span.range.start, span.range.end, span.value));

        self.text = Arc::from(next);
        self.revision = before.next();
        Ok(EditSummary {
            before,
            after: self.revision,
            replaced,
            inserted_bytes: inserted.len(),
            work,
        })
    }

    fn validate_range(&self, range: Range<usize>) -> Result<(), ModelError> {
        if range.start > range.end || range.end > self.text.len() {
            return Err(ModelError::InvalidRange {
                range,
                text_len: self.text.len(),
            });
        }
        self.validate_boundary(range.start)?;
        self.validate_boundary(range.end)
    }

    fn validate_boundary(&self, offset: usize) -> Result<(), ModelError> {
        if offset > self.text.len() {
            return Err(ModelError::InvalidRange {
                range: offset..offset,
                text_len: self.text.len(),
            });
        }
        if self.text.is_char_boundary(offset) {
            Ok(())
        } else {
            Err(ModelError::NonBoundary { offset })
        }
    }
}

fn map_boundary(offset: usize, bias: Bias, replaced: &Range<usize>, inserted: usize) -> usize {
    if replaced.is_empty() {
        return match offset.cmp(&replaced.start) {
            std::cmp::Ordering::Less => offset,
            std::cmp::Ordering::Equal => {
                if bias == Bias::After {
                    offset + inserted
                } else {
                    offset
                }
            }
            std::cmp::Ordering::Greater => offset + inserted,
        };
    }

    if offset < replaced.start {
        return offset;
    }
    if offset >= replaced.end {
        return shift_after_replacement(offset, replaced, inserted);
    }
    match bias {
        Bias::Before => replaced.start,
        Bias::After => replaced.start + inserted,
    }
}

fn shift_after_replacement(offset: usize, replaced: &Range<usize>, inserted: usize) -> usize {
    if inserted >= replaced.len() {
        offset + (inserted - replaced.len())
    } else {
        offset - (replaced.len() - inserted)
    }
}

#[cfg(test)]
mod tests {
    use super::{Bias, CanonicalBaseline, ModelError};

    #[test]
    fn rejects_a_non_utf8_boundary() {
        let mut model = CanonicalBaseline::new("aéb");
        let error = model
            .create_anchor(2, Bias::Before)
            .expect_err("middle of UTF-8 scalar must be rejected");
        assert_eq!(error, ModelError::NonBoundary { offset: 2 });
    }

    #[test]
    fn maps_a_boundary_after_replacement() {
        let mut model = CanonicalBaseline::new("abcdef");
        let end = model.create_anchor(5, Bias::Before).expect("valid anchor");
        model.replace(2..5, "X").expect("valid replacement");
        assert_eq!(model.resolve_anchor(end), Ok(3));
    }

    #[test]
    fn deletion_keeps_bias_for_the_next_insertion() {
        let mut model = CanonicalBaseline::new("abcdef");
        let before = model.create_anchor(3, Bias::Before).expect("valid anchor");
        let after = model.create_anchor(3, Bias::After).expect("valid anchor");
        model.replace(2..5, "").expect("valid deletion");
        assert_eq!(model.resolve_anchor(before), Ok(2));
        assert_eq!(model.resolve_anchor(after), Ok(2));
        model.replace(2..2, "X").expect("valid insertion");
        assert_eq!(model.resolve_anchor(before), Ok(2));
        assert_eq!(model.resolve_anchor(after), Ok(3));
    }

    #[test]
    fn stale_snapshot_range_fails_instead_of_drifting() {
        let mut model = CanonicalBaseline::new("abcdef");
        let range = model.snapshot_range(1..3);
        model.replace(0..0, "X").expect("valid insertion");
        assert!(matches!(
            model.resolve_snapshot_range(&range),
            Err(ModelError::StaleRange { .. })
        ));
    }
}
