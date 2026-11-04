//! The translated texts of `data/lang/<lang>/` (`Custom/MultilingualSupport.ini`, M9-L02).
//!
//! `gameserver/data/xml/SendMessageLocalisationData.java`, `SystemMessageId.loadLocalisations`
//! and `NpcStringId.loadLocalisations`. Each language has up to four files:
//! `NpcNameLocalisation.xml`, `NpcStringLocalisation.xml`, `SystemMessageLocalisation.xml` and
//! `SendMessageLocalisation.xml`. They are read only with `MultiLangEnable = True`, for the
//! languages of `MultiLangAllowed` (the caller decides, see [`load_lang_data`]); a language
//! without a file simply has none of that kind.
//!
//! This crate keeps the texts as the files give them. Turning the `$s1`/`$c1` markers of a
//! translation into a template, and checking the ids against the client tables, is the
//! world's job (`l2_world::custom::multilingual`).

use std::collections::HashMap;
use std::path::Path;

use serde::Deserialize;
use tracing::info;

use crate::loader::{DataError, load_file};

/// Java `SendMessageLocalisationData.SPLIT_STRING`: a message and its translation are cut into
/// parts at this marker, the parts are replaced one by one.
const SPLIT_STRING: &str = "XXX";

/// The longest translation of a system message or NPC string (Java: `text.length() > 255`).
const MAX_TEXT_LENGTH: usize = 255;

/// The four files of one language.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LangTables {
    /// `NpcNameLocalisation.xml`: NPC template id -> (name, title).
    pub npc_names: HashMap<i32, (String, String)>,
    /// `NpcStringLocalisation.xml`: (`NpcStringId` id, translation) in file order.
    pub npc_strings: Vec<(i32, String)>,
    /// `SystemMessageLocalisation.xml`: (`SystemMessageId` id, translation) in file order.
    pub system_messages: Vec<(i32, String)>,
    /// `SendMessageLocalisation.xml`: (message parts, translation parts) in file order.
    pub send_messages: Vec<(Vec<String>, Vec<String>)>,
}

impl LangTables {
    /// True when no file of the language gave an entry.
    pub fn is_empty(&self) -> bool {
        self.npc_names.is_empty()
            && self.npc_strings.is_empty()
            && self.system_messages.is_empty()
            && self.send_messages.is_empty()
    }
}

/// The tables of every loaded language, in the order of `MultiLangAllowed`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LangData {
    pub languages: Vec<(String, LangTables)>,
}

impl LangData {
    /// The tables of `lang`.
    pub fn get(&self, lang: &str) -> Option<&LangTables> {
        self.languages.iter().find(|(l, _)| l == lang).map(|(_, t)| t)
    }
}

#[derive(Deserialize)]
struct RawList {
    #[serde(rename = "localisation", default)]
    items: Vec<RawLocalisation>,
}

/// One `<localisation .../>`; the four files use different attributes.
#[derive(Deserialize)]
struct RawLocalisation {
    #[serde(rename = "@id")]
    id: Option<String>,
    #[serde(rename = "@name")]
    name: Option<String>,
    #[serde(rename = "@title")]
    title: Option<String>,
    #[serde(rename = "@message")]
    message: Option<String>,
    #[serde(rename = "@translation")]
    translation: Option<String>,
}

/// Java `Integer.parseInt`.
fn parse_int(text: &str) -> Option<i32> {
    text.parse().ok()
}

/// Java `String.split("XXX")`: no marker gives the text itself (also when it is empty), else the
/// trailing empty parts are dropped.
fn split_marker(text: &str) -> Vec<String> {
    if !text.contains(SPLIT_STRING) {
        return vec![text.to_owned()];
    }
    let mut parts: Vec<String> = text.split(SPLIT_STRING).map(str::to_owned).collect();
    while parts.last().is_some_and(String::is_empty) {
        parts.pop();
    }
    parts
}

/// `SystemMessageLocalisation.xml` and `NpcStringLocalisation.xml`: the same reader in Java.
fn id_texts(doc: RawList) -> Vec<(i32, String)> {
    let mut out = Vec::new();
    for item in doc.items {
        // No `id` attribute: the element is skipped without a word.
        let Some(id) = item.id else { continue };
        let Some(id) = parse_int(&id) else { break };
        let Some(text) = item.translation else { continue };
        if text.is_empty() || text.encode_utf16().count() > MAX_TEXT_LENGTH {
            continue;
        }
        out.push((id, text));
    }
    out
}

/// `NpcNameLocalisation.xml`: `set.getInt("id")`, `getString("name")` and `getString("title")`
/// throw when the attribute is missing; the entries before stay, the rest of the file is lost.
fn npc_names(doc: RawList) -> HashMap<i32, (String, String)> {
    let mut out = HashMap::new();
    for item in doc.items {
        let (Some(id), Some(name), Some(title)) = (item.id, item.name, item.title) else {
            break;
        };
        let Some(id) = parse_int(&id) else { break };
        out.insert(id, (name, title));
    }
    out
}

/// `SendMessageLocalisation.xml`.
fn send_messages(doc: RawList) -> Vec<(Vec<String>, Vec<String>)> {
    let mut out = Vec::new();
    for item in doc.items {
        let (Some(message), Some(translation)) = (item.message, item.translation) else {
            break;
        };
        out.push((split_marker(&message), split_marker(&translation)));
    }
    out
}

fn read<T>(path: &Path, convert: fn(RawList) -> T) -> Option<T> {
    if !path.is_file() {
        return None;
    }
    let mut out = None;
    load_file(path, |doc: RawList| {
        out = Some(convert(doc));
        Ok::<(), DataError>(())
    });
    out
}

/// Loads one language folder: `<dir>/{NpcName,NpcString,SystemMessage,SendMessage}Localisation.xml`.
pub fn load_lang_dir(dir: &Path) -> LangTables {
    LangTables {
        npc_names: read(&dir.join("NpcNameLocalisation.xml"), npc_names).unwrap_or_default(),
        npc_strings: read(&dir.join("NpcStringLocalisation.xml"), id_texts).unwrap_or_default(),
        system_messages: read(&dir.join("SystemMessageLocalisation.xml"), id_texts).unwrap_or_default(),
        send_messages: read(&dir.join("SendMessageLocalisation.xml"), send_messages).unwrap_or_default(),
    }
}

/// Loads `<root>/data/lang/<lang>/` for every language of `allowed`. The caller passes an empty
/// list when `MultiLangEnable` is off (Java then loads nothing).
pub fn load_lang_data(root: &Path, allowed: &[String]) -> LangData {
    let mut data = LangData::default();
    for lang in allowed {
        let tables = load_lang_dir(&root.join("data/lang").join(lang));
        if tables.is_empty() {
            continue;
        }
        info!("MultilingualSupport: Loaded localisations for [{lang}].");
        data.languages.push((lang.clone(), tables));
    }
    data
}

/// The tables of the XML texts, one per file. For tests and tools.
pub fn lang_tables_from_xml(
    npc_name: &str,
    npc_string: &str,
    system_message: &str,
    send_message: &str,
) -> Result<LangTables, DataError> {
    fn parse(xml: &str) -> Result<RawList, DataError> {
        if xml.trim().is_empty() {
            return Ok(RawList { items: Vec::new() });
        }
        quick_xml::de::from_str(xml).map_err(|source| DataError::Xml {
            path: "<inline>".into(),
            source,
        })
    }
    Ok(LangTables {
        npc_names: npc_names(parse(npc_name)?),
        npc_strings: id_texts(parse(npc_string)?),
        system_messages: id_texts(parse(system_message)?),
        send_messages: send_messages(parse(send_message)?),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_four_files_are_read() {
        let t = lang_tables_from_xml(
            r#"<list><localisation id="30080" name="Klarissa" title="Teleporter"/></list>"#,
            r#"<list><!-- Q00001 --><localisation id="101" translation="Love Letters"/></list>"#,
            r#"<list><localisation id="34" translation="Welcome."/></list>"#,
            r#"<list><localisation message="You have learned XXX new skills." translation="Learned XXX skills."/></list>"#,
        )
        .unwrap();
        assert_eq!(t.npc_names[&30080], ("Klarissa".into(), "Teleporter".into()));
        assert_eq!(t.npc_strings, [(101, "Love Letters".to_owned())]);
        assert_eq!(t.system_messages, [(34, "Welcome.".to_owned())]);
        assert_eq!(t.send_messages[0].0, ["You have learned ", " new skills."]);
        assert_eq!(t.send_messages[0].1, ["Learned ", " skills."]);
    }

    #[test]
    fn bad_entries_follow_java() {
        let long = "x".repeat(256);
        let xml = format!(
            r#"<list>
                <localisation translation="no id"/>
                <localisation id="1" translation=""/>
                <localisation id="2" translation="{long}"/>
                <localisation id="3"/>
                <localisation id="4" translation="ok"/>
                <localisation id="x5" translation="parse error ends the file"/>
                <localisation id="6" translation="never read"/>
            </list>"#
        );
        let t = lang_tables_from_xml("", &xml, "", "").unwrap();
        assert_eq!(t.npc_strings, [(4, "ok".to_owned())]);
    }

    #[test]
    fn a_missing_attribute_of_the_other_files_ends_the_file() {
        let t = lang_tables_from_xml(
            r#"<list><localisation id="1" name="a" title=""/><localisation id="2" name="b"/><localisation id="3" name="c" title="t"/></list>"#,
            "",
            "",
            r#"<list><localisation message="a" translation="b"/><localisation message="c"/><localisation message="d" translation="e"/></list>"#,
        )
        .unwrap();
        assert_eq!(t.npc_names.len(), 1);
        assert_eq!(t.send_messages.len(), 1);
    }

    #[test]
    fn split_follows_java() {
        assert_eq!(split_marker(""), [""]);
        assert_eq!(split_marker("plain"), ["plain"]);
        assert_eq!(split_marker("XXX"), Vec::<String>::new());
        assert_eq!(split_marker("aXXXbXXX"), ["a", "b"]);
        assert_eq!(split_marker("XXXa"), ["", "a"]);
    }

    #[test]
    fn the_shipped_greek_files_load() {
        let root = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game"));
        if !root.join("data/lang/el").is_dir() {
            eprintln!("skipped: {} has no data/lang/el", root.display());
            return;
        }
        let data = load_lang_data(root, &["en".to_owned(), "el".to_owned()]);
        // `en` has no folder.
        assert!(data.get("en").is_none());
        let el = data.get("el").unwrap();
        assert!(el.npc_names.contains_key(&30080));
        assert!(el.system_messages.iter().any(|(id, _)| *id == 34));
        assert!(!el.send_messages.is_empty());
    }
}
