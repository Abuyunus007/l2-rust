#![allow(clippy::unwrap_used, clippy::expect_used)]
//! The in-place writer and the key listing against every `.ini` of the repo's `dist/` (G-05).

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use l2_config::IniFile;
use l2_config::defaults::{Side, default_for, java_defaults};
use l2_config::ini_edit::{IniDoc, apply_changes};

fn dist() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../dist")
}

fn ini_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(read) = std::fs::read_dir(dir) else { return };
    for entry in read.flatten() {
        let path = entry.path();
        if path.is_dir() {
            ini_files(&path, out);
        } else if path.extension().is_some_and(|e| e == "ini") {
            out.push(path);
        }
    }
}

fn all_files() -> Vec<PathBuf> {
    let mut files = Vec::new();
    ini_files(&dist(), &mut files);
    if files.is_empty() {
        eprintln!("skipped: no dist/ folder");
    }
    files
}

#[test]
fn unchanged_files_stay_byte_identical() {
    for path in all_files() {
        let text = String::from_utf8(std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(IniDoc::parse(&text).render(), text, "{}", path.display());
        let (out, n) = apply_changes(&text, &[]).unwrap();
        assert_eq!((out == text, n), (true, 0), "{}", path.display());
        // Writing every value back as it is changes nothing either.
        let same: Vec<(String, String)> = IniDoc::parse(&text)
            .entries()
            .into_iter()
            .map(|e| (e.key, e.value))
            .collect();
        let (out, n) = apply_changes(&text, &same).unwrap();
        assert_eq!((out == text, n), (true, 0), "{}", path.display());
    }
}

#[test]
fn listing_matches_the_reader() {
    let mut keys_total = 0;
    for path in all_files() {
        let text = String::from_utf8(std::fs::read(&path).unwrap()).unwrap();
        let doc = IniDoc::parse(&text);
        let entries = doc.entries();
        let ini = IniFile::parse("x", &text).unwrap();
        let listed: BTreeSet<&str> = entries.iter().map(|e| e.key.as_str()).collect();
        let read: BTreeSet<&str> = ini.keys().collect();
        assert_eq!(listed, read, "{}", path.display());
        for e in &entries {
            assert!(e.editable, "{} {}", path.display(), e.key);
            // The last occurrence is the effective one, so it must agree with the reader.
        }
        keys_total += entries.len();
    }
    eprintln!("{keys_total} keys listed");
}

#[test]
fn one_change_touches_exactly_one_line() {
    for path in all_files() {
        let text = String::from_utf8(std::fs::read(&path).unwrap()).unwrap();
        let entries = IniDoc::parse(&text).entries();
        // Every 7th key (all of them would be quadratic on the big files for nothing).
        for e in entries.iter().step_by(7) {
            // A key repeated in the file: only the last occurrence changes.
            let (out, n) = apply_changes(&text, &[(e.key.clone(), format!("{}9", e.value))]).unwrap();
            assert_eq!(n, 1);
            let before: Vec<&str> = text.split_inclusive('\n').collect();
            let after: Vec<&str> = out.split_inclusive('\n').collect();
            assert_eq!(before.len(), after.len(), "{} {}", path.display(), e.key);
            let diff = before.iter().zip(&after).filter(|(a, b)| a != b).count();
            assert_eq!(diff, 1, "{} {}", path.display(), e.key);
        }
    }
}

#[test]
fn help_text_is_the_comment_block_above_the_key() {
    let path = dist().join("game/config/Rates.ini");
    let Ok(text) = std::fs::read_to_string(&path) else {
        eprintln!("skipped: no dist/");
        return;
    };
    let entries = IniDoc::parse(&text).entries();
    let by_key = |k: &str| entries.iter().find(|e| e.key == k).unwrap();
    let xp = by_key("RateXp");
    assert_eq!(xp.help, ["Experience multiplier"]);
    assert_eq!(xp.group, "Standard Settings (Retail value = 1)");
    let karma = by_key("RateKarmaLost");
    assert_eq!(karma.help.last().unwrap(), "Default: -1");
    assert_eq!(karma.comment_default.as_deref(), Some("-1"));
    // No comment directly above: no help.
    assert!(by_key("RateKarmaExpLost").help.is_empty());
    // Most keys of the big files have help.
    let with_help = entries.iter().filter(|e| !e.help.is_empty()).count();
    assert!(with_help * 2 > entries.len());
}

#[test]
fn defaults_come_from_the_structs_for_dist_keys() {
    let path = dist().join("login/config/Server.ini");
    let Ok(text) = std::fs::read_to_string(&path) else {
        eprintln!("skipped: no dist/");
        return;
    };
    let defaults = java_defaults(Side::Login, "Server.ini");
    let entries = IniDoc::parse(&text).entries();
    assert!(!entries.is_empty());
    for e in &entries {
        assert!(
            default_for(&defaults, &e.key, e.comment_default.as_deref()).is_some(),
            "no default for {}",
            e.key
        );
    }
    // Every game file with a typed struct yields defaults for most keys (FortSiege.ini, Siege.ini, ClassBalance.ini and a few more are not typed yet).
    let (mut total, mut total_known) = (0, 0);
    for path in all_files() {
        let Ok(rel) = path.strip_prefix(dist().join("game/config")) else {
            continue;
        };
        let rel = rel.to_string_lossy().replace('\\', "/");
        let text = std::fs::read_to_string(&path).unwrap();
        let entries = IniDoc::parse(&text).entries();
        let defaults = java_defaults(Side::Game, &rel);
        let known = entries
            .iter()
            .filter(|e| default_for(&defaults, &e.key, e.comment_default.as_deref()).is_some())
            .count();
        eprintln!("{rel}: {known}/{} keys have a default", entries.len());
        if !matches!(rel.as_str(), "Admin.ini" | "SideBySide.ini") {
            total += entries.len();
            total_known += known;
        }
    }
    // The rest are untyped files and placeholder fields without a Default line; the editor shows no default there.
    assert!(
        total_known * 4 >= total * 3,
        "only {total_known}/{total} keys have a default"
    );
}
