// Copyright 2026 the Underwood Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Dependency-free repository policy checks for Underwood.

mod beads;
mod proof;
mod repository;
mod table;

use std::env;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

fn main() -> ExitCode {
    let mut arguments = env::args().skip(1);
    let command = arguments.next().unwrap_or_else(|| String::from("check"));

    if let Some(unexpected) = arguments.next() {
        eprintln!("unexpected argument `{unexpected}`");
        print_usage();
        return ExitCode::FAILURE;
    }

    let root = repository_root();
    let errors = match command.as_str() {
        "check" => run_all(&root),
        "proof" => proof::check(&root),
        "repo" => repository::check(&root),
        "beads" => beads::check(&root),
        "help" | "-h" | "--help" => {
            print_usage();
            return ExitCode::SUCCESS;
        }
        unknown => {
            eprintln!("unknown xtask command `{unknown}`");
            print_usage();
            return ExitCode::FAILURE;
        }
    };

    report(command.as_str(), errors)
}

fn repository_root() -> PathBuf {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    manifest
        .parent()
        .expect("xtask must remain a direct child of the repository root")
        .to_path_buf()
}

fn run_all(root: &Path) -> Vec<String> {
    let mut errors = repository::check(root);
    errors.extend(proof::check(root));
    errors.extend(beads::check(root));
    errors.sort();
    errors.dedup();
    errors
}

fn report(command: &str, errors: Vec<String>) -> ExitCode {
    if errors.is_empty() {
        println!("underwood policy `{command}`: ok");
        return ExitCode::SUCCESS;
    }

    eprintln!(
        "underwood policy `{command}` failed with {} violation(s):",
        errors.len()
    );
    for error in errors {
        eprintln!("  - {error}");
    }
    ExitCode::FAILURE
}

fn print_usage() {
    eprintln!(
        "usage: cargo xtask [check|proof|repo|beads]\n\
         \n\
         check  validate all repository policies (default)\n\
         proof  validate docs/proof/ledger.tsv\n\
         repo   validate repository and crate fences\n\
         beads  validate the checked-in Beads export"
    );
}
