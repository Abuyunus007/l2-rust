//! Item templates: weapons, armor and etc items with the fields inventories, the character screen,
//! `UserInfo` and `ItemList` need.
//!
//! Java: `gameserver/data/xml/ItemData.java`, `util/DocumentItem.java` (+ `DocumentBase.java`),
//! `entity/item/{ItemTemplate,Weapon,Armor,EtcItem}.java` and the enums in [`types`].
//! Data: `data/stats/items/*.xml` (and `custom/` when enabled), `xsd/items.xsd`.
//!
//! # How Java builds an item (and what is ported)
//! `DocumentItem` collects every `<set name=".." val=".."/>` of an `<item>` into a string map (`StatSet`,
//! pre-filled with `item_id` and `name`; a later `<set>` of the same name wins, even `name`). A value
//! that starts with `#` is a reference into a `<table name="#x">a b c</table>` of the same item
//! (always index 0 here, Java passes level 1). When the item is finished the class named by the
//! `type` attribute (`Weapon`, `Armor`, `EtcItem`) reads typed values from the map with defaults;
//! a value that does not parse (`Integer.parseInt`, `Enum.valueOf`) throws and **the item is skipped**
//! with a warning, the rest of the file still loads. Booleans use `Boolean.parseBoolean`, so only
//! `true` (any case) is true: `1` is false.
//!
//! # Stat functions, conditions and skills (M4-DATA-10)
//! `<stats>` becomes [`ItemTemplate::funcs`] (the `switch` of `DocumentItem`: which stat becomes
//! `set`, `add` or `mul`, anything else is skipped with `Unhandled stat type`), followed for
//! weapons and armor by the automatic `enchant` functions. `<conditions>` becomes
//! [`ItemTemplate::conditions`] (with `msg`/`msgId`/`addName`), `<skills>` becomes
//! [`ItemTemplate::skills`] (ids and levels only). Nothing is evaluated here. The parsers are in
//! [`crate::stats`]; the item side is `item/funcs.rs` and `item/skills.rs`.
//!
//! # Not loaded (later cards)
//! The plain `<set>`s only some handlers read (`equip_condition`, `use_condition`, ...). The Java order rule
//! is kept: a `<set>` or `<table>` after `<stats>`, `<conditions>` or `<capsuled_items>` fails the
//! item (Java: "Item created but set node found!"); `<skills>` does not create the item.
//!
//! NOTE(java-quirk): Java also treats an attribute-less text-only element such as
//! `<weight>5</weight>` as a `<set>`. No shipped item file uses that form, so it is not supported
//! here (the element is ignored).

mod capsule;
mod funcs;
mod skills;
pub mod types;

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

use serde::Deserialize;
use tracing::{info, warn};

use crate::loader::{DataError, load_file, log_loaded};
use crate::stats::{ConditionParser, FuncTemplate, MessagedCondition, XmlNode};
pub use capsule::ExtractableProduct;
pub use funcs::{Element, ElementalValue};
pub use skills::{ChanceSkill, ItemSkillType, ItemSkills, SkillRef};
pub use types::{
    ActionType, ArmorType, BodyPart, CrystalType, EtcItemType, ItemGrade, MaterialType, WeaponType,
};

/// `ItemTemplate.TYPE1_WEAPON_RING_EARRING_NECKLACE`.
pub const TYPE1_WEAPON_RING_EARRING_NECKLACE: i32 = 0;
/// `ItemTemplate.TYPE1_SHIELD_ARMOR`.
pub const TYPE1_SHIELD_ARMOR: i32 = 1;
/// `ItemTemplate.TYPE1_ITEM_QUESTITEM_ADENA`.
pub const TYPE1_ITEM_QUESTITEM_ADENA: i32 = 4;
/// `ItemTemplate.TYPE2_WEAPON`.
pub const TYPE2_WEAPON: i32 = 0;
/// `ItemTemplate.TYPE2_SHIELD_ARMOR`.
pub const TYPE2_SHIELD_ARMOR: i32 = 1;
/// `ItemTemplate.TYPE2_ACCESSORY`.
pub const TYPE2_ACCESSORY: i32 = 2;
/// `ItemTemplate.TYPE2_QUEST`.
pub const TYPE2_QUEST: i32 = 3;
/// `ItemTemplate.TYPE2_MONEY`.
pub const TYPE2_MONEY: i32 = 4;
/// `ItemTemplate.TYPE2_OTHER`.
pub const TYPE2_OTHER: i32 = 5;

/// `Inventory.ADENA_ID`.
pub const ADENA_ID: i32 = 57;
/// `Inventory.ANCIENT_ADENA_ID`.
pub const ANCIENT_ADENA_ID: i32 = 5575;

/// Java `Weapon`: what only weapons have.
#[derive(Debug, Clone, PartialEq)]
pub struct WeaponInfo {
    /// `weapon_type` (Java upper-cases the value before `valueOf`; absent means `NONE`).
    pub weapon_type: WeaponType,
    /// `is_magic_weapon`.
    pub magic_weapon: bool,
    /// `soulshots`: soulshots used per attack.
    pub soulshot_count: i32,
    /// `spiritshots`: spiritshots used per attack.
    pub spiritshot_count: i32,
    /// `mp_consume`.
    pub mp_consume: i32,
    /// `attack_range`, default 40.
    pub base_attack_range: i32,
    /// Third part of `damage_range`; 40 when the weapon has none.
    pub base_attack_radius: i32,
    /// Fourth part of `damage_range`; 0 when the weapon has none.
    pub base_attack_angle: i32,
    /// First number of `reduced_soulshot` ("chance,count").
    pub reduced_soulshot_chance: i32,
    /// Second number of `reduced_soulshot`.
    pub reduced_soulshot: i32,
    /// First number of `reduced_mp_consume`.
    pub reduced_mp_consume_chance: i32,
    /// Second number of `reduced_mp_consume`.
    pub reduced_mp_consume: i32,
    /// `change_weaponId`.
    pub change_weapon_id: i32,
    /// `isForceEquip`.
    pub force_equip: bool,
    /// `isAttackWeapon`, default true.
    pub attack_weapon: bool,
    /// `useWeaponSkillsOnly`.
    pub use_weapon_skills_only: bool,
}

/// Java `Armor`: what only armor, shields and jewellery have.
#[derive(Debug, Clone, PartialEq)]
pub struct ArmorInfo {
    /// `armor_type`; `Shield` when Java derives it (no type and the left-hand body part).
    pub armor_type: ArmorType,
}

/// Java `EtcItem`: what only etc items have.
#[derive(Debug, Clone, PartialEq)]
pub struct EtcInfo {
    /// `etcitem_type`, or `Shot` for soulshot/spiritshot default actions.
    pub etc_type: EtcItemType,
    /// `handler`: the name of the item handler, `None` when absent.
    pub handler: Option<String>,
    /// `extractableCountMin`.
    pub extractable_count_min: i32,
    /// `extractableCountMax`.
    pub extractable_count_max: i32,
    /// `<capsuled_items>`: what the box can give, in document order. Empty is Java's `null`
    /// (`getExtractableItems()` before any `addCapsuledItem`).
    pub extractable_items: Vec<ExtractableProduct>,
    /// `blessed`, or a blessed soulshot/spiritshot by name.
    pub blessed: bool,
}

/// Which of Java's three item classes this is, with that class' own fields.
#[derive(Debug, Clone, PartialEq)]
pub enum ItemKind {
    /// Java `Weapon`.
    Weapon(WeaponInfo),
    /// Java `Armor`.
    Armor(ArmorInfo),
    /// Java `EtcItem`.
    Etc(EtcInfo),
}

/// Java `ItemTemplate` (and its three subclasses, see [`ItemKind`]).
#[derive(Debug, Clone, PartialEq)]
pub struct ItemTemplate {
    /// `item_id`.
    pub id: i32,
    /// `displayId`: the id shown to the client; the item id when absent.
    pub display_id: i32,
    /// Display name.
    pub name: String,
    /// `icon`, e.g. `icon.etc_adena_i00`.
    pub icon: Option<String>,
    /// `weight`.
    pub weight: i32,
    /// `is_stackable`.
    pub stackable: bool,
    /// `material`, default steel.
    pub material: MaterialType,
    /// `crystal_type`.
    pub crystal_type: CrystalType,
    /// `crystal_count`.
    pub crystal_count: i32,
    /// `equip_reuse_delay` in milliseconds (the XML has seconds).
    pub equip_reuse_delay: i32,
    /// `duration` (mana for shadow items), -1 when absent.
    pub duration: i32,
    /// `time` (lifetime in minutes), -1 when absent.
    pub time: i64,
    /// `auto_destroy_time` in milliseconds (the XML has seconds); -1000 when absent.
    pub auto_destroy_time: i32,
    /// `bodypart`.
    pub body_part: BodyPart,
    /// `price`: the reference (shop) price.
    pub price: i32,
    /// `is_sellable`, default true.
    pub sellable: bool,
    /// `is_dropable`, default true.
    pub dropable: bool,
    /// `is_destroyable`, default true.
    pub destroyable: bool,
    /// `is_tradable`, default true.
    pub tradeable: bool,
    /// `is_depositable`, default true.
    pub depositable: bool,
    /// `enchant_enabled`. Java's `isEnchantable()` also checks a config blacklist.
    pub enchantable: bool,
    /// `element_enabled`.
    pub elementable: bool,
    /// `is_questitem`.
    pub quest_item: bool,
    /// `is_freightable`.
    pub freightable: bool,
    /// `allow_self_resurrection`.
    pub allow_self_resurrection: bool,
    /// `is_oly_restricted`. Java's `isOlyRestrictedItem()` also checks a config list.
    pub oly_restricted: bool,
    /// `for_npc`.
    pub for_npc: bool,
    /// `immediate_effect`.
    pub immediate_effect: bool,
    /// `ex_immediate_effect`.
    pub ex_immediate_effect: bool,
    /// `enchanted`: the enchant level the item is created with.
    pub default_enchant_level: i32,
    /// `default_action`.
    pub default_action: ActionType,
    /// `useSkillDisTime`.
    pub use_skill_dis_time: i32,
    /// `reuse_delay`; weapons that are bows or crossbows get 1500 when it is 0.
    pub reuse_delay: i32,
    /// `shared_reuse_group`.
    pub shared_reuse_group: i32,
    /// Java `getType1()`: the first item type sent in `ItemList`.
    pub type1: i32,
    /// Java `getType2()`: the second item type sent in `ItemList`.
    pub type2: i32,
    /// The class-specific part.
    pub kind: ItemKind,
    /// Java `_funcTemplates`: the stat functions worn/wielded by this item, in Java's order of
    /// attachment (`<stats>` in document order, then the automatic enchant functions).
    pub funcs: Vec<FuncTemplate>,
    /// Java `_elementals`: the base elements that elemental stats set (`fireRes`, `waterPower`, ...).
    pub elementals: Vec<ElementalValue>,
    /// Java `_preConditions`: the `<conditions>` blocks that must hold to use/equip the item.
    /// Blocks without a recognised condition are left out (Java stores `null` and skips it).
    pub conditions: Vec<MessagedCondition>,
    /// The skills the item carries (Java `getSkills`, `getUnequipSkill`, enchant4/on-crit/on-magic).
    pub skills: ItemSkills,
    /// Java keeps these in `EnchantItemOptionsData`: the option ids (0 = none) of each enchant level
    /// of this item, which `Item.getEnchantOptions()` reads for the item packets and
    /// `applyEnchantStats`. Filled by [`ItemTemplates::attach_enchant_options`] after both tables
    /// are loaded, so the packet code (which only has the template) can reach them.
    pub enchant_options: BTreeMap<i32, [i32; 3]>,
}

impl ItemTemplate {
    /// Java `Item.getEnchantOptions()` for this template at `enchant_level`: the three option ids,
    /// `[0, 0, 0]` (`DEFAULT_ENCHANT_OPTIONS`) for a level without options.
    pub fn enchant_options_at(&self, enchant_level: i32) -> [i32; 3] {
        self.enchant_options
            .get(&enchant_level)
            .copied()
            .unwrap_or([0; 3])
    }

    /// Java `isWeapon()`.
    pub fn is_weapon(&self) -> bool {
        matches!(self.kind, ItemKind::Weapon(_))
    }

    /// Java `isArmor()`.
    pub fn is_armor(&self) -> bool {
        matches!(self.kind, ItemKind::Armor(_))
    }

    /// Java `isEtcItem()`.
    pub fn is_etc_item(&self) -> bool {
        matches!(self.kind, ItemKind::Etc(_))
    }

    /// The weapon part, if this is a weapon.
    pub fn weapon(&self) -> Option<&WeaponInfo> {
        match &self.kind {
            ItemKind::Weapon(w) => Some(w),
            _ => None,
        }
    }

    /// The armor part, if this is armor.
    pub fn armor(&self) -> Option<&ArmorInfo> {
        match &self.kind {
            ItemKind::Armor(a) => Some(a),
            _ => None,
        }
    }

    /// The etc part, if this is an etc item.
    pub fn etc(&self) -> Option<&EtcInfo> {
        match &self.kind {
            ItemKind::Etc(e) => Some(e),
            _ => None,
        }
    }

    /// Java `isMagicWeapon()`.
    pub fn is_magic_weapon(&self) -> bool {
        self.weapon().is_some_and(|w| w.magic_weapon)
    }

    /// Java `getItemMask()`: the item-type bit used by conditions and shops.
    pub fn item_mask(&self) -> i32 {
        match &self.kind {
            ItemKind::Weapon(w) => w.weapon_type.mask(),
            ItemKind::Armor(a) => a.armor_type.mask(),
            ItemKind::Etc(e) => e.etc_type.mask(),
        }
    }

    /// Java `isEquipable()`: has a body part and is not an etc item.
    pub fn is_equipable(&self) -> bool {
        self.body_part != BodyPart::None && !self.is_etc_item()
    }

    /// Java `isCrystallizable()`.
    pub fn is_crystallizable(&self) -> bool {
        self.crystal_type != CrystalType::None && self.crystal_count > 0
    }

    /// Java `getItemGrade()`.
    pub fn item_grade(&self) -> ItemGrade {
        self.crystal_type.item_grade()
    }

    /// Java `getCrystalTypePlus()`: S80 and S84 count as S.
    pub fn crystal_type_plus(&self) -> CrystalType {
        self.crystal_type.plus()
    }

    /// Java `getCrystalItemId()`.
    pub fn crystal_item_id(&self) -> i32 {
        self.crystal_type.crystal_id()
    }

    /// Java `getCrystalCount(enchantLevel)`: crystals gained when crystallizing at that enchant.
    pub fn crystal_count_at(&self, enchant_level: i32) -> i32 {
        let armor_bonus = self.crystal_type.crystal_enchant_bonus_armor();
        let weapon_bonus = self.crystal_type.crystal_enchant_bonus_weapon();
        if enchant_level > 3 {
            return match self.type2 {
                TYPE2_SHIELD_ARMOR | TYPE2_ACCESSORY => {
                    self.crystal_count + armor_bonus * (3 * enchant_level - 6)
                }
                TYPE2_WEAPON => self.crystal_count + weapon_bonus * (2 * enchant_level - 3),
                _ => self.crystal_count,
            };
        }
        if enchant_level <= 0 {
            return self.crystal_count;
        }
        match self.type2 {
            TYPE2_SHIELD_ARMOR | TYPE2_ACCESSORY => self.crystal_count + armor_bonus * enchant_level,
            TYPE2_WEAPON => self.crystal_count + weapon_bonus * enchant_level,
            _ => self.crystal_count,
        }
    }

    /// Java `isCommon()`.
    pub fn is_common(&self) -> bool {
        (11605..=12361).contains(&self.id)
    }

    /// Java `isHeroItem()`.
    pub fn is_hero_item(&self) -> bool {
        (6611..=6621).contains(&self.id) || (9388..=9390).contains(&self.id) || self.id == 6842
    }

    /// Java `isPvpItem()`.
    pub fn is_pvp_item(&self) -> bool {
        const RANGES: [(i32, i32); 10] = [
            (10667, 10835),
            (12852, 12977),
            (14363, 14525),
            (15913, 16024),
            (16134, 16147),
            (16168, 16176),
            (16179, 16220),
            (21932, 21934),
            (21952, 21954),
            (21970, 21972),
        ];
        const SINGLES: [i32; 11] = [
            14528, 14529, 14558, 16149, 16151, 16153, 16155, 16157, 16159, 21938, 21958,
        ];
        RANGES.iter().any(|&(lo, hi)| (lo..=hi).contains(&self.id)) || SINGLES.contains(&self.id)
    }

    /// Java `isPotion()`.
    pub fn is_potion(&self) -> bool {
        self.etc().is_some_and(|e| e.etc_type == EtcItemType::Potion)
    }

    /// Java `isElixir()`.
    pub fn is_elixir(&self) -> bool {
        self.etc().is_some_and(|e| e.etc_type == EtcItemType::Elixir)
    }

    /// Java `isScroll()`.
    pub fn is_scroll(&self) -> bool {
        self.etc().is_some_and(|e| e.etc_type == EtcItemType::Scroll)
    }

    /// Java `isPetItem()`.
    pub fn is_pet_item(&self) -> bool {
        self.etc().is_some_and(|e| e.etc_type == EtcItemType::PetCollar)
    }

    /// Java `isConditionAttached()`.
    pub fn is_condition_attached(&self) -> bool {
        !self.conditions.is_empty()
    }

    /// Java `hasSkills()`: the item has an `item_skill` entry.
    pub fn has_skills(&self) -> bool {
        self.skills.item_skills.is_some()
    }

    /// Java `toString()`: `name(id)`, as used in Java's log lines.
    pub fn label(&self) -> String {
        format!("{}({})", self.name, self.id)
    }
}

/// All item templates, immutable after loading. Java `ItemData`.
#[derive(Debug, Default)]
pub struct ItemTemplates {
    items: BTreeMap<i32, ItemTemplate>,
}

impl ItemTemplates {
    const LOADER: &'static str = "ItemData";

    /// Loads every file in `<root>/data/stats/items` (not its subfolders) and logs Java's count
    /// lines. With `custom_items` (Java `GeneralConfig.CUSTOM_ITEMS_LOAD`) the files in
    /// `data/stats/items/custom` are loaded after them, so on a duplicate id the custom item wins.
    pub fn load(root: &Path, custom_items: bool) -> Self {
        let mut table = Self::default();
        let dirs =
            std::iter::once("data/stats/items").chain(custom_items.then_some("data/stats/items/custom"));
        for dir in dirs {
            // Java `parseDatapackDirectory`, spelled out here because the condition parser's log
            // lines name the file (`load_dir` does not hand the path to its closure).
            for path in xml_files(&root.join(dir)) {
                let file = path
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default();
                load_file(&path, |doc: RawItemFile| {
                    table.add_file(doc, &file);
                    Ok(())
                });
            }
        }
        table.log_counts();
        table
    }

    /// Builds a table from the text of one item file. For tests and tools.
    pub fn from_xml(xml: &str) -> Result<Self, DataError> {
        let doc = quick_xml::de::from_str(xml).map_err(|source| DataError::Xml {
            path: "<inline>".into(),
            source,
        })?;
        let mut table = Self::default();
        table.add_file(doc, "<inline>");
        Ok(table)
    }

    /// Java `DocumentItem.parseDocument` for one file.
    fn add_file(&mut self, doc: RawItemFile, file: &str) {
        // Java keeps the `<table>`s per file and only clears them after an item succeeded, so a
        // failed item leaves its tables for the next one.
        let mut tables = Tables::new();
        for raw in doc.items {
            let label = raw.id.clone().unwrap_or_default();
            match build_item(raw, &mut tables, file) {
                Ok(item) => {
                    tables.clear();
                    self.items.insert(item.id, item);
                }
                Err(error) => warn!("Cannot create item {label}: {error}"),
            }
        }
    }

    fn log_counts(&self) {
        info!("{}: Highest item id used: {}", Self::LOADER, self.highest_id());
        log_loaded(Self::LOADER, self.etc_item_count(), "etc items");
        log_loaded(Self::LOADER, self.armor_count(), "armor items");
        log_loaded(Self::LOADER, self.weapon_count(), "weapon items");
        log_loaded(Self::LOADER, self.len(), "items in total");
    }

    /// Hands the enchant options (`EnchantItemOptionsData`) to the templates they belong to.
    pub fn attach_enchant_options(&mut self, options: &crate::EnchantItemOptions) {
        for (item_id, levels) in options.iter() {
            if let Some(template) = self.items.get_mut(&item_id) {
                template.enchant_options = levels.clone();
            }
        }
    }

    /// Java `getTemplate(id)`: `None` for an unknown id.
    pub fn get(&self, id: i32) -> Option<&ItemTemplate> {
        self.items.get(&id)
    }

    /// All items, ordered by id.
    pub fn iter(&self) -> impl Iterator<Item = &ItemTemplate> {
        self.items.values()
    }

    /// Java `getAllWeapons()`.
    pub fn weapons(&self) -> impl Iterator<Item = &ItemTemplate> {
        self.iter().filter(|i| i.is_weapon())
    }

    /// Java `getAllArmors()`.
    pub fn armors(&self) -> impl Iterator<Item = &ItemTemplate> {
        self.iter().filter(|i| i.is_armor())
    }

    /// Java `getAllEtcItems()`.
    pub fn etc_items(&self) -> impl Iterator<Item = &ItemTemplate> {
        self.iter().filter(|i| i.is_etc_item())
    }

    /// Number of weapons.
    pub fn weapon_count(&self) -> usize {
        self.weapons().count()
    }

    /// Number of armor items (including shields and jewellery).
    pub fn armor_count(&self) -> usize {
        self.armors().count()
    }

    /// Number of etc items.
    pub fn etc_item_count(&self) -> usize {
        self.etc_items().count()
    }

    /// The highest item id (0 when empty). Java's `getArraySize() - 1`.
    pub fn highest_id(&self) -> i32 {
        self.items.keys().next_back().copied().unwrap_or(0)
    }

    /// Number of items.
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// True when nothing was loaded.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

/// The `.xml` files directly inside `dir`, sorted by path (same order and rules as
/// `loader::load_dir` without recursion).
fn xml_files(dir: &Path) -> Vec<PathBuf> {
    if !dir.is_dir() {
        warn!("Directory not found: {}", dir.display());
        return Vec::new();
    }
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(error) => {
            warn!("Cannot list {}: {error}", dir.display());
            return Vec::new();
        }
    };
    let mut files: Vec<PathBuf> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.is_file() && path.extension().is_some_and(|e| e.eq_ignore_ascii_case("xml")))
        .collect();
    files.sort();
    files
}

// ---- building one item from its <set>s (Java DocumentItem + the ItemTemplate constructors) ----

/// Java `DocumentBase._tables`: `#name` to its whitespace separated values.
type Tables = HashMap<String, Vec<String>>;

/// Java `StatSet` as far as items use it: every value is kept as text and parsed on read.
struct ItemSet(HashMap<String, String>);

impl ItemSet {
    fn string(&self, key: &str) -> Option<&str> {
        self.0.get(key).map(String::as_str)
    }

    /// Java `getInt(key, default)`: a present value that is not an integer is an error.
    fn int(&self, key: &str, default: i32) -> Result<i32, DataError> {
        match self.string(key) {
            None => Ok(default),
            Some(v) => v
                .parse()
                .map_err(|_| DataError::Invalid(format!("integer value required for {key}, found: {v}"))),
        }
    }

    /// Java `getBoolean(key, default)`: `Boolean.parseBoolean`, so anything but `true` is false.
    fn boolean(&self, key: &str, default: bool) -> bool {
        self.string(key)
            .map_or(default, |v| v.eq_ignore_ascii_case("true"))
    }

    /// Java `getEnum(key, class, default)`: exact name, an unknown name is an error.
    fn enumeration<T>(
        &self,
        key: &str,
        default: T,
        from_name: fn(&str) -> Option<T>,
    ) -> Result<T, DataError> {
        match self.string(key) {
            None => Ok(default),
            Some(v) => {
                from_name(v).ok_or_else(|| DataError::Invalid(format!("unknown value for {key}: {v}")))
            }
        }
    }
}

/// Java `String.split(regex)` for a plain delimiter: no match returns the input, trailing empty
/// strings are dropped.
fn java_split(text: &str, delimiter: char) -> Vec<&str> {
    if !text.contains(delimiter) {
        return vec![text];
    }
    let mut parts: Vec<&str> = text.split(delimiter).collect();
    while parts.last() == Some(&"") {
        parts.pop();
    }
    parts
}

/// Java `StringUtil.isNumeric`.
fn is_numeric(text: &str) -> bool {
    !text.is_empty() && text.chars().all(|c| c.is_ascii_digit())
}

fn parse_int(text: &str) -> Result<i32, DataError> {
    text.parse()
        .map_err(|_| DataError::Invalid(format!("integer value required, found: {text}")))
}

/// Java `parseBeanSet`: one `<set>`.
fn apply_set(raw: &RawSet, set: &mut ItemSet, tables: &Tables) -> Result<(), DataError> {
    let Some(name) = raw.name.as_deref() else {
        return Ok(());
    };
    // The `val` attribute (old form), otherwise the element text (new form).
    let value = raw.val.as_deref().or(raw.text.as_deref()).unwrap_or("").trim();
    if value.is_empty() {
        return Ok(());
    }
    let value = if value.starts_with('#') {
        // Java passes level 1, which is element 0 of the table.
        tables
            .get(value)
            .and_then(|table| table.first())
            .ok_or_else(|| DataError::Invalid(format!("unknown table {value}")))?
            .as_str()
    } else {
        value
    };
    set.0.insert(name.trim().to_owned(), value.to_owned());
    Ok(())
}

/// Java `parseTable`: `<table name="#x">1 2 3</table>`.
fn apply_table(raw: &RawTable, tables: &mut Tables) -> Result<(), DataError> {
    let name = raw
        .name
        .as_deref()
        .ok_or_else(|| DataError::Invalid("table without a name".into()))?;
    if !name.starts_with('#') {
        return Err(DataError::Invalid("Table name must start with #".into()));
    }
    let text = raw
        .text
        .as_deref()
        .ok_or_else(|| DataError::Invalid(format!("table {name} has no values")))?;
    tables.insert(
        name.to_owned(),
        text.split_whitespace().map(str::to_owned).collect(),
    );
    Ok(())
}

/// Java `DocumentItem.parseItem` + `makeItem`.
fn build_item(raw: RawItem, tables: &mut Tables, file: &str) -> Result<ItemTemplate, DataError> {
    let missing = |what: &str| DataError::Invalid(format!("item without {what}"));
    let id_text = raw.id.as_deref().ok_or_else(|| missing("id"))?;
    let item_id = parse_int(id_text.trim())?;
    let class = raw.class.as_deref().ok_or_else(|| missing("type"))?;
    let name = raw.name.as_deref().ok_or_else(|| missing("name"))?;

    let mut set = ItemSet(HashMap::new());
    set.0.insert("item_id".into(), item_id.to_string());
    set.0.insert("name".into(), name.to_owned());

    // Set to true by the first node that makes Java create the item (`stats`, `conditions`,
    // `capsuled_items`). After that, `set` and `table` nodes are an error.
    let mut created = false;
    for child in &raw.children {
        match child {
            RawChild::Set(s) => {
                if created {
                    return Err(DataError::Invalid(format!(
                        "Item created but set node found! Item {item_id}"
                    )));
                }
                apply_set(s, &mut set, tables)?;
            }
            RawChild::Table(t) => {
                if created {
                    return Err(DataError::Invalid(format!(
                        "Item created but table node found! Item {item_id}"
                    )));
                }
                apply_table(t, tables)?;
            }
            RawChild::Stats(_) | RawChild::Conditions(_) | RawChild::CapsuledItems(_) => created = true,
            // `<skills>` only writes `item_skill` & co. into the set; it does not create the item.
            RawChild::Skills(node) => skills::apply_skills_node(node, &mut set)?,
            RawChild::Other => {}
        }
    }

    let mut item = match class {
        "Weapon" | "Armor" | "EtcItem" => build_template(class, &set)?,
        other => return Err(DataError::Invalid(format!("unknown item class {other}"))),
    };
    item.skills = skills::build_skills(&set, class, &item.label())?;

    // Java attaches while walking the children, so keep the document order of `<stats>` and
    // `<conditions>` (they share nothing, but the order is the order of `funcs`).
    // Items have no `#table` values in conditions: `getValue(value, item)` throws for them.
    let parser = ConditionParser::new(file, &|_| None);
    for child in &raw.children {
        match child {
            RawChild::Stats(node) => funcs::apply_stats_node(&mut item, node)?,
            RawChild::Conditions(node) => {
                let block = parser
                    .parse_conditions_block(node)
                    .map_err(|error| DataError::Invalid(error.to_string()))?;
                // Java attaches a null condition too, and skips it when testing.
                item.conditions.extend(block);
            }
            RawChild::CapsuledItems(node) => {
                // `ItemTemplate.addCapsuledItem` does nothing for weapons and armor.
                let etc = match &mut item.kind {
                    ItemKind::Etc(etc) => Some(etc),
                    _ => None,
                };
                capsule::apply_capsuled_items(node, etc)?;
            }
            _ => {}
        }
    }
    Ok(item)
}

/// Java `ItemTemplate.set` followed by the subclass' `set`.
fn build_template(class: &str, set: &ItemSet) -> Result<ItemTemplate, DataError> {
    let id = set.int("item_id", 0)?;
    if id < 0 {
        // Java would fail later with an out of bounds index when filling its lookup array.
        return Err(DataError::Invalid(format!("negative item id {id}")));
    }
    let body_part_name = set.string("bodypart").unwrap_or("none");
    // Java's `fromName` returns null for an unknown name and fails later on first use; fail early.
    let body_part = BodyPart::from_name(body_part_name)
        .ok_or_else(|| DataError::Invalid(format!("unknown bodypart {body_part_name}")))?;
    let default_action = set.enumeration("default_action", ActionType::None, ActionType::from_name)?;

    let mut item = ItemTemplate {
        id,
        display_id: set.int("displayId", id)?,
        name: set.string("name").unwrap_or_default().to_owned(),
        icon: set.string("icon").map(str::to_owned),
        weight: set.int("weight", 0)?,
        stackable: set.boolean("is_stackable", false),
        material: set.enumeration("material", MaterialType::Steel, MaterialType::from_name)?,
        crystal_type: set.enumeration("crystal_type", CrystalType::None, CrystalType::from_name)?,
        crystal_count: set.int("crystal_count", 0)?,
        // Java int arithmetic wraps.
        equip_reuse_delay: set.int("equip_reuse_delay", 0)?.wrapping_mul(1000),
        duration: set.int("duration", -1)?,
        time: i64::from(set.int("time", -1)?),
        auto_destroy_time: set.int("auto_destroy_time", -1)?.wrapping_mul(1000),
        body_part,
        price: set.int("price", 0)?,
        sellable: set.boolean("is_sellable", true),
        dropable: set.boolean("is_dropable", true),
        destroyable: set.boolean("is_destroyable", true),
        tradeable: set.boolean("is_tradable", true),
        depositable: set.boolean("is_depositable", true),
        enchantable: set.boolean("enchant_enabled", false),
        elementable: set.boolean("element_enabled", false),
        quest_item: set.boolean("is_questitem", false),
        freightable: set.boolean("is_freightable", false),
        allow_self_resurrection: set.boolean("allow_self_resurrection", false),
        oly_restricted: set.boolean("is_oly_restricted", false),
        for_npc: set.boolean("for_npc", false),
        immediate_effect: set.boolean("immediate_effect", false),
        ex_immediate_effect: set.boolean("ex_immediate_effect", false),
        default_enchant_level: set.int("enchanted", 0)?,
        default_action,
        use_skill_dis_time: set.int("useSkillDisTime", 0)?,
        reuse_delay: set.int("reuse_delay", 0)?,
        shared_reuse_group: set.int("shared_reuse_group", 0)?,
        type1: 0,
        type2: 0,
        // Replaced below.
        kind: ItemKind::Armor(ArmorInfo {
            armor_type: ArmorType::None,
        }),
        funcs: Vec::new(),
        elementals: Vec::new(),
        conditions: Vec::new(),
        skills: ItemSkills::default(),
        enchant_options: BTreeMap::new(),
    };

    match class {
        "Weapon" => build_weapon(&mut item, set)?,
        "Armor" => build_armor(&mut item, set)?,
        _ => build_etc(&mut item, set)?,
    }
    Ok(item)
}

/// Java `Weapon.set`.
fn build_weapon(item: &mut ItemTemplate, set: &ItemSet) -> Result<(), DataError> {
    // `toUpperCase()` before `valueOf`, and the default is "none".
    let type_name = set.string("weapon_type").unwrap_or("none").to_uppercase();
    let weapon_type = WeaponType::from_name(&type_name)
        .ok_or_else(|| DataError::Invalid(format!("unknown weapon_type {type_name}")))?;
    item.type1 = TYPE1_WEAPON_RING_EARRING_NECKLACE;
    item.type2 = TYPE2_WEAPON;

    // "0?;0?;fan sector;base attack angle"
    let damage_range = java_split(set.string("damage_range").unwrap_or(""), ';');
    let (base_attack_radius, base_attack_angle) = if damage_range.len() > 1 {
        // NOTE(java-quirk): Java indexes [2] (and [3] only if [2] is numeric) as soon as there are
        // two parts, so a value with too few parts throws and the item is skipped.
        let part = |index: usize| {
            damage_range
                .get(index)
                .copied()
                .ok_or_else(|| DataError::Invalid("damage_range has fewer than 4 parts".into()))
        };
        let radius = part(2)?;
        if is_numeric(radius) && is_numeric(part(3)?) {
            (parse_int(radius)?, parse_int(part(3)?)?)
        } else {
            (40, 0)
        }
    } else {
        (40, 0)
    };

    // "chance,count"; anything but exactly two parts means none.
    let pair = |key: &str| -> Result<(i32, i32), DataError> {
        let parts = java_split(set.string(key).unwrap_or(""), ',');
        if parts.len() == 2 {
            Ok((parse_int(parts[0])?, parse_int(parts[1])?))
        } else {
            Ok((0, 0))
        }
    };
    let (reduced_soulshot_chance, reduced_soulshot) = pair("reduced_soulshot")?;
    let (reduced_mp_consume_chance, reduced_mp_consume) = pair("reduced_mp_consume")?;

    // Bows and crossbows without a reuse delay get 1500.
    if item.reuse_delay == 0 && weapon_type.is_ranged() {
        item.reuse_delay = 1500;
    }

    item.kind = ItemKind::Weapon(WeaponInfo {
        weapon_type,
        magic_weapon: set.boolean("is_magic_weapon", false),
        soulshot_count: set.int("soulshots", 0)?,
        spiritshot_count: set.int("spiritshots", 0)?,
        mp_consume: set.int("mp_consume", 0)?,
        base_attack_range: set.int("attack_range", 40)?,
        base_attack_radius,
        base_attack_angle,
        reduced_soulshot_chance,
        reduced_soulshot,
        reduced_mp_consume_chance,
        reduced_mp_consume,
        change_weapon_id: set.int("change_weaponId", 0)?,
        force_equip: set.boolean("isForceEquip", false),
        attack_weapon: set.boolean("isAttackWeapon", true),
        use_weapon_skills_only: set.boolean("useWeaponSkillsOnly", false),
    });
    Ok(())
}

/// Java `Armor.set`.
fn build_armor(item: &mut ItemTemplate, set: &ItemSet) -> Result<(), DataError> {
    let mut armor_type = set.enumeration("armor_type", ArmorType::None, ArmorType::from_name)?;
    match item.body_part {
        BodyPart::Neck
        | BodyPart::LeftEar
        | BodyPart::RightEar
        | BodyPart::BothEars
        | BodyPart::LeftFinger
        | BodyPart::RightFinger
        | BodyPart::BothFingers
        | BodyPart::RightBracelet
        | BodyPart::LeftBracelet => {
            item.type1 = TYPE1_WEAPON_RING_EARRING_NECKLACE;
            item.type2 = TYPE2_ACCESSORY;
        }
        body_part => {
            // Shields have no armor type in the XML: they are "no type" worn in the left hand.
            if armor_type == ArmorType::None && body_part == BodyPart::LeftHand {
                armor_type = ArmorType::Shield;
            }
            item.type1 = TYPE1_SHIELD_ARMOR;
            item.type2 = TYPE2_SHIELD_ARMOR;
        }
    }
    item.kind = ItemKind::Armor(ArmorInfo { armor_type });
    Ok(())
}

/// Java `EtcItem.set`.
fn build_etc(item: &mut ItemTemplate, set: &ItemSet) -> Result<(), DataError> {
    let mut etc_type = set.enumeration("etcitem_type", EtcItemType::None, EtcItemType::from_name)?;
    // l2j custom: shots get their own type.
    if matches!(
        item.default_action,
        ActionType::SoulShot
            | ActionType::SummonSoulShot
            | ActionType::SummonSpiritShot
            | ActionType::SpiritShot
    ) {
        etc_type = EtcItemType::Shot;
    }

    item.type1 = TYPE1_ITEM_QUESTITEM_ADENA;
    item.type2 = TYPE2_OTHER;
    if item.quest_item {
        item.type2 = TYPE2_QUEST;
    } else if item.id == ADENA_ID || item.id == ANCIENT_ADENA_ID {
        item.type2 = TYPE2_MONEY;
    }

    let extractable_count_min = set.int("extractableCountMin", 0)?;
    let extractable_count_max = set.int("extractableCountMax", 0)?;
    if extractable_count_min > extractable_count_max {
        warn!(
            "Item {}({}) extractableCountMin is bigger than extractableCountMax!",
            item.name, item.id
        );
    }

    let blessed_shot = matches!(item.default_action, ActionType::SpiritShot | ActionType::SoulShot)
        && item.name.contains("Blessed");
    item.kind = ItemKind::Etc(EtcInfo {
        etc_type,
        handler: set.string("handler").map(str::to_owned),
        extractable_count_min,
        extractable_count_max,
        extractable_items: Vec::new(),
        blessed: set.boolean("blessed", false) || blessed_shot,
    });
    Ok(())
}

// Raw XML shape (xsd/items.xsd). Everything is optional text here so that a broken item fails
// alone (like in Java) instead of failing the whole file.
#[derive(Deserialize)]
struct RawItemFile {
    #[serde(rename = "item", default)]
    items: Vec<RawItem>,
}

#[derive(Deserialize)]
struct RawItem {
    #[serde(rename = "@id")]
    id: Option<String>,
    #[serde(rename = "@type")]
    class: Option<String>,
    #[serde(rename = "@name")]
    name: Option<String>,
    // Document order matters (see `build_item`), hence one enum list instead of one field per element.
    #[serde(rename = "$value", default)]
    children: Vec<RawChild>,
}

#[derive(Deserialize)]
enum RawChild {
    #[serde(rename = "set")]
    Set(RawSet),
    #[serde(rename = "table")]
    Table(RawTable),
    // Free-form blocks: the stat and condition parsers walk the whole subtree.
    #[serde(rename = "stats")]
    Stats(XmlNode),
    #[serde(rename = "skills")]
    Skills(XmlNode),
    #[serde(rename = "conditions")]
    Conditions(XmlNode),
    #[serde(rename = "capsuled_items")]
    CapsuledItems(XmlNode),
    #[serde(other)]
    Other,
}

#[derive(Deserialize)]
struct RawSet {
    #[serde(rename = "@name")]
    name: Option<String>,
    #[serde(rename = "@val")]
    val: Option<String>,
    #[serde(rename = "$text")]
    text: Option<String>,
}

#[derive(Deserialize)]
struct RawTable {
    #[serde(rename = "@name")]
    name: Option<String>,
    #[serde(rename = "$text")]
    text: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn load(items: &str) -> ItemTemplates {
        ItemTemplates::from_xml(&format!(r#"<?xml version="1.0"?><list>{items}</list>"#)).unwrap()
    }

    #[test]
    fn weapon_fields_and_defaults() {
        let t = load(
            r#"<item id="1" type="Weapon" name="Short Sword">
                <set name="icon" val="icon.weapon_small_sword_i00" />
                <set name="default_action" val="EQUIP" />
                <set name="weapon_type" val="SWORD" />
                <set name="bodypart" val="rhand" />
                <set name="damage_range" val="0;0;40;120" />
                <set name="material" val="STEEL" />
                <set name="weight" val="1600" />
                <set name="price" val="590" />
                <set name="soulshots" val="1" />
                <stats><stat type="pAtk">8</stat></stats>
            </item>"#,
        );
        assert_eq!(t.len(), 1);
        let i = t.get(1).unwrap();
        assert_eq!((i.name.as_str(), i.weight, i.price), ("Short Sword", 1600, 590));
        assert_eq!(i.icon.as_deref(), Some("icon.weapon_small_sword_i00"));
        assert_eq!(i.body_part.mask(), 0x80);
        assert_eq!((i.type1, i.type2), (0, 0));
        assert_eq!(i.default_action, ActionType::Equip);
        let w = i.weapon().unwrap();
        assert_eq!(w.weapon_type, WeaponType::Sword);
        assert_eq!((w.base_attack_radius, w.base_attack_angle), (40, 120));
        assert_eq!(
            (w.base_attack_range, w.soulshot_count, w.spiritshot_count),
            (40, 1, 0)
        );
        assert!(w.attack_weapon);
        assert_eq!(i.item_mask(), 1);
        // Defaults
        assert!(i.sellable && i.dropable && i.destroyable && i.tradeable && i.depositable);
        assert!(!i.stackable && !i.quest_item && !i.enchantable);
        assert_eq!((i.duration, i.time, i.auto_destroy_time), (-1, -1, -1000));
        assert_eq!(i.display_id, 1);
        assert_eq!(i.crystal_type, CrystalType::None);
    }

    #[test]
    fn etc_item_adena_and_quest_item_types() {
        let t = load(
            r#"<item id="57" type="EtcItem" name="Adena">
                <set name="material" val="GOLD" />
                <set name="price" val="1" />
                <set name="is_sellable" val="false" />
                <set name="is_stackable" val="true" />
            </item>
            <item id="1000" type="EtcItem" name="Quest Thing">
                <set name="is_questitem" val="true" />
            </item>
            <item id="1001" type="EtcItem" name="Thing" />"#,
        );
        let adena = t.get(57).unwrap();
        assert_eq!((adena.type1, adena.type2), (4, 4));
        assert!(adena.stackable && !adena.sellable);
        assert_eq!(adena.material, MaterialType::Gold);
        assert_eq!(t.get(1000).unwrap().type2, TYPE2_QUEST);
        assert_eq!(t.get(1001).unwrap().type2, TYPE2_OTHER);
        assert_eq!(t.get(1001).unwrap().item_mask(), 0);
    }

    #[test]
    fn armor_types_follow_the_body_part() {
        let t = load(
            r#"<item id="1" type="Armor" name="Shield"><set name="bodypart" val="lhand" /></item>
            <item id="2" type="Armor" name="Ring"><set name="bodypart" val="rfinger;lfinger" /></item>
            <item id="3" type="Armor" name="Plate"><set name="armor_type" val="HEAVY" /><set name="bodypart" val="chest" /></item>
            <item id="4" type="Armor" name="Bracelet"><set name="bodypart" val="rbracelet" /></item>"#,
        );
        assert_eq!(t.get(1).unwrap().armor().unwrap().armor_type, ArmorType::Shield);
        assert_eq!((t.get(1).unwrap().type1, t.get(1).unwrap().type2), (1, 1));
        assert_eq!((t.get(2).unwrap().type1, t.get(2).unwrap().type2), (0, 2));
        assert_eq!(t.get(2).unwrap().body_part.mask(), 0x30);
        assert_eq!(t.get(3).unwrap().armor().unwrap().armor_type, ArmorType::Heavy);
        assert_eq!(t.get(3).unwrap().item_mask(), 1 << 19);
        assert_eq!(t.get(4).unwrap().type2, TYPE2_ACCESSORY);
    }

    #[test]
    fn shots_get_the_shot_type_and_blessed_by_name() {
        let t = load(
            r#"<item id="1" type="EtcItem" name="Blessed Spiritshot: D-grade">
                <set name="default_action" val="SPIRITSHOT" /></item>
            <item id="2" type="EtcItem" name="Soulshot: D-grade"><set name="default_action" val="SOULSHOT" /></item>
            <item id="3" type="EtcItem" name="Potion"><set name="blessed" val="true" /></item>"#,
        );
        let a = t.get(1).unwrap().etc().unwrap();
        assert_eq!(a.etc_type, EtcItemType::Shot);
        assert!(a.blessed);
        assert!(!t.get(2).unwrap().etc().unwrap().blessed);
        assert!(t.get(3).unwrap().etc().unwrap().blessed);
    }

    #[test]
    fn tables_are_resolved_at_index_zero() {
        let t = load(
            r##"<item id="1" type="EtcItem" name="Tabled">
                <table name="#w"> 7 8 9 </table>
                <set name="weight" val="#w" />
            </item>
            <item id="2" type="EtcItem" name="Other"><set name="weight" val="3" /></item>"##,
        );
        assert_eq!(t.get(1).unwrap().weight, 7);
        assert_eq!(t.get(2).unwrap().weight, 3);
    }

    #[test]
    fn set_text_form_and_later_set_wins() {
        let t = load(
            r#"<item id="1" type="EtcItem" name="A">
                <set name="weight">  12 </set>
                <set name="price" val="5" />
                <set name="price" val="6" />
                <set name="name" val="Renamed" />
                <set name="empty" val="" />
            </item>"#,
        );
        let i = t.get(1).unwrap();
        assert_eq!((i.weight, i.price), (12, 6));
        assert_eq!(i.name, "Renamed");
    }

    #[test]
    fn broken_items_are_skipped_and_the_rest_loads() {
        let t = load(
            r##"<item id="1" type="EtcItem" name="ok" />
            <item id="2" type="EtcItem" name="bad int"><set name="weight" val="abc" /></item>
            <item id="3" type="EtcItem" name="bad enum"><set name="material" val="steel" /></item>
            <item id="4" type="Nothing" name="bad class" />
            <item id="5" type="EtcItem" name="bad table"><set name="weight" val="#nope" /></item>
            <item id="6" type="Weapon" name="bad bodypart"><set name="bodypart" val="foot" /></item>
            <item id="7" type="Weapon" name="bad weapon_type"><set name="weapon_type" val="spoon" /></item>
            <item id="x" type="EtcItem" name="bad id" />
            <item type="EtcItem" name="no id" />
            <item id="8" type="EtcItem" name="set after stats"><stats/><set name="weight" val="1" /></item>
            <item id="9" type="EtcItem" name="ok too" />"##,
        );
        let ids: Vec<i32> = t.iter().map(|i| i.id).collect();
        assert_eq!(ids, vec![1, 9]);
    }

    #[test]
    fn stats_skills_conditions_and_capsules_are_parsed_without_errors() {
        let t = load(
            r#"<item id="1" type="Armor" name="Fancy">
                <set name="bodypart" val="chest" />
                <skills><skill id="1" level="1" /><skill id="2" level="2" type="ON_ENCHANT_4" /></skills>
                <stats><stat type="pDef">10.5</stat></stats>
                <conditions msgId="1518" addName="1"><player races="HUMAN,ELF" /><and><player level="40"/></and></conditions>
                <capsuled_items><item id="57" min="1" max="2" chance="50" /></capsuled_items>
                <unknown a="b">text</unknown>
            </item>"#,
        );
        assert_eq!(t.get(1).unwrap().body_part, BodyPart::Chest);
        // The <item> inside capsuled_items must not be mistaken for an item.
        assert_eq!(t.len(), 1);
    }

    #[test]
    fn capsuled_items_of_an_etc_item_are_kept() {
        let t = load(
            r#"<item id="1" type="EtcItem" name="Box">
                <set name="extractableCountMin" val="1" /><set name="extractableCountMax" val="2" />
                <capsuled_items>
                    <item id="57" min="1" max="2" chance="50" />
                    <item id="1060" min="5" max="5" chance="0.5" minEnchant="1" maxEnchant="2" />
                </capsuled_items>
            </item>
            <item id="2" type="EtcItem" name="Empty"><capsuled_items /></item>
            <item id="3" type="EtcItem" name="Broken"><capsuled_items><item id="57" min="1" max="2" /></capsuled_items></item>"#,
        );
        let etc = t.get(1).unwrap().etc().unwrap();
        assert_eq!((etc.extractable_count_min, etc.extractable_count_max), (1, 2));
        assert_eq!(etc.extractable_items.len(), 2);
        assert_eq!(etc.extractable_items[0].chance, 50_000);
        assert_eq!(etc.extractable_items[1].chance, 500);
        assert_eq!(
            (
                etc.extractable_items[1].min_enchant,
                etc.extractable_items[1].max_enchant
            ),
            (1, 2)
        );
        assert!(t.get(2).unwrap().etc().unwrap().extractable_items.is_empty());
        // Java unboxes the missing `chance`: the item fails.
        assert!(t.get(3).is_none());
    }

    #[test]
    fn xml_entities_are_decoded() {
        let t = load(r#"<item id="1" type="EtcItem" name="Dragon&apos;s &amp; Co" />"#);
        assert_eq!(t.get(1).unwrap().name, "Dragon's & Co");
    }

    #[test]
    fn weapon_special_values() {
        let t = load(
            r#"<item id="1" type="Weapon" name="Bow"><set name="weapon_type" val="bow" /></item>
            <item id="2" type="Weapon" name="Bow 2"><set name="weapon_type" val="BOW" /><set name="reuse_delay" val="700" /></item>
            <item id="3" type="Weapon" name="Reduced">
                <set name="reduced_soulshot" val="59,7" /><set name="reduced_mp_consume" val="42,2" />
                <set name="damage_range" val="0;0;10;0" /></item>
            <item id="4" type="Weapon" name="Short range"><set name="damage_range" val="0;0" /></item>
            <item id="5" type="Weapon" name="Odd"><set name="reduced_soulshot" val="1,2,3" />
                <set name="damage_range" val="0;0;x;y" /></item>"#,
        );
        // weapon_type is upper-cased by Java, and bows get a default reuse delay.
        assert_eq!(t.get(1).unwrap().weapon().unwrap().weapon_type, WeaponType::Bow);
        assert_eq!(t.get(1).unwrap().reuse_delay, 1500);
        assert_eq!(t.get(2).unwrap().reuse_delay, 700);
        let r = t.get(3).unwrap().weapon().unwrap();
        assert_eq!((r.reduced_soulshot_chance, r.reduced_soulshot), (59, 7));
        assert_eq!((r.reduced_mp_consume_chance, r.reduced_mp_consume), (42, 2));
        assert_eq!((r.base_attack_radius, r.base_attack_angle), (10, 0));
        // NOTE(java-quirk): two parts, but Java reads index 2: the item fails.
        assert!(t.get(4).is_none());
        let odd = t.get(5).unwrap().weapon().unwrap();
        assert_eq!(
            (
                odd.reduced_soulshot,
                odd.base_attack_radius,
                odd.base_attack_angle
            ),
            (0, 40, 0)
        );
    }

    #[test]
    fn booleans_use_parse_boolean() {
        let t = load(
            r#"<item id="1" type="EtcItem" name="A"><set name="is_stackable" val="TRUE" />
                <set name="is_questitem" val="1" /><set name="is_sellable" val="yes" /></item>"#,
        );
        let i = t.get(1).unwrap();
        assert!(i.stackable);
        assert!(!i.quest_item);
        // A present but non-"true" value is false, even when the default is true.
        assert!(!i.sellable);
    }

    #[test]
    fn crystal_count_by_enchant() {
        let t = load(
            r#"<item id="1" type="Weapon" name="W"><set name="crystal_type" val="B" /><set name="crystal_count" val="100" /></item>
            <item id="2" type="Armor" name="A"><set name="crystal_type" val="S84" /><set name="crystal_count" val="100" /><set name="bodypart" val="chest" /></item>"#,
        );
        let w = t.get(1).unwrap();
        assert!(w.is_crystallizable());
        assert_eq!(w.crystal_count_at(0), 100);
        assert_eq!(w.crystal_count_at(2), 100 + 67 * 2);
        assert_eq!(w.crystal_count_at(5), 100 + 67 * 7);
        let a = t.get(2).unwrap();
        assert_eq!(a.crystal_count_at(3), 100 + 25 * 3);
        assert_eq!(a.crystal_count_at(4), 100 + 25 * 6);
        assert_eq!(a.crystal_type_plus(), CrystalType::S);
        assert_eq!(a.crystal_item_id(), 1462);
    }

    #[test]
    fn id_based_flags() {
        let t = load(
            r#"<item id="6611" type="Weapon" name="Hero" /><item id="11605" type="EtcItem" name="Common" />
            <item id="14528" type="EtcItem" name="Pvp" /><item id="14530" type="EtcItem" name="Not" />"#,
        );
        assert!(t.get(6611).unwrap().is_hero_item());
        assert!(t.get(11605).unwrap().is_common());
        assert!(t.get(14528).unwrap().is_pvp_item());
        assert!(!t.get(14530).unwrap().is_pvp_item());
    }

    #[test]
    fn java_split_matches_java() {
        assert_eq!(java_split("", ';'), vec![""]);
        assert_eq!(java_split("a;b;;", ';'), vec!["a", "b"]);
        assert_eq!(java_split(";", ';'), Vec::<&str>::new());
        assert_eq!(java_split("a", ';'), vec!["a"]);
    }
}
