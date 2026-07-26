// Copyright 2026 the Underwood Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Immutable document state and atomic edit transactions.

use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::ops::Range;

use crate::{
    EditError, EditErrorKind, SnapshotTextPosition, SnapshotTextRange, SnapshotTextSelection,
    SnapshotTextSelectionSet, TextAffinity, TextSelectionMode,
};

mod model;
mod sequence;
mod transaction;

pub use model::{
    ChangeSet, Document, DocumentId, DocumentRevision, DocumentSnapshot, InlineRole, ParagraphId,
    ParagraphRole, Publication, SelectionReplacement, SemanticId, TextId,
};
pub use transaction::Edit;

pub(crate) use model::{DocumentState, Paragraph, TextLeaf};
use transaction::{ReplacementOperation, validate_replacement_plans};

#[cfg(test)]
mod tests;
