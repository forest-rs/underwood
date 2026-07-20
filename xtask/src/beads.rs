// Copyright 2026 the Underwood Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Validation for the checked-in Beads planning export.

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

const CONFIG_PATH: &str = ".beads/config.yaml";
const EXPORT_PATH: &str = ".beads/issues.jsonl";

pub(crate) fn check(root: &Path) -> Vec<String> {
    let mut errors = Vec::new();

    match fs::read_to_string(root.join(CONFIG_PATH)) {
        Ok(config) => {
            if !config.contains("auto: true") || !config.contains("path: issues.jsonl") {
                errors.push(format!(
                    "{CONFIG_PATH}: scrubbed JSONL auto-export must remain enabled"
                ));
            }
        }
        Err(error) => errors.push(format!("cannot read {CONFIG_PATH}: {error}")),
    }

    let export = match fs::read_to_string(root.join(EXPORT_PATH)) {
        Ok(export) => export,
        Err(error) => {
            errors.push(format!("cannot read {EXPORT_PATH}: {error}"));
            return errors;
        }
    };
    errors.extend(validate_export(&export));
    errors
}

fn validate_export(export: &str) -> Vec<String> {
    let mut errors = Vec::new();
    let mut ids = BTreeSet::new();
    let mut issue_count = 0_usize;
    let mut required_titles = [
        "Underwood: five-year document platform",
        "Bootstrap the executable constitution",
        "Charter-000: spearhead, proof, and stewardship",
        "ADR-0001: position and canonical storage contract",
        "ADR-0002: resumable flow and virtual extent contract",
        "ADR-0003: text-data provisioning and identity",
        "ADR-0004: Parley boundary and contingency",
        "Gate: mandatory pre-foundation decisions ratified",
        "First campaign: living document semantic-to-scene spine",
    ]
    .map(|title| (title, false));
    let mut mandatory_gate_id = None;
    let mut first_campaign_line = None;

    for (index, line) in export.lines().enumerate() {
        let line_number = index + 1;
        if line.trim().is_empty() {
            continue;
        }
        issue_count += 1;
        if !line.starts_with('{') || !line.ends_with('}') || !line.contains("\"_type\":\"issue\"") {
            errors.push(format!(
                "{EXPORT_PATH}: line {line_number} is not an exported issue object"
            ));
            continue;
        }
        if !has_nonempty_field(line, "acceptance_criteria") {
            errors.push(format!(
                "{EXPORT_PATH}: line {line_number} lacks nonempty acceptance criteria"
            ));
        }

        let issue_id = field(line, "id");
        match issue_id {
            Some(id) if id.starts_with("und-") => {
                if !ids.insert(id) {
                    errors.push(format!(
                        "{EXPORT_PATH}: line {line_number} duplicates issue `{id}`"
                    ));
                }
            }
            Some(id) => errors.push(format!(
                "{EXPORT_PATH}: line {line_number} has non-Underwood issue `{id}`"
            )),
            None => errors.push(format!("{EXPORT_PATH}: line {line_number} has no issue id")),
        }

        if let Some(title) = field(line, "title") {
            for (required, found) in &mut required_titles {
                if title == *required {
                    *found = true;
                }
            }
            if title == "Gate: mandatory pre-foundation decisions ratified" {
                mandatory_gate_id = issue_id.map(str::to_owned);
            } else if title == "First campaign: living document semantic-to-scene spine" {
                first_campaign_line = Some(line.to_owned());
            }
        }
    }

    if issue_count < 20 {
        errors.push(format!(
            "{EXPORT_PATH}: expected the durable capability graph, found only {issue_count} issues"
        ));
    }
    for (title, found) in required_titles {
        if !found {
            errors.push(format!(
                "{EXPORT_PATH}: required issue `{title}` is missing"
            ));
        }
    }
    let first_campaign_has_gate = mandatory_gate_id
        .as_deref()
        .zip(first_campaign_line.as_deref())
        .is_some_and(|(gate_id, campaign)| has_dependency(campaign, gate_id));
    if !first_campaign_has_gate {
        errors.push(format!(
            "{EXPORT_PATH}: the first campaign is not blocked by the mandatory-decision gate"
        ));
    }

    errors
}

fn field<'a>(line: &'a str, name: &str) -> Option<&'a str> {
    let prefix = format!("\"{name}\":\"");
    let start = line.find(&prefix)? + prefix.len();
    let rest = &line[start..];
    let end = rest.find('"')?;
    Some(&rest[..end])
}

fn has_dependency(line: &str, dependency_id: &str) -> bool {
    line.contains(&format!("\"depends_on_id\":\"{dependency_id}\""))
}

fn has_nonempty_field(line: &str, name: &str) -> bool {
    field(line, name).is_some_and(|value| !value.is_empty())
}

#[cfg(test)]
mod tests {
    use super::{field, has_dependency, has_nonempty_field};

    #[test]
    fn extracts_a_simple_json_string_field() {
        let line = r#"{"id":"und-test","title":"Example"}"#;
        assert_eq!(field(line, "id"), Some("und-test"));
        assert_eq!(field(line, "title"), Some("Example"));
        assert_eq!(field(line, "missing"), None);
    }

    #[test]
    fn finds_a_dependency_by_semantic_gate_id() {
        let line = r#"{"dependencies":[{"depends_on_id":"und-gate"}]}"#;
        assert!(has_dependency(line, "und-gate"));
        assert!(!has_dependency(line, "und-other"));
    }

    #[test]
    fn rejects_an_empty_required_field() {
        let line = r#"{"acceptance_criteria":""}"#;
        assert!(!has_nonempty_field(line, "acceptance_criteria"));
        assert!(!has_nonempty_field(line, "missing"));
    }
}
