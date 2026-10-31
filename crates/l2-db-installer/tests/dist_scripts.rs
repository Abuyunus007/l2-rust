#![allow(clippy::unwrap_used, clippy::expect_used)]
//! The statement splitter against every script of the repository's `dist/db_installer/sql`.

use std::path::PathBuf;

use l2_db_installer::installer::{Schema, load_scripts};
use l2_db_installer::splitter::split_statements;

fn sql_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../dist/db_installer/sql")
}

#[test]
fn every_dist_script_splits_cleanly() {
    let mut files = 0;
    for schema in [Schema::Login, Schema::Game] {
        let dir = sql_dir().join(schema.dir_name());
        for entry in std::fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().and_then(|e| e.to_str()) != Some("sql") {
                continue;
            }
            files += 1;
            let text = String::from_utf8(std::fs::read(&path).unwrap()).unwrap();
            let statements = split_statements(&text);
            let name = path.display().to_string();
            assert!(!statements.is_empty(), "{name}: no statements");

            // The quirks of the Java splitter must not matter for these files:
            // no DELIMITER, no `--` inside a quoted string, and nothing after the last `;`.
            let mut last_code_line = "";
            for line in text.lines().map(str::trim) {
                assert!(!line.to_uppercase().starts_with("DELIMITER"), "{name}: DELIMITER");
                if line.is_empty() || line.starts_with("--") || line.starts_with('#') {
                    continue;
                }
                if let Some(at) = line.find("--") {
                    let quotes = line[..at].matches('\'').count();
                    assert_eq!(quotes % 2, 0, "{name}: `--` inside a string: {line}");
                    last_code_line = line[..at].trim();
                } else {
                    last_code_line = line;
                }
            }
            assert!(last_code_line.ends_with(';'), "{name}: unterminated end");
            for s in &statements {
                assert!(s.ends_with(';'), "{name}: {s}");
            }
        }
    }
    assert!(files >= 100, "found only {files} scripts");
}

#[test]
fn load_scripts_returns_login_files_in_java_order() {
    let scripts = load_scripts(&sql_dir(), Schema::Login).unwrap();
    let names: Vec<_> = scripts.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(
        names,
        [
            "account_data.sql",
            "accounts.sql",
            "accounts_ipauth.sql",
            "gameservers.sql"
        ]
    );
}

#[test]
fn the_account_table_script_has_a_drop_and_a_create() {
    let scripts = load_scripts(&sql_dir(), Schema::Login).unwrap();
    let accounts = scripts.iter().find(|s| s.name == "accounts.sql").unwrap();
    assert_eq!(accounts.statements.len(), 2);
    assert_eq!(accounts.statements[0], "DROP TABLE IF EXISTS `accounts`;");
    assert!(accounts.statements[1].starts_with("CREATE TABLE IF NOT EXISTS `accounts` ("));
}
