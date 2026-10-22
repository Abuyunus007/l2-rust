//! Summons: servitors now, pets (PET-01) and cubics (CUBIC-01) on top of the same core.
// Java: entity/actor/Summon.java, entity/actor/instance/Servitor.java
// Java: ai/SummonAI.java (see `ai.rs`)
// Java: network/clientpackets/RequestActionUse.java (the summon branch, see `actions.rs`)
// Java: data/sql/{CharSummonTable,SummonEffectTable}.java (see `servitor.rs`)
//
// # Where a summon lives (D-11, D-17)
// - The creature itself (position, vitals, calculators, skills, movement, pawn) is an [`Npc`]
//   entry in `World::npcs` with `class == NpcClass::Summon`, and an `ObjectKind::Summon` entry in
//   the object table. So the stat engine reads it from its NPC template, the movement, attack,
//   cast, effect and status code handle it like any creature in `npcs`, and only what a summon
//   does differently asks [`World::is_summon`]. `is_npc()` is false for it, like Java's
//   `isNpc()`.
// - What only a summon has (owner, kind, follow flag, `SummonAI` fields, the servitor's life
//   time) is a [`Summon`] in `SummonWorld::summons`, by the summon's object id. The owner's link
//   to it (Java `Player._summon`) is `SummonWorld::by_owner`. Both are ids, never references.
// - A pet (PET-01) is a `Summon` of kind [`SummonKind::Pet`] with its own pet state next to it.
//
// # Left out, with the system that brings it
// - Cubics are in `cubic.rs` (CUBIC-01). Duels (`setDuelState(DUELSTATE_INTERRUPTED)` in
//   `SummonStatus`): DUEL-01.
// - Olympiad, sieges and territory wars (`isSiegeFriend`, `isInOlympiadMode`): M7/M8; they read
//   as "not in one".
// - The servitor restore at the first action after a login (`Player.onActionRequest` with spawn
//   protection, `CharSummonTable.restoreServitor`): spawn protection is not modelled. The
//   `character_summons` row is written and removed exactly like Java, so the Java GS restores it.
//   For the same reason `Summon.isInvul()` has no `_owner.isSpawnProtected()` part.
// - The `PathFinding.findPath` refusals of `SummonAI` and `Summon.useMagic` (see `ai.rs`).
// - Transfer pain (`SummonStatus.reduceHp`'s `TRANSFER_DAMAGE_TO_PLAYER`), `TamedBeast`, mounts,
//   the Mobius auto play.

pub mod actions;
pub mod ai;
pub mod baby_pet;
pub mod cubic;
pub mod cubic_formulas;
pub mod info;
pub mod party;
pub mod pet;
pub mod pet_items;
pub mod pet_offline;
pub mod servitor;
pub mod share;
pub mod shots;
pub mod spawn;

use std::collections::HashMap;
use std::sync::Arc;

use l2_data::PetSkillData;
use l2_util::ObjectId;

use crate::effects::SkillKey;
use crate::timer::TimerId;
use crate::world::World;

/// What kind of summon (Java's `Servitor` / `Pet` classes, `getSummonType()`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SummonKind {
    Servitor,
    Pet,
}

impl SummonKind {
    /// Java `getSummonType()`: "1 for Summon, 2 for Pets" (the action buttons, `PetInfo`,
    /// `PetDelete`).
    pub fn summon_type(self) -> i32 {
        match self {
            Self::Servitor => 1,
            Self::Pet => 2,
        }
    }
}

/// The timers of a summon (one `TimerKind` variant for the module).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SummonTimer {
    /// `Servitor.run()`: the 5 s life task.
    Life(ObjectId),
    /// `SummonAI.run()`: the 100 ms avoid task.
    Avoid(ObjectId),
    /// `Summon.updateAbnormalEffect()`'s 50 ms task.
    AbnormalEffect(ObjectId),
    /// `Pet.FeedTask`: every 10 s (PET-01, `pet.rs`).
    PetFeed(ObjectId),
    /// `BabyPet.CastTask`: first after 3 s, then every 2 s (PET-01, `baby_pet.rs`).
    BabyCast(ObjectId),
    /// A cubic's action task and disappear task, see `cubic.rs`.
    Cubic(cubic::CubicTimer),
}

/// The `SummonAI` fields (Java `ai/SummonAI.java`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SummonAi {
    /// `_startFollow`: whether the summon goes back to following its owner when idle.
    pub start_follow: bool,
    /// `_lastAttack`: the attack target a cast interrupted, attacked again after the cast.
    pub last_attack: Option<ObjectId>,
    /// `_startAvoid`: the summon was hit near its owner and steps aside on the next avoid tick.
    pub start_avoid: bool,
    /// `_avoidTask`: the 100 ms avoid task.
    pub avoid_timer: Option<TimerId>,
    /// `_thinking`.
    pub thinking: bool,
}

/// The fields only a `Servitor` has.
#[derive(Debug, Clone, PartialEq)]
pub struct ServitorState {
    /// `_expMultiplier`: the owner's exp from kills is multiplied by it.
    pub exp_multiplier: f32,
    /// `_itemConsume`: the item (id, count) the servitor eats every `consume_interval` ms.
    pub consume_item: (i32, i32),
    /// `_lifeTime` and `_lifeTimeRemaining`, ms.
    pub life_time: i32,
    pub life_time_remaining: i32,
    /// `_consumeItemInterval` and `_consumeItemIntervalRemaining`, ms.
    pub consume_interval: i32,
    pub consume_interval_remaining: i32,
    /// `_summonLifeTask`: the 5 s task.
    pub life_timer: Option<TimerId>,
    /// `_referenceSkill`: the id of the skill that summoned it (the key of its saved rows).
    pub reference_skill: i32,
}

/// One summon (Java `Summon` and `Servitor` fields; the creature itself is in `World::npcs`).
#[derive(Debug, Clone, PartialEq)]
pub struct Summon {
    /// `_owner`.
    pub owner: ObjectId,
    /// The owner left the world while this (dead) summon stayed (`getOwner().isOnline()` is
    /// false): its title is empty and nothing is sent to the owner any more.
    pub owner_offline: bool,
    pub kind: SummonKind,
    /// `getName()`: the template's name for a servitor (`setName(template.getName())`).
    pub name: String,
    /// `getTitle()`: the owner's name.
    pub title: String,
    /// `_follow`.
    pub follow: bool,
    /// `_previousFollowStatus`.
    pub previous_follow: bool,
    /// `_restoreSummon`: the summon is stored for a restore when it is unsummoned.
    pub restore_summon: bool,
    /// `_attackRange` (36 = melee).
    pub attack_range: i32,
    /// `isShowSummonAnimation()`: the next info packets play the summon animation.
    pub show_summon_animation: bool,
    /// `SummonStat` exp and SP (`getStat().getExp()`).
    pub exp: i64,
    pub sp: i64,
    /// The `SummonAI` fields.
    pub ai: SummonAi,
    /// `_abnormalEffectTask` of `updateAbnormalEffect()` (50 ms).
    pub abnormal_effect_timer: Option<TimerId>,
    /// `Servitor`'s fields; `None` for a pet.
    pub servitor: Option<ServitorState>,
    /// `Pet`'s fields (PET-01); `None` for a servitor.
    pub pet: Option<pet::PetState>,
}

/// The `.ini` values summons read. `Default` is the Java default of each key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SummonSettings {
    /// `PlayerConfig.SUMMON_STORE_SKILL_COOLTIME` (`SummonStoreSkillCooltime`).
    pub summon_store_skill_cooltime: bool,
    /// `PlayerConfig.RESTORE_SERVITOR_ON_RECONNECT` (`RestoreServitorOnReconnect`).
    pub restore_servitor_on_reconnect: bool,
}

impl Default for SummonSettings {
    fn default() -> Self {
        Self {
            summon_store_skill_cooltime: true,
            restore_servitor_on_reconnect: true,
        }
    }
}

/// One saved buff of a servitor (Java `SummonEffectTable.SummonEffect`): the skill and the
/// seconds it had run (`BuffInfo.getTime()`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SummonEffect {
    pub skill: SkillKey,
    pub time: i32,
}

/// Every summon in the world and what the summon tables keep.
#[derive(Debug, Default)]
pub struct SummonWorld {
    pub(crate) summons: HashMap<ObjectId, Summon>,
    /// Java `Player._summon`: owner -> summon.
    pub(crate) by_owner: HashMap<ObjectId, ObjectId>,
    pub(crate) settings: SummonSettings,
    /// Java `SummonEffectTable._servitorEffects`: owner -> class index -> reference skill ->
    /// the buffs kept for the next summon of that skill.
    pub(crate) servitor_effects: HashMap<ObjectId, HashMap<i32, HashMap<i32, Vec<SummonEffect>>>>,
    /// Java `CharSummonTable._servitors`: owner -> reference skill of the stored servitor.
    pub(crate) saved_servitors: HashMap<ObjectId, i32>,
    /// `PetSkillData.xml` (`PetSkillData.getAvailableLevel`).
    pub(crate) pet_skills: Option<Arc<PetSkillData>>,
    /// Java `Player._servitorShare`: owner -> the stat multipliers of `ServitorShare`.
    pub(crate) servitor_share: HashMap<ObjectId, Vec<(l2_data::Stat, f64)>>,
    /// The world's view of `character_summon_skills_save`: (owner, class index, reference skill)
    /// -> the rows (see `servitor.rs`).
    pub(crate) saved_rows: HashMap<(ObjectId, i32, i32), Vec<servitor::SummonSkillSave>>,
    /// The pet tables and the pet `.ini` values (PET-01, see `pet.rs`).
    pub(crate) pet_world: pet::PetWorld,
    /// The cubics of every player and `CubicData` (CUBIC-01, see `cubic.rs`).
    pub(crate) cubics: cubic::CubicWorld,
}

impl World {
    /// The timers of the summon module.
    pub(crate) fn on_summon_timer(&mut self, timer: SummonTimer) {
        match timer {
            SummonTimer::Life(id) => self.on_servitor_life_timer(id),
            SummonTimer::Avoid(id) => self.on_summon_avoid_timer(id),
            SummonTimer::AbnormalEffect(id) => self.on_summon_abnormal_effect_timer(id),
            SummonTimer::PetFeed(id) => self.on_pet_feed_timer(id),
            SummonTimer::BabyCast(id) => self.on_baby_cast_timer(id),
            SummonTimer::Cubic(timer) => self.on_cubic_timer(timer),
        }
    }

    /// Installs the summon `.ini` values. Call before players enter.
    pub fn set_summon_settings(&mut self, settings: SummonSettings) {
        self.summons.settings = settings;
    }

    /// Installs `PetSkillData.xml`. Call before players enter.
    pub fn set_pet_skill_data(&mut self, data: Arc<PetSkillData>) {
        self.summons.pet_skills = Some(data);
    }

    /// Java `isSummon()`.
    pub fn is_summon(&self, id: ObjectId) -> bool {
        self.summons.summons.contains_key(&id)
    }

    /// Java `isServitor()`.
    pub fn is_servitor(&self, id: ObjectId) -> bool {
        self.summon(id).is_some_and(|s| s.kind == SummonKind::Servitor)
    }

    /// Java `isPet()`.
    pub fn is_pet(&self, id: ObjectId) -> bool {
        self.summon(id).is_some_and(|s| s.kind == SummonKind::Pet)
    }

    /// The summon state of `id`, if it is a summon.
    pub fn summon(&self, id: ObjectId) -> Option<&Summon> {
        self.summons.summons.get(&id)
    }

    pub(crate) fn summon_mut(&mut self, id: ObjectId) -> Option<&mut Summon> {
        self.summons.summons.get_mut(&id)
    }

    /// Java `Player.getSummon()`.
    pub fn summon_of(&self, owner: ObjectId) -> Option<ObjectId> {
        self.summons.by_owner.get(&owner).copied()
    }

    /// Java `Player.hasServitor()`.
    pub fn has_servitor(&self, owner: ObjectId) -> bool {
        self.summon_of(owner).is_some_and(|s| self.is_servitor(s))
    }

    /// Java `Player.hasPet()`.
    pub fn has_pet(&self, owner: ObjectId) -> bool {
        self.summon_of(owner).is_some_and(|s| self.is_pet(s))
    }

    /// Java `Summon.getOwner()`.
    pub fn summon_owner(&self, summon: ObjectId) -> Option<ObjectId> {
        self.summon(summon).map(|s| s.owner)
    }

    /// Java `asPlayer()`: a player is itself, a summon is its owner, anything else none.
    pub fn acting_player(&self, id: ObjectId) -> Option<ObjectId> {
        if self.players.contains_key(&id) {
            return Some(id);
        }
        self.summon_owner(id).filter(|o| self.players.contains_key(o))
    }

    /// Java `Player.setPet(summon)`.
    pub(crate) fn set_pet(&mut self, owner: ObjectId, summon: Option<ObjectId>) {
        match summon {
            Some(summon) => {
                self.summons.by_owner.insert(owner, summon);
            }
            None => {
                self.summons.by_owner.remove(&owner);
            }
        }
    }

    /// The exp penalty of `Attackable.calculateRewards`: the servitor's `getExpMultiplier()`,
    /// 1 without a servitor.
    pub(crate) fn servitor_exp_multiplier(&self, owner: ObjectId) -> f32 {
        self.summon_of(owner)
            .and_then(|s| self.summon(s))
            .and_then(|s| s.servitor.as_ref())
            .map_or(1.0, |s| s.exp_multiplier)
    }

    /// Java `ConditionPlayerCanSummon`'s test: the player (`effector.asPlayer()`) may call a
    /// summon now. Tells the player why not.
    // Spawn and teleport protection and mounts are not modelled.
    pub(crate) fn player_can_summon(&self, id: ObjectId) -> bool {
        let Some(player) = self.acting_player(id) else {
            return false;
        };
        let refused = (self.summons.settings.restore_servitor_on_reconnect
            && self.summons.saved_servitors.contains_key(&player))
            || (self.summons.pet_world.settings.restore_pet_on_reconnect
                && self.summons.pet_world.saved.contains_key(&player))
            || self.summon_of(player).is_some();
        if refused {
            self.send(
                player,
                &l2_protocol_game::server::SystemMessage::new(
                    l2_protocol_game::SystemMessageId::YouMayNotUseMultiplePetsOrServitorsAtTheSameTime,
                ),
            );
            return false;
        }
        true
    }

    /// Java `Summon.getSoulShotsPerHit()`: the template's `soulShot`, at least 1.
    pub fn soul_shots_per_hit(&self, summon: ObjectId) -> i32 {
        // `Pet.getSoulShotsPerHit()`: the level row's count.
        if let Some(level) = self.pet_level_data_of(summon) {
            return i32::from(level.soul_shot);
        }
        let shots = self.summon_template(summon).map_or(0, |t| t.soul_shot);
        if shots > 0 { shots } else { 1 }
    }

    /// Java `Summon.getSpiritShotsPerHit()`.
    pub fn spirit_shots_per_hit(&self, summon: ObjectId) -> i32 {
        // `Pet.getSpiritShotsPerHit()`.
        if let Some(level) = self.pet_level_data_of(summon) {
            return i32::from(level.spirit_shot);
        }
        let shots = self.summon_template(summon).map_or(0, |t| t.spirit_shot);
        if shots > 0 { shots } else { 1 }
    }

    /// The NPC template of a summon.
    pub(crate) fn summon_template(&self, summon: ObjectId) -> Option<&l2_data::NpcTemplate> {
        let npc = self.npcs.get(&summon)?;
        self.npc_world.template(npc.npc_id)
    }

    /// Java `Summon.getId()`: the template id.
    pub fn summon_npc_id(&self, summon: ObjectId) -> Option<i32> {
        self.npcs.get(&summon).map(|n| n.npc_id)
    }

    /// Java `Summon.getFormId()`: the look of the four summons that change with their level.
    pub fn summon_form_id(&self, summon: ObjectId) -> i32 {
        let Some(npc) = self.npcs.get(&summon) else {
            return 0;
        };
        let level = self.stats(summon).map_or(0, |s| s.level());
        if matches!(npc.npc_id, 16041 | 16042 | 16025 | 16037) {
            if level > 69 {
                return 3;
            } else if level > 64 {
                return 2;
            } else if level > 59 {
                return 1;
            }
        }
        0
    }

    /// Java `Summon.getExpForThisLevel()`: 0 at or above the highest pet level.
    pub fn summon_exp_for_this_level(&self, summon: ObjectId) -> i64 {
        // `Pet.getExpForThisLevel()`: the pet's own table.
        if self.is_pet(summon) {
            return self.pet_exp_for_this_level(summon);
        }
        let level = self.stats(summon).map_or(0, |s| s.level());
        let Some(max_pet_level) = self.exp_table_max_pet_level() else {
            return 0;
        };
        if level >= max_pet_level {
            return 0;
        }
        self.exp_needed_for_level(level).unwrap_or(0)
    }

    /// Java `Summon.getExpForNextLevel()`: 0 from one below the highest pet level on.
    pub fn summon_exp_for_next_level(&self, summon: ObjectId) -> i64 {
        // `Pet.getExpForNextLevel()`.
        if self.is_pet(summon) {
            return self.pet_exp_for_next_level(summon);
        }
        let level = self.stats(summon).map_or(0, |s| s.level());
        let Some(max_pet_level) = self.exp_table_max_pet_level() else {
            return 0;
        };
        if level >= max_pet_level - 1 {
            return 0;
        }
        self.exp_needed_for_level(level + 1).unwrap_or(0)
    }

    /// Java `Summon.isInCombat()`: the owner's.
    pub(crate) fn summon_in_combat(&self, summon: ObjectId) -> bool {
        self.summon_owner(summon).is_some_and(|o| self.is_in_combat(o))
    }

    /// Java `Summon.sendPacket(packet)`: what a summon is told goes to its owner.
    pub(crate) fn send_to_summon_owner(&self, summon: ObjectId, packet: &impl l2_net::Encode) {
        if let Some(owner) = self.summon_owner(summon) {
            self.send(owner, packet);
        }
    }

    /// Drops everything the summon tables keep about an owner who left (Java's `Player` object
    /// and its summon references are gone with it). The saved rows of `CharSummonTable` and
    /// `SummonEffectTable` stay: Java keeps them for the next login.
    pub(crate) fn forget_summon_owner(&mut self, owner: ObjectId) {
        self.summons.by_owner.remove(&owner);
    }
}
