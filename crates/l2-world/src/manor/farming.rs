//! Sowing and harvesting: the seeded state of a monster, the `Seed` and `Harvester` item handlers
//! and the `Sow` and `Harvesting` effects.
// Java: entity/actor/Attackable.java (setSeeded, getSeed, getSeederId, isSeeded, takeHarvest)
// Java: dist/game/data/scripts/handlers/items/{Seed,Harvester}.java
// Java: dist/game/data/scripts/handlers/skill/effects/{Sow,Harvesting}.java
//
// A player uses a seed on a monster: the handler stores the seed and the seeder in the monster
// and casts the item's skill (Sowing, 2097). The `Sow` effect takes the seed from the inventory
// and rolls the chance; on success the monster is seeded and the crop it will give is fixed
// (`Attackable.setSeeded(player)`). When the monster is dead, the harvester (Harvesting, 2098)
// gives the seeder the crop. The state belongs to the monster and goes with it (`respawn.rs`).

use l2_data::item::ItemTemplate;
use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{ActionFailed, PlaySound, SystemMessage, SystemMessageParam};
use l2_util::ObjectId;
use tracing::warn;

use crate::effects::SkillKey;
use crate::inventory::item_name_param;
use crate::player::ItemData;
use crate::rewards::drop::ItemHolder;
use crate::world::World;

/// The skill ids of the monsters that give more crops: "Strong type x2" to "x9" (4303 to 4310).
fn strong_type_factor(skill_id: i32) -> Option<i32> {
    match skill_id {
        4303..=4310 => Some(skill_id - 4301),
        _ => None,
    }
}

/// Java `Sow.calcSuccess`: the chance of a sowing in percent. The roll is `Rnd.get(99) <` it.
// NOTE(java-quirk): Java calls `Math.max(basicSuccess, 1)` and drops the result, so the chance
// can be below 1 (a roll never succeeds then).
pub(crate) fn sow_chance(seed_level: i32, alternative: bool, player_level: i32, target_level: i32) -> i32 {
    let min_level_seed = seed_level - 5;
    let max_level_seed = seed_level + 5;
    let mut basic_success = if alternative { 20 } else { 90 };

    // seed level
    if target_level < min_level_seed {
        basic_success -= 5 * (min_level_seed - target_level);
    }
    if target_level > max_level_seed {
        basic_success -= 5 * (target_level - max_level_seed);
    }

    // 5% decrease in chance if player level is more than +/- 5 levels to target's level
    let diff = (player_level - target_level).abs();
    if diff > 5 {
        basic_success -= 5 * (diff - 5);
    }
    basic_success
}

/// Java `Harvesting.calcSuccess`: the chance of a harvest in percent (at least 1).
pub(crate) fn harvest_chance(player_level: i32, target_level: i32) -> i32 {
    let diff = (player_level - target_level).abs();
    // Apply penalty, target <=> player levels. 5% penalty for each level.
    let mut basic_success = 100;
    if diff > 5 {
        basic_success -= (diff - 5) * 5;
    }
    // Success rate can't be less than 1%.
    basic_success.max(1)
}

impl World {
    // ------------------------------------------------------------------ the state of a monster

    /// Java `Attackable.isSeeded()`.
    pub fn is_seeded(&self, monster: ObjectId) -> bool {
        self.manor.monsters.get(&monster).is_some_and(|m| m.seeded)
    }

    /// Java `Attackable.getSeederId()` (`0` is `None`).
    pub fn seeder_of(&self, monster: ObjectId) -> Option<ObjectId> {
        self.manor.monsters.get(&monster).and_then(|m| m.seeder)
    }

    /// Java `Attackable.getSeed()`: the item id of the seed.
    pub fn seed_of(&self, monster: ObjectId) -> Option<i32> {
        self.manor.monsters.get(&monster).and_then(|m| m.seed)
    }

    /// Java `Attackable.setSeeded(seed, seeder)`: stores the seed parameters, "but not the seed
    /// state". Does nothing for a monster that is seeded already.
    pub(crate) fn monster_set_seed(&mut self, monster: ObjectId, seed_id: i32, seeder: ObjectId) {
        let state = self.manor.monsters.entry(monster).or_default();
        if state.seeded {
            return;
        }
        state.seed = Some(seed_id);
        state.seeder = Some(seeder);
    }

    /// Java `Attackable.setSeeded(seeder)`: the monster is seeded and the crop it will give is
    /// fixed: one, times the "strong type" skills it has, plus its levels above the seed's, times
    /// `RateDropManor`.
    pub(crate) fn monster_set_seeded(&mut self, monster: ObjectId, seeder: ObjectId) {
        let Some(seed_id) = self
            .manor
            .monsters
            .get(&monster)
            .filter(|m| m.seeder == Some(seeder))
            .and_then(|m| m.seed)
        else {
            return;
        };
        let Some(seed) = self.manor.seeds.get(seed_id).cloned() else {
            return;
        };
        let mut count: i32 = 1;
        let mut skill_ids: Vec<i32> = self
            .npcs
            .get(&monster)
            .and_then(|n| self.npc_world.template(n.npc_id))
            .map(|t| t.skills.iter().map(|s| s.id).collect())
            .unwrap_or_default();
        skill_ids.sort_unstable();
        skill_ids.dedup();
        for skill_id in skill_ids {
            if let Some(factor) = strong_type_factor(skill_id) {
                count = count.wrapping_mul(factor);
            }
        }
        // hi-level mobs bonus
        let diff = self.manor_level_of(monster) - seed.level - 5;
        if diff > 0 {
            count = count.wrapping_add(diff);
        }
        let amount = i64::from(count.wrapping_mul(self.manor.settings.rate_drop_manor));
        let state = self.manor.monsters.entry(monster).or_default();
        state.seeded = true;
        state.harvest = Some(ItemHolder {
            id: seed.crop_id,
            count: amount,
        });
    }

    /// Java `Attackable.takeHarvest()`: the crop, once.
    pub(crate) fn monster_take_harvest(&mut self, monster: ObjectId) -> Option<ItemHolder> {
        self.manor.monsters.get_mut(&monster)?.harvest.take()
    }

    /// `getLevel()` of a player or an NPC.
    fn manor_level_of(&self, id: ObjectId) -> i32 {
        match self.players.get(&id) {
            Some(p) => p.core.level,
            None => self.npcs.get(&id).map_or(0, |n| n.core.level),
        }
    }

    // ------------------------------------------------------------------ the item handlers

    /// Java `Seed.onItemUse`.
    pub(crate) fn seed_on_use(&mut self, id: ObjectId, item: &ItemData, template: &ItemTemplate) -> bool {
        if !self.manor.settings.allow_manor {
            return false;
        }
        if !self.is_player(id) {
            self.send(
                id,
                &SystemMessage::new(SystemMessageId::YourPetCannotCarryThisItem),
            );
            return false;
        }

        let target = self.target_of(id).filter(|t| self.is_npc(*t));
        let Some(target) = target else {
            self.send(id, &SystemMessage::new(SystemMessageId::InvalidTarget));
            return false;
        };
        if !self.is_monster(target) || self.is_raid(target) || self.is_chest(target) {
            self.send(
                id,
                &SystemMessage::new(SystemMessageId::TheTargetIsUnavailableForSeeding),
            );
            return false;
        }
        if self.is_dead(target) {
            self.send(id, &SystemMessage::new(SystemMessageId::InvalidTarget));
            return false;
        } else if self.is_seeded(target) {
            self.send(id, &ActionFailed);
            return false;
        }

        let Some(seed) = self.manor.seeds.get(item.item_id).cloned() else {
            return false;
        };
        // `MapRegionData.getAreaCastle(playable)`; TODO in Java: replace with the tax zone.
        let area_castle = match (self.position_of(id), self.zoning.map_regions.as_ref()) {
            (Some((x, y, _)), Some(regions)) => regions.area_castle(x, y),
            _ => 0,
        };
        if seed.castle_id != area_castle {
            self.send(id, &SystemMessage::new(SystemMessageId::ThisSeedMayNotBeSownHere));
            return false;
        }

        self.monster_set_seed(target, seed.seed_id, id);

        self.cast_item_skills(id, template);
        true
    }

    /// Java `Harvester.onItemUse`.
    pub(crate) fn harvester_on_use(&mut self, id: ObjectId, template: &ItemTemplate) -> bool {
        if !self.manor.settings.allow_manor {
            return false;
        }
        if !self.is_player(id) {
            self.send(
                id,
                &SystemMessage::new(SystemMessageId::YourPetCannotCarryThisItem),
            );
            return false;
        }
        if template.skills.item_skills.is_none() {
            warn!("Harvester: is missing skills!");
            return false;
        }

        let target = self.target_of(id);
        if !target.is_some_and(|t| self.is_monster(t) && self.is_dead(t)) {
            self.send(id, &SystemMessage::new(SystemMessageId::InvalidTarget));
            self.send(id, &ActionFailed);
            return false;
        }

        self.cast_item_skills(id, template);
        true
    }

    /// `for (SkillHolder sk : skills) player.useMagic(sk.getSkill(), false, false)`.
    fn cast_item_skills(&mut self, id: ObjectId, template: &ItemTemplate) {
        let Some(skills) = template.skills.item_skills.clone() else {
            return;
        };
        let Some(table) = self.skill_table() else {
            return;
        };
        for skill_ref in skills {
            // A skill that does not exist is skipped (`SkillHolder.getSkill()` is null).
            let Some(skill) = table.get(skill_ref.id, skill_ref.level) else {
                continue;
            };
            self.use_magic(id, SkillKey::of(skill), false, false);
        }
    }

    // ------------------------------------------------------------------ the effects

    /// Java `Sow.onStart(effector, effected, skill)`.
    pub(crate) fn manor_sow(&mut self, effector: ObjectId, effected: ObjectId) {
        if !self.is_player(effector) || !self.is_monster(effected) {
            return;
        }
        let can_be_sown = self
            .npcs
            .get(&effected)
            .and_then(|n| self.npc_world.template(n.npc_id))
            .is_some_and(|t| t.can_be_sown);
        if self.is_dead(effected)
            || !can_be_sown
            || self.is_seeded(effected)
            || self.seeder_of(effected) != Some(effector)
        {
            return;
        }

        // Consuming used seed
        let Some(seed) = self
            .seed_of(effected)
            .and_then(|s| self.manor.seeds.get(s))
            .cloned()
        else {
            return;
        };
        if !self.destroy_item_by_item_id(effector, seed.seed_id, 1, false) {
            return;
        }

        let message = if self.manor_calc_sow_success(effector, effected, seed.level, seed.alternative) {
            self.send(effector, &PlaySound::file("ItemSound.quest_itemget"));
            self.monster_set_seeded(effected, effector);
            SystemMessage::new(SystemMessageId::TheSeedWasSuccessfullySown)
        } else {
            SystemMessage::new(SystemMessageId::TheSeedWasNotSown)
        };

        if self.is_in_party(effector) {
            for member in self.party_members_of(effector) {
                self.send(member, &message);
            }
        } else {
            self.send(effector, &message);
        }

        // TODO in Java: the mob should not aggro on the player, this way doesn't work really nice.
        self.set_intention_idle(effected);
    }

    fn manor_calc_sow_success(
        &mut self,
        player: ObjectId,
        target: ObjectId,
        seed_level: i32,
        alternative: bool,
    ) -> bool {
        let chance = sow_chance(
            seed_level,
            alternative,
            self.manor_level_of(player),
            self.manor_level_of(target),
        );
        self.rng().below(99) < chance
    }

    /// Java `Harvesting.onStart(effector, effected, skill)`.
    pub(crate) fn manor_harvest(&mut self, effector: ObjectId, effected: ObjectId) {
        if !self.is_player(effector) || !self.is_monster(effected) || !self.is_dead(effected) {
            return;
        }
        if self.seeder_of(effected) != Some(effector) {
            self.send(
                effector,
                &SystemMessage::new(SystemMessageId::YouAreNotAuthorizedToHarvest),
            );
        } else if self.is_seeded(effected) {
            let chance = harvest_chance(self.manor_level_of(effector), self.manor_level_of(effected));
            if self.rng().below(99) < chance {
                if let Some(item) = self.monster_take_harvest(effected) {
                    self.manor_give_harvest(effector, item);
                }
            } else {
                self.send(
                    effector,
                    &SystemMessage::new(SystemMessageId::TheHarvestHasFailed),
                );
            }
        } else {
            self.send(
                effector,
                &SystemMessage::new(SystemMessageId::TheHarvestFailedBecauseTheSeedWasNotSown),
            );
        }
    }

    /// The reward of a harvest: the item, the messages to the player and to the party.
    fn manor_give_harvest(&mut self, player: ObjectId, item: ItemHolder) {
        // Add item
        self.inventory_add_item_by_id(player, item.id, item.count);

        // Send system msg
        let templates = self.item_templates();
        let name = item_name_param(templates.as_deref(), item.id);
        let message = if item.count == 1 {
            SystemMessage::new(SystemMessageId::YouHaveObtainedS1).with(name.clone())
        } else {
            SystemMessage::new(SystemMessageId::YouHaveObtainedS2S1)
                .with(name.clone())
                .with(SystemMessageParam::LongNumber(item.count))
        };
        self.send(player, &message);

        // Send msg to party.
        if self.is_in_party(player) {
            let player_name = self
                .players
                .get(&player)
                .map(|p| p.data.name.clone())
                .unwrap_or_default();
            let message = if item.count == 1 {
                SystemMessage::new(SystemMessageId::C1HarvestedS2)
                    .with(SystemMessageParam::Text(player_name))
                    .with(name)
            } else {
                SystemMessage::new(SystemMessageId::C1HarvestedS3S2S)
                    .with(SystemMessageParam::Text(player_name))
                    .with(SystemMessageParam::LongNumber(item.count))
                    .with(name)
            };
            for member in self.party_members_of(player) {
                if member != player {
                    self.send(member, &message);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sowing_chance_follows_the_java_formula() {
        // A regular seed (90) on a monster of the seed's level, a player of the same level.
        assert_eq!(sow_chance(20, false, 20, 20), 90);
        // An alternative seed starts at 20.
        assert_eq!(sow_chance(20, true, 20, 20), 20);
        // The monster 3 levels under the seed's range (15): 5% per level.
        assert_eq!(sow_chance(20, false, 12, 12), 90 - 5 * 3);
        // The monster 2 levels above the range.
        assert_eq!(sow_chance(20, false, 27, 27), 90 - 5 * 2);
        // The player 8 levels above the monster: 5% for every level past 5 (the monster is in range).
        assert_eq!(sow_chance(20, false, 28, 20), 90 - 5 * 3);
        // The chance can go below 1: Java's `Math.max` result is thrown away.
        assert_eq!(sow_chance(20, true, 70, 40), 20 - 5 * 15 - 5 * 25);
    }

    #[test]
    fn harvesting_chance_follows_the_java_formula() {
        assert_eq!(harvest_chance(40, 40), 100);
        assert_eq!(harvest_chance(40, 45), 100);
        assert_eq!(harvest_chance(40, 48), 85);
        assert_eq!(harvest_chance(40, 60), 25);
        // Never below 1.
        assert_eq!(harvest_chance(40, 80), 1);
    }

    #[test]
    fn strong_monsters_give_more() {
        assert_eq!(strong_type_factor(4303), Some(2));
        assert_eq!(strong_type_factor(4310), Some(9));
        assert_eq!(strong_type_factor(4302), None);
        assert_eq!(strong_type_factor(4311), None);
    }
}
