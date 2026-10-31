//! Generates `GAME_SCHEMA` (the `CREATE TABLE` statements of every table) from the repo's own
//! `dist/db_installer/sql/game/*.sql` (D-55), so the test schema is never a second copy.
//!
//! The dist files also carry seed rows (`INSERT`, `REPLACE`: castles, announcements, ...). Tests
//! build an empty database, so only the `CREATE` and `DROP` statements are kept.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::print_stdout)]

use std::fmt::Write as _;
use std::path::PathBuf;

fn main() {
    let manifest = PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let dir = manifest.join("../../dist/db_installer/sql/game");
    println!("cargo:rerun-if-changed={}", dir.display());

    let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)
        .map(|d| {
            d.filter_map(Result::ok)
                .map(|e| e.path())
                .filter(|p| p.extension().is_some_and(|x| x == "sql"))
                .collect()
        })
        .unwrap_or_default();
    if files.is_empty() {
        println!(
            "cargo:warning=no SQL files in {}: GAME_SCHEMA is empty",
            dir.display()
        );
    }
    files.sort();

    let mut out = String::from("pub const GAME_SCHEMA: &[&str] = &[\n");
    let mut arms = String::new();
    for file in &files {
        println!("cargo:rerun-if-changed={}", file.display());
        let text = std::fs::read_to_string(file).unwrap();
        let mut kept = String::new();
        let without_comments: String = text
            .replace("\r\n", "\n")
            .lines()
            .filter(|l| !l.trim_start().starts_with("--"))
            .map(|l| format!("{l}\n"))
            .collect();
        for statement in without_comments.split(';').map(str::trim) {
            let head = statement.to_ascii_uppercase();
            if head.starts_with("CREATE") || head.starts_with("DROP") {
                kept.push_str(statement);
                kept.push_str(";\n");
            }
        }
        writeln!(out, "    {kept:?},").unwrap();
        let table = file.file_stem().unwrap().to_string_lossy();
        writeln!(arms, "    ({table:?}) => {{ {kept:?} }};").unwrap();
    }
    out.push_str("];\n");
    // `l2_db::game_sql!("characters")` is the filtered file as a string literal, so it also works
    // in `const` items (where `include_str!` of a copy was used before D-55).
    out.push_str("#[macro_export]\nmacro_rules! game_sql {\n");
    out.push_str(&arms);
    out.push_str("}\n");

    let target = PathBuf::from(std::env::var_os("OUT_DIR").unwrap()).join("game_schema.rs");
    std::fs::write(target, out).unwrap();
}
