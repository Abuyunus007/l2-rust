//! The sorted withdraw list: `SortedWareHouseWithdrawalList` with its filters and comparators.
// Java: network/serverpackets/SortedWareHouseWithdrawalList.java
// Java: network/enums/WarehouseListType.java, entity/item/WarehouseItem.java
// Java: config/custom/WarehouseSortingConfig.java (the `WhSortedP.htm` / `WhSortedC.htm` pages)
//
// Left out: the `RECIPE` sort by recipe level (`RecipeData`, the craft card): the list is only
// sorted by name there (the first of Java's two sorts).

use std::cmp::Ordering;
use std::collections::HashSet;

use l2_data::item::{EtcItemType, ItemTemplate, TYPE2_MONEY, TYPE2_WEAPON};
use l2_data::{ClassId, ItemTemplates};
use l2_protocol_game::server::{SortedWareHouseWithdrawalList, SortedWarehouseItem};
use l2_util::ObjectId;

use super::container::ContainerRef;
use crate::player::ItemData;
use crate::world::{World, unix_ms_now};

/// Java `SortedWareHouseWithdrawalList.MAX_SORT_LIST_ITEMS`.
pub const MAX_SORT_LIST_ITEMS: usize = 300;

/// Java `SortedWareHouseWithdrawalList.A2Z`.
pub const A2Z: i8 = 1;
/// Java `SortedWareHouseWithdrawalList.Z2A`.
pub const Z2A: i8 = -1;
/// Java `SortedWareHouseWithdrawalList.GRADE`.
pub const GRADE: i8 = 2;
/// Java `SortedWareHouseWithdrawalList.LEVEL`.
pub const LEVEL: i8 = 3;
/// Java `SortedWareHouseWithdrawalList.TYPE`.
pub const TYPE: i8 = 4;
/// Java `SortedWareHouseWithdrawalList.WEAR`.
pub const WEAR: i8 = 5;

/// Java `WarehouseListType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WarehouseListType {
    Weapon,
    Armor,
    EtcItem,
    Material,
    Recipe,
    Amulett,
    SpellBook,
    Shot,
    Scroll,
    Consumable,
    Seed,
    Potion,
    Quest,
    Pet,
    Other,
    All,
}

impl WarehouseListType {
    /// Java `WarehouseListType.valueOf(name)`: exact and case-sensitive, `None` where Java throws.
    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "WEAPON" => Self::Weapon,
            "ARMOR" => Self::Armor,
            "ETCITEM" => Self::EtcItem,
            "MATERIAL" => Self::Material,
            "RECIPE" => Self::Recipe,
            "AMULETT" => Self::Amulett,
            "SPELLBOOK" => Self::SpellBook,
            "SHOT" => Self::Shot,
            "SCROLL" => Self::Scroll,
            "CONSUMABLE" => Self::Consumable,
            "SEED" => Self::Seed,
            "POTION" => Self::Potion,
            "QUEST" => Self::Quest,
            "PET" => Self::Pet,
            "OTHER" => Self::Other,
            "ALL" => Self::All,
            _ => return None,
        })
    }
}

/// Java `SortedWareHouseWithdrawalList.getOrder(order)`: the sort order of the text, `A2Z` for
/// anything unknown.
pub fn get_order(order: &str) -> i8 {
    if order.starts_with("A2Z") {
        A2Z
    } else if order.starts_with("Z2A") {
        Z2A
    } else if order.starts_with("GRADE") {
        GRADE
    } else if order.starts_with("TYPE") {
        TYPE
    } else if order.starts_with("WEAR") {
        WEAR
    } else {
        // `Byte.parseByte(order)`; a number that is not a byte falls back to A2Z.
        parse_java_byte(order).unwrap_or(A2Z)
    }
}

/// Java `Byte.parseByte(text)`: an optional sign and decimal digits that fit a byte.
fn parse_java_byte(text: &str) -> Option<i8> {
    let digits = text.strip_prefix(['+', '-']).unwrap_or(text);
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    text.strip_prefix('+').unwrap_or(text).parse().ok()
}

/// Java `WarehouseItem`: an item of the container with its template.
struct Entry<'a> {
    item: &'a ItemData,
    template: &'a ItemTemplate,
}

impl Entry<'_> {
    fn is_money(&self) -> bool {
        self.template.type2 == TYPE2_MONEY
    }

    fn etc_type(&self) -> Option<EtcItemType> {
        self.template.etc().map(|e| e.etc_type)
    }

    fn is_amulet_name(&self) -> bool {
        self.template.name.to_uppercase().starts_with("AMULET")
    }
}

/// Java `String.compareTo`: UTF-16 code units.
fn compare_java_strings(a: &str, b: &str) -> Ordering {
    a.encode_utf16().cmp(b.encode_utf16())
}

/// The first rule of every Java comparator: adena sorts first for A..Z and last otherwise.
fn money_rule(a: &Entry<'_>, b: &Entry<'_>, order: i8) -> Option<Ordering> {
    match (a.is_money(), b.is_money()) {
        (true, false) => Some(if order == A2Z {
            Ordering::Less
        } else {
            Ordering::Greater
        }),
        (false, true) => Some(if order == A2Z {
            Ordering::Greater
        } else {
            Ordering::Less
        }),
        _ => None,
    }
}

/// Java `WarehouseItemNameComparator`.
fn by_name(a: &Entry<'_>, b: &Entry<'_>, order: i8) -> Ordering {
    if let Some(result) = money_rule(a, b, order) {
        return result;
    }
    if order == A2Z {
        compare_java_strings(&a.template.name, &b.template.name)
    } else {
        compare_java_strings(&b.template.name, &a.template.name)
    }
}

/// Java `WarehouseItemGradeComparator` (always called with `A2Z`).
fn by_grade(a: &Entry<'_>, b: &Entry<'_>, order: i8) -> Ordering {
    if let Some(result) = money_rule(a, b, order) {
        return result;
    }
    if order == A2Z {
        a.template.crystal_type.cmp(&b.template.crystal_type)
    } else {
        b.template.crystal_type.cmp(&a.template.crystal_type)
    }
}

/// Java `WarehouseItemBodypartComparator`.
fn by_body_part(a: &Entry<'_>, b: &Entry<'_>, order: i8) -> Ordering {
    if let Some(result) = money_rule(a, b, order) {
        return result;
    }
    let (ma, mb) = (a.template.body_part.mask(), b.template.body_part.mask());
    if order == A2Z { ma.cmp(&mb) } else { mb.cmp(&ma) }
}

/// Java `WarehouseItemTypeComparator`: by the material.
fn by_material(a: &Entry<'_>, b: &Entry<'_>, order: i8) -> Ordering {
    if let Some(result) = money_rule(a, b, order) {
        return result;
    }
    if order == A2Z {
        a.template.material.cmp(&b.template.material)
    } else {
        b.template.material.cmp(&a.template.material)
    }
}

/// Whether the entry belongs to the filtered list (the `create*List` methods of Java).
fn passes(kind: WarehouseListType, entry: &Entry<'_>, book_ids: &HashSet<i32>) -> bool {
    let t = entry.template;
    let etc = entry.etc_type();
    let is_etc = t.is_etc_item();
    match kind {
        WarehouseListType::Weapon => {
            t.is_weapon()
                || t.type2 == TYPE2_WEAPON
                || (is_etc && etc == Some(EtcItemType::Arrow))
                || entry.is_money()
        }
        WarehouseListType::Armor => t.is_armor() || entry.is_money(),
        WarehouseListType::EtcItem => is_etc || entry.is_money(),
        WarehouseListType::Material => etc == Some(EtcItemType::Material) || entry.is_money(),
        WarehouseListType::Recipe => etc == Some(EtcItemType::Recipe) || entry.is_money(),
        WarehouseListType::Amulett => (is_etc && entry.is_amulet_name()) || entry.is_money(),
        WarehouseListType::SpellBook => {
            (is_etc && book_ids.contains(&t.id) && !entry.is_amulet_name()) || entry.is_money()
        }
        WarehouseListType::Consumable => {
            matches!(etc, Some(EtcItemType::Scroll | EtcItemType::Shot)) || entry.is_money()
        }
        WarehouseListType::Shot => etc == Some(EtcItemType::Shot) || entry.is_money(),
        WarehouseListType::Scroll => etc == Some(EtcItemType::Scroll) || entry.is_money(),
        WarehouseListType::Seed => etc == Some(EtcItemType::Seed) || entry.is_money(),
        WarehouseListType::Other => {
            (is_etc
                && !matches!(
                    etc,
                    Some(
                        EtcItemType::Material | EtcItemType::Recipe | EtcItemType::Scroll | EtcItemType::Shot
                    )
                ))
                || entry.is_money()
        }
        // `ALL` and the types Java has no list for (`POTION`, `QUEST`, `PET`).
        WarehouseListType::All
        | WarehouseListType::Potion
        | WarehouseListType::Quest
        | WarehouseListType::Pet => true,
    }
}

/// Java `createSpellbookList`: the items some class needs to learn a skill.
fn skill_book_ids(world: &World) -> HashSet<i32> {
    let mut ids = HashSet::new();
    // `PlayerClass.values()`: the ids of the classes go to 136.
    for class in (0..=136u16).map(ClassId) {
        for learn in world.rewards.skill_trees.complete_class_tree(class) {
            ids.extend(learn.required_items.iter().map(|i| i.item_id));
        }
    }
    ids
}

impl World {
    /// Java `new SortedWareHouseWithdrawalList(player, type, itemType, sortOrder)` for the active
    /// warehouse. `None` when the player has none.
    pub(crate) fn sorted_warehouse_list(
        &self,
        player: ObjectId,
        wh_type: i16,
        list_type: WarehouseListType,
        order: i8,
    ) -> Option<SortedWareHouseWithdrawalList> {
        let active: ContainerRef = self.players.get(&player)?.warehouse.active?;
        let items = self.container_items(active)?;
        let templates: std::sync::Arc<ItemTemplates> = self.item_templates()?;
        let book_ids = if list_type == WarehouseListType::SpellBook {
            skill_book_ids(self)
        } else {
            HashSet::new()
        };

        let mut entries: Vec<Entry<'_>> = Vec::new();
        for item in items {
            let Some(template) = templates.get(item.item_id) else {
                continue;
            };
            let entry = Entry { item, template };
            // `&& (list.size() < MAX_SORT_LIST_ITEMS)`
            if entries.len() < MAX_SORT_LIST_ITEMS && passes(list_type, &entry, &book_ids) {
                entries.push(entry);
            }
        }

        match order {
            A2Z | Z2A => entries.sort_by(|a, b| by_name(a, b, order)),
            GRADE if matches!(list_type, WarehouseListType::Armor | WarehouseListType::Weapon) => {
                entries.sort_by(|a, b| by_name(a, b, A2Z));
                entries.sort_by(|a, b| by_grade(a, b, A2Z));
            }
            // The second sort, by recipe level, needs `RecipeData` (the craft card).
            LEVEL if list_type == WarehouseListType::Recipe => entries.sort_by(|a, b| by_name(a, b, A2Z)),
            TYPE if list_type == WarehouseListType::Material => {
                entries.sort_by(|a, b| by_name(a, b, A2Z));
                entries.sort_by(|a, b| by_material(a, b, A2Z));
            }
            WEAR if list_type == WarehouseListType::Armor => {
                entries.sort_by(|a, b| by_name(a, b, A2Z));
                entries.sort_by(|a, b| by_body_part(a, b, A2Z));
            }
            _ => {}
        }

        let now_ms = unix_ms_now();
        Some(SortedWareHouseWithdrawalList {
            wh_type,
            player_adena: self.adena_of(player),
            items: entries
                .iter()
                .map(|e| sorted_item(e.item, e.template, now_ms))
                .collect(),
        })
    }
}

/// The fields of `WarehouseItem` as the packet writes them.
fn sorted_item(item: &ItemData, template: &ItemTemplate, now_ms: i64) -> SortedWarehouseItem {
    SortedWarehouseItem {
        object_id: item.object_id.0,
        display_id: template.display_id,
        location_slot: item.loc_data,
        count: item.count,
        type2: template.type2 as i16,
        // Java writes the low 16 bits of these ints.
        custom_type1: item.custom_type1 as i16,
        body_part: template.body_part.mask(),
        enchant: item.enchant_level as i16,
        custom_type2: item.custom_type2 as i16,
        augmentation: item.augmentation.unwrap_or(0),
        mana: item.mana_left,
        time: if item.time > 0 {
            ((item.time - now_ms) / 1000) as i32
        } else {
            -1
        },
        attack_element_type: item.attack_element_type(Some(template)) as i16,
        attack_element_power: item.attack_element_power(Some(template)) as i16,
        element_defense: std::array::from_fn(|element| {
            item.element_def_attr(element as i32, Some(template)) as i16
        }),
        // `item.getEnchantOptions()`; Java writes the low 16 bits.
        enchant_options: template
            .enchant_options_at(item.enchant_level)
            .map(|id| id as i16),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn order_names_follow_java() {
        assert_eq!(get_order("A2Z"), A2Z);
        assert_eq!(get_order("Z2A"), Z2A);
        assert_eq!(get_order("GRADE"), GRADE);
        assert_eq!(get_order("TYPE"), TYPE);
        assert_eq!(get_order("WEAR"), WEAR);
        // `startsWith`: a longer word still matches.
        assert_eq!(get_order("GRADEXYZ"), GRADE);
        // `Byte.parseByte`
        assert_eq!(get_order("3"), LEVEL);
        assert_eq!(get_order("+5"), WEAR);
        assert_eq!(get_order("-1"), Z2A);
        assert_eq!(get_order("0"), 0);
        // Not a byte, or not a number: A2Z.
        assert_eq!(get_order("300"), A2Z);
        assert_eq!(get_order("LEVEL"), A2Z);
        assert_eq!(get_order(""), A2Z);
    }

    #[test]
    fn list_type_names_are_exact() {
        assert_eq!(
            WarehouseListType::from_name("ARMOR"),
            Some(WarehouseListType::Armor)
        );
        assert_eq!(
            WarehouseListType::from_name("AMULETT"),
            Some(WarehouseListType::Amulett)
        );
        assert_eq!(WarehouseListType::from_name("armor"), None);
        assert_eq!(WarehouseListType::from_name("AMULET"), None);
    }

    #[test]
    fn java_string_order_is_by_utf16_unit() {
        assert_eq!(compare_java_strings("Apple", "Banana"), Ordering::Less);
        assert_eq!(compare_java_strings("apple", "Banana"), Ordering::Greater);
        assert_eq!(compare_java_strings("same", "same"), Ordering::Equal);
        assert_eq!(compare_java_strings("ab", "abc"), Ordering::Less);
    }
}
