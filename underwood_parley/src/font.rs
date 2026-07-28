// Copyright 2026 the Underwood Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Immutable font resources and adapter-construction errors.
//!
//! This module owns Fontique catalog construction and validation; it explicitly
//! does not own paragraph shaping or fallback-selection policy during shaping.

use alloc::{string::String, sync::Arc, vec::Vec};
use core::fmt;

use fontique::{
    Blob, Collection, CollectionOptions, FallbackKey, FontInfo, SourceCache, SourceId, SourceInfo,
    SourceKind,
};
use underwood::{GenericFamily, Language, Script};

/// Owned validated font bytes and a face within them.
#[derive(Clone)]
pub struct Font {
    blob: Blob<u8>,
    index: u32,
}

impl fmt::Debug for Font {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Font")
            .field("index", &self.index)
            .finish_non_exhaustive()
    }
}

impl Font {
    /// Copies the bytes and validates face zero in a font file or collection.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, AdapterError> {
        let index = 0;
        units_per_em(bytes, index)
            .ok_or_else(|| AdapterError::new(AdapterErrorKind::InvalidFont))?;
        let blob = Blob::from(bytes.to_vec());
        let source = SourceInfo::new(SourceId::new(), SourceKind::Memory(blob.clone()));
        FontInfo::from_source(source, index)
            .ok_or_else(|| AdapterError::new(AdapterErrorKind::InvalidFont))?;
        Ok(Self { blob, index })
    }
}

/// Immutable Fontique catalog snapshot for one application's text engines.
///
/// Registered font bytes always use shared [`Arc`] backing. With this crate's
/// `std` feature, the Fontique collection and source cache also use synchronized
/// shared backing, so cloning a set for another paragraph engine does not copy
/// the catalog or reload file-backed font data. Without `std`, Fontique clones
/// the small catalog structures locally while continuing to share font blobs.
#[derive(Clone)]
pub struct FontSet {
    collection: Collection,
    source_cache: SourceCache,
    font_count: usize,
    registered_family_names: Arc<[String]>,
}

impl fmt::Debug for FontSet {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("FontSet")
            .field("font_count", &self.font_count)
            .field("registered_family_names", &self.registered_family_names)
            .finish_non_exhaustive()
    }
}

impl FontSet {
    pub(crate) fn resources_mut(&mut self) -> (&mut Collection, &mut SourceCache) {
        (&mut self.collection, &mut self.source_cache)
    }

    /// Creates a deterministic catalog without registered or platform fonts.
    ///
    /// This is the starting point for a system-font-only catalog when the
    /// `system-fonts` feature is enabled. It is also useful for testing
    /// missing-font behavior without consulting the host platform.
    #[must_use]
    pub fn empty() -> Self {
        Self {
            collection: Collection::new(CollectionOptions {
                shared: cfg!(feature = "std"),
                system_fonts: false,
            }),
            source_cache: new_source_cache(),
            font_count: 0,
            registered_family_names: Arc::from([]),
        }
    }

    /// Registers a non-empty set of memory fonts with system discovery disabled.
    pub fn try_from_fonts(fonts: impl IntoIterator<Item = Font>) -> Result<Self, AdapterError> {
        let fonts: Vec<_> = fonts.into_iter().collect();
        if fonts.is_empty() {
            return Err(AdapterError::new(AdapterErrorKind::EmptyFontSet));
        }
        let mut set = Self::empty();
        let mut registered_family_names = Vec::new();
        for font in fonts {
            let blob = font.blob;
            let registered = set.collection.register_fonts(blob.clone(), None);
            if registered.is_empty()
                || registered.iter().any(|(_, fonts)| {
                    fonts
                        .iter()
                        .any(|font| units_per_em(blob.as_ref(), font.index()).is_none())
                })
            {
                return Err(AdapterError::new(AdapterErrorKind::InvalidFont));
            }
            set.font_count = set.font_count.saturating_add(
                registered
                    .iter()
                    .map(|(_, fonts)| fonts.len())
                    .sum::<usize>(),
            );
            registered_family_names.extend(
                registered
                    .iter()
                    .filter(|(_, fonts)| !fonts.is_empty())
                    .filter_map(|(family, _)| {
                        set.collection.family_name(*family).map(String::from)
                    }),
            );
        }
        registered_family_names.sort_unstable();
        registered_family_names.dedup();
        set.registered_family_names = registered_family_names.into();
        Ok(set)
    }

    /// Returns sorted, deduplicated family names found in registered fonts.
    ///
    /// Platform-discovered families are intentionally excluded so the result
    /// remains stable across hosts and repeated system-font discovery.
    #[must_use]
    pub fn registered_family_names(&self) -> &[String] {
        &self.registered_family_names
    }

    /// Returns a catalog with a fixed snapshot of platform fonts available for fallback.
    ///
    /// This operation is available only with the `system-fonts` feature. It is
    /// intended for native hosts whose users can enter scripts not covered by
    /// an application's bundled resources. Deterministic proofs and benchmarks
    /// should continue to use [`Self::try_from_fonts`] alone.
    ///
    /// The snapshot is loaded before the catalog enters a paragraph engine;
    /// this adapter does not observe later platform font-database changes.
    #[cfg(feature = "system-fonts")]
    #[must_use]
    pub fn with_system_fonts(mut self) -> Self {
        self.collection.load_system_fonts();
        self
    }

    /// Returns a copy whose generic family resolves to the supplied named families.
    pub fn with_generic_families(
        mut self,
        generic: GenericFamily,
        family_names: impl IntoIterator<Item = impl AsRef<str>>,
    ) -> Result<Self, AdapterError> {
        let families = resolve_family_ids(&mut self.collection, family_names)?;
        self.collection
            .set_generic_families(generic, families.into_iter());
        Ok(self)
    }

    /// Returns a copy with named fallback families for one script and optional language.
    pub fn with_fallbacks(
        mut self,
        script: Script,
        language: Option<Language>,
        family_names: impl IntoIterator<Item = impl AsRef<str>>,
    ) -> Result<Self, AdapterError> {
        let families = resolve_family_ids(&mut self.collection, family_names)?;
        if !self.collection.set_fallbacks(
            FallbackKey::new(script, language.as_ref()),
            families.into_iter(),
        ) {
            return Err(AdapterError::new(AdapterErrorKind::UnsupportedFallback));
        }
        Ok(self)
    }
}

fn new_source_cache() -> SourceCache {
    #[cfg(feature = "std")]
    {
        SourceCache::new_shared()
    }
    #[cfg(not(feature = "std"))]
    {
        SourceCache::default()
    }
}

fn resolve_family_ids(
    collection: &mut Collection,
    family_names: impl IntoIterator<Item = impl AsRef<str>>,
) -> Result<Vec<fontique::FamilyId>, AdapterError> {
    family_names
        .into_iter()
        .map(|name| {
            collection
                .family_id(name.as_ref())
                .ok_or_else(|| AdapterError::new(AdapterErrorKind::UnknownFamily))
        })
        .collect()
}

fn units_per_em(bytes: &[u8], face_index: u32) -> Option<u16> {
    let font_offset = if bytes.get(0..4)? == b"ttcf" {
        let index = usize::try_from(face_index).ok()?;
        read_u32(bytes, 12_usize.checked_add(index.checked_mul(4)?)?)? as usize
    } else if face_index == 0 {
        0
    } else {
        return None;
    };
    let table_count = usize::from(read_u16(bytes, font_offset.checked_add(4)?)?);
    let records = font_offset.checked_add(12)?;
    for index in 0..table_count {
        let record = records.checked_add(index.checked_mul(16)?)?;
        if bytes.get(record..record.checked_add(4)?)? == b"head" {
            let offset = read_u32(bytes, record.checked_add(8)?)? as usize;
            let units = read_u16(bytes, offset.checked_add(18)?)?;
            return (units != 0).then_some(units);
        }
    }
    None
}

pub(crate) fn read_u16(bytes: &[u8], offset: usize) -> Option<u16> {
    let data: [u8; 2] = bytes.get(offset..offset.checked_add(2)?)?.try_into().ok()?;
    Some(u16::from_be_bytes(data))
}

pub(crate) fn read_u32(bytes: &[u8], offset: usize) -> Option<u32> {
    let data: [u8; 4] = bytes.get(offset..offset.checked_add(4)?)?.try_into().ok()?;
    Some(u32::from_be_bytes(data))
}

/// Stable category for adapter construction failures.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum AdapterErrorKind {
    /// Supplied bytes contain no usable first face or font metrics.
    InvalidFont,
    /// A font set contains no fonts.
    EmptyFontSet,
    /// A configured generic or fallback family is absent from the catalog.
    UnknownFamily,
    /// Fontique does not track the requested script and language fallback key.
    UnsupportedFallback,
}

/// Concrete adapter construction error.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct AdapterError {
    kind: AdapterErrorKind,
}

impl AdapterError {
    const fn new(kind: AdapterErrorKind) -> Self {
        Self { kind }
    }

    /// Returns the stable error category.
    #[must_use]
    pub const fn kind(&self) -> AdapterErrorKind {
        self.kind
    }
}

impl fmt::Display for AdapterError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "Parley adapter construction failed: {:?}",
            self.kind
        )
    }
}

impl core::error::Error for AdapterError {}
