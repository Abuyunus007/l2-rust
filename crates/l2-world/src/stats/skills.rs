//! The stat side of skills: what `Creature.addSkill` / `removeSkill` do to the calculators, the
//! passive skills' effect functions, and the skills the inventory listeners and the penalty
//! checks give a player.
// Java: entity/actor/Creature.java (addSkill, removeSkill, the NPC constructor's template skills)
// Java: mechanics/skill/Skill.java (applyEffects with passive = true, getStatFuncs, checkCondition)
// Java: entity/actor/holders/creature/EffectList.java (add for a passive skill)
// Java: mechanics/skill/BuffInfo.java (initializeEffects, removeStats)
// Java: mechanics/effects/AbstractEffect.java (getStatFuncs), handlers/skill/effects/* (canStart,
//       isInstant, and the onStart of EnableCloak, TalismanSlot, CrystalGradeModify)
// Java: entity/itemcontainer/Inventory.java (ArmorSetListener, ItemSkillsListener)
// Java: entity/actor/Player.java (refreshOverloaded, refreshExpertisePenalty)
//
// A passive skill's effect functions are owned by `FuncOwner::Skill { id, level }`. Java owns
// them by the effect objects and removes them with `BuffInfo.removeStats` (every effect and the
// skill); removing everything of the skill's id and level is the same.
//
// Only the stat part is ported here. What the skill system of M5 brings: skill reuse timestamps
// of active item skills, `SkillList` updates, the system messages of a failed passive condition,
// effects other than stat functions (the `onStart` of every handler except the three named
// above), and skills of summons.

use l2_data::SkillTemplate;
use l2_data::item::{CrystalType, EtcItemType};
use l2_data::skill::{EffectScope, EffectTemplate};
use l2_data::stats::AbnormalType;
use l2_protocol_game::server::charscreen::paperdoll;
use l2_util::ObjectId;
use tracing::warn;

use super::calculators::{Func, FuncOwner};
use super::conditions::{self, CondEnv};
use super::creature_stat::StatView;
use super::{CalcSkill, StatData};
use crate::player::ItemData;
use crate::world::World;

/// `CommonSkill.WEAPON_GRADE_PENALTY`, `ARMOR_GRADE_PENALTY`, the weight penalty skill and
/// Expertise (`getExpertiseLevel()`).
pub const WEAPON_GRADE_PENALTY: i32 = 6209;
pub const ARMOR_GRADE_PENALTY: i32 = 6213;
pub const WEIGHT_PENALTY: i32 = 4270;
pub const EXPERTISE: i32 = 239;

/// The effect handlers whose `isInstant()` is true: they never stay on a passive skill.
const INSTANT_EFFECTS: &[&str] = &[
    "AddHate",
    "ApplySkillEffects",
    "Backstab",
    "Blink",
    "Bluff",
    "CallParty",
    "CallPc",
    "CallSkill",
    "ChangeFace",
    "ChangeHairColor",
    "ChangeHairStyle",
    "Confuse",
    "ConsumeBody",
    "ConvertItem",
    "CpDamPercent",
    "CpHeal",
    "CpHealPercent",
    "DeathLink",
    "DeleteHate",
    "DeleteHateOfMe",
    "DetectHiddenObjects",
    "Detection",
    "DispelAll",
    "DispelByCategory",
    "DispelBySlot",
    "DispelBySlotProbability",
    "EnemyCharge",
    "EnergyDamage",
    "Escape",
    "FatalBlow",
    "Fishing",
    "FlipBlock",
    "FocusEnergy",
    "FocusMaxEnergy",
    "FocusSouls",
    "GetAgro",
    "GiveRecommendation",
    "GiveSp",
    "Harvesting",
    "HeadquarterCreate",
    "Heal",
    "HealPercent",
    "HpByLevel",
    "HpDrain",
    "Lethal",
    "MagicalDamage",
    "MagicalDamageByAbnormal",
    "MagicalDamageMp",
    "MagicalSoulDamage",
    "ManaHeal",
    "ManaHealByLevel",
    "ManaHealPercent",
    "OpenChest",
    "OpenCommonRecipeBook",
    "OpenDoor",
    "OpenDwarfRecipeBook",
    "OutpostCreate",
    "OutpostDestroy",
    "PhysicalDamage",
    "PhysicalDamageHpLink",
    "PhysicalSoulDamage",
    "Pumping",
    "RandomizeHate",
    "RebalanceHP",
    "Recovery",
    "Reeling",
    "RefuelAirship",
    "Restoration",
    "RestorationRandom",
    "Resurrection",
    "Ride",
    "RunAway",
    "SetSkill",
    "SkillTurning",
    "SoulBlow",
    "Sow",
    "Spoil",
    "StaticDamage",
    "StealAbnormal",
    "Summon",
    "SummonAgathion",
    "SummonCubic",
    "SummonNpc",
    "SummonPet",
    "SummonTrap",
    "Sweeper",
    "TakeCastle",
    "TakeFort",
    "TakeFortStart",
    "TakeTerritoryFlag",
    "TargetCancel",
    "TargetMeProbability",
    "Teleport",
    "TeleportToTarget",
    "ThrowUp",
    "TransferHate",
    "TrapDetect",
    "TrapRemove",
    "Unsummon",
    "UnsummonAgathion",
    "VitalityPointUp",
];

/// Java `SkillData.getSkill(id, level)` without its log lines: a level above the skill's highest
/// gives the highest. The callers log what Java logs at their place (`restoreSkills`, NpcData
/// once per template at load time).
fn lookup(data: &StatData, skill_id: i32, level: i32) -> Option<&SkillTemplate> {
    data.skills.get(skill_id, level).or_else(|| {
        let max = data.skills.max_level(skill_id);
        (max > 0 && level > max)
            .then(|| data.skills.get(skill_id, max))
            .flatten()
    })
}

/// The skill as the calculation sees it.
pub(crate) fn calc_skill(skill: &SkillTemplate) -> CalcSkill {
    CalcSkill {
        id: skill.id,
        level: skill.level,
        magic: skill.is_magic(),
    }
}

/// `AbstractEffect.canStart(effector, effected, skill)` of the handler, for a passive skill (the
/// effector is the effected).
fn can_start(effect: &EffectTemplate, view: &StatView<'_>) -> bool {
    match effect.name.as_str() {
        "Passive" => view.is_attackable(),
        "CrystalGradeModify"
        | "EnableCloak"
        | "TalismanSlot"
        | "Lucky"
        | "CubicMastery"
        | "EnlargeAbnormalSlot"
        | "NevitsHourglass"
        | "Disarm"
        | "BlockAction"
        | "BlockChat"
        | "BlockParty"
        | "ProtectionBlessing"
        | "Flag"
        | "Transformation" => view.is_player(),
        "NoblesseBless" => view.is_playable(),
        // `effector.isPlayer() && effected.isSummon()`.
        "Betray" => false,
        // Players, and attackables that are no siege NPCs.
        "Fear" => {
            view.is_player()
                || (view.is_attackable()
                    && !view.npc_template().is_some_and(|t| {
                        matches!(t.npc_type.as_str(), "Defender" | "FortCommander" | "SiegeFlag")
                            || t.race == l2_data::npc::NpcRace::SiegeWeapon
                    }))
        }
        _ => true,
    }
}

/// The effects of a passive skill that stay on its owner (`applyEffectScope(PASSIVE, info,
/// false, true)`: not instant, `canStart`).
fn lasting_passive_effects<'a>(
    skill: &'a SkillTemplate,
    view: &'a StatView<'_>,
) -> impl Iterator<Item = &'a EffectTemplate> + 'a {
    skill
        .effects
        .get(EffectScope::Passive)
        .iter()
        .filter(|e| !INSTANT_EFFECTS.contains(&e.name.as_str()))
        .filter(move |e| can_start(e, view))
}

/// `Skill.checkCondition(creature, creature, false)` for a passive skill.
fn check_condition(skill: &SkillTemplate, view: &StatView<'_>, skill_ctx: &CalcSkill) -> bool {
    // "creature.isGM() && !GM_SKILL_RESTRICTION". (Fake players are not ported; mounted players
    // only fail for skills with negative effects, which passives are not.)
    if view.is_gm() && !view.settings().gm_skill_restriction {
        return true;
    }
    skill.pre_conditions.iter().all(|c| {
        conditions::test(
            &c.condition,
            &CondEnv {
                actor: view,
                target: Some(view.object_id()),
                skill: Some(skill_ctx),
            },
        )
    })
}

/// The functions a passive skill gives its owner (`EffectList.add` -> `initializeEffects` ->
/// `effect.getStatFuncs(effector, effected, skill)`). `None` when the skill's conditions fail
/// (the passive is then not applied at all).
fn passive_funcs(skill: &SkillTemplate, view: &StatView<'_>) -> Option<Vec<Func>> {
    let ctx = calc_skill(skill);
    if !check_condition(skill, view, &ctx) {
        return None;
    }
    let owner = FuncOwner::Skill {
        id: skill.id,
        level: skill.level,
    };
    let env = CondEnv {
        actor: view,
        target: Some(view.object_id()),
        skill: Some(&ctx),
    };
    let funcs = lasting_passive_effects(skill, view)
        .flat_map(|e| e.funcs.iter())
        .filter(|t| t.attach_cond.as_deref().is_none_or(|c| conditions::test(c, &env)))
        .map(|t| Func::from_template(t, owner))
        .collect();
    Some(funcs)
}

impl<'w> StatView<'w> {
    /// Java `isAttackable()`: monsters, guards, friendly mobs, siege defenders, territory wards.
    pub fn is_attackable(&self) -> bool {
        self.npc_template().is_some_and(|t| {
            matches!(
                t.npc_type.as_str(),
                "Attackable"
                    | "Monster"
                    | "RaidBoss"
                    | "GrandBoss"
                    | "Chest"
                    | "EventMonster"
                    | "FeedableBeast"
                    | "RiftInvader"
                    | "FestivalMonster"
                    | "Block"
                    | "FriendlyMob"
                    | "Guard"
                    | "Defender"
                    | "FortCommander"
                    | "TerritoryWard"
            )
        })
    }

    /// The applied passive skills (Java `EffectList._passives`) with their templates.
    fn passive_templates(&self) -> impl Iterator<Item = &'w SkillTemplate> + 'w {
        let skills = self.stat_data().map(|d| &d.skills);
        self.core()
            .passives
            .iter()
            .filter_map(move |&(id, level)| skills?.get(id, level))
    }

    /// The parameter `key` of every lasting passive effect named `name`.
    fn passive_effect_params(&self, name: &str, key: &str) -> Vec<i32> {
        let mut out = Vec::new();
        for skill in self.passive_templates() {
            for effect in lasting_passive_effects(skill, self) {
                if effect.name == name {
                    out.push(effect.param(key).and_then(|v| v.trim().parse().ok()).unwrap_or(0));
                }
            }
        }
        out
    }

    /// `PlayerStat._cloakSlot`: set by an `EnableCloak` passive effect.
    pub(crate) fn cloak_slot(&self) -> bool {
        !self.passive_effect_params("EnableCloak", "").is_empty()
    }

    /// `PlayerStat.getTalismanSlots()`: the `slots` of every `TalismanSlot` passive effect.
    pub fn talisman_slots(&self) -> i32 {
        if !self.is_player() {
            return 0;
        }
        self.passive_effect_params("TalismanSlot", "slots").iter().sum()
    }

    /// `Player._expertisePenaltyBonus`: the `grade` of the last `CrystalGradeModify` effect.
    pub fn expertise_penalty_bonus(&self) -> i32 {
        self.passive_effect_params("CrystalGradeModify", "grade")
            .last()
            .copied()
            .unwrap_or(0)
    }
}

/// Java `HashMap<Integer, V>` iteration order of `keys` (in insertion order, no duplicates):
/// by bucket `(h ^ (h >>> 16)) & (capacity - 1)` of the final capacity, then insertion order
/// within a bucket. NPC templates keep their skills in such a map.
// NOTE: a bucket that grows to 8 entries in a table of 64 or more becomes a tree in Java, which
// changes the order within that bucket; NPC skill lists are far too small for that.
pub(crate) fn java_hash_map_order(keys: &[i32]) -> Vec<i32> {
    let mut capacity: usize = 16;
    while keys.len() > capacity * 3 / 4 {
        capacity *= 2;
    }
    let bucket = |k: i32| {
        let h = k as u32;
        ((h ^ (h >> 16)) as usize) & (capacity - 1)
    };
    let mut indexed: Vec<(usize, usize, i32)> =
        keys.iter().enumerate().map(|(i, &k)| (bucket(k), i, k)).collect();
    indexed.sort_unstable();
    indexed.into_iter().map(|(_, _, k)| k).collect()
}

impl World {
    fn skill_templates(&self) -> Option<&StatData> {
        self.data.stat_data()
    }

    /// Java `Creature.addSkill(skill)`, the stat part: the skill replaces a known one of the same
    /// id (its functions and passive effects go), then a passive skill applies its effects.
    /// `false` when the skill does not exist (Java skips it with a warning).
    pub fn add_skill(&mut self, id: ObjectId, skill_id: i32, level: i32) -> bool {
        let Some((skill_id, level, passive)) = self
            .skill_templates()
            .and_then(|d| lookup(d, skill_id, level))
            .map(|s| (s.id, s.level, s.is_passive()))
        else {
            return false;
        };
        // `_skills.put(id, skill)`: the old one of that id, if any.
        let Some(core) = self.core_mut(id) else {
            return false;
        };
        let old = match core.skills.iter_mut().find(|(s, _)| *s == skill_id) {
            Some(entry) => Some(std::mem::replace(entry, (skill_id, level))),
            None => {
                core.skills.push((skill_id, level));
                None
            }
        };
        if let Some((_, old_level)) = old {
            // `removeStatsOwner(oldSkill)` and, for a passive, `stopSkillEffects(id)`.
            core.calculators.remove_owner(FuncOwner::Skill {
                id: skill_id,
                level: old_level,
            });
            core.passives.retain(|(s, _)| *s != skill_id);
        }
        // `addStatFuncs(newSkill.getStatFuncs(null, this))`: skills carry no functions of their
        // own in the H5 data (DocumentSkill only attaches functions to effects).
        if passive {
            self.apply_passive(id, skill_id, level);
        }
        true
    }

    /// `Skill.applyEffects(this, this, false, true, false, 0)` -> `EffectList.add(info)`.
    fn apply_passive(&mut self, id: ObjectId, skill_id: i32, level: i32) {
        let funcs = {
            let Some(view) = self.stats(id) else {
                return;
            };
            let Some(skill) = view.stat_data().and_then(|d| d.skills.get(skill_id, level)) else {
                return;
            };
            if skill.abnormal_type != AbnormalType::None {
                warn!(
                    "Passive {}({skill_id}/{level}) with abnormal type: {}!",
                    skill.name,
                    skill.abnormal_type.name()
                );
            }
            match passive_funcs(skill, &view) {
                Some(funcs) => funcs,
                None => return,
            }
        };
        let Some(core) = self.core_mut(id) else {
            return;
        };
        // "Removes the old stats from the creature if the skill was present."
        let old: Vec<(i32, i32)> = core
            .passives
            .iter()
            .copied()
            .filter(|(s, _)| *s == skill_id)
            .collect();
        for (s, l) in old {
            core.calculators
                .remove_owner(FuncOwner::Skill { id: s, level: l });
        }
        core.passives.retain(|(s, _)| *s != skill_id);
        core.passives.push((skill_id, level));
        core.calculators.add_all(funcs);
    }

    /// Java `Creature.removeSkill(skillId, cancelEffect)`, the stat part.
    pub fn remove_skill(&mut self, id: ObjectId, skill_id: i32, cancel_effect: bool) -> bool {
        let Some(core) = self.core_mut(id) else {
            return false;
        };
        let Some(at) = core.skills.iter().position(|(s, _)| *s == skill_id) else {
            return false;
        };
        let (_, level) = core.skills.remove(at);
        core.calculators
            .remove_owner(FuncOwner::Skill { id: skill_id, level });
        if cancel_effect {
            let passives: Vec<(i32, i32)> = core
                .passives
                .iter()
                .copied()
                .filter(|(s, _)| *s == skill_id)
                .collect();
            for (s, l) in passives {
                core.calculators
                    .remove_owner(FuncOwner::Skill { id: s, level: l });
            }
            core.passives.retain(|(s, _)| *s != skill_id);
        }
        true
    }

    /// `removeSkill(skill, false, skill.isPassive())`.
    fn remove_skill_of(&mut self, id: ObjectId, skill_id: i32, level: i32) {
        let passive = self
            .skill_templates()
            .and_then(|d| lookup(d, skill_id, level))
            .is_some_and(SkillTemplate::is_passive);
        self.remove_skill(id, skill_id, passive);
    }

    fn core_mut(&mut self, id: ObjectId) -> Option<&mut crate::player::CreatureCore> {
        if let Some(p) = self.players.get_mut(&id) {
            return Some(&mut p.core);
        }
        self.npcs.get_mut(&id).map(|n| &mut n.core)
    }

    /// The NPC constructor: `for (Skill skill : template.getSkills().values()) addSkill(skill)`,
    /// in the order of Java's `HashMap` (NpcData keeps the last entry of each id).
    pub(crate) fn add_npc_template_skills(&mut self, id: ObjectId) {
        let Some(template) = self.npcs.get(&id).and_then(|n| self.npc_world.template(n.npc_id)) else {
            return;
        };
        let Some(data) = self.skill_templates() else {
            return;
        };
        // `skills.put(skill.getId(), skill)` for every skill that exists.
        let mut order: Vec<i32> = Vec::new();
        let mut levels: std::collections::HashMap<i32, i32> = std::collections::HashMap::new();
        for s in &template.skills {
            let Some(skill) = lookup(data, s.id, s.level) else {
                continue;
            };
            if levels.insert(skill.id, skill.level).is_none() {
                order.push(skill.id);
            }
        }
        for skill_id in java_hash_map_order(&order) {
            if let Some(&level) = levels.get(&skill_id) {
                self.add_skill(id, skill_id, level);
            }
        }
    }

    /// The skill parts of `ArmorSetListener.notifyEquiped` and `ItemSkillsListener.notifyEquiped`
    /// for `item` in paperdoll `slot`.
    pub(crate) fn equip_item_skills(&mut self, id: ObjectId, item: &ItemData) {
        let slot = usize::try_from(item.loc_data).unwrap_or(usize::MAX);
        let mut add: Vec<(i32, i32)> = Vec::new();
        {
            let Some(view) = self.stats(id) else {
                return;
            };
            let Some(data) = view.stat_data() else {
                return;
            };
            // ArmorSetListener: the set of the worn chest.
            if let Some(chest) = view.paperdoll(paperdoll::CHEST)
                && let Some(set) = data.armor_sets.get(chest.item_id)
            {
                if contain_item(set, slot, item.item_id) {
                    if view.armor_set_complete(set) {
                        add.extend(set.skills.iter().map(|h| (h.id, h.level)));
                        if view
                            .paperdoll(paperdoll::LHAND)
                            .is_some_and(|s| set.shield.contains(&s.item_id))
                        {
                            add.extend(set.shield_skills.iter().map(|h| (h.id, h.level)));
                        }
                        if view.armor_set_enchanted6(set) {
                            add.extend(set.enchant6_skills.iter().map(|h| (h.id, h.level)));
                        }
                    }
                } else if set.shield.contains(&item.item_id) {
                    add.extend(set.shield_skills.iter().map(|h| (h.id, h.level)));
                }
            }
            // ItemSkillsListener: the +4 skill, then the item's own skills.
            if let Some(template) = data.items.get(item.item_id) {
                // Augmentation bonuses, element attributes and enchant options (`applyEnchantStats`)
                // are not loaded yet.
                if item.enchant_level >= 4
                    && let Some(s) = template.skills.enchant4_skill
                {
                    add.push((s.id, s.level));
                }
                for s in template.skills.item_skills.iter().flatten() {
                    add.push((s.id, s.level));
                }
            }
        }
        for (skill_id, level) in add {
            if !self.add_skill(id, skill_id, level) {
                warn!("Inventory: Incorrect skill: {skill_id}-{level}.");
            }
        }
    }

    /// The skill parts of `ArmorSetListener.notifyUnequiped` and `ItemSkillsListener
    /// .notifyUnequiped`. `item` has already left its slot (Java clears the slot first).
    pub(crate) fn unequip_item_skills(&mut self, id: ObjectId, item: &ItemData, slot: usize) {
        let mut remove: Vec<(i32, i32)> = Vec::new();
        let mut readd: Vec<(i32, i32)> = Vec::new();
        {
            let Some(view) = self.stats(id) else {
                return;
            };
            let Some(data) = view.stat_data() else {
                return;
            };
            // ArmorSetListener.
            let set = if slot == paperdoll::CHEST {
                data.armor_sets.get(item.item_id)
            } else {
                view.paperdoll(paperdoll::CHEST)
                    .and_then(|c| data.armor_sets.get(c.item_id))
            };
            if let Some(set) = set {
                let skills = || set.skills.iter().map(|h| (h.id, h.level));
                let shield = || set.shield_skills.iter().map(|h| (h.id, h.level));
                let six = || set.enchant6_skills.iter().map(|h| (h.id, h.level));
                if slot == paperdoll::CHEST || contain_item(set, slot, item.item_id) {
                    remove.extend(skills());
                    remove.extend(shield());
                    remove.extend(six());
                } else if set.shield.contains(&item.item_id) {
                    remove.extend(shield());
                }
            }
            // ItemSkillsListener.
            if let Some(template) = data.items.get(item.item_id) {
                if item.enchant_level >= 4
                    && let Some(s) = template.skills.enchant4_skill
                {
                    remove.push((s.id, s.level));
                }
                for s in template.skills.item_skills.iter().flatten() {
                    remove.push((s.id, s.level));
                }
                // An armor taken off gives the skills of the other worn items back that it took.
                if template.is_armor() {
                    if let Some(p) = view.player() {
                        for other in p
                            .data
                            .items
                            .iter()
                            .filter(|i| i.loc == "PAPERDOLL" && i.object_id != item.object_id)
                        {
                            if let Some(t) = data.items.get(other.item_id) {
                                for s in t.skills.item_skills.iter().flatten() {
                                    readd.push((s.id, s.level));
                                }
                            }
                        }
                    }
                }
            }
        }
        for (skill_id, level) in remove {
            self.remove_skill_of(id, skill_id, level);
        }
        for (skill_id, level) in readd {
            if self.stats(id).is_some_and(|v| v.skill_level(skill_id) == 0) {
                self.add_skill(id, skill_id, level);
            }
        }
        // The unequip skill (`activateSkill`) is an active skill: M5.
    }

    /// Java `Player.refreshOverloaded()`, the skill part: the weight penalty skill of the load.
    /// Returns whether the penalty level changed.
    pub fn refresh_overloaded(&mut self, id: ObjectId) -> bool {
        let Some(view) = self.stats(id) else {
            return false;
        };
        let max_load = view.max_load();
        if max_load <= 0 {
            return false;
        }
        let weight_proc =
            i64::from(view.current_load() - view.bonus_weight_penalty()) * 1000 / i64::from(max_load);
        // `_dietMode` (the admin diet command) is not modelled: off.
        let penalty = match weight_proc {
            ..500 => 0,
            500..666 => 1,
            666..800 => 2,
            800..1000 => 3,
            _ => 4,
        };
        let Some(player) = self.players.get_mut(&id) else {
            return false;
        };
        if player.weight_penalty == penalty {
            return false;
        }
        player.weight_penalty = penalty;
        if penalty > 0 {
            self.add_skill(id, WEIGHT_PENALTY, penalty);
        } else {
            self.remove_skill(id, WEIGHT_PENALTY, true);
        }
        true
    }

    /// Java `Player.refreshExpertisePenalty()`: the grade penalty skills of the worn items above
    /// the character's Expertise. Returns whether a penalty changed.
    pub fn refresh_expertise_penalty(&mut self, id: ObjectId) -> bool {
        if !self.settings.stats.expertise_penalty {
            return false;
        }
        let Some(view) = self.stats(id) else {
            return false;
        };
        let Some(player) = view.player() else {
            return false;
        };
        let expertise = view.skill_level(EXPERTISE);
        let (mut weapon_penalty, mut armor_penalty) = (0, 0);
        for item in player.data.items.iter().filter(|i| i.loc == "PAPERDOLL") {
            let Some(t) = view.item_template(item.item_id) else {
                continue;
            };
            if t.etc()
                .is_some_and(|e| matches!(e.etc_type, EtcItemType::Arrow | EtcItemType::Bolt))
            {
                continue;
            }
            let grade = crystal_level(t.crystal_type);
            if grade > expertise {
                if t.is_weapon() && grade > weapon_penalty {
                    weapon_penalty = grade;
                } else if grade > armor_penalty {
                    armor_penalty = grade;
                }
            }
        }
        let bonus = view.expertise_penalty_bonus();
        let weapon_penalty = (weapon_penalty - expertise - bonus).clamp(0, 4);
        let armor_penalty = (armor_penalty - expertise - bonus).clamp(0, 4);
        let (weapon_known, armor_known) = (
            view.skill_level(WEAPON_GRADE_PENALTY),
            view.skill_level(ARMOR_GRADE_PENALTY),
        );
        let (old_weapon, old_armor) = (player.expertise_weapon_penalty, player.expertise_armor_penalty);
        let mut changed = false;
        if old_weapon != weapon_penalty || weapon_known != weapon_penalty {
            if let Some(p) = self.players.get_mut(&id) {
                p.expertise_weapon_penalty = weapon_penalty;
            }
            if weapon_penalty > 0 {
                self.add_skill(id, WEAPON_GRADE_PENALTY, weapon_penalty);
            } else {
                self.remove_skill(id, WEAPON_GRADE_PENALTY, true);
            }
            changed = true;
        }
        if old_armor != armor_penalty || armor_known != armor_penalty {
            if let Some(p) = self.players.get_mut(&id) {
                p.expertise_armor_penalty = armor_penalty;
            }
            if armor_penalty > 0 {
                self.add_skill(id, ARMOR_GRADE_PENALTY, armor_penalty);
            } else {
                self.remove_skill(id, ARMOR_GRADE_PENALTY, true);
            }
            changed = true;
        }
        changed
    }
}

/// `CrystalType.getLevel()`.
fn crystal_level(grade: CrystalType) -> i32 {
    grade.level()
}

/// `ArmorSet.containItem(slot, itemId)`.
fn contain_item(set: &l2_data::ArmorSet, slot: usize, item_id: i32) -> bool {
    match slot {
        paperdoll::CHEST => set.chest_id == item_id,
        paperdoll::LEGS => set.legs.contains(&item_id),
        paperdoll::HEAD => set.head.contains(&item_id),
        paperdoll::GLOVES => set.gloves.contains(&item_id),
        paperdoll::FEET => set.feet.contains(&item_id),
        _ => false,
    }
}

impl StatView<'_> {
    /// `ArmorSet.containAll(player)`: every listed part is worn (an empty list needs nothing).
    pub(crate) fn armor_set_complete(&self, set: &l2_data::ArmorSet) -> bool {
        let Some(chest) = self.paperdoll(paperdoll::CHEST) else {
            return false;
        };
        let id = |slot| self.paperdoll(slot).map_or(0, |i| i.item_id);
        let part = |ids: &[i32], slot| ids.is_empty() || ids.contains(&id(slot));
        (set.chest_id == 0 || set.chest_id == chest.item_id)
            && part(&set.legs, paperdoll::LEGS)
            && part(&set.head, paperdoll::HEAD)
            && part(&set.gloves, paperdoll::GLOVES)
            && part(&set.feet, paperdoll::FEET)
    }

    /// `ArmorSet.isEnchanted6(player)`: complete, and every worn part of the set at +6 or more.
    pub(crate) fn armor_set_enchanted6(&self, set: &l2_data::ArmorSet) -> bool {
        if !self.armor_set_complete(set) {
            return false;
        }
        let enchanted = |slot| self.paperdoll(slot).is_some_and(|i| i.enchant_level >= 6);
        if !enchanted(paperdoll::CHEST) {
            return false;
        }
        (set.legs.is_empty() || enchanted(paperdoll::LEGS))
            && (set.head.is_empty() || enchanted(paperdoll::HEAD))
            && (set.gloves.is_empty() || enchanted(paperdoll::GLOVES))
            && (set.feet.is_empty() || enchanted(paperdoll::FEET))
    }
}
