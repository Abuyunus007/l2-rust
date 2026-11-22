//! The statements of an SQL script of `dist/db_installer/sql`, split like the Java installer
//! (shared by the database installer, the setup and the database tests; a plain split at `;`
//! breaks the scripts that have `;` inside a string, such as `siegable_clanhall.sql`).

/// Statements of one script, in file order, each without its trailing newline.
///
/// Rules (all taken from Java, not from the SQL grammar):
/// - every line is trimmed; empty lines and lines starting with `--` are skipped;
/// - a `--` inside a line cuts the rest of the line off;
/// - a line starting with `#` is a comment too;
/// - the remaining lines are joined with one space until a line ends with `;`, which closes the
///   statement (the `;` stays in the text);
/// - text after the last `;` is dropped.
pub fn split_statements(script: &str) -> Vec<String> {
    let mut statements = Vec::new();
    let mut current = String::new();
    for raw in script.lines() {
        let mut line = raw.trim();
        // DEVIATION from Java: `#` comment lines are skipped too (see the doc above).
        if line.starts_with("--") || line.starts_with('#') || line.is_empty() {
            continue;
        }
        if let Some(at) = line.find("--") {
            line = line[..at].trim();
        }
        current.push_str(line);
        current.push(' ');
        if line.ends_with(';') {
            let sql = current.trim();
            if !sql.is_empty() {
                statements.push(sql.to_owned());
            }
            current.clear();
        }
    }
    statements
}

/// [`split_statements`] without the closing `;` of each statement: what a single prepared
/// statement takes.
pub fn script_statements(script: &str) -> Vec<String> {
    split_statements(script)
        .into_iter()
        .map(|s| s.trim_end_matches(';').trim_end().to_owned())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_semicolon_inside_a_string_does_not_split_the_statement() {
        let script = "INSERT INTO t VALUES ('14;0;0;12;00'),\n(2, 'x;y');\nSELECT 1;\n";
        assert_eq!(
            script_statements(script),
            ["INSERT INTO t VALUES ('14;0;0;12;00'), (2, 'x;y')", "SELECT 1"]
        );
    }

    #[test]
    fn every_shipped_script_splits_into_whole_statements() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../dist/db_installer/sql");
        for side in ["login", "game"] {
            for entry in std::fs::read_dir(root.join(side)).unwrap() {
                let path = entry.unwrap().path();
                let text = std::fs::read_to_string(&path).unwrap();
                for statement in script_statements(&text) {
                    let first = statement
                        .split_whitespace()
                        .next()
                        .unwrap_or("")
                        .to_ascii_uppercase();
                    assert!(
                        [
                            "CREATE", "INSERT", "REPLACE", "DROP", "ALTER", "UPDATE", "DELETE", "SET"
                        ]
                        .contains(&first.as_str()),
                        "{}: {statement}",
                        path.display()
                    );
                }
            }
        }
    }
}
