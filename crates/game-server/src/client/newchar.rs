//! Everything a new character is made of, worked out without touching the database: the stat
//! numbers, the starting items and what is equipped, the skills, the shortcut bar and macros.
// Java: org/l2jmobius/gameserver/network/clientpackets/CharacterCreate.java (initNewChar)
// Java: org/l2jmobius/gameserver/entity/actor/Player.java (create, createDb, storeMe, deleteMe)
// Java: entity/itemcontainer/{ItemContainer,Inventory,PlayerInventory}.java (addItem, equipItem,
// Java: setPaperdollItem), managers/ItemManager.java (createItem), entity/item/instance/Item.java
// Java: mechanics/stats/{Formulas,BaseStat}.java + functions/formulas/FuncMax{Hp,Mp,Cp}Mul.java
// Java: data/xml/{InitialShortcutData,SkillTreeData}.java,
// Java: entity/actor/holders/player/{Shortcuts,MacroList}.java
//
// Java builds the character in memory and writes it in pieces while it does (INSERT characters,
// then an item row whenever an item is equipped, a skill row per `addSkill`, a shortcut row per
// `registerShortcut`, and at the end `storeMe` + `deleteMe` rewrite the rest). The Java game
// server only ever reads the final state back, so this module computes the final state and
// [`NewCharacterPlan`] lists the rows once, in the order Java first writes them. Statements that
// Java runs but that cannot change anything for a character that has just been given a fresh
// object id (the `DELETE`s of `storeEffect` / `storeItemReuseDelay`, `UPDATE items` with the
// values just inserted, the Seven Signs and variables stores) are not repeated.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::Path;

use l2_config::GameConfig;
use l2_data::item::{ADENA_ID, BodyPart, EtcItemType, WeaponType};
use l2_data::{
    ClassId, Datapack, ItemTemplate, MacroCommand, MacroPreset, MacroType, PlayerTemplate, Race, ShortcutType,
};
use l2_db::{
    CalendarDate, CharacterUpdate, ItemLocation, NewCharacter, NewItem, NewMacro, NewShortcut, SkillLevel,
};
use l2_protocol_game::server::charscreen::paperdoll as slot;
use l2_util::WorldRng;
use quick_xml::events::{BytesStart, Event};
use quick_xml::{Reader, XmlVersion};
use tracing::{info, warn};

/// `PlayerStat.MAX_VITALITY_POINTS` (`VITALITY_LEVELS[4]`).
pub const MAX_VITALITY_POINTS: i32 = 20000;
/// `PlayerStat.MIN_VITALITY_POINTS`.
pub const MIN_VITALITY_POINTS: i32 = 1;
/// `Shortcuts.MAX_SHORTCUTS_PER_BAR`.
const MAX_SHORTCUTS_PER_BAR: i32 = 12;
/// `Player.create`: "Give 20 recommendations."
const STARTING_RECOMMENDATIONS: i32 = 20;
/// The width of Java's `BaseStat` bonus arrays.
const MAX_STAT_VALUE: usize = 100;

// ---- stat tables -----------------------------------------------------------------------------

/// The `CON` and `MEN` columns of `data/stats/statBonus.xml` (`BaseStat`), which give max HP/CP and
/// max MP their multiplier.
#[derive(Debug, Clone, PartialEq)]
pub struct StatBonus {
    con: Vec<f64>,
    men: Vec<f64>,
}

impl Default for StatBonus {
    /// Java's arrays start at 0.0 and the file fills them in.
    fn default() -> Self {
        Self {
            con: vec![0.0; MAX_STAT_VALUE],
            men: vec![0.0; MAX_STAT_VALUE],
        }
    }
}

impl StatBonus {
    /// Reads `<root>/data/stats/statBonus.xml`. Java throws an `Error` when the file is missing
    /// (the server does not start); here the table stays at zero and the problem is logged.
    pub fn load(root: &Path) -> Self {
        let path = root.join("data/stats/statBonus.xml");
        match std::fs::read(&path) {
            Ok(bytes) => Self::from_xml(&String::from_utf8_lossy(&bytes)),
            Err(e) => {
                warn!("[BaseStats] Could not read {}: {e}", path.display());
                Self::default()
            }
        }
    }

    /// Parses the file text. A broken value is skipped like Java's "Invalid stats value".
    pub fn from_xml(xml: &str) -> Self {
        let mut table = Self::default();
        let mut reader = Reader::from_str(xml);
        let mut section: Option<String> = None;
        loop {
            match reader.read_event() {
                Ok(Event::Start(e)) => {
                    let name = e.name().as_ref().to_owned();
                    if name != "list" && name != "stat" {
                        section = Some(name);
                    }
                }
                Ok(Event::End(e)) => {
                    if section.as_deref().is_some_and(|s| s == e.name().as_ref()) {
                        section = None;
                    }
                }
                Ok(Event::Empty(e)) if e.name().as_ref() == "stat" => {
                    let (Some(value), Some(bonus)) = (attribute(&e, "value"), attribute(&e, "bonus")) else {
                        continue;
                    };
                    let (Ok(value), Ok(bonus)) = (value.parse::<usize>(), bonus.parse::<f64>()) else {
                        warn!("[BaseStats] Invalid stats value, skipping");
                        continue;
                    };
                    let target = match section.as_deref() {
                        Some(s) if s.eq_ignore_ascii_case("CON") => Some(&mut table.con),
                        Some(s) if s.eq_ignore_ascii_case("MEN") => Some(&mut table.men),
                        _ => None,
                    };
                    if let Some(slot) = target.and_then(|t| t.get_mut(value)) {
                        *slot = bonus;
                    }
                }
                Ok(Event::Eof) => break,
                Ok(_) => {}
                Err(e) => {
                    warn!("[BaseStats] Could not parse statBonus.xml: {e}");
                    break;
                }
            }
        }
        table
    }

    /// Java `BaseStat.CON.calcBonus` for a creature with that CON.
    pub fn con_bonus(&self, con: i32) -> f64 {
        usize::try_from(con)
            .ok()
            .and_then(|c| self.con.get(c).copied())
            .unwrap_or(0.0)
    }

    /// Java `BaseStat.MEN.calcBonus`.
    pub fn men_bonus(&self, men: i32) -> f64 {
        usize::try_from(men)
            .ok()
            .and_then(|c| self.men.get(c).copied())
            .unwrap_or(0.0)
    }
}

/// What the equipped starting items add to max MP: `<stats><stat type="maxMp">` of the item
/// files (`DocumentItem` makes it an `add` function of order 0, which runs before
/// `FuncMaxMpMul`). Nothing else of an item's `<stats>` touches max HP, MP or CP in Java.
// Java: util/DocumentItem.java (the `stats` branch)
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ItemMaxMp {
    add: HashMap<i32, f64>,
}

impl ItemMaxMp {
    /// Scans the item files below `<root>/data/stats/items` (and `custom/` when `custom_items`)
    /// for the `maxMp` of the items in `wanted`. A later definition of an id replaces an earlier
    /// one, like the item table does.
    pub fn load(root: &Path, wanted: &HashSet<i32>, custom_items: bool) -> Self {
        let mut table = Self::default();
        let dirs =
            std::iter::once("data/stats/items").chain(custom_items.then_some("data/stats/items/custom"));
        for dir in dirs {
            let Ok(entries) = std::fs::read_dir(root.join(dir)) else {
                continue;
            };
            let mut files: Vec<_> = entries
                .flatten()
                .map(|e| e.path())
                .filter(|p| p.is_file() && p.extension().is_some_and(|x| x == "xml"))
                .collect();
            files.sort();
            for file in files {
                match std::fs::read(&file) {
                    Ok(bytes) => table.scan(&String::from_utf8_lossy(&bytes), wanted),
                    Err(e) => warn!("Could not read file {}: {e}", file.display()),
                }
            }
        }
        table
    }

    /// Scans the text of one item file.
    pub fn scan(&mut self, xml: &str, wanted: &HashSet<i32>) {
        let mut reader = Reader::from_str(xml);
        let mut current: Option<i32> = None;
        let mut in_stats = false;
        let mut max_mp_stat = false;
        loop {
            match reader.read_event() {
                Ok(Event::Start(e)) => match e.name().as_ref() {
                    "item" => {
                        current = attribute(&e, "id")
                            .and_then(|id| id.parse().ok())
                            .filter(|id| wanted.contains(id));
                        if let Some(id) = current {
                            self.add.remove(&id);
                        }
                    }
                    "stats" => in_stats = true,
                    "stat" if in_stats => max_mp_stat = attribute(&e, "type").as_deref() == Some("maxMp"),
                    _ => {}
                },
                Ok(Event::Text(t)) if max_mp_stat => {
                    if let Some(id) = current
                        && let Ok(value) = t.xml10_content().trim().parse::<f64>()
                    {
                        *self.add.entry(id).or_insert(0.0) += value;
                    }
                }
                Ok(Event::End(e)) => match e.name().as_ref() {
                    "item" => {
                        current = None;
                        in_stats = false;
                        max_mp_stat = false;
                    }
                    "stats" => in_stats = false,
                    "stat" => max_mp_stat = false,
                    _ => {}
                },
                Ok(Event::Eof) | Err(_) => break,
                Ok(_) => {}
            }
        }
    }

    /// The MP the item adds while equipped.
    pub fn max_mp_of(&self, item_id: i32) -> f64 {
        self.add.get(&item_id).copied().unwrap_or(0.0)
    }

    /// Every item id the starting equipment of all classes mentions.
    pub fn wanted_ids(datapack: &Datapack) -> HashSet<i32> {
        datapack
            .player_templates
            .iter()
            .filter_map(|t| datapack.initial_equipment.get(t.class_id))
            .flatten()
            .map(|e| e.item_id)
            .collect()
    }
}

fn attribute(element: &BytesStart<'_>, key: &str) -> Option<String> {
    element
        .attributes()
        .flatten()
        .find(|attr| attr.key.as_ref() == key)
        .and_then(|attr| {
            attr.normalized_value(XmlVersion::Implicit1_0)
                .ok()
                .map(std::borrow::Cow::into_owned)
        })
}

/// Java `getMaxHp()` and friends for a player without effects: `(int) ((base + adds) * bonus)`,
/// where a result of zero or less becomes 1 (`CreatureStat.calcStat`).
fn calc_max(base: f32, adds: f64, bonus: f64) -> i32 {
    let value = (f64::from(base) + adds) * bonus;
    let value = if value <= 0.0 { 1.0 } else { value };
    value as i32
}

// ---- the plan --------------------------------------------------------------------------------

/// What the client asked for in `CharacterCreate` (after the checks of the packet handler).
#[derive(Debug, Clone, Copy)]
pub struct CreateRequest<'a> {
    pub account: &'a str,
    pub name: &'a str,
    pub is_female: bool,
    pub hair_style: i8,
    pub hair_color: i8,
    pub face: i8,
}

/// Everything read-only the plan needs.
#[derive(Clone, Copy)]
pub struct PlanContext<'a> {
    pub datapack: &'a Datapack,
    pub config: &'a GameConfig,
    pub stat_bonus: &'a StatBonus,
    pub item_max_mp: &'a ItemMaxMp,
}

/// Hands out object ids and remembers them, so a failed creation can give them back.
pub struct IdSource<'a> {
    allocate: &'a (dyn Fn() -> Option<i32> + Sync),
    /// Every id handed out so far.
    pub taken: Vec<i32>,
}

impl<'a> IdSource<'a> {
    pub fn new(allocate: &'a (dyn Fn() -> Option<i32> + Sync)) -> Self {
        Self {
            allocate,
            taken: Vec::new(),
        }
    }

    fn next(&mut self) -> Option<i32> {
        let id = (self.allocate)()?;
        self.taken.push(id);
        Some(id)
    }
}

/// Why no plan could be made.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum PlanError {
    /// The id range is exhausted.
    #[error("no object id left for the new character")]
    OutOfIds,
    /// The template has no HP/MP/CP row for the level (Java throws).
    #[error("the class template has no stat row for level {0}")]
    NoStatRow(i32),
}

/// The rows of a new character. Written in this order: `insert`, `items`, `skills`, `macros`,
/// `shortcuts`, `update`, then `last_access` (the offline flag) and `reco`.
#[derive(Debug, Clone, PartialEq)]
pub struct NewCharacterPlan {
    pub char_id: i32,
    /// `Player.createDb`: `INSERT INTO characters`.
    pub insert: NewCharacter,
    /// The inventory at the end, with equipped items already in `PAPERDOLL`. Java inserts every
    /// one of them (`Item.insertIntoDb`).
    pub items: Vec<NewItem>,
    /// `Player.storeSkill` for each skill (`REPLACE INTO character_skills`, class index 0).
    pub skills: Vec<SkillLevel>,
    /// `MacroList.registerMacroInDb`.
    pub macros: Vec<NewMacro>,
    /// `Shortcuts.registerShortcutInDb` (`REPLACE INTO character_shortcuts`, class index 0).
    pub shortcuts: Vec<NewShortcut>,
    /// `Player.storeCharBase`: the full `UPDATE characters` that `storeMe` runs.
    pub update: CharacterUpdate,
    /// `Player.updateOnlineStatus` from `deleteMe`: `online = 0, lastAccess = <this>`.
    pub last_access: i64,
    /// `Player.storeRecommendations`: `rec_have`, `rec_left`, `time_left`.
    pub reco: (i32, i32, i64),
    /// Every object id the plan uses (the character and each item).
    pub object_ids: Vec<i32>,
}

/// An item of the new inventory while it is being put together.
#[derive(Debug, Clone)]
struct InvItem {
    object_id: i32,
    item_id: i32,
    count: i64,
    loc: ItemLocation,
    loc_data: i32,
    enchant_level: i32,
    mana: i32,
    time: i64,
}

/// The new character's inventory: `Inventory._items` and the paperdoll.
struct Inventory<'a> {
    datapack: &'a Datapack,
    items: Vec<InvItem>,
    paperdoll: [Option<usize>; slot::TOTAL_SLOTS],
    multiple_item_drop: bool,
    max_adena: i64,
    now_ms: i64,
    owner: i32,
}

impl Inventory<'_> {
    fn template(&self, index: usize) -> Option<&ItemTemplate> {
        self.datapack.items.get(self.items[index].item_id)
    }

    /// Java `getItemByItemId`: the first item with that id.
    fn by_item_id(&self, item_id: i32) -> Option<usize> {
        self.items.iter().position(|i| i.item_id == item_id)
    }

    /// Java `ItemContainer.addItem(process, itemId, count, ...)`.
    fn add_item(
        &mut self,
        ids: &mut IdSource<'_>,
        item_id: i32,
        count: i64,
    ) -> Result<Option<usize>, PlanError> {
        let existing = self.by_item_id(item_id);
        if let Some(index) = existing
            && self.template(index).is_some_and(|t| t.stackable)
        {
            // Item.changeCount
            let max = if item_id == ADENA_ID {
                self.max_adena
            } else {
                i64::MAX
            };
            let item = &mut self.items[index];
            if count != 0 {
                item.count = if count > 0 && item.count > max - count {
                    max
                } else {
                    item.count + count
                };
                if item.count < 0 {
                    item.count = 0;
                }
            }
            return Ok(Some(index));
        }
        let mut last = None;
        for _ in 0..count {
            let Some(template) = self.datapack.items.get(item_id) else {
                warn!("Invalid ItemId ({item_id}) requested by REWARD");
                return Ok(None);
            };
            let (stackable, duration, lifetime, enchanted) = (
                template.stackable,
                template.duration,
                template.time,
                template.default_enchant_level,
            );
            let object_id = ids.next().ok_or(PlanError::OutOfIds)?;
            // ItemManager.createItem: `new Item` starts at count 1, a stack gets its count.
            let item_count = if stackable && count > 1 { count } else { 1 };
            self.items.push(InvItem {
                object_id,
                item_id,
                count: item_count,
                loc: ItemLocation::Inventory,
                loc_data: 0,
                enchant_level: enchanted,
                mana: duration,
                time: if lifetime == -1 {
                    -1
                } else {
                    self.now_ms + lifetime * 60 * 1000
                },
            });
            last = Some(self.items.len() - 1);
            if stackable || !self.multiple_item_drop {
                break;
            }
        }
        Ok(last)
    }

    /// Java `Item.isEquipable`.
    fn is_equipable(&self, index: usize) -> bool {
        let Some(t) = self.template(index) else {
            return false;
        };
        t.body_part != BodyPart::None
            && !matches!(
                t.etc().map(|e| e.etc_type),
                Some(EtcItemType::Arrow | EtcItemType::Bolt | EtcItemType::Lure)
            )
    }

    fn weapon_type(&self, index: usize) -> Option<WeaponType> {
        self.template(index)?.weapon().map(|w| w.weapon_type)
    }

    fn etc_type(&self, index: usize) -> Option<EtcItemType> {
        self.template(index)?.etc().map(|e| e.etc_type)
    }

    fn body_part(&self, index: usize) -> BodyPart {
        self.template(index).map_or(BodyPart::None, |t| t.body_part)
    }

    /// Java `Inventory.setPaperdollItem`, with the `BowCrossRodListener` (the other listeners only
    /// change stats or give skills that are not stored).
    fn set_paperdoll(&mut self, slot_index: usize, item: Option<usize>) {
        let old = self.paperdoll[slot_index];
        if old == item {
            return;
        }
        if let Some(old) = old {
            self.paperdoll[slot_index] = None;
            self.items[old].loc = ItemLocation::Inventory;
            self.items[old].loc_data = 0;
            if slot_index == slot::RHAND
                && matches!(
                    self.weapon_type(old),
                    Some(WeaponType::Bow | WeaponType::CrossBow | WeaponType::FishingRod)
                )
                && self.paperdoll[slot::LHAND].is_some()
            {
                self.set_paperdoll(slot::LHAND, None);
            }
        }
        if let Some(new) = item {
            self.paperdoll[slot_index] = Some(new);
            self.items[new].loc = ItemLocation::Paperdoll;
            self.items[new].loc_data = i32::try_from(slot_index).unwrap_or(0);
            if slot_index == slot::RHAND {
                let finder = match self.weapon_type(new) {
                    Some(WeaponType::Bow) => Some(EtcItemType::Arrow),
                    Some(WeaponType::CrossBow) => Some(EtcItemType::Bolt),
                    _ => None,
                };
                if let Some(wanted) = finder {
                    let grade = self.template(new).map(|t| t.crystal_type_plus());
                    let found = (0..self.items.len()).find(|&i| {
                        self.etc_type(i) == Some(wanted)
                            && self.template(i).map(|t| t.crystal_type_plus()) == grade
                    });
                    if let Some(found) = found {
                        self.set_paperdoll(slot::LHAND, Some(found));
                    }
                }
            }
        }
    }

    /// Java `Inventory.equipItem` for a player that is not in a store.
    fn equip_item(&mut self, index: usize) {
        let Some(template) = self.template(index) else {
            return;
        };
        // Hero weapons need a hero (or a GM); a new character is neither.
        if template.is_hero_item() {
            return;
        }
        let body_part = template.body_part;
        let item_id = template.id;
        // Formal wear blocks everything except the chest slot and the wedding bouquet.
        if let Some(chest) = self.paperdoll[slot::CHEST]
            && item_id != 21163
            && self.body_part(chest) == BodyPart::AllDress
            && matches!(
                body_part,
                BodyPart::BothHands
                    | BodyPart::LeftHand
                    | BodyPart::RightHand
                    | BodyPart::Legs
                    | BodyPart::Feet
                    | BodyPart::Gloves
                    | BodyPart::Head
            )
        {
            return;
        }
        match body_part {
            BodyPart::BothHands => {
                self.set_paperdoll(slot::LHAND, None);
                self.set_paperdoll(slot::RHAND, Some(index));
            }
            BodyPart::LeftHand => {
                if let Some(rh) = self.paperdoll[slot::RHAND]
                    && self.body_part(rh) == BodyPart::BothHands
                {
                    let keeps = matches!(
                        (self.weapon_type(rh), self.etc_type(index)),
                        (Some(WeaponType::Bow), Some(EtcItemType::Arrow))
                            | (Some(WeaponType::CrossBow), Some(EtcItemType::Bolt))
                            | (Some(WeaponType::FishingRod), Some(EtcItemType::Lure))
                    );
                    if !keeps {
                        self.set_paperdoll(slot::RHAND, None);
                    }
                }
                self.set_paperdoll(slot::LHAND, Some(index));
            }
            BodyPart::RightHand => self.set_paperdoll(slot::RHAND, Some(index)),
            BodyPart::LeftEar | BodyPart::RightEar | BodyPart::BothEars => {
                let target = if self.paperdoll[slot::LEAR].is_none() {
                    slot::LEAR
                } else if self.paperdoll[slot::REAR].is_none() {
                    slot::REAR
                } else {
                    slot::LEAR
                };
                self.set_paperdoll(target, Some(index));
            }
            BodyPart::LeftFinger | BodyPart::RightFinger | BodyPart::BothFingers => {
                let target = if self.paperdoll[slot::LFINGER].is_none() {
                    slot::LFINGER
                } else if self.paperdoll[slot::RFINGER].is_none() {
                    slot::RFINGER
                } else {
                    slot::LFINGER
                };
                self.set_paperdoll(target, Some(index));
            }
            BodyPart::Neck => self.set_paperdoll(slot::NECK, Some(index)),
            BodyPart::FullArmor => {
                self.set_paperdoll(slot::LEGS, None);
                self.set_paperdoll(slot::CHEST, Some(index));
            }
            BodyPart::Chest => self.set_paperdoll(slot::CHEST, Some(index)),
            BodyPart::Legs => {
                if let Some(chest) = self.paperdoll[slot::CHEST]
                    && self.body_part(chest) == BodyPart::FullArmor
                {
                    self.set_paperdoll(slot::CHEST, None);
                }
                self.set_paperdoll(slot::LEGS, Some(index));
            }
            BodyPart::Feet => self.set_paperdoll(slot::FEET, Some(index)),
            BodyPart::Gloves => self.set_paperdoll(slot::GLOVES, Some(index)),
            BodyPart::Head => self.set_paperdoll(slot::HEAD, Some(index)),
            BodyPart::Hair => {
                let all = self.paperdoll[slot::HAIR].is_some_and(|h| self.body_part(h) == BodyPart::HairAll);
                self.set_paperdoll(if all { slot::HAIR2 } else { slot::HAIR }, None);
                self.set_paperdoll(slot::HAIR, Some(index));
            }
            BodyPart::Hair2 => {
                let all = self.paperdoll[slot::HAIR].is_some_and(|h| self.body_part(h) == BodyPart::HairAll);
                self.set_paperdoll(if all { slot::HAIR } else { slot::HAIR2 }, None);
                self.set_paperdoll(slot::HAIR2, Some(index));
            }
            BodyPart::HairAll => {
                self.set_paperdoll(slot::HAIR2, None);
                self.set_paperdoll(slot::HAIR, Some(index));
            }
            BodyPart::Underwear => self.set_paperdoll(slot::UNDER, Some(index)),
            BodyPart::Back => self.set_paperdoll(slot::CLOAK, Some(index)),
            BodyPart::LeftBracelet => self.set_paperdoll(slot::LBRACELET, Some(index)),
            BodyPart::RightBracelet => self.set_paperdoll(slot::RBRACELET, Some(index)),
            // A new character has no talisman slot (`getTalismanSlots() == 0`).
            BodyPart::Deco => {}
            BodyPart::Belt => self.set_paperdoll(slot::BELT, Some(index)),
            BodyPart::AllDress => {
                for s in [
                    slot::LEGS,
                    slot::LHAND,
                    slot::RHAND,
                    slot::HEAD,
                    slot::FEET,
                    slot::GLOVES,
                ] {
                    self.set_paperdoll(s, None);
                }
                self.set_paperdoll(slot::CHEST, Some(index));
            }
            other => warn!("Unknown body slot {other:?} for Item ID: {item_id}"),
        }
    }

    /// The `maxMp` the equipped items add.
    fn equipped_max_mp(&self, extra: &ItemMaxMp) -> f64 {
        self.paperdoll
            .iter()
            .flatten()
            .map(|&i| extra.max_mp_of(self.items[i].item_id))
            .sum()
    }
}

/// Java `MacroList.registerMacroInDb`'s command column.
pub(crate) fn macro_commands(commands: &[MacroCommand]) -> String {
    let mut text = String::new();
    for cmd in commands {
        text.push_str(&format!("{},{},{}", macro_type_ordinal(cmd.kind), cmd.d1, cmd.d2));
        if !cmd.text.is_empty() {
            text.push(',');
            text.push_str(&cmd.text);
        }
        text.push(';');
    }
    // `sb.setLength(255)` counts UTF-16 units.
    if text.encode_utf16().count() > 255 {
        let units: Vec<u16> = text.encode_utf16().take(255).collect();
        text = String::from_utf16_lossy(&units);
    }
    text
}

fn macro_type_ordinal(kind: MacroType) -> i32 {
    match kind {
        MacroType::None => 0,
        MacroType::Skill => 1,
        MacroType::Action => 2,
        MacroType::Text => 3,
        MacroType::Shortcut => 4,
        MacroType::Item => 5,
        MacroType::Delay => 6,
    }
}

/// `ShortcutType.ordinal()`.
pub fn shortcut_type_ordinal(kind: ShortcutType) -> i32 {
    match kind {
        ShortcutType::None => 0,
        ShortcutType::Item => 1,
        ShortcutType::Skill => 2,
        ShortcutType::Action => 3,
        ShortcutType::Macro => 4,
        ShortcutType::Recipe => 5,
        ShortcutType::Bookmark => 6,
    }
}

fn macro_row(preset: &MacroPreset) -> NewMacro {
    NewMacro {
        id: preset.id,
        icon: preset.icon,
        name: preset.name.clone(),
        descr: preset.description.clone(),
        acronym: preset.acronym.clone(),
        commands: macro_commands(&preset.commands),
    }
}

/// Java `SkillTreeData.getAvailableSkills(player, class, false, true)`: the skills of the class tree
/// the player may learn next, given the skills it knows (`known`: skill id to level).
fn available_skills(
    datapack: &Datapack,
    config: &GameConfig,
    class: ClassId,
    level: i32,
    known: &BTreeMap<i32, i32>,
) -> Vec<(i32, i32)> {
    let mut result = Vec::new();
    for skill in datapack.skill_trees.complete_class_tree(class) {
        if skill.skill_id == l2_data::skill_tree::DIVINE_INSPIRATION_ID
            && !config.player.auto_learn_divine_inspiration
        {
            continue;
        }
        if (!skill.auto_get && !skill.learned_by_npc) || level < skill.get_level {
            continue;
        }
        match known.get(&skill.skill_id) {
            Some(&old) if old == skill.skill_level - 1 => result.push((skill.skill_id, skill.skill_level)),
            None if skill.skill_level == 1 => result.push((skill.skill_id, skill.skill_level)),
            _ => {}
        }
    }
    result
}

/// Java `SkillTreeData.getAvailableAutoGetSkills`: auto-get skills above what the player knows.
fn available_auto_get_skills(
    datapack: &Datapack,
    class: ClassId,
    level: i32,
    known: &BTreeMap<i32, i32>,
) -> Vec<(i32, i32)> {
    datapack
        .skill_trees
        .complete_class_tree(class)
        .filter(|s| s.auto_get && level >= s.get_level)
        .filter(|s| known.get(&s.skill_id).is_none_or(|&old| old < s.skill_level))
        .map(|s| (s.skill_id, s.skill_level))
        .collect()
}

/// Works out the new character, in the order of `Player.create` and `CharacterCreate.initNewChar`.
///
/// `char_id` is the id taken for the character itself (Java takes it in the `Player`
/// constructor); the item ids come from `ids`.
// Java: Player.create, CharacterCreate.runImpl (after the checks), initNewChar, Player.storeMe
pub fn build_plan(
    ctx: &PlanContext<'_>,
    request: &CreateRequest<'_>,
    template: &PlayerTemplate,
    char_id: i32,
    ids: &mut IdSource<'_>,
    rng: &mut WorldRng,
    now_ms: i64,
) -> Result<NewCharacterPlan, PlanError> {
    let config = ctx.config;
    let datapack = ctx.datapack;
    let class_id = template.class_id;
    let class = i32::from(class_id.0);
    let race = template.race;
    let race_ordinal = race as i32;

    let hp_mp_cp = |level: i32, extra_mp: f64| -> Result<(i32, i32, i32), PlanError> {
        let index = usize::try_from(level).map_err(|_| PlanError::NoStatRow(level))?;
        let (Some(hp), Some(mp), Some(cp)) = (
            template.base_hp_max(index),
            template.base_mp_max(index),
            template.base_cp_max(index),
        ) else {
            return Err(PlanError::NoStatRow(level));
        };
        let con = ctx.stat_bonus.con_bonus(template.base_con.max(1));
        let men = ctx.stat_bonus.men_bonus(template.base_men.max(1));
        Ok((
            calc_max(hp, 0.0, con),
            calc_max(mp, extra_mp, men),
            calc_max(cp, 0.0, con),
        ))
    };

    // Player.create + createDb: level 1, nothing equipped, no skills.
    let mut level = 1;
    let (max_hp, max_mp, max_cp) = hp_mp_cp(level, 0.0)?;
    let create_date = CalendarDate::from_unix_millis_utc(now_ms);
    let sex = i32::from(request.is_female);
    let title_color = 0x00EC_F9A2; // PlayerAppearance.DEFAULT_TITLE_COLOR
    let insert = NewCharacter {
        account_name: request.account.to_owned(),
        char_id,
        char_name: request.name.to_owned(),
        level,
        max_hp,
        cur_hp: 0.0,
        max_cp,
        cur_cp: 0.0,
        max_mp,
        cur_mp: 0.0,
        face: i32::from(request.face),
        hair_style: i32::from(request.hair_style),
        hair_color: i32::from(request.hair_color),
        sex,
        exp: 0,
        sp: 0,
        karma: 0,
        fame: 0,
        pvp_kills: 0,
        pk_kills: 0,
        clan_id: 0,
        race: race_ordinal,
        class_id: class,
        delete_time: 0,
        // NOTE(java-quirk): `hasDwarvenCraft()` is asked before any skill exists and
        // `UPDATE_CHARACTER` has no `cancraft`, so the column stays 0 even for a dwarf.
        can_craft: false,
        // NOTE(java-quirk): a new Player has a null title until initNewChar sets one.
        title: None,
        title_color,
        access_level: 0,
        online: 0,
        is_in_7s_dungeon: false,
        clan_privs: 0,
        wants_peace: 0,
        base_class: class,
        newbie: 1,
        noble: false,
        create_date,
        last_access: now_ms,
    };

    // CharacterCreate.runImpl: "HP and MP are at maximum and CP is zero by default."
    let mut cur_hp = f64::from(max_hp);
    let mut cur_mp = f64::from(max_mp);
    let mut cur_cp = 0.0;

    // initNewChar: Adena first.
    let max_adena = config.effective_max_adena();
    let mut inventory = Inventory {
        datapack,
        items: Vec::new(),
        paperdoll: [None; slot::TOTAL_SLOTS],
        multiple_item_drop: config.general.multiple_item_drop,
        max_adena,
        now_ms,
        owner: char_id,
    };
    if config.player.starting_adena > 0 {
        // Player.addAdena: the amount is cut to what fits under MaxAdena.
        let count = config.player.starting_adena.min(max_adena);
        if count > 0 {
            inventory.add_item(ids, ADENA_ID, count)?;
        }
    }

    // Start location: custom, faction, or one of the creation points.
    let (x, y, z) = if config.starting_location.custom_starting_loc {
        (
            config.starting_location.custom_starting_loc_x,
            config.starting_location.custom_starting_loc_y,
            config.starting_location.custom_starting_loc_z,
        )
    } else if config.faction_system.faction_system_enabled {
        let loc = config.manual.faction_system.faction_starting_location;
        (loc.x, loc.y, loc.z)
    } else {
        let points = &template.creation_points;
        let pick = usize::try_from(rng.below(i32::try_from(points.len()).unwrap_or(i32::MAX))).unwrap_or(0);
        points.get(pick).map_or((0, 0, 0), |p| (p.x, p.y, p.z))
    };

    // Player.setTitle: a player title of more than 21 characters is cut to 20.
    let mut title = if config.starting_title.enable_custom_starting_title {
        config.starting_title.custom_starting_title.clone()
    } else {
        String::new()
    };
    if title.chars().count() > 21 {
        title = title.chars().take(20).collect();
    }

    let mut vitality = 1;
    if config.player.enable_vitality {
        vitality = config
            .player
            .starting_vitality_points
            .min(MAX_VITALITY_POINTS)
            .clamp(MIN_VITALITY_POINTS, MAX_VITALITY_POINTS);
    }

    // The skills the character knows (id to level), in the order they were given.
    let mut skills: BTreeMap<i32, i32> = BTreeMap::new();

    // PlayerStat.addLevel(STARTING_LEVEL - 1)
    let mut exp = 0_i64;
    let starting_level = i32::from(config.player.starting_level);
    if starting_level > 1 {
        let wanted = starting_level;
        let max_level = datapack
            .experience
            .max_level()
            .and_then(|m| i32::try_from(m).ok());
        if max_level.is_some_and(|max| wanted <= max) {
            level = wanted;
            // "Sync up exp with current level."
            let for_level = datapack.experience.exp_for_level(level);
            let for_next = datapack.experience.exp_for_level(level + 1);
            if let (Some(for_level), Some(for_next)) = (for_level, for_next)
                && (exp >= for_next || for_level > exp)
            {
                exp = for_level;
            }
            let (hp, mp, cp) = hp_mp_cp(level, 0.0)?;
            cur_hp = f64::from(hp);
            cur_mp = f64::from(mp);
            cur_cp = f64::from(cp);
            // Player.rewardSkills -> giveAvailableAutoGetSkills (store = true).
            // NOTE: Java walks a HashMap, so for a skill with several levels the one it ends up
            // with depends on the iteration order; the highest level is used here.
            // AutoLearnSkills (giveAvailableSkills) is not ported and acts like the default.
            if config.player.auto_learn_skills {
                warn!(
                    "AutoLearnSkills with StartingLevel above 1 is not supported; only auto-get skills are given."
                );
            }
            for (id, skill_level) in available_auto_get_skills(datapack, class_id, level, &skills) {
                let entry = skills.entry(id).or_insert(skill_level);
                *entry = (*entry).max(skill_level);
            }
        }
    }

    // addSp(STARTING_SP)
    let mut sp = 0_i64;
    if config.player.starting_sp > 0 {
        sp = i64::from(config.player.starting_sp).min(config.manual.player.max_sp);
    }

    // Starting equipment.
    if let Some(equipment) = datapack.initial_equipment.get(class_id) {
        for entry in equipment {
            let item = inventory.add_item(ids, entry.item_id, entry.count)?;
            let Some(item) = item else {
                warn!(
                    "Could not create item during player creation: itemId {}, amount {}.",
                    entry.item_id, entry.count
                );
                continue;
            };
            if inventory.is_equipable(item) && entry.equipped {
                inventory.equip_item(item);
            }
        }
    }

    // Skills of the class tree (`addSkill(skill, true)`).
    for (id, skill_level) in available_skills(datapack, config, class_id, level, &skills) {
        skills.insert(id, skill_level);
    }

    // Shortcuts and the macros they use.
    let mut macros: BTreeMap<i32, NewMacro> = BTreeMap::new();
    let mut bar: BTreeMap<i32, NewShortcut> = BTreeMap::new();
    let presets = &datapack.initial_shortcuts;
    let class_list = presets.class_shortcuts(class_id).unwrap_or(&[]);
    for preset in presets.global().iter().chain(class_list) {
        let mut shortcut_id = preset.id;
        match preset.kind {
            ShortcutType::Item => {
                let Some(item) = inventory.by_item_id(shortcut_id) else {
                    continue;
                };
                shortcut_id = inventory.items[item].object_id;
            }
            ShortcutType::Skill => {
                if !skills.contains_key(&shortcut_id) {
                    continue;
                }
            }
            ShortcutType::Macro => {
                let Some(found) = presets.macro_preset(shortcut_id) else {
                    continue;
                };
                // MacroList.registerMacro: a macro registered twice is deleted and stored again.
                macros.insert(found.id, macro_row(found));
            }
            _ => {}
        }
        // Shortcuts.registerShortcut: the bar is keyed by `slot + page * 12`; a new shortcut
        // replaces (and deletes) whatever had that key.
        bar.insert(
            preset.slot + preset.page * MAX_SHORTCUTS_PER_BAR,
            NewShortcut {
                slot: preset.slot,
                page: preset.page,
                kind: shortcut_type_ordinal(preset.kind),
                shortcut_id,
                level: preset.level,
            },
        );
    }

    // Player.storeMe: the stats now include the equipped items.
    let extra_mp = inventory.equipped_max_mp(ctx.item_max_mp);
    let (max_hp, max_mp, max_cp) = hp_mp_cp(level, extra_mp)?;
    let access_level = 0;
    if config.general.everybody_has_admin_rights {
        // TODO(M2-CHAR-01): Java gives the master access level of config/AccessLevels.xml.
        warn!(
            "EverybodyHasAdminRights is not supported by the character screen yet; access level 0 is stored."
        );
    }
    let update = CharacterUpdate {
        level,
        max_hp,
        cur_hp,
        max_cp,
        cur_cp,
        max_mp,
        cur_mp,
        face: i32::from(request.face),
        hair_style: i32::from(request.hair_style),
        hair_color: i32::from(request.hair_color),
        sex,
        heading: 0,
        x,
        y,
        z,
        exp,
        exp_before_death: 0,
        sp,
        karma: 0,
        fame: 0,
        pvp_kills: 0,
        pk_kills: 0,
        clan_id: 0,
        race: race_ordinal,
        class_id: class,
        delete_time: 0,
        title: Some(title),
        title_color,
        access_level,
        // `isOnlineInt()` is 0: Disconnection detached the client before the store.
        online: 0,
        is_in_7s_dungeon: false,
        clan_privs: 0,
        wants_peace: 0,
        base_class: class,
        online_time: 0,
        newbie: 1,
        noble: false,
        power_grade: 0,
        subpledge: 0,
        lvl_joined_academy: 0,
        apprentice: 0,
        sponsor: 0,
        clan_join_expiry_time: 0,
        clan_create_expiry_time: 0,
        char_name: request.name.to_owned(),
        death_penalty_level: 0,
        bookmark_slot: 0,
        vitality_points: vitality,
        // NOTE: a Player's `_lang` is null until the client language is known.
        language: None,
        faction: 0,
        pccafe_points: 0,
    };

    let items = inventory
        .items
        .iter()
        .map(|i| NewItem {
            owner_id: inventory.owner,
            item_id: i.item_id,
            count: i.count,
            loc: i.loc,
            loc_data: i.loc_data,
            enchant_level: i.enchant_level,
            object_id: i.object_id,
            custom_type1: 0,
            custom_type2: 0,
            mana_left: i.mana,
            time: i.time,
        })
        .collect();
    let mut object_ids = vec![char_id];
    object_ids.extend(inventory.items.iter().map(|i| i.object_id));
    info!(
        "New character plan: class {class}, {} items, {} skills, {} shortcuts.",
        inventory.items.len(),
        skills.len(),
        bar.len()
    );
    Ok(NewCharacterPlan {
        char_id,
        insert,
        items,
        skills: skills
            .into_iter()
            .map(|(skill_id, skill_level)| SkillLevel {
                skill_id,
                skill_level,
            })
            .collect(),
        macros: macros.into_values().collect(),
        shortcuts: bar.into_values().collect(),
        update,
        last_access: now_ms,
        reco: (0, STARTING_RECOMMENDATIONS, 0),
        object_ids,
    })
}

/// The race of a class, for the `AllowedPlayerRaces` checks.
pub fn race_allowed(config: &GameConfig, race: Race) -> bool {
    let allowed = &config.allowed_player_races;
    match race {
        Race::Human => allowed.allow_human,
        Race::Elf => allowed.allow_elf,
        Race::DarkElf => allowed.allow_darkelf,
        Race::Orc => allowed.allow_orc,
        Race::Dwarf => allowed.allow_dwarf,
        Race::Kamael => allowed.allow_kamael,
    }
}
