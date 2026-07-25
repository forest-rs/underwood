// Copyright 2026 the Underwood Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::sync::Arc;
use alloc::vec::Vec;

use crate::{
    BaseDirection, Brush, FontFamily, FontFamilyName, FontFeature, FontStyle, FontVariation,
    FontWeight, FontWidth, Language, OverflowWrap, ParagraphId, StyleError, StyleErrorKind, TextId,
    TextWrapMode, WhitespaceCollapse, WordBreak,
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
    font_families: Arc<[FontFamilyName<'static>]>,
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
            font_families: canonical_font_family(font_family)?,
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
        self.font_families = canonical_font_family(font_family)?;
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

    /// Returns the ordered canonical family names.
    #[must_use]
    pub fn font_families(&self) -> &[FontFamilyName<'static>] {
        &self.font_families
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

fn canonical_font_family(
    font_family: FontFamily<'_>,
) -> Result<Arc<[FontFamilyName<'static>]>, StyleError> {
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
    Ok(names.into())
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

/// Basis used to resolve a [`LineHeight`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LineHeightBasis {
    /// Scale the selected font's preferred ascent, descent, and leading.
    Metrics,
    /// Scale the computed font size.
    FontSize,
    /// Use an absolute height in scene units.
    Absolute,
}

/// Validated computed line-height policy.
///
/// Relative values are resolved independently for every shaped run. Absolute
/// values use scene units and remain independent of font size.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LineHeight {
    basis: LineHeightBasis,
    value: f32,
}

impl LineHeight {
    /// Normal line height from the selected font's preferred metrics.
    pub const NORMAL: Self = Self {
        basis: LineHeightBasis::Metrics,
        value: 1.0,
    };

    /// Validates a finite, strictly positive font-size-relative factor.
    ///
    /// This preserves the pre-stable spelling used before Underwood represented
    /// all three line-height bases. New callers should use
    /// [`Self::font_size_relative`].
    pub fn from_multiplier(multiplier: f32) -> Result<Self, StyleError> {
        Self::font_size_relative(multiplier)
    }

    /// Validates a factor applied to preferred font metrics.
    pub fn metrics_relative(factor: f32) -> Result<Self, StyleError> {
        Self::new(LineHeightBasis::Metrics, factor)
    }

    /// Validates a factor applied to the computed font size.
    pub fn font_size_relative(factor: f32) -> Result<Self, StyleError> {
        Self::new(LineHeightBasis::FontSize, factor)
    }

    /// Validates an absolute line height in scene units.
    pub fn absolute(height: f32) -> Result<Self, StyleError> {
        Self::new(LineHeightBasis::Absolute, height)
    }

    fn new(basis: LineHeightBasis, value: f32) -> Result<Self, StyleError> {
        if !value.is_finite() || value <= 0.0 {
            return Err(StyleError::new(StyleErrorKind::InvalidNumber));
        }
        Ok(Self { basis, value })
    }

    /// Returns how this value is resolved.
    #[must_use]
    pub const fn basis(self) -> LineHeightBasis {
        self.basis
    }

    /// Returns the validated factor or absolute height.
    #[must_use]
    pub const fn value(self) -> f32 {
        self.value
    }

    /// Resolves the line height from a font size and preferred metrics height.
    #[must_use]
    pub fn resolve(self, font_size: f32, metrics_height: f32) -> f32 {
        match self.basis {
            LineHeightBasis::Metrics => metrics_height * self.value,
            LineHeightBasis::FontSize => font_size * self.value,
            LineHeightBasis::Absolute => self.value,
        }
    }
}

impl Default for LineHeight {
    fn default() -> Self {
        Self::NORMAL
    }
}

/// Computed extra spacing in scene units.
///
/// Negative values are permitted because tightening is meaningful text policy.
/// Backends may clamp a resulting advance to preserve finite, non-negative
/// geometry.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TextSpacing {
    letter: f32,
    word: f32,
}

impl TextSpacing {
    /// No additional letter or word spacing.
    pub const NORMAL: Self = Self {
        letter: 0.0,
        word: 0.0,
    };

    /// Creates finite extra letter and word spacing in scene units.
    pub fn new(letter: f32, word: f32) -> Result<Self, StyleError> {
        if !letter.is_finite() || !word.is_finite() {
            return Err(StyleError::new(StyleErrorKind::InvalidNumber));
        }
        Ok(Self { letter, word })
    }

    /// Returns additional spacing between eligible typographic units.
    #[must_use]
    pub const fn letter(self) -> f32 {
        self.letter
    }

    /// Returns additional spacing at eligible word separators.
    #[must_use]
    pub const fn word(self) -> f32 {
        self.word
    }
}

/// Complete computed values consumed by Unicode analysis.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AnalysisStyle {
    word_break: WordBreak,
}

impl AnalysisStyle {
    /// Creates analysis values from authored word-breaking policy.
    #[must_use]
    pub const fn new(word_break: WordBreak) -> Self {
        Self { word_break }
    }

    /// Returns the word-breaking policy supplied to Unicode analysis.
    #[must_use]
    pub const fn word_break(self) -> WordBreak {
        self.word_break
    }
}

/// Complete computed values consumed only by inline flow and geometry.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct InlineFlowStyle {
    line_height: LineHeight,
    spacing: TextSpacing,
    overflow_wrap: OverflowWrap,
    text_wrap_mode: TextWrapMode,
}

impl InlineFlowStyle {
    /// Creates inline-flow values from a validated line height.
    #[must_use]
    pub const fn new(line_height: LineHeight) -> Self {
        Self {
            line_height,
            spacing: TextSpacing::NORMAL,
            overflow_wrap: OverflowWrap::Normal,
            text_wrap_mode: TextWrapMode::Wrap,
        }
    }

    /// Returns a copy with computed extra letter and word spacing.
    #[must_use]
    pub const fn with_spacing(mut self, spacing: TextSpacing) -> Self {
        self.spacing = spacing;
        self
    }

    /// Returns a copy with emergency line-breaking policy.
    #[must_use]
    pub const fn with_overflow_wrap(mut self, overflow_wrap: OverflowWrap) -> Self {
        self.overflow_wrap = overflow_wrap;
        self
    }

    /// Returns a copy with soft wrapping enabled or disabled.
    #[must_use]
    pub const fn with_text_wrap_mode(mut self, text_wrap_mode: TextWrapMode) -> Self {
        self.text_wrap_mode = text_wrap_mode;
        self
    }

    /// Returns the computed line height.
    #[must_use]
    pub const fn line_height(self) -> LineHeight {
        self.line_height
    }

    /// Returns computed extra letter and word spacing.
    #[must_use]
    pub const fn spacing(self) -> TextSpacing {
        self.spacing
    }

    /// Returns emergency line-breaking policy.
    #[must_use]
    pub const fn overflow_wrap(self) -> OverflowWrap {
        self.overflow_wrap
    }

    /// Returns whether soft wrapping is enabled.
    #[must_use]
    pub const fn text_wrap_mode(self) -> TextWrapMode {
        self.text_wrap_mode
    }
}

/// One complete computed inline style.
#[derive(Clone, Debug, PartialEq)]
pub struct ComputedInlineStyle {
    analysis: AnalysisStyle,
    shaping: ShapingStyle,
    inline_flow: InlineFlowStyle,
    paint: PaintSlot,
}

impl ComputedInlineStyle {
    /// Joins the three independently invalidated style partitions.
    #[must_use]
    pub fn new(shaping: ShapingStyle, inline_flow: InlineFlowStyle, paint: PaintSlot) -> Self {
        Self {
            analysis: AnalysisStyle::default(),
            shaping,
            inline_flow,
            paint,
        }
    }

    /// Returns a copy with new Unicode-analysis values.
    #[must_use]
    pub fn with_analysis(mut self, analysis: AnalysisStyle) -> Self {
        self.analysis = analysis;
        self
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

    /// Returns the Unicode-analysis partition.
    #[must_use]
    pub const fn analysis(&self) -> AnalysisStyle {
        self.analysis
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

/// Complete computed values that apply to one paragraph as a whole.
///
/// Paragraph values are kept separate from [`ShapingStyle`] because they
/// affect analysis and line formation rather than inline font shaping.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ParagraphStyle {
    base_direction: BaseDirection,
    whitespace_collapse: WhitespaceCollapse,
}

impl ParagraphStyle {
    /// Automatically inferred paragraph direction and otherwise default
    /// paragraph behavior.
    pub const DEFAULT: Self = Self {
        base_direction: BaseDirection::Auto,
        whitespace_collapse: WhitespaceCollapse::Preserve,
    };

    /// Creates paragraph values with an explicit or automatically inferred
    /// base direction.
    #[must_use]
    pub const fn new(base_direction: BaseDirection) -> Self {
        Self {
            base_direction,
            whitespace_collapse: WhitespaceCollapse::Preserve,
        }
    }

    /// Returns the paragraph base direction used during Unicode analysis.
    #[must_use]
    pub const fn base_direction(self) -> BaseDirection {
        self.base_direction
    }

    /// Returns a copy with paragraph-stream whitespace processing.
    ///
    /// Collapse state crosses inline style and semantic boundaries.
    #[must_use]
    pub const fn with_whitespace_collapse(mut self, policy: WhitespaceCollapse) -> Self {
        self.whitespace_collapse = policy;
        self
    }

    /// Returns paragraph-stream whitespace processing.
    #[must_use]
    pub const fn whitespace_collapse(self) -> WhitespaceCollapse {
        self.whitespace_collapse
    }
}

impl Default for ParagraphStyle {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// Complete per-paragraph and per-leaf computed styles over default values.
#[derive(Clone, Debug)]
pub struct StyleMap {
    pub(crate) default: ComputedInlineStyle,
    styles: Vec<(TextId, ComputedInlineStyle)>,
    default_paragraph: ParagraphStyle,
    paragraph_styles: Vec<(ParagraphId, ParagraphStyle)>,
}

impl StyleMap {
    /// Creates a style map with no per-leaf overrides.
    #[must_use]
    pub fn new(default: ComputedInlineStyle) -> Self {
        Self {
            default,
            styles: Vec::new(),
            default_paragraph: ParagraphStyle::default(),
            paragraph_styles: Vec::new(),
        }
    }

    /// Returns a style map whose unassigned paragraphs use `style`.
    #[must_use]
    pub fn with_default_paragraph_style(mut self, style: ParagraphStyle) -> Self {
        self.default_paragraph = style;
        self
    }

    /// Assigns one complete style to a text identity.
    pub fn set(&mut self, text: TextId, style: ComputedInlineStyle) {
        if let Some((_, current)) = self.styles.iter_mut().find(|(id, _)| *id == text) {
            *current = style;
        } else {
            self.styles.push((text, style));
        }
    }

    /// Assigns complete paragraph-level values to one paragraph identity.
    pub fn set_paragraph_style(&mut self, paragraph: ParagraphId, style: ParagraphStyle) {
        if let Some((_, current)) = self
            .paragraph_styles
            .iter_mut()
            .find(|(id, _)| *id == paragraph)
        {
            *current = style;
        } else {
            self.paragraph_styles.push((paragraph, style));
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

    /// Returns the assigned paragraph style or the paragraph default.
    #[must_use]
    pub fn paragraph_style_for(&self, paragraph: ParagraphId) -> ParagraphStyle {
        self.paragraph_styles
            .iter()
            .find_map(|(id, style)| (*id == paragraph).then_some(*style))
            .unwrap_or(self.default_paragraph)
    }

    /// Returns the paragraph-level default.
    #[must_use]
    pub const fn default_paragraph_style(&self) -> ParagraphStyle {
        self.default_paragraph
    }

    pub(crate) fn overrides(&self) -> &[(TextId, ComputedInlineStyle)] {
        &self.styles
    }

    pub(crate) fn paragraph_overrides(&self) -> &[(ParagraphId, ParagraphStyle)] {
        &self.paragraph_styles
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

    use super::{
        AnalysisStyle, InlineFlowStyle, LineHeight, LineHeightBasis, ParagraphStyle, ShapingStyle,
        StyleMap, TextSpacing,
    };
    use crate::{
        BaseDirection, ComputedInlineStyle, Document, DocumentId, OverflowWrap, PaintSlot,
        ParagraphRole, TextWrapMode, WordBreak,
    };

    #[test]
    fn paragraph_styles_are_independent_from_inline_shaping_values() {
        let mut document = Document::new(DocumentId::from_bytes(*b"paragraph-style0"));
        let mut edit = document.edit();
        let paragraph = edit
            .append_paragraph(ParagraphRole::BODY)
            .expect("fixture paragraph is valid");
        edit.commit().expect("fixture document is valid");
        let inline = ComputedInlineStyle::new(
            ShapingStyle::new(FontFamily::named("Test"), 16.0).expect("fixture style is valid"),
            InlineFlowStyle::default(),
            PaintSlot::new(0),
        );
        let mut styles = StyleMap::new(inline);
        assert_eq!(
            styles.paragraph_style_for(paragraph),
            ParagraphStyle::DEFAULT
        );
        styles.set_paragraph_style(paragraph, ParagraphStyle::new(BaseDirection::Rtl));
        assert_eq!(
            styles.paragraph_style_for(paragraph).base_direction(),
            BaseDirection::Rtl
        );
    }

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
    fn computed_text_policy_has_validated_stage_owned_values() {
        let metrics = LineHeight::metrics_relative(1.2).expect("metrics factor is valid");
        let font_size = LineHeight::font_size_relative(1.4).expect("font-size factor is valid");
        let absolute = LineHeight::absolute(24.0).expect("absolute height is valid");
        assert_eq!(metrics.basis(), LineHeightBasis::Metrics);
        assert_eq!(font_size.basis(), LineHeightBasis::FontSize);
        assert_eq!(absolute.basis(), LineHeightBasis::Absolute);
        assert_eq!(metrics.value(), 1.2);
        assert_eq!(font_size.value(), 1.4);
        assert_eq!(absolute.value(), 24.0);
        assert!(LineHeight::metrics_relative(0.0).is_err());
        assert!(LineHeight::font_size_relative(f32::NAN).is_err());
        assert!(LineHeight::absolute(f32::INFINITY).is_err());

        let spacing = TextSpacing::new(-0.5, 2.0).expect("finite spacing is valid");
        assert_eq!(spacing.letter(), -0.5);
        assert_eq!(spacing.word(), 2.0);
        assert!(TextSpacing::new(f32::NAN, 0.0).is_err());
        assert!(TextSpacing::new(0.0, f32::INFINITY).is_err());

        let analysis = AnalysisStyle::new(WordBreak::KeepAll);
        let flow = InlineFlowStyle::new(metrics)
            .with_spacing(spacing)
            .with_overflow_wrap(OverflowWrap::Anywhere)
            .with_text_wrap_mode(TextWrapMode::NoWrap);
        assert_eq!(analysis.word_break(), WordBreak::KeepAll);
        assert_eq!(flow.spacing(), spacing);
        assert_eq!(flow.overflow_wrap(), OverflowWrap::Anywhere);
        assert_eq!(flow.text_wrap_mode(), TextWrapMode::NoWrap);

        let computed = ComputedInlineStyle::new(
            ShapingStyle::new(FontFamily::named("Test"), 16.0).expect("fixture style is valid"),
            flow,
            PaintSlot::new(0),
        )
        .with_analysis(analysis);
        assert_eq!(computed.analysis(), analysis);
    }

    #[test]
    fn font_families_are_owned_parsed_and_canonical() {
        let style = ShapingStyle::new(FontFamily::from("Roboto Flex, sans-serif"), 16.0)
            .expect("CSS family source is valid");
        assert_eq!(
            style.font_families(),
            &[
                FontFamilyName::named("Roboto Flex").into_owned(),
                FontFamilyName::Generic(parlance::GenericFamily::SansSerif),
            ]
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

    #[test]
    fn shaping_style_clones_share_owned_variable_sized_inputs() {
        let style = ShapingStyle::new(FontFamily::from("Roboto Flex, sans-serif"), 16.0)
            .expect("family request is valid")
            .with_features([FontFeature::new(Tag::new(b"liga"), 1)])
            .with_variations([FontVariation::new(Tag::new(b"wght"), 650.0)])
            .expect("variation is valid");
        let clone = style.clone();

        assert!(alloc::sync::Arc::ptr_eq(
            &style.font_families,
            &clone.font_families
        ));
        assert!(alloc::sync::Arc::ptr_eq(&style.features, &clone.features));
        assert!(alloc::sync::Arc::ptr_eq(
            &style.variations,
            &clone.variations
        ));
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

/// A finite, strictly positive constrained inline size.
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

    /// Returns the validated finite, strictly positive width.
    #[must_use]
    pub const fn get(self) -> f64 {
        self.0
    }
}

/// Inline-size constraint used for paragraph formation and intrinsic measurement.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TextConstraint {
    /// Form the narrowest width obtained by taking every legal soft break.
    MinContent,
    /// Form without soft wrapping while still honoring mandatory breaks.
    MaxContent,
    /// Greedily wrap to a finite, strictly positive maximum inline size.
    Wrap(FiniteWidth),
}

impl From<FiniteWidth> for TextConstraint {
    fn from(width: FiniteWidth) -> Self {
        Self::Wrap(width)
    }
}

/// Borrowed inputs for one scene preparation.
#[derive(Clone, Copy, Debug)]
pub struct SceneRequest<'a> {
    pub(crate) constraint: TextConstraint,
    pub(crate) styles: &'a StyleMap,
    pub(crate) paint: &'a PaintTable,
    pub(crate) region_flow: Option<&'a crate::RegionFlow>,
}

impl<'a> SceneRequest<'a> {
    /// Creates a request from a text constraint, style map, and paint table.
    #[must_use]
    pub fn new(constraint: TextConstraint, styles: &'a StyleMap, paint: &'a PaintTable) -> Self {
        Self {
            constraint,
            styles,
            paint,
            region_flow: None,
        }
    }

    /// Returns a request that fills exact slots from an immutable region flow.
    ///
    /// Region slots replace the single wrapping width for line formation.
    /// Intrinsic requests continue to use [`Self::new`] without regions.
    #[must_use]
    pub fn with_region_flow(mut self, region_flow: &'a crate::RegionFlow) -> Self {
        self.constraint = TextConstraint::Wrap(FiniteWidth(region_flow.max_inline_size()));
        self.region_flow = Some(region_flow);
        self
    }

    /// Returns the exact region policy, when one was requested.
    #[must_use]
    pub const fn region_flow(self) -> Option<&'a crate::RegionFlow> {
        self.region_flow
    }
}
