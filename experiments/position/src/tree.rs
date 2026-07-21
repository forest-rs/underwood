// Copyright 2026 the Underwood Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Private semantic model for the ADR-0001 tree-anchor lifecycle.
//!
//! The identifiers and representation in this module exist only to execute
//! trace laws. They are not candidate production storage or public APIs.

use crate::model::Bias;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct NodeId(u32);

impl NodeId {
    pub(crate) const fn new(value: u32) -> Self {
        Self(value)
    }

    pub(crate) const fn get(self) -> u32 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct TreeAnchorToken(usize);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ResolvedTreeAnchor {
    pub(crate) node: NodeId,
    pub(crate) offset: usize,
    pub(crate) bias: Bias,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct TreeAnchorState {
    node: NodeId,
    offset: usize,
    bias: Bias,
    resolved: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct TextNode {
    id: NodeId,
    text: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum TreeError {
    DuplicateNode(NodeId),
    UnknownNode(NodeId),
    UnknownAnchor(TreeAnchorToken),
    UnresolvedAnchor(TreeAnchorToken),
    InvalidOffset {
        node: NodeId,
        offset: usize,
        text_len: usize,
    },
    NonBoundary {
        node: NodeId,
        offset: usize,
    },
    NodesNotAdjacent {
        left: NodeId,
        right: NodeId,
    },
}

#[derive(Clone, Debug, Default)]
pub(crate) struct TreeBaseline {
    nodes: Vec<TextNode>,
    anchors: Vec<TreeAnchorState>,
}

impl TreeBaseline {
    pub(crate) fn new(nodes: impl IntoIterator<Item = (NodeId, &'static str)>) -> Self {
        let mut document = Self::default();
        for (id, text) in nodes {
            assert!(
                document.node_index(id).is_none(),
                "trace fixtures must use unique node identities"
            );
            document.nodes.push(TextNode {
                id,
                text: text.to_owned(),
            });
        }
        document
    }

    pub(crate) fn node_order(&self) -> Vec<NodeId> {
        self.nodes.iter().map(|node| node.id).collect()
    }

    pub(crate) fn text(&self, node: NodeId) -> Result<&str, TreeError> {
        let index = self.node_index(node).ok_or(TreeError::UnknownNode(node))?;
        Ok(&self.nodes[index].text)
    }

    pub(crate) fn create_anchor(
        &mut self,
        node: NodeId,
        offset: usize,
        bias: Bias,
    ) -> Result<TreeAnchorToken, TreeError> {
        self.validate_boundary(node, offset)?;
        let token = TreeAnchorToken(self.anchors.len());
        self.anchors.push(TreeAnchorState {
            node,
            offset,
            bias,
            resolved: true,
        });
        Ok(token)
    }

    pub(crate) fn resolve_anchor(
        &self,
        token: TreeAnchorToken,
    ) -> Result<ResolvedTreeAnchor, TreeError> {
        let anchor = self
            .anchors
            .get(token.0)
            .ok_or(TreeError::UnknownAnchor(token))?;
        if !anchor.resolved {
            return Err(TreeError::UnresolvedAnchor(token));
        }
        Ok(ResolvedTreeAnchor {
            node: anchor.node,
            offset: anchor.offset,
            bias: anchor.bias,
        })
    }

    pub(crate) fn split(
        &mut self,
        node: NodeId,
        offset: usize,
        right: NodeId,
    ) -> Result<(), TreeError> {
        if self.node_index(right).is_some() {
            return Err(TreeError::DuplicateNode(right));
        }
        self.validate_boundary(node, offset)?;
        let left_index = self
            .node_index(node)
            .expect("validated node must remain present");
        let right_text = self.nodes[left_index].text.split_off(offset);
        self.nodes.insert(
            left_index + 1,
            TextNode {
                id: right,
                text: right_text,
            },
        );

        for anchor in &mut self.anchors {
            if anchor.resolved
                && anchor.node == node
                && (anchor.offset > offset
                    || (anchor.offset == offset && anchor.bias == Bias::After))
            {
                anchor.node = right;
                anchor.offset -= offset;
            }
        }
        Ok(())
    }

    pub(crate) fn join(&mut self, left: NodeId, right: NodeId) -> Result<(), TreeError> {
        let left_index = self.node_index(left).ok_or(TreeError::UnknownNode(left))?;
        let right_index = self
            .node_index(right)
            .ok_or(TreeError::UnknownNode(right))?;
        if right_index != left_index + 1 {
            return Err(TreeError::NodesNotAdjacent { left, right });
        }

        let left_len = self.nodes[left_index].text.len();
        let right_node = self.nodes.remove(right_index);
        self.nodes[left_index].text.push_str(&right_node.text);
        for anchor in &mut self.anchors {
            if anchor.resolved && anchor.node == right {
                anchor.node = left;
                anchor.offset = left_len
                    .checked_add(anchor.offset)
                    .expect("trace tree-anchor offset overflow");
            }
        }
        Ok(())
    }

    pub(crate) fn move_before(&mut self, node: NodeId, before: NodeId) -> Result<(), TreeError> {
        let source = self.node_index(node).ok_or(TreeError::UnknownNode(node))?;
        self.node_index(before)
            .ok_or(TreeError::UnknownNode(before))?;
        if node == before {
            return Ok(());
        }

        let moved = self.nodes.remove(source);
        let target = self
            .node_index(before)
            .expect("move target must remain after removing a distinct node");
        self.nodes.insert(target, moved);
        Ok(())
    }

    pub(crate) fn delete(&mut self, node: NodeId) -> Result<(), TreeError> {
        let index = self.node_index(node).ok_or(TreeError::UnknownNode(node))?;
        self.nodes.remove(index);
        for anchor in &mut self.anchors {
            if anchor.resolved && anchor.node == node {
                anchor.resolved = false;
            }
        }
        Ok(())
    }

    fn node_index(&self, id: NodeId) -> Option<usize> {
        self.nodes.iter().position(|node| node.id == id)
    }

    fn validate_boundary(&self, node: NodeId, offset: usize) -> Result<(), TreeError> {
        let index = self.node_index(node).ok_or(TreeError::UnknownNode(node))?;
        let text = &self.nodes[index].text;
        if offset > text.len() {
            return Err(TreeError::InvalidOffset {
                node,
                offset,
                text_len: text.len(),
            });
        }
        if text.is_char_boundary(offset) {
            Ok(())
        } else {
            Err(TreeError::NonBoundary { node, offset })
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::model::Bias;

    use super::{NodeId, TreeBaseline, TreeError};

    #[test]
    fn split_rejects_a_duplicate_right_identity() {
        let left = NodeId::new(1);
        let right = NodeId::new(2);
        let mut tree = TreeBaseline::new([(left, "ab"), (right, "cd")]);

        assert_eq!(
            tree.split(left, 1, right),
            Err(TreeError::DuplicateNode(right))
        );
    }

    #[test]
    fn anchor_rejects_a_non_utf8_boundary() {
        let node = NodeId::new(1);
        let mut tree = TreeBaseline::new([(node, "aéb")]);

        assert_eq!(
            tree.create_anchor(node, 2, Bias::Before),
            Err(TreeError::NonBoundary { node, offset: 2 })
        );
    }

    #[test]
    fn join_rejects_nonadjacent_nodes() {
        let first = NodeId::new(1);
        let middle = NodeId::new(2);
        let last = NodeId::new(3);
        let mut tree = TreeBaseline::new([(first, "a"), (middle, "b"), (last, "c")]);

        assert_eq!(
            tree.join(first, last),
            Err(TreeError::NodesNotAdjacent {
                left: first,
                right: last
            })
        );
    }
}
