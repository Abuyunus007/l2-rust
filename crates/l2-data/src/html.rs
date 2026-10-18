//! The html cache: every `.htm` / `.html` file of the datapack, read once at startup.
//!
//! Java: `gameserver/cache/HtmCache.java`.
//!
//! # What Java does, and what is kept
//! * Java walks the **whole datapack root** (`ServerConfig.DATAPACK_ROOT`, the `dist/game` folder),
//!   not only `data/html`. Scripts keep their own html under `data/scripts/**` and look it up by the
//!   same kind of path, so this port walks the root too. The shipped datapack has about 4,000
//!   files under `data/html` and about 17,000 more under `data/scripts`.
//! * Only files whose name ends in `.htm` or `.html` (any case) are read. Subfolders are walked.
//! * The key is the path relative to the datapack root with `/` separators and the file's own
//!   spelling, e.g. `data/html/teleporter/30006.htm`. Lookups are exact and case sensitive, like
//!   Java's `HashMap`; the caller passes the same key Java code would pass.
//! * The text is decoded as UTF-8 (invalid bytes become U+FFFD, like `new String(bytes, UTF_8)`),
//!   then every `<!-- ... -->` comment is removed, then every tab and every `\n` is removed. A `\r`
//!   is *not* removed, so the shipped CRLF files keep their `\r` characters, exactly as the client
//!   receives them from Java. An unclosed `<!--` stays in the text.
//!
//! # Lazy cache
//! `General.ini` `HtmCache` (code default true) selects Java's preload. With `false` Java reads
//! files on request instead, and the shipped `General.ini` says `False`. The result for the client
//! is the same, so this port always preloads (no file IO in the world, design doc
//! `m3-world.md`). [`HtmCache::load`] logs the note once when the caller asks for lazy mode.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use tracing::{info, warn};

/// Java `HtmCache.HTML_CACHE` plus the counters `getMemoryUsage()` and `getLoadedFiles()` use.
#[derive(Debug, Default, Clone)]
pub struct HtmCache {
    /// Processed text by datapack-relative path (`data/html/teleporter/30006.htm`).
    files: HashMap<String, String>,
    /// Raw byte size of every loaded file, as Java counts it (before comments are removed).
    raw_bytes: u64,
}

impl HtmCache {
    /// Loads every html file below `root` (the folder that contains `data/`), with Java's code
    /// default for `CheckHtmlEncoding` (true). See [`HtmCache::load_with`].
    pub fn load(root: &Path, lazy_cache_requested: bool) -> Self {
        Self::load_with(root, lazy_cache_requested, true)
    }

    /// Loads every html file below `root` (the folder that contains `data/`).
    ///
    /// `lazy_cache_requested` is Java's `HtmCache = False`: the cache still preloads, and the
    /// note is logged once. A missing root is not an error: Java logs nothing in that case either
    /// and the cache is simply empty.
    ///
    /// `check_encoding` is `General.ini` `CheckHtmlEncoding`: when true, a file with non ASCII text
    /// outside `data/lang` is logged. The text is loaded the same way either way.
    pub fn load_with(root: &Path, lazy_cache_requested: bool, check_encoding: bool) -> Self {
        if lazy_cache_requested {
            info!(
                "Cache[HTML]: LazyCache is set, but html is preloaded (same text, no file IO in the world)."
            );
        }
        info!("Html cache start...");
        let mut cache = Self::default();
        cache.parse_dir(root, root, check_encoding);
        info!(
            "Cache[HTML]: {:.3} megabytes on {} files loaded.",
            cache.memory_mb(),
            cache.loaded_files()
        );
        cache
    }

    /// Java `getHtm` for a path that is already complete (no language prefix).
    pub fn get(&self, path: &str) -> Option<&str> {
        self.files.get(path).map(String::as_str)
    }

    /// Java `getHtm(player, path)` without the lazy-load and GM debug parts: the path is tried with
    /// the player's language `prefix` first (`data/lang/<lang>/` in Java) and falls back to the plain
    /// `path` when the localised file does not exist. An empty prefix means no localisation.
    pub fn get_localized(&self, prefix: &str, path: &str) -> Option<&str> {
        let localised = format!("{prefix}{path}");
        match self.get(&localised) {
            Some(text) => Some(text),
            None if !prefix.is_empty() => self.get(path),
            None => None,
        }
    }

    /// Java `contains(path)`.
    pub fn contains(&self, path: &str) -> bool {
        self.files.contains_key(path)
    }

    /// Java `getLoadedFiles()`: the number of files in the cache.
    pub fn loaded_files(&self) -> usize {
        self.files.len()
    }

    /// Java `getMemoryUsage()`: raw megabytes (1 MiB = 1048576 bytes) of the loaded files.
    pub fn memory_mb(&self) -> f64 {
        self.raw_bytes as f64 / 1_048_576.0
    }

    /// Bytes this process keeps for the cached text (the processed strings, not the raw files).
    pub fn stored_bytes(&self) -> usize {
        self.files.iter().map(|(key, text)| key.len() + text.len()).sum()
    }

    /// True when no file was loaded.
    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }

    /// Java `parseDir`: every entry of `dir`, recursively. Unreadable folders are skipped.
    fn parse_dir(&mut self, root: &Path, dir: &Path, check_encoding: bool) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        let mut paths: Vec<PathBuf> = entries.flatten().map(|entry| entry.path()).collect();
        // Java lists files in the OS order; sorting keeps the warnings and the count the same on
        // every platform.
        paths.sort();
        for path in paths {
            if path.is_dir() {
                self.parse_dir(root, &path, check_encoding);
            } else {
                self.load_file(root, &path, check_encoding);
            }
        }
    }

    /// Java `loadFile`: reads one html file into the cache. Problems are logged and skipped.
    fn load_file(&mut self, root: &Path, file: &Path, check_encoding: bool) {
        if !file.is_file() || !is_html_name(file) {
            return;
        }
        let Some(key) = datapack_key(root, file) else {
            return;
        };
        let raw = match std::fs::read(file) {
            Ok(raw) => raw,
            Err(error) => {
                warn!("Problem with htm file {key}: {error}");
                return;
            }
        };
        let text = process_html(&raw);
        if warns_about_encoding(&key, &text, check_encoding) {
            warn!("HTML encoding check: File {key} contains non ASCII content.");
        }
        self.raw_bytes += raw.len() as u64;
        self.files.insert(key, text);
    }
}

/// Java `loadFile` with `GeneralConfig.CHECK_HTML_ENCODING`: non ASCII text is reported, except
/// under `data/lang`, where the translations may use other characters.
// Java: gameserver/cache/HtmCache.java:140
fn warns_about_encoding(key: &str, text: &str, check_encoding: bool) -> bool {
    check_encoding && !key.starts_with("data/lang") && !text.is_ascii()
}

/// Java `HtmCache.loadFile` name check: the lower-cased name ends in `.htm` or `.html`.
fn is_html_name(file: &Path) -> bool {
    file.file_name().is_some_and(|name| {
        let name = name.to_string_lossy().to_lowercase();
        name.ends_with(".htm") || name.ends_with(".html")
    })
}

/// The key Java builds from the URI path: the path under the datapack root with `/` separators.
fn datapack_key(root: &Path, file: &Path) -> Option<String> {
    let relative = file.strip_prefix(root).ok()?;
    let parts: Vec<String> = relative
        .components()
        .map(|part| part.as_os_str().to_string_lossy().into_owned())
        .collect();
    Some(parts.join("/"))
}

/// Java `loadFile` text handling: UTF-8 decode, comment removal, tab and newline removal.
///
/// NOTE(java-quirk): the comment regex `(?s)<!--.*?-->` has no nesting, so `<!-- a <!-- b -->`
/// removes everything up to the first `-->`. The same happens here.
pub(crate) fn process_html(raw: &[u8]) -> String {
    let text = String::from_utf8_lossy(raw);
    let without_comments = remove_comments(&text);
    without_comments
        .chars()
        .filter(|c| *c != '\t' && *c != '\n')
        .collect()
}

/// `replaceAll("(?s)<!--.*?-->", "")`: a comment with no closing `-->` is left in the text.
fn remove_comments(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find("<!--") {
        let after_open = &rest[start + 4..];
        let Some(end) = after_open.find("-->") else {
            break;
        };
        out.push_str(&rest[..start]);
        rest = &after_open[end + 3..];
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    fn write(root: &Path, relative: &str, bytes: &[u8]) {
        let path = root.join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, bytes).unwrap();
    }

    #[test]
    fn comments_tabs_and_newlines_are_removed_but_cr_stays() {
        assert_eq!(process_html(b"a<!-- x\n-->b\r\n\tc\n"), "ab\rc");
        assert_eq!(process_html(b"<!-- one --><!-- two -->z"), "z");
    }

    #[test]
    fn unclosed_comment_stays() {
        assert_eq!(process_html(b"a<!-- never closed\nb"), "a<!-- never closedb");
    }

    #[test]
    fn only_html_names_are_loaded_recursively_with_relative_keys() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "data/html/a.htm", b"<html>A</html>");
        write(dir.path(), "data/html/sub/B.HTML", b"B");
        write(dir.path(), "data/html/notes.txt", b"not html");
        write(dir.path(), "data/scripts/q.java", b"not html");
        let cache = HtmCache::load(dir.path(), false);
        assert_eq!(cache.loaded_files(), 2);
        assert_eq!(cache.get("data/html/a.htm"), Some("<html>A</html>"));
        assert_eq!(cache.get("data/html/sub/B.HTML"), Some("B"));
        assert!(!cache.contains("data/html/notes.txt"));
        // Lookups are case sensitive, like Java's HashMap.
        assert!(cache.get("data/html/A.htm").is_none());
    }

    #[test]
    fn localised_lookup_falls_back_to_the_plain_path() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "data/html/x.htm", b"plain");
        write(dir.path(), "data/lang/en/data/html/x.htm", b"english");
        let cache = HtmCache::load(dir.path(), false);
        assert_eq!(
            cache.get_localized("data/lang/en/", "data/html/x.htm"),
            Some("english")
        );
        assert_eq!(
            cache.get_localized("data/lang/fr/", "data/html/x.htm"),
            Some("plain")
        );
        assert_eq!(cache.get_localized("", "data/html/missing.htm"), None);
    }

    #[test]
    fn encoding_warning_follows_the_check_flag() {
        assert!(warns_about_encoding("data/html/a.htm", "Zoë", true));
        assert!(!warns_about_encoding("data/html/a.htm", "Zoë", false));
        assert!(!warns_about_encoding("data/html/a.htm", "plain", true));
        // Translations may use other characters.
        assert!(!warns_about_encoding("data/lang/en/data/html/a.htm", "Zoë", true));
    }

    #[test]
    fn the_check_flag_does_not_change_the_loaded_text() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "data/html/u.htm", "Zoë".as_bytes());
        let checked = HtmCache::load_with(dir.path(), false, true);
        let unchecked = HtmCache::load_with(dir.path(), false, false);
        assert_eq!(checked.get("data/html/u.htm"), Some("Zoë"));
        assert_eq!(unchecked.get("data/html/u.htm"), Some("Zoë"));
    }

    #[test]
    fn missing_root_gives_an_empty_cache() {
        let dir = tempfile::tempdir().unwrap();
        let cache = HtmCache::load(&dir.path().join("nope"), true);
        assert!(cache.is_empty());
        assert_eq!(cache.memory_mb(), 0.0);
    }
}
