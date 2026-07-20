// Copyright 2026 the Underwood Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Validation for the machine-readable capability proof ledger.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Component, Path};

use crate::table;

const LEDGER_PATH: &str = "docs/proof/ledger.tsv";
const MARKER: &str = "# underwood-proof-ledger-v1";
const HEADER: [&str; 7] = [
    "capability",
    "state",
    "proof",
    "owner",
    "spec",
    "evidence",
    "product",
];

pub(crate) fn check(root: &Path) -> Vec<String> {
    let path = root.join(LEDGER_PATH);
    let contents = match fs::read_to_string(&path) {
        Ok(contents) => contents,
        Err(error) => {
            return vec![format!("cannot read {LEDGER_PATH}: {error}")];
        }
    };

    validate(&contents, |relative| root.join(relative).is_file())
}

fn validate(contents: &str, path_exists: impl Fn(&str) -> bool) -> Vec<String> {
    let rows = match table::parse(contents, MARKER, &HEADER) {
        Ok(rows) => rows,
        Err(errors) => {
            return errors
                .into_iter()
                .map(|error| format!("{LEDGER_PATH}: {error}"))
                .collect();
        }
    };

    let mut errors = Vec::new();
    let mut capabilities = BTreeSet::new();

    for row in rows {
        let [capability, state, proof, owner, spec, evidence, product] = row.fields.as_slice()
        else {
            errors.push(format!(
                "{LEDGER_PATH}: line {} has an internal field-count error",
                row.line
            ));
            continue;
        };
        let prefix = format!("{LEDGER_PATH}: line {}", row.line);

        if !is_kebab_case(capability) {
            errors.push(format!(
                "{prefix}: capability `{capability}` must be kebab-case"
            ));
        }
        if !capabilities.insert(*capability) {
            errors.push(format!(
                "{prefix}: capability `{capability}` appears more than once"
            ));
        }
        if !matches!(*state, "active" | "gated" | "dormant") {
            errors.push(format!("{prefix}: unknown state `{state}`"));
        }
        let Some(current_rank) = proof_rank(proof) else {
            errors.push(format!("{prefix}: unknown proof status `{proof}`"));
            continue;
        };
        if *state == "active" && *owner == "unassigned" {
            errors.push(format!("{prefix}: active capability has no owner"));
        }

        let spec_path = spec.split('#').next().unwrap_or_default();
        if !safe_repository_path(spec_path) {
            errors.push(format!(
                "{prefix}: specification path `{spec_path}` is not repository-relative"
            ));
        } else if !path_exists(spec_path) {
            errors.push(format!(
                "{prefix}: specification path `{spec_path}` does not exist"
            ));
        }

        let evidence_paths: Vec<_> = if *evidence == "-" {
            Vec::new()
        } else {
            evidence.split(';').collect()
        };

        if current_rank >= proof_rank("executable").expect("known status")
            && evidence_paths.is_empty()
        {
            errors.push(format!("{prefix}: `{proof}` requires checked-in evidence"));
        }
        for evidence_path in &evidence_paths {
            if !safe_repository_path(evidence_path) {
                errors.push(format!(
                    "{prefix}: evidence path `{evidence_path}` is not repository-relative"
                ));
            } else if !path_exists(evidence_path) {
                errors.push(format!(
                    "{prefix}: evidence path `{evidence_path}` does not exist"
                ));
            }
        }

        if current_rank >= proof_rank("measured").expect("known status")
            && !evidence_paths
                .iter()
                .any(|path| contains_any(path, &["bench", "measure", "budget"]))
        {
            errors.push(format!(
                "{prefix}: measured-or-higher proof needs benchmark, measurement, or budget evidence"
            ));
        }
        if current_rank >= proof_rank("conformant").expect("known status")
            && !evidence_paths
                .iter()
                .any(|path| contains_any(path, &["corpus", "conformance", "platform"]))
        {
            errors.push(format!(
                "{prefix}: conformant-or-higher proof needs corpus, conformance, or platform evidence"
            ));
        }
        if current_rank >= proof_rank("product-proven").expect("known status") && *product == "-" {
            errors.push(format!(
                "{prefix}: product-proven capability needs a product scenario"
            ));
        }
    }

    errors
}

fn proof_rank(proof: &str) -> Option<usize> {
    match proof {
        "specified" => Some(0),
        "executable" => Some(1),
        "measured" => Some(2),
        "conformant" => Some(3),
        "product-proven" => Some(4),
        _ => None,
    }
}

fn is_kebab_case(value: &str) -> bool {
    !value.is_empty()
        && !value.starts_with('-')
        && !value.ends_with('-')
        && !value.contains("--")
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
}

fn safe_repository_path(value: &str) -> bool {
    let path = Path::new(value);
    !path.as_os_str().is_empty()
        && !path.is_absolute()
        && path
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
}

fn contains_any(value: &str, needles: &[&str]) -> bool {
    let lowercase = value.to_ascii_lowercase();
    needles.iter().any(|needle| lowercase.contains(needle))
}

#[cfg(test)]
mod tests {
    use super::{HEADER, MARKER, validate};

    fn ledger(row: &str) -> String {
        format!("{MARKER}\n{}\n{row}\n", HEADER.join("\t"))
    }

    #[test]
    fn accepts_a_specified_capability_without_evidence() {
        let errors = validate(
            &ledger("positions\tgated\tspecified\thuman\tspec.md\t-\t-"),
            |path| path == "spec.md",
        );

        assert!(errors.is_empty(), "unexpected errors: {errors:?}");
    }

    #[test]
    fn executable_capability_requires_existing_evidence() {
        let errors = validate(
            &ledger("positions\tactive\texecutable\tproject\tspec.md\tmissing.md\t-"),
            |path| path == "spec.md",
        );

        assert!(
            errors.iter().any(|error| error.contains("missing.md")),
            "missing evidence should be reported: {errors:?}"
        );
    }

    #[test]
    fn active_capability_requires_an_owner() {
        let errors = validate(
            &ledger("positions\tactive\tspecified\tunassigned\tspec.md\t-\t-"),
            |path| path == "spec.md",
        );

        assert!(
            errors.iter().any(|error| error.contains("no owner")),
            "owner diagnostic should be present: {errors:?}"
        );
    }

    #[test]
    fn evidence_must_be_inside_the_repository() {
        let errors = validate(
            &ledger("positions\tactive\texecutable\tproject\tspec.md\t../outside.md\t-"),
            |_| true,
        );

        assert!(
            errors
                .iter()
                .any(|error| error.contains("not repository-relative")),
            "path-escape diagnostic should be present: {errors:?}"
        );
    }
}
