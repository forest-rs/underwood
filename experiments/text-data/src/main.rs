// Copyright 2026 the Underwood Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Private ADR-0003 text-data distribution wind tunnel.

use parley_core::{Analysis, AnalysisOptions, Analyzer, Boundary};

const PARLEY_REVISION: &str = "45da4a90248b1600277a4294b70d8bfde5ca8e97";
const ICU4X_VERSION: &str = "2.2.0";
const CORPUS: &str = concat!(
    "Latin cafe\u{301} résumé. ",
    "Ελληνικά Кириллица العربية עברית. ",
    "ภาษาไทยไม่มีช่องว่าง ภาษาไทยทดสอบ. ",
    "ភាសាខ្មែរត្រូវការការបំបែកពាក្យ. ",
    "ພາສາລາວຕ້ອງການການແບ່ງຄຳ. ",
    "မြန်မာစာစကားလုံးခွဲခြားမှု. ",
    "日本語の改行、中国文字、한글. ",
    "emoji 👩🏽‍💻 family 👨‍👩‍👧‍👦 flags 🇹🇭.\n",
);
#[cfg(not(target_arch = "wasm32"))]
const THROUGHPUT_CORPUS: &str = concat!(
    "Latin cafe\u{301} résumé and line breaking. ",
    "Ελληνικά Кириллица العربية עברית. ",
    "emoji 👩🏽‍💻 family 👨‍👩‍👧‍👦 flags 🇹🇭.\n",
);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Capability {
    ComplexSegmentation,
    Hyphenation,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct TraceIdentity {
    parley_revision: &'static str,
    icu4x_version: &'static str,
    tier: &'static str,
}

impl TraceIdentity {
    const fn compiled() -> Self {
        Self {
            parley_revision: PARLEY_REVISION,
            icu4x_version: ICU4X_VERSION,
            tier: if cfg!(feature = "complex-scripts") {
                "complex-segmentation"
            } else {
                "minimal"
            },
        }
    }

    const fn upgraded() -> Self {
        Self {
            parley_revision: "candidate-upgrade",
            icu4x_version: ICU4X_VERSION,
            tier: "minimal",
        }
    }

    const fn supports(self, capability: Capability) -> bool {
        match capability {
            Capability::ComplexSegmentation => cfg!(feature = "complex-scripts"),
            Capability::Hyphenation => false,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ReplayKind {
    PrimitiveEdit,
    DataDependentCommand,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TraceError {
    DataIdentityMismatch {
        recorded: TraceIdentity,
        installed: TraceIdentity,
    },
    MissingCapability {
        requested: Capability,
        installed: TraceIdentity,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct AnalysisReport {
    source_bytes: usize,
    characters: usize,
    word_boundaries: usize,
    line_boundaries: usize,
    mandatory_boundaries: usize,
    bidi_characters: usize,
    emoji_or_pictographs: usize,
    forced_normalizations: usize,
    digest: u64,
}

fn analyze(text: &str) -> AnalysisReport {
    let mut analyzer = Analyzer::new();
    let mut analysis = Analysis::new();
    let options = AnalysisOptions {
        word_break: &[],
        line_break_override: None,
    };
    analyzer.analyze(text, &options, &mut analysis);

    let mut digest = 0xcbf2_9ce4_8422_2325_u64;
    for byte in text.bytes() {
        digest = mix(digest, byte);
    }

    let mut word_boundaries = 0;
    let mut line_boundaries = 0;
    let mut mandatory_boundaries = 0;
    let mut emoji_or_pictographs = 0;
    let mut forced_normalizations = 0;
    for info in analysis.char_info() {
        let boundary = match info.boundary {
            Boundary::None => 0,
            Boundary::Word => {
                word_boundaries += 1;
                1
            }
            Boundary::Line => {
                line_boundaries += 1;
                2
            }
            Boundary::Mandatory => {
                mandatory_boundaries += 1;
                3
            }
        };
        digest = mix(digest, boundary);
        if info.is_emoji_or_pictograph() {
            emoji_or_pictographs += 1;
            digest = mix(digest, 5);
        }
        if info.force_normalize() {
            forced_normalizations += 1;
            digest = mix(digest, 7);
        }
    }
    for level in analysis.bidi_levels() {
        digest = mix(digest, *level);
    }

    AnalysisReport {
        source_bytes: text.len(),
        characters: analysis.char_info().len(),
        word_boundaries,
        line_boundaries,
        mandatory_boundaries,
        bidi_characters: analysis.bidi_levels().len(),
        emoji_or_pictographs,
        forced_normalizations,
        digest,
    }
}

const fn mix(hash: u64, byte: u8) -> u64 {
    (hash ^ byte as u64).wrapping_mul(0x0000_0100_0000_01b3)
}

fn replay(
    kind: ReplayKind,
    recorded: TraceIdentity,
    installed: TraceIdentity,
) -> Result<(), TraceError> {
    if kind == ReplayKind::DataDependentCommand && recorded != installed {
        return Err(TraceError::DataIdentityMismatch {
            recorded,
            installed,
        });
    }
    Ok(())
}

fn require(bundle: TraceIdentity, requested: Capability) -> Result<(), TraceError> {
    if !bundle.supports(requested) {
        return Err(TraceError::MissingCapability {
            requested,
            installed: bundle,
        });
    }
    Ok(())
}

fn assert_trace_laws(bundle: TraceIdentity) {
    assert_eq!(
        replay(ReplayKind::PrimitiveEdit, bundle, TraceIdentity::upgraded()),
        Ok(()),
        "primitive edits must survive a data-only upgrade"
    );
    assert!(
        matches!(
            replay(
                ReplayKind::DataDependentCommand,
                bundle,
                TraceIdentity::upgraded()
            ),
            Err(TraceError::DataIdentityMismatch { .. })
        ),
        "data-dependent replay must reject a changed bundle identity"
    );
    assert!(
        matches!(
            require(bundle, Capability::Hyphenation),
            Err(TraceError::MissingCapability { .. })
        ),
        "the compiled Parley tiers do not provide hyphenation"
    );
    assert_eq!(
        require(bundle, Capability::ComplexSegmentation).is_ok(),
        cfg!(feature = "complex-scripts"),
        "complex segmentation must follow the exact compiled feature set"
    );
}

fn main() {
    let bundle = TraceIdentity::compiled();
    let report = analyze(CORPUS);
    assert_trace_laws(bundle);
    std::hint::black_box((bundle, report));

    #[cfg(not(target_arch = "wasm32"))]
    run_native_report(bundle, report);
}

#[cfg(not(target_arch = "wasm32"))]
fn run_native_report(bundle: TraceIdentity, report: AnalysisReport) {
    use std::time::Instant;

    const ITERATIONS: usize = 1_000;
    let start = Instant::now();
    for _ in 0..ITERATIONS {
        std::hint::black_box(analyze(std::hint::black_box(THROUGHPUT_CORPUS)));
    }
    let elapsed = start.elapsed();
    let bytes = THROUGHPUT_CORPUS.len() * ITERATIONS;
    let mebibytes_per_second = bytes as f64 / elapsed.as_secs_f64() / (1024.0 * 1024.0);

    println!(
        "tier={} parley={} icu4x={} bytes={} chars={} words={} lines={} mandatory={} bidi={} emoji={} normalize={} digest={:016x} iterations={} elapsed_ns={} mib_per_second={:.3}",
        bundle.tier,
        bundle.parley_revision,
        bundle.icu4x_version,
        report.source_bytes,
        report.characters,
        report.word_boundaries,
        report.line_boundaries,
        report.mandatory_boundaries,
        report.bidi_characters,
        report.emoji_or_pictographs,
        report.forced_normalizations,
        report.digest,
        ITERATIONS,
        elapsed.as_nanos(),
        mebibytes_per_second,
    );
}

#[cfg(test)]
mod tests {
    use super::{
        AnalysisReport, CORPUS, Capability, ReplayKind, TraceError, TraceIdentity, analyze, replay,
        require,
    };

    #[test]
    fn multilingual_analysis_is_deterministic_and_exercises_the_inventory() {
        let first = analyze(CORPUS);
        let second = analyze(CORPUS);
        assert_eq!(first, second, "analysis must be deterministic");
        assert!(
            first.characters > 100,
            "the corpus must remain multilingual"
        );
        assert!(
            first.word_boundaries > 0,
            "the corpus must exercise word boundaries"
        );
        assert!(
            first.line_boundaries > 0,
            "the corpus must exercise line boundaries"
        );
        assert!(
            first.bidi_characters > 0,
            "the corpus must exercise bidi analysis"
        );
        assert!(
            first.emoji_or_pictographs > 0,
            "the corpus must exercise emoji properties"
        );
        assert!(
            first.forced_normalizations > 0,
            "the corpus must exercise normalization"
        );
        assert_ne!(
            first.digest,
            AnalysisReport::default_for_test().digest,
            "the trace digest must reflect the analyzed result"
        );
    }

    #[test]
    fn replay_rejects_only_data_dependent_work_after_an_upgrade() {
        let recorded = TraceIdentity::compiled();
        let installed = TraceIdentity::upgraded();
        assert_eq!(
            replay(ReplayKind::PrimitiveEdit, recorded, installed),
            Ok(()),
            "primitive replay must not depend on text-data identity"
        );
        assert!(
            matches!(
                replay(ReplayKind::DataDependentCommand, recorded, installed),
                Err(TraceError::DataIdentityMismatch { .. })
            ),
            "data-dependent replay must name a changed identity"
        );
    }

    #[test]
    fn unavailable_capabilities_are_diagnostic() {
        let bundle = TraceIdentity::compiled();
        assert!(
            matches!(
                require(bundle, Capability::Hyphenation),
                Err(TraceError::MissingCapability { .. })
            ),
            "hyphenation must be diagnosed as absent"
        );
        assert_eq!(
            require(bundle, Capability::ComplexSegmentation).is_ok(),
            cfg!(feature = "complex-scripts"),
            "capability negotiation must match the compiled tier"
        );
    }

    impl AnalysisReport {
        const fn default_for_test() -> Self {
            Self {
                source_bytes: 0,
                characters: 0,
                word_boundaries: 0,
                line_boundaries: 0,
                mandatory_boundaries: 0,
                bidi_characters: 0,
                emoji_or_pictographs: 0,
                forced_normalizations: 0,
                digest: 0,
            }
        }
    }
}
