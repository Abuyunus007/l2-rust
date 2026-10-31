#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Loads the real Java html files and teleporter lists. Skipped when the datapack folder does not
//! exist (CI).
//!
//! The folder defaults to `dist/game`; set `L2JMOBIUS_GAME_DIR` to use another.
//! The expected numbers were counted from the files with grep (198 teleporter XML files, one
//! `<npc id=` each, no `<npcs>` alias) and by walking the folders in the test itself.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use l2_data::{HtmCache, TeleportType, TeleporterData};

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

fn html_cache() -> Option<&'static HtmCache> {
    static CACHE: OnceLock<HtmCache> = OnceLock::new();
    let dir = game_dir()?;
    Some(CACHE.get_or_init(|| HtmCache::load(&dir, false)))
}

fn teleporters() -> Option<&'static TeleporterData> {
    static DATA: OnceLock<TeleporterData> = OnceLock::new();
    let dir = game_dir()?;
    Some(DATA.get_or_init(|| TeleporterData::load(&dir)))
}

/// Counts `.htm` / `.html` files below `dir` with a plain walk (independent of the loader).
fn count_html_files(dir: &Path) -> usize {
    let mut count = 0;
    let Ok(entries) = std::fs::read_dir(dir) else {
        return 0;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            count += count_html_files(&path);
        } else {
            let name = entry.file_name().to_string_lossy().to_lowercase();
            if name.ends_with(".htm") || name.ends_with(".html") {
                count += 1;
            }
        }
    }
    count
}

#[test]
fn every_html_file_of_data_html_is_cached() {
    let Some(cache) = html_cache() else { return };
    let dir = game_dir().unwrap();
    // data/html holds 4062 .htm/.html files (plus admin/documentation.txt, which is not html).
    let expected = count_html_files(&dir.join("data/html"));
    assert_eq!(expected, 4062);
    // The cache walks the whole datapack root (Java's DATAPACK_ROOT), so it holds data/html and more.
    let cached = cache.loaded_files();
    assert!(cached >= expected, "{cached} cached, {expected} in data/html");
    assert!(cache.contains("data/html/teleporter/30006.htm"));
}

#[test]
fn html_text_is_the_file_with_tabs_and_newlines_removed() {
    let Some(cache) = html_cache() else { return };
    let dir = game_dir().unwrap();
    let raw = std::fs::read(dir.join("data/html/teleporter/30006.htm")).unwrap();
    let raw_text = String::from_utf8(raw).unwrap();
    let text = cache.get("data/html/teleporter/30006.htm").unwrap();
    // The shipped file is CRLF: the \r stays, only \n (and tabs) go.
    assert_eq!(text, raw_text.replace(['\n', '\t'], ""));
    assert!(text.starts_with("<html><body>Gatekeeper Roxxy:<br>\r"));
    assert!(text.ends_with("</body></html>"));
    assert!(text.contains("%objectId%"));
}

#[test]
fn comments_are_removed_from_cached_text() {
    let Some(cache) = html_cache() else { return };
    let dir = game_dir().unwrap();
    // olympiad/hero_main.htm has comments (grep counts them); the cached text must have none left.
    let raw = std::fs::read(dir.join("data/html/olympiad/hero_main.htm")).unwrap();
    let raw_text = String::from_utf8_lossy(&raw).into_owned();
    assert!(raw_text.contains("<!--"));
    let text = cache.get("data/html/olympiad/hero_main.htm").unwrap();
    assert!(!text.contains("<!--"));
    assert!(!text.contains('\n') && !text.contains('\t'));
    assert!(cache.contains("data/html/teleporter/teleports.htm"));
    assert!(cache.get("data/html/teleporter/missing.htm").is_none());
    assert!(cache.get("DATA/HTML/teleporter/30006.htm").is_none());
}

#[test]
fn localised_lookup_uses_the_plain_path_without_a_translation() {
    let Some(cache) = html_cache() else { return };
    let plain = cache.get("data/html/teleporter/30006.htm").unwrap();
    assert_eq!(
        cache.get_localized("data/lang/xx/", "data/html/teleporter/30006.htm"),
        Some(plain)
    );
    assert_eq!(cache.get_localized("", "data/html/teleporter/nope.htm"), None);
}

#[test]
fn cache_reports_memory_like_java() {
    let Some(cache) = html_cache() else { return };
    assert!(cache.memory_mb() > 1.0);
    assert!(cache.stored_bytes() > 0);
    eprintln!(
        "html cache: {} files, {:.3} MiB raw, {:.3} MiB stored",
        cache.loaded_files(),
        cache.memory_mb(),
        cache.stored_bytes() as f64 / 1_048_576.0
    );
}

#[test]
fn gatekeeper_30006_has_its_four_lists() {
    let Some(data) = teleporters() else { return };
    let normal = data.get_holder(30006, "NORMAL").unwrap();
    assert_eq!(normal.kind(), TeleportType::Normal);
    assert_eq!(normal.locations().len(), 10);
    let gludin = normal.location(0).unwrap();
    assert_eq!(gludin.name.as_deref(), Some("The Village of Gludin"));
    assert_eq!((gludin.x, gludin.y, gludin.z), (-80684, 149770, -3040));
    assert_eq!(gludin.fee_id, 57);
    assert_eq!(gludin.fee_count, 18000);
    assert!(gludin.castle_ids.is_empty());

    let token = data.get_holder(30006, "NOBLES_TOKEN").unwrap();
    assert!(token.is_noblesse());
    assert_eq!(token.locations().len(), 4);
    assert_eq!(token.location(0).unwrap().fee_id, 13722);
    assert_eq!(token.location(0).unwrap().fee_count, 1);

    let adena = data.get_holder(30006, "NOBLES_ADENA").unwrap();
    assert_eq!(adena.kind(), TeleportType::NoblesAdena);
    assert_eq!(adena.locations().len(), 4);
    assert_eq!(adena.location(3).unwrap().fee_count, 1000);

    // The OTHER list has one location with no name and no fee.
    let other = data.get_holder(30006, "OTHER").unwrap();
    assert_eq!(other.name(), "OTHER");
    let airship = other.location(0).unwrap();
    assert_eq!(airship.name, None);
    assert_eq!((airship.fee_count, airship.fee_id), (0, 57));
    assert!(other.location(1).is_none());
}

#[test]
fn second_gatekeeper_30059_has_three_lists() {
    let Some(data) = teleporters() else { return };
    assert!(data.get_holder(30059, "NORMAL").is_some());
    assert!(data.get_holder(30059, "NOBLES_TOKEN").is_some());
    assert!(data.get_holder(30059, "NOBLES_ADENA").is_some());
    assert!(data.get_holder(30059, "OTHER").is_none());
    assert!(data.get_holder(30006, "NOBLES_ADENA").unwrap().is_noblesse());
}

#[test]
fn teleporter_npc_count_is_the_number_of_distinct_ids() {
    let Some(data) = teleporters() else { return };
    // 198 files with one `<npc>` each (grep), no `<npcs>` element anywhere. Three ids are in two
    // files (30256, 30719, 32163), so Java's `size()` of its map counts 195 NPCs.
    assert_eq!(data.npc_count(), 195);
    assert!(!data.is_empty());
    assert!(data.get_holder(1, "NORMAL").is_none());
}
