//! Splits an SQL script into statements exactly like the Java installer.

pub use l2_db::split_statements;

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
        assert!(split_statements("INSERT INTO t VALUES ('a--b');\n").is_empty());
    }

    #[test]
    fn empty_script_has_no_statements() {
        assert!(split_statements("").is_empty());
        assert!(split_statements("-- only a comment\n").is_empty());
    }
}
