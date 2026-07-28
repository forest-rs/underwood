// Copyright 2026 the Underwood Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use super::*;

fn analyze_with_word_break(text: &str, word_break: WordBreak) -> parley_engine::Analysis {
    let overrides = [(0..text.len(), word_break)];
    let mut analysis = parley_engine::Analysis::new();
    parley_engine::Analyzer::new().analyze(
        text,
        &parley_engine::AnalysisOptions {
            base_direction: BaseDirection::Auto,
            word_break: if word_break == WordBreak::Normal {
                &[]
            } else {
                &overrides
            },
            ..parley_engine::AnalysisOptions::default()
        },
        &mut analysis,
    );
    analysis
}

fn boundaries(text: &str, word_break: WordBreak) -> Vec<(usize, parley_engine::Boundary)> {
    let analysis = analyze_with_word_break(text, word_break);
    text.char_indices()
        .zip(analysis.char_info())
        .filter_map(|((byte, _), info)| {
            (info.boundary != parley_engine::Boundary::None).then_some((byte, info.boundary))
        })
        .collect()
}

fn line_boundaries(text: &str, word_break: WordBreak) -> Vec<usize> {
    boundaries(text, word_break)
        .into_iter()
        .filter_map(|(byte, boundary)| (boundary == parley_engine::Boundary::Line).then_some(byte))
        .collect()
}

fn shape_with_fixture_font(text: &str, analysis: &parley_engine::Analysis) -> ShapedText {
    let font = FontInstance {
        font: FontData::new(Blob::from(LATIN_FONT.to_vec()), 0),
        synthesis: Synthesis::default(),
    };
    let style_indices = vec![0; text.chars().count()];
    let mut shaper = Shaper::default();
    let mut shaped = ShapedText::new();
    for item in analysis.itemize(text, |_| false) {
        shaper.shape_item(
            text,
            analysis,
            &item,
            &ShapeOptions {
                font_size: 20.0,
                language: None,
                features: &[],
                variations: &[],
                char_style_indices: &style_indices,
            },
            |_| Some(font.clone()),
            &mut shaped,
        );
    }
    shaped
}

#[test]
fn normal_cjk_boundaries_lock_punctuation_script_and_grapheme_traps() {
    for (name, text, expected) in [
        ("japanese-closing-punctuation", "漢。字", &[6][..]),
        ("japanese-opening-punctuation", "漢「字", &[3][..]),
        ("small-kana", "漢ゃ字", &[6][..]),
        ("iteration-mark", "漢々字", &[6][..]),
        ("ideographic-space", "漢　字", &[6][..]),
        ("hiragana-katakana", "かなカナ", &[3, 6, 9][..]),
        ("chinese", "你好世界", &[3, 6, 9][..]),
        ("korean", "한글문자", &[3, 6, 9][..]),
        ("mixed-latin-han", "abc漢字", &[3, 6][..]),
        ("emoji-zwj", "漢👩\u{200d}💻字", &[3, 14][..]),
    ] {
        assert_eq!(
            line_boundaries(text, WordBreak::Normal),
            expected,
            "{name}: {text:?}"
        );
    }
}

#[test]
fn word_break_values_have_distinct_cjk_and_latin_effects() {
    assert_eq!(line_boundaries("AB", WordBreak::Normal), []);
    assert_eq!(line_boundaries("AB", WordBreak::BreakAll), [1]);
    assert_eq!(line_boundaries("AB", WordBreak::KeepAll), []);

    assert_eq!(line_boundaries("漢字", WordBreak::Normal), [3]);
    assert_eq!(line_boundaries("漢字", WordBreak::BreakAll), [3]);
    assert_eq!(line_boundaries("漢字", WordBreak::KeepAll), []);
}

#[test]
fn mandatory_breaks_and_emoji_sequences_remain_atomic() {
    assert!(
        boundaries("漢\n字", WordBreak::KeepAll)
            .iter()
            .any(|&(byte, boundary)| byte == 4 && boundary == parley_engine::Boundary::Mandatory),
        "word-break policy cannot suppress an authored mandatory break"
    );

    let text = "漢👩\u{200d}💻字";
    let emoji = text.find('👩').expect("emoji starts");
    let after_emoji = text.find('字').expect("suffix starts");
    assert!(
        line_boundaries(text, WordBreak::BreakAll)
            .into_iter()
            .all(|byte| byte <= emoji || byte >= after_emoji),
        "break-all cannot split an extended emoji ZWJ grapheme"
    );
}

#[test]
fn reusable_line_formation_commits_only_analyzed_cjk_boundaries() {
    let text = "漢。字";
    let analysis = analyze_with_word_break(text, WordBreak::Normal);
    let shaped = shape_with_fixture_font(text, &analysis);
    let clusters =
        collect_logical_clusters(text, &shaped).expect("fixture clusters must be complete");

    let first = choose_line(&clusters, 0, TextConstraint::MinContent)
        .expect("first min-content candidate must form");
    assert_eq!(first.reason, TestLineBreakReason::Regular);
    assert_eq!(
        clusters[first.end - 1].source.end,
        6,
        "closing punctuation stays with the preceding ideograph"
    );

    let second = choose_line(&clusters, first.end, TextConstraint::MinContent)
        .expect("remaining min-content candidate must form");
    assert_eq!(second.reason, TestLineBreakReason::End);
    assert_eq!(clusters[second.end - 1].source.end, text.len());
}

#[cfg(all(feature = "system-fonts", target_vendor = "apple"))]
#[test]
fn public_scene_path_preserves_cjk_word_break_policy() {
    let text = "漢字";
    let (document, normal_styles, paint) = fixture_document(text, 1.2);
    let keep_all_styles = StyleMap::new(
        normal_styles
            .default_style()
            .clone()
            .with_analysis(AnalysisStyle::new(WordBreak::KeepAll)),
    );
    let fonts = FontSet::try_from_fonts([
        Font::from_bytes("latin", LATIN_FONT).expect("Latin fixture font is valid")
    ])
    .expect("fixture catalog is valid")
    .with_system_fonts();
    let mut engine = LayoutEngine::new(ParleyParagraphEngine::new(fonts), CacheBudget::new(4));

    let normal = engine
        .prepare(
            &document.snapshot(),
            &editable_scene_request(TextConstraint::MinContent, &normal_styles, &paint),
        )
        .expect("native Han fallback must prepare the normal line policy");
    assert_eq!(normal.scene.lines().len(), 2);
    assert_eq!(
        normal
            .scene
            .line(0)
            .expect("line exists")
            .sources()
            .expect("line belongs to source scene")
            .iter()
            .next()
            .expect("source exists")
            .bytes(),
        0..3,
        "normal CJK text must expose the ordinary inter-ideograph opportunity"
    );

    let keep_all = engine
        .prepare(
            &document.snapshot(),
            &editable_scene_request(TextConstraint::MinContent, &keep_all_styles, &paint),
        )
        .expect("native Han fallback must prepare keep-all");
    assert_eq!(
        keep_all.scene.lines().len(),
        1,
        "keep-all must suppress the ordinary inter-ideograph opportunity"
    );
}
