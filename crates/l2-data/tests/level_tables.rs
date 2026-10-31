#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Loads the real `experienceLoss.xml` and `karmaLoss.xml` (M4-EXP-01). Skipped when the folder
//! does not exist (CI).
//!
//! The folder defaults to `dist/game`; set `L2JMOBIUS_GAME_DIR` to use another.
//! The expected numbers are read off the XML files.

use std::path::PathBuf;

use l2_data::{ExperienceLoss, KarmaLoss};

fn game_dir() -> Option<PathBuf> {
    let dir = std::env::var_os("L2JMOBIUS_GAME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game")));
    if dir.join("data").is_dir() {
        Some(dir)
    } else {
        eprintln!("skipped: {} does not exist", dir.display());
        None
    }
}

#[test]
fn experience_loss_matches_the_xml() {
    let Some(dir) = game_dir() else { return };
    let table = ExperienceLoss::load(&dir);
    assert_eq!(table.percent_lost(1), 10.0);
    assert_eq!(table.percent_lost(2), 9.875);
    assert_eq!(table.percent_lost(20), 7.625);
    assert_eq!(table.percent_lost(40), 5.125);
    assert_eq!(table.percent_lost(60), 4.0);
    assert_eq!(table.percent_lost(85), 1.0);
    // A level past the table gives the last entry.
    assert_eq!(table.percent_lost(120), 1.0);
}

#[test]
fn karma_loss_matches_the_xml() {
    let Some(dir) = game_dir() else { return };
    let table = KarmaLoss::load(&dir);
    assert_eq!(table.modifier(1), 0.772184315);
    assert_eq!(table.modifier(2), 2.069377971);
    assert_eq!(table.modifier(85), 35.25374225);
}
