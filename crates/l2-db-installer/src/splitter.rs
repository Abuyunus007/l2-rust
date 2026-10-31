//! Splits an SQL script into statements exactly like the Java installer.
// Java: org/l2jmobius/tools/DatabaseInstaller.java (`executeSQLScript`, `executeSQLScriptConsole`)

/// Statements of one script, in file order, each without its trailing newline.
///
/// Rules (all taken from Java, not from the SQL grammar):
/// - every line is trimmed; empty lines and lines starting with `--` are skipped;
/// - a `--` inside a line cuts the rest of the line off;
/// - a line starting with `#` is a comment too;
/// - the remaining lines are joined with one space until a line ends with `;`, which closes the
///   statement (the `;` stays in the text);
/// - text after the last `;` is dropped.
///
/// NOTE(java-quirk): the `--` cut also hits a `--` inside a string literal, and `DELIMITER` or a
/// `;` at the end of a line inside a quoted string are not understood. No file in `dist` does
/// either (checked by the tests).
/// NOTE(java-quirk): Java does NOT know `#` comments. There, `# NAME` above a `CREATE INDEX`
/// (five game scripts) is glued to the statement, the whole text becomes one MySQL comment and
/// the index is lost with an `Query was empty` error. This port skips the `#` lines so the
/// indexes are created; the deviation is deliberate and reported.
/// NOTE(java-quirk): Java also drops a final statement that has no `;`.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn joins_lines_and_keeps_the_semicolon() {
        let script =
            "CREATE TABLE `a` (\n  `x` INT, -- the id\n  PRIMARY KEY (`x`)\n);\nINSERT INTO a VALUES (1);\n";
        assert_eq!(
            split_statements(script),
            vec![
                "CREATE TABLE `a` ( `x` INT, PRIMARY KEY (`x`) );".to_owned(),
                "INSERT INTO a VALUES (1);".to_owned(),
            ]
        );
    }

    #[test]
    fn skips_comments_and_blank_lines() {
        let script = "-- header\n\n   -- indented comment\nDROP TABLE a;\n";
        assert_eq!(split_statements(script), vec!["DROP TABLE a;".to_owned()]);
    }

    #[test]
    fn hash_comment_lines_do_not_swallow_the_next_statement() {
        let script = "# RESTORE_X
CREATE INDEX i ON t (a);

# trailing
";
        assert_eq!(
            split_statements(script),
            vec!["CREATE INDEX i ON t (a);".to_owned()]
        );
    }

    #[test]
    fn crlf_and_indentation_are_trimmed() {
        let script = "\tSELECT 1;\r\n  SELECT 2 ;\r\n";
        assert_eq!(
            split_statements(script),
            vec!["SELECT 1;".to_owned(), "SELECT 2 ;".to_owned()]
        );
    }

    #[test]
    fn drops_a_last_statement_without_semicolon() {
        // NOTE(java-quirk): same as Java.
        assert_eq!(
            split_statements("SELECT 1;\nSELECT 2"),
            vec!["SELECT 1;".to_owned()]
        );
    }

    #[test]
    fn a_comment_after_the_semicolon_still_ends_the_statement() {
        assert_eq!(
            split_statements("SELECT 1; -- done\n"),
            vec!["SELECT 1;".to_owned()]
        );
    }

    #[test]
    fn a_comment_cuts_inside_string_literals_like_java() {
        // NOTE(java-quirk): the text after `--` is lost even inside quotes, so the line no
        // longer ends with `;` and the statement is never closed.
        assert!(split_statements("INSERT INTO t VALUES ('a--b');\n").is_empty());
    }

    #[test]
    fn empty_script_has_no_statements() {
        assert!(split_statements("").is_empty());
        assert!(split_statements("-- only a comment\n").is_empty());
    }
}
