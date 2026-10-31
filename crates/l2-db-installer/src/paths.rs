//! Where the SQL scripts and `Interface.ini` are looked for when no option says otherwise.

use std::path::{Path, PathBuf};

use l2_config::{IniFile, InterfaceConfig};

/// The folder the installer runs from, for the "next to the exe" and "from the repo" cases.
#[derive(Debug, Clone, Default)]
pub struct Roots {
    /// Directory of the executable.
    pub exe_dir: Option<PathBuf>,
    /// Current working directory.
    pub cwd: Option<PathBuf>,
}

impl Roots {
    pub fn detect() -> Self {
        Self {
            exe_dir: std::env::current_exe()
                .ok()
                .and_then(|p| p.parent().map(Path::to_path_buf)),
            cwd: std::env::current_dir().ok(),
        }
    }

    /// `<sub>` below the places a `db_installer` folder may be: `../db_installer` next to the
    /// exe (a release layout), `dist/db_installer` from the repo root (also from `target/...`),
    /// and the folder of the working directory itself (the start scripts `cd` there).
    fn candidates(&self, sub: &str) -> Vec<PathBuf> {
        let mut out = Vec::new();
        if let Some(exe) = &self.exe_dir {
            out.push(exe.join("..").join("db_installer").join(sub));
            out.push(exe.join(sub));
            // target/release -> repo root
            out.push(exe.join("../..").join("dist/db_installer").join(sub));
        }
        if let Some(cwd) = &self.cwd {
            out.push(cwd.join(sub));
            out.push(cwd.join("dist/db_installer").join(sub));
            out.push(cwd.join("db_installer").join(sub));
            out.push(cwd.join("..").join("db_installer").join(sub));
        }
        out
    }
}

/// The first existing directory of the default places (see [`Roots`]), or `None`.
pub fn default_sql_dir(roots: &Roots) -> Option<PathBuf> {
    roots.candidates("sql").into_iter().find(|p| p.is_dir())
}

/// `Interface.ini` from the default places; the Java defaults when the file is missing.
pub fn load_interface(roots: &Roots) -> InterfaceConfig {
    let found = roots
        .candidates("config/Interface.ini")
        .into_iter()
        .find(|p| p.is_file());
    let ini = found
        .and_then(|p| IniFile::load(p).ok())
        .unwrap_or_else(|| IniFile::empty("Interface.ini"));
    InterfaceConfig::from_ini(&ini)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn finds_the_sql_folder_next_to_the_exe_layout() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join("bin")).unwrap();
        fs::create_dir_all(dir.path().join("db_installer/sql")).unwrap();
        let roots = Roots {
            exe_dir: Some(dir.path().join("bin")),
            cwd: None,
        };
        let found = default_sql_dir(&roots).unwrap();
        assert!(found.ends_with("db_installer/sql") || found.ends_with("db_installer\\sql"));
    }

    #[test]
    fn finds_dist_from_the_repo_root() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join("dist/db_installer/sql")).unwrap();
        let roots = Roots {
            exe_dir: None,
            cwd: Some(dir.path().to_path_buf()),
        };
        assert!(default_sql_dir(&roots).is_some());
    }

    #[test]
    fn nothing_found_is_none() {
        let dir = tempfile::tempdir().unwrap();
        let roots = Roots {
            exe_dir: None,
            cwd: Some(dir.path().to_path_buf()),
        };
        assert!(default_sql_dir(&roots).is_none());
    }

    #[test]
    fn interface_defaults_without_a_file_and_reads_the_file() {
        let dir = tempfile::tempdir().unwrap();
        let roots = Roots {
            exe_dir: None,
            cwd: Some(dir.path().to_path_buf()),
        };
        let cfg = load_interface(&roots);
        assert!(cfg.enable_gui && cfg.dark_theme);

        fs::create_dir_all(dir.path().join("config")).unwrap();
        fs::write(
            dir.path().join("config/Interface.ini"),
            "EnableGUI = False\nDarkTheme = True\n",
        )
        .unwrap();
        let cfg = load_interface(&roots);
        assert!(!cfg.enable_gui);
    }
}
