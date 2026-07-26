// Copyright 2026 the Underwood Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Layout orchestration and coordinated retained-cache policy.
//!
//! This module owns cache identity, lifetime, and preparation sequencing; it
//! explicitly does not own semantic projection or geometry construction.

use super::*;
use core::mem::size_of;

/// Independent retained-geometry limits for committed and composition lanes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CacheBudget {
    max_entries: usize,
    max_composition_entries: usize,
    shared_preparation_bytes: usize,
}

impl CacheBudget {
    /// Creates a budget with the given maximum in each geometry lane.
    ///
    /// Committed and transient composition geometry are enforced independently
    /// so composition cannot evict committed work. Use
    /// [`Self::with_composition_entries`] to give the transient lane a
    /// different limit. A zero lane budget still materializes caller-owned
    /// outputs without retaining entries in that lane.
    #[must_use]
    pub const fn new(max_entries: usize) -> Self {
        Self {
            max_entries,
            max_composition_entries: max_entries,
            shared_preparation_bytes: 0,
        }
    }

    /// Returns a budget with a distinct transient-composition entry limit.
    ///
    /// Zero disables engine-owned composition retention without affecting
    /// committed geometry.
    #[must_use]
    pub const fn with_composition_entries(mut self, max_entries: usize) -> Self {
        self.max_composition_entries = max_entries;
        self
    }

    /// Returns a budget that may retain this many bytes of exact shared
    /// paragraph preparation.
    ///
    /// The default is zero, so cross-identity retention is always explicit.
    #[must_use]
    pub const fn with_shared_preparation_bytes(mut self, bytes: usize) -> Self {
        self.shared_preparation_bytes = bytes;
        self
    }

    /// Returns the maximum number of retained committed geometry entries.
    #[must_use]
    pub const fn max_entries(self) -> usize {
        self.max_entries
    }

    /// Returns the maximum number of retained transient-composition entries.
    #[must_use]
    pub const fn max_composition_entries(self) -> usize {
        self.max_composition_entries
    }

    /// Returns the maximum accounting charge for shared preparation entries.
    #[must_use]
    pub const fn shared_preparation_bytes(self) -> usize {
        self.shared_preparation_bytes
    }
}

/// Snapshot of coordinated retained-cache state and cumulative activity.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CacheDiagnostics {
    budget: usize,
    composition_budget: usize,
    committed_entries: usize,
    composition_entries: usize,
    backend_entries: Option<usize>,
    scene_cache_accounted_bytes: usize,
    peak_entries: usize,
    hits: usize,
    misses: usize,
    evictions: usize,
    releases: usize,
    shared_preparation_budget: usize,
    shared_preparation_entries: usize,
    shared_preparation_resident_bytes: usize,
    shared_preparation_peak_bytes: usize,
    shared_preparation_hits: usize,
    shared_preparation_misses: usize,
    shared_preparation_evictions: usize,
    shared_preparation_oversized_non_retentions: usize,
}

impl CacheDiagnostics {
    /// Returns the configured maximum retained committed geometry entries.
    #[must_use]
    pub const fn budget(self) -> usize {
        self.budget
    }

    /// Returns the configured maximum retained composition geometry entries.
    #[must_use]
    pub const fn composition_budget(self) -> usize {
        self.composition_budget
    }

    /// Returns resident committed geometry entries.
    #[must_use]
    pub const fn committed_entries(self) -> usize {
        self.committed_entries
    }

    /// Returns resident transient-composition geometry entries.
    #[must_use]
    pub const fn composition_entries(self) -> usize {
        self.composition_entries
    }

    /// Returns all resident geometry entries across the independently
    /// budgeted committed and composition lanes.
    #[must_use]
    pub const fn current_entries(self) -> usize {
        self.committed_entries + self.composition_entries
    }

    /// Returns retained backend preparation entries, when the backend reports them.
    #[must_use]
    pub const fn backend_entries(self) -> Option<usize> {
        self.backend_entries
    }

    /// Returns the deterministic capacity charge for retained scene-cache data.
    ///
    /// Shared font blobs, backend-private storage, and allocator overhead are
    /// deliberately excluded.
    #[must_use]
    pub const fn scene_cache_accounted_bytes(self) -> usize {
        self.scene_cache_accounted_bytes
    }

    /// Returns the highest observed resident geometry entry count.
    #[must_use]
    pub const fn peak_entries(self) -> usize {
        self.peak_entries
    }

    /// Returns paragraph-identity lookups that found a resident geometry entry.
    #[must_use]
    pub const fn hits(self) -> usize {
        self.hits
    }

    /// Returns paragraph-identity lookups that created a geometry entry.
    #[must_use]
    pub const fn misses(self) -> usize {
        self.misses
    }

    /// Returns entries removed to enforce the configured budget.
    #[must_use]
    pub const fn evictions(self) -> usize {
        self.evictions
    }

    /// Returns entries removed by explicit document or whole-cache release.
    #[must_use]
    pub const fn releases(self) -> usize {
        self.releases
    }

    /// Returns the configured shared-preparation byte budget.
    #[must_use]
    pub const fn shared_preparation_budget(self) -> usize {
        self.shared_preparation_budget
    }

    /// Returns resident identity-free preparation entries.
    #[must_use]
    pub const fn shared_preparation_entries(self) -> usize {
        self.shared_preparation_entries
    }

    /// Returns the current deterministic shared-preparation accounting charge.
    #[must_use]
    pub const fn shared_preparation_resident_bytes(self) -> usize {
        self.shared_preparation_resident_bytes
    }

    /// Returns the highest observed shared-preparation accounting charge.
    #[must_use]
    pub const fn shared_preparation_peak_bytes(self) -> usize {
        self.shared_preparation_peak_bytes
    }

    /// Returns exact cross-identity prepared-fact cache hits.
    #[must_use]
    pub const fn shared_preparation_hits(self) -> usize {
        self.shared_preparation_hits
    }

    /// Returns eligible shared-preparation lookups that missed.
    #[must_use]
    pub const fn shared_preparation_misses(self) -> usize {
        self.shared_preparation_misses
    }

    /// Returns shared entries removed to enforce the byte budget.
    #[must_use]
    pub const fn shared_preparation_evictions(self) -> usize {
        self.shared_preparation_evictions
    }

    /// Returns prepared values served but too large to retain.
    #[must_use]
    pub const fn shared_preparation_oversized_non_retentions(self) -> usize {
        self.shared_preparation_oversized_non_retentions
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct CacheWork {
    scene_cache_accounted_bytes: usize,
    peak_entries: usize,
    hits: usize,
    misses: usize,
    evictions: usize,
    releases: usize,
}

#[derive(Debug, Default)]
struct PrepareScratch {
    region_attempts: Vec<crate::RegionAttempt>,
}

impl PrepareScratch {
    fn accounted_capacity_bytes(&self) -> usize {
        vec_bytes::<crate::RegionAttempt>(self.region_attempts.capacity())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum CacheKind {
    Committed,
    Composition,
}

impl CacheKind {
    const fn preparation_id(self, paragraph: ParagraphId) -> ParagraphPreparationId {
        ParagraphPreparationId::new(
            paragraph,
            match self {
                Self::Committed => 0,
                Self::Composition => 1,
            },
        )
    }
}

#[derive(Clone, Debug)]
struct PublishedScene {
    snapshot: DocumentSnapshot,
    styles: StyleMap,
    constraint: ConstraintKey,
    region_flow: Option<RegionFlow>,
    required_paint_slots: usize,
    core: Arc<SceneCore>,
    region_transcript: Option<RegionTranscript>,
    region_attempts: usize,
    region_height_rejections: usize,
}

#[derive(Clone, Debug)]
struct PublishedComposition {
    snapshot: DocumentSnapshot,
    styles: StyleMap,
    constraint: ConstraintKey,
    region_flow: Option<RegionFlow>,
    composition: CompositionSession,
    required_paint_slots: usize,
    core: Arc<SceneCore>,
    region_transcript: Option<RegionTranscript>,
    region_attempts: usize,
    region_height_rejections: usize,
}

#[derive(Clone, Debug)]
struct BlockStyles {
    inline: ComputedInlineStyle,
    paragraph: ParagraphStyle,
    map: StyleMap,
}

/// Mutable owner of one paragraph adapter and its retained stage caches.
pub struct LayoutEngine {
    paragraphs: Box<dyn ParagraphFormation>,
    cache: BTreeMap<ParagraphId, ParagraphCache>,
    composition_cache: BTreeMap<ParagraphId, ParagraphCache>,
    committed_recency: BTreeSet<(u64, ParagraphId)>,
    composition_recency: BTreeSet<(u64, ParagraphId)>,
    documents: BTreeMap<crate::DocumentId, BTreeSet<(CacheKind, ParagraphId)>>,
    clock: u64,
    budget: CacheBudget,
    cache_work: CacheWork,
    shared_preparation: SharedPreparationCache,
    scratch: PrepareScratch,
    published: BTreeMap<crate::DocumentId, PublishedScene>,
    published_compositions: BTreeMap<crate::DocumentId, PublishedComposition>,
    block_styles: BTreeMap<crate::DocumentId, BlockStyles>,
}

impl core::fmt::Debug for LayoutEngine {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("LayoutEngine")
            .field("cached_paragraphs", &self.cache.len())
            .field(
                "cached_composition_paragraphs",
                &self.composition_cache.len(),
            )
            .field(
                "shared_preparation_entries",
                &self.shared_preparation.diagnostics().entries,
            )
            .finish_non_exhaustive()
    }
}

impl LayoutEngine {
    /// Creates an engine owning one configured paragraph adapter and cache budget.
    #[must_use]
    pub fn new(paragraphs: impl ParagraphFormation + 'static, budget: CacheBudget) -> Self {
        Self {
            paragraphs: Box::new(paragraphs),
            cache: BTreeMap::new(),
            composition_cache: BTreeMap::new(),
            committed_recency: BTreeSet::new(),
            composition_recency: BTreeSet::new(),
            documents: BTreeMap::new(),
            clock: 0,
            budget,
            cache_work: CacheWork::default(),
            shared_preparation: SharedPreparationCache::new(budget.shared_preparation_bytes),
            scratch: PrepareScratch::default(),
            published: BTreeMap::new(),
            published_compositions: BTreeMap::new(),
            block_styles: BTreeMap::new(),
        }
    }

    /// Prepares an immutable scene without publishing partial results on failure.
    pub fn prepare(
        &mut self,
        snapshot: &DocumentSnapshot,
        request: &SceneRequest<'_>,
    ) -> Result<SceneOutput, SceneError> {
        if let Some(output) = self.reuse_published_scene(snapshot, request) {
            return Ok(output);
        }
        if let Some(output) = self.prepare_localized_normal_flow(snapshot, request)? {
            return Ok(output);
        }
        let required_paint_slots = validate_styles(snapshot, request)?;
        let previous_spine = self.reusable_scene_spine(snapshot, request);
        let mut spine = previous_spine.clone();
        let mut initial_segments = spine
            .is_none()
            .then(|| Vec::with_capacity(snapshot.paragraphs().len()));

        let cache_before = request.trace.then(|| self.cache_diagnostics());
        let scratch_capacity_before = request
            .trace
            .then(|| self.scratch.accounted_capacity_bytes());
        self.scratch.region_attempts.clear();
        let mut work = WorkReport::default();
        let mut reuse = PreparationReuse::default();
        let region_start = request.region_flow.map(RegionFlow::cursor);
        let mut region_cursor = region_start;

        for (paragraph_index, paragraph) in snapshot.paragraphs().iter().enumerate() {
            self.clock = self.clock.saturating_add(1);
            let access = if let Some(access) = reuse_paragraph_geometry(
                &mut self.cache,
                paragraph,
                request,
                region_cursor,
                self.clock,
                &mut work,
                &mut reuse,
            ) {
                access
            } else {
                let projection = Projection::new(paragraph, request)?;
                let preflight_key = ParagraphPreflightKey::new(paragraph, request, region_cursor);
                prepare_paragraph_geometry(
                    self.paragraphs.as_mut(),
                    &mut self.cache,
                    self.composition_cache.get(&paragraph.id),
                    CacheKind::Committed,
                    paragraph,
                    &projection,
                    preflight_key,
                    request.constraint,
                    request.region_flow,
                    region_cursor,
                    self.clock,
                    &mut self.shared_preparation,
                    &mut work,
                    &mut reuse,
                )?
            };
            self.record_access(CacheKind::Committed, &access);
            if let Some(transcript) = &access.region_transcript {
                self.scratch
                    .region_attempts
                    .extend_from_slice(transcript.attempts());
                region_cursor = Some(transcript.end());
            }
            let segment = Arc::clone(
                &self
                    .cache
                    .get(&paragraph.id)
                    .expect("prepared committed geometry must remain resident")
                    .segment,
            );
            debug_assert_eq!(
                segment.paragraph, paragraph.id,
                "the committed scene segment must retain its paragraph identity"
            );
            if let Some(segments) = &mut initial_segments {
                segments.push(Arc::clone(&segment));
            } else if !spine
                .as_ref()
                .and_then(|spine| spine.segment(paragraph_index))
                .is_some_and(|retained| Arc::ptr_eq(retained, &segment))
            {
                spine = Some(
                    spine
                        .as_ref()
                        .expect("a reused spine remains present")
                        .replace(paragraph_index, Arc::clone(&segment))
                        .expect("a reused spine has matching paragraph count"),
                );
            }
            self.enforce_budget();
        }

        let spine = initial_segments.map_or_else(
            || spine.unwrap_or_else(|| SceneSpine::empty(request.region_flow.is_none())),
            |segments| SceneSpine::from_segments(&segments, request.region_flow.is_none()),
        );
        let summary = spine.summary();
        work.paint = StageWork {
            paragraphs: snapshot.paragraphs().len(),
            records: summary.fragments,
        };
        let metrics = TextMetrics::from_summary(summary);
        let region_attempts = self.scratch.region_attempts.len();
        let region_height_rejections = self
            .scratch
            .region_attempts
            .iter()
            .filter(|attempt| attempt.outcome() == RegionAttemptOutcome::HeightRejected)
            .count();
        let region_transcript = match (request.region_flow, region_start, region_cursor) {
            (Some(flow), Some(start), Some(end)) => Some(RegionTranscript::try_new(
                flow,
                start,
                end,
                self.scratch.region_attempts.iter().cloned(),
            )?),
            (None, None, None) => None,
            _ => return Err(SceneError::new(SceneErrorKind::Flow)),
        };
        let trace = request.trace.then(|| PreparationTrace {
            work: work.clone(),
            reuse,
            memory: PreparationMemory {
                cache_before: cache_before.expect("traced request records initial cache state"),
                cache_after: self.cache_diagnostics(),
                scene_output_capacity_bytes: spine
                    .unshared_node_bytes_from(previous_spine.as_ref()),
                scratch_capacity_before: scratch_capacity_before
                    .expect("traced request records initial scratch state"),
                scratch_capacity_after: self.scratch.accounted_capacity_bytes(),
            },
            region_attempts,
            region_height_rejections,
        });
        let core = Arc::new(SceneCore {
            paragraph_count: snapshot.paragraphs().len(),
            spine,
            metrics,
        });
        let output = SceneOutput {
            scene: TextScene {
                document: snapshot.id(),
                revision: snapshot.revision(),
                paint: request.paint.clone(),
                core: Arc::clone(&core),
            },
            work,
            trace,
            region_transcript: region_transcript.clone(),
        };
        if snapshot
            .paragraphs()
            .iter()
            .all(|paragraph| self.cache.contains_key(&paragraph.id))
        {
            self.published.insert(
                snapshot.id(),
                PublishedScene {
                    snapshot: snapshot.clone(),
                    styles: request.styles.clone(),
                    constraint: ConstraintKey::from(request.constraint),
                    region_flow: request.region_flow.cloned(),
                    required_paint_slots,
                    core,
                    region_transcript,
                    region_attempts,
                    region_height_rejections,
                },
            );
        } else {
            self.published.remove(&snapshot.id());
        }
        Ok(output)
    }

    /// Prepares one retained block through the same paragraph and scene path as a document.
    pub fn prepare_block(
        &mut self,
        snapshot: &TextBlockSnapshot,
        request: &BlockRequest<'_>,
    ) -> Result<SceneOutput, SceneError> {
        let document = snapshot.document().id();
        let styles = self
            .block_styles
            .get(&document)
            .filter(|styles| {
                styles.inline == *request.style && styles.paragraph == request.paragraph_style
            })
            .map(|styles| styles.map.clone())
            .unwrap_or_else(|| {
                let styles = StyleMap::new(request.style.clone())
                    .with_default_paragraph_style(request.paragraph_style);
                self.block_styles.insert(
                    document,
                    BlockStyles {
                        inline: request.style.clone(),
                        paragraph: request.paragraph_style,
                        map: styles.clone(),
                    },
                );
                styles
            });
        let scene_request = match request.region_flow {
            Some(flow) => {
                SceneRequest::new(request.constraint, &styles, request.paint).with_region_flow(flow)
            }
            None => SceneRequest::new(request.constraint, &styles, request.paint),
        };
        let scene_request = if request.trace {
            scene_request.with_preparation_trace()
        } else {
            scene_request
        };
        self.prepare(snapshot.document(), &scene_request)
    }

    /// Prepares a transient generated-text scene without evicting committed work.
    pub fn prepare_composition(
        &mut self,
        snapshot: &DocumentSnapshot,
        request: &SceneRequest<'_>,
        composition: &CompositionSession,
    ) -> Result<CompositionSceneOutput, SceneError> {
        if composition.document() != snapshot.id()
            || composition.base_revision() != snapshot.revision()
        {
            return Err(SceneError::for_document(
                SceneErrorKind::InvalidComposition,
                snapshot.id(),
            ));
        }
        if let Some(output) = self.reuse_published_composition(snapshot, request, composition) {
            return Ok(output);
        }
        let required_paint_slots = validate_styles(snapshot, request)?;
        let target = composition.target_text().ok_or_else(|| {
            SceneError::for_document(SceneErrorKind::InvalidComposition, snapshot.id())
        })?;
        let previous_spine = self.reusable_composition_spine(snapshot, request);
        let mut spine = previous_spine.clone();
        let mut initial_segments = spine
            .is_none()
            .then(|| Vec::with_capacity(snapshot.paragraphs().len()));

        let cache_before = request.trace.then(|| self.cache_diagnostics());
        let scratch_capacity_before = request
            .trace
            .then(|| self.scratch.accounted_capacity_bytes());
        self.scratch.region_attempts.clear();
        let mut work = WorkReport::default();
        let mut reuse = PreparationReuse::default();
        let region_start = request.region_flow.map(RegionFlow::cursor);
        let mut region_cursor = region_start;

        for (paragraph_index, paragraph) in snapshot.paragraphs().iter().enumerate() {
            let transient = paragraph.id.index == target.paragraph;
            self.clock = self.clock.saturating_add(1);
            let (kind, access) = if !transient
                && let Some(access) = reuse_paragraph_geometry(
                    &mut self.cache,
                    paragraph,
                    request,
                    region_cursor,
                    self.clock,
                    &mut work,
                    &mut reuse,
                ) {
                (CacheKind::Committed, access)
            } else {
                let projection = if transient {
                    Projection::with_composition(paragraph, request, composition)?
                } else {
                    Projection::new(paragraph, request)?
                };
                let preflight_key = ParagraphPreflightKey::new(paragraph, request, region_cursor);
                if transient {
                    (
                        CacheKind::Composition,
                        prepare_paragraph_geometry(
                            self.paragraphs.as_mut(),
                            &mut self.composition_cache,
                            self.cache.get(&paragraph.id),
                            CacheKind::Composition,
                            paragraph,
                            &projection,
                            preflight_key,
                            request.constraint,
                            request.region_flow,
                            region_cursor,
                            self.clock,
                            &mut self.shared_preparation,
                            &mut work,
                            &mut reuse,
                        )?,
                    )
                } else {
                    (
                        CacheKind::Committed,
                        prepare_paragraph_geometry(
                            self.paragraphs.as_mut(),
                            &mut self.cache,
                            self.composition_cache.get(&paragraph.id),
                            CacheKind::Committed,
                            paragraph,
                            &projection,
                            preflight_key,
                            request.constraint,
                            request.region_flow,
                            region_cursor,
                            self.clock,
                            &mut self.shared_preparation,
                            &mut work,
                            &mut reuse,
                        )?,
                    )
                }
            };
            self.record_access(kind, &access);
            if let Some(transcript) = &access.region_transcript {
                self.scratch
                    .region_attempts
                    .extend_from_slice(transcript.attempts());
                region_cursor = Some(transcript.end());
            }
            let segment = match kind {
                CacheKind::Committed => Arc::clone(
                    &self
                        .cache
                        .get(&paragraph.id)
                        .expect("prepared committed geometry must remain resident")
                        .segment,
                ),
                CacheKind::Composition => Arc::clone(
                    &self
                        .composition_cache
                        .get(&paragraph.id)
                        .expect("prepared composition geometry must remain resident")
                        .segment,
                ),
            };
            debug_assert_eq!(
                segment.paragraph, paragraph.id,
                "the projected scene segment must retain its paragraph identity"
            );
            if let Some(segments) = &mut initial_segments {
                segments.push(Arc::clone(&segment));
            } else if !spine
                .as_ref()
                .and_then(|spine| spine.segment(paragraph_index))
                .is_some_and(|retained| Arc::ptr_eq(retained, &segment))
            {
                spine = Some(
                    spine
                        .as_ref()
                        .expect("a reused composition spine remains present")
                        .replace(paragraph_index, Arc::clone(&segment))
                        .expect("a reused composition spine has matching paragraph count"),
                );
            }
            self.enforce_budget();
        }

        let spine = initial_segments.map_or_else(
            || spine.unwrap_or_else(|| SceneSpine::empty(request.region_flow.is_none())),
            |segments| SceneSpine::from_segments(&segments, request.region_flow.is_none()),
        );
        let summary = spine.summary();
        work.paint = StageWork {
            paragraphs: snapshot.paragraphs().len(),
            records: summary.fragments,
        };
        let metrics = TextMetrics::from_summary(summary);
        let region_attempts = self.scratch.region_attempts.len();
        let region_height_rejections = self
            .scratch
            .region_attempts
            .iter()
            .filter(|attempt| attempt.outcome() == RegionAttemptOutcome::HeightRejected)
            .count();
        let region_transcript = match (request.region_flow, region_start, region_cursor) {
            (Some(flow), Some(start), Some(end)) => Some(RegionTranscript::try_new(
                flow,
                start,
                end,
                self.scratch.region_attempts.iter().cloned(),
            )?),
            (None, None, None) => None,
            _ => return Err(SceneError::new(SceneErrorKind::Flow)),
        };
        let trace = request.trace.then(|| PreparationTrace {
            work: work.clone(),
            reuse,
            memory: PreparationMemory {
                cache_before: cache_before.expect("traced request records initial cache state"),
                cache_after: self.cache_diagnostics(),
                scene_output_capacity_bytes: spine
                    .unshared_node_bytes_from(previous_spine.as_ref()),
                scratch_capacity_before: scratch_capacity_before
                    .expect("traced request records initial scratch state"),
                scratch_capacity_after: self.scratch.accounted_capacity_bytes(),
            },
            region_attempts,
            region_height_rejections,
        });
        let core = Arc::new(SceneCore {
            paragraph_count: snapshot.paragraphs().len(),
            spine,
            metrics,
        });
        let output = CompositionSceneOutput {
            scene: CompositionScene {
                document: snapshot.id(),
                revision: snapshot.revision(),
                composition: composition.id(),
                epoch: composition.epoch(),
                paint: request.paint.clone(),
                core: Arc::clone(&core),
            },
            work,
            trace,
            region_transcript: region_transcript.clone(),
        };
        if snapshot.paragraphs().iter().all(|paragraph| {
            if paragraph.id.index == target.paragraph {
                self.composition_cache.contains_key(&paragraph.id)
            } else {
                self.cache.contains_key(&paragraph.id)
            }
        }) {
            self.published_compositions.insert(
                snapshot.id(),
                PublishedComposition {
                    snapshot: snapshot.clone(),
                    styles: request.styles.clone(),
                    constraint: ConstraintKey::from(request.constraint),
                    region_flow: request.region_flow.cloned(),
                    composition: composition.clone(),
                    required_paint_slots,
                    core,
                    region_transcript,
                    region_attempts,
                    region_height_rejections,
                },
            );
        } else {
            self.published_compositions.remove(&snapshot.id());
        }
        Ok(output)
    }

    /// Releases every retained geometry and backend entry for one document.
    pub fn release_document(&mut self, document: crate::DocumentId) {
        self.published.remove(&document);
        self.published_compositions.remove(&document);
        self.block_styles.remove(&document);
        let Some(entries) = self.documents.remove(&document) else {
            return;
        };
        let mut preparations = BTreeSet::new();
        for (kind, paragraph) in entries {
            let removed = match kind {
                CacheKind::Committed => self.cache.remove(&paragraph),
                CacheKind::Composition => self.composition_cache.remove(&paragraph),
            };
            if let Some(entry) = removed {
                preparations.insert(entry.preparation);
                match kind {
                    CacheKind::Committed => {
                        self.committed_recency.remove(&(entry.last_used, paragraph));
                    }
                    CacheKind::Composition => {
                        self.composition_recency
                            .remove(&(entry.last_used, paragraph));
                    }
                }
                self.cache_work.scene_cache_accounted_bytes = self
                    .cache_work
                    .scene_cache_accounted_bytes
                    .saturating_sub(entry.accounted_bytes);
                self.cache_work.releases += 1;
            }
        }
        for preparation in preparations {
            self.paragraphs.release(preparation);
        }
    }

    /// Releases all retained geometry, shared preparation, and backend entries.
    pub fn clear_cache(&mut self) {
        self.cache_work.releases += self.cache.len() + self.composition_cache.len();
        self.cache.clear();
        self.composition_cache.clear();
        self.committed_recency.clear();
        self.composition_recency.clear();
        self.documents.clear();
        self.published.clear();
        self.published_compositions.clear();
        self.block_styles.clear();
        self.shared_preparation.clear();
        self.paragraphs.clear();
        self.cache_work.scene_cache_accounted_bytes = 0;
    }

    /// Returns a snapshot of coordinated cache state and cumulative activity.
    #[must_use]
    pub fn cache_diagnostics(&self) -> CacheDiagnostics {
        let shared = self.shared_preparation.diagnostics();
        CacheDiagnostics {
            budget: self.budget.max_entries,
            composition_budget: self.budget.max_composition_entries,
            committed_entries: self.cache.len(),
            composition_entries: self.composition_cache.len(),
            backend_entries: self.paragraphs.retained_entries(),
            scene_cache_accounted_bytes: self.cache_work.scene_cache_accounted_bytes,
            peak_entries: self.cache_work.peak_entries,
            hits: self.cache_work.hits,
            misses: self.cache_work.misses,
            evictions: self.cache_work.evictions,
            releases: self.cache_work.releases,
            shared_preparation_budget: shared.budget,
            shared_preparation_entries: shared.entries,
            shared_preparation_resident_bytes: shared.resident_bytes,
            shared_preparation_peak_bytes: shared.peak_bytes,
            shared_preparation_hits: shared.hits,
            shared_preparation_misses: shared.misses,
            shared_preparation_evictions: shared.evictions,
            shared_preparation_oversized_non_retentions: shared.oversized_non_retentions,
        }
    }

    #[cfg(test)]
    pub(super) fn replace_first_shared_facts_for_test(
        &mut self,
        facts: Arc<PreparedParagraphFacts>,
    ) {
        self.shared_preparation.replace_first_facts_for_test(facts);
    }

    #[cfg(test)]
    pub(super) fn collide_shared_bucket_for_test(&mut self, source: &str, target: &str) {
        self.shared_preparation
            .collide_bucket_for_test(source, target);
    }

    #[cfg(test)]
    pub(super) fn cached_geometry_for_test(
        &self,
        paragraph: ParagraphId,
    ) -> Option<Arc<CachedGeometry>> {
        self.cache
            .get(&paragraph)
            .map(|entry| Arc::clone(&entry.segment.geometry))
    }

    fn reuse_published_scene(
        &self,
        snapshot: &DocumentSnapshot,
        request: &SceneRequest<'_>,
    ) -> Option<SceneOutput> {
        let published = self.published.get(&snapshot.id())?;
        if !published.snapshot.shares_state_with(snapshot)
            || !published.styles.shares_state_with(request.styles)
            || published.constraint != ConstraintKey::from(request.constraint)
            || !region_provenance_matches(published.region_flow.as_ref(), request.region_flow)
            || request.paint.len() < published.required_paint_slots
        {
            return None;
        }
        let paragraph_count = published.core.paragraph_count;
        let work = WorkReport {
            reused_paragraphs: paragraph_count,
            ..WorkReport::default()
        };
        let trace = request.trace.then(|| {
            let diagnostics = self.cache_diagnostics();
            PreparationTrace {
                work: work.clone(),
                reuse: PreparationReuse {
                    paragraphs: paragraph_count,
                    preflight_reuses: paragraph_count,
                    exact_geometry_reuses: paragraph_count,
                    ..PreparationReuse::default()
                },
                memory: PreparationMemory {
                    cache_before: diagnostics,
                    cache_after: diagnostics,
                    scene_output_capacity_bytes: 0,
                    scratch_capacity_before: self.scratch.accounted_capacity_bytes(),
                    scratch_capacity_after: self.scratch.accounted_capacity_bytes(),
                },
                region_attempts: published.region_attempts,
                region_height_rejections: published.region_height_rejections,
            }
        });
        Some(SceneOutput {
            scene: TextScene {
                document: snapshot.id(),
                revision: snapshot.revision(),
                paint: request.paint.clone(),
                core: Arc::clone(&published.core),
            },
            work,
            trace,
            region_transcript: published.region_transcript.clone(),
        })
    }

    fn prepare_localized_normal_flow(
        &mut self,
        snapshot: &DocumentSnapshot,
        request: &SceneRequest<'_>,
    ) -> Result<Option<SceneOutput>, SceneError> {
        let Some(published) = self.published.get(&snapshot.id()) else {
            return Ok(None);
        };
        if !published.styles.shares_state_with(request.styles)
            || published.constraint != ConstraintKey::from(request.constraint)
            || published.region_flow.is_some()
            || request.region_flow.is_some()
            || published.core.paragraph_count != snapshot.paragraphs().len()
            || request.paint.len() < published.required_paint_slots
        {
            return Ok(None);
        }
        let previous = published.snapshot.clone();
        let previous_core = Arc::clone(&published.core);
        let mut changed = snapshot.changed_paragraphs_from(&previous).ok_or_else(|| {
            SceneError::for_document(SceneErrorKind::SourceCoverage, snapshot.id())
        })?;
        if changed.any(|index| {
            let Some(previous) = previous.paragraphs().get(index) else {
                return true;
            };
            let Some(current) = snapshot.paragraphs().get(index) else {
                return true;
            };
            !paragraph_source_structure_matches(previous, current)
                || !self.cache.contains_key(&current.id)
        }) {
            return Ok(None);
        }

        let mut spine = published.core.spine.clone();
        let required_paint_slots = published.required_paint_slots;
        let cache_before = request.trace.then(|| self.cache_diagnostics());
        let scratch_capacity_before = request
            .trace
            .then(|| self.scratch.accounted_capacity_bytes());
        let mut work = WorkReport::default();
        let mut reuse = PreparationReuse::default();
        let mut changed_count = 0_usize;
        let mut paint_records = 0_usize;

        for paragraph_index in snapshot
            .changed_paragraphs_from(&previous)
            .expect("the preflight proved matching paragraph sequences")
        {
            let paragraph = snapshot
                .paragraphs()
                .get(paragraph_index)
                .expect("the structural diff yields an existing paragraph");
            self.clock = self.clock.saturating_add(1);
            let projection = Projection::new(paragraph, request)?;
            let preflight_key = ParagraphPreflightKey::new(paragraph, request, None);
            let access = prepare_paragraph_geometry(
                self.paragraphs.as_mut(),
                &mut self.cache,
                self.composition_cache.get(&paragraph.id),
                CacheKind::Committed,
                paragraph,
                &projection,
                preflight_key,
                request.constraint,
                None,
                None,
                self.clock,
                &mut self.shared_preparation,
                &mut work,
                &mut reuse,
            )?;
            self.record_access(CacheKind::Committed, &access);
            let segment = Arc::clone(
                &self
                    .cache
                    .get(&paragraph.id)
                    .expect("localized preparation retains its paragraph segment")
                    .segment,
            );
            paint_records = paint_records.saturating_add(segment.geometry.fragments.len());
            spine = spine
                .replace(paragraph_index, segment)
                .expect("the retained spine has the same paragraph count");
            self.enforce_budget();
            changed_count = changed_count.saturating_add(1);
        }

        let paragraph_count = snapshot.paragraphs().len();
        let unchanged = paragraph_count.saturating_sub(changed_count);
        work.reused_paragraphs = work.reused_paragraphs.saturating_add(unchanged);
        reuse.paragraphs = reuse.paragraphs.saturating_add(unchanged);
        reuse.preflight_reuses = reuse.preflight_reuses.saturating_add(unchanged);
        reuse.exact_geometry_reuses = reuse.exact_geometry_reuses.saturating_add(unchanged);
        let summary = spine.summary();
        work.paint = StageWork {
            paragraphs: changed_count,
            records: paint_records,
        };
        let trace = request.trace.then(|| PreparationTrace {
            work: work.clone(),
            reuse,
            memory: PreparationMemory {
                cache_before: cache_before.expect("traced request records initial cache state"),
                cache_after: self.cache_diagnostics(),
                scene_output_capacity_bytes: spine
                    .unshared_node_bytes_from(Some(&previous_core.spine)),
                scratch_capacity_before: scratch_capacity_before
                    .expect("traced request records initial scratch state"),
                scratch_capacity_after: self.scratch.accounted_capacity_bytes(),
            },
            region_attempts: 0,
            region_height_rejections: 0,
        });
        let core = if changed_count == 0 {
            previous_core
        } else {
            Arc::new(SceneCore {
                paragraph_count,
                metrics: TextMetrics::from_summary(summary),
                spine,
            })
        };
        let output = SceneOutput {
            scene: TextScene {
                document: snapshot.id(),
                revision: snapshot.revision(),
                paint: request.paint.clone(),
                core: Arc::clone(&core),
            },
            work,
            trace,
            region_transcript: None,
        };
        self.published.insert(
            snapshot.id(),
            PublishedScene {
                snapshot: snapshot.clone(),
                styles: request.styles.clone(),
                constraint: ConstraintKey::from(request.constraint),
                region_flow: None,
                required_paint_slots,
                core,
                region_transcript: None,
                region_attempts: 0,
                region_height_rejections: 0,
            },
        );
        Ok(Some(output))
    }

    fn reusable_scene_spine(
        &self,
        snapshot: &DocumentSnapshot,
        request: &SceneRequest<'_>,
    ) -> Option<SceneSpine> {
        let published = self.published.get(&snapshot.id())?;
        (published.styles.shares_state_with(request.styles)
            && published.constraint == ConstraintKey::from(request.constraint)
            && region_provenance_matches(published.region_flow.as_ref(), request.region_flow)
            && published.core.paragraph_count == snapshot.paragraphs().len())
        .then(|| published.core.spine.clone())
    }

    fn reuse_published_composition(
        &self,
        snapshot: &DocumentSnapshot,
        request: &SceneRequest<'_>,
        composition: &CompositionSession,
    ) -> Option<CompositionSceneOutput> {
        let published = self.published_compositions.get(&snapshot.id())?;
        if !published.snapshot.shares_state_with(snapshot)
            || !published.styles.shares_state_with(request.styles)
            || published.constraint != ConstraintKey::from(request.constraint)
            || !region_provenance_matches(published.region_flow.as_ref(), request.region_flow)
            || !published.composition.shares_state_with(composition)
            || request.paint.len() < published.required_paint_slots
        {
            return None;
        }
        let paragraph_count = published.core.paragraph_count;
        let work = WorkReport {
            reused_paragraphs: paragraph_count,
            ..WorkReport::default()
        };
        let trace = request.trace.then(|| {
            let diagnostics = self.cache_diagnostics();
            PreparationTrace {
                work: work.clone(),
                reuse: PreparationReuse {
                    paragraphs: paragraph_count,
                    preflight_reuses: paragraph_count,
                    exact_geometry_reuses: paragraph_count,
                    ..PreparationReuse::default()
                },
                memory: PreparationMemory {
                    cache_before: diagnostics,
                    cache_after: diagnostics,
                    scene_output_capacity_bytes: 0,
                    scratch_capacity_before: self.scratch.accounted_capacity_bytes(),
                    scratch_capacity_after: self.scratch.accounted_capacity_bytes(),
                },
                region_attempts: published.region_attempts,
                region_height_rejections: published.region_height_rejections,
            }
        });
        Some(CompositionSceneOutput {
            scene: CompositionScene {
                document: snapshot.id(),
                revision: snapshot.revision(),
                composition: composition.id(),
                epoch: composition.epoch(),
                paint: request.paint.clone(),
                core: Arc::clone(&published.core),
            },
            work,
            trace,
            region_transcript: published.region_transcript.clone(),
        })
    }

    fn reusable_composition_spine(
        &self,
        snapshot: &DocumentSnapshot,
        request: &SceneRequest<'_>,
    ) -> Option<SceneSpine> {
        self.published_compositions
            .get(&snapshot.id())
            .filter(|published| {
                published.snapshot.shares_state_with(snapshot)
                    && published.styles.shares_state_with(request.styles)
                    && published.constraint == ConstraintKey::from(request.constraint)
                    && region_provenance_matches(
                        published.region_flow.as_ref(),
                        request.region_flow,
                    )
                    && published.core.paragraph_count == snapshot.paragraphs().len()
            })
            .map(|published| published.core.spine.clone())
            .or_else(|| self.reusable_scene_spine(snapshot, request))
    }

    fn record_access(&mut self, kind: CacheKind, access: &CacheAccess) {
        if access.previous_accounted_bytes != access.current_accounted_bytes {
            self.cache_work.scene_cache_accounted_bytes = self
                .cache_work
                .scene_cache_accounted_bytes
                .saturating_sub(access.previous_accounted_bytes)
                .saturating_add(access.current_accounted_bytes);
        }
        if let Some(previous) = access.previous_use {
            match kind {
                CacheKind::Committed => {
                    self.committed_recency.remove(&(previous, access.paragraph));
                }
                CacheKind::Composition => {
                    self.composition_recency
                        .remove(&(previous, access.paragraph));
                }
            }
            self.cache_work.hits += 1;
        } else {
            self.cache_work.misses += 1;
            self.documents
                .entry(access.paragraph.document)
                .or_default()
                .insert((kind, access.paragraph));
        }
        match kind {
            CacheKind::Committed => {
                self.committed_recency
                    .insert((access.current_use, access.paragraph));
            }
            CacheKind::Composition => {
                self.composition_recency
                    .insert((access.current_use, access.paragraph));
            }
        }
        self.cache_work.peak_entries = self
            .cache_work
            .peak_entries
            .max(self.cache.len() + self.composition_cache.len());
    }

    fn enforce_budget(&mut self) {
        self.enforce_lane_budget(CacheKind::Committed, self.budget.max_entries);
        self.enforce_lane_budget(CacheKind::Composition, self.budget.max_composition_entries);
    }

    fn enforce_lane_budget(&mut self, kind: CacheKind, max_entries: usize) {
        while match kind {
            CacheKind::Committed => self.cache.len(),
            CacheKind::Composition => self.composition_cache.len(),
        } > max_entries
        {
            let next = match kind {
                CacheKind::Committed => self.committed_recency.pop_first(),
                CacheKind::Composition => self.composition_recency.pop_first(),
            };
            let Some((_, paragraph)) = next else {
                break;
            };
            // An exact root is retainable only while every segment it names is
            // inside its coordinated lane budget. A committed eviction also
            // invalidates composition roots that share committed siblings;
            // evicting a transient target cannot invalidate the independent
            // committed root.
            match kind {
                CacheKind::Committed => {
                    self.published.remove(&paragraph.document);
                    self.published_compositions.remove(&paragraph.document);
                }
                CacheKind::Composition => {
                    self.published_compositions.remove(&paragraph.document);
                }
            }
            let removed = match kind {
                CacheKind::Committed => self.cache.remove(&paragraph),
                CacheKind::Composition => self.composition_cache.remove(&paragraph),
            };
            if let Some(entry) = removed {
                self.paragraphs.release(entry.preparation);
                self.cache_work.scene_cache_accounted_bytes = self
                    .cache_work
                    .scene_cache_accounted_bytes
                    .saturating_sub(entry.accounted_bytes);
            }
            if let Some(entries) = self.documents.get_mut(&paragraph.document) {
                entries.remove(&(kind, paragraph));
                if entries.is_empty() {
                    self.documents.remove(&paragraph.document);
                    self.block_styles.remove(&paragraph.document);
                }
            }
            self.cache_work.evictions += 1;
        }
    }
}

fn paragraph_source_structure_matches(previous: &Paragraph, current: &Paragraph) -> bool {
    previous.id == current.id
        && previous.leaves.len() == current.leaves.len()
        && previous
            .leaves
            .iter()
            .zip(&current.leaves)
            .all(|(previous, current)| previous.id == current.id)
}

#[derive(Clone, Debug)]
struct CacheAccess {
    paragraph: ParagraphId,
    previous_use: Option<u64>,
    current_use: u64,
    previous_accounted_bytes: usize,
    current_accounted_bytes: usize,
    region_transcript: Option<RegionTranscript>,
}

fn reuse_paragraph_geometry(
    cache: &mut BTreeMap<ParagraphId, ParagraphCache>,
    paragraph: &Paragraph,
    request: &SceneRequest<'_>,
    region_cursor: Option<RegionCursor>,
    current_use: u64,
    work: &mut WorkReport,
    reuse: &mut PreparationReuse,
) -> Option<CacheAccess> {
    let entry = cache.get_mut(&paragraph.id)?;
    if !entry
        .preflight_key
        .matches(paragraph, request, region_cursor)
    {
        return None;
    }
    let previous_use = Some(entry.last_used);
    entry.last_used = current_use;
    reuse.paragraphs = reuse.paragraphs.saturating_add(1);
    reuse.preflight_reuses = reuse.preflight_reuses.saturating_add(1);
    reuse.exact_geometry_reuses = reuse.exact_geometry_reuses.saturating_add(1);
    work.reused_paragraphs = work.reused_paragraphs.saturating_add(1);
    Some(CacheAccess {
        paragraph: paragraph.id,
        previous_use,
        current_use,
        previous_accounted_bytes: entry.accounted_bytes,
        current_accounted_bytes: entry.accounted_bytes,
        region_transcript: entry.segment.region_transcript.clone(),
    })
}

fn prepare_paragraph_geometry(
    paragraphs: &mut dyn ParagraphFormation,
    cache: &mut BTreeMap<ParagraphId, ParagraphCache>,
    alternate: Option<&ParagraphCache>,
    cache_kind: CacheKind,
    paragraph: &Paragraph,
    projection: &Projection<'_>,
    preflight_key: ParagraphPreflightKey,
    constraint: TextConstraint,
    region_flow: Option<&RegionFlow>,
    region_cursor: Option<RegionCursor>,
    current_use: u64,
    shared_preparation: &mut SharedPreparationCache,
    work: &mut WorkReport,
    reuse: &mut PreparationReuse,
) -> Result<CacheAccess, SceneError> {
    reuse.paragraphs = reuse.paragraphs.saturating_add(1);
    let preparation = cache.get(&paragraph.id).map_or_else(
        || cache_kind.preparation_id(paragraph.id),
        |entry| entry.preparation,
    );
    let formation_change =
        cache
            .get(&paragraph.id)
            .map_or_else(ParagraphFormationChange::all, |entry| {
                entry.formation_key.adapter_change(
                    projection,
                    constraint,
                    region_flow,
                    region_cursor,
                    entry.paint_runs != projection.paint_runs,
                )
            });
    let reusable_preparation = alternate
        .filter(|entry| {
            entry
                .formation_key
                .adapter_change(
                    projection,
                    constraint,
                    region_flow,
                    region_cursor,
                    entry.paint_runs != projection.paint_runs,
                )
                .output_retained()
        })
        .map(|entry| entry.preparation);
    let cached = cache.contains_key(&paragraph.id);
    let formation_matches = cache.get(&paragraph.id).is_some_and(|entry| {
        entry.formation_key.matches(
            paragraph.version,
            projection,
            constraint,
            region_flow,
            region_cursor,
        )
    });
    let paint_matches = cache
        .get(&paragraph.id)
        .is_some_and(|entry| entry.paint_runs == projection.paint_runs);
    let adjustment_matches = cache.get(&paragraph.id).is_some_and(|entry| {
        entry.formation_key.paragraph_style.alignment() == projection.paragraph_style.alignment()
    });
    let retained_paint_geometry =
        (formation_matches && adjustment_matches && !paint_matches).then(|| {
            Arc::clone(
                &cache
                    .get(&paragraph.id)
                    .expect("paint-only reuse requires retained geometry")
                    .segment
                    .geometry,
            )
        });
    if !cached {
        reuse.cold_paragraphs = reuse.cold_paragraphs.saturating_add(1);
    } else {
        if !formation_matches {
            reuse.formation_invalidations = reuse.formation_invalidations.saturating_add(1);
        }
        if !adjustment_matches {
            reuse.adjustment_invalidations = reuse.adjustment_invalidations.saturating_add(1);
        }
        if !paint_matches {
            reuse.paint_invalidations = reuse.paint_invalidations.saturating_add(1);
        }
    }
    if formation_matches && paint_matches && adjustment_matches {
        let entry = cache
            .get_mut(&paragraph.id)
            .expect("a reusable cache entry must exist");
        let previous_use = Some(entry.last_used);
        entry.last_used = current_use;
        entry.preflight_key = preflight_key;
        if let Some((id, epoch)) = projection.composition_identity() {
            let segment = Arc::make_mut(&mut entry.segment);
            rebind_composition_geometry(Arc::make_mut(&mut segment.geometry), id, epoch);
        }
        work.reused_paragraphs += 1;
        reuse.exact_geometry_reuses = reuse.exact_geometry_reuses.saturating_add(1);
        return Ok(CacheAccess {
            paragraph: paragraph.id,
            previous_use,
            current_use,
            previous_accounted_bytes: entry.accounted_bytes,
            current_accounted_bytes: entry.accounted_bytes,
            region_transcript: entry.segment.region_transcript.clone(),
        });
    }

    let shaping_styles: Vec<_> = projection
        .shaping_styles
        .iter()
        .map(|style| (*style).clone())
        .collect();
    let text_len = u32::try_from(projection.mapping.text().len())
        .map_err(|_| SceneError::for_paragraph(SceneErrorKind::SourceCoverage, paragraph.id))?;
    let preparation_epoch = paragraphs.shared_preparation_epoch();
    shared_preparation.synchronize_epoch(preparation_epoch);
    let shared_query = preparation_epoch
        .filter(|_| shared_preparation.is_enabled())
        .map(|epoch| SharedPreparationQuery {
            epoch,
            projection,
            constraint,
            region_flow,
            region_cursor,
        });
    let shared_hit = shared_query
        .as_ref()
        .and_then(|query| shared_preparation.lookup(query, current_use));
    let (prepared, candidate_transcript, backend_called) = if let Some(hit) = shared_hit {
        // The shared result may represent a state the identity-bound backend
        // never observed. Drop any older lane-local facts so a later call
        // cannot apply a relative change record to the wrong base.
        paragraphs.release(preparation);
        work.shared_preparations = work.shared_preparations.saturating_add(1);
        reuse.shared_preparation_reuses = reuse.shared_preparation_reuses.saturating_add(1);
        let transcript = hit.region_transcript(paragraph.id, region_flow)?;
        (
            PreparedParagraph::from_shared_facts(paragraph.id, hit.facts),
            transcript,
            false,
        )
    } else {
        reuse.adapter_calls = reuse.adapter_calls.saturating_add(1);
        let constraints = match (region_flow, region_cursor) {
            (Some(flow), Some(cursor)) => ParagraphConstraints::in_regions(
                constraint,
                projection.empty_line_height(),
                flow.clone(),
                cursor,
            ),
            (None, None) => ParagraphConstraints::new(constraint, projection.empty_line_height()),
            _ => {
                return Err(SceneError::for_paragraph(
                    SceneErrorKind::Flow,
                    paragraph.id,
                ));
            }
        };
        let output = match paragraphs.form(
            ParagraphInput::new(
                preparation,
                reusable_preparation,
                formation_change,
                paragraph.id,
                projection.paragraph_style,
                projection.mapping.text(),
                &projection.analysis_styles,
                &projection.analysis_runs,
                &shaping_styles,
                &projection.shaping_runs,
                &projection.inline_flow_styles,
                &projection.inline_flow_runs,
                &projection.paint_runs,
            ),
            constraints,
        ) {
            Ok(output) => output,
            Err(error) => {
                paragraphs.release(preparation);
                return Err(SceneError::from_preparation(paragraph.id, error.kind()));
            }
        };
        record_formation_work(work, output.work());
        (
            output.paragraph().clone(),
            output.region_transcript().cloned(),
            true,
        )
    };
    if prepared.paragraph() != paragraph.id || prepared.text_len() != text_len {
        if backend_called {
            paragraphs.release(preparation);
        }
        return Err(SceneError::for_paragraph(
            SceneErrorKind::SourceCoverage,
            paragraph.id,
        ));
    }
    if let Err(error) = validate_prepared(&prepared, projection) {
        if backend_called {
            paragraphs.release(preparation);
        }
        return Err(error);
    }
    let region_transcript = match (region_flow, region_cursor, candidate_transcript) {
        (Some(flow), Some(cursor), Some(transcript))
            if transcript.start() == cursor
                && transcript.replay(flow) == Ok(transcript.end())
                && region_output_matches(&prepared, &transcript, projection) =>
        {
            Some(transcript)
        }
        (None, None, None) => None,
        _ => {
            if backend_called {
                paragraphs.release(preparation);
            }
            return Err(SceneError::for_paragraph(
                SceneErrorKind::Flow,
                paragraph.id,
            ));
        }
    };
    let slots_match = prepared
        .lines()
        .iter()
        .all(|line| line.slot().is_some() == region_flow.is_some());
    if !slots_match {
        if backend_called {
            paragraphs.release(preparation);
        }
        return Err(SceneError::for_paragraph(
            SceneErrorKind::Flow,
            paragraph.id,
        ));
    }
    if backend_called && projection.mapping.text().is_empty() && !formation_matches {
        work.flow.add_paragraph(1);
    }
    let geometry = match retained_paint_geometry.as_ref().map_or_else(
        || {
            build_geometry(
                &prepared,
                projection,
                constraint,
                region_transcript.as_ref(),
            )
        },
        |retained| repaint_geometry(&prepared, projection, retained),
    ) {
        Ok(geometry) => geometry,
        Err(error) => {
            if backend_called {
                paragraphs.release(preparation);
            }
            return Err(error);
        }
    };
    if backend_called && let Some(query) = &shared_query {
        shared_preparation.insert(
            query,
            prepared.shared_facts(),
            region_transcript.as_ref(),
            current_use,
        );
    }
    work.adjustment.add_paragraph(if geometry.lines.is_empty() {
        1
    } else {
        geometry.lines.len()
    });
    work.geometry.add_paragraph(geometry.fragments.len());
    let formation_key = FormationKey::new(
        paragraph.version,
        alloc::string::String::from(projection.mapping.text()),
        projection.analysis_styles.clone(),
        projection.analysis_runs.clone(),
        shaping_styles,
        projection.shaping_runs.clone(),
        projection.inline_flow_styles.clone(),
        projection.inline_flow_runs.clone(),
        projection.paragraph_style,
        constraint,
        region_flow.cloned(),
        region_cursor,
        projection.empty_line_height_key(),
        projection,
    );
    let (previous_use, previous_accounted_bytes, current_accounted_bytes) =
        if let Some(entry) = cache.get_mut(&paragraph.id) {
            let previous_use = Some(entry.last_used);
            let previous_accounted_bytes = entry.accounted_bytes;
            entry.last_used = current_use;
            entry.preflight_key = preflight_key;
            entry.formation_key = formation_key;
            entry.paint_runs = projection.paint_runs.clone();
            entry.segment = Arc::new(ParagraphSceneSegment::new(
                paragraph.id,
                Arc::new(geometry),
                region_transcript.clone(),
            ));
            entry.accounted_bytes = entry.calculate_accounted_owned_bytes();
            (
                previous_use,
                previous_accounted_bytes,
                entry.accounted_bytes,
            )
        } else {
            let mut entry = ParagraphCache {
                last_used: current_use,
                preparation,
                preflight_key,
                formation_key,
                paint_runs: projection.paint_runs.clone(),
                segment: Arc::new(ParagraphSceneSegment::new(
                    paragraph.id,
                    Arc::new(geometry),
                    region_transcript.clone(),
                )),
                accounted_bytes: 0,
            };
            entry.accounted_bytes = entry.calculate_accounted_owned_bytes();
            let current_accounted_bytes = entry.accounted_bytes;
            cache.insert(paragraph.id, entry);
            (None, 0, current_accounted_bytes)
        };
    Ok(CacheAccess {
        paragraph: paragraph.id,
        previous_use,
        current_use,
        previous_accounted_bytes,
        current_accounted_bytes,
        region_transcript,
    })
}

#[derive(Clone, Debug)]
struct ParagraphPreflightKey {
    version: u64,
    styles: StyleMap,
    default_style: ComputedInlineStyle,
    source_styles: Vec<ComputedInlineStyle>,
    paragraph_style: ParagraphStyle,
    constraint: ConstraintKey,
    region_flow: Option<RegionFlow>,
    region_cursor: Option<RegionCursor>,
}

impl ParagraphPreflightKey {
    fn new(
        paragraph: &Paragraph,
        request: &SceneRequest<'_>,
        region_cursor: Option<RegionCursor>,
    ) -> Self {
        Self {
            version: paragraph.version,
            styles: request.styles.clone(),
            default_style: request.styles.default_style().clone(),
            source_styles: paragraph
                .leaves
                .iter()
                .map(|leaf| request.styles.style_for(leaf.id).clone())
                .collect(),
            paragraph_style: request.styles.paragraph_style_for(paragraph.id),
            constraint: ConstraintKey::from(request.constraint),
            region_flow: request.region_flow.cloned(),
            region_cursor,
        }
    }

    fn matches(
        &self,
        paragraph: &Paragraph,
        request: &SceneRequest<'_>,
        region_cursor: Option<RegionCursor>,
    ) -> bool {
        self.version == paragraph.version
            && self.constraint == ConstraintKey::from(request.constraint)
            && self.region_cursor == region_cursor
            && region_provenance_matches(self.region_flow.as_ref(), request.region_flow)
            && (self.styles.shares_state_with(request.styles)
                || (self.default_style == *request.styles.default_style()
                    && self.paragraph_style == request.styles.paragraph_style_for(paragraph.id)
                    && self.source_styles.len() == paragraph.leaves.len()
                    && self
                        .source_styles
                        .iter()
                        .zip(&paragraph.leaves)
                        .all(|(cached, leaf)| cached == request.styles.style_for(leaf.id))))
    }
}

#[derive(Clone, Debug, PartialEq)]
struct FormationKey {
    version: u64,
    text: alloc::string::String,
    source_map: Vec<ProjectionSourceKey>,
    analysis_styles: Vec<AnalysisStyle>,
    analysis_runs: Vec<AnalysisRun>,
    shaping_styles: Vec<ShapingStyle>,
    shaping_runs: Vec<ShapingRun>,
    inline_flow_styles: Vec<InlineFlowStyle>,
    inline_flow_runs: Vec<InlineFlowRun>,
    paragraph_style: ParagraphStyle,
    constraint: ConstraintKey,
    region_flow: Option<RegionFlow>,
    region_cursor: Option<RegionCursor>,
    empty_line_height: u64,
}

impl FormationKey {
    fn new(
        version: u64,
        text: alloc::string::String,
        analysis_styles: Vec<AnalysisStyle>,
        analysis_runs: Vec<AnalysisRun>,
        shaping_styles: Vec<ShapingStyle>,
        shaping_runs: Vec<ShapingRun>,
        inline_flow_styles: Vec<InlineFlowStyle>,
        inline_flow_runs: Vec<InlineFlowRun>,
        paragraph_style: ParagraphStyle,
        constraint: TextConstraint,
        region_flow: Option<RegionFlow>,
        region_cursor: Option<RegionCursor>,
        empty_line_height: u64,
        projection: &Projection<'_>,
    ) -> Self {
        Self {
            version,
            text,
            source_map: ProjectionSourceKey::from_projection(projection),
            analysis_styles,
            analysis_runs,
            shaping_styles,
            shaping_runs,
            inline_flow_styles,
            inline_flow_runs,
            paragraph_style,
            constraint: ConstraintKey::from(constraint),
            region_flow,
            region_cursor,
            empty_line_height,
        }
    }

    fn matches(
        &self,
        version: u64,
        projection: &Projection<'_>,
        constraint: TextConstraint,
        region_flow: Option<&RegionFlow>,
        region_cursor: Option<RegionCursor>,
    ) -> bool {
        self.version == version
            && self.text == projection.mapping.text()
            && self.source_map == ProjectionSourceKey::from_projection(projection)
            && self.analysis_styles == projection.analysis_styles
            && self.analysis_runs == projection.analysis_runs
            && self.shaping_styles.len() == projection.shaping_styles.len()
            && self
                .shaping_styles
                .iter()
                .zip(&projection.shaping_styles)
                .all(|(cached, projected)| cached == *projected)
            && self.shaping_runs == projection.shaping_runs
            && self.inline_flow_styles == projection.inline_flow_styles
            && self.inline_flow_runs == projection.inline_flow_runs
            && self.paragraph_style.base_direction() == projection.paragraph_style.base_direction()
            && self.paragraph_style.whitespace_collapse()
                == projection.paragraph_style.whitespace_collapse()
            && self.constraint == ConstraintKey::from(constraint)
            && option_ref_eq(self.region_flow.as_ref(), region_flow)
            && self.region_cursor == region_cursor
            && self.empty_line_height == projection.empty_line_height_key()
    }

    fn adapter_change(
        &self,
        projection: &Projection<'_>,
        constraint: TextConstraint,
        region_flow: Option<&RegionFlow>,
        region_cursor: Option<RegionCursor>,
        paint: bool,
    ) -> ParagraphFormationChange {
        let analysis = self.text != projection.mapping.text()
            || self.analysis_styles != projection.analysis_styles
            || self.analysis_runs != projection.analysis_runs
            || self.paragraph_style.base_direction() != projection.paragraph_style.base_direction();
        let font_selection =
            !shaping_styles_match(&self.shaping_styles, &projection.shaping_styles)
                || self.shaping_runs != projection.shaping_runs;
        let ligature_policy = !inline_flow_values_match(
            &self.inline_flow_styles,
            &self.inline_flow_runs,
            &projection.inline_flow_styles,
            &projection.inline_flow_runs,
            |left, right| (left.spacing().letter() == 0.0) == (right.spacing().letter() == 0.0),
        );
        let inline_flow_projection = self.inline_flow_styles != projection.inline_flow_styles
            || self.inline_flow_runs != projection.inline_flow_runs;
        let spacing = !inline_flow_values_match(
            &self.inline_flow_styles,
            &self.inline_flow_runs,
            &projection.inline_flow_styles,
            &projection.inline_flow_runs,
            |left, right| left.spacing() == right.spacing(),
        );
        let line_metrics = !inline_flow_values_match(
            &self.inline_flow_styles,
            &self.inline_flow_runs,
            &projection.inline_flow_styles,
            &projection.inline_flow_runs,
            |left, right| left.line_height() == right.line_height(),
        );
        let break_policy = !inline_flow_values_match(
            &self.inline_flow_styles,
            &self.inline_flow_runs,
            &projection.inline_flow_styles,
            &projection.inline_flow_runs,
            |left, right| {
                left.overflow_wrap() == right.overflow_wrap()
                    && left.text_wrap_mode() == right.text_wrap_mode()
            },
        );
        let constraints = self.constraint != ConstraintKey::from(constraint)
            || !option_ref_eq(self.region_flow.as_ref(), region_flow)
            || self.region_cursor != region_cursor
            || (projection.mapping.text().is_empty()
                && self.empty_line_height != projection.empty_line_height_key());
        ParagraphFormationChange::new(
            analysis,
            font_selection,
            ligature_policy,
            inline_flow_projection,
            spacing,
            line_metrics,
            break_policy,
            constraints,
            paint,
        )
    }
}

fn shaping_styles_match(left: &[ShapingStyle], right: &[&ShapingStyle]) -> bool {
    left.len() == right.len()
        && left
            .iter()
            .zip(right)
            .all(|(cached, projected)| cached == *projected)
}

fn inline_flow_values_match(
    left_styles: &[InlineFlowStyle],
    left_runs: &[InlineFlowRun],
    right_styles: &[InlineFlowStyle],
    right_runs: &[InlineFlowRun],
    values_match: impl Fn(InlineFlowStyle, InlineFlowStyle) -> bool,
) -> bool {
    if left_runs.is_empty() || right_runs.is_empty() {
        return left_runs.is_empty() && right_runs.is_empty();
    }
    let mut left = 0_usize;
    let mut right = 0_usize;
    let mut source = 0_u32;
    while let (Some(left_run), Some(right_run)) = (left_runs.get(left), right_runs.get(right)) {
        let left_range = left_run.bytes();
        let right_range = right_run.bytes();
        if left_range.start > source
            || source >= left_range.end
            || right_range.start > source
            || source >= right_range.end
        {
            return false;
        }
        let Some(left_style) = left_styles.get(left_run.style().index()).copied() else {
            return false;
        };
        let Some(right_style) = right_styles.get(right_run.style().index()).copied() else {
            return false;
        };
        if !values_match(left_style, right_style) {
            return false;
        }
        source = left_range.end.min(right_range.end);
        if source == left_range.end {
            left += 1;
        }
        if source == right_range.end {
            right += 1;
        }
    }
    left == left_runs.len() && right == right_runs.len()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ConstraintKey {
    MinContent,
    MaxContent,
    Wrap(u64),
}

impl From<TextConstraint> for ConstraintKey {
    fn from(constraint: TextConstraint) -> Self {
        match constraint {
            TextConstraint::MinContent => Self::MinContent,
            TextConstraint::MaxContent => Self::MaxContent,
            TextConstraint::Wrap(width) => Self::Wrap(width.0.to_bits()),
        }
    }
}

#[derive(Clone, Debug)]
struct ParagraphCache {
    last_used: u64,
    preparation: ParagraphPreparationId,
    preflight_key: ParagraphPreflightKey,
    formation_key: FormationKey,
    paint_runs: Vec<PaintRun>,
    segment: Arc<ParagraphSceneSegment>,
    accounted_bytes: usize,
}

impl ParagraphCache {
    fn calculate_accounted_owned_bytes(&self) -> usize {
        size_of::<Self>()
            .saturating_add(vec_bytes::<ComputedInlineStyle>(
                self.preflight_key.source_styles.capacity(),
            ))
            .saturating_add(self.formation_key.accounted_owned_bytes())
            .saturating_add(vec_bytes::<PaintRun>(self.paint_runs.capacity()))
            .saturating_add(
                self.segment
                    .region_transcript
                    .as_ref()
                    .map_or(0, |transcript| {
                        vec_bytes::<crate::RegionAttempt>(transcript.attempts().len())
                    }),
            )
            .saturating_add(self.segment.geometry.accounted_owned_bytes())
    }
}

impl FormationKey {
    fn accounted_owned_bytes(&self) -> usize {
        self.text
            .capacity()
            .saturating_add(vec_bytes::<ProjectionSourceKey>(self.source_map.capacity()))
            .saturating_add(vec_bytes::<AnalysisStyle>(self.analysis_styles.capacity()))
            .saturating_add(vec_bytes::<AnalysisRun>(self.analysis_runs.capacity()))
            .saturating_add(vec_bytes::<ShapingStyle>(self.shaping_styles.capacity()))
            .saturating_add(vec_bytes::<ShapingRun>(self.shaping_runs.capacity()))
            .saturating_add(vec_bytes::<InlineFlowStyle>(
                self.inline_flow_styles.capacity(),
            ))
            .saturating_add(vec_bytes::<InlineFlowRun>(self.inline_flow_runs.capacity()))
    }
}

const fn vec_bytes<T>(capacity: usize) -> usize {
    size_of::<T>().saturating_mul(capacity)
}

fn option_ref_eq<T: PartialEq>(left: Option<&T>, right: Option<&T>) -> bool {
    match (left, right) {
        (Some(left), Some(right)) => left == right,
        (None, None) => true,
        _ => false,
    }
}

fn region_provenance_matches(left: Option<&RegionFlow>, right: Option<&RegionFlow>) -> bool {
    match (left, right) {
        (Some(left), Some(right)) => left.shares_backing_with(right),
        (None, None) => true,
        _ => false,
    }
}

fn region_output_matches(
    paragraph: &PreparedParagraph,
    transcript: &RegionTranscript,
    projection: &Projection<'_>,
) -> bool {
    if transcript.attempts().iter().any(|attempt| {
        attempt.paragraph() != paragraph.paragraph()
            || projection
                .mapping
                .text()
                .get(attempt.source().start as usize..attempt.source().end as usize)
                .is_none()
    }) {
        return false;
    }
    if paragraph.lines().is_empty() {
        let mut accepted = transcript
            .attempts()
            .iter()
            .filter(|attempt| attempt.outcome() == RegionAttemptOutcome::Accepted);
        return projection.mapping.text().is_empty()
            && accepted.next().is_some_and(|attempt| {
                attempt.source().is_empty()
                    && attempt.line_height() == projection.empty_line_height()
            })
            && accepted.next().is_none();
    }
    let mut accepted = transcript
        .attempts()
        .iter()
        .filter(|attempt| attempt.outcome() == RegionAttemptOutcome::Accepted);
    paragraph.lines().iter().all(|line| {
        accepted.next().is_some_and(|attempt| {
            line.source() == attempt.source()
                && line.slot() == Some(attempt.slot())
                && line.height() == attempt.line_height()
        })
    }) && accepted.next().is_none()
}
