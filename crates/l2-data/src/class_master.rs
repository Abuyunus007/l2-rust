//! The class master configuration: which class changes the Mr. Cat and Miss Queen NPCs offer and
//! for what price.
//!
//! Java: `dist/game/data/scripts/ai/others/ClassMaster/ClassMaster.java` (`load`, `parseDocument`;
//! the script reads the file itself). Data: `config/ClassMaster.xml` (not under `data/`).
//!
//! The script that uses this is `l2_scripts::ai::others::class_master`.

use std::path::Path;

use serde::Deserialize;
use tracing::{error, warn};

use crate::loader::{DataError, load_file, log_loaded};
use crate::stats::CategoryType;
use crate::stats::jnum::parse_boolean;

/// Java `ItemHolder`: an item id and a count.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ItemHolder {
    pub id: i32,
    pub count: i64,
}

/// Java `ClassMaster.ClassChangeData`: one `classChangeOption`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClassChangeData {
    /// `name`.
    pub name: String,
    /// `appliesTo/category`.
    pub applied_categories: Vec<CategoryType>,
    /// `rewards/setNoble`.
    pub reward_noblesse: bool,
    /// `rewards/setHero`.
    pub reward_hero: bool,
    /// `conditions/item`.
    pub items_required: Vec<ItemHolder>,
    /// `rewards/item`.
    pub items_rewarded: Vec<ItemHolder>,
}

/// The `classMaster` element of `config/ClassMaster.xml`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ClassMasterConfig {
    /// `classChangeEnabled` (default `false`).
    pub is_enabled: bool,
    /// `spawnClassMasters` (default `true` when enabled; `false` for a disabled file, which Java
    /// never reads that far).
    pub spawn_class_masters: bool,
    /// `showPopupWindow` (default `false`).
    pub show_popup_window: bool,
    /// The `classChangeOption`s that have a category, in file order.
    pub class_change_data: Vec<ClassChangeData>,
}

impl ClassMasterConfig {
    const LOADER: &'static str = "ClassMaster";

    /// Loads `<root>/config/ClassMaster.xml` (`root` is the server folder, the one that holds
    /// `data/` and `config/`) and logs `Loaded N class change options.` when the class master is
    /// enabled. A missing or broken file leaves the default (disabled).
    pub fn load(root: &Path) -> Self {
        let mut config = Self::default();
        load_file(&root.join("config/ClassMaster.xml"), |doc: RawList| {
            config.add(doc);
            Ok(())
        });
        if config.is_enabled {
            log_loaded(
                Self::LOADER,
                config.class_change_data.len(),
                "class change options",
            );
        }
        config
    }

    /// Builds the configuration from XML text. For tests and tools.
    pub fn from_xml(xml: &str) -> Result<Self, DataError> {
        let doc = quick_xml::de::from_str(xml).map_err(|source| DataError::Xml {
            path: "<inline>".into(),
            source,
        })?;
        let mut config = Self::default();
        config.add(doc);
        Ok(config)
    }

    /// Java `parseDocument`.
    fn add(&mut self, doc: RawList) {
        for node in doc.children {
            let RawListChild::ClassMaster(cm) = node else {
                continue;
            };
            self.is_enabled = cm.enabled.as_deref().is_some_and(parse_boolean);
            if !self.is_enabled {
                // `return`: the rest of the file, other `classMaster` elements included, is not read.
                return;
            }
            self.spawn_class_masters = cm.spawn.as_deref().is_none_or(parse_boolean);
            self.show_popup_window = cm.popup.as_deref().is_some_and(parse_boolean);
            for child in cm.children {
                if let RawClassMasterChild::ClassChangeOption(option) = child
                    && let Some(data) = build_option(option)
                {
                    self.class_change_data.push(data);
                }
            }
        }
    }
}

/// One `classChangeOption`; `None` for an option without a valid category (Java skips it).
fn build_option(option: RawOption) -> Option<ClassChangeData> {
    let mut applied_categories = Vec::new();
    let mut items_required = Vec::new();
    let mut items_rewarded = Vec::new();
    let mut reward_noblesse = false;
    let mut reward_hero = false;
    let name = option.name.unwrap_or_default();
    for part in option.children {
        match part {
            RawOptionChild::AppliesTo(applies) => {
                for entry in applies.children {
                    if let RawAppliesChild::Category(category) = entry {
                        match CategoryType::from_name(category.text.trim()) {
                            Some(category) => applied_categories.push(category),
                            None => error!("ClassMaster: Incorrect category type: {}", category.text.trim()),
                        }
                    }
                }
            }
            RawOptionChild::Rewards(rewards) => {
                for entry in rewards.children {
                    match entry {
                        RawRewardChild::Item(item) => match item.holder() {
                            Some(holder) => items_rewarded.push(holder),
                            None => warn!("ClassMaster: an item of option {name} has no valid id."),
                        },
                        RawRewardChild::SetNoble => reward_noblesse = true,
                        RawRewardChild::SetHero => reward_hero = true,
                        RawRewardChild::Other => {}
                    }
                }
            }
            RawOptionChild::Conditions(conditions) => {
                for entry in conditions.children {
                    if let RawConditionChild::Item(item) = entry {
                        match item.holder() {
                            Some(holder) => items_required.push(holder),
                            None => warn!("ClassMaster: an item of option {name} has no valid id."),
                        }
                    }
                }
            }
            RawOptionChild::Other => {}
        }
    }
    if applied_categories.is_empty() {
        warn!("ClassMaster: Class change option: {name} has no categories to be applied on. Skipping!");
        return None;
    }
    Some(ClassChangeData {
        name,
        applied_categories,
        reward_noblesse,
        reward_hero,
        items_required,
        items_rewarded,
    })
}

// Raw XML shape (xsd/classMaster.xsd). Java reads the children in document order, so every level
// is a `$value` list of an element enum.
#[derive(Deserialize)]
struct RawList {
    #[serde(rename = "$value", default)]
    children: Vec<RawListChild>,
}

#[derive(Deserialize)]
enum RawListChild {
    #[serde(rename = "classMaster")]
    ClassMaster(RawClassMaster),
    #[serde(other)]
    Other,
}

#[derive(Deserialize)]
struct RawClassMaster {
    #[serde(rename = "@classChangeEnabled", default)]
    enabled: Option<String>,
    #[serde(rename = "@spawnClassMasters", default)]
    spawn: Option<String>,
    #[serde(rename = "@showPopupWindow", default)]
    popup: Option<String>,
    #[serde(rename = "$value", default)]
    children: Vec<RawClassMasterChild>,
}

#[derive(Deserialize)]
enum RawClassMasterChild {
    #[serde(rename = "classChangeOption")]
    ClassChangeOption(RawOption),
    #[serde(other)]
    Other,
}

#[derive(Deserialize)]
struct RawOption {
    #[serde(rename = "@name", default)]
    name: Option<String>,
    #[serde(rename = "$value", default)]
    children: Vec<RawOptionChild>,
}

#[derive(Deserialize)]
enum RawOptionChild {
    #[serde(rename = "appliesTo")]
    AppliesTo(RawApplies),
    #[serde(rename = "rewards")]
    Rewards(RawRewards),
    #[serde(rename = "conditions")]
    Conditions(RawConditions),
    #[serde(other)]
    Other,
}

#[derive(Deserialize)]
struct RawApplies {
    #[serde(rename = "$value", default)]
    children: Vec<RawAppliesChild>,
}

#[derive(Deserialize)]
enum RawAppliesChild {
    #[serde(rename = "category")]
    Category(RawCategory),
    #[serde(other)]
    Other,
}

#[derive(Deserialize)]
struct RawCategory {
    #[serde(rename = "$text", default)]
    text: String,
}

#[derive(Deserialize)]
struct RawRewards {
    #[serde(rename = "$value", default)]
    children: Vec<RawRewardChild>,
}

#[derive(Deserialize)]
enum RawRewardChild {
    #[serde(rename = "item")]
    Item(RawItem),
    #[serde(rename = "setNoble")]
    SetNoble,
    #[serde(rename = "setHero")]
    SetHero,
    #[serde(other)]
    Other,
}

#[derive(Deserialize)]
struct RawConditions {
    #[serde(rename = "$value", default)]
    children: Vec<RawConditionChild>,
}

#[derive(Deserialize)]
enum RawConditionChild {
    #[serde(rename = "item")]
    Item(RawItem),
    #[serde(other)]
    Other,
}

#[derive(Deserialize)]
struct RawItem {
    #[serde(rename = "@id", default)]
    id: Option<String>,
    #[serde(rename = "@count", default)]
    count: Option<String>,
}

impl RawItem {
    /// `parseInteger(attrs, "id")` and `parseInteger(attrs, "count", 1)`. NOTE(java-quirk): Java
    /// throws for a missing id and aborts the file; here the item is skipped.
    fn holder(&self) -> Option<ItemHolder> {
        let id = self.id.as_deref()?.trim().parse().ok()?;
        let count: i32 = match self.count.as_deref() {
            Some(text) => text.trim().parse().ok()?,
            None => 1,
        };
        Some(ItemHolder {
            id,
            count: i64::from(count),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const XML: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<list xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" xsi:noNamespaceSchemaLocation="../data/xsd/classMaster.xsd">
	<classMaster classChangeEnabled="true" spawnClassMasters="false" showPopupWindow="true">
		<classChangeOption name="Free">
			<appliesTo>
				<category>FIRST_CLASS_GROUP</category>
			</appliesTo>
		</classChangeOption>
		<classChangeOption name="Pay">
			<appliesTo>
				<category>SECOND_CLASS_GROUP</category>
				<category>NOT_A_CATEGORY</category>
			</appliesTo>
			<rewards>
				<item id="6622" count="2" />
				<setNoble />
				<item id="6623" />
				<setHero />
			</rewards>
			<conditions>
				<item id="57" count="1000000" />
			</conditions>
		</classChangeOption>
		<classChangeOption name="Nothing">
			<appliesTo>
				<category>NOT_A_CATEGORY</category>
			</appliesTo>
		</classChangeOption>
	</classMaster>
</list>"#;

    #[test]
    fn parses_the_options_in_document_order() {
        let config = ClassMasterConfig::from_xml(XML).unwrap();
        assert!(config.is_enabled);
        assert!(!config.spawn_class_masters);
        assert!(config.show_popup_window);
        // "Nothing" has no valid category and is skipped.
        assert_eq!(config.class_change_data.len(), 2);
        let free = &config.class_change_data[0];
        assert_eq!(free.name, "Free");
        assert_eq!(free.applied_categories, [CategoryType::FirstClassGroup]);
        assert!(free.items_required.is_empty() && free.items_rewarded.is_empty());
        let pay = &config.class_change_data[1];
        assert_eq!(pay.applied_categories, [CategoryType::SecondClassGroup]);
        assert!(pay.reward_noblesse && pay.reward_hero);
        assert_eq!(
            pay.items_rewarded,
            [
                ItemHolder { id: 6622, count: 2 },
                ItemHolder { id: 6623, count: 1 }
            ]
        );
        assert_eq!(
            pay.items_required,
            [ItemHolder {
                id: 57,
                count: 1_000_000
            }]
        );
    }

    #[test]
    fn spawn_defaults_to_true_and_popup_to_false() {
        let config = ClassMasterConfig::from_xml(
            r#"<list><classMaster classChangeEnabled="true"></classMaster></list>"#,
        )
        .unwrap();
        assert!(config.is_enabled && config.spawn_class_masters && !config.show_popup_window);
    }

    #[test]
    fn a_disabled_file_reads_nothing_else() {
        let config = ClassMasterConfig::from_xml(
            r#"<list><classMaster classChangeEnabled="false" spawnClassMasters="true">
            <classChangeOption name="x"><appliesTo><category>FIRST_CLASS_GROUP</category></appliesTo>
            </classChangeOption></classMaster></list>"#,
        )
        .unwrap();
        assert_eq!(config, ClassMasterConfig::default());
    }

    #[test]
    fn the_shipped_file_is_disabled_with_its_two_options() {
        let root = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game"));
        let path = root.join("config/ClassMaster.xml");
        let Ok(text) = std::fs::read_to_string(&path) else {
            eprintln!("skipped: {} does not exist", path.display());
            return;
        };
        // The shipped file has `classChangeEnabled="false"`: Java reads no option.
        let config = ClassMasterConfig::from_xml(&text).unwrap();
        assert_eq!(config, ClassMasterConfig::default());
        // The same options, enabled.
        let enabled = text.replace(
            r#"classChangeEnabled="false" spawnClassMasters="false""#,
            r#"classChangeEnabled="true" spawnClassMasters="false""#,
        );
        let config = ClassMasterConfig::from_xml(&enabled).unwrap();
        assert!(config.is_enabled);
        assert_eq!(config.class_change_data.len(), 2);
        assert_eq!(config.class_change_data[0].name, "Free");
        assert_eq!(
            config.class_change_data[1].items_required,
            [ItemHolder {
                id: 57,
                count: 1_000_000
            }]
        );
    }

    #[test]
    fn a_missing_file_is_the_default() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(ClassMasterConfig::load(dir.path()), ClassMasterConfig::default());
    }
}
