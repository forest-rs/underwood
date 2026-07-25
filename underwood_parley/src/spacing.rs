// Copyright 2026 the Underwood Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Post-shape letter and word spacing.
//!
//! This module owns advance adjustment after glyph selection. It does not own
//! Unicode boundary analysis, font selection, line breaking, or scene lowering.

use alloc::vec::Vec;

use parley_engine::{Analysis, ShapedText, shape::Whitespace};
use underwood::InlineFlowStyle;
use underwood::adapter::{InlineFlowRun, PreparationError};

use crate::shaping::has_joining_behavior;

pub(crate) fn collect_joining_units(
    text: &str,
    analysis: &Analysis,
    interaction_units: &[core::ops::Range<usize>],
    joining_units: &mut Vec<bool>,
) -> Result<(), PreparationError> {
    joining_units.clear();
    joining_units.resize(interaction_units.len(), false);
    let mut unit = 0_usize;
    let mut characters = 0_usize;
    for ((byte, _), info) in text.char_indices().zip(analysis.char_info()) {
        characters += 1;
        while interaction_units
            .get(unit)
            .is_some_and(|range| byte >= range.end)
        {
            unit += 1;
        }
        if !interaction_units
            .get(unit)
            .is_some_and(|range| range.start <= byte && byte < range.end)
        {
            return Err(PreparationError::invalid_output());
        }
        joining_units[unit] |= has_joining_behavior(info.joining_type);
    }
    if characters != text.chars().count() || characters != analysis.char_info().len() {
        return Err(PreparationError::invalid_output());
    }
    Ok(())
}

pub(crate) fn capture_base_advances(
    shaped: &ShapedText,
    clusters: &mut Vec<f32>,
    glyphs: &mut Vec<f32>,
) {
    clusters.clear();
    clusters.extend(shaped.clusters().iter().map(|cluster| cluster.advance));
    glyphs.clear();
    glyphs.extend(shaped.glyphs().iter().map(|glyph| glyph.advance));
}

pub(crate) fn restore_base_advances(
    shaped: &mut ShapedText,
    clusters: &[f32],
    glyphs: &[f32],
) -> Result<(), PreparationError> {
    if shaped.clusters().len() != clusters.len() || shaped.glyphs().len() != glyphs.len() {
        return Err(PreparationError::invalid_output());
    }
    for (cluster, advance) in shaped.clusters_mut().iter_mut().zip(clusters) {
        cluster.advance = *advance;
    }
    for (glyph, advance) in shaped.glyphs_mut().iter_mut().zip(glyphs) {
        glyph.advance = *advance;
    }
    Ok(())
}

pub(crate) fn apply_spacing(
    shaped: &mut ShapedText,
    styles: &[InlineFlowStyle],
    runs: &[InlineFlowRun],
    interaction_units: &[core::ops::Range<usize>],
    joining_units: &[bool],
) -> Result<(), PreparationError> {
    if interaction_units.len() != joining_units.len() {
        return Err(PreparationError::invalid_output());
    }
    for run_index in 0..shaped.runs().len() {
        let (source_start, clusters_start, clusters_end, glyphs_start) = {
            let run = &shaped.runs()[run_index];
            (
                run.range.byte_range.start,
                run.clusters_range.start,
                run.clusters_range.end,
                run.glyphs_range.start,
            )
        };
        let flow = flow_style_at(source_start, styles, runs)?;
        let spacing = flow.spacing();
        if spacing == underwood::TextSpacing::NORMAL {
            continue;
        }
        for cluster_index in clusters_start..clusters_end {
            let (source, whitespace, component, glyph_offset, glyph_len, base_advance) = {
                let cluster = shaped
                    .clusters()
                    .get(cluster_index)
                    .ok_or_else(PreparationError::invalid_output)?;
                let start = source_start
                    .checked_add(usize::from(cluster.text_offset))
                    .ok_or_else(PreparationError::invalid_output)?;
                let end = start
                    .checked_add(usize::from(cluster.text_len))
                    .ok_or_else(PreparationError::invalid_output)?;
                (
                    start..end,
                    cluster.info.whitespace(),
                    cluster.is_ligature_component(),
                    cluster.glyph_offset,
                    cluster.glyph_len,
                    cluster.advance,
                )
            };
            if base_advance <= 0.0 || component || whitespace == Whitespace::Newline {
                continue;
            }
            let unit = interaction_units
                .binary_search_by_key(&source.start, |unit| unit.start)
                .ok();
            let mut requested = 0.0_f32;
            if unit.is_some_and(|unit| !joining_units[unit]) {
                requested += spacing.letter();
            }
            if matches!(whitespace, Whitespace::Space | Whitespace::NoBreakSpace) {
                requested += spacing.word();
            }
            if requested == 0.0 {
                continue;
            }
            let adjusted = (base_advance + requested).max(0.0);
            let applied = adjusted - base_advance;
            shaped.clusters_mut()[cluster_index].advance = adjusted;
            if glyph_len != 0 && glyph_len != u8::MAX {
                let glyph = glyphs_start
                    .checked_add(glyph_offset as usize)
                    .and_then(|start| start.checked_add(usize::from(glyph_len) - 1))
                    .ok_or_else(PreparationError::invalid_output)?;
                let glyph = shaped
                    .glyphs_mut()
                    .get_mut(glyph)
                    .ok_or_else(PreparationError::invalid_output)?;
                glyph.advance += applied;
            }
        }
    }
    Ok(())
}

fn flow_style_at(
    source: usize,
    styles: &[InlineFlowStyle],
    runs: &[InlineFlowRun],
) -> Result<InlineFlowStyle, PreparationError> {
    let index = runs.partition_point(|run| run.bytes().end as usize <= source);
    let run = runs
        .get(index)
        .filter(|run| {
            let bytes = run.bytes();
            bytes.start as usize <= source && source < bytes.end as usize
        })
        .ok_or_else(PreparationError::invalid_output)?;
    styles
        .get(run.style().index())
        .copied()
        .ok_or_else(PreparationError::invalid_output)
}

#[cfg(test)]
mod tests {
    use alloc::vec::Vec;

    use parley_engine::{Analysis, AnalysisOptions, Analyzer};

    use super::collect_joining_units;
    use crate::interaction::collect_analysis_units;

    #[test]
    fn joining_units_come_from_unicode_data_not_script_lists() {
        let text = "Aب\u{64e}𐫍";
        let mut analysis = Analysis::new();
        Analyzer::new().analyze(text, &AnalysisOptions::default(), &mut analysis);
        let units = collect_analysis_units(text, &analysis).expect("analysis units are valid");
        let mut joining = Vec::new();

        collect_joining_units(text, &analysis, &units, &mut joining)
            .expect("joining facts align with analysis units");

        assert_eq!(units.len(), 3);
        assert_eq!(joining, [false, true, true]);
    }
}
