// Copyright 2026 the Underwood Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Private ADR-0001 position and storage wind tunnel.

mod candidate;
mod model;
mod report;
mod trace;

use std::process::ExitCode;

fn main() -> ExitCode {
    if report::run() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}
