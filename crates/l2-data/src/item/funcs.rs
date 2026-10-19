//! The stat functions of an item template: the `<stats>` block, the automatic enchant functions
//! and the base elements that elemental stats set.
//!
//! Java: the `stats` branch of `util/DocumentItem.java`, `ItemTemplate.attach(FuncTemplate)` and
//! `ItemTemplate.setElementals`, `entity/item/holders/Elementals.java` (the element ids).

use super::{ArmorType, ItemKind, ItemTemplate, TYPE1_WEAPON_RING_EARRING_NECKLACE, TYPE2_ACCESSORY};
use crate::loader::DataError;
use crate::stats::jnum::parse_double;
use crate::stats::{FuncKind, FuncTemplate, Stat, XmlNode};

/// Java `Elementals.FIRE` .. `DARK` (the ids are the Java byte constants).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Element {
    /// `Elementals.FIRE` (0).
    Fire = 0,
    /// `Elementals.WATER` (1).
    Water = 1,
    /// `Elementals.WIND` (2).
    Wind = 2,
    /// `Elementals.EARTH` (3).
    Earth = 3,
    /// `Elementals.HOLY` (4).
    Holy = 4,
    /// `Elementals.DARK` (5).
    Dark = 5,
}

/// One entry of `ItemTemplate._elementals`: the element an item has by itself and its value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ElementalValue {
    /// The element.
    pub element: Element,
    /// The `(int)` of the stat value.
    pub value: i32,
}

/// Java `ItemTemplate.attach(FuncTemplate)`: elemental stats also set the base element, then the
/// template is appended.
pub(super) fn attach(item: &mut ItemTemplate, template: FuncTemplate) {
    let element = match template.stat {
        Stat::FireRes | Stat::FirePower => Some(Element::Fire),
        Stat::WaterRes | Stat::WaterPower => Some(Element::Water),
        Stat::WindRes | Stat::WindPower => Some(Element::Wind),
        Stat::EarthRes | Stat::EarthPower => Some(Element::Earth),
        Stat::HolyRes | Stat::HolyPower => Some(Element::Holy),
        Stat::DarkRes | Stat::DarkPower => Some(Element::Dark),
        _ => None,
    };
    if let Some(element) = element {
        // Java `setElementals`: update the entry of that element, else append.
        let value = template.value as i32;
        match item.elementals.iter_mut().find(|e| e.element == element) {
            Some(existing) => existing.value = value,
            None => item.elementals.push(ElementalValue { element, value }),
        }
    }
    item.funcs.push(template);
}

fn function(kind: FuncKind, stat: Stat, value: f64) -> FuncTemplate {
    // Every function of an item is created with an explicit order 0x00 (not the kind's default).
    FuncTemplate::with_kind(None, None, kind, 0x00, stat, value)
}

/// The `<stats>` branch: one function per `<stat type=".." >value</stat>`, then (for weapons and
/// armor) the automatic enchant functions. An unknown stat name, a missing `type` attribute or a
/// bad number fails the item, like in Java.
pub(super) fn apply_stats_node(item: &mut ItemTemplate, node: &XmlNode) -> Result<(), DataError> {
    for stat_node in &node.children {
        if stat_node.name != "stat" {
            continue;
        }
        let type_name = stat_node
            .attr("type")
            .ok_or_else(|| DataError::Invalid("stat without type".into()))?;
        let stat = Stat::from_xml(type_name)
            .ok_or_else(|| DataError::Invalid(format!("Unknown name '{type_name}' for enum BaseStats")))?;
        let text = stat_node.text_content();
        let value = parse_double(&text).ok_or_else(|| {
            DataError::Invalid(format!("number required for stat {type_name}, found: {text}"))
        })?;

        let kind = match stat {
            Stat::PowerAttack
            | Stat::PowerAttackSpeed
            | Stat::PowerAttackRange
            | Stat::CriticalRate
            | Stat::MagicAttack
            | Stat::ShieldDefence
            | Stat::ShieldRate => FuncKind::Set,
            Stat::AccuracyCombat
            | Stat::EvasionRate
            | Stat::RandomDamage
            | Stat::PowerDefence
            | Stat::MagicDefence
            | Stat::MaxMp
            | Stat::FirePower
            | Stat::WaterPower
            | Stat::WindPower
            | Stat::EarthPower
            | Stat::HolyPower
            | Stat::DarkPower
            | Stat::FireRes
            | Stat::WaterRes
            | Stat::WindRes
            | Stat::EarthRes
            | Stat::HolyRes
            | Stat::DarkRes => FuncKind::Add,
            Stat::MagicSuccessRes => FuncKind::Mul,
            other => {
                tracing::warn!("Unhandled stat type {}", other.java_name());
                continue;
            }
        };
        attach(item, function(kind, stat, value));
    }

    // "Enable enchant functions."
    // NOTE(java-quirk): Java guards this with `hasFunction(FuncEnchant.class)`, which compares the
    // class of a `FuncTemplate` with `FuncEnchant` and so is always false. A second `<stats>`
    // block of the same item therefore adds the enchant functions again; kept.
    match &item.kind {
        ItemKind::Armor(armor) => {
            let stat = if armor.armor_type == ArmorType::Shield {
                Stat::ShieldDefence
            } else if item.type1 == TYPE1_WEAPON_RING_EARRING_NECKLACE && item.type2 == TYPE2_ACCESSORY {
                Stat::MagicDefence
            } else {
                Stat::PowerDefence
            };
            attach(item, function(FuncKind::Enchant, stat, 0.0));
        }
        ItemKind::Weapon(_) => {
            attach(item, function(FuncKind::Enchant, Stat::PowerAttack, 0.0));
            attach(item, function(FuncKind::Enchant, Stat::MagicAttack, 0.0));
        }
        ItemKind::Etc(_) => {}
    }
    Ok(())
}
