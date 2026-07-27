// Copyright 2026 the Underwood Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Layout orchestration and coordinated retained-cache policy.
//!
//! This module owns cache identity, lifetime, and preparation sequencing; it
//! explicitly does not own semantic projection or geometry construction.

use super::residency::paragraph_bytes;
use super::*;
use core::mem::size_of;

/// Independent retained-geometry limits for committed and composition lanes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CacheBudget {
    max_entries: usize,
    max_composition_entries: usize,
    shared_preparation_bytes: usize,
    adapter_facts_bytes: usize,
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
            adapter_facts_bytes: 0,
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

    /// Returns a budget that may retain this many bytes of identity-local
    /// paragraph-adapter facts.
    ///
    /// This budget is independent from published scene geometry and
    /// cross-identity shared preparation. The default is zero: outputs remain
    /// valid, but a later edit or capability upgrade may need cold formation.
    #[must_use]
    pub const fn with_adapter_facts_bytes(mut self, bytes: usize) -> Self {
        self.adapter_facts_bytes = bytes;
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

    /// Returns the deterministic byte budget for identity-local adapter facts.
    #[must_use]
    pub const fn adapter_facts_bytes(self) -> usize {
        self.adapter_facts_bytes
    }
}

/// Snapshot of coordinated retained-cache state and cumulative activity.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CacheDiagnostics {
    budget: usize,
    composition_budget: usize,
    committed_entries: usize,
    composition_entries: usize,
    adapter_facts: Option<ParagraphFormationCacheDiagnostics>,
    scene_cache_accounted_bytes: usize,
    scene_cache_residency: SceneResidencyBytes,
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

    /// Returns retained paragraph-adapter facts, when the backend reports
    /// deterministic accounting.
    #[must_use]
    pub const fn adapter_facts(self) -> Option<ParagraphFormationCacheDiagnostics> {
        self.adapter_facts
    }

    /// Returns the deterministic capacity charge for retained scene-cache data.
    ///
    /// Shared font blobs, backend-private storage, and allocator overhead are
    /// deliberately excluded.
    #[must_use]
    pub const fn scene_cache_accounted_bytes(self) -> usize {
        self.scene_cache_accounted_bytes
    }

    /// Returns capability-category charges for retained scene segments.
    ///
    /// This is the scene-output subset of [`Self::scene_cache_accounted_bytes`];
    /// cache keys and lookup metadata are included only in the latter.
    #[must_use]
    pub const fn scene_cache_residency(self) -> SceneResidencyBytes {
        self.scene_cache_residency
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
    region_segments: Vec<Arc<ParagraphSceneSegment>>,
}

impl PrepareScratch {
    fn accounted_capacity_bytes(&self) -> usize {
        vec_bytes::<Arc<ParagraphSceneSegment>>(self.region_segments.capacity())
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
    last_used: u64,
    required_paint_slots: usize,
    core: Arc<SceneCore>,
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
    target: ParagraphId,
    last_used: u64,
    required_paint_slots: usize,
    core: Arc<SceneCore>,
    region_attempts: usize,
    region_height_rejections: usize,
}

#[derive(Clone, Debug)]
struct PublishedBlock {
    snapshot: TextBlockSnapshot,
    last_used: u64,
    core: Arc<SceneCore>,
    region_attempts: usize,
    region_height_rejections: usize,
}

/// Mutable owner of one paragraph adapter and its retained stage caches.
pub struct LayoutEngine {
    paragraphs: Box<dyn ParagraphFormation>,
    cache: ParagraphCacheStore,
    composition_cache: ParagraphCacheStore,
    clock: u64,
    budget: CacheBudget,
    cache_work: CacheWork,
    shared_preparation: SharedPreparationCache,
    scratch: PrepareScratch,
    published: BTreeMap<crate::DocumentId, PublishedScene>,
    published_compositions: BTreeMap<crate::DocumentId, PublishedComposition>,
    published_blocks: BTreeMap<crate::DocumentId, PublishedBlock>,
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
        let mut paragraphs: Box<dyn ParagraphFormation> = Box::new(paragraphs);
        paragraphs.set_retained_facts_budget(budget.adapter_facts_bytes);
        Self {
            paragraphs,
            cache: ParagraphCacheStore::default(),
            composition_cache: ParagraphCacheStore::default(),
            clock: 0,
            budget,
            cache_work: CacheWork::default(),
            shared_preparation: SharedPreparationCache::new(budget.shared_preparation_bytes),
            scratch: PrepareScratch::default(),
            published: BTreeMap::new(),
            published_compositions: BTreeMap::new(),
            published_blocks: BTreeMap::new(),
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
        if let Some(output) = self.prepare_appended_scene(snapshot, request)? {
            return Ok(output);
        }
        if let Some(output) = self.prepare_localized_normal_flow(snapshot, request)? {
            return Ok(output);
        }
        if let Some(output) = self.prepare_localized_region_flow(snapshot, request)? {
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
        self.scratch.region_segments.clear();
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
                let preflight_key =
                    ParagraphPreflightKey::new(paragraph, None, request, region_cursor);
                prepare_paragraph_geometry(
                    self.paragraphs.as_mut(),
                    &mut self.cache,
                    self.composition_cache.get(&paragraph.id),
                    CacheKind::Committed,
                    paragraph,
                    &projection,
                    request.features.features_for(paragraph.id),
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
            self.record_access(&access);
            if let Some(transcript) = &access.region_transcript {
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
        let region = scene_region_binding(
            summary,
            snapshot.paragraphs().len(),
            request.region_flow,
            region_start,
            region_cursor,
        )?;
        let region_attempts = region.map_or(0, |region| region.attempts);
        let region_height_rejections = region.map_or(0, |region| region.height_rejections);
        let trace = request.trace.then(|| {
            Arc::new(PreparationTrace {
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
            })
        });
        let core = Arc::new(SceneCore {
            paragraph_count: snapshot.paragraphs().len(),
            resident: resident_feature_policy(&spine, request.features.default_features()),
            spine,
            metrics,
            region,
        });
        let output = SceneOutput {
            scene: TextScene {
                document: snapshot.id(),
                revision: snapshot.revision(),
                paint: request.paint.clone(),
                requested: request.features.clone(),
                core: Arc::clone(&core),
            },
            work,
            trace,
        };
        if snapshot
            .paragraphs()
            .iter()
            .all(|paragraph| self.cache.contains_key(&paragraph.id))
        {
            self.clock = self.clock.saturating_add(1);
            self.published.insert(
                snapshot.id(),
                PublishedScene {
                    snapshot: snapshot.clone(),
                    styles: request.styles.clone(),
                    constraint: ConstraintKey::from(request.constraint),
                    region_flow: request.region_flow.cloned(),
                    last_used: self.clock,
                    required_paint_slots,
                    core,
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
        if let Some(output) = self.reuse_published_block(snapshot, request) {
            return Ok(output);
        }
        let styles = StyleMap::new(request.style.clone())
            .with_default_paragraph_style(request.paragraph_style);
        let scene_request = match request.region_flow {
            Some(flow) => {
                SceneRequest::new(request.constraint, &styles, request.paint).with_region_flow(flow)
            }
            None => SceneRequest::new(request.constraint, &styles, request.paint),
        }
        .with_features(request.features);
        let scene_request = if request.trace {
            scene_request.with_preparation_trace()
        } else {
            scene_request
        };
        let document = snapshot.materialize_document();
        let output = self.prepare(&document.snapshot(), &scene_request)?;
        if let Some(published) = self.published.remove(&snapshot.id()) {
            self.published_blocks.insert(
                snapshot.id(),
                PublishedBlock {
                    snapshot: snapshot.clone(),
                    last_used: published.last_used,
                    core: Arc::clone(&published.core),
                    region_attempts: published.region_attempts,
                    region_height_rejections: published.region_height_rejections,
                },
            );
        } else {
            self.published_blocks.remove(&snapshot.id());
        }
        Ok(output)
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
        let target_paragraph = usize::try_from(target.paragraph)
            .ok()
            .and_then(|index| snapshot.paragraphs().get(index))
            .filter(|paragraph| paragraph.id.index == target.paragraph)
            .map(|paragraph| paragraph.id)
            .filter(|_| snapshot.text(target).is_some())
            .ok_or_else(|| {
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
        self.scratch.region_segments.clear();
        let mut work = WorkReport::default();
        let mut reuse = PreparationReuse::default();
        let region_start = request.region_flow.map(RegionFlow::cursor);
        let mut region_cursor = region_start;

        for (paragraph_index, paragraph) in snapshot.paragraphs().iter().enumerate() {
            let transient = paragraph.id == target_paragraph;
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
                let preflight_key = ParagraphPreflightKey::new(
                    paragraph,
                    transient.then(|| {
                        Arc::new(CompositionPreparationKey::new(
                            composition.id(),
                            projection.mapping.text(),
                        ))
                    }),
                    request,
                    region_cursor,
                );
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
                            request
                                .features
                                .features_for(paragraph.id)
                                .with_native_text_input(),
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
                            request.features.features_for(paragraph.id),
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
            self.record_access(&access);
            if let Some(transcript) = &access.region_transcript {
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
        let region = scene_region_binding(
            summary,
            snapshot.paragraphs().len(),
            request.region_flow,
            region_start,
            region_cursor,
        )?;
        let region_attempts = region.map_or(0, |region| region.attempts);
        let region_height_rejections = region.map_or(0, |region| region.height_rejections);
        let trace = request.trace.then(|| {
            Arc::new(PreparationTrace {
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
            })
        });
        let effective_features = request.features.clone().with_paragraph(
            target_paragraph,
            request
                .features
                .features_for(target_paragraph)
                .with_native_text_input(),
        );
        let core = Arc::new(SceneCore {
            paragraph_count: snapshot.paragraphs().len(),
            resident: resident_feature_policy(&spine, effective_features.default_features()),
            spine,
            metrics,
            region,
        });
        let output = CompositionSceneOutput {
            scene: CompositionScene {
                document: snapshot.id(),
                revision: snapshot.revision(),
                composition: composition.id(),
                epoch: composition.epoch(),
                paint: request.paint.clone(),
                requested: effective_features,
                core: Arc::clone(&core),
            },
            work,
            trace,
        };
        if snapshot.paragraphs().iter().all(|paragraph| {
            if paragraph.id == target_paragraph {
                self.composition_cache.contains_key(&paragraph.id)
            } else {
                self.cache.contains_key(&paragraph.id)
            }
        }) {
            self.clock = self.clock.saturating_add(1);
            self.published_compositions.insert(
                snapshot.id(),
                PublishedComposition {
                    snapshot: snapshot.clone(),
                    styles: request.styles.clone(),
                    constraint: ConstraintKey::from(request.constraint),
                    region_flow: request.region_flow.cloned(),
                    composition: composition.clone(),
                    target: target_paragraph,
                    last_used: self.clock,
                    required_paint_slots,
                    core,
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
        self.published_blocks.remove(&document);
        let mut preparations = Vec::new();
        for cache in [&mut self.cache, &mut self.composition_cache] {
            cache.retain(|paragraph, entry| {
                if paragraph.document != document {
                    return true;
                }
                preparations.push(entry.preparation);
                self.cache_work.scene_cache_accounted_bytes = self
                    .cache_work
                    .scene_cache_accounted_bytes
                    .saturating_sub(entry.accounted_bytes);
                self.cache_work.releases = self.cache_work.releases.saturating_add(1);
                false
            });
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
        self.published.clear();
        self.published_compositions.clear();
        self.published_blocks.clear();
        self.shared_preparation.clear();
        self.paragraphs.clear();
        self.cache_work.scene_cache_accounted_bytes = 0;
    }

    /// Drops reusable adapter facts without invalidating retained or
    /// caller-owned scenes.
    ///
    /// A later edit or capability upgrade may perform cold paragraph
    /// formation and reports that work normally.
    pub fn trim_adapter_facts(&mut self) {
        self.paragraphs.trim_retained_facts();
    }

    /// Returns a snapshot of coordinated cache state and cumulative activity.
    #[must_use]
    pub fn cache_diagnostics(&self) -> CacheDiagnostics {
        let shared = self.shared_preparation.diagnostics();
        let mut scene_cache_residency = SceneResidencyBytes::default();
        for entry in self.cache.values().chain(self.composition_cache.values()) {
            scene_cache_residency.add_assign(paragraph_bytes(&entry.segment));
        }
        CacheDiagnostics {
            budget: self.budget.max_entries,
            composition_budget: self.budget.max_composition_entries,
            committed_entries: self.cache.len(),
            composition_entries: self.composition_cache.len(),
            adapter_facts: self.paragraphs.retained_facts(),
            scene_cache_accounted_bytes: self.cache_work.scene_cache_accounted_bytes,
            scene_cache_residency,
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
        &mut self,
        snapshot: &DocumentSnapshot,
        request: &SceneRequest<'_>,
    ) -> Option<SceneOutput> {
        let published = self.published.get(&snapshot.id())?;
        if !published.snapshot.shares_state_with(snapshot)
            || !published.styles.shares_state_with(request.styles)
            || !published.core.resident.contains_policy(&request.features)
            || published.constraint != ConstraintKey::from(request.constraint)
            || !region_provenance_matches(published.region_flow.as_ref(), request.region_flow)
            || request.paint.len() < published.required_paint_slots
        {
            return None;
        }
        self.clock = self.clock.saturating_add(1);
        self.published
            .get_mut(&snapshot.id())
            .expect("the validated published scene remains present")
            .last_used = self.clock;
        let published = self
            .published
            .get(&snapshot.id())
            .expect("the refreshed published scene remains present");
        let paragraph_count = published.core.paragraph_count;
        let work = WorkReport {
            reused_paragraphs: paragraph_count,
            ..WorkReport::default()
        };
        let trace = request.trace.then(|| {
            let diagnostics = self.cache_diagnostics();
            Arc::new(PreparationTrace {
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
            })
        });
        Some(SceneOutput {
            scene: TextScene {
                document: snapshot.id(),
                revision: snapshot.revision(),
                paint: request.paint.clone(),
                requested: request.features.clone(),
                core: Arc::clone(&published.core),
            },
            work,
            trace,
        })
    }

    fn reuse_published_block(
        &mut self,
        snapshot: &TextBlockSnapshot,
        request: &BlockRequest<'_>,
    ) -> Option<SceneOutput> {
        let paragraph = snapshot.paragraph_id();
        let published = self.published_blocks.get(&snapshot.id())?;
        let cache = self.cache.get(&paragraph)?;
        let preflight = &cache.preflight_key;
        let region_cursor = request.region_flow.map(RegionFlow::cursor);
        if !published.snapshot.shares_state_with(snapshot)
            || published.core.paragraph_count != 1
            || !cache.segment.geometry.features.contains(request.features)
            || preflight.version != snapshot.revision().0
            || preflight.styles.default_style() != request.style
            || preflight.styles.style_for(snapshot.text_id()) != request.style
            || preflight.styles.paragraph_style_for(paragraph) != request.paragraph_style
            || preflight.constraint != ConstraintKey::from(request.constraint)
            || preflight.region_cursor != region_cursor
            || !region_provenance_matches(preflight.region_flow.as_ref(), request.region_flow)
            || request.paint.brush(request.style.paint()).is_none()
        {
            return None;
        }

        self.clock = self.clock.saturating_add(1);
        self.published_blocks
            .get_mut(&snapshot.id())
            .expect("the validated block root remains present")
            .last_used = self.clock;
        let published = self
            .published_blocks
            .get(&snapshot.id())
            .expect("the refreshed block root remains present");
        let work = WorkReport {
            reused_paragraphs: 1,
            ..WorkReport::default()
        };
        let trace = request.trace.then(|| {
            let diagnostics = self.cache_diagnostics();
            Arc::new(PreparationTrace {
                work: work.clone(),
                reuse: PreparationReuse {
                    paragraphs: 1,
                    preflight_reuses: 1,
                    exact_geometry_reuses: 1,
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
            })
        });
        Some(SceneOutput {
            scene: TextScene {
                document: snapshot.id(),
                revision: snapshot.revision(),
                paint: request.paint.clone(),
                requested: SceneFeaturePolicy::uniform(request.features),
                core: Arc::clone(&published.core),
            },
            work,
            trace,
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
        if published.constraint != ConstraintKey::from(request.constraint)
            || published.region_flow.is_some()
            || request.region_flow.is_some()
            || !published.core.resident.contains_policy(&request.features)
            || published.core.paragraph_count != snapshot.paragraphs().len()
            || request.paint.len() < published.required_paint_slots
        {
            return Ok(None);
        }
        let previous = published.snapshot.clone();
        let previous_styles = published.styles.clone();
        let previous_core = Arc::clone(&published.core);
        let mut changed: Vec<_> = snapshot
            .changed_paragraphs_from(&previous)
            .ok_or_else(|| SceneError::for_document(SceneErrorKind::SourceCoverage, snapshot.id()))?
            .collect();
        let Some(style_changes) = request
            .styles
            .changed_paragraphs_from(&previous_styles, snapshot.id())
        else {
            return Ok(None);
        };
        changed.extend(style_changes);
        changed.sort_unstable();
        changed.dedup();
        if changed.iter().copied().any(|index| {
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
        let cache_before = request.trace.then(|| self.cache_diagnostics());
        let scratch_capacity_before = request
            .trace
            .then(|| self.scratch.accounted_capacity_bytes());
        let mut work = WorkReport::default();
        let mut reuse = PreparationReuse::default();
        let mut changed_count = 0_usize;
        let mut paint_records = 0_usize;
        let mut required_paint_slots = published.required_paint_slots;

        for paragraph_index in changed {
            let paragraph = snapshot
                .paragraphs()
                .get(paragraph_index)
                .expect("the structural diff yields an existing paragraph");
            required_paint_slots = required_paint_slots.max(validate_paragraph_styles(
                paragraph,
                request.styles,
                request.paint,
            )?);
            self.clock = self.clock.saturating_add(1);
            let projection = Projection::new(paragraph, request)?;
            let preflight_key = ParagraphPreflightKey::new(paragraph, None, request, None);
            let access = prepare_paragraph_geometry(
                self.paragraphs.as_mut(),
                &mut self.cache,
                self.composition_cache.get(&paragraph.id),
                CacheKind::Committed,
                paragraph,
                &projection,
                request.features.features_for(paragraph.id),
                preflight_key,
                request.constraint,
                None,
                None,
                self.clock,
                &mut self.shared_preparation,
                &mut work,
                &mut reuse,
            )?;
            self.record_access(&access);
            let segment = Arc::clone(
                &self
                    .cache
                    .get(&paragraph.id)
                    .expect("localized preparation retains its paragraph segment")
                    .segment,
            );
            paint_records = paint_records.saturating_add(segment.paint.fragments.len());
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
        let trace = request.trace.then(|| {
            Arc::new(PreparationTrace {
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
            })
        });
        let core = if changed_count == 0 {
            previous_core
        } else {
            Arc::new(SceneCore {
                paragraph_count,
                metrics: TextMetrics::from_summary(summary),
                resident: resident_feature_policy(&spine, request.features.default_features()),
                spine,
                region: None,
            })
        };
        let output = SceneOutput {
            scene: TextScene {
                document: snapshot.id(),
                revision: snapshot.revision(),
                paint: request.paint.clone(),
                requested: request.features.clone(),
                core: Arc::clone(&core),
            },
            work,
            trace,
        };
        self.clock = self.clock.saturating_add(1);
        self.published.insert(
            snapshot.id(),
            PublishedScene {
                snapshot: snapshot.clone(),
                styles: request.styles.clone(),
                constraint: ConstraintKey::from(request.constraint),
                region_flow: None,
                last_used: self.clock,
                required_paint_slots,
                core,
                region_attempts: 0,
                region_height_rejections: 0,
            },
        );
        Ok(Some(output))
    }

    fn prepare_appended_scene(
        &mut self,
        snapshot: &DocumentSnapshot,
        request: &SceneRequest<'_>,
    ) -> Result<Option<SceneOutput>, SceneError> {
        let Some(published) = self.published.get(&snapshot.id()) else {
            return Ok(None);
        };
        if !published.core.resident.contains_policy(&request.features) {
            return Ok(None);
        }
        if published.constraint != ConstraintKey::from(request.constraint)
            || !region_provenance_matches(published.region_flow.as_ref(), request.region_flow)
            || request.paint.len() < published.required_paint_slots
        {
            return Ok(None);
        }
        let previous = published.snapshot.clone();
        let previous_styles = published.styles.clone();
        let previous_core = Arc::clone(&published.core);
        let mut required_paint_slots = published.required_paint_slots;
        let Some(appended) = snapshot.appended_paragraphs_from(&previous) else {
            return Ok(None);
        };
        if appended.is_empty() {
            return Ok(None);
        }
        let Some(style_changes) = request
            .styles
            .changed_paragraphs_from(&previous_styles, snapshot.id())
        else {
            return Ok(None);
        };
        if style_changes.iter().any(|index| !appended.contains(index)) {
            return Ok(None);
        }

        let cache_before = request.trace.then(|| self.cache_diagnostics());
        let scratch_capacity_before = request
            .trace
            .then(|| self.scratch.accounted_capacity_bytes());
        let previous_count = previous.paragraphs().len();
        let mut spine = previous_core.spine.clone();
        let mut work = WorkReport {
            reused_paragraphs: previous_count,
            ..WorkReport::default()
        };
        let mut reuse = PreparationReuse {
            paragraphs: previous_count,
            preflight_reuses: previous_count,
            exact_geometry_reuses: previous_count,
            ..PreparationReuse::default()
        };
        let region_start = request.region_flow.map(RegionFlow::cursor);
        let mut region_cursor = match (request.region_flow, previous_count) {
            (Some(flow), 0) => Some(flow.cursor()),
            (Some(_), _) => previous_core.region.map(|region| region.end),
            (None, _) => None,
        };
        let mut paint_records = 0_usize;

        for paragraph_index in appended.clone() {
            let paragraph = snapshot
                .paragraphs()
                .get(paragraph_index)
                .expect("an appended range names a represented paragraph");
            required_paint_slots = required_paint_slots.max(validate_paragraph_styles(
                paragraph,
                request.styles,
                request.paint,
            )?);
            self.clock = self.clock.saturating_add(1);
            let projection = Projection::new(paragraph, request)?;
            let preflight_key = ParagraphPreflightKey::new(paragraph, None, request, region_cursor);
            let access = prepare_paragraph_geometry(
                self.paragraphs.as_mut(),
                &mut self.cache,
                self.composition_cache.get(&paragraph.id),
                CacheKind::Committed,
                paragraph,
                &projection,
                request.features.features_for(paragraph.id),
                preflight_key,
                request.constraint,
                request.region_flow,
                region_cursor,
                self.clock,
                &mut self.shared_preparation,
                &mut work,
                &mut reuse,
            )?;
            self.record_access(&access);
            if let Some(transcript) = &access.region_transcript {
                region_cursor = Some(transcript.end());
            }
            let segment = Arc::clone(
                &self
                    .cache
                    .get(&paragraph.id)
                    .expect("appended preparation retains its paragraph segment")
                    .segment,
            );
            paint_records = paint_records.saturating_add(segment.paint.fragments.len());
            spine = spine.append(segment);
            self.enforce_budget();
        }

        let summary = spine.summary();
        work.paint = StageWork {
            paragraphs: appended.len(),
            records: paint_records,
        };
        let region = scene_region_binding(
            summary,
            snapshot.paragraphs().len(),
            request.region_flow,
            region_start,
            region_cursor,
        )?;
        let region_attempts = region.map_or(0, |region| region.attempts);
        let region_height_rejections = region.map_or(0, |region| region.height_rejections);
        let trace = request.trace.then(|| {
            Arc::new(PreparationTrace {
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
                region_attempts,
                region_height_rejections,
            })
        });
        let core = Arc::new(SceneCore {
            paragraph_count: snapshot.paragraphs().len(),
            metrics: TextMetrics::from_summary(summary),
            resident: resident_feature_policy(&spine, request.features.default_features()),
            spine,
            region,
        });
        let output = SceneOutput {
            scene: TextScene {
                document: snapshot.id(),
                revision: snapshot.revision(),
                paint: request.paint.clone(),
                requested: request.features.clone(),
                core: Arc::clone(&core),
            },
            work,
            trace,
        };
        self.clock = self.clock.saturating_add(1);
        self.published.insert(
            snapshot.id(),
            PublishedScene {
                snapshot: snapshot.clone(),
                styles: request.styles.clone(),
                constraint: ConstraintKey::from(request.constraint),
                region_flow: request.region_flow.cloned(),
                last_used: self.clock,
                required_paint_slots,
                core,
                region_attempts,
                region_height_rejections,
            },
        );
        Ok(Some(output))
    }

    fn prepare_localized_region_flow(
        &mut self,
        snapshot: &DocumentSnapshot,
        request: &SceneRequest<'_>,
    ) -> Result<Option<SceneOutput>, SceneError> {
        let Some(published) = self.published.get(&snapshot.id()) else {
            return Ok(None);
        };
        if !published.core.resident.contains_policy(&request.features) {
            return Ok(None);
        }
        let Some(region_flow) = request.region_flow else {
            return Ok(None);
        };
        if published.constraint != ConstraintKey::from(request.constraint)
            || !region_provenance_matches(published.region_flow.as_ref(), Some(region_flow))
            || published.core.paragraph_count != snapshot.paragraphs().len()
            || request.paint.len() < published.required_paint_slots
        {
            return Ok(None);
        }

        let previous = published.snapshot.clone();
        let previous_styles = published.styles.clone();
        let previous_core = Arc::clone(&published.core);
        let mut required_paint_slots = published.required_paint_slots;
        let mut changed: Vec<_> = snapshot
            .changed_paragraphs_from(&previous)
            .ok_or_else(|| SceneError::for_document(SceneErrorKind::SourceCoverage, snapshot.id()))?
            .collect();
        let Some(style_changes) = request
            .styles
            .changed_paragraphs_from(&previous_styles, snapshot.id())
        else {
            return Ok(None);
        };
        changed.extend(style_changes);
        changed.sort_unstable();
        changed.dedup();
        if changed.iter().copied().any(|index| {
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
        for &index in &changed {
            let paragraph = snapshot
                .paragraphs()
                .get(index)
                .expect("validated style change names a represented paragraph");
            required_paint_slots = required_paint_slots.max(validate_paragraph_styles(
                paragraph,
                request.styles,
                request.paint,
            )?);
        }

        let cache_before = request.trace.then(|| self.cache_diagnostics());
        let scratch_capacity_before = request
            .trace
            .then(|| self.scratch.accounted_capacity_bytes());
        let mut spine = previous_core.spine.clone();
        let mut work = WorkReport::default();
        let mut reuse = PreparationReuse::default();
        let mut processed = 0_usize;
        let mut paint_records = 0_usize;
        let mut changed = changed.into_iter().peekable();

        while let Some(run_start) = changed.next() {
            let mut region_cursor = if run_start == 0 {
                region_flow.cursor()
            } else {
                spine
                    .segment(run_start - 1)
                    .and_then(|segment| segment.region_transcript.as_ref())
                    .map(RegionTranscript::end)
                    .ok_or_else(|| SceneError::for_document(SceneErrorKind::Flow, snapshot.id()))?
            };
            let mut paragraph_index = run_start;
            self.scratch.region_segments.clear();

            loop {
                let paragraph = snapshot
                    .paragraphs()
                    .get(paragraph_index)
                    .expect("a structural diff index remains in bounds");
                let additionally_changed = paragraph_index != run_start
                    && changed.peek().copied() == Some(paragraph_index);
                let structurally_changed = paragraph_index == run_start || additionally_changed;
                if additionally_changed {
                    changed.next();
                }

                if !structurally_changed
                    && self.cache.get(&paragraph.id).is_some_and(|entry| {
                        entry
                            .preflight_key
                            .matches(paragraph, request, Some(region_cursor))
                    })
                {
                    break;
                }

                self.clock = self.clock.saturating_add(1);
                let projection = Projection::new(paragraph, request)?;
                let preflight_key =
                    ParagraphPreflightKey::new(paragraph, None, request, Some(region_cursor));
                let access = prepare_paragraph_geometry(
                    self.paragraphs.as_mut(),
                    &mut self.cache,
                    self.composition_cache.get(&paragraph.id),
                    CacheKind::Committed,
                    paragraph,
                    &projection,
                    request.features.features_for(paragraph.id),
                    preflight_key,
                    request.constraint,
                    Some(region_flow),
                    Some(region_cursor),
                    self.clock,
                    &mut self.shared_preparation,
                    &mut work,
                    &mut reuse,
                )?;
                self.record_access(&access);
                region_cursor = access
                    .region_transcript
                    .as_ref()
                    .map(RegionTranscript::end)
                    .ok_or_else(|| SceneError::for_paragraph(SceneErrorKind::Flow, paragraph.id))?;
                let segment = Arc::clone(
                    &self
                        .cache
                        .get(&paragraph.id)
                        .expect("localized region preparation retains its segment")
                        .segment,
                );
                paint_records = paint_records.saturating_add(segment.paint.fragments.len());
                self.scratch.region_segments.push(segment);
                processed = processed.saturating_add(1);
                self.enforce_budget();
                paragraph_index = paragraph_index.saturating_add(1);
                if paragraph_index == snapshot.paragraphs().len() {
                    break;
                }
            }

            spine = spine
                .replace_range(run_start, &self.scratch.region_segments)
                .expect("the retained spine has the same paragraph count");
        }

        let paragraph_count = snapshot.paragraphs().len();
        let unchanged = paragraph_count.saturating_sub(processed);
        work.reused_paragraphs = work.reused_paragraphs.saturating_add(unchanged);
        reuse.paragraphs = reuse.paragraphs.saturating_add(unchanged);
        reuse.preflight_reuses = reuse.preflight_reuses.saturating_add(unchanged);
        reuse.exact_geometry_reuses = reuse.exact_geometry_reuses.saturating_add(unchanged);
        work.paint = StageWork {
            paragraphs: processed,
            records: paint_records,
        };
        let summary = spine.summary();
        let region_end = if paragraph_count == 0 {
            Some(region_flow.cursor())
        } else {
            summary.region_end
        };
        let region = scene_region_binding(
            summary,
            paragraph_count,
            Some(region_flow),
            Some(region_flow.cursor()),
            region_end,
        )?;
        let region_attempts = region.map_or(0, |region| region.attempts);
        let region_height_rejections = region.map_or(0, |region| region.height_rejections);
        let trace = request.trace.then(|| {
            Arc::new(PreparationTrace {
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
                region_attempts,
                region_height_rejections,
            })
        });
        let core = if processed == 0 {
            previous_core
        } else {
            Arc::new(SceneCore {
                paragraph_count,
                metrics: TextMetrics::from_summary(summary),
                resident: resident_feature_policy(&spine, request.features.default_features()),
                spine,
                region,
            })
        };
        let output = SceneOutput {
            scene: TextScene {
                document: snapshot.id(),
                revision: snapshot.revision(),
                paint: request.paint.clone(),
                requested: request.features.clone(),
                core: Arc::clone(&core),
            },
            work,
            trace,
        };
        self.clock = self.clock.saturating_add(1);
        self.published.insert(
            snapshot.id(),
            PublishedScene {
                snapshot: snapshot.clone(),
                styles: request.styles.clone(),
                constraint: ConstraintKey::from(request.constraint),
                region_flow: Some(region_flow.clone()),
                last_used: self.clock,
                required_paint_slots,
                core,
                region_attempts,
                region_height_rejections,
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
            && published.core.resident.contains_policy(&request.features)
            && published.core.paragraph_count == snapshot.paragraphs().len())
        .then(|| published.core.spine.clone())
    }

    fn reuse_published_composition(
        &mut self,
        snapshot: &DocumentSnapshot,
        request: &SceneRequest<'_>,
        composition: &CompositionSession,
    ) -> Option<CompositionSceneOutput> {
        let published = self.published_compositions.get(&snapshot.id())?;
        let effective_features = effective_composition_features(request, published.target);
        if !published.snapshot.shares_state_with(snapshot)
            || !published.styles.shares_state_with(request.styles)
            || !published.core.resident.contains_policy(&effective_features)
            || published.constraint != ConstraintKey::from(request.constraint)
            || !region_provenance_matches(published.region_flow.as_ref(), request.region_flow)
            || !published.composition.shares_state_with(composition)
            || request.paint.len() < published.required_paint_slots
        {
            return None;
        }
        self.clock = self.clock.saturating_add(1);
        self.published_compositions
            .get_mut(&snapshot.id())
            .expect("the validated published composition remains present")
            .last_used = self.clock;
        let published = self
            .published_compositions
            .get(&snapshot.id())
            .expect("the refreshed published composition remains present");
        let paragraph_count = published.core.paragraph_count;
        let work = WorkReport {
            reused_paragraphs: paragraph_count,
            ..WorkReport::default()
        };
        let trace = request.trace.then(|| {
            let diagnostics = self.cache_diagnostics();
            Arc::new(PreparationTrace {
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
            })
        });
        Some(CompositionSceneOutput {
            scene: CompositionScene {
                document: snapshot.id(),
                revision: snapshot.revision(),
                composition: composition.id(),
                epoch: composition.epoch(),
                paint: request.paint.clone(),
                requested: effective_features,
                core: Arc::clone(&published.core),
            },
            work,
            trace,
        })
    }

    fn reusable_composition_spine(
        &self,
        snapshot: &DocumentSnapshot,
        request: &SceneRequest<'_>,
    ) -> Option<SceneSpine> {
        let target = self
            .published_compositions
            .get(&snapshot.id())
            .map(|published| published.target);
        self.published_compositions
            .get(&snapshot.id())
            .filter(|published| {
                let effective_features = effective_composition_features(request, published.target);
                published.snapshot.shares_state_with(snapshot)
                    && published.styles.shares_state_with(request.styles)
                    && published.constraint == ConstraintKey::from(request.constraint)
                    && region_provenance_matches(
                        published.region_flow.as_ref(),
                        request.region_flow,
                    )
                    && published.core.resident.contains_policy(&effective_features)
                    && published.core.paragraph_count == snapshot.paragraphs().len()
            })
            .map(|published| published.core.spine.clone())
            .or_else(|| {
                let target = target?;
                let effective_features = effective_composition_features(request, target);
                let published = self.published.get(&snapshot.id())?;
                (published.styles.shares_state_with(request.styles)
                    && published.constraint == ConstraintKey::from(request.constraint)
                    && region_provenance_matches(
                        published.region_flow.as_ref(),
                        request.region_flow,
                    )
                    && published.core.resident.contains_policy(&effective_features)
                    && published.core.paragraph_count == snapshot.paragraphs().len())
                .then(|| published.core.spine.clone())
            })
    }

    fn record_access(&mut self, access: &CacheAccess) {
        if access.previous_accounted_bytes != access.current_accounted_bytes {
            self.cache_work.scene_cache_accounted_bytes = self
                .cache_work
                .scene_cache_accounted_bytes
                .saturating_sub(access.previous_accounted_bytes)
                .saturating_add(access.current_accounted_bytes);
        }
        if access.previous_use.is_some() {
            self.cache_work.hits += 1;
        } else {
            self.cache_work.misses += 1;
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
            let cache = match kind {
                CacheKind::Committed => &self.cache,
                CacheKind::Composition => &self.composition_cache,
            };
            let Some(paragraph) = cache
                .iter()
                .map(|(paragraph, entry)| {
                    (
                        entry.last_used.max(self.root_use_for(kind, *paragraph)),
                        *paragraph,
                    )
                })
                .min()
                .map(|(_, paragraph)| paragraph)
            else {
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
                    self.published_blocks.remove(&paragraph.document);
                    if self
                        .published_compositions
                        .get(&paragraph.document)
                        .is_some_and(|published| published.target != paragraph)
                    {
                        self.published_compositions.remove(&paragraph.document);
                    }
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
            self.cache_work.evictions += 1;
        }
    }

    fn root_use_for(&self, kind: CacheKind, paragraph: ParagraphId) -> u64 {
        let committed = self
            .published
            .get(&paragraph.document)
            .map_or(0, |published| published.last_used);
        let committed = self
            .published_blocks
            .get(&paragraph.document)
            .map_or(committed, |published| committed.max(published.last_used));
        let composition = self.published_compositions.get(&paragraph.document);
        match kind {
            CacheKind::Committed => composition
                .filter(|published| published.target != paragraph)
                .map_or(committed, |published| committed.max(published.last_used)),
            CacheKind::Composition => composition
                .filter(|published| published.target == paragraph)
                .map_or(0, |published| published.last_used),
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

fn validate_paragraph_styles(
    paragraph: &Paragraph,
    styles: &StyleMap,
    paint: &PaintTable,
) -> Result<usize, SceneError> {
    let represented_overrides = paragraph
        .leaves
        .iter()
        .filter(|leaf| styles.style_override(leaf.id).is_some())
        .count();
    if represented_overrides != styles.inline_override_count_for(paragraph.id) {
        return Err(SceneError::for_paragraph(
            SceneErrorKind::InvalidStyle,
            paragraph.id,
        ));
    }
    let mut required = 0_usize;
    for leaf in &paragraph.leaves {
        let slot = styles.style_for(leaf.id).paint();
        if paint.brush(slot).is_none() {
            return Err(SceneError::for_paragraph(
                SceneErrorKind::InvalidStyle,
                paragraph.id,
            ));
        }
        required = required.max(slot.index() as usize + 1);
    }
    Ok(required)
}

#[derive(Clone, Debug)]
struct CacheAccess {
    previous_use: Option<u64>,
    previous_accounted_bytes: usize,
    current_accounted_bytes: usize,
    region_transcript: Option<RegionTranscript>,
}

fn reuse_paragraph_geometry(
    cache: &mut ParagraphCacheStore,
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
        || !entry
            .segment
            .geometry
            .features
            .contains(request.features.features_for(paragraph.id))
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
        previous_use,
        previous_accounted_bytes: entry.accounted_bytes,
        current_accounted_bytes: entry.accounted_bytes,
        region_transcript: entry.segment.region_transcript.clone(),
    })
}

fn prepare_paragraph_geometry(
    paragraphs: &mut dyn ParagraphFormation,
    cache: &mut ParagraphCacheStore,
    alternate: Option<&ParagraphCache>,
    cache_kind: CacheKind,
    paragraph: &Paragraph,
    projection: &Projection<'_>,
    features: SceneFeatures,
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
                entry
                    .preflight_key
                    .adapter_change(&preflight_key, paragraph)
            });
    let cached = cache.contains_key(&paragraph.id);
    let formation_matches = cache.get(&paragraph.id).is_some_and(|entry| {
        entry
            .preflight_key
            .formation_matches(&preflight_key, paragraph)
    });
    let paint_matches = cache
        .get(&paragraph.id)
        .is_some_and(|entry| entry.preflight_key.paint_matches(&preflight_key, paragraph));
    let adjustment_matches = cache.get(&paragraph.id).is_some_and(|entry| {
        entry
            .preflight_key
            .adjustment_matches(&preflight_key, paragraph.id)
    });
    let retained_layout = (formation_matches && adjustment_matches).then(|| {
        Arc::clone(
            &cache
                .get(&paragraph.id)
                .expect("layout reuse requires retained geometry")
                .segment
                .geometry,
        )
    });
    let retained_paint_layout = (formation_matches
        && adjustment_matches
        && !paint_matches
        && cache
            .get(&paragraph.id)
            .is_some_and(|entry| entry.segment.geometry.features.contains(features)))
    .then(|| {
        Arc::clone(
            &cache
                .get(&paragraph.id)
                .expect("paint-only reuse requires retained geometry")
                .segment
                .geometry,
        )
    });
    let capability_upgrade = formation_matches
        && paint_matches
        && adjustment_matches
        && cache
            .get(&paragraph.id)
            .is_some_and(|entry| !entry.segment.geometry.features.contains(features));
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
    if formation_matches
        && paint_matches
        && adjustment_matches
        && cache
            .get(&paragraph.id)
            .is_some_and(|entry| entry.segment.geometry.features.contains(features))
    {
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
            previous_use,
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
            features,
        });
    let shared_hit = shared_query
        .as_ref()
        .and_then(|query| shared_preparation.lookup(query, current_use));
    let retained_artifact = (formation_matches && paint_matches)
        .then(|| cache.get(&paragraph.id))
        .flatten()
        .filter(|entry| {
            entry
                .segment
                .geometry
                .artifact
                .features()
                .contains(features)
        })
        .map(|entry| {
            (
                Arc::clone(&entry.segment.geometry.artifact),
                entry.segment.region_transcript.clone(),
            )
        })
        .or_else(|| {
            alternate
                .filter(|entry| {
                    entry
                        .preflight_key
                        .alternate_adapter_change(
                            &preflight_key,
                            paragraph,
                            projection.mapping.text(),
                        )
                        .is_unchanged()
                        && entry
                            .segment
                            .geometry
                            .artifact
                            .features()
                            .contains(features)
                })
                .map(|entry| {
                    (
                        Arc::clone(&entry.segment.geometry.artifact),
                        entry.segment.region_transcript.clone(),
                    )
                })
        });
    let (prepared, candidate_transcript, formation_reuse) = if let Some((facts, transcript)) =
        retained_artifact
    {
        (
            PreparedParagraph::from_shared_facts(paragraph.id, facts),
            transcript,
            None,
        )
    } else if let Some(hit) = shared_hit {
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
            None,
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
                formation_change,
                features,
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
        let formation_reuse = output.reuse();
        record_formation_work(work, output.work());
        (
            output.paragraph().clone(),
            output.region_transcript().cloned(),
            Some(formation_reuse),
        )
    };
    let backend_called = formation_reuse.is_some();
    if let Some(formation_reuse) = formation_reuse {
        if formation_reuse.is_hit() {
            reuse.adapter_fact_hits = reuse.adapter_fact_hits.saturating_add(1);
        } else {
            reuse.adapter_fact_misses = reuse.adapter_fact_misses.saturating_add(1);
        }
        if capability_upgrade {
            match formation_reuse {
                ParagraphFormationReuse::Cold => {
                    reuse.cold_capability_upgrades =
                        reuse.cold_capability_upgrades.saturating_add(1);
                }
                ParagraphFormationReuse::RetainedFacts => {
                    reuse.warm_capability_upgrades =
                        reuse.warm_capability_upgrades.saturating_add(1);
                }
            }
        }
    }
    if prepared.paragraph() != paragraph.id
        || prepared.text_len() != text_len
        || !prepared.features().contains(features)
    {
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
    let geometry = match retained_paint_layout {
        Some(retained) => retained,
        None => {
            let mut geometry = match build_geometry(
                &prepared,
                projection,
                features,
                constraint,
                region_transcript.as_ref(),
            ) {
                Ok(geometry) => geometry,
                Err(error) => {
                    if backend_called {
                        paragraphs.release(preparation);
                    }
                    return Err(error);
                }
            };
            if let Some(retained) = retained_layout {
                geometry.facts = Arc::clone(&retained.facts);
                geometry.retain_sidecars_from(&retained);
            }
            Arc::new(geometry)
        }
    };
    let paint = match build_paint_topology(&prepared, projection, &geometry) {
        Ok(paint) => paint,
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
    work.geometry.add_paragraph(paint.fragments.len());
    let (previous_use, previous_accounted_bytes, current_accounted_bytes) =
        if let Some(entry) = cache.get_mut(&paragraph.id) {
            let previous_use = Some(entry.last_used);
            let previous_accounted_bytes = entry.accounted_bytes;
            entry.last_used = current_use;
            entry.preflight_key = preflight_key;
            entry.segment = Arc::new(ParagraphSceneSegment::new(
                paragraph.id,
                geometry,
                paint,
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
                segment: Arc::new(ParagraphSceneSegment::new(
                    paragraph.id,
                    geometry,
                    paint,
                    region_transcript.clone(),
                )),
                accounted_bytes: 0,
            };
            entry.accounted_bytes = entry.calculate_accounted_owned_bytes();
            let current_accounted_bytes = entry.accounted_bytes;
            cache.insert(paragraph.id, entry);
            (None, 0, current_accounted_bytes)
        };
    if backend_called {
        paragraphs.commit_preparation(preparation);
    }
    Ok(CacheAccess {
        previous_use,
        previous_accounted_bytes,
        current_accounted_bytes,
        region_transcript,
    })
}

#[derive(Clone, Debug)]
struct ParagraphPreflightKey {
    version: u64,
    composition: Option<Arc<CompositionPreparationKey>>,
    styles: StyleMap,
    constraint: ConstraintKey,
    region_flow: Option<RegionFlow>,
    region_cursor: Option<RegionCursor>,
}

impl ParagraphPreflightKey {
    fn new(
        paragraph: &Paragraph,
        composition: Option<Arc<CompositionPreparationKey>>,
        request: &SceneRequest<'_>,
        region_cursor: Option<RegionCursor>,
    ) -> Self {
        Self {
            version: paragraph.version,
            composition,
            styles: request.styles.clone(),
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
            && self.composition.is_none()
            && self.constraint == ConstraintKey::from(request.constraint)
            && self.region_cursor == region_cursor
            && region_provenance_matches(self.region_flow.as_ref(), request.region_flow)
            && (self.styles.shares_state_with(request.styles)
                || (self.styles.default_style() == request.styles.default_style()
                    && self.styles.paragraph_style_for(paragraph.id)
                        == request.styles.paragraph_style_for(paragraph.id)
                    && paragraph.leaves.iter().all(|leaf| {
                        self.styles.style_for(leaf.id) == request.styles.style_for(leaf.id)
                    })))
    }

    fn adapter_change(&self, current: &Self, paragraph: &Paragraph) -> ParagraphFormationChange {
        self.adapter_change_with_text_identity(
            current,
            paragraph,
            self.version == current.version && self.composition == current.composition,
        )
    }

    fn alternate_adapter_change(
        &self,
        current: &Self,
        paragraph: &Paragraph,
        current_projected_text: &str,
    ) -> ParagraphFormationChange {
        let same_text = self.composition.as_ref().is_some_and(|composition| {
            composition.projected_text.as_ref() == current_projected_text
        });
        self.adapter_change_with_text_identity(current, paragraph, same_text)
    }

    fn adapter_change_with_text_identity(
        &self,
        current: &Self,
        paragraph: &Paragraph,
        same_text: bool,
    ) -> ParagraphFormationChange {
        if !same_text {
            return ParagraphFormationChange::all();
        }
        let previous_paragraph = self.styles.paragraph_style_for(paragraph.id);
        let current_paragraph = current.styles.paragraph_style_for(paragraph.id);
        if previous_paragraph.whitespace_collapse() != current_paragraph.whitespace_collapse() {
            return ParagraphFormationChange::all();
        }

        let mut analysis =
            previous_paragraph.base_direction() != current_paragraph.base_direction();
        let mut font_selection = false;
        let mut ligature_policy = false;
        let mut inline_flow_projection = false;
        let mut spacing = false;
        let mut line_metrics = false;
        let mut break_policy = false;
        let mut paint = false;
        paragraph_inline_styles_match(
            paragraph,
            &self.styles,
            &current.styles,
            |previous, current| {
                analysis |= previous.analysis() != current.analysis();
                font_selection |= previous.shaping() != current.shaping();
                let previous_flow = previous.inline_flow();
                let current_flow = current.inline_flow();
                ligature_policy |= (previous_flow.spacing().letter() == 0.0)
                    != (current_flow.spacing().letter() == 0.0);
                inline_flow_projection |= previous_flow != current_flow;
                spacing |= previous_flow.spacing() != current_flow.spacing();
                line_metrics |= previous_flow.line_height() != current_flow.line_height();
                break_policy |= previous_flow.overflow_wrap() != current_flow.overflow_wrap()
                    || previous_flow.text_wrap_mode() != current_flow.text_wrap_mode();
                paint |= previous.paint() != current.paint();
                true
            },
        );
        let empty = paragraph.leaves.iter().all(|leaf| leaf.text().is_empty());
        let constraints = self.constraint != current.constraint
            || !option_ref_eq(self.region_flow.as_ref(), current.region_flow.as_ref())
            || self.region_cursor != current.region_cursor
            || (empty && line_metrics);
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

    fn formation_matches(&self, current: &Self, paragraph: &Paragraph) -> bool {
        let change = self.adapter_change(current, paragraph);
        !change.analysis_changed()
            && !change.font_selection_changed()
            && !change.ligature_policy_changed()
            && !change.inline_flow_projection_changed()
            && !change.spacing_changed()
            && !change.line_metrics_changed()
            && !change.break_policy_changed()
            && !change.constraints_changed()
    }

    fn adjustment_matches(&self, current: &Self, paragraph: ParagraphId) -> bool {
        self.styles.paragraph_style_for(paragraph).alignment()
            == current.styles.paragraph_style_for(paragraph).alignment()
    }

    fn paint_matches(&self, current: &Self, paragraph: &Paragraph) -> bool {
        self.version == current.version
            && self.composition == current.composition
            && paragraph_inline_styles_match(
                paragraph,
                &self.styles,
                &current.styles,
                |previous, current| previous.paint() == current.paint(),
            )
    }
}

#[derive(Debug, PartialEq, Eq)]
struct CompositionPreparationKey {
    id: CompositionId,
    projected_text: Arc<str>,
}

impl CompositionPreparationKey {
    fn new(id: CompositionId, projected_text: &str) -> Self {
        Self {
            id,
            projected_text: Arc::from(projected_text),
        }
    }
}

fn paragraph_inline_styles_match(
    paragraph: &Paragraph,
    previous: &StyleMap,
    current: &StyleMap,
    mut matches: impl FnMut(&ComputedInlineStyle, &ComputedInlineStyle) -> bool,
) -> bool {
    if paragraph.leaves.is_empty() {
        return matches(previous.default_style(), current.default_style());
    }
    paragraph
        .leaves
        .iter()
        .all(|leaf| matches(previous.style_for(leaf.id), current.style_for(leaf.id)))
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
    segment: Arc<ParagraphSceneSegment>,
    accounted_bytes: usize,
}

#[derive(Debug, Default)]
struct ParagraphCacheStore {
    index: BTreeMap<ParagraphId, usize>,
    entries: Vec<(ParagraphId, ParagraphCache)>,
}

impl ParagraphCacheStore {
    fn len(&self) -> usize {
        self.entries.len()
    }

    fn clear(&mut self) {
        self.index.clear();
        self.entries.clear();
    }

    fn contains_key(&self, paragraph: &ParagraphId) -> bool {
        self.index.contains_key(paragraph)
    }

    fn get(&self, paragraph: &ParagraphId) -> Option<&ParagraphCache> {
        let index = *self.index.get(paragraph)?;
        self.entries.get(index).map(|(_, entry)| entry)
    }

    fn get_mut(&mut self, paragraph: &ParagraphId) -> Option<&mut ParagraphCache> {
        let index = *self.index.get(paragraph)?;
        self.entries.get_mut(index).map(|(_, entry)| entry)
    }

    fn insert(&mut self, paragraph: ParagraphId, entry: ParagraphCache) -> Option<ParagraphCache> {
        if let Some(index) = self.index.get(&paragraph).copied() {
            return Some(core::mem::replace(&mut self.entries[index].1, entry));
        }
        let index = self.entries.len();
        self.entries.push((paragraph, entry));
        self.index.insert(paragraph, index);
        None
    }

    fn remove(&mut self, paragraph: &ParagraphId) -> Option<ParagraphCache> {
        let index = self.index.remove(paragraph)?;
        let (_, removed) = self.entries.swap_remove(index);
        if let Some((moved, _)) = self.entries.get(index) {
            *self
                .index
                .get_mut(moved)
                .expect("the swapped cache entry remains indexed") = index;
        }
        Some(removed)
    }

    fn retain(&mut self, mut keep: impl FnMut(&ParagraphId, &ParagraphCache) -> bool) {
        let mut index = 0;
        while index < self.entries.len() {
            if keep(&self.entries[index].0, &self.entries[index].1) {
                index += 1;
            } else {
                let paragraph = self.entries[index].0;
                self.remove(&paragraph);
            }
        }
    }

    fn values(&self) -> impl Iterator<Item = &ParagraphCache> {
        self.entries.iter().map(|(_, entry)| entry)
    }

    fn iter(&self) -> impl Iterator<Item = (&ParagraphId, &ParagraphCache)> {
        self.entries
            .iter()
            .map(|(paragraph, entry)| (paragraph, entry))
    }
}

impl ParagraphCache {
    fn calculate_accounted_owned_bytes(&self) -> usize {
        size_of::<Self>()
            .saturating_add(
                self.segment
                    .region_transcript
                    .as_ref()
                    .map_or(0, |transcript| {
                        vec_bytes::<crate::RegionAttempt>(transcript.attempts().len())
                    }),
            )
            .saturating_add(self.segment.geometry.accounted_owned_bytes())
            .saturating_add(self.segment.paint.residency_bytes())
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

fn effective_composition_features(
    request: &SceneRequest<'_>,
    target: ParagraphId,
) -> SceneFeaturePolicy {
    request.features.clone().with_paragraph(
        target,
        request
            .features
            .features_for(target)
            .with_native_text_input(),
    )
}

fn resident_feature_policy(spine: &SceneSpine, default: SceneFeatures) -> SceneFeaturePolicy {
    SceneFeaturePolicy::from_resolved(
        default,
        spine.segments().map(|positioned| {
            (
                positioned.segment.paragraph,
                positioned.segment.geometry.features,
            )
        }),
    )
}

fn region_provenance_matches(left: Option<&RegionFlow>, right: Option<&RegionFlow>) -> bool {
    match (left, right) {
        (Some(left), Some(right)) => left.shares_backing_with(right),
        (None, None) => true,
        _ => false,
    }
}

fn scene_region_binding(
    summary: SceneSummary,
    paragraph_count: usize,
    region_flow: Option<&RegionFlow>,
    start: Option<RegionCursor>,
    end: Option<RegionCursor>,
) -> Result<Option<SceneRegionBinding>, SceneError> {
    match (region_flow, start, end) {
        (None, None, None)
            if summary.region_chain_valid
                && summary.region_start.is_none()
                && summary.region_end.is_none()
                && summary.region_attempts == 0 =>
        {
            Ok(None)
        }
        (Some(_), Some(start), Some(end))
            if summary.region_chain_valid
                && ((paragraph_count == 0
                    && summary.region_start.is_none()
                    && summary.region_end.is_none()
                    && start == end)
                    || (summary.region_start == Some(start)
                        && summary.region_end == Some(end))) =>
        {
            Ok(Some(SceneRegionBinding {
                start,
                end,
                attempts: summary.region_attempts,
                height_rejections: summary.region_height_rejections,
            }))
        }
        _ => Err(SceneError::new(SceneErrorKind::Flow)),
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
