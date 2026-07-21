// Copyright 2026 the Underwood Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Private ADR-0002 resumable-flow and virtual-extent wind tunnel.

mod extent;
mod flow;
mod report;

fn main() -> std::process::ExitCode {
    report::run()
}
