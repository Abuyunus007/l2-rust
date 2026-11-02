//! The chat lines of the fake players (`data/FakePlayerChatData.xml`, `Custom/FakePlayers.ini`).
//!
//! Java: `gameserver/managers/FakePlayerChatManager.java` (`parseDocument`) and
//! `gameserver/data/holders/FakePlayerChatHolder.java`. The file is only read with
//! `EnableFakePlayers` and `FakePlayerChat` on (the caller decides).

use std::path::Path;

use serde::Deserialize;

use crate::loader::{DataError, load_file};

/// Java `FakePlayerChatHolder`: what a fake player answers to a whisper.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FakePlayerChat {
    /// `fpcName`: the fake player's name, or `ALL`.
    pub fpc_name: String,
    /// `searchMethod`: `EQUALS`, `STARTS_WITH` or `CONTAINS`.
    pub search_method: String,
    /// `searchText.split(";")`.
    pub search_text: Vec<String>,
    /// `answers.split(";")`.
    pub answers: Vec<String>,
}

#[derive(Deserialize)]
struct RawList {
    #[serde(rename = "fakePlayerChat", default)]
    chats: Vec<RawChat>,
}

#[derive(Deserialize)]
struct RawChat {
    #[serde(rename = "@fpcName")]
    fpc_name: Option<String>,
    #[serde(rename = "@searchMethod")]
    search_method: Option<String>,
    #[serde(rename = "@searchText")]
    search_text: Option<String>,
    #[serde(rename = "@answers")]
    answers: Option<String>,
}

/// Java `String.split(";")`: the trailing empty pieces are dropped (and `""` gives one empty piece).
fn java_split(text: &str) -> Vec<String> {
    let mut parts: Vec<String> = text.split(';').map(str::to_owned).collect();
    if text.is_empty() {
        return parts;
    }
    while parts.last().is_some_and(String::is_empty) {
        parts.pop();
    }
    parts
}

fn convert(doc: RawList) -> Vec<FakePlayerChat> {
    doc.chats
        .into_iter()
        // `set.getString(..)` of a missing attribute throws in Java: the line is not added.
        .filter_map(|c| {
            Some(FakePlayerChat {
                fpc_name: c.fpc_name?,
                search_method: c.search_method?,
                search_text: java_split(&c.search_text?),
                answers: java_split(&c.answers?),
            })
        })
        .collect()
}

/// Loads `<root>/data/FakePlayerChatData.xml`; a missing file gives no lines (Java logs it).
pub fn load_fake_player_chat(root: &Path) -> Vec<FakePlayerChat> {
    let mut lines = Vec::new();
    load_file(&root.join("data/FakePlayerChatData.xml"), |doc: RawList| {
        lines = convert(doc);
        Ok::<(), DataError>(())
    });
    lines
}

/// The lines of the XML text. For tests and tools.
pub fn fake_player_chat_from_xml(xml: &str) -> Result<Vec<FakePlayerChat>, DataError> {
    let doc = quick_xml::de::from_str(xml).map_err(|source| DataError::Xml {
        path: "<inline>".into(),
        source,
    })?;
    Ok(convert(doc))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines_are_split_at_semicolons() {
        let xml = r#"<list>
            <fakePlayerChat fpcName="ALL" searchMethod="EQUALS" searchText="hi" answers="hello;hi;hi there"/>
            <fakePlayerChat fpcName="Evi" searchMethod="CONTAINS" searchText="server;ha;bug" answers="a;b;"/>
            <fakePlayerChat fpcName="Broken" searchMethod="EQUALS"/>
        </list>"#;
        let lines = fake_player_chat_from_xml(xml).unwrap();
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].answers, ["hello", "hi", "hi there"]);
        assert_eq!(lines[1].search_text, ["server", "ha", "bug"]);
        // Java's `split` drops the trailing empty piece.
        assert_eq!(lines[1].answers, ["a", "b"]);
    }

    #[test]
    fn the_shipped_file_loads() {
        let root = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game"));
        let path = root.join("data/FakePlayerChatData.xml");
        let Ok(text) = std::fs::read_to_string(&path) else {
            eprintln!("skipped: {} does not exist", path.display());
            return;
        };
        let lines = fake_player_chat_from_xml(&text).unwrap();
        assert_eq!(lines.len(), text.matches("<fakePlayerChat ").count());
        assert!(
            lines
                .iter()
                .any(|l| l.fpc_name == "ALL" && l.search_method == "EQUALS")
        );
    }
}
