// Copyright 2026 the Underwood Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Live native proof of Underwood's retained document pipeline.

mod app;
mod content;
mod host;
mod interaction;
mod presentation;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    app::run()
}
