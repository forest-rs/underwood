// Copyright 2026 the Underwood Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Strict parsing for the small versioned TSV policy files.

#[derive(Debug)]
pub(crate) struct Row<'a> {
    pub(crate) line: usize,
    pub(crate) fields: Vec<&'a str>,
}

pub(crate) fn parse<'a>(
    contents: &'a str,
    marker: &str,
    expected_header: &[&str],
) -> Result<Vec<Row<'a>>, Vec<String>> {
    let mut errors = Vec::new();
    let mut lines = contents.lines().enumerate();

    let first = lines.next().map(|(_, line)| line.trim());
    if first != Some(marker) {
        errors.push(format!("line 1 must be the format marker `{marker}`"));
    }

    let expected_header_text = expected_header.join("\t");
    let mut found_header = false;
    let mut rows = Vec::new();

    for (index, raw_line) in lines {
        let line_number = index + 1;
        let line = raw_line.trim_end();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        if !found_header {
            found_header = true;
            if line != expected_header_text {
                errors.push(format!(
                    "line {line_number} must be the header `{expected_header_text}`"
                ));
            }
            continue;
        }

        let fields: Vec<_> = line.split('\t').collect();
        if fields.len() != expected_header.len() {
            errors.push(format!(
                "line {line_number} has {} field(s), expected {}",
                fields.len(),
                expected_header.len()
            ));
            continue;
        }
        if fields.iter().any(|field| field.is_empty()) {
            errors.push(format!("line {line_number} contains an empty field"));
            continue;
        }
        rows.push(Row {
            line: line_number,
            fields,
        });
    }

    if !found_header {
        errors.push(String::from("the table header is missing"));
    } else if rows.is_empty() {
        errors.push(String::from("the table contains no data rows"));
    }

    if errors.is_empty() {
        Ok(rows)
    } else {
        Err(errors)
    }
}

#[cfg(test)]
mod tests {
    use super::parse;

    #[test]
    fn parses_a_versioned_table() {
        let rows = parse(
            "# format-v1\nname\tstate\nalpha\tactive\n",
            "# format-v1",
            &["name", "state"],
        )
        .expect("valid table");

        assert_eq!(rows.len(), 1, "one row should be parsed");
        assert_eq!(rows[0].line, 3, "source line should be retained");
        assert_eq!(rows[0].fields, ["alpha", "active"]);
    }

    #[test]
    fn rejects_the_wrong_number_of_fields() {
        let errors = parse(
            "# format-v1\nname\tstate\nalpha\n",
            "# format-v1",
            &["name", "state"],
        )
        .expect_err("invalid row");

        assert!(
            errors.iter().any(|error| error.contains("expected 2")),
            "field-count diagnostic should be actionable: {errors:?}"
        );
    }
}
