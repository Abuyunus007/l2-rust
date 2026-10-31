//! The `CREATE TABLE` statements of every game server table, taken from the repo's own
//! `dist/db_installer/sql/game` files (D-55) by `build.rs` (seed rows left out). Tests that start
//! a real game server or its loader create all of them, so a new table cannot be forgotten in
//! one test's list.

// Every file of `dist/db_installer/sql/game`, in file name order, one entry per file.
include!(concat!(env!("OUT_DIR"), "/game_schema.rs"));

#[cfg(test)]
mod tests {
    #[test]
    fn every_game_schema_file_is_listed() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../dist/db_installer/sql/game");
        let count = std::fs::read_dir(dir)
            .map(|d| {
                d.filter_map(Result::ok)
                    .filter(|e| e.path().extension().is_some_and(|x| x == "sql"))
                    .count()
            })
            .unwrap_or(0);
        assert_eq!(super::GAME_SCHEMA.len(), count, "dist/ and GAME_SCHEMA disagree");
    }

    #[test]
    fn schema_has_tables_and_no_seed_rows() {
        if super::GAME_SCHEMA.is_empty() {
            eprintln!("skipped: no dist/db_installer/sql/game");
            return;
        }
        let all = super::GAME_SCHEMA.concat();
        assert!(all.contains("CREATE TABLE IF NOT EXISTS `characters`"));
        assert!(
            !all.contains("INSERT"),
            "seed rows must not reach the test schema"
        );
        assert!(
            !all.contains("REPLACE"),
            "seed rows must not reach the test schema"
        );
    }
}
