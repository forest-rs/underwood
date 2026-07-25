// Copyright 2026 the Underwood Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Showcase-owned page geometry lowered into public Underwood regions.

use underwood::{FloatSide, FlowRegion, Rect, RegionFloat, RegionFlow, SceneError, Size};

const WIDE_PAGE_MINIMUM: f64 = 720.0;
const RETRY_PROBE_HEIGHT: f64 = 11.0;
const FLOAT_GUTTER: f64 = 13.0;
const CONTINUATION_HEIGHT: f64 = 4_000.0;

/// Presentation kind for one obstacle that also participates in text flow.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PageDecorationKind {
    /// A physical right float in the hero region.
    HeroFloat,
    /// A caller-authored exclusion inside the second column.
    ColumnExclusion,
}

/// One decorative rectangle painted inside a larger text-flow obstacle.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct PageDecoration {
    pub(crate) kind: PageDecorationKind,
    pub(crate) bounds: Rect,
    pub(crate) flow_bounds: Rect,
}

/// One responsive living-page plan and its exact compiled region flow.
#[derive(Clone, Debug)]
pub(crate) struct LivingPagePlan {
    flow: RegionFlow,
    decorations: Vec<PageDecoration>,
    column_regions: Vec<Rect>,
}

impl LivingPagePlan {
    /// Builds a narrow single-region page or a wide hero-plus-columns page.
    pub(crate) fn new(content_width: f64) -> Result<Self, SceneError> {
        let retry_probe = FlowRegion::new(Rect::new(0.0, 0.0, content_width, RETRY_PROBE_HEIGHT))?;
        if content_width < WIDE_PAGE_MINIMUM {
            return Self::narrow(content_width, retry_probe);
        }
        Self::wide(content_width, retry_probe)
    }

    /// Returns the exact flow consumed by scene preparation.
    pub(crate) const fn flow(&self) -> &RegionFlow {
        &self.flow
    }

    /// Returns decorations whose bounds are also real float/exclusion inputs.
    pub(crate) fn decorations(&self) -> &[PageDecoration] {
        &self.decorations
    }

    /// Returns visible column bounds, excluding the retry probe and hero.
    pub(crate) fn column_regions(&self) -> &[Rect] {
        &self.column_regions
    }

    /// Returns the off-page continuation region that prevents ordinary overflow from failing.
    #[cfg(test)]
    pub(crate) fn continuation_region(&self) -> Option<usize> {
        (!self.column_regions.is_empty()).then(|| self.flow.regions().count() - 1)
    }

    fn narrow(content_width: f64, retry_probe: FlowRegion) -> Result<Self, SceneError> {
        let float_width = (content_width * 0.32).clamp(64.0, 112.0);
        let float_height = 92.0;
        let float_offset = 148.0;
        let bounds = Rect::new(
            content_width - float_width,
            float_offset,
            content_width,
            float_offset + float_height,
        );
        let page = FlowRegion::new(Rect::new(0.0, 0.0, content_width, 4_000.0))?.with_floats([
            RegionFloat::new(
                FloatSide::Right,
                float_offset,
                Size::new(float_width, float_height),
            )?,
        ])?;
        Ok(Self {
            flow: RegionFlow::new([retry_probe, page])?,
            decorations: vec![PageDecoration {
                kind: PageDecorationKind::HeroFloat,
                bounds: inset(bounds, FLOAT_GUTTER),
                flow_bounds: bounds,
            }],
            column_regions: Vec::new(),
        })
    }

    fn wide(content_width: f64, retry_probe: FlowRegion) -> Result<Self, SceneError> {
        let hero_height = 210.0;
        let float_width = (content_width * 0.255).clamp(184.0, 244.0);
        let float_height = 110.0;
        let float_offset = 100.0;
        let hero_float_bounds = Rect::new(
            content_width - float_width,
            float_offset,
            content_width,
            float_offset + float_height,
        );
        let hero =
            FlowRegion::new(Rect::new(0.0, 0.0, content_width, hero_height))?.with_floats([
                RegionFloat::new(
                    FloatSide::Right,
                    float_offset,
                    Size::new(float_width, float_height),
                )?,
            ])?;

        let column_gap = 24.0;
        let column_count = if content_width >= 840.0 { 3 } else { 2 };
        let column_height = if column_count == 3 { 300.0 } else { 650.0 };
        let total_gap = column_gap * f64::from(column_count - 1);
        let column_width = (content_width - total_gap) / f64::from(column_count);
        let column_y = hero_height + 14.0;
        let column_bounds = |index: u8| {
            let x0 = f64::from(index) * (column_width + column_gap);
            Rect::new(x0, column_y, x0 + column_width, column_y + column_height)
        };
        let first_column = column_bounds(0);
        let second_column = column_bounds(1);
        let exclusion_flow_bounds = Rect::new(
            second_column.x0,
            column_y + 104.0,
            second_column.x0 + 88.0,
            column_y + 200.0,
        );
        let first = FlowRegion::new(first_column)?;
        let second = FlowRegion::new(second_column)?.with_exclusions([exclusion_flow_bounds])?;
        let mut regions = vec![retry_probe, hero, first, second];
        let mut column_regions = vec![first_column, second_column];
        if column_count == 3 {
            let third_column = column_bounds(2);
            regions.push(FlowRegion::new(third_column)?);
            column_regions.push(third_column);
        }
        let continuation_y = column_y + column_height + column_gap;
        regions.push(FlowRegion::new(Rect::new(
            0.0,
            continuation_y,
            content_width,
            continuation_y + CONTINUATION_HEIGHT,
        ))?);

        Ok(Self {
            flow: RegionFlow::new(regions)?,
            decorations: vec![
                PageDecoration {
                    kind: PageDecorationKind::HeroFloat,
                    bounds: inset(hero_float_bounds, FLOAT_GUTTER),
                    flow_bounds: hero_float_bounds,
                },
                PageDecoration {
                    kind: PageDecorationKind::ColumnExclusion,
                    bounds: inset(exclusion_flow_bounds, FLOAT_GUTTER),
                    flow_bounds: exclusion_flow_bounds,
                },
            ],
            column_regions,
        })
    }
}

fn inset(bounds: Rect, amount: f64) -> Rect {
    Rect::new(
        bounds.x0 + amount,
        bounds.y0 + amount,
        bounds.x1 - amount,
        bounds.y1 - amount,
    )
}

#[cfg(test)]
mod tests {
    use super::{LivingPagePlan, PageDecorationKind, RETRY_PROBE_HEIGHT};

    #[test]
    fn wide_page_is_a_retry_probe_hero_and_three_real_columns() {
        let plan = LivingPagePlan::new(960.0).expect("wide plan is valid");
        let regions: Vec<_> = plan.flow().regions().collect();

        assert_eq!(regions.len(), 6);
        assert_eq!(regions[0].bounds().height(), RETRY_PROBE_HEIGHT);
        assert_eq!(plan.column_regions().len(), 3);
        assert!(plan.column_regions().len() > 1);
        assert_eq!(
            plan.decorations()
                .iter()
                .map(|decoration| decoration.kind)
                .collect::<Vec<_>>(),
            [
                PageDecorationKind::HeroFloat,
                PageDecorationKind::ColumnExclusion,
            ]
        );
        assert_eq!(regions[1].floats().len(), 1);
        assert_eq!(
            regions[3].exclusions(),
            &[plan.decorations()[1].flow_bounds]
        );
        assert!(
            plan.decorations()
                .iter()
                .all(|decoration| decoration.flow_bounds.contains_rect(decoration.bounds))
        );
        assert_eq!(plan.column_regions()[0].y0, plan.column_regions()[1].y0);
        assert_eq!(
            plan.column_regions()[0].height(),
            plan.column_regions()[1].height()
        );
        assert!(plan.column_regions()[0].x1 < plan.column_regions()[1].x0);
        let continuation = plan
            .continuation_region()
            .expect("wide pages have an overflow continuation");
        assert_eq!(continuation, regions.len() - 1);
        assert!(regions[continuation].bounds().y0 > plan.column_regions()[0].y1);
    }

    #[test]
    fn medium_page_uses_two_equal_bounded_columns() {
        let plan = LivingPagePlan::new(760.0).expect("medium plan is valid");
        let regions: Vec<_> = plan.flow().regions().collect();

        assert_eq!(regions.len(), 5);
        assert_eq!(plan.column_regions().len(), 2);
        assert_eq!(
            plan.column_regions()[0].height(),
            plan.column_regions()[1].height()
        );
        assert!(plan.column_regions()[0].x1 < plan.column_regions()[1].x0);
    }

    #[test]
    fn narrow_page_retains_the_retry_and_float_without_fake_columns() {
        let plan = LivingPagePlan::new(420.0).expect("narrow plan is valid");
        let regions: Vec<_> = plan.flow().regions().collect();

        assert_eq!(regions.len(), 2);
        assert!(plan.column_regions().is_empty());
        assert_eq!(plan.continuation_region(), None);
        assert_eq!(regions[1].floats().len(), 1);
        assert_eq!(plan.decorations()[0].kind, PageDecorationKind::HeroFloat);
    }
}
