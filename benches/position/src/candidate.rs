// Copyright 2026 the Underwood Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::ops::Range;
use std::sync::Arc;

use crate::model::{AuthoredSpan, ModelError, map_boundary};

const CHUNK_TARGET: usize = 4 * 1024;
const RANGE_BLOCK_TARGET: usize = 1024;

#[derive(Clone, Debug)]
enum AppendNode {
    Leaf(Arc<str>),
    Branch {
        len: usize,
        left: Arc<Self>,
        right: Arc<Self>,
    },
}

impl AppendNode {
    fn len(&self) -> usize {
        match self {
            Self::Leaf(text) => text.len(),
            Self::Branch { len, .. } => *len,
        }
    }

    fn branch(left: Arc<Self>, right: Arc<Self>) -> Self {
        Self::Branch {
            len: left
                .len()
                .checked_add(right.len())
                .expect("trace append length overflow"),
            left,
            right,
        }
    }

    fn retained_batches(&self) -> usize {
        match self {
            Self::Leaf(_) => 1,
            Self::Branch { left, right, .. } => left.retained_batches() + right.retained_batches(),
        }
    }

    #[cfg(test)]
    fn append_to(&self, text: &mut String) {
        match self {
            Self::Leaf(leaf) => text.push_str(leaf),
            Self::Branch { left, right, .. } => {
                left.append_to(text);
                right.append_to(text);
            }
        }
    }
}

#[derive(Clone, Debug, Default)]
pub(crate) struct AppendStream {
    len: usize,
    batches: usize,
    levels: Arc<[Option<Arc<AppendNode>>]>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct AppendWork {
    pub(crate) level_records_visited: usize,
    pub(crate) node_records_created: usize,
    pub(crate) source_bytes_copied: usize,
    pub(crate) unpublished_batches: usize,
}

impl AppendStream {
    pub(crate) fn len(&self) -> usize {
        self.len
    }

    pub(crate) fn batches(&self) -> usize {
        self.batches
    }

    pub(crate) fn retained_batches(&self) -> usize {
        self.levels
            .iter()
            .flatten()
            .map(|node| node.retained_batches())
            .sum()
    }

    pub(crate) fn append(&self, batch: Arc<str>) -> (Self, AppendWork) {
        let batch_len = batch.len();
        let mut work = AppendWork {
            node_records_created: 1,
            ..AppendWork::default()
        };
        let mut levels = self.levels.to_vec();
        let mut carry = Arc::new(AppendNode::Leaf(batch));
        let mut level = 0;

        loop {
            work.level_records_visited += 1;
            if level == levels.len() {
                levels.push(Some(carry));
                break;
            }
            match levels[level].take() {
                Some(left) => {
                    carry = Arc::new(AppendNode::branch(left, carry));
                    work.node_records_created += 1;
                    level += 1;
                }
                None => {
                    levels[level] = Some(carry);
                    break;
                }
            }
        }

        (
            Self {
                len: self
                    .len
                    .checked_add(batch_len)
                    .expect("trace append length overflow"),
                batches: self
                    .batches
                    .checked_add(1)
                    .expect("trace append batch count overflow"),
                levels: Arc::from(levels),
            },
            work,
        )
    }

    #[cfg(test)]
    pub(crate) fn to_text(&self) -> String {
        let mut text = String::with_capacity(self.len);
        for node in self.levels.iter().rev().flatten() {
            node.append_to(&mut text);
        }
        text
    }
}

#[derive(Clone, Debug)]
struct TextChunk {
    start: usize,
    text: Arc<str>,
}

impl TextChunk {
    fn end(&self) -> usize {
        self.start + self.text.len()
    }
}

#[derive(Clone, Debug)]
pub(crate) struct ChunkedText {
    len: usize,
    chunks: Arc<[TextChunk]>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct TextEditWork {
    pub(crate) chunk_records_visited: usize,
    pub(crate) source_bytes_copied: usize,
    pub(crate) source_chunks_reused: usize,
}

impl ChunkedText {
    pub(crate) fn new(text: &str) -> Self {
        Self {
            len: text.len(),
            chunks: Arc::from(chunk_string(0, text)),
        }
    }

    pub(crate) fn len(&self) -> usize {
        self.len
    }

    pub(crate) fn shares_index_with(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.chunks, &other.chunks)
    }

    #[cfg(test)]
    pub(crate) fn to_text(&self) -> String {
        let mut text = String::with_capacity(self.len);
        for chunk in self.chunks.iter() {
            text.push_str(&chunk.text);
        }
        text
    }

    pub(crate) fn replace(
        &self,
        replaced: Range<usize>,
        inserted: &str,
    ) -> Result<(Self, TextEditWork), ModelError> {
        self.validate_range(replaced.clone())?;
        let removed = replaced.len();
        let next_len = self
            .len
            .checked_sub(removed)
            .and_then(|len| len.checked_add(inserted.len()))
            .expect("trace text length overflow");
        let delta = signed_delta(inserted.len(), removed);

        let mut before = Vec::new();
        let mut after = Vec::new();
        let mut prefix_fragment = "";
        let mut suffix_fragment = "";
        let mut replacement_start = replaced.start;
        let mut work = TextEditWork::default();

        for chunk in self.chunks.iter() {
            work.chunk_records_visited += 1;
            if chunk.end() <= replaced.start {
                before.push(chunk.clone());
                work.source_chunks_reused += 1;
                continue;
            }
            if chunk.start >= replaced.end && !(replaced.is_empty() && chunk.start < replaced.start)
            {
                let mut shifted = chunk.clone();
                shifted.start = shift_offset(shifted.start, delta);
                after.push(shifted);
                work.source_chunks_reused += 1;
                continue;
            }

            if chunk.start < replaced.start && replaced.start < chunk.end() {
                let local = replaced.start - chunk.start;
                prefix_fragment = &chunk.text[..local];
                replacement_start = chunk.start;
            }
            if chunk.start < replaced.end && replaced.end < chunk.end() {
                let local = replaced.end - chunk.start;
                suffix_fragment = &chunk.text[local..];
            }
            if replaced.is_empty() && chunk.start < replaced.start && replaced.start < chunk.end() {
                let local = replaced.start - chunk.start;
                prefix_fragment = &chunk.text[..local];
                suffix_fragment = &chunk.text[local..];
                replacement_start = chunk.start;
            }
        }

        let mut replacement =
            String::with_capacity(prefix_fragment.len() + inserted.len() + suffix_fragment.len());
        replacement.push_str(prefix_fragment);
        replacement.push_str(inserted);
        replacement.push_str(suffix_fragment);
        work.source_bytes_copied = replacement.len();

        let replacement_chunks = chunk_string(replacement_start, &replacement);
        let mut chunks = Vec::with_capacity(before.len() + replacement_chunks.len() + after.len());
        chunks.extend(before);
        chunks.extend(replacement_chunks);
        chunks.extend(after);

        debug_assert_eq!(
            chunks.last().map_or(0, TextChunk::end),
            next_len,
            "candidate chunk index must cover the full text"
        );
        Ok((
            Self {
                len: next_len,
                chunks: Arc::from(chunks),
            },
            work,
        ))
    }

    fn validate_range(&self, range: Range<usize>) -> Result<(), ModelError> {
        if range.start > range.end || range.end > self.len {
            return Err(ModelError::InvalidRange {
                range,
                text_len: self.len,
            });
        }
        self.validate_boundary(range.start)?;
        self.validate_boundary(range.end)
    }

    fn validate_boundary(&self, offset: usize) -> Result<(), ModelError> {
        if offset == self.len {
            return Ok(());
        }
        let Some(chunk) = self
            .chunks
            .iter()
            .find(|chunk| chunk.start <= offset && offset < chunk.end())
        else {
            return Err(ModelError::InvalidRange {
                range: offset..offset,
                text_len: self.len,
            });
        };
        let local = offset - chunk.start;
        if chunk.text.is_char_boundary(local) {
            Ok(())
        } else {
            Err(ModelError::NonBoundary { offset })
        }
    }
}

#[derive(Clone, Debug)]
struct RangeBlock {
    shift: isize,
    spans: Arc<[AuthoredSpan]>,
    min_start: usize,
    max_end: usize,
}

impl RangeBlock {
    fn new(shift: isize, spans: Arc<[AuthoredSpan]>) -> Self {
        let min_start = spans
            .iter()
            .map(|span| span.range.start)
            .min()
            .expect("candidate range block is nonempty");
        let max_end = spans
            .iter()
            .map(|span| span.range.end)
            .max()
            .expect("candidate range block is nonempty");
        Self {
            shift,
            spans,
            min_start,
            max_end,
        }
    }

    fn first_start(&self) -> usize {
        shift_offset(self.min_start, self.shift)
    }

    fn last_end(&self) -> usize {
        shift_offset(self.max_end, self.shift)
    }
}

#[derive(Clone, Debug)]
pub(crate) struct BlockedRanges {
    blocks: Arc<[RangeBlock]>,
    len: usize,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct RangeEditWork {
    pub(crate) block_records_visited: usize,
    pub(crate) spans_visited: usize,
    pub(crate) span_blocks_reused: usize,
}

impl BlockedRanges {
    pub(crate) fn new(mut spans: Vec<AuthoredSpan>) -> Self {
        spans.sort_by_key(|span| (span.range.start, span.range.end, span.value));
        let len = spans.len();
        let blocks = spans
            .chunks(RANGE_BLOCK_TARGET)
            .map(|block| RangeBlock::new(0, Arc::from(block.to_vec())))
            .collect::<Vec<_>>();
        Self {
            blocks: Arc::from(blocks),
            len,
        }
    }

    pub(crate) fn len(&self) -> usize {
        self.len
    }

    pub(crate) fn shares_index_with(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.blocks, &other.blocks)
    }

    pub(crate) fn transform(
        &self,
        replaced: Range<usize>,
        inserted: usize,
    ) -> (Self, RangeEditWork) {
        let delta = signed_delta(inserted, replaced.len());
        let mut before = Vec::new();
        let mut affected = Vec::new();
        let mut after = Vec::new();
        let mut work = RangeEditWork::default();

        for block in self.blocks.iter() {
            work.block_records_visited += 1;
            if block.last_end() < replaced.start {
                before.push(block.clone());
                work.span_blocks_reused += 1;
                continue;
            }
            if block.first_start() > replaced.end {
                let mut shifted = block.clone();
                shifted.shift = shifted
                    .shift
                    .checked_add(delta)
                    .expect("trace range shift overflow");
                after.push(shifted);
                work.span_blocks_reused += 1;
                continue;
            }

            for span in block.spans.iter() {
                let start = shift_offset(span.range.start, block.shift);
                let end = shift_offset(span.range.end, block.shift);
                let mapped_start = map_boundary(start, span.edges.start, &replaced, inserted);
                let mapped_end =
                    map_boundary(end, span.edges.end, &replaced, inserted).max(mapped_start);
                affected.push(AuthoredSpan {
                    range: mapped_start..mapped_end,
                    edges: span.edges,
                    value: span.value,
                });
                work.spans_visited += 1;
            }
        }

        affected.sort_by_key(|span| (span.range.start, span.range.end, span.value));
        let affected_blocks = affected
            .chunks(RANGE_BLOCK_TARGET)
            .map(|block| RangeBlock::new(0, Arc::from(block.to_vec())));
        let mut next = Vec::with_capacity(
            before.len() + affected.len().div_ceil(RANGE_BLOCK_TARGET) + after.len(),
        );
        next.extend(before);
        next.extend(affected_blocks);
        next.extend(after);

        (
            Self {
                blocks: Arc::from(next),
                len: self.len,
            },
            work,
        )
    }

    #[cfg(test)]
    pub(crate) fn materialize(&self) -> Vec<AuthoredSpan> {
        let mut spans = Vec::with_capacity(self.len);
        for block in self.blocks.iter() {
            spans.extend(block.spans.iter().map(|span| AuthoredSpan {
                range: shift_offset(span.range.start, block.shift)
                    ..shift_offset(span.range.end, block.shift),
                edges: span.edges,
                value: span.value,
            }));
        }
        spans
    }
}

fn chunk_string(start: usize, text: &str) -> Vec<TextChunk> {
    let mut chunks = Vec::new();
    let mut chunk_start = 0;
    while chunk_start < text.len() {
        let mut chunk_end = (chunk_start + CHUNK_TARGET).min(text.len());
        while chunk_end > chunk_start && !text.is_char_boundary(chunk_end) {
            chunk_end -= 1;
        }
        if chunk_end == chunk_start {
            chunk_end = text[chunk_start..]
                .char_indices()
                .nth(1)
                .map_or(text.len(), |(offset, _)| chunk_start + offset);
        }
        chunks.push(TextChunk {
            start: start + chunk_start,
            text: Arc::from(&text[chunk_start..chunk_end]),
        });
        chunk_start = chunk_end;
    }
    chunks
}

fn signed_delta(inserted: usize, removed: usize) -> isize {
    if inserted >= removed {
        isize::try_from(inserted - removed).expect("trace delta fits isize")
    } else {
        -isize::try_from(removed - inserted).expect("trace delta fits isize")
    }
}

fn shift_offset(offset: usize, delta: isize) -> usize {
    offset
        .checked_add_signed(delta)
        .expect("trace offset shift remains nonnegative")
}

#[cfg(test)]
mod tests {
    use crate::model::{Bias, CanonicalBaseline, EdgeBehavior};

    use std::sync::Arc;

    use super::{AppendStream, AuthoredSpan, BlockedRanges, ChunkedText};

    #[test]
    fn append_stream_preserves_old_snapshots_and_source_order() {
        let first = Arc::<str>::from("alpha");
        let second = Arc::<str>::from("βeta");
        let third = Arc::<str>::from("🙂");
        let empty = AppendStream::default();
        let (one, first_work) = empty.append(first);
        let snapshot = one.clone();
        let (two, _) = one.append(second);
        let (three, _) = two.append(third);

        assert_eq!(first_work.source_bytes_copied, 0);
        assert_eq!(snapshot.to_text(), "alpha");
        assert_eq!(three.to_text(), "alphaβeta🙂");
        assert_eq!(three.len(), "alphaβeta🙂".len());
        assert_eq!(three.batches(), 3);
    }

    #[test]
    fn append_stream_preserves_order_across_binary_carries() {
        let mut stream = AppendStream::default();
        let mut expected = String::new();
        let mut snapshot = None;

        for value in 0..257_u16 {
            let payload = format!("{value:04x}|");
            expected.push_str(&payload);
            (stream, _) = stream.append(Arc::<str>::from(payload));
            if value == 126 {
                snapshot = Some((stream.clone(), expected.clone()));
            }
        }

        let (snapshot, snapshot_text) = snapshot.expect("checkpoint was captured");
        assert_eq!(snapshot.to_text(), snapshot_text);
        assert_eq!(snapshot.batches(), 127);
        assert_eq!(stream.to_text(), expected);
        assert_eq!(stream.batches(), 257);
    }

    #[test]
    fn append_stream_publication_work_is_logarithmic() {
        let batch = Arc::<str>::from("x".repeat(64 * 1024));
        let mut stream = AppendStream::default();
        let mut maximum_records = 0;
        for _ in 0..1024 {
            let (next, work) = stream.append(Arc::clone(&batch));
            maximum_records =
                maximum_records.max(work.level_records_visited + work.node_records_created);
            assert_eq!(work.source_bytes_copied, 0);
            assert_eq!(work.unpublished_batches, 0);
            stream = next;
        }
        assert_eq!(stream.batches(), 1024);
        assert_eq!(stream.len(), 1024 * 64 * 1024);
        assert!(
            maximum_records <= 22,
            "1,024 appends must touch/create at most two logarithmic paths"
        );
    }

    #[test]
    fn chunked_edit_reuses_unchanged_source_and_preserves_text() {
        let text = format!(
            "{}{}{}",
            "a".repeat(4096),
            "b".repeat(4096),
            "c".repeat(4096)
        );
        let candidate = ChunkedText::new(&text);
        let snapshot = candidate.clone();
        assert!(candidate.shares_index_with(&snapshot));
        let (edited, work) = candidate
            .replace(5000..5001, "B")
            .expect("ASCII edit is valid");
        let mut expected = text;
        expected.replace_range(5000..5001, "B");
        assert_eq!(edited.to_text(), expected);
        assert!(
            work.source_bytes_copied <= 4096,
            "one changed chunk should be copied"
        );
        assert!(
            work.source_chunks_reused >= 2,
            "prefix and suffix chunks should be reused"
        );
    }

    #[test]
    fn chunked_edit_rejects_a_non_utf8_boundary() {
        let candidate = ChunkedText::new("aéb");
        assert!(candidate.replace(2..2, "x").is_err());
    }

    #[test]
    fn blocked_ranges_shift_suffix_blocks_without_visiting_their_spans() {
        let spans = (0..4096)
            .map(|value| AuthoredSpan {
                range: value..value + 1,
                edges: EdgeBehavior {
                    start: Bias::Before,
                    end: Bias::After,
                },
                value: u32::try_from(value).expect("small test value"),
            })
            .collect();
        let candidate = BlockedRanges::new(spans);
        let snapshot = candidate.clone();
        assert!(candidate.shares_index_with(&snapshot));
        let (edited, work) = candidate.transform(2048..2049, 1);
        assert_eq!(edited.len(), 4096);
        assert!(
            work.spans_visited <= 2048,
            "only boundary blocks should visit spans"
        );
        let materialized = edited.materialize();
        assert_eq!(materialized[0].range, 0..1);
        assert_eq!(materialized[2048].range, 2048..2049);
        assert_eq!(materialized[4095].range, 4095..4096);
    }

    #[test]
    fn chunked_text_matches_string_across_deterministic_edit_sequences() {
        let mut expected = "aé🙂".repeat(3000);
        let mut candidate = ChunkedText::new(&expected);
        let replacements = ["", "x", "é", "🙂", "retained"];
        let mut random = 0x5eed_1234_9876_abcd_u64;

        for step in 0..500 {
            let boundaries = expected
                .char_indices()
                .map(|(offset, _)| offset)
                .chain(std::iter::once(expected.len()))
                .collect::<Vec<_>>();
            let left = boundaries[random_index(&mut random, boundaries.len())];
            let right = boundaries[random_index(&mut random, boundaries.len())];
            let range = left.min(right)..left.max(right);
            let replacement = replacements[random_index(&mut random, replacements.len())];

            let (next, _) = candidate
                .replace(range.clone(), replacement)
                .expect("generated UTF-8 boundaries are valid");
            expected.replace_range(range, replacement);
            candidate = next;
            assert_eq!(
                candidate.to_text(),
                expected,
                "candidate diverged at deterministic edit {step}"
            );
        }
    }

    #[test]
    fn blocked_ranges_match_flat_semantics_across_edit_sequences() {
        let source = "x".repeat(10_000);
        let spans = (0..5_000)
            .map(|value| {
                let start = (value * 37) % 9_900;
                AuthoredSpan {
                    range: start..start + value % 31 + 1,
                    edges: EdgeBehavior {
                        start: if value % 2 == 0 {
                            Bias::Before
                        } else {
                            Bias::After
                        },
                        end: if value % 3 == 0 {
                            Bias::After
                        } else {
                            Bias::Before
                        },
                    },
                    value: u32::try_from(value).expect("small test value"),
                }
            })
            .collect::<Vec<_>>();
        let mut flat = CanonicalBaseline::new(&source);
        flat.replace_authored(spans.clone());
        let mut candidate = BlockedRanges::new(spans);
        let mut random = 0x600d_f00d_dead_beef_u64;

        for step in 0..100 {
            let text_len = flat.text_len();
            let start = random_index(&mut random, text_len + 1);
            let removed = random_index(&mut random, 9);
            let end = (start + removed).min(text_len);
            let inserted = "y".repeat(random_index(&mut random, 5));
            flat.replace(start..end, &inserted)
                .expect("generated ASCII edit is valid");
            (candidate, _) = candidate.transform(start..end, inserted.len());
            assert_eq!(
                candidate.materialize(),
                flat.authored(),
                "range candidate diverged at deterministic edit {step}"
            );
        }
    }

    fn next_random(state: &mut u64) -> u64 {
        *state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        *state
    }

    fn random_index(state: &mut u64, len: usize) -> usize {
        let modulus = u64::try_from(len).expect("test collection length fits u64");
        usize::try_from(next_random(state) % modulus).expect("reduced random index fits usize")
    }
}
