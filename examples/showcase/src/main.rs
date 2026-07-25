// Copyright 2026 the Underwood Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Live native proof of Underwood's retained document pipeline.

mod app;
mod content;
mod host;
mod interaction;
mod page;
mod presentation;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    if std::env::args_os().nth(1).as_deref() == Some(std::ffi::OsStr::new("--write-snapshot")) {
        let path = app::write_snapshot()?;
        println!("wrote {}", path.display());
        return Ok(());
    }
    app::run()
}
