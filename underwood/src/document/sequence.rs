// Copyright 2026 the Underwood Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Persistent paragraph order with bounded path copying.
//!
//! This is deliberately a small typed sequence rather than a general arena.
//! Published revisions share immutable nodes; a transaction copies at most one
//! 32-way root path before mutating a paragraph.

use super::model::Paragraph;
use alloc::sync::Arc;
use alloc::vec;
use alloc::vec::Vec;
use core::ops::Index;

const BRANCHING: usize = 32;
// 32^13 exceeds every value representable by a 64-bit `usize`.
const MAX_DEPTH: usize = 14;

#[derive(Clone, Debug, Default)]
pub(crate) struct ParagraphSequence {
    root: Option<Arc<ParagraphNode>>,
    len: usize,
    height: u8,
}

#[derive(Clone, Debug)]
enum ParagraphNode {
    Leaf(Vec<Arc<Paragraph>>),
    Branch {
        len: usize,
        children: Vec<Arc<Self>>,
    },
}

impl ParagraphSequence {
    pub(crate) const fn len(&self) -> usize {
        self.len
    }

    pub(crate) fn get(&self, index: usize) -> Option<&Paragraph> {
        (index < self.len)
            .then(|| get_node(self.root.as_deref()?, self.height, index))
            .flatten()
    }

    pub(super) fn get_mut(&mut self, index: usize) -> Option<&mut Paragraph> {
        if index >= self.len {
            return None;
        }
        get_node_mut(self.root.as_mut()?, self.height, index)
    }

    pub(super) fn push(&mut self, paragraph: Paragraph) {
        let paragraph = Arc::new(paragraph);
        let Some(root) = &mut self.root else {
            self.root = Some(Arc::new(ParagraphNode::Leaf(vec![paragraph])));
            self.len = 1;
            return;
        };

        if self.len == node_capacity(self.height) {
            let old_root = Arc::clone(root);
            let new_path = singleton_path(self.height, paragraph);
            self.root = Some(Arc::new(ParagraphNode::Branch {
                len: self.len.saturating_add(1),
                children: vec![old_root, new_path],
            }));
            self.height = self.height.saturating_add(1);
        } else {
            push_node(Arc::make_mut(root), self.height, paragraph);
        }
        self.len = self.len.saturating_add(1);
    }

    pub(crate) fn iter(&self) -> ParagraphIter<'_> {
        ParagraphIter::new(self)
    }

    pub(crate) fn changed_indices<'a>(
        &'a self,
        previous: &'a Self,
    ) -> Option<ChangedParagraphs<'a>> {
        (self.len == previous.len).then(|| ChangedParagraphs::new(previous, self))
    }
}

impl Index<usize> for ParagraphSequence {
    type Output = Paragraph;

    fn index(&self, index: usize) -> &Self::Output {
        self.get(index)
            .expect("paragraph sequence index must be in bounds")
    }
}

impl<'a> IntoIterator for &'a ParagraphSequence {
    type Item = &'a Paragraph;
    type IntoIter = ParagraphIter<'a>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

#[derive(Clone, Copy)]
struct IterFrame<'a> {
    node: &'a ParagraphNode,
    next: usize,
}

#[derive(Clone)]
pub(crate) struct ParagraphIter<'a> {
    stack: [Option<IterFrame<'a>>; MAX_DEPTH],
    depth: usize,
    remaining: usize,
}

impl<'a> ParagraphIter<'a> {
    fn new(sequence: &'a ParagraphSequence) -> Self {
        let mut this = Self {
            stack: [None; MAX_DEPTH],
            depth: 0,
            remaining: sequence.len,
        };
        if let Some(root) = sequence.root.as_deref() {
            this.push(root);
        }
        this
    }

    fn push(&mut self, node: &'a ParagraphNode) {
        debug_assert!(
            self.depth < MAX_DEPTH,
            "paragraph sequence depth must fit its fixed iterator stack"
        );
        self.stack[self.depth] = Some(IterFrame { node, next: 0 });
        self.depth += 1;
    }
}

impl<'a> Iterator for ParagraphIter<'a> {
    type Item = &'a Paragraph;

    fn next(&mut self) -> Option<Self::Item> {
        while self.depth != 0 {
            let frame = self.stack[self.depth - 1]
                .as_mut()
                .expect("every live iterator depth has one frame");
            match frame.node {
                ParagraphNode::Leaf(paragraphs) => {
                    if let Some(paragraph) = paragraphs.get(frame.next) {
                        frame.next += 1;
                        self.remaining -= 1;
                        return Some(paragraph);
                    }
                    self.depth -= 1;
                    self.stack[self.depth] = None;
                }
                ParagraphNode::Branch { children, .. } => {
                    if let Some(child) = children.get(frame.next) {
                        frame.next += 1;
                        let child = child.as_ref();
                        self.push(child);
                    } else {
                        self.depth -= 1;
                        self.stack[self.depth] = None;
                    }
                }
            }
        }
        None
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.remaining, Some(self.remaining))
    }
}

impl ExactSizeIterator for ParagraphIter<'_> {}

#[derive(Clone, Copy)]
struct DiffFrame<'a> {
    previous: &'a ParagraphNode,
    current: &'a ParagraphNode,
    base: usize,
    next: usize,
    child_base: usize,
}

#[derive(Clone)]
pub(crate) struct ChangedParagraphs<'a> {
    stack: [Option<DiffFrame<'a>>; MAX_DEPTH],
    depth: usize,
    fallback: core::ops::Range<usize>,
}

impl<'a> ChangedParagraphs<'a> {
    fn new(previous: &'a ParagraphSequence, current: &'a ParagraphSequence) -> Self {
        let mut this = Self {
            stack: [None; MAX_DEPTH],
            depth: 0,
            fallback: 0..0,
        };
        match (previous.root.as_deref(), current.root.as_deref()) {
            (Some(previous), Some(current)) => this.push(previous, current, 0),
            (None, None) => {}
            (None, Some(_)) | (Some(_), None) => {
                this.fallback = 0..current.len;
            }
        }
        this
    }

    fn push(&mut self, previous: &'a ParagraphNode, current: &'a ParagraphNode, base: usize) {
        debug_assert!(
            self.depth < MAX_DEPTH,
            "paragraph diff must fit the persistent sequence depth"
        );
        self.stack[self.depth] = Some(DiffFrame {
            previous,
            current,
            base,
            next: 0,
            child_base: base,
        });
        self.depth += 1;
    }

    fn pop(&mut self) {
        self.depth -= 1;
        self.stack[self.depth] = None;
    }
}

impl Iterator for ChangedParagraphs<'_> {
    type Item = usize;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(index) = self.fallback.next() {
            return Some(index);
        }
        while self.depth != 0 {
            let frame = self.stack[self.depth - 1]
                .as_mut()
                .expect("every live paragraph diff depth has one frame");
            if core::ptr::eq(frame.previous, frame.current) {
                self.pop();
                continue;
            }
            match (frame.previous, frame.current) {
                (ParagraphNode::Leaf(previous), ParagraphNode::Leaf(current))
                    if previous.len() == current.len() =>
                {
                    while frame.next < current.len() {
                        let index = frame.next;
                        frame.next += 1;
                        if !Arc::ptr_eq(&previous[index], &current[index]) {
                            return Some(frame.base + index);
                        }
                    }
                    self.pop();
                }
                (
                    ParagraphNode::Branch {
                        children: previous, ..
                    },
                    ParagraphNode::Branch {
                        children: current, ..
                    },
                ) if previous.len() == current.len() => {
                    if frame.next == current.len() {
                        self.pop();
                        continue;
                    }
                    let child_index = frame.next;
                    let child_base = frame.child_base;
                    let previous_child = previous[child_index].as_ref();
                    let current_child = current[child_index].as_ref();
                    if node_len(previous_child) != node_len(current_child) {
                        let end = frame.base.saturating_add(node_len(frame.current));
                        self.pop();
                        self.fallback = child_base..end;
                        return self.fallback.next();
                    }
                    frame.next += 1;
                    frame.child_base = frame.child_base.saturating_add(node_len(current_child));
                    self.push(previous_child, current_child, child_base);
                }
                _ => {
                    let start = frame.base;
                    let end = start.saturating_add(node_len(frame.current));
                    self.pop();
                    self.fallback = start..end;
                    return self.fallback.next();
                }
            }
        }
        None
    }
}

fn get_node(node: &ParagraphNode, height: u8, index: usize) -> Option<&Paragraph> {
    match node {
        ParagraphNode::Leaf(paragraphs) if height == 0 => paragraphs.get(index).map(Arc::as_ref),
        ParagraphNode::Branch { children, .. } if height != 0 => {
            let child_capacity = node_capacity(height - 1);
            let child = index / child_capacity;
            get_node(
                children.get(child)?.as_ref(),
                height - 1,
                index % child_capacity,
            )
        }
        ParagraphNode::Leaf(_) | ParagraphNode::Branch { .. } => None,
    }
}

fn get_node_mut(node: &mut Arc<ParagraphNode>, height: u8, index: usize) -> Option<&mut Paragraph> {
    match Arc::make_mut(node) {
        ParagraphNode::Leaf(paragraphs) if height == 0 => {
            paragraphs.get_mut(index).map(Arc::make_mut)
        }
        ParagraphNode::Branch { children, .. } if height != 0 => {
            let child_capacity = node_capacity(height - 1);
            let child = index / child_capacity;
            get_node_mut(children.get_mut(child)?, height - 1, index % child_capacity)
        }
        ParagraphNode::Leaf(_) | ParagraphNode::Branch { .. } => None,
    }
}

fn push_node(node: &mut ParagraphNode, height: u8, paragraph: Arc<Paragraph>) {
    match node {
        ParagraphNode::Leaf(paragraphs) if height == 0 => {
            debug_assert!(
                paragraphs.len() < BRANCHING,
                "a leaf must grow a parent before exceeding its branching factor"
            );
            paragraphs.push(paragraph);
        }
        ParagraphNode::Branch { len, children } if height != 0 => {
            let child_height = height - 1;
            let child_capacity = node_capacity(child_height);
            let last = children
                .last_mut()
                .expect("a paragraph branch always has a child");
            if node_len(last) == child_capacity {
                debug_assert!(
                    children.len() < BRANCHING,
                    "a branch must grow a parent before exceeding its branching factor"
                );
                children.push(singleton_path(child_height, paragraph));
            } else {
                push_node(Arc::make_mut(last), child_height, paragraph);
            }
            *len = len.saturating_add(1);
        }
        ParagraphNode::Leaf(_) | ParagraphNode::Branch { .. } => {
            unreachable!("paragraph node kind must agree with its height")
        }
    }
}

fn singleton_path(height: u8, paragraph: Arc<Paragraph>) -> Arc<ParagraphNode> {
    if height == 0 {
        Arc::new(ParagraphNode::Leaf(vec![paragraph]))
    } else {
        Arc::new(ParagraphNode::Branch {
            len: 1,
            children: vec![singleton_path(height - 1, paragraph)],
        })
    }
}

fn node_len(node: &ParagraphNode) -> usize {
    match node {
        ParagraphNode::Leaf(paragraphs) => paragraphs.len(),
        ParagraphNode::Branch { len, .. } => *len,
    }
}

fn node_capacity(height: u8) -> usize {
    BRANCHING
        .checked_pow(u32::from(height) + 1)
        .unwrap_or(usize::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DocumentId, ParagraphId, ParagraphRole};

    fn paragraph(index: u32) -> Paragraph {
        Paragraph {
            id: ParagraphId {
                document: DocumentId::from_bytes(*b"sequence-test-01"),
                index,
            },
            role: ParagraphRole::BODY,
            version: 1,
            leaves: Vec::new(),
        }
    }

    #[test]
    fn persistent_sequence_indexes_and_iterates_across_branch_boundaries() {
        let mut sequence = ParagraphSequence::default();
        for index in 0..2_048_u32 {
            sequence.push(paragraph(index));
        }
        assert_eq!(sequence.len(), 2_048);
        assert_eq!(sequence[0].id.index, 0);
        assert_eq!(sequence[31].id.index, 31);
        assert_eq!(sequence[32].id.index, 32);
        assert_eq!(sequence[1_023].id.index, 1_023);
        assert_eq!(sequence[1_024].id.index, 1_024);
        assert_eq!(sequence[2_047].id.index, 2_047);
        assert_eq!(
            sequence
                .iter()
                .map(|paragraph| paragraph.id.index)
                .sum::<u32>(),
            (0..2_048_u32).sum()
        );
    }

    #[test]
    fn mutation_copies_only_the_target_path_and_paragraph() {
        let mut original = ParagraphSequence::default();
        for index in 0..1_000_u32 {
            original.push(paragraph(index));
        }
        let mut edited = original.clone();
        edited.get_mut(511).expect("target exists").version = 2;

        assert_eq!(original[511].version, 1);
        assert_eq!(edited[511].version, 2);
        assert!(
            core::ptr::eq(&original[510], &edited[510]),
            "an untouched neighboring paragraph must remain pointer-shared"
        );
        assert!(
            core::ptr::eq(&original[999], &edited[999]),
            "an untouched distant paragraph must remain pointer-shared"
        );
        assert_eq!(
            edited
                .changed_indices(&original)
                .expect("lengths match")
                .collect::<Vec<_>>(),
            [511],
            "the structural diff must skip every shared subtree"
        );
    }

    #[test]
    fn structural_diff_reports_multiple_paths_in_document_order() {
        let mut original = ParagraphSequence::default();
        for index in 0..2_048_u32 {
            original.push(paragraph(index));
        }
        let mut edited = original.clone();
        for index in [0, 31, 32, 1_023, 1_024, 2_047] {
            edited.get_mut(index).expect("target exists").version = 2;
        }
        assert_eq!(
            edited
                .changed_indices(&original)
                .expect("lengths match")
                .collect::<Vec<_>>(),
            [0, 31, 32, 1_023, 1_024, 2_047]
        );
    }
}
