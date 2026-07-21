// Copyright 2026 the Underwood Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::sync::Arc;
use alloc::vec::Vec;

use crate::{Brush, StyleError, StyleErrorKind, TextId};

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

/// Shaping and default-paint values for first-slice text.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextStyle {
    pub(crate) font_size: f32,
    pub(crate) paint: PaintSlot,
}

impl TextStyle {
    /// Creates a text style with a finite, strictly positive font size.
    pub fn new(font_size: f32, paint: PaintSlot) -> Result<Self, StyleError> {
        if !font_size.is_finite() || font_size <= 0.0 {
            return Err(StyleError::new(StyleErrorKind::InvalidNumber));
        }
        Ok(Self { font_size, paint })
    }
}

/// Per-leaf style overrides over one default style.
#[derive(Clone, Debug)]
pub struct StyleMap {
    pub(crate) default: TextStyle,
    paint: Vec<(TextId, PaintSlot)>,
}

impl StyleMap {
    /// Creates a style map with no per-leaf overrides.
    #[must_use]
    pub fn new(default: TextStyle) -> Self {
        Self {
            default,
            paint: Vec::new(),
        }
    }

    /// Sets the paint slot for one text identity.
    pub fn set_paint(&mut self, text: TextId, paint: PaintSlot) -> Result<(), StyleError> {
        if let Some((_, current)) = self.paint.iter_mut().find(|(id, _)| *id == text) {
            *current = paint;
        } else {
            self.paint.push((text, paint));
        }
        Ok(())
    }

    pub(crate) fn paint_for(&self, text: TextId) -> PaintSlot {
        self.paint
            .iter()
            .find_map(|(id, paint)| (*id == text).then_some(*paint))
            .unwrap_or(self.default.paint)
    }

    pub(crate) fn overrides(&self) -> &[(TextId, PaintSlot)] {
        &self.paint
    }
}

/// Immutable mapping from paint slots to renderer-neutral brushes.
#[derive(Clone, Debug, PartialEq)]
pub struct PaintTable {
    values: Arc<[Brush]>,
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
