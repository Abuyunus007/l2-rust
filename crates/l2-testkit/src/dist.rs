//! The repo's own `dist/` folder (D-55): datapack, configs and SQL the tests read.
//!
//! Crates that cannot dev-depend on `l2-testkit` (dependency cycles) build the same path with
//! `concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist")`.

use std::path::PathBuf;

use crate::vectors::workspace_root;

/// `<repo>/dist`. `L2JMOBIUS_DIST` overrides it (e.g. to test against a newer release).
pub fn dist_dir() -> PathBuf {
    std::env::var_os("L2JMOBIUS_DIST").map_or_else(|| workspace_root().join("dist"), PathBuf::from)
}

/// `dist/game` (datapack in `data/`, configs in `config/`). `L2JMOBIUS_GAME_DIR` overrides it.
pub fn game_dir() -> PathBuf {
    std::env::var_os("L2JMOBIUS_GAME_DIR").map_or_else(|| dist_dir().join("game"), PathBuf::from)
}

/// `dist/login` (login server `config/` and `data/`).
pub fn login_dir() -> PathBuf {
    dist_dir().join("login")
}

/// `dist/db_installer/sql` (`game/` and `login/` hold one `.sql` file per table).
pub fn sql_dir() -> PathBuf {
    dist_dir().join("db_installer").join("sql")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folders_hang_off_the_dist_dir() {
        // Env overrides are process-wide, so only check the relations that hold either way.
        assert!(login_dir().starts_with(dist_dir()));
        assert!(sql_dir().starts_with(dist_dir()));
        assert!(sql_dir().ends_with("db_installer/sql"));
    }

    #[test]
    fn the_repo_dist_exists() {
        if std::env::var_os("L2JMOBIUS_DIST").is_some() {
            return;
        }
        assert!(game_dir().join("config").is_dir() || std::env::var_os("L2JMOBIUS_GAME_DIR").is_some());
        assert!(sql_dir().join("game").is_dir());
    }
}
