// Copyright 2026 the Underwood Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

#![no_std]
#![doc = include_str!("../README.md")]

extern crate alloc;

mod engine;
mod font;
mod interaction;
mod line_break;
mod lowering;
mod shaping;
mod validation;

pub use engine::ParleyParagraphEngine;
pub use font::{AdapterError, AdapterErrorKind, Font, FontSet};

#[cfg(test)]
mod tests;
