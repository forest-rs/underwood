// Copyright 2026 the Underwood Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::borrow::Cow;
use alloc::sync::Arc;
use alloc::vec::Vec;

use crate::{
    Brush, FontFamily, FontFamilyName, FontFeature, FontStyle, FontVariation, FontWeight,
    FontWidth, Language, StyleError, StyleErrorKind, TextId,
};

/// Dense caller-defined index into a [`PaintTable`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PaintSlot(pub(crate) u32);

impl PaintSlot {
    /// Creates a paint slot. Its presence is checked against a paint table at use time.
    #[must_use]
    pub const fn new(index: u32) -> Self {
        Self(index)
    }

    pub(crate) const fn index(self) -> u32 {
        self.0
    }
}

/// Complete computed values that can change text shaping.
///
/// Settings are owned and canonicalized by OpenType tag. When the same tag is
/// supplied more than once, the last value wins. Unsupported settings are a
/// deterministic no-op for a selected font.
#[derive(Clone, Debug, PartialEq)]
pub struct ShapingStyle {
    font_family: FontFamily<'static>,
    font_weight: FontWeight,
    font_width: FontWidth,
    font_style: FontStyle,
    font_size: f32,
    language: Option<Language>,
    features: Arc<[FontFeature]>,
    variations: Arc<[FontVariation]>,
}

impl ShapingStyle {
    /// Creates shaping values with an owned canonical family request and a
    /// finite, strictly positive font size.
    pub fn new(font_family: FontFamily<'_>, font_size: f32) -> Result<Self, StyleError> {
        if !font_size.is_finite() || font_size <= 0.0 {
            return Err(StyleError::new(StyleErrorKind::InvalidNumber));
        }
        Ok(Self {
            font_family: canonical_font_family(font_family)?,
            font_weight: FontWeight::NORMAL,
            font_width: FontWidth::NORMAL,
            font_style: FontStyle::Normal,
            font_size,
            language: None,
            features: Arc::from([]),
            variations: Arc::from([]),
        })
    }

    /// Returns a copy with a new owned canonical family request.
    pub fn with_font_family(mut self, font_family: FontFamily<'_>) -> Result<Self, StyleError> {
        self.font_family = canonical_font_family(font_family)?;
        Ok(self)
    }

    /// Returns a copy with a finite, strictly positive font weight request.
    pub fn with_font_weight(mut self, font_weight: FontWeight) -> Result<Self, StyleError> {
        if !font_weight.value().is_finite() || font_weight.value() <= 0.0 {
            return Err(StyleError::new(StyleErrorKind::InvalidNumber));
        }
        self.font_weight = font_weight;
        Ok(self)
    }

    /// Returns a copy with a finite, strictly positive font width request.
    pub fn with_font_width(mut self, font_width: FontWidth) -> Result<Self, StyleError> {
        if !font_width.ratio().is_finite() || font_width.ratio() <= 0.0 {
            return Err(StyleError::new(StyleErrorKind::InvalidNumber));
        }
        self.font_width = font_width;
        Ok(self)
    }

    /// Returns a copy with a finite font style request.
    pub fn with_font_style(mut self, font_style: FontStyle) -> Result<Self, StyleError> {
        if matches!(font_style, FontStyle::Oblique(Some(angle)) if !angle.is_finite()) {
            return Err(StyleError::new(StyleErrorKind::InvalidNumber));
        }
        self.font_style = font_style;
        Ok(self)
    }

    /// Returns a copy with a shaping language.
    #[must_use]
    pub fn with_language(mut self, language: Option<Language>) -> Self {
        self.language = language;
        self
    }

    /// Returns a copy with canonicalized OpenType feature settings.
    #[must_use]
    pub fn with_features(mut self, features: impl IntoIterator<Item = FontFeature>) -> Self {
        self.features = canonical_features(features);
        self
    }

    /// Returns a copy with validated, canonicalized variation coordinates.
    pub fn with_variations(
        mut self,
        variations: impl IntoIterator<Item = FontVariation>,
    ) -> Result<Self, StyleError> {
        self.variations = canonical_variations(variations)?;
        Ok(self)
    }

    /// Returns the ordered family request.
    #[must_use]
    pub const fn font_family(&self) -> &FontFamily<'static> {
        &self.font_family
    }

    /// Returns the requested font weight.
    #[must_use]
    pub const fn font_weight(&self) -> FontWeight {
        self.font_weight
    }

    /// Returns the requested font width.
    #[must_use]
    pub const fn font_width(&self) -> FontWidth {
        self.font_width
    }

    /// Returns the requested font style.
    #[must_use]
    pub const fn font_style(&self) -> FontStyle {
        self.font_style
    }

    /// Returns the font size in scene units.
    #[must_use]
    pub const fn font_size(&self) -> f32 {
        self.font_size
    }

    /// Returns the shaping language.
    #[must_use]
    pub const fn language(&self) -> Option<Language> {
        self.language
    }

    /// Returns canonicalized OpenType feature settings.
    #[must_use]
    pub fn features(&self) -> &[FontFeature] {
        &self.features
    }

    /// Returns canonicalized variable-font coordinates.
    #[must_use]
    pub fn variations(&self) -> &[FontVariation] {
        &self.variations
    }
}

fn canonical_font_family(font_family: FontFamily<'_>) -> Result<FontFamily<'static>, StyleError> {
    let names: Vec<FontFamilyName<'static>> = match font_family {
        FontFamily::Source(source) => FontFamilyName::parse_css_list(source.as_ref())
            .map(|name| name.map(FontFamilyName::into_owned))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| StyleError::new(StyleErrorKind::InvalidFontFamily))?,
        FontFamily::Single(name) => alloc::vec![name.into_owned()],
        FontFamily::List(names) => names
            .iter()
            .cloned()
            .map(FontFamilyName::into_owned)
            .collect(),
    };
    if names.is_empty()
        || names
            .iter()
            .any(|name| matches!(name, FontFamilyName::Named(name) if name.trim().is_empty()))
        || names
            .iter()
            .enumerate()
            .any(|(index, name)| names[..index].contains(name))
    {
        return Err(StyleError::new(StyleErrorKind::InvalidFontFamily));
    }
    let mut names = names.into_iter();
    let first = names
        .next()
        .ok_or_else(|| StyleError::new(StyleErrorKind::InvalidFontFamily))?;
    let remaining: Vec<_> = names.collect();
    if remaining.is_empty() {
        Ok(FontFamily::Single(first))
    } else {
        let mut canonical = Vec::with_capacity(remaining.len() + 1);
        canonical.push(first);
        canonical.extend(remaining);
        Ok(FontFamily::List(Cow::Owned(canonical)))
    }
}

fn canonical_features(features: impl IntoIterator<Item = FontFeature>) -> Arc<[FontFeature]> {
    let mut input: Vec<_> = features.into_iter().collect();
    let mut canonical = Vec::with_capacity(input.len());
    while let Some(feature) = input.pop() {
        if !canonical
            .iter()
            .any(|candidate: &FontFeature| candidate.tag == feature.tag)
        {
            canonical.push(feature);
        }
    }
    canonical.sort_by_key(|feature| feature.tag);
    canonical.into()
}

fn canonical_variations(
    variations: impl IntoIterator<Item = FontVariation>,
) -> Result<Arc<[FontVariation]>, StyleError> {
    let mut input: Vec<_> = variations.into_iter().collect();
    if input.iter().any(|variation| !variation.value.is_finite()) {
        return Err(StyleError::new(StyleErrorKind::InvalidNumber));
    }
    let mut canonical = Vec::with_capacity(input.len());
    while let Some(mut variation) = input.pop() {
        if variation.value == 0.0 {
            variation.value = 0.0;
        }
        if !canonical
            .iter()
            .any(|candidate: &FontVariation| candidate.tag == variation.tag)
        {
            canonical.push(variation);
        }
    }
    canonical.sort_by_key(|variation| variation.tag);
    Ok(canonical.into())
}

/// Computed line height as a multiple of the shaping font size.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LineHeight(f32);

impl LineHeight {
    /// Underwood's current normal line-height multiplier.
    pub const NORMAL: Self = Self(1.25);

    /// Validates a finite, strictly positive multiplier.
    pub fn from_multiplier(multiplier: f32) -> Result<Self, StyleError> {
        if !multiplier.is_finite() || multiplier <= 0.0 {
            return Err(StyleError::new(StyleErrorKind::InvalidNumber));
        }
        Ok(Self(multiplier))
    }

    /// Returns the multiplier applied to the shaping font size.
    #[must_use]
    pub const fn multiplier(self) -> f32 {
        self.0
    }
}

impl Default for LineHeight {
    fn default() -> Self {
        Self::NORMAL
    }
}

/// Complete computed values consumed only by inline flow and geometry.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct InlineFlowStyle {
    line_height: LineHeight,
}

impl InlineFlowStyle {
    /// Creates inline-flow values from a validated line height.
    #[must_use]
    pub const fn new(line_height: LineHeight) -> Self {
        Self { line_height }
    }

    /// Returns the computed line height.
    #[must_use]
    pub const fn line_height(self) -> LineHeight {
        self.line_height
    }
}

/// One complete computed inline style.
#[derive(Clone, Debug, PartialEq)]
pub struct ComputedInlineStyle {
    shaping: ShapingStyle,
    inline_flow: InlineFlowStyle,
    paint: PaintSlot,
}

impl ComputedInlineStyle {
    /// Joins the three independently invalidated style partitions.
    #[must_use]
    pub fn new(shaping: ShapingStyle, inline_flow: InlineFlowStyle, paint: PaintSlot) -> Self {
        Self {
            shaping,
            inline_flow,
            paint,
        }
    }

    /// Returns a copy with new shaping values.
    #[must_use]
    pub fn with_shaping(mut self, shaping: ShapingStyle) -> Self {
        self.shaping = shaping;
        self
    }

    /// Returns a copy with new inline-flow values.
    #[must_use]
    pub fn with_inline_flow(mut self, inline_flow: InlineFlowStyle) -> Self {
        self.inline_flow = inline_flow;
        self
    }

    /// Returns a copy with a new paint slot.
    #[must_use]
    pub fn with_paint(mut self, paint: PaintSlot) -> Self {
        self.paint = paint;
        self
    }

    /// Returns the shaping partition.
    #[must_use]
    pub const fn shaping(&self) -> &ShapingStyle {
        &self.shaping
    }

    /// Returns the inline-flow partition.
    #[must_use]
    pub const fn inline_flow(&self) -> InlineFlowStyle {
        self.inline_flow
    }

    /// Returns the paint slot.
    #[must_use]
    pub const fn paint(&self) -> PaintSlot {
        self.paint
    }
}

/// Complete per-leaf computed styles over one default style.
#[derive(Clone, Debug)]
pub struct StyleMap {
    pub(crate) default: ComputedInlineStyle,
    styles: Vec<(TextId, ComputedInlineStyle)>,
}

impl StyleMap {
    /// Creates a style map with no per-leaf overrides.
    #[must_use]
    pub fn new(default: ComputedInlineStyle) -> Self {
        Self {
            default,
            styles: Vec::new(),
        }
    }

    /// Assigns one complete style to a text identity.
    pub fn set(&mut self, text: TextId, style: ComputedInlineStyle) {
        if let Some((_, current)) = self.styles.iter_mut().find(|(id, _)| *id == text) {
            *current = style;
        } else {
            self.styles.push((text, style));
        }
    }

    /// Returns the assigned style or the default when no override exists.
    #[must_use]
    pub fn style_for(&self, text: TextId) -> &ComputedInlineStyle {
        self.styles
            .iter()
            .find_map(|(id, style)| (*id == text).then_some(style))
            .unwrap_or(&self.default)
    }

    /// Returns the default style.
    #[must_use]
    pub const fn default_style(&self) -> &ComputedInlineStyle {
        &self.default
    }

    pub(crate) fn overrides(&self) -> &[(TextId, ComputedInlineStyle)] {
        &self.styles
    }
}

/// Immutable mapping from paint slots to renderer-neutral brushes.
#[derive(Clone, Debug, PartialEq)]
pub struct PaintTable {
    values: Arc<[Brush]>,
}

#[cfg(test)]
mod tests {
    use alloc::borrow::Cow;

    use parlance::{
        FontFamily, FontFamilyName, FontFeature, FontStyle, FontVariation, FontWeight, FontWidth,
        Tag,
    };

    use super::{LineHeight, ShapingStyle};

    #[test]
    fn shaping_numbers_are_validated() {
        assert!(ShapingStyle::new(FontFamily::named("Test"), 0.0).is_err());
        assert!(ShapingStyle::new(FontFamily::named("Test"), f32::NAN).is_err());
        assert!(
            ShapingStyle::new(FontFamily::named("Test"), 16.0)
                .expect("base request is valid")
                .with_font_weight(FontWeight::new(f32::NAN))
                .is_err()
        );
        assert!(
            ShapingStyle::new(FontFamily::named("Test"), 16.0)
                .expect("base request is valid")
                .with_font_width(FontWidth::from_ratio(0.0))
                .is_err()
        );
        assert!(
            ShapingStyle::new(FontFamily::named("Test"), 16.0)
                .expect("base request is valid")
                .with_font_style(FontStyle::Oblique(Some(f32::INFINITY)))
                .is_err()
        );
        assert!(LineHeight::from_multiplier(-1.0).is_err());
        assert!(LineHeight::from_multiplier(f32::INFINITY).is_err());
    }

    #[test]
    fn font_families_are_owned_parsed_and_canonical() {
        let style = ShapingStyle::new(FontFamily::from("Roboto Flex, sans-serif"), 16.0)
            .expect("CSS family source is valid");
        assert_eq!(
            style.font_family(),
            &FontFamily::List(Cow::Owned(alloc::vec![
                FontFamilyName::named("Roboto Flex").into_owned(),
                FontFamilyName::Generic(parlance::GenericFamily::SansSerif),
            ]))
        );
        assert!(ShapingStyle::new(FontFamily::from("Roboto Flex,, serif"), 16.0).is_err());
        assert!(
            ShapingStyle::new(
                FontFamily::List(Cow::Owned(alloc::vec![
                    FontFamilyName::named("Roboto Flex").into_owned(),
                    FontFamilyName::named("Roboto Flex").into_owned(),
                ])),
                16.0,
            )
            .is_err()
        );
    }

    #[test]
    fn feature_settings_are_canonical_and_last_wins() {
        let liga = Tag::new(b"liga");
        let kern = Tag::new(b"kern");
        let style = ShapingStyle::new(FontFamily::named("Test"), 16.0)
            .expect("font size is valid")
            .with_features([
                FontFeature::new(liga, 1),
                FontFeature::new(kern, 0),
                FontFeature::new(liga, 0),
            ]);
        assert_eq!(
            style.features(),
            &[FontFeature::new(kern, 0), FontFeature::new(liga, 0)]
        );
    }

    #[test]
    fn variation_settings_are_canonical_validated_and_last_wins() {
        let opsz = Tag::new(b"opsz");
        let wght = Tag::new(b"wght");
        let style = ShapingStyle::new(FontFamily::named("Test"), 16.0)
            .expect("font size is valid")
            .with_variations([
                FontVariation::new(wght, 400.0),
                FontVariation::new(opsz, 16.0),
                FontVariation::new(wght, 700.0),
            ])
            .expect("coordinates are finite");
        assert_eq!(
            style.variations(),
            &[
                FontVariation::new(opsz, 16.0),
                FontVariation::new(wght, 700.0),
            ]
        );
        assert!(
            ShapingStyle::new(FontFamily::named("Test"), 16.0)
                .expect("font size is valid")
                .with_variations([FontVariation::new(wght, f32::NAN)])
                .is_err()
        );
    }
}

impl PaintTable {
    /// Collects brushes in slot-index order.
    #[must_use]
    pub fn from_brushes(values: impl IntoIterator<Item = Brush>) -> Self {
        Self {
            values: values.into_iter().collect::<Vec<_>>().into(),
        }
    }

    /// Returns a copy with one slot replaced.
    pub fn with_brush(&self, slot: PaintSlot, value: Brush) -> Result<Self, StyleError> {
        let mut values = self.values.to_vec();
        let target = values
            .get_mut(slot.index() as usize)
            .ok_or_else(|| StyleError::for_paint(StyleErrorKind::AbsentPaintSlot, slot))?;
        *target = value;
        Ok(Self {
            values: values.into(),
        })
    }

    /// Returns the brush at a slot, if present.
    #[must_use]
    pub fn brush(&self, slot: PaintSlot) -> Option<&Brush> {
        self.values.get(slot.index() as usize)
    }

    /// Returns the number of paint slots.
    #[must_use]
    pub fn len(&self) -> usize {
        self.values.len()
    }

    /// Returns whether the table contains no brushes.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }
}

/// A finite, strictly positive first-slice layout width.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FiniteWidth(pub(crate) f64);

impl FiniteWidth {
    /// Validates a finite, strictly positive width.
    pub fn new(width: f64) -> Result<Self, crate::SceneError> {
        if !width.is_finite() || width <= 0.0 {
            return Err(crate::SceneError::new(crate::SceneErrorKind::InvalidWidth));
        }
        Ok(Self(width))
    }
}

/// Borrowed inputs for one scene preparation.
#[derive(Clone, Copy, Debug)]
pub struct SceneRequest<'a> {
    pub(crate) width: FiniteWidth,
    pub(crate) styles: &'a StyleMap,
    pub(crate) paint: &'a PaintTable,
}

impl<'a> SceneRequest<'a> {
    /// Creates a request from validated width, style, and paint values.
    #[must_use]
    pub fn new(width: FiniteWidth, styles: &'a StyleMap, paint: &'a PaintTable) -> Self {
        Self {
            width,
            styles,
            paint,
        }
    }
}
