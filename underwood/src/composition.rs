// Copyright 2026 the Underwood Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::ops::Range;

use crate::{
    CompositionError, CompositionErrorKind, Document, DocumentId, DocumentRevision, EditError,
    SelectionReplacement, SnapshotTextRange, SnapshotTextSelectionSet, TextAffinity, TextId,
};

/// Caller-provided identity of one native composition session.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct CompositionId([u8; 16]);

impl CompositionId {
    /// Creates a composition identity from caller-owned opaque bytes.
    #[must_use]
    pub const fn from_bytes(value: [u8; 16]) -> Self {
        Self(value)
    }
}

/// Monotonic transient revision within one composition session.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CompositionEpoch(u64);

impl CompositionEpoch {
    pub(crate) const INITIAL: Self = Self(0);

    /// Returns the monotonic epoch number for diagnostics and host invalidation.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }

    fn next(self) -> Option<Self> {
        self.0.checked_add(1).map(Self)
    }
}

/// Presentation category attached to one IME-authored clause.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum CompositionClauseKind {
    /// Raw input which has not yet been converted by the input method.
    Raw,
    /// Converted text which is not the currently selected candidate clause.
    Converted,
    /// Clause currently selected by the input method.
    Selected,
}

/// One validated byte range within generated preedit text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompositionClause {
    bytes: Range<u32>,
    kind: CompositionClauseKind,
}

impl CompositionClause {
    /// Creates a clause; bounds are validated when installing an update.
    #[must_use]
    pub const fn new(bytes: Range<u32>, kind: CompositionClauseKind) -> Self {
        Self { bytes, kind }
    }

    /// Returns the UTF-8 byte range within the preedit string.
    #[must_use]
    pub fn bytes(&self) -> Range<u32> {
        self.bytes.clone()
    }

    /// Returns the presentation category reported by the input method.
    #[must_use]
    pub const fn kind(&self) -> CompositionClauseKind {
        self.kind
    }
}

/// Replacement snapshot supplied for one composition epoch.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompositionUpdate {
    text: String,
    selection: Option<Range<u32>>,
    clauses: Vec<CompositionClause>,
}

impl CompositionUpdate {
    /// Creates a preedit update with no selected range or clauses.
    #[must_use]
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            selection: None,
            clauses: Vec::new(),
        }
    }

    /// Attaches the selected byte range within the generated preedit.
    #[must_use]
    pub fn with_selection(mut self, selection: Range<u32>) -> Self {
        self.selection = Some(selection);
        self
    }

    /// Attaches IME-authored clause ranges within the generated preedit.
    #[must_use]
    pub fn with_clauses(mut self, clauses: impl IntoIterator<Item = CompositionClause>) -> Self {
        self.clauses = clauses.into_iter().collect();
        self
    }
}

/// Exact position in generated text for one composition epoch.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct CompositionTextPosition {
    id: CompositionId,
    epoch: CompositionEpoch,
    byte: u32,
    affinity: TextAffinity,
}

impl CompositionTextPosition {
    pub(crate) const fn new(
        id: CompositionId,
        epoch: CompositionEpoch,
        byte: u32,
        affinity: TextAffinity,
    ) -> Self {
        Self {
            id,
            epoch,
            byte,
            affinity,
        }
    }

    /// Returns the composition session identity.
    #[must_use]
    pub const fn id(self) -> CompositionId {
        self.id
    }

    /// Returns the exact composition epoch.
    #[must_use]
    pub const fn epoch(self) -> CompositionEpoch {
        self.epoch
    }

    /// Returns the UTF-8 byte boundary within the generated preedit.
    #[must_use]
    pub const fn byte(self) -> u32 {
        self.byte
    }

    /// Returns which logical side owns the position.
    #[must_use]
    pub const fn affinity(self) -> TextAffinity {
        self.affinity
    }
}

/// Exact source range in generated text for one composition epoch.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompositionTextRange {
    id: CompositionId,
    epoch: CompositionEpoch,
    bytes: Range<u32>,
}

impl CompositionTextRange {
    pub(crate) const fn new(id: CompositionId, epoch: CompositionEpoch, bytes: Range<u32>) -> Self {
        Self { id, epoch, bytes }
    }

    /// Returns the composition session identity.
    #[must_use]
    pub const fn id(&self) -> CompositionId {
        self.id
    }

    /// Returns the exact composition epoch.
    #[must_use]
    pub const fn epoch(&self) -> CompositionEpoch {
        self.epoch
    }

    /// Returns the UTF-8 byte range within the generated preedit.
    #[must_use]
    pub fn bytes(&self) -> Range<u32> {
        self.bytes.clone()
    }
}

/// Active generated-text projection over one committed snapshot selection.
#[derive(Clone, Debug)]
pub struct CompositionSession {
    id: CompositionId,
    epoch: CompositionEpoch,
    document: DocumentId,
    base_revision: DocumentRevision,
    target: SnapshotTextSelectionSet,
    text: Arc<str>,
    selection: Option<Range<u32>>,
    clauses: Arc<[CompositionClause]>,
}

impl CompositionSession {
    pub(crate) fn new(id: CompositionId, target: SnapshotTextSelectionSet) -> Self {
        Self {
            id,
            epoch: CompositionEpoch::INITIAL,
            document: target.document(),
            base_revision: target.revision(),
            target,
            text: Arc::from(""),
            selection: None,
            clauses: Arc::from([]),
        }
    }

    /// Returns the caller-provided session identity.
    #[must_use]
    pub const fn id(&self) -> CompositionId {
        self.id
    }

    /// Returns the current transient epoch.
    #[must_use]
    pub const fn epoch(&self) -> CompositionEpoch {
        self.epoch
    }

    /// Returns the immutable document revision under the projection.
    #[must_use]
    pub const fn base_revision(&self) -> DocumentRevision {
        self.base_revision
    }

    /// Returns the normalized single insertion point replaced by this session.
    #[must_use]
    pub const fn target(&self) -> &SnapshotTextSelectionSet {
        &self.target
    }

    /// Returns the generated preedit string.
    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Returns the selected UTF-8 byte range within the generated preedit.
    #[must_use]
    pub fn selection(&self) -> Option<Range<u32>> {
        self.selection.clone()
    }

    /// Returns IME-authored clauses in preedit order.
    #[must_use]
    pub fn clauses(&self) -> &[CompositionClause] {
        &self.clauses
    }

    /// Installs the next complete preedit snapshot.
    ///
    /// `expected` prevents a delayed native callback from overwriting newer
    /// transient state. A successful update advances the epoch exactly once.
    pub fn update(
        &mut self,
        expected: CompositionEpoch,
        update: CompositionUpdate,
    ) -> Result<CompositionEpoch, CompositionError> {
        if expected != self.epoch {
            return Err(CompositionError::new(CompositionErrorKind::StaleEpoch));
        }
        validate_optional_range(&update.text, update.selection.as_ref())?;
        let mut previous_end = 0_u32;
        for clause in &update.clauses {
            validate_range(&update.text, &clause.bytes)
                .map_err(|_| CompositionError::new(CompositionErrorKind::InvalidClauseRange))?;
            if clause.bytes.start < previous_end {
                return Err(CompositionError::new(
                    CompositionErrorKind::InvalidClauseRange,
                ));
            }
            previous_end = clause.bytes.end;
        }
        self.epoch = self
            .epoch
            .next()
            .ok_or_else(|| CompositionError::new(CompositionErrorKind::EpochExhausted))?;
        self.text = Arc::from(update.text);
        self.selection = update.selection;
        self.clauses = update.clauses.into();
        Ok(self.epoch)
    }

    /// Commits one replacement transaction and ends this session.
    ///
    /// The committed string may differ from the latest preedit, as native input
    /// methods commonly send a final committed payload after ending marked text.
    pub fn commit(
        self,
        document: &mut Document,
        committed: &str,
    ) -> Result<SelectionReplacement, EditError> {
        document.replace_selections(&self.target, committed)
    }

    /// Cancels transient text without publishing and returns the normalized selection.
    #[must_use]
    pub fn cancel(self) -> SnapshotTextSelectionSet {
        self.target
    }

    pub(crate) const fn document(&self) -> DocumentId {
        self.document
    }

    pub(crate) fn replacement_ranges(&self) -> &[SnapshotTextRange] {
        self.target
            .primary()
            .map_or(&[], |selection| selection.ranges())
    }

    pub(crate) fn target_text(&self) -> Option<TextId> {
        self.replacement_ranges()
            .first()
            .map(SnapshotTextRange::text)
    }
}

/// Result of normalizing a selection set into one native composition target.
#[derive(Clone, Debug)]
pub struct CompositionStart {
    session: CompositionSession,
    selections: SnapshotTextSelectionSet,
    selection_changed: bool,
}

impl CompositionStart {
    pub(crate) const fn new(
        session: CompositionSession,
        selections: SnapshotTextSelectionSet,
        selection_changed: bool,
    ) -> Self {
        Self {
            session,
            selections,
            selection_changed,
        }
    }

    /// Returns the new composition session.
    #[must_use]
    pub const fn session(&self) -> &CompositionSession {
        &self.session
    }

    /// Takes ownership of the new composition session.
    #[must_use]
    pub fn into_session(self) -> CompositionSession {
        self.session
    }

    /// Returns the normalized selection set visible to the native host.
    #[must_use]
    pub const fn selections(&self) -> &SnapshotTextSelectionSet {
        &self.selections
    }

    /// Returns whether starting composition changed the visible selection set.
    #[must_use]
    pub const fn selection_changed(&self) -> bool {
        self.selection_changed
    }
}

fn validate_optional_range(text: &str, range: Option<&Range<u32>>) -> Result<(), CompositionError> {
    if let Some(range) = range {
        validate_range(text, range)?;
    }
    Ok(())
}

fn validate_range(text: &str, range: &Range<u32>) -> Result<(), CompositionError> {
    if range.start > range.end || text.get(range.start as usize..range.end as usize).is_none() {
        Err(CompositionError::new(
            CompositionErrorKind::InvalidPreeditRange,
        ))
    } else {
        Ok(())
    }
}
