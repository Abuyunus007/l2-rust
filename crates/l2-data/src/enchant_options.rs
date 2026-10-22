//! Enchant options: the option ids (see [`crate::AugmentationOption`]) an enchant level of an item
//! gives its wearer.
//!
//! Java: `gameserver/data/xml/EnchantItemOptionsData.java`, `mechanics/options/EnchantOptions.java`.
//! Data: `data/EnchantItemOptions.xml` (`xsd/enchantItemOptions.xsd`). The options themselves are
//! the ones of `data/stats/augmentation/options` (Java `OptionData`).
//!
//! What an option does to a player (`Options.apply`) is the world's: `l2_world::enchant`.

use std::collections::BTreeMap;
use std::path::Path;

use tracing::{info, warn};

use crate::augmentation::AugmentationOptions;
use crate::item::ItemTemplates;
use crate::stats::jnum::decode_i32;
use crate::stats::node::parse_dom;

/// How many options one enchant level has.
pub const OPTIONS_PER_LEVEL: usize = 3;

/// Java `EnchantItemOptionsData`: item id to enchant level to its three option ids (0 = none).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EnchantItemOptions {
    data: BTreeMap<i32, BTreeMap<i32, [i32; OPTIONS_PER_LEVEL]>>,
}

impl EnchantItemOptions {
    const LOADER: &'static str = "EnchantItemOptionsData";

    /// Loads `<root>/data/EnchantItemOptions.xml` and logs Java's count line. Every item must have a
    /// template in `items` and every option id an option in `options`.
    pub fn load(root: &Path, items: &ItemTemplates, options: &AugmentationOptions) -> Self {
        let path = root.join("data/EnchantItemOptions.xml");
        let mut table = Self::default();
        let counter = match std::fs::read(&path) {
            Ok(bytes) => table.add_xml(&String::from_utf8_lossy(&bytes), items, options),
            Err(error) => {
                warn!("Cannot parse EnchantItemOptions.xml: {error}");
                0
            }
        };
        info!(
            "{}: Loaded {} items and {} options.",
            Self::LOADER,
            table.data.len(),
            counter
        );
        table
    }

    /// Builds the table from XML text. For tests and tools.
    pub fn from_xml(xml: &str, items: &ItemTemplates, options: &AugmentationOptions) -> Self {
        let mut table = Self::default();
        table.add_xml(xml, items, options);
        table
    }

    /// Java `parseDocument`; returns the `counter` (every `<options>` element read).
    fn add_xml(&mut self, xml: &str, items: &ItemTemplates, options: &AugmentationOptions) -> usize {
        let root = match parse_dom(xml) {
            Ok(root) => root,
            Err(error) => {
                warn!("Error parsing EnchantItemOptions.xml: {error}");
                return 0;
            }
        };
        if !root.name.eq_ignore_ascii_case("list") {
            return 0;
        }
        let mut counter = 0;
        // `ITEM:` of the Java loop.
        'item: for item in root
            .children
            .iter()
            .filter(|i| i.name.eq_ignore_ascii_case("item"))
        {
            // `parseInteger(attrs, "id")` throws for a bad id and the rest of the file is lost.
            let Some(item_id) = item.attr("id").and_then(decode_i32) else {
                warn!("Error parsing EnchantItemOptions.xml: bad item id");
                return counter;
            };
            if items.get(item_id).is_none() {
                warn!("{}: Could not find item template for id {item_id}", Self::LOADER);
                continue 'item;
            }
            for node in item
                .children
                .iter()
                .filter(|o| o.name.eq_ignore_ascii_case("options"))
            {
                let Some(level) = node.attr("level").and_then(decode_i32) else {
                    warn!("Error parsing EnchantItemOptions.xml: bad options level");
                    return counter;
                };
                // Java builds the `EnchantOptions` and registers it with the first option that
                // exists; `putIfAbsent` keeps the first `<options>` of a level, a later one
                // fills an object nobody holds.
                let mut ids = [0; OPTIONS_PER_LEVEL];
                let mut registered = false;
                for (index, slot) in ids.iter_mut().enumerate() {
                    let Some(text) = node.attr(&format!("option{}", index + 1)) else {
                        continue;
                    };
                    // `StringUtil.isNumeric`.
                    if text.is_empty() || !text.chars().all(|c| c.is_ascii_digit()) {
                        continue;
                    }
                    let Some(id) = decode_i32(text) else {
                        warn!("Error parsing EnchantItemOptions.xml: bad option id");
                        return counter;
                    };
                    if options.get(id).is_none() {
                        warn!(
                            "{}: Could not find option {id} for item {}",
                            Self::LOADER,
                            item_label(items, item_id)
                        );
                        // `continue ITEM`: what was registered before stays, the counter is
                        // not raised for this element.
                        if registered {
                            self.store(item_id, level, ids);
                        }
                        continue 'item;
                    }
                    *slot = id;
                    registered = true;
                }
                if registered {
                    self.store(item_id, level, ids);
                }
                counter += 1;
            }
        }
        counter
    }

    /// `data.putIfAbsent(level, op)`.
    fn store(&mut self, item_id: i32, level: i32, ids: [i32; OPTIONS_PER_LEVEL]) {
        self.data.entry(item_id).or_default().entry(level).or_insert(ids);
    }

    /// Java `getOptions(itemId, enchantLevel)`: the three option ids (0 is none), `None` for an item
    /// or a level without options.
    pub fn get(&self, item_id: i32, enchant_level: i32) -> Option<[i32; OPTIONS_PER_LEVEL]> {
        self.data.get(&item_id)?.get(&enchant_level).copied()
    }

    /// Every item with options and its levels, by item id.
    pub fn iter(&self) -> impl Iterator<Item = (i32, &BTreeMap<i32, [i32; OPTIONS_PER_LEVEL]>)> + '_ {
        self.data.iter().map(|(&id, levels)| (id, levels))
    }

    /// Number of items with options.
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// True when nothing was loaded.
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }
}

/// `ItemTemplate.toString()` for the warning: `name(id)`.
fn item_label(items: &ItemTemplates, id: i32) -> String {
    items
        .get(id)
        .map_or_else(|| id.to_string(), |template| template.label())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn items() -> ItemTemplates {
        ItemTemplates::from_xml(
            r#"<list>
	<item id="100" type="Weapon" name="Sword"><set name="weapon_type" val="SWORD"/><set name="bodypart" val="rhand"/></item>
	<item id="101" type="Armor" name="Shirt"><set name="bodypart" val="chest"/></item>
</list>"#,
        )
        .unwrap()
    }

    fn options() -> AugmentationOptions {
        AugmentationOptions::from_xml(
            r#"<list><option id="1" name="a"/><option id="2" name="b"/><option id="3" name="c"/></list>"#,
            &crate::skill::SkillTemplates::default(),
        )
        .unwrap()
    }

    #[test]
    fn levels_hold_their_options() {
        let xml = r#"<list>
	<item id="100">
		<options level="0" option1="1" />
		<options level="4" option1="1" option2="2" option3="3" />
		<options level="5" option2="3" />
	</item>
	<item id="101"><options level="1" option1="2"/></item>
</list>"#;
        let table = EnchantItemOptions::from_xml(xml, &items(), &options());
        assert_eq!(table.len(), 2);
        assert_eq!(table.get(100, 0), Some([1, 0, 0]));
        assert_eq!(table.get(100, 4), Some([1, 2, 3]));
        assert_eq!(table.get(100, 5), Some([0, 3, 0]));
        assert_eq!(table.get(100, 6), None);
        assert_eq!(table.get(101, 1), Some([2, 0, 0]));
        assert_eq!(table.get(7, 1), None);
    }

    #[test]
    fn templates_get_their_options() {
        let xml = r#"<list><item id="100"><options level="4" option1="1" option3="3"/></item></list>"#;
        let table = EnchantItemOptions::from_xml(xml, &items(), &options());
        let mut items = items();
        items.attach_enchant_options(&table);
        let sword = items.get(100).unwrap();
        assert_eq!(sword.enchant_options_at(4), [1, 0, 3]);
        // `DEFAULT_ENCHANT_OPTIONS` for any other level and item.
        assert_eq!(sword.enchant_options_at(5), [0, 0, 0]);
        assert_eq!(items.get(101).unwrap().enchant_options_at(4), [0, 0, 0]);
    }

    #[test]
    fn unknown_items_are_skipped_and_the_first_level_entry_wins() {
        let xml = r#"<list>
	<item id="999"><options level="1" option1="1"/></item>
	<item id="100">
		<options level="1" option1="1"/>
		<options level="1" option1="2"/>
		<options level="2"/>
	</item>
</list>"#;
        let table = EnchantItemOptions::from_xml(xml, &items(), &options());
        assert_eq!(table.len(), 1);
        assert_eq!(table.get(100, 1), Some([1, 0, 0]));
        // A level without any option is not stored.
        assert_eq!(table.get(100, 2), None);
    }

    #[test]
    fn an_unknown_option_ends_the_item_but_keeps_what_was_stored() {
        let xml = r#"<list>
	<item id="100">
		<options level="1" option1="1" option2="77" option3="3"/>
		<options level="2" option1="2"/>
	</item>
	<item id="101"><options level="1" option1="77"/><options level="2" option1="2"/></item>
</list>"#;
        let table = EnchantItemOptions::from_xml(xml, &items(), &options());
        // Item 100: level 1 was registered with option1 when option2 failed; level 2 is never read.
        assert_eq!(table.get(100, 1), Some([1, 0, 0]));
        assert_eq!(table.get(100, 2), None);
        // Item 101: the first option of the first element fails before anything is registered.
        assert_eq!(table.get(101, 1), None);
        assert_eq!(table.get(101, 2), None);
        assert_eq!(table.len(), 1);
    }
}
