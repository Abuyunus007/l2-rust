//! Parses every `.ini` of the Java distribution. Skipped when that folder does not exist (CI).
//!
//! The folder defaults to `dist`; set `L2JMOBIUS_DIST` to use another one.

use std::path::{Path, PathBuf};

use l2_config::IniFile;

fn dist_dir() -> PathBuf {
    std::env::var_os("L2JMOBIUS_DIST")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist")))
}

fn collect_ini_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_ini_files(&path, out);
        } else if path.extension().is_some_and(|e| e.eq_ignore_ascii_case("ini")) {
            out.push(path);
        }
    }
}

#[test]
fn parses_every_dist_ini() {
    let dist = dist_dir();
    if !dist.is_dir() {
        eprintln!("skipped: {} does not exist", dist.display());
        return;
    }
    let mut files = Vec::new();
    collect_ini_files(&dist, &mut files);
    files.sort();
    assert!(!files.is_empty(), "no .ini files under {}", dist.display());

    let mut total_keys = 0;
    for path in &files {
        let file = IniFile::load(path).unwrap_or_else(|e| panic!("{} failed to parse: {e}", path.display()));
        assert!(!file.is_empty(), "{} has no keys", path.display());
        total_keys += file.len();
    }
    eprintln!("{} ini files, {total_keys} keys", files.len());
}

#[test]
fn known_values_in_dist_files() {
    let dist = dist_dir();
    if !dist.is_dir() {
        eprintln!("skipped: {} does not exist", dist.display());
        return;
    }
    // Values copied from the shipped files; they catch a reader that mangles real input.
    let server = IniFile::load(dist.join("login/config/Server.ini")).unwrap();
    assert_eq!(server.get_value("LoginserverPort"), Some("2106"));
    assert_eq!(server.get_int("LoginserverPort", 0), 2106);
}
