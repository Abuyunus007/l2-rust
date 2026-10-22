//! The player-only parts of the stat getters, the player's equipment as the formulas see it, and
//! applying item functions when items are equipped.
// Java: entity/actor/stat/{PlayableStat,PlayerStat}.java (getMaxHp/Mp/Cp, getRunSpeed,
//       getWalkSpeed, getPAtkSpd, canEquipCloak)
// Java: entity/actor/Player.java (getActiveWeaponItem, findFistsWeaponItem, getChestArmorInstance,
//       recalcHennaStats, getHennaStat*, getCollisionRadius/Height, getInventoryLimit)
// Java: entity/itemcontainer/Inventory.java (StatsListener, getWearedMask, isPaperdollSlotEmpty)
// Java: entity/item/ItemTemplate.java (getStatFuncs), data/xml/EnchantItemHPBonusData.java (the
//       ENCHANTHP template it attaches to armors)
// Java: data/holders/ArmorSet.java (containAll), mechanics/stats/functions/formulas/FuncArmorSet

use l2_data::item::{BodyPart, CrystalType};
use l2_data::{FuncKind, FuncTemplate, Henna, ItemTemplate, PlayerTemplate, Stat};
use l2_protocol_game::server::charscreen::paperdoll;
use l2_util::ObjectId;
use tracing::warn;

use super::calculators::{CalcSet, Calculators, Func, FuncOwner};
use super::creature_stat::{StatView, Who};
use crate::player::ItemData;
use crate::world::World;

/// Java `Inventory` item location of a worn item.
const LOC_PAPERDOLL: &str = "PAPERDOLL";

/// Java `Player._hennaSTR` ...: the dye bonuses of the six base stats.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct HennaStats {
    pub str: i32,
    pub con: i32,
    pub dex: i32,
    pub int: i32,
    pub wit: i32,
    pub men: i32,
}

impl HennaStats {
    /// Java `Player.recalcHennaStats()`.
    // NOTE(java-quirk): every stat is capped at 5 per dye as it is added, so a negative dye that
    // comes after the cap was reached still lowers it (`5 - total` is negative then too).
    pub fn recalc<'a>(hennas: impl IntoIterator<Item = &'a Henna>) -> Self {
        let mut s = Self::default();
        let add = |total: &mut i32, value: i32| {
            *total += if *total + value > 5 { 5 - *total } else { value };
        };
        for h in hennas {
            add(&mut s.int, h.stat_int);
            add(&mut s.str, h.stat_str);
            add(&mut s.men, h.stat_men);
            add(&mut s.con, h.stat_con);
            add(&mut s.wit, h.stat_wit);
            add(&mut s.dex, h.stat_dex);
        }
        s
    }

    /// The bonus of one base stat (0 for any other stat).
    pub fn of(&self, stat: Stat) -> i32 {
        match stat {
            Stat::StatStr => self.str,
            Stat::StatCon => self.con,
            Stat::StatDex => self.dex,
            Stat::StatInt => self.int,
            Stat::StatWit => self.wit,
            Stat::StatMen => self.men,
            _ => 0,
        }
    }
}

/// Java `Player.findFistsWeaponItem(classId)`: the weapon an unarmed character fights with.
pub fn fists_item_id(class_id: i32) -> i32 {
    match class_id {
        0x00..=0x09 => 246,
        0x0a..=0x11 => 251,
        0x12..=0x18 => 244,
        0x19..=0x1e => 249,
        0x1f..=0x25 => 245,
        0x26..=0x2b => 250,
        0x2c..=0x30 => 248,
        0x31..=0x34 => 252,
        0x35..=0x39 => 247,
        // "Default fist item to avoid NPEs."
        _ => 246,
    }
}

/// Java `EnchantItemHPBonusData`: the body parts whose armors get the `ENCHANTHP` function.
fn takes_enchant_hp(body_part: BodyPart) -> bool {
    matches!(
        body_part,
        BodyPart::Chest
            | BodyPart::Feet
            | BodyPart::Gloves
            | BodyPart::Head
            | BodyPart::Legs
            | BodyPart::Back
            | BodyPart::FullArmor
            | BodyPart::Underwear
            | BodyPart::LeftHand
            | BodyPart::Belt
    )
}

impl<'w> StatView<'w> {
    /// The player's class template (Java `getTemplate()`: the active class).
    pub fn player_template(&self) -> Option<&'w PlayerTemplate> {
        let p = self.player()?;
        self.stat_data()?.player_templates.get_by_id(p.data.class_id)
    }

    /// Java `getBaseTemplate()`: the template of the base class.
    pub fn base_template(&self) -> Option<&'w PlayerTemplate> {
        let p = self.player()?;
        self.stat_data()?.player_templates.get_by_id(p.data.base_class)
    }

    /// A per-level template value (`getBaseHpMax(level)` ...).
    // NOTE(java-quirk): Java indexes the level table directly and throws for a level past it; such
    // a level reads as 0 here.
    pub(crate) fn player_level_value(&self, value: impl Fn(&PlayerTemplate, usize) -> Option<f32>) -> f32 {
        let level = usize::try_from(self.level()).unwrap_or(0);
        self.player_template()
            .and_then(|t| value(t, level))
            .unwrap_or(0.0)
    }

    /// `PlayerStat.getRunSpeed()` / `getWalkSpeed()` after `CreatureStat`: `RunSpeedBoost` and
    /// the `MaxRunSpeed` cap (not for GMs). Mount penalties come with mounts. Other creatures:
    /// unchanged.
    // Java `PlayableStat` also multiplies by a swamp zone's move bonus; swamp zones do not set
    // their flag yet (see `zones.rs`), so it never applies.
    pub(crate) fn player_speed_cap(&self, value: f64) -> f64 {
        if !self.is_player() {
            return value;
        }
        let s = self.settings();
        let value = value + f64::from(s.run_spd_boost);
        if value > f64::from(s.max_run_speed) && !self.is_gm() {
            return f64::from(s.max_run_speed);
        }
        value
    }

    /// Java `isGM()`.
    pub fn is_gm(&self) -> bool {
        self.player()
            .is_some_and(|p| self.world().data.access_level(p.data.access_level).is_gm)
    }

    /// The collision of the base class template for the character's sex.
    pub(crate) fn player_collision(&self) -> (f32, f32) {
        let female = self.player().is_some_and(|p| p.data.sex != 0);
        self.base_template().map_or((0.0, 0.0), |t| {
            let c = if female {
                t.collision_female
            } else {
                t.collision_male
            };
            (c.radius, c.height)
        })
    }

    // ------------------------------------------------------------------ equipment

    /// The template of an item id.
    pub(crate) fn item_template(&self, item_id: i32) -> Option<&'w ItemTemplate> {
        self.stat_data()?.items.get(item_id)
    }

    /// `getInventory().getPaperdollItem(slot)`.
    pub fn paperdoll(&self, slot: usize) -> Option<&'w ItemData> {
        let p = self.player()?;
        let slot = i32::try_from(slot).ok()?;
        p.data
            .items
            .iter()
            .find(|i| i.loc == LOC_PAPERDOLL && i.loc_data == slot)
    }

    /// The template of the item in a paperdoll slot.
    pub(crate) fn paperdoll_template(&self, slot: usize) -> Option<&'w ItemTemplate> {
        self.item_template(self.paperdoll(slot)?.item_id)
    }

    /// One of the player's items by object id, with its template (Java: the `Item` a function
    /// belongs to).
    pub(crate) fn item(&self, object_id: ObjectId) -> Option<(&'w ItemData, Option<&'w ItemTemplate>)> {
        let p = self.player()?;
        let item = p.data.items.iter().find(|i| i.object_id == object_id)?;
        Some((item, self.item_template(item.item_id)))
    }

    /// Java `getActiveWeaponInstance()`: the right hand item of a player, nothing for an NPC.
    pub fn active_weapon_instance(&self) -> Option<&'w ItemData> {
        self.paperdoll(paperdoll::RHAND)
    }

    /// Java `getActiveWeaponItem()`: a player's right hand weapon, else the class's fists; an
    /// NPC's template weapon; none for a summon (`Summon.getActiveWeaponItem()`).
    pub fn active_weapon_item(&self) -> Option<&'w ItemTemplate> {
        match self.who {
            Who::Player(p) => match self.active_weapon_instance() {
                Some(item) => self.item_template(item.item_id),
                None => self.item_template(fists_item_id(p.data.class_id)),
            },
            Who::Npc(_) if self.is_summon() => None,
            Who::Npc(_) => {
                let id = self.npc_template()?.rhand_id;
                if id < 1 {
                    return None;
                }
                self.item_template(id).filter(|t| t.is_weapon())
            }
        }
    }

    /// `Item.getAttackElementType()` of the right hand item: the template's first element, else
    /// the first element attribute of the instance.
    pub(crate) fn weapon_attack_element(&self) -> Option<i8> {
        let item = self.active_weapon_instance()?;
        let template = self.item_template(item.item_id)?;
        Some(item.attack_element_type(Some(template)) as i8)
    }

    /// Java `Inventory.getWearedMask()`: the item masks of everything worn.
    pub fn weared_mask(&self) -> i32 {
        let Some(p) = self.player() else {
            return 0;
        };
        p.data
            .items
            .iter()
            .filter(|i| i.loc == LOC_PAPERDOLL)
            .filter_map(|i| self.item_template(i.item_id))
            .fold(0, |mask, t| mask | t.item_mask())
    }

    /// Java `Player.getHennaStat*()`: `None` for anything but a player.
    pub fn henna_stats(&self) -> Option<HennaStats> {
        let p = self.player()?;
        let hennas = self.stat_data().map(|d| &d.hennas);
        Some(HennaStats::recalc(
            p.data
                .profile
                .hennas
                .iter()
                .filter(|id| **id != 0)
                .filter_map(|id| hennas?.get(*id)),
        ))
    }

    /// `FuncArmorSet`: the bonus of the set whose chest is worn, when every part is worn.
    pub fn armor_set_bonus(&self, stat: Stat) -> i32 {
        if !self.is_player() {
            return 0;
        }
        let Some(chest) = self.paperdoll(paperdoll::CHEST) else {
            return 0;
        };
        let Some(set) = self.stat_data().and_then(|d| d.armor_sets.get(chest.item_id)) else {
            return 0;
        };
        if !self.armor_set_complete(set) {
            return 0;
        }
        match stat {
            Stat::StatStr => set.str_,
            Stat::StatDex => set.dex,
            Stat::StatInt => set.int_,
            Stat::StatMen => set.men,
            Stat::StatCon => set.con,
            Stat::StatWit => set.wit,
            _ => 0,
        }
    }

    // ------------------------------------------------------------------ weight and inventory

    /// Java `getCurrentLoad()`.
    pub fn current_load(&self) -> i32 {
        // `Pet`: the weight of its own inventory.
        if self.is_pet() {
            return self
                .world()
                .summon_owner(self.object_id())
                .map_or(0, |o| self.world().pet_total_weight(o));
        }
        self.player()
            .map_or(0, |p| crate::enter::current_load(&p.data, &*self.world().data))
    }

    /// Java `Creature.getMaxLoad()`: `floor(CON bonus * 69000 * AltWeightLimit)` through
    /// `WEIGHT_LIMIT`; 0 for anything but a player (and pets, later).
    pub fn max_load(&self) -> i32 {
        if !self.is_player() && !self.is_pet() {
            return 0;
        }
        let base = (self.bonus(super::BaseStat::Con) * 69_000.0 * self.settings().alt_weight_limit).floor();
        self.calc_stat(Stat::WeightLimit, base, Some(self.object_id()), None) as i32
    }

    /// Java `getBonusWeightPenalty()`.
    pub fn bonus_weight_penalty(&self) -> i32 {
        if !self.is_player() && !self.is_pet() {
            return 0;
        }
        self.calc_stat(Stat::WeightPenalty, 1.0, Some(self.object_id()), None) as i32
    }

    /// Java `PlayerInventory.getNonQuestSize()`.
    pub fn non_quest_size(&self) -> i32 {
        let Some(p) = self.player() else {
            return 0;
        };
        let data = &*self.world().data;
        let count = p
            .data
            .items
            .iter()
            .filter(|i| i.loc == "INVENTORY" || i.loc == LOC_PAPERDOLL)
            .filter(|i| !data.item(i.item_id).is_some_and(|t| t.quest_item))
            .count();
        i32::try_from(count).unwrap_or(i32::MAX)
    }

    /// Java `Player.getInventoryLimit()`.
    pub fn inventory_limit(&self) -> i32 {
        let Some(p) = self.player() else {
            return 0;
        };
        let base = crate::enter::inventory_limit(&p.data, self.is_gm(), &self.world().settings);
        base + self.calc(Stat::InvLim, 0.0) as i32
    }

    /// Java `PlayerStat.canEquipCloak()`: always before the player entered the world, then
    /// `_cloakSlot` (an `EnableCloak` passive effect).
    pub fn can_equip_cloak(&self) -> bool {
        self.player()
            .is_some_and(|p| !p.entered_world || self.cloak_slot())
    }

    /// Java `isSubClassActive()` (`_classIndex > 0`).
    // The subclass tables are not loaded; a character whose class is not its base class plays a
    // subclass (that is how Java's `restore` finds the index).
    pub fn is_subclass_active(&self) -> bool {
        self.player()
            .is_some_and(|p| p.data.class_id != p.data.base_class)
    }
}

/// The functions an item brings when it is equipped (Java `Item.getStatFuncs(owner)`): its
/// template functions whose attach condition holds, then the `ENCHANTHP` function
/// `EnchantItemHPBonusData` appends to armors.
pub(crate) fn item_funcs(view: &StatView<'_>, item: &ItemData) -> Vec<Func> {
    let Some(data) = view.stat_data() else {
        return Vec::new();
    };
    let Some(template) = data.items.get(item.item_id) else {
        return Vec::new();
    };
    let owner = FuncOwner::Item(item.object_id);
    let env_holds = |t: &FuncTemplate| {
        t.attach_cond.as_deref().is_none_or(|cond| {
            super::conditions::test(
                cond,
                &super::conditions::CondEnv {
                    actor: view,
                    target: Some(view.object_id()),
                    skill: None,
                },
            )
        })
    };
    let mut funcs: Vec<Func> = template
        .funcs
        .iter()
        .filter(|t| env_holds(t))
        .map(|t| Func::from_template(t, owner))
        .collect();
    // Java attaches `new FuncTemplate(null, null, "EnchantHp", -1, MAX_HP, 0)` to every armor with
    // a grade in one of these body parts, once the bonus file has an entry.
    // NOTE: Java checks `getCrystalType() != NONE` (not the "plus" grade) and only attaches when
    // `enchantHPBonus.xml` loaded at least one grade; the empty table gives every item 0 anyway.
    if template.is_armor()
        && template.crystal_type != CrystalType::None
        && takes_enchant_hp(template.body_part)
    {
        let enchant_hp = FuncTemplate::with_kind(None, None, FuncKind::EnchantHp, -1, Stat::MaxHp, 0.0);
        funcs.push(Func::from_template(&enchant_hp, owner));
    }
    funcs
}

impl World {
    /// Java `Formulas.addFuncsToNewCharacter` + `Inventory.restore` (the `StatsListener` of every
    /// equipped item, in `ORDER BY loc_data`): the calculators a player enters the world with,
    /// then the max HP/MP/CP and the current values clamped to them (`Player.restore` sets the
    /// current values after the inventory).
    pub(crate) fn init_player_stats(&mut self, id: ObjectId) {
        // A new `Player` object: a new effect list (the passives join it below).
        self.forget_effects(id);
        let Some(player) = self.players.get_mut(&id) else {
            return;
        };
        player.core.calculators = Calculators::own(CalcSet::std_player());
        player.core.skills.clear();
        // The session loads the items `ORDER BY loc_data`, like `Inventory.restore`.
        let worn: Vec<ItemData> = player
            .data
            .items
            .iter()
            .filter(|i| i.loc == LOC_PAPERDOLL)
            .cloned()
            .collect();
        let learned: Vec<(i32, i32)> = player
            .data
            .skills
            .iter()
            .map(|s| (s.skill_id, s.skill_level))
            .collect();
        for item in worn {
            self.on_equip(id, &item);
        }
        // `restoreSkills()`: every stored skill (`SkillData.getSkill`, a missing one is skipped).
        // `rewardSkills()` (auto-get / auto-learn skills not stored yet) is the skill learning of M5.
        for (skill_id, level) in learned {
            if !self.add_skill(id, skill_id, level) {
                warn!(
                    "Skipped null skill Id: {skill_id} Level: {level} while restoring player skills for playerObjId: {id}"
                );
            }
        }
        // `refreshOverloaded()`, `refreshExpertisePenalty()` (Java sends nothing for them here: the
        // character is not in the world yet).
        self.refresh_overloaded(id);
        self.refresh_expertise_penalty(id);
        // Java `setCurrentCp/Hp/Mp(stored)` after the inventory: clamped to the new maxima.
        self.refresh_max_vitals(id, true);
    }

    /// Java `StatsListener.notifyEquiped`: the item's functions join the player's calculators.
    /// Returns the stats that changed (`addStatFuncs`), for the caller's `broadcastModifiedStats`.
    /// `item` must already be in the player's items (M4-INV-01 moves it there first, like
    /// `setPaperdollItem` does before it notifies the listeners).
    // Java's other listeners are not stat functions: `ArmorSetListener` and `ItemSkillsListener`
    // add skills (phase 2 of M4-STAT-01), `BowCrossRodListener` and `BraceletListener` move items
    // (M4-INV-01). The armor set *stat* bonus is `FuncArmorSet`, which reads the paperdoll.
    pub fn on_equip(&mut self, id: ObjectId, item: &ItemData) -> Vec<Stat> {
        // `ArmorSetListener`, then `ItemSkillsListener` (they come before `StatsListener`).
        // The augmentation and element bonuses are applied by `ItemSkillsListener`, so their
        // stats come first.
        let mut stats = self.equip_item_skills(id, item);
        let funcs = match self.stats(id) {
            Some(view) => item_funcs(&view, item),
            None => return stats,
        };
        let Some(player) = self.players.get_mut(&id) else {
            return stats;
        };
        stats.extend(player.core.calculators.add_all(funcs));
        stats
    }

    /// Java's paperdoll listeners on unequip: the armor set and item skills go
    /// (`ArmorSetListener`, `ItemSkillsListener`), then `StatsListener`'s
    /// `removeStatsOwner(item)`. `item` has already left `slot` (INV-01 clears the slot first,
    /// like `setPaperdollItem`). Returns the stats `removeStatsOwner` changed.
    pub fn on_unequip(&mut self, id: ObjectId, item: &ItemData, slot: usize) -> Vec<Stat> {
        let mut stats = self.unequip_item_skills(id, item, slot);
        let Some(player) = self.players.get_mut(&id) else {
            return stats;
        };
        stats.extend(
            player
                .core
                .calculators
                .remove_owner(FuncOwner::Item(item.object_id)),
        );
        stats
    }

    /// The max HP/MP/CP of a player or NPC into its `CreatureCore`, and the current values
    /// clamped to them (Java `PlayerStat.getMaxHp`'s "stats watch" calls `setCurrentHp`, which
    /// caps the value at the new maximum). `force` clamps even when the maximum did not change
    /// (entering the world).
    pub(crate) fn refresh_max_vitals(&mut self, id: ObjectId, force: bool) {
        let Some(view) = self.stats(id) else {
            return;
        };
        let (hp, mp, cp) = (
            f64::from(view.max_hp()),
            f64::from(view.max_mp()),
            f64::from(view.max_cp()),
        );
        let is_player = view.is_player();
        let core = if let Some(p) = self.players.get_mut(&id) {
            &mut p.core
        } else if let Some(n) = self.npcs.get_mut(&id) {
            &mut n.core
        } else {
            return;
        };
        // Java `setCurrentHp` does nothing for the dead (HP below 0.5 is dead in `restore`).
        let alive = core.cur_hp >= 0.5;
        let clamp = |cur: &mut f64, max: f64, old: f64| {
            if alive && (force || old != max) && *cur >= max {
                *cur = max;
            }
        };
        clamp(&mut core.cur_hp, hp, core.max_hp);
        clamp(&mut core.cur_mp, mp, core.max_mp);
        if is_player {
            // `PlayerStatus.setCurrentCp`: `Math.max(0, value)` first.
            if alive && (force || core.max_cp != cp) {
                core.cur_cp = core.cur_cp.max(0.0);
            }
            clamp(&mut core.cur_cp, cp, core.max_cp);
        }
        core.max_hp = hp;
        core.max_mp = mp;
        core.max_cp = cp;
    }
}
