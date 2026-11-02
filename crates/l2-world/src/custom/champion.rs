//! Champion monsters (`Custom/ChampionMonsters.ini`): a monster that spawns with a chance as a
//! stronger "champion" with better rewards.
// Java: config/custom/ChampionMonstersConfig.java, entity/spawns/Spawn.java (`initializeNpc`),
//       entity/actor/Attackable.java (`setChampion`, `isChampion`, `useVitalityRate`,
//       `calculateRewards`), entity/actor/Creature.java (`reduceCurrentHp`),
//       entity/actor/stat/CreatureStat.java (attack and speed: `stats/creature_stat.rs`),
//       mechanics/stats/Formulas.java (`calcHpRegen`), ai/AttackableAI.java (`ChampionPassive`),
//       entity/actor/templates/NpcTemplate.java (the drops: `rewards/drop.rs`),
//       mechanics/script/Quest.java (`giveItemRandomly`), network/serverpackets/AbstractNpcInfo.java
//
// Where each effect lives, for the reviewer:
// - the roll and the flag (`Npc::champion`): here, called from `npc.rs` (`initialize_npc`,
//   `spawn_npc_at`);
// - the attack and attack speed bonus: `StatView::p_atk` & co. (they read `StatSettings` and
//   `is_champion`, which now answers);
// - the HP division: `combat/damage.rs`; the HP regen: `status.rs`; exp and SP: `rewards/exp.rs`;
//   vitality: `rewards/vitality.rs`; drops: `rewards/drop.rs`; quest drops: `script/ctx.rs`;
//   the title and the red aura: `npc.rs` (`npc_info`); the passive AI: `ai/attackable.rs`.

use l2_util::ObjectId;

use crate::rewards::drop::{ChampionDrops, ItemHolder};
use crate::world::World;

/// The adena item ids `giveItemRandomly` treats as adena.
const ADENA_ID: i32 = 57;
const ANCIENT_ADENA_ID: i32 = 5575;

/// `Team.RED` (`AbstractNpcInfo` writes the team id).
const TEAM_RED: u8 = 2;

/// Java `ChampionMonstersConfig`. `Default` is the Java default of each key (champions off).
#[derive(Debug, Clone, PartialEq)]
pub struct ChampionSettings {
    /// `ChampionEnable`.
    pub enable: bool,
    /// `ChampionPassive`: champions are never aggressive.
    pub passive: bool,
    /// `ChampionFrequency`: the chance, in percent, that a spawn is a champion.
    pub frequency: i32,
    /// `ChampionTitle`.
    pub title: String,
    /// `ChampionAura` (`SHOW_CHAMPION_AURA`): champions get the red team aura.
    pub show_aura: bool,
    /// `ChampionMinLevel`.
    pub min_level: i32,
    /// `ChampionMaxLevel`.
    pub max_level: i32,
    /// `ChampionHp`: the damage a champion takes is divided by it (0 turns that off).
    pub hp: i32,
    /// `ChampionHpRegen`, a Java `float`.
    pub hp_regen: f32,
    /// `ChampionRewardsExpSp`.
    pub rewards_exp_sp: f32,
    /// `ChampionRewardsChance`.
    pub rewards_chance: f32,
    /// `ChampionRewardsAmount`.
    pub rewards_amount: f32,
    /// `ChampionAdenasRewardsChance`.
    pub adenas_rewards_chance: f32,
    /// `ChampionAdenasRewardsAmount`.
    pub adenas_rewards_amount: f32,
    /// `ChampionRewardLowerLvlItemChance`: percent, when the champion is lower than the killer.
    pub reward_lower_level_item_chance: i32,
    /// `ChampionRewardHigherLvlItemChance`.
    pub reward_higher_level_item_chance: i32,
    /// `ChampionRewardItems`.
    pub reward_items: Vec<ItemHolder>,
    /// `ChampionEnableVitality`.
    pub enable_vitality: bool,
    /// `ChampionEnableInInstances`.
    pub enable_in_instances: bool,
}

impl Default for ChampionSettings {
    fn default() -> Self {
        Self {
            enable: false,
            passive: false,
            frequency: 0,
            title: "Champion".to_owned(),
            show_aura: true,
            min_level: 20,
            max_level: 60,
            hp: 7,
            hp_regen: 1.0,
            rewards_exp_sp: 8.0,
            rewards_chance: 8.0,
            rewards_amount: 1.0,
            adenas_rewards_chance: 1.0,
            adenas_rewards_amount: 1.0,
            reward_lower_level_item_chance: 0,
            reward_higher_level_item_chance: 0,
            reward_items: vec![ItemHolder { id: 4356, count: 10 }],
            enable_vitality: false,
            enable_in_instances: false,
        }
    }
}

impl World {
    /// Java `Creature.isChampion()`: only an attackable that was rolled one.
    pub fn is_champion(&self, id: ObjectId) -> bool {
        self.npcs.get(&id).is_some_and(|n| n.champion)
    }

    /// Java `Attackable.setChampion(champ)`. The red aura (`setTeam`) is derived from the flag
    /// when `NpcInfo` is built (`champion_team`).
    pub fn set_champion(&mut self, id: ObjectId, champion: bool) {
        if let Some(npc) = self.npcs.get_mut(&id) {
            npc.champion = champion;
        }
    }

    /// The end of Java `Spawn.initializeNpc(npc)`: "Set champion on next spawn." `instance_id` is
    /// the instance of the spawn. Draws from the RNG only when every other condition holds.
    pub(crate) fn roll_champion(&mut self, id: ObjectId, instance_id: i32) {
        // `npc.asAttackable().setChampion(false)` first: a respawned object is no champion.
        self.set_champion(id, false);
        let c = &self.custom.settings.champion;
        if !c.enable {
            return;
        }
        let Some(npc) = self.npcs.get(&id) else {
            return;
        };
        let Some(template) = self.npc_world.template(npc.npc_id) else {
            return;
        };
        let level = npc.core.level;
        if !(npc.class.is_monster()
            && !template.quest_monster
            && !template.undying
            && !npc.class.is_raid()
            && !self.is_raid_minion(id)
            && c.frequency > 0
            && level >= c.min_level
            && level <= c.max_level
            && (c.enable_in_instances || instance_id == 0))
        {
            return;
        }
        let frequency = c.frequency;
        if self.rng().below(100) < frequency {
            self.set_champion(id, true);
        }
    }

    /// `CHAMPION_ENABLE && isChampion()`.
    fn champion_active(&self, id: ObjectId) -> bool {
        self.custom.settings.champion.enable && self.is_champion(id)
    }

    /// Java `Creature.reduceCurrentHp` for a champion: `amount / CHAMPION_HP` (when `ChampionHp`
    /// is not 0). Every other creature takes `amount`.
    pub(crate) fn champion_damage(&self, id: ObjectId, amount: f64) -> f64 {
        let hp = self.custom.settings.champion.hp;
        if self.champion_active(id) && hp != 0 {
            amount / f64::from(hp)
        } else {
            amount
        }
    }

    /// `ChampionMonstersConfig.CHAMPION_HP_REGEN` for `Formulas.calcHpRegen`.
    pub(crate) fn champion_hp_regen(&self, id: ObjectId) -> Option<f32> {
        self.champion_active(id)
            .then_some(self.custom.settings.champion.hp_regen)
    }

    /// The champion factor of the exp and SP of `Attackable.calculateRewards`.
    pub(crate) fn champion_exp_sp(&self, id: ObjectId, exp: f64, sp: f64) -> (f64, f64) {
        if self.champion_active(id) {
            let factor = f64::from(self.custom.settings.champion.rewards_exp_sp);
            (exp * factor, sp * factor)
        } else {
            (exp, sp)
        }
    }

    /// Java `Attackable.useVitalityRate()` for a plain attackable: `!_champion || CHAMPION_ENABLE_VITALITY`.
    pub(crate) fn champion_allows_vitality(&self, id: ObjectId) -> bool {
        !self.is_champion(id) || self.custom.settings.champion.enable_vitality
    }

    /// The part of `NpcInfo.getTeam()` a champion adds: `setTeam(RED)` when `ChampionAura` is on.
    // NOTE(java-quirk): Java's `setChampion` writes the NPC's one team field, so it also resets a
    // team another feature gave a monster; here the other feature's team wins.
    pub(crate) fn champion_team(&self, id: ObjectId) -> u8 {
        if self.is_champion(id) && self.custom.settings.champion.show_aura {
            TEAM_RED
        } else {
            0
        }
    }

    /// The champion part of the title in `AbstractNpcInfo.NpcInfo`: with the level / aggression
    /// title (`title` is the text built so far) a champion's title is `ChampionTitle + " " + title`.
    pub(crate) fn champion_level_title(&self, id: ObjectId, title: String) -> String {
        if self.is_champion(id) {
            format!("{} {title}", self.custom.settings.champion.title)
        } else {
            title
        }
    }

    /// The `else if (CHAMPION_ENABLE && isChampion())` title of `AbstractNpcInfo.NpcInfo`.
    pub(crate) fn champion_plain_title(&self, id: ObjectId) -> Option<String> {
        self.champion_active(id)
            .then(|| self.custom.settings.champion.title.clone())
    }

    /// `ChampionPassive` (`AttackableAI.autoAttackCondition`): a champion does not start fights.
    pub(crate) fn champion_is_passive(&self, id: ObjectId) -> bool {
        self.is_champion(id) && self.custom.settings.champion.passive
    }

    /// The `ChampionMonstersConfig` values `NpcTemplate.calculateDrops` reads.
    pub(crate) fn champion_drops(&self) -> ChampionDrops {
        let c = &self.custom.settings.champion;
        ChampionDrops {
            rewards_chance: c.rewards_chance,
            rewards_amount: c.rewards_amount,
            adenas_rewards_chance: c.adenas_rewards_chance,
            adenas_rewards_amount: c.adenas_rewards_amount,
            lower_level_item_chance: c.reward_lower_level_item_chance,
            higher_level_item_chance: c.reward_higher_level_item_chance,
            reward_items: c.reward_items.clone(),
        }
    }

    /// The champion part of Java `Quest.giveItemRandomly(.., npc, ..)`: the factors of the drop
    /// chance and of the amounts, `None` when `npc` is no champion (or champions are off).
    pub fn champion_quest_drop_factors(&self, npc: ObjectId, item_id: i32) -> Option<(f32, f32)> {
        if !self.champion_active(npc) {
            return None;
        }
        let c = &self.custom.settings.champion;
        if item_id == ADENA_ID || item_id == ANCIENT_ADENA_ID {
            Some((c.adenas_rewards_chance, c.adenas_rewards_amount))
        } else {
            Some((c.rewards_chance, c.rewards_amount))
        }
    }
}
