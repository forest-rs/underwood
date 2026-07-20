// Copyright 2026 the Underwood Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Validation for repository structure, crate classes, and ownership fences.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Component, Path};

use crate::table;

const REGISTRY_PATH: &str = "docs/governance/crates.tsv";
const MARKER: &str = "# underwood-crate-registry-v1";
const HEADER: [&str; 6] = ["path", "name", "class", "no_std", "published", "fence"];

const REQUIRED_PATHS: &[&str] = &[
    ".beads/issues.jsonl",
    ".cargo/config.toml",
    ".github/PULL_REQUEST_TEMPLATE.md",
    ".github/copyright.sh",
    ".github/workflows/ci.yml",
    ".typos.toml",
    "AGENTS.md",
    "AUTHORS",
    "CHANGELOG.md",
    "Cargo.lock",
    "Cargo.toml",
    "LICENSE-APACHE",
    "LICENSE-MIT",
    "README.md",
    "UNDERWOOD_HANDOVER.md",
    "docs/CONSTITUTION.md",
    "docs/adr/0000-template.md",
    "docs/charter/000-spearhead-proof-stewardship.md",
    "docs/governance/crates.tsv",
    "docs/proof/ledger.tsv",
    "rust-toolchain.toml",
    "taplo.toml",
    "xtask/Cargo.toml",
];

pub(crate) fn check(root: &Path) -> Vec<String> {
    let mut errors = check_required_paths(root);
    errors.extend(check_root_policy(root));
    errors.extend(check_registry(root));
    errors.extend(check_rust_headers(root));
    errors
}

fn check_required_paths(root: &Path) -> Vec<String> {
    REQUIRED_PATHS
        .iter()
        .filter(|path| !root.join(path).exists())
        .map(|path| format!("required repository path `{path}` is missing"))
        .collect()
}

fn check_root_policy(root: &Path) -> Vec<String> {
    let mut errors = Vec::new();
    let cargo = read(root, "Cargo.toml", &mut errors);
    for required in [
        "[workspace]",
        "resolver = \"2\"",
        "rust-version = \"1.92\"",
        "rust.unsafe_code = \"deny\"",
    ] {
        if !cargo.contains(required) {
            errors.push(format!("Cargo.toml must contain `{required}`"));
        }
    }

    let clippy = read(root, "clippy.toml", &mut errors);
    if !clippy.contains("msrv = \"1.92\"") {
        errors.push(String::from(
            "clippy.toml must keep `msrv = \"1.92\"` synchronized",
        ));
    }

    let toolchain = read(root, "rust-toolchain.toml", &mut errors);
    if !toolchain.contains("channel = \"1.96.0\"") {
        errors.push(String::from(
            "rust-toolchain.toml must keep `channel = \"1.96.0\"` synchronized",
        ));
    }

    let agents = read(root, "AGENTS.md", &mut errors);
    for required in [
        "bd ready",
        "cargo xtask check",
        "Specified -> Executable -> Measured -> Conformant -> Product-proven",
        "New production dependencies require human approval",
    ] {
        if !agents.contains(required) {
            errors.push(format!("AGENTS.md must contain `{required}`"));
        }
    }

    let ci = read(root, ".github/workflows/ci.yml", &mut errors);
    for required in [
        "merge_group:",
        "cargo xtask check",
        "-D warnings",
        "RUST_STABLE_VER: \"1.96\"",
        "RUST_MIN_VER: \"1.92\"",
    ] {
        if !ci.contains(required) {
            errors.push(format!(
                ".github/workflows/ci.yml must contain `{required}`"
            ));
        }
    }

    errors
}

fn check_registry(root: &Path) -> Vec<String> {
    let path = root.join(REGISTRY_PATH);
    let contents = match fs::read_to_string(path) {
        Ok(contents) => contents,
        Err(error) => return vec![format!("cannot read {REGISTRY_PATH}: {error}")],
    };
    let rows = match table::parse(&contents, MARKER, &HEADER) {
        Ok(rows) => rows,
        Err(errors) => {
            return errors
                .into_iter()
                .map(|error| format!("{REGISTRY_PATH}: {error}"))
                .collect();
        }
    };

    let mut errors = Vec::new();
    let mut paths = BTreeSet::new();
    let mut names = BTreeSet::new();
    let mut has_core = false;
    let root_manifest = read(root, "Cargo.toml", &mut errors);
    let workspace_members = match array_values(&root_manifest, "members") {
        Ok(values) => values,
        Err(error) => {
            errors.push(format!("Cargo.toml: {error}"));
            Vec::new()
        }
    };

    for row in rows {
        let [path, name, class, no_std, published, fence] = row.fields.as_slice() else {
            errors.push(format!(
                "{REGISTRY_PATH}: line {} has an internal field-count error",
                row.line
            ));
            continue;
        };
        let prefix = format!("{REGISTRY_PATH}: line {}", row.line);

        if !safe_relative_path(path) {
            errors.push(format!("{prefix}: unsafe crate path `{path}`"));
            continue;
        }
        if !paths.insert(*path) {
            errors.push(format!("{prefix}: duplicate crate path `{path}`"));
        }
        if !workspace_members.iter().any(|member| member == path) {
            errors.push(format!(
                "{prefix}: crate path `{path}` is absent from workspace members"
            ));
        }
        if !names.insert(*name) {
            errors.push(format!("{prefix}: duplicate crate name `{name}`"));
        }
        if !matches!(
            *class,
            "tooling" | "core" | "facade" | "adapter" | "test" | "example" | "benchmark" | "host"
        ) {
            errors.push(format!("{prefix}: unknown crate class `{class}`"));
        }
        if !matches!(*no_std, "yes" | "no" | "n/a") {
            errors.push(format!("{prefix}: no_std must be yes, no, or n/a"));
        }
        if !matches!(*published, "yes" | "no") {
            errors.push(format!("{prefix}: published must be yes or no"));
        }
        if !fence.starts_with("This crate owns ")
            || !fence.contains("; it explicitly does not own ")
        {
            errors.push(format!(
                "{prefix}: fence must use `This crate owns X; it explicitly does not own Y.`"
            ));
        }

        let crate_root = root.join(path);
        if !crate_root.is_dir() {
            errors.push(format!("{prefix}: crate directory `{path}` does not exist"));
            continue;
        }
        let manifest_path = crate_root.join("Cargo.toml");
        let manifest = match fs::read_to_string(&manifest_path) {
            Ok(manifest) => manifest,
            Err(error) => {
                errors.push(format!("{prefix}: cannot read {path}/Cargo.toml: {error}"));
                continue;
            }
        };
        if !manifest.contains("[lints]\nworkspace = true") {
            errors.push(format!(
                "{prefix}: {path}/Cargo.toml must inherit workspace lints"
            ));
        }
        if *published == "no" && !manifest.contains("publish = false") {
            errors.push(format!(
                "{prefix}: unpublished crate `{name}` must set publish = false"
            ));
        }

        if *class == "tooling" && *published != "no" {
            errors.push(format!(
                "{prefix}: tooling crate `{name}` cannot be published"
            ));
        }
        if *class == "core" {
            has_core = true;
            if *no_std != "yes" {
                errors.push(format!(
                    "{prefix}: core crate `{name}` must declare no_std=yes"
                ));
            }
            let library = crate_root.join("src/lib.rs");
            match fs::read_to_string(&library) {
                Ok(source) if source.lines().any(|line| line.trim() == "#![no_std]") => {}
                Ok(_) => errors.push(format!("{prefix}: core crate `{name}` lacks #![no_std]")),
                Err(error) => errors.push(format!(
                    "{prefix}: cannot read core crate source {}: {error}",
                    library.display()
                )),
            }
            if section_has_entries(&manifest, "dev-dependencies") {
                errors.push(format!(
                    "{prefix}: core crate `{name}` has forbidden dev-dependencies"
                ));
            }
        }
    }

    for member in workspace_members {
        if !paths.contains(member.as_str()) {
            errors.push(format!(
                "Cargo.toml workspace member `{member}` is absent from {REGISTRY_PATH}"
            ));
        }
    }

    if has_core {
        let ci = read(root, ".github/workflows/ci.yml", &mut errors);
        for target in ["x86_64-unknown-none", "wasm32-unknown-unknown"] {
            if !ci.contains(target) {
                errors.push(format!(
                    "core crates exist but CI does not name target `{target}`"
                ));
            }
        }
    }

    errors
}

fn check_rust_headers(root: &Path) -> Vec<String> {
    let mut rust_files = Vec::new();
    collect_rust_files(root, root, &mut rust_files);
    let mut errors = Vec::new();
    let expected_second = "// SPDX-License-Identifier: Apache-2.0 OR MIT";

    for path in rust_files {
        match fs::read_to_string(&path) {
            Ok(source) => {
                let mut lines = source.lines();
                if !lines.next().is_some_and(valid_copyright)
                    || lines.next() != Some(expected_second)
                {
                    let relative = path.strip_prefix(root).unwrap_or(&path);
                    errors.push(format!(
                        "{} lacks the required copyright and SPDX header",
                        relative.display()
                    ));
                }
            }
            Err(error) => errors.push(format!("cannot read {}: {error}", path.display())),
        }
    }

    errors
}

fn valid_copyright(line: &str) -> bool {
    let Some(remainder) = line.strip_prefix("// Copyright ") else {
        return false;
    };
    let Some((year, owner)) = remainder.split_once(' ') else {
        return false;
    };
    year.len() == 4
        && matches!(&year[..2], "19" | "20")
        && year.bytes().all(|byte| byte.is_ascii_digit())
        && owner == "the Underwood Authors"
}

fn collect_rust_files(root: &Path, directory: &Path, output: &mut Vec<std::path::PathBuf>) {
    let Ok(entries) = fs::read_dir(directory) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            let relative = path.strip_prefix(root).unwrap_or(&path);
            if matches!(
                relative.components().next(),
                Some(Component::Normal(name))
                    if name == ".git" || name == ".beads" || name == "target"
            ) {
                continue;
            }
            collect_rust_files(root, &path, output);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            output.push(path);
        }
    }
}

fn read(root: &Path, relative: &str, errors: &mut Vec<String>) -> String {
    match fs::read_to_string(root.join(relative)) {
        Ok(contents) => contents,
        Err(error) => {
            errors.push(format!("cannot read {relative}: {error}"));
            String::new()
        }
    }
}

fn safe_relative_path(path: &str) -> bool {
    let path = Path::new(path);
    !path.as_os_str().is_empty()
        && !path.is_absolute()
        && path
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
}

fn section_has_entries(manifest: &str, section: &str) -> bool {
    let mut in_section = false;

    for raw_line in manifest.lines() {
        let line = raw_line.trim();
        if let Some(section_name) = line
            .strip_prefix('[')
            .and_then(|line| line.strip_suffix(']'))
        {
            in_section = section_name == section || section_name.ends_with(&format!(".{section}"));
            continue;
        }
        if in_section && !line.is_empty() && !line.starts_with('#') {
            return true;
        }
    }

    false
}

fn array_values(manifest: &str, key: &str) -> Result<Vec<String>, String> {
    let needle = format!("{key} =");
    let assignment = manifest
        .match_indices(&needle)
        .find_map(|(index, _)| {
            (index == 0 || manifest.as_bytes().get(index.wrapping_sub(1)) == Some(&b'\n'))
                .then_some(index)
        })
        .ok_or_else(|| format!("array `{key}` is missing"))?;
    let remainder = &manifest[assignment + needle.len()..];
    let open = remainder
        .find('[')
        .ok_or_else(|| format!("array `{key}` has no opening bracket"))?;
    let after_open = &remainder[open + 1..];
    let close = after_open
        .find(']')
        .ok_or_else(|| format!("array `{key}` has no closing bracket"))?;

    let mut values = Vec::new();
    for raw_value in after_open[..close].split(',') {
        let value = raw_value.trim();
        if value.is_empty() {
            continue;
        }
        let Some(value) = value
            .strip_prefix('"')
            .and_then(|value| value.strip_suffix('"'))
        else {
            return Err(format!("array `{key}` contains non-string value `{value}`"));
        };
        values.push(value.to_owned());
    }
    if values.is_empty() {
        return Err(format!("array `{key}` is empty"));
    }
    Ok(values)
}

#[cfg(test)]
mod tests {
    use super::{array_values, safe_relative_path, section_has_entries, valid_copyright};

    #[test]
    fn accepts_only_normal_relative_crate_paths() {
        assert!(safe_relative_path("crates/underwood_core"));
        assert!(!safe_relative_path("../outside"));
        assert!(!safe_relative_path("/absolute"));
        assert!(!safe_relative_path(""));
    }

    #[test]
    fn finds_nonempty_manifest_sections() {
        let manifest =
            "[dependencies]\n\n[target.'cfg(unix)'.dev-dependencies]\nproptest = \"1\"\n";
        assert!(section_has_entries(manifest, "dev-dependencies"));
        assert!(!section_has_entries(manifest, "build-dependencies"));
    }

    #[test]
    fn accepts_a_standard_underwood_copyright() {
        assert!(valid_copyright("// Copyright 2026 the Underwood Authors"));
        assert!(!valid_copyright("// Copyright 0026 the Underwood Authors"));
        assert!(!valid_copyright("// Copyright 26 the Underwood Authors"));
        assert!(!valid_copyright("// Copyright 2026 Somebody Else"));
    }

    #[test]
    fn parses_inline_and_multiline_workspace_members() {
        let inline = "[workspace]\nmembers = [\"xtask\"]\ndefault-members = [\"xtask\"]\n";
        assert_eq!(
            array_values(inline, "members").expect("inline members"),
            ["xtask"]
        );

        let multiline = "[workspace]\nmembers = [\n  \"xtask\",\n  \"underwood\",\n]\n";
        assert_eq!(
            array_values(multiline, "members").expect("multiline members"),
            ["xtask", "underwood"]
        );
    }
}
