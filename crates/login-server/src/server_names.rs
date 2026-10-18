//! Game server names from `data/servername.xml` (server id -> display name).
// Java: org/l2jmobius/loginserver/data/GameServerTable.java (`load`, `parseDocument`)
// Java: org/l2jmobius/commons/util/IXmlReader.java (`parseFile`, `parseInteger`)

use std::collections::BTreeMap;
use std::path::Path;

use quick_xml::events::{BytesStart, Event};
use quick_xml::{Reader, XmlVersion};
use tracing::{info, warn};

/// Name of the file inside the data directory.
pub const SERVER_NAMES_FILE: &str = "servername.xml";

/// Server id -> name, ordered by id.
///
/// Java keeps a `HashMap<Integer, String>`. For the small ids used here its iteration order is
/// ascending, which `registerWithFirstAvailableId` relies on, so a sorted map matches it.
pub type ServerNames = BTreeMap<i32, String>;

/// Why parsing stopped. The names read before the problem are kept (Java: the exception ends
/// `parseDocument` half way and `parseFile` only logs it).
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ServerNamesError {
    #[error("malformed XML: {0}")]
    Xml(String),
    #[error("<server> element without a usable id attribute (id={0:?})")]
    BadId(Option<String>),
}

/// Reads `path` and returns its names. Never fails: a missing or broken file is logged and gives
/// the names parsed so far, like Java.
// Java: GameServerTable.load
pub fn load_server_names(path: &Path) -> ServerNames {
    let mut names = ServerNames::new();
    if !path.is_file() {
        warn!(
            "Cannot parse {}: file does not exist or is not valid.",
            path.file_name().map_or_else(
                || path.display().to_string(),
                |name| name.to_string_lossy().into_owned()
            )
        );
    } else {
        match std::fs::read(path) {
            Ok(bytes) => {
                if let Err(error) = parse_server_names(&String::from_utf8_lossy(&bytes), &mut names) {
                    warn!("Could not parse file {}: {error}", path.display());
                }
            }
            Err(error) => warn!("Could not read file {}: {error}", path.display()),
        }
    }
    info!("Loaded {} server names.", names.len());
    names
}

/// Adds every `<server id=".." name=".."/>` of `xml` to `names`. A later entry with the same id
/// replaces the earlier one (Java `HashMap.put`).
// Java: GameServerTable.parseDocument
pub fn parse_server_names(xml: &str, names: &mut ServerNames) -> Result<(), ServerNamesError> {
    let mut reader = Reader::from_str(xml);
    loop {
        match reader.read_event() {
            Ok(Event::Start(element) | Event::Empty(element)) if element.name().as_ref() == "server" => {
                let id_text = attribute(&element, "id")?;
                let id = id_text
                    .as_deref()
                    .and_then(java_integer_decode)
                    .ok_or(ServerNamesError::BadId(id_text))?;
                match attribute(&element, "name")? {
                    Some(name) => {
                        names.insert(id, name);
                    }
                    None => warn!("Server name entry with id {id} has no name attribute; skipped."),
                }
            }
            Ok(Event::Eof) => return Ok(()),
            Ok(_) => {}
            Err(error) => return Err(ServerNamesError::Xml(error.to_string())),
        }
    }
}

fn attribute(element: &BytesStart<'_>, key: &str) -> Result<Option<String>, ServerNamesError> {
    for attr in element.attributes() {
        let attr = attr.map_err(|error| ServerNamesError::Xml(error.to_string()))?;
        if attr.key.as_ref() == key {
            let value = attr
                .normalized_value(XmlVersion::Implicit1_0)
                .map_err(|error| ServerNamesError::Xml(error.to_string()))?;
            return Ok(Some(value.into_owned()));
        }
    }
    Ok(None)
}

/// Java `Integer.decode`: optional sign, then decimal, `0x`/`0X`/`#` hex or `0` octal.
fn java_integer_decode(text: &str) -> Option<i32> {
    let (negative, rest) = match text.as_bytes().first()? {
        b'-' => (true, &text[1..]),
        b'+' => (false, &text[1..]),
        _ => (false, text),
    };
    let (radix, digits) = if let Some(hex) = rest.strip_prefix("0x").or_else(|| rest.strip_prefix("0X")) {
        (16, hex)
    } else if let Some(hex) = rest.strip_prefix('#') {
        (16, hex)
    } else if rest.len() > 1 && rest.starts_with('0') {
        (8, &rest[1..])
    } else {
        (10, rest)
    };
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_alphanumeric()) {
        return None;
    }
    let value = i64::from_str_radix(digits, radix).ok()?;
    i32::try_from(if negative { -value } else { value }).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<servers_list xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" xsi:noNamespaceSchemaLocation="servername.xsd">
	<!-- a comment -->
	<server id="1" name="Bartz" />
	<server id="2" name="Sieghardt"></server>
	<server id="9" name="Teon (EURO)" />
	<server id="10" name="A &amp; B" />
</servers_list>"#;

    #[test]
    fn parses_the_dist_format() {
        let mut names = ServerNames::new();
        parse_server_names(SAMPLE, &mut names).unwrap();
        assert_eq!(names.len(), 4);
        assert_eq!(names[&1], "Bartz");
        assert_eq!(names[&2], "Sieghardt");
        assert_eq!(names[&9], "Teon (EURO)");
        assert_eq!(names[&10], "A & B");
        assert_eq!(names.keys().copied().collect::<Vec<_>>(), [1, 2, 9, 10]);
    }

    #[test]
    fn later_duplicate_replaces_earlier() {
        let mut names = ServerNames::new();
        parse_server_names(
            r#"<l><server id="1" name="a"/><server id="1" name="b"/></l>"#,
            &mut names,
        )
        .unwrap();
        assert_eq!(names[&1], "b");
    }

    #[test]
    fn bad_id_stops_parsing_but_keeps_earlier_names() {
        let mut names = ServerNames::new();
        let error = parse_server_names(
            r#"<l><server id="1" name="a"/><server id="x" name="b"/><server id="3" name="c"/></l>"#,
            &mut names,
        )
        .unwrap_err();
        assert_eq!(error, ServerNamesError::BadId(Some("x".to_owned())));
        assert_eq!(names.len(), 1);

        let mut names = ServerNames::new();
        assert_eq!(
            parse_server_names(r#"<l><server name="b"/></l>"#, &mut names).unwrap_err(),
            ServerNamesError::BadId(None)
        );
    }

    #[test]
    fn nameless_entry_is_skipped() {
        let mut names = ServerNames::new();
        parse_server_names(r#"<l><server id="1"/><server id="2" name="b"/></l>"#, &mut names).unwrap();
        assert_eq!(names.len(), 1);
    }

    #[test]
    fn broken_xml_is_an_error() {
        let mut names = ServerNames::new();
        assert!(parse_server_names("<l><server id=\"1\" name=\"a\"></l>", &mut names).is_err());
    }

    #[test]
    fn integer_decode_matches_java() {
        assert_eq!(java_integer_decode("12"), Some(12));
        assert_eq!(java_integer_decode("-12"), Some(-12));
        assert_eq!(java_integer_decode("+7"), Some(7));
        assert_eq!(java_integer_decode("0x1F"), Some(31));
        assert_eq!(java_integer_decode("#ff"), Some(255));
        assert_eq!(java_integer_decode("010"), Some(8));
        assert_eq!(java_integer_decode("0"), Some(0));
        assert_eq!(java_integer_decode("-0x80000000"), Some(i32::MIN));
        assert_eq!(java_integer_decode("2147483648"), None);
        assert_eq!(java_integer_decode(""), None);
        assert_eq!(java_integer_decode("-"), None);
        assert_eq!(java_integer_decode("0x"), None);
        assert_eq!(java_integer_decode("1 2"), None);
        assert_eq!(java_integer_decode("--5"), None);
        assert_eq!(java_integer_decode("089"), None);
    }

    #[test]
    fn load_reads_a_file_and_survives_a_missing_one() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(SERVER_NAMES_FILE);
        std::fs::write(&path, SAMPLE).unwrap();
        assert_eq!(load_server_names(&path).len(), 4);
        assert!(load_server_names(&dir.path().join("missing.xml")).is_empty());
    }
}
