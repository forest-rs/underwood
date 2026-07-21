// Copyright 2026 the Underwood Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::mem::size_of;
use std::process::ExitCode;

use crate::extent::{ExtentMap, Knowledge};
use crate::flow::{
    Checkpoint, Document, Fragment, Policy, layout, reflow_until_convergence, select_restart,
};

const LONG_BOOK_BLOCKS: u32 = 100_000;
const REGION_BLOCKS: u32 = 512;
const NAVIGATION_SAMPLES: usize = 10_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Status {
    Pass,
    Fail,
    NotRun,
    Screen,
}

impl Status {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Pass => "PASS",
            Self::Fail => "FAIL",
            Self::NotRun => "NOT_RUN",
            Self::Screen => "SCREEN",
        }
    }
}

#[derive(Debug)]
struct Observation {
    id: &'static str,
    status: Status,
    observed: String,
    limit: &'static str,
    note: &'static str,
}

#[derive(Debug)]
struct Density {
    policy: &'static str,
    checkpoints: usize,
    bytes: usize,
    memory_limit: usize,
    p95_restart: usize,
    max_restart: usize,
}

pub(crate) fn run() -> ExitCode {
    println!("flow-trace-v0");
    println!("candidate\tsynthetic-continuation-model-v0");
    println!(
        "machine\t{}-{}\tallocator=system",
        std::env::consts::OS,
        std::env::consts::ARCH
    );
    println!(
        "status-key\tPASS=implemented-law-or-complete-work-gate\tSCREEN=preliminary-not-proof"
    );

    let document = Document::synthetic(LONG_BOOK_BLOCKS, REGION_BLOCKS);
    let policies = [
        ("region-boundary", Policy::RegionBoundary),
        ("fixed-16", Policy::Fixed { blocks: 16 }),
        ("fixed-64", Policy::Fixed { blocks: 64 }),
        ("fixed-256", Policy::Fixed { blocks: 256 }),
        (
            "adaptive",
            Policy::Adaptive {
                work_threshold: 640,
                hard_max_blocks: 1024,
            },
        ),
    ];
    let densities = policies
        .into_iter()
        .map(|(name, policy)| density(&document, name, policy))
        .collect::<Vec<_>>();
    print_densities(&densities);

    let observations = semantic_observations(&document, &densities);
    print_observations(&observations);
    if observations
        .iter()
        .filter(|observation| observation.status != Status::NotRun)
        .all(|observation| observation.status != Status::Fail)
    {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

fn density(document: &Document, name: &'static str, policy: Policy) -> Density {
    let result = layout(document, policy, 1, None);
    assert!(result.published, "uncancelled layout publishes");
    let bytes = result
        .checkpoints
        .iter()
        .map(|checkpoint| checkpoint.encode().len())
        .sum();
    let realized_memory = result.fragments.len() * size_of::<Fragment>();
    let memory_limit = (realized_memory * 2 / 100).max(document.blocks.len() * 8);
    let mut distances = Vec::with_capacity(NAVIGATION_SAMPLES);
    let mut random = 0x5eed_f10a_0000_0002_u64;
    for _ in 0..NAVIGATION_SAMPLES {
        let invalidated = random_index(&mut random, document.blocks.len());
        let restart = select_restart(
            &result.checkpoints,
            document,
            policy,
            document.source_revision,
            invalidated,
        )
        .map_or(0, |checkpoint| checkpoint.next_block() as usize);
        distances.push(invalidated - restart);
    }
    distances.sort_unstable();
    Density {
        policy: name,
        checkpoints: result.checkpoints.len(),
        bytes,
        memory_limit,
        p95_restart: distances[NAVIGATION_SAMPLES * 95 / 100],
        max_restart: *distances.last().expect("samples are nonempty"),
    }
}

fn print_densities(densities: &[Density]) {
    println!("density_policy\tcheckpoints\tbytes\tmemory_limit\tp95_restart\tmax_restart");
    for density in densities {
        println!(
            "{}\t{}\t{}\t{}\t{}\t{}",
            density.policy,
            density.checkpoints,
            density.bytes,
            density.memory_limit,
            density.p95_restart,
            density.max_restart
        );
    }
}

fn semantic_observations(document: &Document, densities: &[Density]) -> Vec<Observation> {
    let policy = Policy::Adaptive {
        work_threshold: 640,
        hard_max_blocks: 1024,
    };
    let previous = layout(document, policy, 1, None);
    let checkpoint = previous
        .checkpoints
        .first()
        .expect("adaptive policy creates checkpoints");
    let encoded = checkpoint.encode();
    let decoded = Checkpoint::decode(&encoded).expect("checkpoint round trip");

    let edit_index = 50_000;
    let glyph_edit = document.edit_metrics_preserving(edit_index);
    let glyph_reflow = reflow_until_convergence(
        &previous,
        document.source_revision,
        &glyph_edit,
        policy,
        edit_index,
        2,
    );
    let extent_edit = document.edit_extent(edit_index, 1024);
    let extent_reflow = reflow_until_convergence(
        &previous,
        document.source_revision,
        &extent_edit,
        policy,
        edit_index,
        3,
    );
    let cancellation = layout(document, policy, 4, Some(1_000));

    let adaptive = densities
        .iter()
        .find(|density| density.policy == "adaptive")
        .expect("adaptive density was measured");

    let mut extent_map = ExtentMap::estimated(1_000_000, 1024, 16 * 1024);
    let mut random = 0x5eed_e87e_0000_0002_u64;
    let mut max_comparisons = 0;
    let mut realized_blocks = 0;
    for _ in 0..NAVIGATION_SAMPLES {
        let coordinate = next_random(&mut random) % extent_map.total_extent();
        let navigation = extent_map
            .navigate(coordinate)
            .expect("generated coordinate is in range");
        max_comparisons = max_comparisons.max(navigation.comparisons);
        realized_blocks = realized_blocks.max(navigation.blocks.len());
    }
    let before = extent_map.geometry(200_000);
    let correction = extent_map
        .measure(195, before.current + 1024, 2, Some(200_000))
        .expect("measured segment exists");
    let measured = extent_map.geometry(200_000);
    let semantic_query = extent_map.semantic_child_exists(999_999);
    let unavailable = extent_map.geometry(1_000_000);

    vec![
        Observation {
            id: "checkpoint-round-trip",
            status: pass_if(decoded == *checkpoint && decoded.encode() == encoded),
            observed: format!(
                "bytes={} reencode_equal={} full_state_equal={}",
                encoded.len(),
                decoded.encode() == encoded,
                decoded == *checkpoint
            ),
            limit: "byte-stable canonical re-encode and identical successor",
            note: "private fixed-width little-endian experiment encoding",
        },
        Observation {
            id: "adaptive-checkpoint-memory",
            status: Status::Screen,
            observed: format!(
                "serialized_bytes={} structural_limit={} below_limit={} checkpoints={}",
                adaptive.bytes,
                adaptive.memory_limit,
                adaptive.bytes <= adaptive.memory_limit,
                adaptive.checkpoints
            ),
            limit: "<=max(2% realized flow memory, 8 bytes/source block)",
            note: "serialized bytes and size_of fragments are not retained-memory proof",
        },
        Observation {
            id: "adaptive-restart-distance",
            status: pass_if(adaptive.p95_restart <= 128 && adaptive.max_restart <= 1024),
            observed: format!(
                "p95={} blocks max={} blocks",
                adaptive.p95_restart, adaptive.max_restart
            ),
            limit: "p95 <=128 blocks; max <=1024 blocks",
            note: "10000 deterministic edit positions in 100000-block corpus",
        },
        Observation {
            id: "metrics-preserving-convergence",
            status: pass_if(
                glyph_reflow.prefix_fragments_emitted == 0
                    && glyph_reflow.converged_at.is_some()
                    && glyph_reflow.emitted_blocks < document.blocks.len(),
            ),
            observed: format!(
                "restart={} converge={:?} emitted={} prefix={}",
                glyph_reflow.restart_block,
                glyph_reflow.converged_at,
                glyph_reflow.emitted_blocks,
                glyph_reflow.prefix_fragments_emitted
            ),
            limit: "zero prefix emission; stop at matching successor checkpoint",
            note: "fingerprint changes while extent and carried state remain stable",
        },
        Observation {
            id: "extent-change-convergence",
            status: pass_if(
                extent_reflow.prefix_fragments_emitted == 0 && extent_reflow.converged_at.is_some(),
            ),
            observed: format!(
                "restart={} converge={:?} emitted={} prefix={}",
                extent_reflow.restart_block,
                extent_reflow.converged_at,
                extent_reflow.emitted_blocks,
                extent_reflow.prefix_fragments_emitted
            ),
            limit: "zero prefix emission; bounded by affected region frontier",
            note: "region reset removes the changed block-coordinate state",
        },
        Observation {
            id: "cancellation-publication",
            status: pass_if(
                !cancellation.published
                    && cancellation.fragments.is_empty()
                    && cancellation.checkpoints.is_empty(),
            ),
            observed: format!(
                "published={} fragments={} checkpoints={}",
                cancellation.published,
                cancellation.fragments.len(),
                cancellation.checkpoints.len()
            ),
            limit: "publication count remains zero",
            note: "deterministic work-budget cancellation",
        },
        Observation {
            id: "million-line-navigation-work",
            status: pass_if(max_comparisons <= 10 && realized_blocks <= 1024),
            observed: format!(
                "segments={} max_comparisons={max_comparisons} candidate_blocks={realized_blocks}",
                extent_map.segment_count()
            ),
            limit: "no full-prefix flow; at most one 1024-block candidate segment",
            note: "binary search over ordered virtual prefix aggregation",
        },
        Observation {
            id: "extent-honesty-and-correction",
            status: pass_if(
                before.knowledge == Knowledge::Estimated
                    && measured.knowledge == Knowledge::Measured
                    && !correction.estimator_violation
                    && correction.delta_above == 0
                    && correction.delta_through == 1024
                    && correction.delta_below == 1024
                    && correction.anchor_survived,
            ),
            observed: format!(
                "before={:?} after={:?} delta=({},{},{}) violation={} reaggregated={}",
                before.knowledge,
                measured.knowledge,
                correction.delta_above,
                correction.delta_through,
                correction.delta_below,
                correction.estimator_violation,
                correction.segments_reaggregated
            ),
            limit: "knowledge tagged; exact correction; violation diagnostic when out of bounds",
            note: "host anchor policy remains outside the extent map",
        },
        Observation {
            id: "accessibility-knowledge",
            status: pass_if(
                semantic_query
                    && before.knowledge == Knowledge::Estimated
                    && measured.knowledge == Knowledge::Measured
                    && unavailable.knowledge == Knowledge::Unavailable,
            ),
            observed: format!(
                "semantic_query={semantic_query} estimated={:?} measured={:?} outside={:?}",
                before.knowledge, measured.knowledge, unavailable.knowledge
            ),
            limit: "semantic query independent; geometry explicitly tagged",
            note: "no precise geometry is fabricated for estimated or absent blocks",
        },
        Observation {
            id: "adversarial-layout-features",
            status: Status::NotRun,
            observed: String::from(
                "synthetic carried-state digests only; no floats, footnotes, tables, or nested flow",
            ),
            limit: "all accepted adversarial continuation traces",
            note: "blocks conformance and checkpoint-format selection",
        },
        Observation {
            id: "wall-clock-latency",
            status: Status::Screen,
            observed: String::from("not calibrated"),
            limit: "50 ms navigation; 16 ms corrected viewport where applicable",
            note: "work gates are primary until machine and confidence method are ratified",
        },
    ]
}

fn print_observations(observations: &[Observation]) {
    println!("observation\tstatus\tobserved\tlimit\tnote");
    for observation in observations {
        println!(
            "{}\t{}\t{}\t{}\t{}",
            observation.id,
            observation.status.as_str(),
            observation.observed,
            observation.limit,
            observation.note
        );
    }
}

const fn pass_if(condition: bool) -> Status {
    if condition {
        Status::Pass
    } else {
        Status::Fail
    }
}

fn random_index(state: &mut u64, len: usize) -> usize {
    let modulus = u64::try_from(len).expect("synthetic collection length fits u64");
    usize::try_from(next_random(state) % modulus).expect("reduced random index fits usize")
}

fn next_random(state: &mut u64) -> u64 {
    *state = state
        .wrapping_mul(6_364_136_223_846_793_005)
        .wrapping_add(1_442_695_040_888_963_407);
    *state
}

#[cfg(test)]
mod tests {
    use super::{Document, Policy, density};

    #[test]
    fn adaptive_density_meets_restart_and_memory_work_gates() {
        let document = Document::synthetic(100_000, 512);
        let measured = density(
            &document,
            "adaptive",
            Policy::Adaptive {
                work_threshold: 640,
                hard_max_blocks: 1024,
            },
        );
        assert!(measured.bytes <= measured.memory_limit);
        assert!(measured.p95_restart <= 128);
        assert!(measured.max_restart <= 1024);
    }
}
