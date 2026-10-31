//! Enchant scrolls, support items, the success chances per enchant level, and the element stones.
//!
//! Java: `gameserver/data/xml/{EnchantItemData,EnchantItemGroupsData,ElementalAttributeData}.java`,
//! `gameserver/entity/item/enchant/*.java`, `gameserver/data/holders/{ElementalItemHolder,
//! RangeChanceHolder}.java`, `gameserver/entity/item/enums/ElementalItemType.java` and the
//! constants of `gameserver/entity/item/holders/Elementals.java`.
//! Data: `data/EnchantItemData.xml`, `data/EnchantItemGroups.xml`, `data/ElementalAttributeData.xml`.
//!
//! The loaders only read and validate. What an enchant does to a player is `l2_world::enchant`.
//! Java's `Item` is passed in as an [`EnchantTarget`] (the template, the enchant level, and
//! whether `Item.isEnchantable()` holds, which needs the item's location and the config's
//! blacklist).

use std::collections::{HashMap, HashSet};
use std::path::Path;

use serde::Deserialize;
use tracing::{info, warn};

use crate::item::types::{BodyPart, CrystalType, EtcItemType};
use crate::item::{ItemTemplate, ItemTemplates, TYPE2_ACCESSORY, TYPE2_SHIELD_ARMOR, TYPE2_WEAPON};
use crate::loader::{DataError, load_file, log_loaded};
use crate::stats::jnum::{decode_i32, java_split, parse_boolean, parse_double};

/// Java `Elementals`: the element ids and the attribute value tables.
pub mod elementals {
    /// `Elementals.NONE`.
    pub const NONE: i32 = -1;
    pub const FIRE: i32 = 0;
    pub const WATER: i32 = 1;
    pub const WIND: i32 = 2;
    pub const EARTH: i32 = 3;
    pub const HOLY: i32 = 4;
    pub const DARK: i32 = 5;

    /// `Elementals.FIRST_WEAPON_BONUS`.
    pub const FIRST_WEAPON_BONUS: i32 = 20;
    /// `Elementals.NEXT_WEAPON_BONUS`.
    pub const NEXT_WEAPON_BONUS: i32 = 5;
    /// `Elementals.ARMOR_BONUS`.
    pub const ARMOR_BONUS: i32 = 6;

    /// `Elementals.WEAPON_VALUES`: the attribute a weapon has at most, per stone level.
    pub const WEAPON_VALUES: [i32; 14] = [
        0,
        25,
        75,
        150,
        175,
        225,
        300,
        325,
        375,
        450,
        475,
        525,
        600,
        i32::MAX,
    ];

    /// `Elementals.ARMOR_VALUES`.
    pub const ARMOR_VALUES: [i32; 14] = [0, 12, 30, 60, 72, 90, 120, 132, 150, 180, 192, 210, 240, i32::MAX];

    /// Java `Elementals.getOppositeElement(element)` on a `byte`.
    // NOTE(java-quirk): `-1 % 2` is `-1` in Java (and Rust), so `NONE` is "odd" and gives `-2`.
    pub fn opposite_element(element: i32) -> i32 {
        i32::from(
            (if element % 2 == 0 {
                element.wrapping_add(1)
            } else {
                element.wrapping_sub(1)
            }) as i8,
        )
    }

    /// Java `Elementals.getElementName(element)`.
    pub fn element_name(element: i32) -> &'static str {
        match element {
            FIRE => "Fire",
            WATER => "Water",
            WIND => "Wind",
            EARTH => "Earth",
            DARK => "Dark",
            HOLY => "Holy",
            _ => "None",
        }
    }

    /// Java `Elementals.getElementId(name)`: case-insensitive, `NONE` for an unknown name.
    pub fn element_id(name: &str) -> i32 {
        match name.to_lowercase().as_str() {
            "fire" => FIRE,
            "water" => WATER,
            "wind" => WIND,
            "earth" => EARTH,
            "dark" => DARK,
            "holy" => HOLY,
            _ => NONE,
        }
    }
}

// ---- the item as the enchant rules see it ---------------------------------------------------------

/// The parts of Java's `Item` the enchant rules read.
#[derive(Debug, Clone, Copy)]
pub struct EnchantTarget<'a> {
    pub template: &'a ItemTemplate,
    /// `getEnchantLevel()`.
    pub enchant_level: i32,
    /// `Item.isEnchantable()`: in the inventory or worn, and the template allows it (the blacklist
    /// of `Player.ini` included).
    pub enchantable: bool,
}

// ---- RangeChanceHolder / EnchantItemGroup ---------------------------------------------------------

/// Java `RangeChanceHolder`: the chance for the enchant levels `min..=max`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RangeChance {
    pub min: i32,
    pub max: i32,
    pub chance: f64,
}

/// Java `EnchantItemGroup`: the success chance by current enchant level.
#[derive(Debug, Clone, PartialEq)]
pub struct EnchantItemGroup {
    pub name: String,
    chances: Vec<RangeChance>,
}

impl EnchantItemGroup {
    /// Java `getChance(index)`: the first range that holds the level; a level past every range
    /// takes the last range's chance; `-1` (with a warning) for a group without ranges.
    pub fn chance(&self, index: i32) -> f64 {
        if let Some(last) = self.chances.last() {
            return self
                .chances
                .iter()
                .find(|holder| holder.min <= index && holder.max >= index)
                .map_or(last.chance, |holder| holder.chance);
        }
        warn!(
            "EnchantItemGroup: item group: {} doesn't have any chances!",
            self.name
        );
        -1.0
    }

    /// The ranges, in file order.
    pub fn chances(&self) -> &[RangeChance] {
        &self.chances
    }
}

// ---- EnchantRateItem / EnchantScrollGroup ---------------------------------------------------------

/// Java `EnchantRateItem`: which items a rate group (a named [`EnchantItemGroup`]) applies to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnchantRateItem {
    pub name: String,
    items: HashSet<i32>,
    slot: i32,
    magic_weapon: Option<bool>,
}

impl EnchantRateItem {
    /// Java `validate(item)`.
    pub fn validate(&self, item: &ItemTemplate) -> bool {
        if !self.items.is_empty() && !self.items.contains(&item.id) {
            return false;
        }
        if self.slot != 0 && (item.body_part.mask() & self.slot) == 0 {
            return false;
        }
        if let Some(magic) = self.magic_weapon
            && item.is_magic_weapon() != magic
        {
            return false;
        }
        true
    }
}

/// Java `EnchantScrollGroup`: the rate groups of one scroll group, in file order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnchantScrollGroup {
    pub id: i32,
    rate_groups: Vec<EnchantRateItem>,
}

impl EnchantScrollGroup {
    /// Java `getRateGroup(item)`: the first rate group that takes the item.
    pub fn rate_group(&self, item: &ItemTemplate) -> Option<&EnchantRateItem> {
        self.rate_groups.iter().find(|group| group.validate(item))
    }
}

// ---- EnchantItemGroupsData ------------------------------------------------------------------------

/// Java `EnchantItemGroupsData`.
#[derive(Debug, Default)]
pub struct EnchantItemGroups {
    item_groups: HashMap<String, EnchantItemGroup>,
    scroll_groups: HashMap<i32, EnchantScrollGroup>,
    max_weapon_enchant: i32,
    max_armor_enchant: i32,
    max_accessory_enchant: i32,
}

impl EnchantItemGroups {
    const LOADER: &'static str = "EnchantItemGroupsData";

    /// Loads `<root>/data/EnchantItemGroups.xml` and logs Java's count lines. The "Max weapon
    /// enchant" lines Java adds with `OverEnchantProtection` are [`Self::log_max_enchants`].
    pub fn load(root: &Path) -> Self {
        let mut table = Self::default();
        load_file(&root.join("data/EnchantItemGroups.xml"), |doc: RawGroupsFile| {
            table.add(doc)
        });
        table.finish();
        log_loaded(Self::LOADER, table.item_groups.len(), "item group templates");
        log_loaded(Self::LOADER, table.scroll_groups.len(), "scroll group templates");
        table
    }

    /// The three "Max ... enchant is set to" lines of Java's `load()` (printed when
    /// `OverEnchantProtection` is on).
    pub fn log_max_enchants(&self) {
        info!(
            "{}: Max weapon enchant is set to {}.",
            Self::LOADER,
            self.max_weapon_enchant
        );
        info!(
            "{}: Max armor enchant is set to {}.",
            Self::LOADER,
            self.max_armor_enchant
        );
        info!(
            "{}: Max accessory enchant is set to {}.",
            Self::LOADER,
            self.max_accessory_enchant
        );
    }

    /// Builds the table from XML text. For tests and tools.
    pub fn from_xml(xml: &str) -> Result<Self, DataError> {
        let doc = quick_xml::de::from_str(xml).map_err(|source| DataError::Xml {
            path: "<inline>".into(),
            source,
        })?;
        let mut table = Self::default();
        table.add(doc)?;
        table.finish();
        Ok(table)
    }

    fn add(&mut self, doc: RawGroupsFile) -> Result<(), DataError> {
        for child in doc.children {
            match child {
                RawGroupsChild::RateGroup(group) => self.add_rate_group(*group)?,
                RawGroupsChild::ScrollGroup(group) => self.add_scroll_group(*group)?,
                RawGroupsChild::Other => {}
            }
        }
        Ok(())
    }

    fn add_rate_group(&mut self, raw: RawRateGroup) -> Result<(), DataError> {
        let name = required(raw.name, "enchantRateGroup", "name")?;
        let mut group = EnchantItemGroup {
            name: name.clone(),
            chances: Vec::new(),
        };
        let result: Result<(), DataError> = (|| {
            for current in raw.current {
                let range = required(current.enchant, "current", "enchant")?;
                let chance_text = required(current.chance, "current", "chance")?;
                let chance = parse_double(&chance_text)
                    .ok_or_else(|| DataError::Invalid(format!("current chance=\"{chance_text}\"")))?;
                let mut min = -1;
                let mut max = 0;
                if range.contains('-') {
                    let split = java_split(&range, '-');
                    if split.len() == 2 && is_numeric(split[0]) && is_numeric(split[1]) {
                        min = parse_int(split[0])?;
                        max = parse_int(split[1])?;
                    }
                } else if is_numeric(&range) {
                    min = parse_int(&range)?;
                    max = min;
                }
                if min > -1 && max > -1 {
                    group.chances.push(RangeChance { min, max, chance });
                }
                // Try to get a generic max value.
                if chance > 0.0 {
                    if name.contains("WEAPON") {
                        self.max_weapon_enchant = self.max_weapon_enchant.max(max);
                    } else if name.contains("ACCESSORIES")
                        || name.contains("RING")
                        || name.contains("EARRING")
                        || name.contains("NECK")
                    {
                        self.max_accessory_enchant = self.max_accessory_enchant.max(max);
                    } else {
                        self.max_armor_enchant = self.max_armor_enchant.max(max);
                    }
                }
            }
            Ok(())
        })();
        // Java puts the group only after all its ranges parsed; an error leaves it out.
        result?;
        self.item_groups.insert(name, group);
        Ok(())
    }

    fn add_scroll_group(&mut self, raw: RawScrollGroup) -> Result<(), DataError> {
        let id = parse_int(&required(raw.id, "enchantScrollGroup", "id")?)?;
        let mut group = EnchantScrollGroup {
            id,
            rate_groups: Vec::new(),
        };
        for rate in raw.rates {
            let mut rate_group = EnchantRateItem {
                name: required(rate.group, "enchantRate", "group")?,
                items: HashSet::new(),
                slot: 0,
                magic_weapon: None,
            };
            for item in rate.items {
                if let Some(slot) = item.slot {
                    // Java: `BodyPart.fromName(..).getMask()` throws for an unknown name.
                    let part = BodyPart::from_name(&slot)
                        .ok_or_else(|| DataError::Invalid(format!("unknown body part \"{slot}\"")))?;
                    rate_group.slot |= part.mask();
                }
                if let Some(magic) = item.magic_weapon {
                    rate_group.magic_weapon = Some(parse_boolean(&magic));
                }
                if let Some(item_id) = item.item_id {
                    rate_group.items.insert(parse_int(&item_id)?);
                }
            }
            group.rate_groups.push(rate_group);
        }
        self.scroll_groups.insert(id, group);
        Ok(())
    }

    /// The end of Java's `parseDocument`.
    fn finish(&mut self) {
        // In case there is no accessories group set.
        if self.max_accessory_enchant == 0 {
            self.max_accessory_enchant = self.max_armor_enchant;
        }
        // Max enchant values are set to current max enchant + 1.
        self.max_weapon_enchant += 1;
        self.max_armor_enchant += 1;
        self.max_accessory_enchant += 1;
    }

    /// Java `getItemGroup(item, scrollGroup)`. `None` where Java returns `null`, and for a scroll
    /// group that does not exist (Java would throw; the callers check `scroll_group` first).
    pub fn item_group_of(&self, item: &ItemTemplate, scroll_group: i32) -> Option<&EnchantItemGroup> {
        let rate_group = self.scroll_groups.get(&scroll_group)?.rate_group(item)?;
        self.item_groups.get(&rate_group.name)
    }

    /// Java `getItemGroup(name)`.
    pub fn item_group(&self, name: &str) -> Option<&EnchantItemGroup> {
        self.item_groups.get(name)
    }

    /// Java `getScrollGroup(id)`.
    pub fn scroll_group(&self, id: i32) -> Option<&EnchantScrollGroup> {
        self.scroll_groups.get(&id)
    }

    /// Java `getMaxWeaponEnchant()`: the highest weapon enchant a chance exists for, plus one.
    pub fn max_weapon_enchant(&self) -> i32 {
        self.max_weapon_enchant
    }

    /// Java `getMaxArmorEnchant()`.
    pub fn max_armor_enchant(&self) -> i32 {
        self.max_armor_enchant
    }

    /// Java `getMaxAccessoryEnchant()`.
    pub fn max_accessory_enchant(&self) -> i32 {
        self.max_accessory_enchant
    }

    /// Number of item groups.
    pub fn item_group_count(&self) -> usize {
        self.item_groups.len()
    }

    /// Number of scroll groups.
    pub fn scroll_group_count(&self) -> usize {
        self.scroll_groups.len()
    }
}

// ---- AbstractEnchantItem / EnchantScroll / EnchantSupportItem -------------------------------------

/// Java `AbstractEnchantItem`: what scrolls and support items share.
#[derive(Debug, Clone, PartialEq)]
pub struct EnchantBase {
    pub id: i32,
    /// `getGrade()`: the grade of the items it can enchant.
    pub grade: CrystalType,
    /// `getMaxEnchantLevel()`: 65535 when the file gives none.
    pub max_enchant_level: i32,
    /// `getBonusRate()`.
    pub bonus_rate: f64,
}

impl EnchantBase {
    /// Java `AbstractEnchantItem.isValid(itemToEnchant, supportItem)`.
    fn is_valid(&self, target: &EnchantTarget<'_>, is_weapon: bool) -> bool {
        if !target.enchantable {
            return false;
        }
        let type2_fits = match target.template.type2 {
            TYPE2_WEAPON => is_weapon,
            TYPE2_SHIELD_ARMOR | TYPE2_ACCESSORY => !is_weapon,
            _ => false,
        };
        if !type2_fits {
            return false;
        }
        if self.max_enchant_level != 0 && target.enchant_level >= self.max_enchant_level {
            return false;
        }
        self.grade == target.template.crystal_type_plus()
    }
}

/// Java `EnchantSupportItem`.
#[derive(Debug, Clone, PartialEq)]
pub struct EnchantSupportItem {
    pub base: EnchantBase,
    /// `isWeapon()`: `SCRL_INC_ENCHANT_PROP_WP`.
    pub is_weapon: bool,
}

impl EnchantSupportItem {
    /// `isValid` of the base class (the support item does not override it).
    pub fn is_valid(&self, target: &EnchantTarget<'_>) -> bool {
        self.base.is_valid(target, self.is_weapon)
    }

    /// `getBonusRate()`.
    pub fn bonus_rate(&self) -> f64 {
        self.base.bonus_rate
    }
}

/// Java `EnchantScroll`.
#[derive(Debug, Clone, PartialEq)]
pub struct EnchantScroll {
    pub base: EnchantBase,
    /// `isWeapon()`.
    pub is_weapon: bool,
    /// `isBlessed()`: the item stays (at +0) when the enchant fails.
    pub is_blessed: bool,
    /// `isSafe()`: the enchant level stays when the enchant fails.
    pub is_safe: bool,
    scroll_group_id: i32,
    /// `_items`: the items this scroll is limited to, with the scroll group each one uses.
    items: HashMap<i32, i32>,
}

/// What `EnchantScroll.calculateSuccess` decided, before the dice.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EnchantOdds {
    /// `EnchantResultType.ERROR`: the scroll does not fit, or no chance is defined.
    Error,
    /// The percent chance to succeed (at most 100).
    Chance(f64),
}

impl EnchantScroll {
    /// Java `addItem(itemId, scrollGroupId)`.
    fn add_item(&mut self, item_id: i32, scroll_group_id: i32) {
        let group = if scroll_group_id > -1 {
            scroll_group_id
        } else {
            self.scroll_group_id
        };
        self.items.insert(item_id, group);
    }

    /// Java `getItems()`: the item ids the scroll is limited to (empty: any fitting item).
    pub fn items(&self) -> impl Iterator<Item = i32> + '_ {
        self.items.keys().copied()
    }

    /// Whether `item_id` is one of the items this scroll is limited to.
    pub fn limited_to(&self, item_id: i32) -> bool {
        self.items.contains_key(&item_id)
    }

    /// Java `getMaxEnchantLevel()`.
    pub fn max_enchant_level(&self) -> i32 {
        self.base.max_enchant_level
    }

    /// Java `isValid(itemToEnchant, supportItem)`. `data` is Java's
    /// `EnchantItemData.getInstance()` (the scrolls that are limited to other items).
    pub fn is_valid(
        &self,
        data: &EnchantItemData,
        target: &EnchantTarget<'_>,
        support: Option<&EnchantSupportItem>,
    ) -> bool {
        if !self.items.is_empty() && !self.items.contains_key(&target.template.id) {
            return false;
        }
        if let Some(support) = support {
            if self.is_blessed {
                return false;
            }
            if !support.is_valid(target) {
                return false;
            }
            if support.is_weapon != self.is_weapon {
                return false;
            }
        }
        if self.items.is_empty() {
            // A scroll for any item does not take an item another scroll is made for.
            for scroll in data.scrolls.values() {
                if scroll.base.id == self.base.id {
                    continue;
                }
                if !scroll.items.is_empty() && scroll.items.contains_key(&target.template.id) {
                    return false;
                }
            }
        }
        self.base.is_valid(target, self.is_weapon)
    }

    /// Java `getChance(player, enchantItem)`: the chance of the item's group at its enchant level;
    /// `-1` when the scroll group or the item group does not exist.
    pub fn chance(&self, groups: &EnchantItemGroups, target: &EnchantTarget<'_>) -> f64 {
        let scroll_group_id = self
            .items
            .get(&target.template.id)
            .copied()
            .unwrap_or(self.scroll_group_id);
        if groups.scroll_group(scroll_group_id).is_none() {
            warn!(
                "EnchantScroll: Unexistent enchant scroll group specified for enchant scroll: {}",
                self.base.id
            );
            return -1.0;
        }
        let Some(group) = groups.item_group_of(target.template, scroll_group_id) else {
            warn!(
                "EnchantScroll: Couldn't find enchant item group for scroll: {}",
                self.base.id
            );
            return -1.0;
        };
        group.chance(target.enchant_level)
    }

    /// Java `calculateSuccess` up to the dice: [`EnchantOdds::Error`] or the final percent
    /// `min(chance + bonusRate + supportBonusRate, 100)`. The caller rolls
    /// `100 * Rnd.nextDouble() < chance`.
    pub fn odds(
        &self,
        data: &EnchantItemData,
        groups: &EnchantItemGroups,
        target: &EnchantTarget<'_>,
        support: Option<&EnchantSupportItem>,
    ) -> EnchantOdds {
        if !self.is_valid(data, target, support) {
            return EnchantOdds::Error;
        }
        let chance = self.chance(groups, target);
        // NOTE(java-quirk): Java compares with `== -1`, so another negative chance would go on.
        if chance == -1.0 {
            return EnchantOdds::Error;
        }
        let support_bonus = support.map_or(0.0, EnchantSupportItem::bonus_rate);
        EnchantOdds::Chance((chance + self.base.bonus_rate + support_bonus).min(100.0))
    }
}

// ---- EnchantItemData ------------------------------------------------------------------------------

/// Java `EnchantItemData`: the scrolls and support items by item id.
#[derive(Debug, Default)]
pub struct EnchantItemData {
    scrolls: HashMap<i32, EnchantScroll>,
    supports: HashMap<i32, EnchantSupportItem>,
}

/// `AbstractEnchantItem.ENCHANT_TYPES`.
fn is_enchant_type(etc_type: EtcItemType) -> bool {
    matches!(
        etc_type,
        EtcItemType::AncientCrystalEnchantAm
            | EtcItemType::AncientCrystalEnchantWp
            | EtcItemType::BlessScrlEnchantAm
            | EtcItemType::BlessScrlEnchantWp
            | EtcItemType::ScrlEnchantAm
            | EtcItemType::ScrlEnchantWp
            | EtcItemType::ScrlIncEnchantPropAm
            | EtcItemType::ScrlIncEnchantPropWp
    )
}

impl EnchantItemData {
    const LOADER: &'static str = "EnchantItemData";

    /// Loads `<root>/data/EnchantItemData.xml` and logs Java's count lines. `items` is
    /// `ItemData`: every scroll must be an item of one of the enchant types.
    pub fn load(root: &Path, items: &ItemTemplates) -> Self {
        let mut table = Self::default();
        load_file(&root.join("data/EnchantItemData.xml"), |doc: RawEnchantFile| {
            table.add(doc, items)
        });
        log_loaded(Self::LOADER, table.scrolls.len(), "enchant scrolls");
        log_loaded(Self::LOADER, table.supports.len(), "support items");
        table
    }

    /// Builds the table from XML text. For tests and tools.
    pub fn from_xml(xml: &str, items: &ItemTemplates) -> Result<Self, DataError> {
        let doc = quick_xml::de::from_str(xml).map_err(|source| DataError::Xml {
            path: "<inline>".into(),
            source,
        })?;
        let mut table = Self::default();
        table.add(doc, items)?;
        Ok(table)
    }

    fn add(&mut self, doc: RawEnchantFile, items: &ItemTemplates) -> Result<(), DataError> {
        for child in doc.children {
            match child {
                RawEnchantChild::Enchant(raw) => self.add_scroll(*raw, items)?,
                RawEnchantChild::Support(raw) => self.add_support(&raw, items)?,
                RawEnchantChild::Other => {}
            }
        }
        Ok(())
    }

    /// The part of `AbstractEnchantItem`'s constructor after the item checks.
    fn base_of(
        id: i32,
        target_grade: Option<&str>,
        max_enchant: Option<&str>,
        bonus_rate: Option<&str>,
    ) -> Result<EnchantBase, DataError> {
        // `getEnum`: a name that is no grade throws; a missing one is `NONE`.
        let grade = match target_grade {
            None => CrystalType::None,
            Some(text) => CrystalType::from_name(text)
                .ok_or_else(|| DataError::Invalid(format!("targetGrade=\"{text}\" is no grade")))?,
        };
        let max_enchant_level = match max_enchant {
            None => 65535,
            Some(text) => parse_int(text)?,
        };
        let bonus_rate = match bonus_rate {
            None => 0.0,
            Some(text) => {
                parse_double(text).ok_or_else(|| DataError::Invalid(format!("bonusRate=\"{text}\"")))?
            }
        };
        Ok(EnchantBase {
            id,
            grade,
            max_enchant_level,
            bonus_rate,
        })
    }

    /// Java `AbstractEnchantItem`'s item checks: `NullPointerException` for an unknown item,
    /// `IllegalAccessError` for one that is no enchant item; both are logged and the entry is
    /// skipped.
    fn enchant_item_type<'a>(
        id: i32,
        kind: &str,
        items: &'a ItemTemplates,
    ) -> Option<(&'a ItemTemplate, EtcItemType)> {
        let Some(template) = items.get(id) else {
            warn!(
                "{}: Unexistent {kind}: {id} defined in enchant data!",
                Self::LOADER
            );
            return None;
        };
        let Some(etc_type) = template.etc().map(|e| e.etc_type).filter(|t| is_enchant_type(*t)) else {
            warn!(
                "{}: Wrong {kind} type: {id} defined in enchant data!",
                Self::LOADER
            );
            return None;
        };
        Some((template, etc_type))
    }

    fn add_scroll(&mut self, raw: RawScroll, items: &ItemTemplates) -> Result<(), DataError> {
        let id = parse_int(&required(raw.id.clone(), "enchant", "id")?)?;
        let Some((_, etc_type)) = Self::enchant_item_type(id, "enchant scroll item", items) else {
            return Ok(());
        };
        let base = Self::base_of(
            id,
            raw.target_grade.as_deref(),
            raw.max_enchant.as_deref(),
            raw.bonus_rate.as_deref(),
        )?;
        let scroll_group_id = match raw.scroll_group_id.as_deref() {
            None => 0,
            Some(text) => parse_int(text)?,
        };
        let mut scroll = EnchantScroll {
            base,
            is_weapon: matches!(
                etc_type,
                EtcItemType::AncientCrystalEnchantWp
                    | EtcItemType::BlessScrlEnchantWp
                    | EtcItemType::ScrlEnchantWp
            ),
            is_blessed: matches!(
                etc_type,
                EtcItemType::BlessScrlEnchantAm | EtcItemType::BlessScrlEnchantWp
            ),
            is_safe: matches!(
                etc_type,
                EtcItemType::AncientCrystalEnchantAm | EtcItemType::AncientCrystalEnchantWp
            ),
            scroll_group_id,
            items: HashMap::new(),
        };
        for item in raw.items {
            let item_id = parse_int(&required(item.id, "item", "id")?)?;
            let alt_group = match item.alt_scroll_group_id.as_deref() {
                None => -1,
                Some(text) => parse_int(text)?,
            };
            scroll.add_item(item_id, alt_group);
        }
        self.scrolls.insert(id, scroll);
        Ok(())
    }

    fn add_support(&mut self, raw: &RawSupport, items: &ItemTemplates) -> Result<(), DataError> {
        let id = parse_int(&required(raw.id.clone(), "support", "id")?)?;
        let Some((_, etc_type)) = Self::enchant_item_type(id, "enchant support item", items) else {
            return Ok(());
        };
        let base = Self::base_of(
            id,
            raw.target_grade.as_deref(),
            raw.max_enchant.as_deref(),
            raw.bonus_rate.as_deref(),
        )?;
        self.supports.insert(
            id,
            EnchantSupportItem {
                base,
                is_weapon: etc_type == EtcItemType::ScrlIncEnchantPropWp,
            },
        );
        Ok(())
    }

    /// Java `getEnchantScroll(item)` by item id.
    pub fn scroll(&self, item_id: i32) -> Option<&EnchantScroll> {
        self.scrolls.get(&item_id)
    }

    /// Java `getSupportItem(item)` by item id.
    pub fn support(&self, item_id: i32) -> Option<&EnchantSupportItem> {
        self.supports.get(&item_id)
    }

    /// Java `getScrolls()`.
    pub fn scrolls(&self) -> impl Iterator<Item = &EnchantScroll> {
        self.scrolls.values()
    }

    /// Number of scrolls.
    pub fn scroll_count(&self) -> usize {
        self.scrolls.len()
    }

    /// Number of support items.
    pub fn support_count(&self) -> usize {
        self.supports.len()
    }
}

// ---- ElementalAttributeData -----------------------------------------------------------------------

/// Java `ElementalItemType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ElementalItemType {
    Stone,
    RoughOre,
    Crystal,
    Jewel,
    Energy,
}

impl ElementalItemType {
    /// Java `valueOf(name)`.
    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "STONE" => Self::Stone,
            "ROUGH_ORE" => Self::RoughOre,
            "CRYSTAL" => Self::Crystal,
            "JEWEL" => Self::Jewel,
            "ENERGY" => Self::Energy,
            _ => return None,
        })
    }

    /// Java `getMaxLevel()`.
    pub fn max_level(self) -> i32 {
        match self {
            Self::Stone | Self::RoughOre => 3,
            Self::Crystal => 6,
            Self::Jewel => 9,
            Self::Energy => 12,
        }
    }
}

/// Java `ElementalItemHolder`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ElementalItemHolder {
    pub item_id: i32,
    /// `getElementId()`: `elementals::FIRE` to `DARK`, or `NONE`.
    pub element_id: i32,
    pub kind: ElementalItemType,
}

/// Java `ElementalAttributeData`: the element stones, crystals, jewels and energies by item id.
#[derive(Debug, Default)]
pub struct ElementalAttributeData {
    items: HashMap<i32, ElementalItemHolder>,
}

impl ElementalAttributeData {
    const LOADER: &'static str = "ElementalAttributeData";

    /// Loads `<root>/data/ElementalAttributeData.xml` and logs Java's count line.
    pub fn load(root: &Path, items: &ItemTemplates) -> Self {
        let mut table = Self::default();
        load_file(
            &root.join("data/ElementalAttributeData.xml"),
            |doc: RawElementalFile| table.add(doc, items),
        );
        log_loaded(Self::LOADER, table.items.len(), "elemental attribute items");
        table
    }

    /// Builds the table from XML text. For tests and tools.
    pub fn from_xml(xml: &str, items: &ItemTemplates) -> Result<Self, DataError> {
        let doc = quick_xml::de::from_str(xml).map_err(|source| DataError::Xml {
            path: "<inline>".into(),
            source,
        })?;
        let mut table = Self::default();
        table.add(doc, items)?;
        Ok(table)
    }

    fn add(&mut self, doc: RawElementalFile, items: &ItemTemplates) -> Result<(), DataError> {
        for raw in doc.items {
            let id = parse_int(&required(raw.id, "item", "id")?)?;
            if items.get(id).is_none() {
                info!("{}: Could not find item with id {id}.", Self::LOADER);
                continue;
            }
            // `set.getString("elemental")` throws for a missing one; an unknown name is `NONE`.
            let element_id = match required(raw.elemental, "item", "elemental")?.as_str() {
                "FIRE" => elementals::FIRE,
                "WATER" => elementals::WATER,
                "WIND" => elementals::WIND,
                "EARTH" => elementals::EARTH,
                "HOLY" => elementals::HOLY,
                "DARK" => elementals::DARK,
                _ => elementals::NONE,
            };
            let kind_text = required(raw.kind, "item", "type")?;
            let kind = ElementalItemType::from_name(&kind_text).ok_or_else(|| {
                DataError::Invalid(format!("type=\"{kind_text}\" is no elemental item type"))
            })?;
            self.items.insert(
                id,
                ElementalItemHolder {
                    item_id: id,
                    element_id,
                    kind,
                },
            );
        }
        Ok(())
    }

    /// Java `getElementalItem(itemId)`.
    pub fn get(&self, item_id: i32) -> Option<&ElementalItemHolder> {
        self.items.get(&item_id)
    }

    /// Java `Elementals.getItemElement(itemId)`: `NONE` for an item that is no stone.
    pub fn item_element(&self, item_id: i32) -> i32 {
        self.get(item_id).map_or(elementals::NONE, |item| item.element_id)
    }

    /// Java `Elementals.getMaxElementLevel(itemId)`: -1 for an item that is no stone.
    pub fn max_element_level(&self, item_id: i32) -> i32 {
        self.get(item_id).map_or(-1, |item| item.kind.max_level())
    }

    /// Number of elemental items.
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// True when nothing was loaded.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

// ---- helpers --------------------------------------------------------------------------------------

/// Java `StringUtil.isNumeric`: non-empty, digits only.
fn is_numeric(text: &str) -> bool {
    !text.is_empty() && text.chars().all(|c| c.is_ascii_digit())
}

/// `Integer.decode` of an attribute; Java throws `NumberFormatException` for a bad one.
fn parse_int(text: &str) -> Result<i32, DataError> {
    decode_i32(text).ok_or_else(|| DataError::Invalid(format!("\"{text}\" is no integer")))
}

/// A required attribute: Java's `parseString`/`parseInteger` on a missing attribute throws.
fn required(value: Option<String>, element: &str, attribute: &str) -> Result<String, DataError> {
    value.ok_or_else(|| DataError::Invalid(format!("<{element}> without {attribute}")))
}

// ---- Raw XML shape (xsd/enchantItemData.xsd, enchantItemGroups.xsd, ElementalAttributeData.xsd) ---

#[derive(Deserialize)]
struct RawEnchantFile {
    #[serde(rename = "$value", default)]
    children: Vec<RawEnchantChild>,
}

#[derive(Deserialize)]
enum RawEnchantChild {
    #[serde(rename = "enchant")]
    Enchant(Box<RawScroll>),
    #[serde(rename = "support")]
    Support(Box<RawSupport>),
    #[serde(other)]
    Other,
}

#[derive(Deserialize)]
struct RawScroll {
    #[serde(rename = "@id")]
    id: Option<String>,
    #[serde(rename = "@targetGrade")]
    target_grade: Option<String>,
    #[serde(rename = "@maxEnchant")]
    max_enchant: Option<String>,
    #[serde(rename = "@bonusRate")]
    bonus_rate: Option<String>,
    #[serde(rename = "@scrollGroupId")]
    scroll_group_id: Option<String>,
    #[serde(rename = "item", default)]
    items: Vec<RawScrollItem>,
}

#[derive(Deserialize)]
struct RawScrollItem {
    #[serde(rename = "@id")]
    id: Option<String>,
    #[serde(rename = "@altScrollGroupId")]
    alt_scroll_group_id: Option<String>,
}

#[derive(Deserialize)]
struct RawSupport {
    #[serde(rename = "@id")]
    id: Option<String>,
    #[serde(rename = "@targetGrade")]
    target_grade: Option<String>,
    #[serde(rename = "@maxEnchant")]
    max_enchant: Option<String>,
    #[serde(rename = "@bonusRate")]
    bonus_rate: Option<String>,
}

#[derive(Deserialize)]
struct RawGroupsFile {
    #[serde(rename = "$value", default)]
    children: Vec<RawGroupsChild>,
}

#[derive(Deserialize)]
enum RawGroupsChild {
    #[serde(rename = "enchantRateGroup")]
    RateGroup(Box<RawRateGroup>),
    #[serde(rename = "enchantScrollGroup")]
    ScrollGroup(Box<RawScrollGroup>),
    #[serde(other)]
    Other,
}

#[derive(Deserialize)]
struct RawRateGroup {
    #[serde(rename = "@name")]
    name: Option<String>,
    #[serde(rename = "current", default)]
    current: Vec<RawCurrent>,
}

#[derive(Deserialize)]
struct RawCurrent {
    #[serde(rename = "@enchant")]
    enchant: Option<String>,
    #[serde(rename = "@chance")]
    chance: Option<String>,
}

#[derive(Deserialize)]
struct RawScrollGroup {
    #[serde(rename = "@id")]
    id: Option<String>,
    #[serde(rename = "enchantRate", default)]
    rates: Vec<RawRate>,
}

#[derive(Deserialize)]
struct RawRate {
    #[serde(rename = "@group")]
    group: Option<String>,
    #[serde(rename = "item", default)]
    items: Vec<RawRateItem>,
}

#[derive(Deserialize)]
struct RawRateItem {
    #[serde(rename = "@slot")]
    slot: Option<String>,
    #[serde(rename = "@magicWeapon")]
    magic_weapon: Option<String>,
    #[serde(rename = "@itemId")]
    item_id: Option<String>,
}

#[derive(Deserialize)]
struct RawElementalFile {
    #[serde(rename = "item", default)]
    items: Vec<RawElementalItem>,
}

#[derive(Deserialize)]
struct RawElementalItem {
    #[serde(rename = "@id")]
    id: Option<String>,
    #[serde(rename = "@elemental")]
    elemental: Option<String>,
    #[serde(rename = "@type")]
    kind: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    const ITEMS: &str = r#"<list>
<item id="2" type="Weapon" name="C Sword"><set name="crystal_type" val="C" /><set name="weapon_type" val="SWORD" /><set name="bodypart" val="rhand" /><set name="enchant_enabled" val="true" /></item>
<item id="3" type="Weapon" name="C Staff"><set name="crystal_type" val="C" /><set name="weapon_type" val="BLUNT" /><set name="bodypart" val="lrhand" /><set name="is_magic_weapon" val="true" /><set name="enchant_enabled" val="true" /></item>
<item id="4" type="Armor" name="C Helmet"><set name="crystal_type" val="C" /><set name="armor_type" val="HEAVY" /><set name="bodypart" val="head" /><set name="enchant_enabled" val="true" /></item>
<item id="729" type="EtcItem" name="Scroll: Enchant Weapon (A)"><set name="etcitem_type" val="SCRL_ENCHANT_WP" /><set name="handler" val="EnchantScrolls" /></item>
<item id="951" type="EtcItem" name="Scroll: Enchant Weapon (C)"><set name="etcitem_type" val="SCRL_ENCHANT_WP" /><set name="handler" val="EnchantScrolls" /></item>
<item id="952" type="EtcItem" name="Scroll: Enchant Armor (C)"><set name="etcitem_type" val="SCRL_ENCHANT_AM" /></item>
<item id="6573" type="EtcItem" name="Blessed Scroll: Enchant Weapon (C)"><set name="etcitem_type" val="BLESS_SCRL_ENCHANT_WP" /></item>
<item id="22014" type="EtcItem" name="Ancient Crystal: Enchant Weapon (C)"><set name="etcitem_type" val="ANCIENT_CRYSTAL_ENCHANT_WP" /></item>
<item id="12364" type="EtcItem" name="Enchant rate support (C)"><set name="etcitem_type" val="SCRL_INC_ENCHANT_PROP_WP" /></item>
<item id="9999" type="EtcItem" name="Not a scroll"><set name="etcitem_type" val="NONE" /></item>
<item id="13539" type="Weapon" name="Special Staff"><set name="weapon_type" val="BLUNT" /><set name="bodypart" val="rhand" /><set name="enchant_enabled" val="true" /></item>
<item id="13540" type="EtcItem" name="Special Scroll"><set name="etcitem_type" val="SCRL_ENCHANT_WP" /></item>
</list>"#;

    const GROUPS: &str = r#"<list>
<enchantRateGroup name="ARMOR_GROUP"><current enchant="0-2" chance="100" /><current enchant="3-15" chance="66" /><current enchant="16-65535" chance="0" /></enchantRateGroup>
<enchantRateGroup name="FIGHTER_WEAPON_GROUP"><current enchant="0-2" chance="100" /><current enchant="3-15" chance="66" /></enchantRateGroup>
<enchantRateGroup name="MAGE_WEAPON_GROUP"><current enchant="0-2" chance="100" /><current enchant="3" chance="50" /><current enchant="4-15" chance="20" /></enchantRateGroup>
<enchantScrollGroup id="0">
<enchantRate group="ARMOR_GROUP"><item slot="head" /><item slot="chest" /></enchantRate>
<enchantRate group="FIGHTER_WEAPON_GROUP"><item slot="rhand" magicWeapon="false" /><item slot="lrhand" magicWeapon="false" /></enchantRate>
<enchantRate group="MAGE_WEAPON_GROUP"><item slot="rhand" magicWeapon="true" /><item slot="lrhand" magicWeapon="true" /></enchantRate>
</enchantScrollGroup>
</list>"#;

    fn items() -> ItemTemplates {
        ItemTemplates::from_xml(ITEMS).unwrap()
    }

    fn data() -> EnchantItemData {
        let xml = r#"<list>
<enchant id="729" targetGrade="A" maxEnchant="16" />
<enchant id="951" targetGrade="C" maxEnchant="16" />
<enchant id="952" targetGrade="C" maxEnchant="16" />
<support id="12364" targetGrade="C" maxEnchant="9" bonusRate="15" />
<enchant id="6573" targetGrade="C" maxEnchant="16" />
<enchant id="22014" targetGrade="C" maxEnchant="16" bonusRate="10" />
<enchant id="13540" maxEnchant="23"><item id="13539" /></enchant>
<enchant id="9999" targetGrade="C" />
<enchant id="777777" targetGrade="C" />
</list>"#;
        EnchantItemData::from_xml(xml, &items()).unwrap()
    }

    fn target<'a>(items: &'a ItemTemplates, id: i32, level: i32) -> EnchantTarget<'a> {
        EnchantTarget {
            template: items.get(id).unwrap(),
            enchant_level: level,
            enchantable: true,
        }
    }

    #[test]
    fn the_loader_keeps_real_scrolls_and_skips_the_rest() {
        let data = data();
        // 9999 is no enchant item, 777777 does not exist (Java logs both and skips them).
        assert_eq!((data.scroll_count(), data.support_count()), (6, 1));
        let scroll = data.scroll(729).unwrap();
        assert_eq!(scroll.base.grade, CrystalType::A);
        assert_eq!(scroll.max_enchant_level(), 16);
        assert!(scroll.is_weapon && !scroll.is_blessed && !scroll.is_safe);
        assert!(data.scroll(6573).unwrap().is_blessed);
        assert!(data.scroll(22014).unwrap().is_safe);
        assert!(!data.scroll(952).unwrap().is_weapon);
        assert!((data.scroll(22014).unwrap().base.bonus_rate - 10.0).abs() < f64::EPSILON);
        // No `maxEnchant` is 65535, no `targetGrade` is NONE.
        assert_eq!(data.scroll(13540).unwrap().base.grade, CrystalType::None);
        let support = data.support(12364).unwrap();
        assert!(support.is_weapon);
        assert!((support.bonus_rate() - 15.0).abs() < f64::EPSILON);
        assert_eq!(support.base.max_enchant_level, 9);
    }

    #[test]
    fn a_scroll_takes_the_right_kind_grade_and_level() {
        let (data, items) = (data(), items());
        let scroll = data.scroll(951).unwrap();
        assert!(scroll.is_valid(&data, &target(&items, 2, 0), None));
        assert!(scroll.is_valid(&data, &target(&items, 3, 15), None));
        // The level limit: `>= maxEnchant` is refused.
        assert!(!scroll.is_valid(&data, &target(&items, 2, 16), None));
        // A weapon scroll does not take armor, an armor scroll does not take a weapon.
        assert!(!scroll.is_valid(&data, &target(&items, 4, 0), None));
        assert!(
            !data
                .scroll(952)
                .unwrap()
                .is_valid(&data, &target(&items, 2, 0), None)
        );
        assert!(
            data.scroll(952)
                .unwrap()
                .is_valid(&data, &target(&items, 4, 0), None)
        );
        // The grade must be the item's.
        assert!(
            !data
                .scroll(729)
                .unwrap()
                .is_valid(&data, &target(&items, 2, 0), None)
        );
        // `Item.isEnchantable()` false (worn elsewhere, blacklisted, not enchantable).
        let mut no = target(&items, 2, 0);
        no.enchantable = false;
        assert!(!scroll.is_valid(&data, &no, None));
    }

    #[test]
    fn a_limited_scroll_only_takes_its_items_and_leaves_them_alone_for_others() {
        let (data, items) = (data(), items());
        let special = data.scroll(13540).unwrap();
        // Limited to item 13539 whose grade is NONE.
        assert!(special.is_valid(&data, &target(&items, 13539, 0), None));
        assert!(!special.is_valid(&data, &target(&items, 2, 0), None));
        // The scroll for any item does not fit an item another scroll is made for.
        assert!(
            !data
                .scroll(951)
                .unwrap()
                .is_valid(&data, &target(&items, 13539, 0), None)
        );
    }

    #[test]
    fn support_items_follow_the_scroll_and_the_item() {
        let (data, items) = (data(), items());
        let support = data.support(12364);
        let normal = data.scroll(951).unwrap();
        assert!(normal.is_valid(&data, &target(&items, 2, 0), support));
        // The support item's own maxEnchant (9).
        assert!(!normal.is_valid(&data, &target(&items, 2, 9), support));
        assert!(normal.is_valid(&data, &target(&items, 2, 8), support));
        // Not with a blessed scroll, not for armor.
        assert!(
            !data
                .scroll(6573)
                .unwrap()
                .is_valid(&data, &target(&items, 2, 0), support)
        );
        assert!(
            !data
                .scroll(952)
                .unwrap()
                .is_valid(&data, &target(&items, 4, 0), support)
        );
    }

    #[test]
    fn rate_groups_pick_the_chance_of_the_current_level() {
        let groups = EnchantItemGroups::from_xml(GROUPS).unwrap();
        let items = items();
        assert_eq!((groups.item_group_count(), groups.scroll_group_count()), (3, 1));
        // 0-2 100, 3-15 66.
        let fighter = groups.item_group("FIGHTER_WEAPON_GROUP").unwrap();
        assert_eq!(
            (fighter.chance(0), fighter.chance(2), fighter.chance(3)),
            (100.0, 100.0, 66.0)
        );
        // A level past every range takes the last range's chance.
        assert_eq!(fighter.chance(40), 66.0);
        // A single level `3` is its own range.
        let mage = groups.item_group("MAGE_WEAPON_GROUP").unwrap();
        assert_eq!((mage.chance(3), mage.chance(4)), (50.0, 20.0));
        // The sword is a fighter weapon, the staff a mage weapon, the helmet an armor.
        let sword = groups.item_group_of(items.get(2).unwrap(), 0).unwrap();
        assert_eq!(sword.name, "FIGHTER_WEAPON_GROUP");
        let staff = groups.item_group_of(items.get(3).unwrap(), 0).unwrap();
        assert_eq!(staff.name, "MAGE_WEAPON_GROUP");
        assert_eq!(
            groups.item_group_of(items.get(4).unwrap(), 0).unwrap().name,
            "ARMOR_GROUP"
        );
        assert!(groups.item_group_of(items.get(2).unwrap(), 5).is_none());
    }

    #[test]
    fn the_max_enchant_is_the_highest_range_with_a_chance_plus_one() {
        let groups = EnchantItemGroups::from_xml(GROUPS).unwrap();
        // WEAPON groups reach 15; ARMOR_GROUP's 16-65535 has chance 0 and does not count, so 15.
        assert_eq!(groups.max_weapon_enchant(), 16);
        assert_eq!(groups.max_armor_enchant(), 16);
        // No accessory group: the armor value (Java sets it before adding one).
        assert_eq!(groups.max_accessory_enchant(), 16);
    }

    #[test]
    fn the_max_enchant_tells_accessories_and_armor_apart() {
        let xml = r#"<list>
<enchantRateGroup name="ARMOR_GROUP"><current enchant="0-9" chance="50" /></enchantRateGroup>
<enchantRateGroup name="RING_GROUP"><current enchant="0-4" chance="50" /></enchantRateGroup>
<enchantRateGroup name="WEAPON_GROUP"><current enchant="0-19" chance="50" /></enchantRateGroup>
</list>"#;
        let groups = EnchantItemGroups::from_xml(xml).unwrap();
        assert_eq!(
            (
                groups.max_weapon_enchant(),
                groups.max_armor_enchant(),
                groups.max_accessory_enchant()
            ),
            (20, 10, 5)
        );
    }

    #[test]
    fn odds_add_the_bonus_rates_and_cap_at_100() {
        let (data, items) = (data(), items());
        let groups = EnchantItemGroups::from_xml(GROUPS).unwrap();
        let sword = |level| target(&items, 2, level);
        let normal = data.scroll(951).unwrap();
        assert_eq!(
            normal.odds(&data, &groups, &sword(0), None),
            EnchantOdds::Chance(100.0)
        );
        assert_eq!(
            normal.odds(&data, &groups, &sword(5), None),
            EnchantOdds::Chance(66.0)
        );
        // +15 (support) +66.
        assert_eq!(
            normal.odds(&data, &groups, &sword(5), data.support(12364)),
            EnchantOdds::Chance(81.0)
        );
        // The ancient crystal's own +10.
        assert_eq!(
            data.scroll(22014).unwrap().odds(&data, &groups, &sword(5), None),
            EnchantOdds::Chance(76.0)
        );
        assert_eq!(
            data.scroll(22014).unwrap().odds(&data, &groups, &sword(0), None),
            EnchantOdds::Chance(100.0)
        );
        // The scroll does not fit: ERROR.
        assert_eq!(
            normal.odds(&data, &groups, &target(&items, 4, 0), None),
            EnchantOdds::Error
        );
        // A scroll group that does not exist: -1, ERROR.
        let bare = EnchantItemGroups::from_xml("<list></list>").unwrap();
        assert_eq!(normal.odds(&data, &bare, &sword(0), None), EnchantOdds::Error);
    }

    #[test]
    fn the_scroll_group_of_a_limited_item_is_the_alternate_one() {
        let (items, groups) = (items(), EnchantItemGroups::from_xml(GROUPS).unwrap());
        let xml = r#"<list>
<enchant id="951" targetGrade="C" scrollGroupId="7"><item id="2" altScrollGroupId="0" /><item id="3" /></enchant>
</list>"#;
        let data = EnchantItemData::from_xml(xml, &items).unwrap();
        let scroll = data.scroll(951).unwrap();
        // Item 2 takes scroll group 0, item 3 the scroll's own group 7 (no such group).
        assert!(scroll.chance(&groups, &target(&items, 2, 0)) > 0.0);
        assert_eq!(scroll.chance(&groups, &target(&items, 3, 0)), -1.0);
        assert_eq!(scroll.items().count(), 2);
        assert!(scroll.limited_to(3));
    }

    #[test]
    fn element_helpers_follow_java() {
        use elementals::*;
        assert_eq!(opposite_element(FIRE), WATER);
        assert_eq!(opposite_element(WATER), FIRE);
        assert_eq!(opposite_element(HOLY), DARK);
        assert_eq!(opposite_element(DARK), HOLY);
        // NOTE(java-quirk): `-1 % 2 == -1`.
        assert_eq!(opposite_element(NONE), -2);
        assert_eq!(opposite_element(-2), -1);
        assert_eq!(element_name(WIND), "Wind");
        assert_eq!(element_name(NONE), "None");
        assert_eq!(element_id("Earth"), EARTH);
        assert_eq!(element_id("nothing"), NONE);
        assert_eq!(WEAPON_VALUES[3], 150);
        assert_eq!(ARMOR_VALUES[12], 240);
    }

    #[test]
    fn elemental_stones_load_with_their_type_and_level() {
        let items = ItemTemplates::from_xml(
            r#"<list><item id="9546" type="EtcItem" name="Fire Stone" /><item id="9552" type="EtcItem" name="Fire Crystal" /></list>"#,
        )
        .unwrap();
        let xml = r#"<list>
<item id="9546" elemental="FIRE" type="STONE" />
<item id="9552" elemental="DARK" type="CRYSTAL" />
<item id="1" elemental="FIRE" type="STONE" />
<item id="9546" elemental="NOPE" type="ENERGY" />
</list>"#;
        let data = ElementalAttributeData::from_xml(xml, &items).unwrap();
        // Item 1 does not exist (skipped); a later row for the same id replaces the earlier one,
        // and an unknown element name is NONE.
        assert_eq!(data.len(), 2);
        assert_eq!(data.item_element(9546), elementals::NONE);
        assert_eq!(data.max_element_level(9546), 12);
        assert_eq!(data.item_element(9552), elementals::DARK);
        assert_eq!(data.max_element_level(9552), 6);
        assert_eq!(data.item_element(5), elementals::NONE);
        assert_eq!(data.max_element_level(5), -1);
    }

    #[test]
    fn the_shipped_files_load() {
        let root = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game"));
        if !root.join("data/EnchantItemData.xml").is_file() {
            eprintln!("skipped: the Java datapack is not here");
            return;
        }
        let items = ItemTemplates::load(root, false);
        let data = EnchantItemData::load(root, &items);
        let groups = EnchantItemGroups::load(root);
        let elements = ElementalAttributeData::load(root, &items);
        assert!(data.scroll_count() > 30, "{} scrolls", data.scroll_count());
        assert!(data.support_count() >= 20);
        assert!(groups.item_group_count() > 3 && groups.scroll_group_count() >= 1);
        assert!(!elements.is_empty());
        // A plain C-grade weapon scroll, +0 to +2 always works, +3 on is 66 %.
        let scroll = data.scroll(951).unwrap();
        let sword = items.iter().find(|t| {
            t.is_weapon() && t.crystal_type == CrystalType::C && t.enchantable && !t.is_magic_weapon()
        });
        if let Some(sword) = sword {
            let at = |level| EnchantTarget {
                template: sword,
                enchant_level: level,
                enchantable: true,
            };
            assert_eq!(scroll.chance(&groups, &at(0)), 100.0);
            assert_eq!(scroll.chance(&groups, &at(3)), 66.0);
            assert_eq!(scroll.chance(&groups, &at(16)), 0.0);
        }
        assert_eq!(groups.max_weapon_enchant(), 16);
    }
}
