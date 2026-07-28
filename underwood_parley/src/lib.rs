// Copyright 2026 the Underwood Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

#![no_std]
#![doc = include_str!("../README.md")]

extern crate alloc;
#[cfg(test)]
extern crate std;

mod engine;
mod font;
mod interaction;
mod line_break;
mod line_former;
mod lowering;
mod shaping;
mod spacing;

pub use engine::ParleyParagraphEngine;
pub use font::{AdapterError, AdapterErrorKind, Font, FontSet};

#[cfg(test)]
mod tests;
