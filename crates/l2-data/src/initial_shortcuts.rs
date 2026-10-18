//! Shortcut bar presets for new characters, and the macro presets they can refer to.
//!
//! Java: `gameserver/data/xml/InitialShortcutData.java`, `entity/actor/holders/player/{Shortcut,
//! Macro,MacroCmd}.java` and the `ShortcutType` / `MacroType` enums.
//! Data: `data/stats/players/initialShortcuts.xml` (`xsd/initialShortcuts.xsd`).
//!
//! Java registers the global shortcuts first, then the class ones, and skips an item, skill or
//! macro the character does not have. That check needs the character, so it belongs to the code
//! that creates the character; this module only holds the presets and their lookups.

use std::collections::BTreeMap;
use std::path::Path;

use serde::Deserialize;

use crate::class_list::ClassId;
use crate::loader::{DataError, load_file, log_loaded};

/// Java `ShortcutType`. `NONE` is in Java's enum, though the XSD does not allow it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum ShortcutType {
    /// Java `NONE`.
    None,
    /// An item, by item id (the character's own object id is looked up at registration).
    Item,
    /// A skill, by skill id.
    Skill,
    /// A client action, by action id.
    Action,
    /// A macro preset, by macro id.
    Macro,
    /// A recipe.
    Recipe,
    /// A bookmark.
    Bookmark,
}

/// Java `MacroType`: what one macro command does.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum MacroType {
    /// Java `NONE`.
    None,
    /// Use a skill (`d1` skill id, `d2` skill level).
    Skill,
    /// Client action (`d1` action id).
    Action,
    /// Text typed into the chat (`MacroCommand::text`).
    Text,
    /// Use a shortcut (`d1` page, `d2` slot).
    Shortcut,
    /// Use an item (`d1` item id).
    Item,
    /// Wait (`d1` seconds).
    Delay,
}

/// One slot of a shortcut bar. Java `Shortcut` as read from the XML (the character type is always
/// 0 for presets, as in Java, so it is not stored).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InitialShortcut {
    /// Slot on the page (`slotId`).
    pub slot: i32,
    /// Page (`pageId`).
    pub page: i32,
    /// What the slot points at.
    pub kind: ShortcutType,
    /// Id of the item, skill, action or macro.
    pub id: i32,
    /// Skill level (0 when not given).
    pub level: i32,
}

/// One command of a macro. Java `MacroCmd`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MacroCommand {
    /// Position in the macro (0-based, counted over every command element).
    pub entry: usize,
    /// What the command does.
    pub kind: MacroType,
    /// First parameter; its meaning depends on `kind` (see [`MacroType`]). 0 when unused.
    pub d1: i32,
    /// Second parameter (skill level, or shortcut slot). 0 when unused.
    pub d2: i32,
    /// The text of the command element (only meaningful for [`MacroType::Text`]).
    pub text: String,
}

/// A macro preset. Java `Macro`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MacroPreset {
    /// Macro id (above 1000 in the shipped data, to stay clear of player macros).
    pub id: i32,
    /// Icon index.
    pub icon: i32,
    /// Display name.
    pub name: String,
    /// Description; `None` when the attribute is missing (Java `null`).
    pub description: Option<String>,
    /// Short text shown on the macro button.
    pub acronym: String,
    /// The commands, in order.
    pub commands: Vec<MacroCommand>,
}

/// All initial shortcut and macro presets. Java `InitialShortcutData`.
#[derive(Debug, Default)]
pub struct InitialShortcuts {
    global: Vec<InitialShortcut>,
    classes: BTreeMap<ClassId, Vec<InitialShortcut>>,
    macros: BTreeMap<i32, MacroPreset>,
}

impl InitialShortcuts {
    const LOADER: &'static str = "InitialShortcutData";

    /// Loads `<root>/data/stats/players/initialShortcuts.xml` and logs Java's three count lines.
    pub fn load(root: &Path) -> Self {
        let mut table = Self::default();
        load_file(
            &root.join("data/stats/players/initialShortcuts.xml"),
            |doc: RawShortcutFile| table.add(doc),
        );
        log_loaded(Self::LOADER, table.global.len(), "initial global shortcut data");
        log_loaded(Self::LOADER, table.classes.len(), "initial shortcut data");
        log_loaded(Self::LOADER, table.macros.len(), "macro presets");
        table
    }

    /// Builds a table from XML text. For tests and tools.
    pub fn from_xml(xml: &str) -> Result<Self, DataError> {
        let doc = quick_xml::de::from_str(xml).map_err(|source| DataError::Xml {
            path: "<inline>".into(),
            source,
        })?;
        let mut table = Self::default();
        table.add(doc)?;
        Ok(table)
    }

    fn add(&mut self, doc: RawShortcutFile) -> Result<(), DataError> {
        for node in doc.shortcuts {
            let mut list = Vec::new();
            for page in node.pages {
                for slot in page.slots {
                    list.push(InitialShortcut {
                        slot: slot.slot_id,
                        page: page.page_id,
                        kind: slot.shortcut_type,
                        id: slot.shortcut_id,
                        level: slot.shortcut_level,
                    });
                }
            }
            match node.class_id {
                // Java replaces the list when a class appears twice.
                Some(id) => {
                    self.classes.insert(player_class(id)?, list);
                }
                // Global shortcuts add up over every node without a class.
                None => self.global.extend(list),
            }
        }
        for node in doc.macros {
            for raw in node.macros {
                // Java skips disabled macros before reading their commands.
                if !raw
                    .enabled
                    .as_deref()
                    .is_none_or(|value| value.eq_ignore_ascii_case("true"))
                {
                    continue;
                }
                let preset = raw.into_preset()?;
                self.macros.insert(preset.id, preset);
            }
        }
        Ok(())
    }

    /// The global shortcuts, in file order (Java `_initialGlobalShortcutList`).
    pub fn global(&self) -> &[InitialShortcut] {
        &self.global
    }

    /// The shortcuts of one class (Java `_initialShortcutData.get(PlayerClass)`). `None` when the
    /// class has no entry, which is not an error.
    pub fn class_shortcuts(&self, class_id: ClassId) -> Option<&[InitialShortcut]> {
        self.classes.get(&class_id).map(Vec::as_slice)
    }

    /// A macro preset by id (Java `_macroPresets.get(id)`).
    pub fn macro_preset(&self, id: i32) -> Option<&MacroPreset> {
        self.macros.get(&id)
    }

    /// Number of classes with a shortcut list.
    pub fn class_count(&self) -> usize {
        self.classes.len()
    }

    /// Number of enabled macro presets.
    pub fn macro_count(&self) -> usize {
        self.macros.len()
    }
}

/// Java `PlayerClass.getPlayerClass(id)`: an id that is not a player class is an error (Java
/// puts `null` into its `EnumMap`, which throws).
fn player_class(id: i32) -> Result<ClassId, DataError> {
    u16::try_from(id)
        .ok()
        .map(ClassId)
        .filter(|class_id| class_id.race().is_some())
        .ok_or_else(|| DataError::Invalid(format!("unknown class id {id}")))
}

/// Java `parseInteger(attributes, name)` without a default: a missing attribute is an error.
fn required(value: Option<i32>, name: &str) -> Result<i32, DataError> {
    value.ok_or_else(|| DataError::Invalid(format!("missing attribute {name}")))
}

// Raw XML shape (xsd/initialShortcuts.xsd).
#[derive(Deserialize)]
struct RawShortcutFile {
    #[serde(rename = "shortcuts", default)]
    shortcuts: Vec<RawShortcuts>,
    #[serde(rename = "macros", default)]
    macros: Vec<RawMacros>,
}

#[derive(Deserialize)]
struct RawShortcuts {
    #[serde(rename = "@classId", default)]
    class_id: Option<i32>,
    #[serde(rename = "page", default)]
    pages: Vec<RawPage>,
}

#[derive(Deserialize)]
struct RawPage {
    #[serde(rename = "@pageId")]
    page_id: i32,
    #[serde(rename = "slot", default)]
    slots: Vec<RawSlot>,
}

#[derive(Deserialize)]
struct RawSlot {
    #[serde(rename = "@slotId")]
    slot_id: i32,
    #[serde(rename = "@shortcutType")]
    shortcut_type: ShortcutType,
    #[serde(rename = "@shortcutId")]
    shortcut_id: i32,
    #[serde(rename = "@shortcutLevel", default)]
    shortcut_level: i32,
}

#[derive(Deserialize)]
struct RawMacros {
    #[serde(rename = "macro", default)]
    macros: Vec<RawMacro>,
}

#[derive(Deserialize)]
struct RawMacro {
    #[serde(rename = "@macroId")]
    macro_id: i32,
    #[serde(rename = "@icon")]
    icon: i32,
    #[serde(rename = "@name")]
    name: String,
    #[serde(rename = "@description", default)]
    description: Option<String>,
    #[serde(rename = "@acronym")]
    acronym: String,
    #[serde(rename = "@enabled", default)]
    enabled: Option<String>,
    #[serde(rename = "command", default)]
    commands: Vec<RawCommand>,
}

impl RawMacro {
    fn into_preset(self) -> Result<MacroPreset, DataError> {
        let commands = self
            .commands
            .into_iter()
            .enumerate()
            .map(|(entry, raw)| raw.into_command(entry))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(MacroPreset {
            id: self.macro_id,
            icon: self.icon,
            name: self.name,
            description: self.description,
            acronym: self.acronym,
            commands,
        })
    }
}

#[derive(Deserialize)]
struct RawCommand {
    #[serde(rename = "@type")]
    kind: MacroType,
    #[serde(rename = "@skillId", default)]
    skill_id: Option<i32>,
    #[serde(rename = "@skillLevel", default)]
    skill_level: Option<i32>,
    #[serde(rename = "@actionId", default)]
    action_id: Option<i32>,
    #[serde(rename = "@page", default)]
    page: Option<i32>,
    #[serde(rename = "@slot", default)]
    slot: Option<i32>,
    #[serde(rename = "@itemId", default)]
    item_id: Option<i32>,
    #[serde(rename = "@delay", default)]
    delay: Option<i32>,
    #[serde(rename = "$text", default)]
    text: String,
}

impl RawCommand {
    /// Java `MacroCmd` parameters: which attributes each type reads, and which are required.
    fn into_command(self, entry: usize) -> Result<MacroCommand, DataError> {
        let (d1, d2) = match self.kind {
            MacroType::Skill => (required(self.skill_id, "skillId")?, self.skill_level.unwrap_or(0)),
            MacroType::Action => (required(self.action_id, "actionId")?, 0),
            MacroType::Text | MacroType::None => (0, 0),
            MacroType::Shortcut => (required(self.page, "page")?, self.slot.unwrap_or(0)),
            MacroType::Item => (required(self.item_id, "itemId")?, 0),
            MacroType::Delay => (required(self.delay, "delay")?, 0),
        };
        Ok(MacroCommand {
            entry,
            kind: self.kind,
            d1,
            d2,
            text: self.text,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<list>
	<shortcuts>
		<page pageId="0">
			<slot slotId="0" shortcutType="ACTION" shortcutId="2" />
			<slot slotId="11" shortcutType="ITEM" shortcutId="5588" />
		</page>
		<page pageId="1">
			<slot slotId="0" shortcutType="MACRO" shortcutId="10001" />
		</page>
	</shortcuts>
	<shortcuts classId="10">
		<page pageId="0">
			<slot slotId="1" shortcutType="SKILL" shortcutId="1177" shortcutLevel="1" />
		</page>
	</shortcuts>
	<macros>
		<macro macroId="10000" icon="1" name="Off" acronym="OFF" enabled="false">
			<command type="TEXT">/loc</command>
		</macro>
		<macro macroId="10001" icon="2" name="On" description="Test" acronym="ON">
			<command type="SKILL" skillId="1177" skillLevel="1" />
			<command type="DELAY" delay="5" />
			<command type="TEXT">!Hello world!</command>
			<command type="SHORTCUT" page="0" slot="11" />
		</macro>
	</macros>
</list>"#;

    #[test]
    fn global_and_class_shortcuts() {
        let table = InitialShortcuts::from_xml(XML).unwrap();
        assert_eq!(table.global().len(), 3);
        assert_eq!(
            table.global()[1],
            InitialShortcut {
                slot: 11,
                page: 0,
                kind: ShortcutType::Item,
                id: 5588,
                level: 0
            }
        );
        let mystic = table.class_shortcuts(ClassId(10)).unwrap();
        assert_eq!(mystic[0].kind, ShortcutType::Skill);
        assert_eq!(mystic[0].level, 1);
        assert!(table.class_shortcuts(ClassId(0)).is_none());
        assert_eq!(table.class_count(), 1);
    }

    #[test]
    fn disabled_macros_are_not_loaded() {
        let table = InitialShortcuts::from_xml(XML).unwrap();
        assert!(table.macro_preset(10000).is_none());
        assert_eq!(table.macro_count(), 1);
    }

    #[test]
    fn macro_commands_keep_their_parameters() {
        let table = InitialShortcuts::from_xml(XML).unwrap();
        let macro_ = table.macro_preset(10001).unwrap();
        assert_eq!(macro_.name, "On");
        assert_eq!(macro_.description.as_deref(), Some("Test"));
        assert_eq!(macro_.icon, 2);
        let cmds = &macro_.commands;
        assert_eq!(cmds.len(), 4);
        assert_eq!(
            (cmds[0].kind, cmds[0].d1, cmds[0].d2),
            (MacroType::Skill, 1177, 1)
        );
        assert_eq!((cmds[1].kind, cmds[1].d1), (MacroType::Delay, 5));
        assert_eq!(cmds[2].text, "!Hello world!");
        assert_eq!(
            (cmds[3].kind, cmds[3].d1, cmds[3].d2),
            (MacroType::Shortcut, 0, 11)
        );
        assert_eq!(cmds[3].entry, 3);
    }

    #[test]
    fn missing_description_is_none() {
        let xml = XML.replace(r#" description="Test""#, "");
        let table = InitialShortcuts::from_xml(&xml).unwrap();
        assert!(table.macro_preset(10001).unwrap().description.is_none());
    }

    #[test]
    fn command_without_its_required_attribute_is_an_error() {
        let xml = XML.replace(r#"skillId="1177" "#, "");
        assert!(matches!(
            InitialShortcuts::from_xml(&xml),
            Err(DataError::Invalid(_))
        ));
    }

    #[test]
    fn unknown_shortcut_type_is_an_error() {
        let xml = XML.replace(r#"shortcutType="ACTION""#, r#"shortcutType="NOPE""#);
        assert!(matches!(
            InitialShortcuts::from_xml(&xml),
            Err(DataError::Xml { .. })
        ));
    }
}
