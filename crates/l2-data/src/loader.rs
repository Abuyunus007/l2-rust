//! The XML loading pattern every datapack loader copies.
//!
//! Java: `org/l2jmobius/commons/util/IXmlReader.java`
//!
//! # The pattern
//! A loader is one module with three parts:
//! 1. **Raw document structs** (private, `#[derive(Deserialize)]`) that mirror the XML. Element and
//!    attribute names are the XML names (`#[serde(rename = "staticData")]`, `#[serde(rename = "@val")]`).
//!    Where Java reads a value with a default, the struct has `#[serde(default = "...")]` with the
//!    same default; where Java reads it without one, the field is required.
//! 2. **The public table** (immutable once built): plain data plus lookup methods. It is shared as
//!    `Arc<Table>` by [`crate::Datapack`].
//! 3. **`Table::load(root)`**: calls [`load_dir`] or [`load_file`] with a closure that turns one raw
//!    document into table entries, then logs the Java count line with [`log_loaded`].
//!
//! # Error policy (same as Java's `parseFile`)
//! A file that is missing, not valid XML, or fails the loader's closure is logged with `warn!` and
//! skipped. Loading continues and the count line shows what actually loaded. Entries the closure
//! already inserted before it returned an error stay (Java behaves the same way).
//!
//! # Unknown elements and attributes
//! Java does not reject them: it reads only the nodes it knows (schema validation errors are printed
//! by the XML parser but do not stop loading). serde ignores unknown elements and attributes by
//! default, so raw structs simply must not use `deny_unknown_fields`.
//!
//! NOTE(java-quirk): Java checks the root element is named `list` (or whatever the loader looks for)
//! and silently loads nothing otherwise. serde does not look at the root name, so a wrong root name
//! is only noticed when required fields are missing.

use std::path::{Path, PathBuf};

use serde::de::DeserializeOwned;
use tracing::{info, warn};

/// Why one datapack file (or the datapack as a whole) could not be loaded.
#[derive(Debug, thiserror::Error)]
pub enum DataError {
    /// The file could not be read.
    #[error("cannot read {path}: {source}")]
    Io {
        /// The file or directory.
        path: PathBuf,
        /// The OS error.
        source: std::io::Error,
    },
    /// The file is not valid XML, or does not have the shape the loader expects.
    #[error("cannot parse {path}: {source}")]
    Xml {
        /// The file.
        path: PathBuf,
        /// What quick-xml/serde reported.
        source: quick_xml::DeError,
    },
    /// The file parsed, but its content is not acceptable (e.g. an unknown class id).
    #[error("invalid data: {0}")]
    Invalid(String),
    /// The datapack root has no `data` folder.
    #[error("datapack root {0} has no data folder")]
    MissingRoot(PathBuf),
}

/// Parses one XML file into `T`. Used by [`load_file`] and [`load_dir`]; also handy in tests.
pub fn parse_file<T: DeserializeOwned>(path: &Path) -> Result<T, DataError> {
    let text = std::fs::read_to_string(path).map_err(|source| DataError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    // A UTF-8 byte order mark would be read as text before the root element.
    let text = text.strip_prefix('\u{feff}').unwrap_or(&text);
    quick_xml::de::from_str(text).map_err(|source| DataError::Xml {
        path: path.to_path_buf(),
        source,
    })
}

/// Java `parseDatapackFile`: parses one file and hands the document to `handle`.
/// Problems are logged and swallowed (see the module docs).
pub fn load_file<T, F>(path: &Path, mut handle: F)
where
    T: DeserializeOwned,
    F: FnMut(T) -> Result<(), DataError>,
{
    if !is_xml_file(path) {
        warn!(
            "Cannot parse {}: file does not exist or is not valid.",
            file_name(path)
        );
        return;
    }
    let result = parse_file::<T>(path).and_then(&mut handle);
    if let Err(error) = result {
        warn!("Error parsing {}: {error}", file_name(path));
    }
}

/// Java `parseDatapackDirectory`: parses every `.xml` file in `dir` (and its subfolders when
/// `recursive`). Files are visited in sorted path order so results do not depend on the OS; when
/// two files define the same key, the later path wins.
pub fn load_dir<T, F>(dir: &Path, recursive: bool, mut handle: F)
where
    T: DeserializeOwned,
    F: FnMut(T) -> Result<(), DataError>,
{
    if !dir.is_dir() {
        warn!("Directory not found: {}", dir.display());
        return;
    }
    let mut files = Vec::new();
    collect_xml_files(dir, recursive, &mut files);
    files.sort();
    for file in files {
        load_file(&file, &mut handle);
    }
}

/// Logs the count line, in Java's format: `PlayerTemplateData: Loaded 103 player templates.`
pub fn log_loaded(loader: &str, count: usize, what: &str) {
    info!("{loader}: Loaded {count} {what}.");
}

fn collect_xml_files(dir: &Path, recursive: bool, out: &mut Vec<PathBuf>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(error) => {
            warn!("Cannot list {}: {error}", dir.display());
            return;
        }
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if recursive {
                collect_xml_files(&path, true, out);
            }
        } else if is_xml_file(&path) {
            out.push(path);
        }
    }
}

/// Java `isValidXmlFile`: a regular file whose name ends in `.xml` (any case).
fn is_xml_file(path: &Path) -> bool {
    path.is_file() && path.extension().is_some_and(|e| e.eq_ignore_ascii_case("xml"))
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use std::fs;

    use serde::Deserialize;

    use super::*;

    #[derive(Deserialize)]
    struct Doc {
        #[serde(rename = "@id")]
        id: u32,
    }

    fn write(dir: &Path, name: &str, text: &str) {
        fs::write(dir.join(name), text).unwrap();
    }

    fn ids_in(dir: &Path, recursive: bool) -> Vec<u32> {
        let mut ids = Vec::new();
        load_dir::<Doc, _>(dir, recursive, |doc| {
            ids.push(doc.id);
            Ok(())
        });
        ids
    }

    #[test]
    fn unknown_attributes_and_elements_are_tolerated() {
        let dir = tempfile::tempdir().unwrap();
        write(
            dir.path(),
            "a.xml",
            r#"<?xml version="1.0"?><list id="7" extra="x"><unknown>1</unknown></list>"#,
        );
        assert_eq!(ids_in(dir.path(), false), vec![7]);
    }

    #[test]
    fn bad_file_is_skipped_and_the_rest_still_loads() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "a.xml", r#"<list id="1"/>"#);
        write(dir.path(), "b.xml", "<list id=");
        write(dir.path(), "c.xml", r#"<list/>"#); // required attribute missing
        write(dir.path(), "d.xml", r#"<list id="4"/>"#);
        assert_eq!(ids_in(dir.path(), false), vec![1, 4]);
    }

    #[test]
    fn handler_error_skips_the_file_only() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "a.xml", r#"<list id="1"/>"#);
        write(dir.path(), "b.xml", r#"<list id="2"/>"#);
        let mut seen = Vec::new();
        load_dir::<Doc, _>(dir.path(), false, |doc| {
            seen.push(doc.id);
            if doc.id == 1 {
                Err(DataError::Invalid("no".into()))
            } else {
                Ok(())
            }
        });
        assert_eq!(seen, vec![1, 2]);
    }

    #[test]
    fn only_xml_files_are_read_in_sorted_order() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "b.XML", r#"<list id="2"/>"#);
        write(dir.path(), "a.xml", r#"<list id="1"/>"#);
        write(dir.path(), "c.txt", r#"<list id="3"/>"#);
        assert_eq!(ids_in(dir.path(), false), vec![1, 2]);
    }

    #[test]
    fn subfolders_only_when_recursive() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "a.xml", r#"<list id="1"/>"#);
        fs::create_dir(dir.path().join("sub")).unwrap();
        write(&dir.path().join("sub"), "b.xml", r#"<list id="2"/>"#);
        assert_eq!(ids_in(dir.path(), false), vec![1]);
        assert_eq!(ids_in(dir.path(), true), vec![1, 2]);
    }

    #[test]
    fn missing_directory_and_file_load_nothing() {
        let dir = tempfile::tempdir().unwrap();
        assert!(ids_in(&dir.path().join("nope"), true).is_empty());
        let mut called = false;
        load_file::<Doc, _>(&dir.path().join("nope.xml"), |_| {
            called = true;
            Ok(())
        });
        assert!(!called);
    }

    #[test]
    fn byte_order_mark_is_ignored() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "a.xml", "\u{feff}<list id=\"9\"/>");
        assert_eq!(ids_in(dir.path(), false), vec![9]);
    }
}
