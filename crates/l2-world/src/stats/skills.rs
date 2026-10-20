//! The stat side of skills: what `Creature.addSkill` / `removeSkill` do, and the skills the
//! inventory listeners and the penalty checks give a player.
// Java: entity/actor/Creature.java (addSkill, removeSkill, the NPC constructor's template skills)
// Java: mechanics/skill/Skill.java (applyEffects with passive = true; see `effects/apply.rs`)
// Java: entity/itemcontainer/Inventory.java (ArmorSetListener, ItemSkillsListener)
// Java: entity/actor/Player.java (refreshOverloaded, refreshExpertisePenalty)
//
// A passive skill is applied through the effect engine (`effects/`, M5-EFFECT-01): its info sits
// in the effect list's passives, its effects' functions are owned by `FuncOwner::Effect`, like
// Java's `EffectList._passives` and the `AbstractEffect` owners.
//
// The talisman slots, the cloak slot and the expertise penalty bonus are set by the handlers
// `TalismanSlot`, `EnableCloak` and `CrystalGradeModify` (`effects/handlers/player_slots.rs`),
// as Java's `onStart` / `onExit` do, and read back from `CreatureControl`.
//
// What other M5 cards bring: skill reuse timestamps of active item skills, `SkillList` updates,
// skills of summons.

use l2_data::SkillTemplate;
use l2_data::item::{CrystalType, EtcItemType};
use l2_protocol_game::server::charscreen::paperdoll;
use l2_util::ObjectId;
use tracing::warn;

use super::calculators::FuncOwner;
use super::creature_stat::StatView;
use super::{CalcSkill, StatData};
use crate::effects::apply::ApplyMode;
use crate::effects::{SkillFinishType, SkillKey};
use crate::player::ItemData;
use crate::world::World;

/// `CommonSkill.WEAPON_GRADE_PENALTY`, `ARMOR_GRADE_PENALTY`, the weight penalty skill and
/// Expertise (`getExpertiseLevel()`).
pub const WEAPON_GRADE_PENALTY: i32 = 6209;
pub const ARMOR_GRADE_PENALTY: i32 = 6213;
pub const WEIGHT_PENALTY: i32 = 4270;
pub const EXPERTISE: i32 = 239;

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
                    | "QuestGuard"
                    | "Defender"
                    | "FortCommander"
                    | "TerritoryWard"
            )
        })
    }

    /// `PlayerStat._cloakSlot`: set by an `EnableCloak` passive effect.
    pub(crate) fn cloak_slot(&self) -> bool {
        self.world()
            .control(self.object_id())
            .is_some_and(|c| c.cloak_slot)
    }

    /// `PlayerStat.getTalismanSlots()`: the `slots` of every `TalismanSlot` passive effect.
    pub fn talisman_slots(&self) -> i32 {
        if !self.is_player() {
            return 0;
        }
        self.world()
            .control(self.object_id())
            .map_or(0, |c| c.talisman_slots)
    }

    /// `Player._expertisePenaltyBonus`: the `grade` of the `CrystalGradeModify` effect.
    pub fn expertise_penalty_bonus(&self) -> i32 {
        self.world()
            .control(self.object_id())
            .map_or(0, |c| c.expertise_penalty_bonus)
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

    /// Java `Creature.addSkill(skill)`: the skill replaces a known one of the same id (its
    /// functions go and, for a passive, its effects), then a passive skill applies its effects.
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
            // `removeStatsOwner(oldSkill)`, then `stopSkillEffects(NORMAL, id)` for a passive.
            core.calculators.remove_owner(FuncOwner::Skill {
                id: skill_id,
                level: old_level,
            });
            let old_passive = self
                .skill_templates()
                .and_then(|d| d.skills.get(skill_id, old_level))
                .is_some_and(SkillTemplate::is_passive);
            if old_passive {
                self.stop_skill_effects(id, SkillFinishType::Normal, skill_id);
            }
        }
        // `addStatFuncs(newSkill.getStatFuncs(null, this))`: skills carry no functions of their
        // own in the H5 data (DocumentSkill only attaches functions to effects).
        if passive {
            // `newSkill.applyEffects(this, this, false, true, false, 0)`.
            self.apply_effects_with(id, id, SkillKey::new(skill_id, level), ApplyMode::PASSIVE);
        }
        true
    }

    /// Java `Creature.removeSkill(skillId, cancelEffect)`: a passive or toggle skill (or any with
    /// `cancel_effect`) also loses its functions and its effects (`REMOVED`).
    pub fn remove_skill(&mut self, id: ObjectId, skill_id: i32, cancel_effect: bool) -> bool {
        let Some(core) = self.core_mut(id) else {
            return false;
        };
        let Some(at) = core.skills.iter().position(|(s, _)| *s == skill_id) else {
            return false;
        };
        let (_, level) = core.skills.remove(at);
        // Stop casting if this skill is used right now (the normal or the simultaneous cast).
        let casting = self.cast_state(id).is_some_and(|c| {
            (c.casting_now && c.last_skill_cast.is_some_and(|s| s.id == skill_id))
                || (c.casting_simultaneously_now
                    && c.last_simultaneous_skill_cast.is_some_and(|s| s.id == skill_id))
        });
        if casting {
            self.abort_cast(id);
        }
        let (toggle, passive) = self
            .skill_templates()
            .and_then(|d| d.skills.get(skill_id, level))
            .map_or((false, false), |s| (s.is_toggle(), s.is_passive()));
        if cancel_effect || toggle || passive {
            if let Some(core) = self.core_mut(id) {
                core.calculators
                    .remove_owner(FuncOwner::Skill { id: skill_id, level });
            }
            self.stop_skill_effects(id, SkillFinishType::Removed, skill_id);
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
