//! Casting: `RequestMagicSkillUse` -> `Player.useMagic` -> the CAST intention (`ai/`) ->
//! `Creature.doCast` -> `beginCast`, then the three phases of `MagicUseTask` on the timer wheel,
//! `abortCast` / `breakCast`, and `callSkill`, which hands the skill to the effect engine.
// Java: entity/actor/Creature.java (doCast, doSimultaneousCast, beginCast (both), onMagicLaunchedTimer,
//       onMagicHitTimer, onMagicFinalizer, completeMagicFinalizer, callSkill, abortCast, breakCast,
//       canAbortCast, isCastingNow, isCastingSimultaneouslyNow, isAttackingOrCastingNow)
// Java: entity/actor/Player.java (useMagic, doCast, setCurrentSkill, setQueuedSkill,
//       setCurrentSkillWorldPosition)
// Java: entity/actor/tasks/creature/{MagicUseTask,QueuedMagicUseTask}.java
// Java: network/clientpackets/RequestMagicSkillUse.java
//
// # The one entry point
// Everything that casts goes through here: the client (`on_request_magic_skill_use`), items
// (M5-ITEM-01: [`World::use_magic`] like `ItemSkillsTemplate`, or [`World::do_simultaneous_cast`]
// for potions and herbs) and NPC AI (M5-AI-03: [`World::set_npc_target`] then [`World::do_cast`],
// exactly where `AttackableAI` calls `npc.setTarget(...)` and `npc.doCast(skill)`).
//
// # The state machine
// A creature's [`CastState`] is Java's cast fields of `Creature` and `Player`. A cast is
// `beginCast` -> phase 1 `onMagicLaunchedTimer` (at `skillTime - 400` ms) -> phase 2
// `onMagicHitTimer` (400 ms later: costs, `callSkill`) -> phase 3 `onMagicFinalizer` (the next tick).
// The [`MagicUseTask`] travels inside its timer ([`CastTimer::MagicUse`]), so cancelling the timer
// in `_skillCast` / `_skillCast2` drops the task, as cancelling Java's future does. Java's
// `ThreadPool.schedule(.., 0)` / `execute` become "the next tick".
//
// # Left for other cards (markers)
// - `// M5-EFX-03:` `Attackable.reduceHate`.
// - `// M6:` summons, olympiad, transformations, talisman mana.
// The channelizer is `channelizer.rs`, the option trigger skills and the weapon's chance skills
// are `triggers.rs` (M6-FU-12).

use std::collections::HashMap;

use l2_data::SkillTemplate;
use l2_data::skill::{EffectScope, TargetType};
use l2_data::stats::AbnormalType;
use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::visibility::status;
use l2_protocol_game::server::{
    ActionFailed, ExRotation, MagicSkillCanceled, MagicSkillLaunched, MagicSkillUse, SetupGauge,
    SkillPosition, StatusUpdate, SystemMessage,
};
use l2_util::ObjectId;
use tracing::warn;

use super::checks::{heading_between, is_channeling, is_continuous};
use super::reuse::ReuseState;
use crate::effects::apply::has_negative_effect;
use crate::effects::buff_info::skill_name_param;
use crate::effects::{EffectType, SkillKey, effect_flag};
use crate::formulas::{SkillMasteryInput, calc_atk_spd, calc_skill_mastery};
use crate::inventory::handlers::shots::ShotType;
use crate::movement::Intention;
use crate::stats::{BaseStat, ClassBalance, Stat};
use crate::timer::{TimerId, TimerKind, TimerWheel};
use crate::world::World;

/// `GameTimeTaskManager.MILLIS_IN_TICK`.
const MILLIS_IN_TICK: i32 = 100;
/// The client shows the hit 400 ms before the cast time ends (`skillTime - 400`).
const LAUNCH_BEFORE_HIT_MS: i32 = 400;
/// "Wait for skill land animation" before the attack that follows a skill.
const NEXT_ACTION_DELAY_MS: u64 = 333;
/// The retry of a potion or herb while another one is being used.
const SIMULTANEOUS_RETRY_MS: u64 = 100;
/// `CommonSkill.RAID_CURSE`, `RAID_CURSE2`.
const RAID_CURSE: SkillKey = SkillKey { id: 4215, level: 1 };
const RAID_CURSE2: SkillKey = SkillKey { id: 4515, level: 1 };
/// `CommonSkill.FAKE_DEATH`.
const FAKE_DEATH: SkillKey = SkillKey { id: 60, level: 1 };
/// Fishing (done in `Player.startFishing()`) and Wolf Collar.
const FISHING_SKILL: i32 = 1312;
const WOLF_COLLAR: i32 = 2046;
/// Lure and Temptation do not put the caster on the attack-by list.
const LURE: i32 = 51;
const TEMPTATION: i32 = 511;

/// The configuration values casting reads. `Default` is Java's default of each key.
#[derive(Debug, Clone, PartialEq)]
pub struct CastSettings {
    /// `PlayerConfig.ALT_GAME_KARMA_PLAYER_CAN_TELEPORT` (`AltKarmaPlayerCanTeleport`).
    pub alt_game_karma_player_can_teleport: bool,
    /// `PlayerConfig.DANCE_CONSUME_ADDITIONAL_MP` (`DanceConsumeAdditionalMP`).
    pub dance_consume_additional_mp: bool,
    /// `NpcConfig.RAID_DISABLE_CURSE` (`DisableRaidCurse`).
    pub raid_disable_curse: bool,
    /// `GrandBossConfig.DISABLE_RAIDBOSS_HEAL_FROM_PLAYERS` (`DisableRaidBossHealFromPlayers`).
    pub disable_raidboss_heal_from_players: bool,
    /// `GrandBossConfig.DISABLE_RAIDBOSS_BUFF_FROM_PLAYERS` (`DisableRaidBossBuffFromPlayers`).
    pub disable_raidboss_buff_from_players: bool,
    /// `ClassBalanceConfig.SKILL_REUSE_MULTIPLIERS` (`SkillReuseMultipliers`), by class id.
    pub skill_reuse_multipliers: Vec<f32>,
}

impl Default for CastSettings {
    fn default() -> Self {
        Self {
            alt_game_karma_player_can_teleport: true,
            dance_consume_additional_mp: true,
            raid_disable_curse: false,
            disable_raidboss_heal_from_players: false,
            disable_raidboss_buff_from_players: false,
            skill_reuse_multipliers: vec![1.0; ClassBalance::CLASSES],
        }
    }
}

/// Java `SkillUseHolder`: a skill with the keys held when it was used.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SkillUse {
    pub skill: SkillKey,
    /// Ctrl: force use (attack a non-enemy).
    pub ctrl_pressed: bool,
    /// Shift: do not move into range.
    pub shift_pressed: bool,
}

/// Java `MagicUseTask`: one cast between its phases.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MagicUseTask {
    pub caster: ObjectId,
    pub targets: Vec<ObjectId>,
    pub skill: SkillKey,
    pub count: i32,
    /// Milliseconds of the cast (`_skillTime`; 0 runs every phase at once).
    pub skill_time: i32,
    /// 1 launched, 2 hit, 3 finalizer.
    pub phase: u8,
    pub simultaneous: bool,
}

/// The casting timers (Java's tasks and delayed lambdas of the cast).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CastTimer {
    /// `MagicUseTask.run()`: the next phase of a cast.
    MagicUse(MagicUseTask),
    /// `beginCast`'s "Queue herbs and potions": the potion waits 100 ms for the other one.
    RetryBeginCast {
        caster: ObjectId,
        skill: SkillKey,
        simultaneous: bool,
        target: ObjectId,
        targets: Vec<ObjectId>,
    },
    /// `CreatureAI.setIntentionCast`'s wait for the attack that is still running.
    ChangeIntentionToCast {
        caster: ObjectId,
        skill: SkillKey,
        target: Option<ObjectId>,
    },
    /// `onMagicFinalizer`'s attack after a skill with `nextActionIsAttack`.
    AttackAfterCast { caster: ObjectId, target: ObjectId },
    /// `onMagicFinalizer`'s delayed `completeMagicFinalizer` of a moving player.
    CompleteMagicFinalizer {
        caster: ObjectId,
        skill: SkillKey,
        target: Option<ObjectId>,
    },
    /// `QueuedMagicUseTask`: the skill the player pressed during the cast.
    QueuedMagicUse { player: ObjectId, queued: SkillUse },
    /// `SkillChannelizer.run()`: one tick of a channeling skill (`channelizer.rs`).
    ChannelTick { channelizer: ObjectId },
}

/// The cast fields of one creature (Java `Creature` and `Player`).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CastState {
    /// `_isCastingNow`.
    pub(crate) casting_now: bool,
    /// `_isCastingSimultaneouslyNow`.
    pub(crate) casting_simultaneously_now: bool,
    /// `_castInterruptTime`, in game ticks.
    pub(crate) cast_interrupt_time: i32,
    /// `_lastSkillCast`, `_lastSimultaneousSkillCast`.
    pub(crate) last_skill_cast: Option<SkillKey>,
    pub(crate) last_simultaneous_skill_cast: Option<SkillKey>,
    /// `_skillCast`, `_skillCast2`: the pending phase timers.
    pub(crate) skill_cast: Option<TimerId>,
    pub(crate) skill_cast2: Option<TimerId>,
    /// `Player._currentSkill`, `_queuedSkill`.
    pub(crate) current_skill: Option<SkillUse>,
    pub(crate) queued_skill: Option<SkillUse>,
    /// `Player._currentSkillWorldPosition`: the ground point of a `GROUND` skill.
    pub(crate) current_skill_world_position: Option<(i32, i32, i32)>,
    /// The reuse stamps and disabled skills (`reuse.rs`).
    pub(crate) reuse: ReuseState,
    /// `Creature._channelizer`: the channeling skill this creature casts (`channelizer.rs`).
    pub(crate) channelizer: super::channelizer::Channelizer,
    /// `Creature._channelized`: who channels a skill at this creature (`channelizer.rs`).
    pub(crate) channelized: super::channelizer::SkillChannelized,
    /// `Creature._triggerSkills`: the activation skills of the worn item options, by skill id in
    /// insertion order (`triggers.rs`).
    pub(crate) trigger_skills: Vec<l2_data::augmentation::OptionSkillHolder>,
}

/// The cast table of the world.
#[derive(Debug, Default)]
pub struct CastWorld {
    pub(crate) creatures: HashMap<ObjectId, CastState>,
}

/// Which `Future` field a phase timer goes into.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Slot {
    /// `_skillCast`.
    Normal,
    /// `_skillCast2`.
    Simultaneous,
}

impl World {
    // ------------------------------------------------------------------ state

    pub(crate) fn cast_state_mut(&mut self, id: ObjectId) -> &mut CastState {
        self.cast.creatures.entry(id).or_default()
    }

    /// The cast fields of a creature, if it ever cast or was given a reuse.
    pub fn cast_state(&self, id: ObjectId) -> Option<&CastState> {
        self.cast.creatures.get(&id)
    }

    /// Java `isCastingNow()`.
    pub fn is_casting_now(&self, id: ObjectId) -> bool {
        self.cast.creatures.get(&id).is_some_and(|c| c.casting_now)
    }

    /// Java `isCastingSimultaneouslyNow()`.
    pub fn is_casting_simultaneously_now(&self, id: ObjectId) -> bool {
        self.cast
            .creatures
            .get(&id)
            .is_some_and(|c| c.casting_simultaneously_now)
    }

    /// Java `setCastingNow(value)`.
    pub fn set_casting_now(&mut self, id: ObjectId, value: bool) {
        if !value && !self.cast.creatures.contains_key(&id) {
            return;
        }
        self.cast_state_mut(id).casting_now = value;
    }

    /// Java `setCastingSimultaneouslyNow(value)`.
    pub fn set_casting_simultaneously_now(&mut self, id: ObjectId, value: bool) {
        if !value && !self.cast.creatures.contains_key(&id) {
            return;
        }
        self.cast_state_mut(id).casting_simultaneously_now = value;
    }

    /// Java `canAbortCast()`: the cast is still before its interrupt time.
    pub fn can_abort_cast(&self, id: ObjectId) -> bool {
        let now = self.game_ticks();
        self.cast
            .creatures
            .get(&id)
            .is_some_and(|c| c.cast_interrupt_time > now)
    }

    /// Java `isAttackingOrCastingNow()`.
    pub fn is_attacking_or_casting_now(&self, id: ObjectId) -> bool {
        self.is_attacking_now(id)
            || self.is_range_attacking_now(id)
            || self.is_casting_now(id)
            || self.is_casting_simultaneously_now(id)
    }

    /// Java `Player.getCurrentSkill()`.
    pub fn current_skill(&self, id: ObjectId) -> Option<SkillUse> {
        self.cast.creatures.get(&id).and_then(|c| c.current_skill)
    }

    /// Java `Player.getQueuedSkill()`.
    pub fn queued_skill(&self, id: ObjectId) -> Option<SkillUse> {
        self.cast.creatures.get(&id).and_then(|c| c.queued_skill)
    }

    /// Java `Player.setCurrentSkill(skill, ctrl, shift)` (`None` clears it).
    fn set_current_skill(&mut self, id: ObjectId, skill: Option<SkillUse>) {
        if skill.is_none() && !self.cast.creatures.contains_key(&id) {
            return;
        }
        self.cast_state_mut(id).current_skill = skill;
    }

    /// Java `Player.setQueuedSkill(skill, ctrl, shift)` (`None` clears it).
    pub fn set_queued_skill(&mut self, id: ObjectId, skill: Option<SkillUse>) {
        if skill.is_none() && !self.cast.creatures.contains_key(&id) {
            return;
        }
        self.cast_state_mut(id).queued_skill = skill;
    }

    /// Java `Player.setCurrentSkillWorldPosition(location)`: where a `GROUND` skill aims
    /// (`RequestExMagicSkillUseGround` sets it before `useMagic`).
    pub fn set_current_skill_world_position(&mut self, id: ObjectId, position: Option<(i32, i32, i32)>) {
        self.cast_state_mut(id).current_skill_world_position = position;
    }

    /// Java `npc.setTarget(target)`: what an NPC's next `doCast` aims at (M5-AI-03 sets it the
    /// way `AttackableAI` does, and puts the old one back after the cast). The same field as
    /// [`World::set_creature_target`].
    pub fn set_npc_target(&mut self, npc: ObjectId, target: Option<ObjectId>) {
        if self.npcs.contains_key(&npc) {
            self.set_creature_target(npc, target);
        }
    }

    /// Drops the cast state of a creature whose object is gone (its timers stop with it).
    pub(crate) fn forget_cast(&mut self, id: ObjectId) {
        // The creatures it channeled a skill at forget it (`stopChanneling`).
        if self.is_channeling(id) {
            self.stop_channeling(id);
        }
        if let Some(state) = self.cast.creatures.remove(&id) {
            for timer in [state.skill_cast, state.skill_cast2, state.channelizer.task()]
                .into_iter()
                .flatten()
            {
                self.timers.cancel(timer);
            }
        }
    }

    fn is_in_world(&self, id: ObjectId) -> bool {
        self.players.contains_key(&id) || self.npcs.contains_key(&id)
    }

    fn template_of(&self, key: SkillKey) -> Option<std::sync::Arc<l2_data::SkillTemplates>> {
        let table = self.skill_table()?;
        table.get(key.id, key.level)?;
        Some(table)
    }

    // ------------------------------------------------------------------ the client's request

    /// Java `RequestMagicSkillUse.runImpl`.
    pub(crate) fn on_request_magic_skill_use(
        &mut self,
        player: ObjectId,
        magic_id: i32,
        ctrl_pressed: bool,
        shift_pressed: bool,
    ) {
        if !self.players.contains_key(&player) {
            return;
        }
        if self.is_dead(player) {
            self.send(player, &ActionFailed);
            return;
        }
        if self.is_fake_death(player) {
            if magic_id != FAKE_DEATH.id {
                self.send(
                    player,
                    &SystemMessage::new(SystemMessageId::YouCannotMoveWhileSitting),
                );
                self.send(player, &ActionFailed);
                return;
            }
            let disabled = self
                .skill_table()
                .and_then(|t| t.get(FAKE_DEATH.id, FAKE_DEATH.level).cloned())
                .is_some_and(|fake_death| self.is_skill_disabled(player, &fake_death));
            if disabled {
                self.send(player, &ActionFailed);
                return;
            }
        }

        // Get the level of the used skill. Custom skills (by display id) do not exist yet; the
        // skills of a transformation are looked up when the player does not know the skill.
        let Some(key) = self
            .known_skill(player, magic_id)
            .or_else(|| self.transform_skill(player, magic_id))
        else {
            self.send(player, &ActionFailed);
            return;
        };
        let Some(table) = self.template_of(key) else {
            self.send(player, &ActionFailed);
            return;
        };
        let Some(skill) = table.get(key.id, key.level) else {
            return;
        };
        // Airships: none. Do not cast non transform skills when transformed.
        if self.stats(player).is_some_and(|view| {
            view.transformation()
                .is_some_and(|form| !form.can_use_weapon_stats())
        }) && !self.has_transform_skill(player, key.id)
        {
            self.send(player, &ActionFailed);
            return;
        }

        // If alternate rule Karma punishment is set to true, forbid teleport to player with Karma.
        let karma = self.players.get(&player).map_or(0, |p| p.data.karma);
        if !self.settings.cast.alt_game_karma_player_can_teleport
            && karma > 0
            && self.skill_has_effect_type(skill, &[EffectType::Teleport])
        {
            return;
        }
        // Players mounted on pets cannot use any toggle skills.
        if skill.is_toggle() && self.is_mounted(player) {
            return;
        }

        // `player.onActionRequest()`: only the pet restore is modelled (`summon/pet.rs`).
        self.on_action_request(player);

        // Stop if use self-buff (except if on AirShip or Boat).
        // NOTE(java-quirk): `(!isInAirShip() || !isInBoat())` is always true.
        if is_continuous(skill) && !skill.is_debuff && skill.target_type == TargetType::SelfTarget {
            if let Some((x, y, z)) = self.position_of(player) {
                self.set_intention_move_to(player, x, y, z);
            }
        }
        self.use_magic(player, key, ctrl_pressed, shift_pressed);
    }

    // ------------------------------------------------------------------ useMagic

    /// Java `Player.useMagic(skill, forceUse, dontMove)`: queue the skill behind a running cast,
    /// or check it and give the AI the CAST intention. `true` when the intention was set.
    pub fn use_magic(&mut self, player: ObjectId, key: SkillKey, force_use: bool, dont_move: bool) -> bool {
        if !self.players.contains_key(&player) {
            return false;
        }
        let Some(table) = self.template_of(key) else {
            self.send(player, &ActionFailed);
            return false;
        };
        let Some(skill) = table.get(key.id, key.level) else {
            return false;
        };
        // Check if the skill is active: just ignore the passive skill request (why does the
        // client send it anyway?).
        if skill.is_passive() {
            self.send(player, &ActionFailed);
            return false;
        }

        // If a skill is currently being used, queue this one if this is not the same.
        if self.is_casting_now(player) {
            // Check if new skill different from current skill in progress.
            let same = self.current_skill(player).is_some_and(|c| c.skill.id == skill.id);
            if same || self.is_skill_disabled(player, skill) {
                self.send(player, &ActionFailed);
                return false;
            }
            self.set_queued_skill(
                player,
                Some(SkillUse {
                    skill: key,
                    ctrl_pressed: force_use,
                    shift_pressed: dont_move,
                }),
            );
            self.send(player, &ActionFailed);
            return false;
        }

        self.set_casting_now(player, true);
        // The current skill keeps the key presses (`Creature._lastSkillCast` could replace it).
        self.set_current_skill(
            player,
            Some(SkillUse {
                skill: key,
                ctrl_pressed: force_use,
                shift_pressed: dont_move,
            }),
        );
        if self.queued_skill(player).is_some() {
            self.set_queued_skill(player, None);
        }

        if !self.check_use_magic_conditions(player, skill, force_use, dont_move) {
            self.set_casting_now(player, false);
            return false;
        }

        // Check if the target is correct and Notify the AI with CAST and target.
        let target = match skill.target_type {
            // AURA, SELF should be cast even if no target has been found.
            TargetType::Aura
            | TargetType::FrontAura
            | TargetType::BehindAura
            | TargetType::Ground
            | TargetType::SelfTarget
            | TargetType::AuraCorpseMob
            | TargetType::CommandChannel
            | TargetType::AuraFriendly => Some(player),
            // Get the first target of the list.
            _ => {
                let selected = self.cast_selected_target(player);
                self.cast_target_list(skill, player, true, selected)
                    .first()
                    .copied()
            }
        };
        self.set_intention_cast(player, key, target);
        true
    }

    // ------------------------------------------------------------------ doCast

    /// Java `doCast(skill)` (`Player.doCast` checks the player's conditions again first): the
    /// creature casts at its current target. NPC AI sets the target with
    /// [`World::set_npc_target`] first.
    pub fn do_cast(&mut self, caster: ObjectId, key: SkillKey) {
        let Some(table) = self.skill_table() else {
            return;
        };
        let Some(skill) = table.get(key.id, key.level) else {
            return;
        };
        if self.is_player(caster)
            && let Some(current) = self.current_skill(caster)
            && !self.check_use_magic_conditions(caster, skill, current.ctrl_pressed, current.shift_pressed)
        {
            self.set_casting_now(caster, false);
            self.set_casting_simultaneously_now(caster, false);
            return;
        }
        self.begin_cast(caster, skill, false);
        if self.is_player(caster) {
            self.set_recent_fake_death(caster, false);
        }
    }

    /// Java `doSimultaneousCast(skill)`: potions and herbs, cast alongside a normal cast.
    pub fn do_simultaneous_cast(&mut self, caster: ObjectId, key: SkillKey) {
        let Some(table) = self.skill_table() else {
            return;
        };
        let Some(skill) = table.get(key.id, key.level) else {
            return;
        };
        self.begin_cast(caster, skill, true);
    }

    /// Java `Creature.doCast(skill, target, targets)`: a cast whose targets are already known
    /// (triggered skills).
    pub fn do_cast_at(&mut self, caster: ObjectId, key: SkillKey, target: ObjectId, targets: Vec<ObjectId>) {
        let Some(table) = self.skill_table() else {
            return;
        };
        let Some(skill) = table.get(key.id, key.level) else {
            return;
        };
        if !self.check_do_cast_conditions(caster, skill) {
            self.set_casting_now(caster, false);
            return;
        }
        // Override casting type.
        if skill.simultaneous_cast {
            if !self.check_do_cast_conditions(caster, skill) {
                self.set_casting_simultaneously_now(caster, false);
                return;
            }
            self.stop_effects_on_action(caster);
            self.begin_cast_at(caster, skill, true, Some(target), targets);
            return;
        }
        self.stop_effects_on_action(caster);
        self.begin_cast_at(caster, skill, false, Some(target), targets);
    }

    /// Ends a refused cast: the casting flag goes down; a player's AI goes idle (and, when
    /// `failed`, the client hears `ActionFailed` first).
    fn refuse_cast(&mut self, caster: ObjectId, simultaneously: bool, action_failed: bool) {
        if simultaneously {
            self.set_casting_simultaneously_now(caster, false);
        } else {
            self.set_casting_now(caster, false);
        }
        if self.is_player(caster) {
            if action_failed {
                self.send(caster, &ActionFailed);
            }
            self.set_intention_active(caster);
        }
    }

    /// Java `Creature.beginCast(skill, isSimultaneous)`: the targets, then the cast.
    fn begin_cast(&mut self, caster: ObjectId, skill: &SkillTemplate, is_simultaneous: bool) {
        // Attackables cannot cast while moving.
        if self.is_attackable_npc(caster) && self.is_moving(caster) {
            return;
        }
        if !self.check_do_cast_conditions(caster, skill) {
            self.refuse_cast(caster, is_simultaneous, false);
            return;
        }
        // Override casting type.
        let simultaneously = is_simultaneous || skill.simultaneous_cast;
        self.stop_effects_on_action(caster);

        // Get all possible targets of the skill in a table in function of the skill target type.
        let selected = self.cast_selected_target(caster);
        let targets = self.cast_target_list(skill, caster, false, selected);
        let target = match skill.target_type {
            // We need it to correct facing: `getSummon()` (a player's; nobody else has one).
            TargetType::AreaSummon => self.summon_of(caster),
            // AURA skills should always be using caster as target.
            TargetType::Aura
            | TargetType::AuraCorpseMob
            | TargetType::FrontAura
            | TargetType::BehindAura
            | TargetType::Ground
            | TargetType::AuraFriendly => Some(caster),
            other => {
                let mut doit = matches!(
                    other,
                    TargetType::SelfTarget
                        | TargetType::Pet
                        | TargetType::Servitor
                        | TargetType::Summon
                        | TargetType::OwnerPet
                        | TargetType::Party
                        | TargetType::Clan
                        | TargetType::PartyClan
                        | TargetType::CommandChannel
                );
                if targets.is_empty() {
                    self.refuse_cast(caster, simultaneously, true);
                    return;
                }
                if (is_continuous(skill) && !skill.is_debuff)
                    || self.skill_has_effect_type(skill, &[EffectType::Cpheal, EffectType::Heal])
                {
                    doit = true;
                }
                if doit {
                    targets
                        .first()
                        .copied()
                        .filter(|t| self.objects.get(t).is_some_and(|e| e.kind.is_creature()))
                } else {
                    selected
                }
            }
        };
        self.begin_cast_at(caster, skill, simultaneously, target, targets);
    }

    /// Java `Creature.beginCast(skill, simultaneously, target, targets)`.
    fn begin_cast_at(
        &mut self,
        caster: ObjectId,
        skill: &SkillTemplate,
        simultaneously: bool,
        target: Option<ObjectId>,
        targets: Vec<ObjectId>,
    ) {
        let Some(target) = target else {
            self.refuse_cast(caster, simultaneously, true);
            return;
        };
        let key = SkillKey::of(skill);
        // ON_CREATURE_SKILL_USE: the `TriggerSkillBySkill` listeners (scripts: D-05).
        self.notify_skill_use(caster, key, target);

        // TODO (Java): Unhardcode using event listeners!
        if self.skill_has_effect_type(skill, &[EffectType::Resurrection])
            && (self.is_affected(caster, effect_flag::BLOCK_RESURRECTION)
                || self.is_affected(target, effect_flag::BLOCK_RESURRECTION))
        {
            // Reject resurrection.
            self.send(caster, &SystemMessage::new(SystemMessageId::RejectResurrection));
            self.send(target, &SystemMessage::new(SystemMessageId::RejectResurrection));
            if simultaneously {
                self.set_casting_simultaneously_now(caster, false);
            } else {
                self.set_casting_now(caster, false);
            }
            if self.is_player(caster) {
                self.set_intention_active(caster);
                self.send(caster, &ActionFailed);
            }
            return;
        }

        let skill_time = self.skill_cast_time(caster, skill);

        // Queue herbs and potions.
        if self.is_casting_simultaneously_now(caster) && simultaneously {
            let due = self.tick + TimerWheel::ms_to_ticks(SIMULTANEOUS_RETRY_MS);
            self.timers.schedule_at(
                due,
                TimerKind::Cast(CastTimer::RetryBeginCast {
                    caster,
                    skill: key,
                    simultaneous: simultaneously,
                    target,
                    targets,
                }),
            );
            return;
        }

        // Set the _castInterruptTime and casting status (Player already has this true).
        if simultaneously {
            self.set_casting_simultaneously_now(caster, true);
        } else {
            self.set_casting_now(caster, true);
        }
        if !simultaneously {
            let interrupt = -2 + self.game_ticks() + (skill_time / MILLIS_IN_TICK);
            let state = self.cast_state_mut(caster);
            state.cast_interrupt_time = interrupt;
            state.last_skill_cast = Some(key);
        } else {
            self.cast_state_mut(caster).last_simultaneous_skill_cast = Some(key);
        }

        let mut reuse_delay = self.skill_reuse_delay(caster, skill);

        // Check if this skill consume mp on start casting.
        let initial_mp = self.mp_initial_consume(caster, skill);
        if initial_mp > 0 {
            self.reduce_mp(caster, f64::from(initial_mp));
            let mut su = StatusUpdate::new(caster.0);
            let mp = self.core_of(caster).map_or(0.0, |c| c.cur_mp);
            su.add_attribute(status::CUR_MP, mp as i32);
            self.send(caster, &su);
        }

        // Disable the skill during the re-use delay.
        if reuse_delay > 10 {
            if self.calc_skill_mastery_for(caster, skill) {
                reuse_delay = 100;
                if self.is_playable(caster) {
                    self.send(
                        caster,
                        &SystemMessage::new(SystemMessageId::ASkillIsReadyToBeUsedAgain),
                    );
                }
            }
            if reuse_delay > 1000 {
                self.add_time_stamp(caster, skill, i64::from(reuse_delay));
            } else {
                self.disable_skill(caster, skill, i64::from(reuse_delay));
            }
        }

        // Make sure that char is facing selected target.
        if target != caster
            && let (Some(from), Some(to)) = (self.position_of(caster), self.position_of(target))
        {
            let heading = heading_between(from, to);
            self.set_heading(caster, heading);
            self.broadcast_creature_packet(
                caster,
                &ExRotation {
                    char_id: caster.0,
                    heading,
                },
            );
        }

        if self.is_playable(caster)
            && skill.item_consume_id > 0
            && !self.destroy_item_by_item_id(
                caster,
                skill.item_consume_id,
                i64::from(skill.item_consume_count),
                true,
            )
        {
            // NOTE(java-quirk): `destroyItemByItemId(.., sendMessage = true)` already sent the
            // same message, so the player reads it twice.
            self.send(caster, &SystemMessage::new(SystemMessageId::IncorrectItemCount2));
            self.abort_cast(caster);
            return;
        }
        // M6: a playable's talisman loses mana (`getReferenceItemId()` of item skills on a DECO
        // item; the template does not carry the reference id yet).

        // Broadcast fly effect if needed.
        if let Some(fly_type) = skill.fly_type
            && let Some((x, y, z)) = self.position_of(target)
        {
            self.fly_to_location(caster, x, y, z, fly_type);
            self.set_xyz(caster, x, y, z);
        }

        if !skill.is_toggle() {
            // MagicSkillUse with target, displayId, level, skillTime, reuseDelay to the Creature
            // and to all Player in its known players.
            let (Some(me), Some(at)) = (self.position_of(caster), self.position_of(target)) else {
                return;
            };
            let ground = if self.is_player(caster) {
                self.cast
                    .creatures
                    .get(&caster)
                    .and_then(|s| s.current_skill_world_position)
                    .map(|(x, y, z)| SkillPosition { x, y, z })
            } else {
                None
            };
            self.broadcast_creature_packet(
                caster,
                &MagicSkillUse {
                    creature_id: caster.0,
                    target_id: target.0,
                    skill_id: skill.display_id,
                    skill_level: skill.display_level,
                    hit_time: skill_time,
                    reuse_delay,
                    creature: SkillPosition {
                        x: me.0,
                        y: me.1,
                        z: me.2,
                    },
                    ground_location: ground,
                    target: SkillPosition {
                        x: at.0,
                        y: at.1,
                        z: at.2,
                    },
                },
            );
            self.broadcast_creature_packet(
                caster,
                &MagicSkillLaunched {
                    creature_id: caster.0,
                    skill_id: skill.display_id,
                    skill_level: skill.display_level,
                    target_ids: targets.iter().map(|t| t.0).collect(),
                },
            );
        }

        // Send a system message to the player.
        if self.is_player(caster) && !skill.abnormal_instant {
            match skill.id {
                // Done in Player.startFishing().
                FISHING_SKILL => {}
                WOLF_COLLAR => {
                    self.send(caster, &SystemMessage::new(SystemMessageId::SummoningYourPet));
                }
                _ => {
                    self.send(
                        caster,
                        &SystemMessage::new(SystemMessageId::UseS1).with(skill_name_param(skill)),
                    );
                }
            }
        }

        if !skill.effects.get(EffectScope::Start).is_empty() {
            self.apply_effect_scope_instant(EffectScope::Start, caster, target, key);
        }

        let mut task = MagicUseTask {
            caster,
            targets,
            skill: key,
            count: 0,
            skill_time,
            phase: 1,
            simultaneous: simultaneously,
        };

        // Launch the magic in skillTime milliseconds.
        if skill_time > 0 {
            // Send a Server->Client packet SetupGauge with the color of the gauge and the
            // casting time.
            if self.is_player(caster) && !simultaneously {
                self.send(caster, &SetupGauge::new(caster.0, SetupGauge::BLUE, skill_time));
            }
            if is_channeling(skill) && skill.channeling_skill_id > 0 {
                self.start_channeling(caster, skill);
            }
            // For client animation reasons (party buffs especially) 400 ms before!
            let delay = (skill_time - LAUNCH_BEFORE_HIT_MS).max(0);
            let slot = if simultaneously {
                Slot::Simultaneous
            } else {
                Slot::Normal
            };
            self.schedule_magic_use(task, delay, slot);
        } else {
            task.skill_time = 0;
            self.on_magic_launched_timer(task);
        }
    }

    /// The cast time of `beginCast`: the base time scaled by the cast or attack speed, with the
    /// client's animation floors.
    pub fn skill_cast_time(&self, caster: ObjectId, skill: &SkillTemplate) -> i32 {
        // Get the Base Casting Time of the Skills.
        let base = skill.hit_time.wrapping_add(skill.cool_time);
        let mut skill_time = base;
        if !is_channeling(skill) || skill.channeling_skill_id == 0 {
            // Calculate the Casting Time of the "Non-Static" Skills (with caster PAtk/MAtkSpd).
            if !skill.is_static()
                && let Some(view) = self.stats(caster)
            {
                skill_time = calc_atk_spd(
                    skill.is_magic(),
                    f64::from(skill_time),
                    view.m_atk_spd(),
                    view.p_atk_spd(),
                );
            }
            // Calculate the Casting Time of Magic Skills (reduced in 40% if using SPS/BSPS).
            if skill.is_magic()
                && (self.is_charged_shot(caster, ShotType::Spiritshots)
                    || self.is_charged_shot(caster, ShotType::BlessedSpiritshots))
            {
                skill_time = (0.6 * f64::from(skill_time)) as i32;
            }
        }
        // Avoid broken Casting Animation. Client can't handle less than 550ms Casting Animation
        // in Magic Skills with more than 550ms base.
        if skill.is_magic() && base > 550 && skill_time < 550 {
            skill_time = 550;
        }
        // Client can't handle less than 500ms Casting Animation in Physical Skills with 500ms
        // base or more.
        else if !skill.is_static() && base >= 500 && skill_time < 500 {
            skill_time = 500;
        }
        skill_time
    }

    /// The reuse delay of `beginCast`.
    pub fn skill_reuse_delay(&self, caster: ObjectId, skill: &SkillTemplate) -> i32 {
        let Some(view) = self.stats(caster) else {
            return skill.reuse_delay;
        };
        let base = f64::from(skill.reuse_delay);
        let mut reuse = if skill.static_reuse || skill.is_static() {
            skill.reuse_delay
        } else if skill.is_magic() {
            (base * view.calc(Stat::MagicReuseRate, 1.0)) as i32
        } else if skill.is_physical() {
            (base * view.calc(Stat::PReuse, 1.0)) as i32
        } else {
            (base * view.calc(Stat::DanceReuse, 1.0)) as i32
        };
        if self.is_playable(caster) {
            let class_id = self.players.get(&caster).map_or(0, |p| p.data.class_id);
            let multiplier = ClassBalance::of(&self.settings.cast.skill_reuse_multipliers, class_id);
            // `int * float` is a float in Java.
            reuse = (reuse as f32 * multiplier) as i32;
        }
        reuse
    }

    /// Java `Formulas.calcSkillMastery(actor, skill)`.
    fn calc_skill_mastery_for(&mut self, actor: ObjectId, skill: &SkillTemplate) -> bool {
        let mastery_multipliers = &self.settings.stats.class_balance.skill_mastery_chance;
        let Some(input) = self.stats(actor).map(|v| SkillMasteryInput {
            actor_is_player: v.is_player(),
            skill_is_static: skill.is_static(),
            skill_mastery: v.calc(Stat::SkillMastery, 0.0),
            str_bonus: v.bonus(BaseStat::Str),
            int_bonus: v.bonus(BaseStat::Int),
            skill_mastery_rate: v.calc(Stat::SkillMasteryRate, 1.0),
            class_balance_multiplier: ClassBalance::of(
                mastery_multipliers,
                v.player().map_or(-1, |p| p.data.class_id),
            ),
        }) else {
            return false;
        };
        calc_skill_mastery(&input, self.rng())
    }

    fn schedule_magic_use(&mut self, task: MagicUseTask, delay_ms: i32, slot: Slot) {
        let caster = task.caster;
        let due = self.tick + TimerWheel::ms_to_ticks(u64::try_from(delay_ms).unwrap_or(0));
        let timer = self
            .timers
            .schedule_at(due, TimerKind::Cast(CastTimer::MagicUse(task)));
        let state = self.cast_state_mut(caster);
        match slot {
            Slot::Normal => state.skill_cast = Some(timer),
            Slot::Simultaneous => state.skill_cast2 = Some(timer),
        }
    }

    /// A casting timer fired.
    pub(crate) fn on_cast_timer(&mut self, timer: CastTimer) {
        match timer {
            CastTimer::ChannelTick { channelizer } => self.on_channel_tick(channelizer),
            CastTimer::MagicUse(task) => {
                if !self.is_in_world(task.caster) {
                    return;
                }
                match task.phase {
                    1 => self.on_magic_launched_timer(task),
                    2 => self.on_magic_hit_timer(task),
                    3 => self.on_magic_finalizer(task),
                    _ => {}
                }
            }
            CastTimer::RetryBeginCast {
                caster,
                skill,
                simultaneous,
                target,
                targets,
            } => {
                if !self.is_in_world(caster) {
                    return;
                }
                let Some(table) = self.skill_table() else {
                    return;
                };
                if let Some(template) = table.get(skill.id, skill.level) {
                    self.begin_cast_at(caster, template, simultaneous, Some(target), targets);
                }
            }
            CastTimer::ChangeIntentionToCast {
                caster,
                skill,
                target,
            } => {
                if !self.is_in_world(caster) {
                    return;
                }
                if self.is_attacking_now(caster) {
                    self.abort_attack(caster);
                }
                self.change_intention_to_cast(caster, skill, target);
            }
            CastTimer::AttackAfterCast { caster, target } => {
                if !self.is_in_world(caster) {
                    return;
                }
                if !self.is_disabled(caster) && !self.is_attacking_or_casting_now(caster) {
                    self.set_intention_attack(caster, target);
                }
            }
            CastTimer::CompleteMagicFinalizer {
                caster,
                skill,
                target,
            } => {
                if self.is_in_world(caster) {
                    self.complete_magic_finalizer(caster, skill, target);
                }
            }
            CastTimer::QueuedMagicUse { player, queued } => {
                if self.players.contains_key(&player) {
                    self.use_magic(player, queued.skill, queued.ctrl_pressed, queued.shift_pressed);
                }
            }
        }
    }

    // ------------------------------------------------------------------ the phases

    /// Java `Creature.onMagicLaunchedTimer(mut)`: who is still in reach, then the hit 400 ms
    /// later.
    fn on_magic_launched_timer(&mut self, mut task: MagicUseTask) {
        let caster = task.caster;
        let Some(table) = self.skill_table() else {
            self.abort_cast(caster);
            return;
        };
        let Some(skill) = table.get(task.skill.id, task.skill.level) else {
            self.abort_cast(caster);
            return;
        };

        if task.targets.is_empty() {
            // Only AURA-type skills can be cast without target.
            let aura = matches!(
                skill.target_type,
                TargetType::Aura
                    | TargetType::FrontAura
                    | TargetType::BehindAura
                    | TargetType::AuraCorpseMob
                    | TargetType::AuraFriendly
            );
            if !aura {
                self.abort_cast(caster);
                return;
            }
        }

        // Escaping from under skill's radius and peace zone check. First version, not perfect in
        // AoE skills.
        let mut escape_range = 0;
        if skill.effect_range > escape_range {
            escape_range = skill.effect_range;
        } else if skill.cast_range < 0 && skill.affect_range > 80 {
            escape_range = skill.affect_range;
        }

        if !task.targets.is_empty() && escape_range > 0 {
            let mut skip_range = 0;
            let mut skip_los = 0;
            let mut skip_peace_zone = 0;
            let mut target_list = Vec::new();
            let radius = escape_range.wrapping_add(self.collision_radius_of(caster));
            let party_heal = skill.target_type == TargetType::Party
                && self.skill_has_effect_type(skill, &[EffectType::Heal]);
            let negative = has_negative_effect(skill);
            let player = self.is_player(caster);
            for &target in &task.targets {
                if !self.objects.get(&target).is_some_and(|e| e.kind.is_creature()) {
                    continue;
                }
                let Some(pos) = self.position_of(target) else {
                    continue;
                };
                if !self.inside_radius_3d(caster, pos, radius) {
                    skip_range += 1;
                    continue;
                }
                // Healing party members should ignore LOS.
                if !party_heal && task.skill_time > 550 && !self.can_see_target(caster, target) {
                    skip_los += 1;
                    continue;
                }
                if negative {
                    let in_peace = if player {
                        self.target_is_inside_peace_zone(target, caster)
                    } else {
                        self.is_inside_peace_zone(target, caster, target)
                    };
                    if in_peace {
                        skip_peace_zone += 1;
                        continue;
                    }
                }
                target_list.push(target);
            }
            if target_list.is_empty() {
                if player {
                    let message = if skip_range > 0 {
                        Some(SystemMessageId::TheDistanceIsTooFarAndSoTheCastingHasBeenStopped)
                    } else if skip_los > 0 {
                        Some(SystemMessageId::CannotSeeTarget)
                    } else if skip_peace_zone > 0 {
                        Some(SystemMessageId::AMaliciousSkillCannotBeUsedInAPeaceZone)
                    } else {
                        None
                    };
                    if let Some(message) = message {
                        self.send(caster, &SystemMessage::new(message));
                    }
                }
                self.abort_cast(caster);
                return;
            }
            task.targets = target_list;
        }

        // Ensure that a cast is in progress. Check if player is using fake death. Static skills
        // can be used while faking death.
        let lost = if task.simultaneous {
            !self.is_casting_simultaneously_now(caster)
        } else {
            !self.is_casting_now(caster)
        };
        if lost || (self.is_alike_dead(caster) && !skill.is_static()) {
            // Now cancels both, simultaneous and normal.
            self.notify_action_cancel(caster);
            return;
        }

        task.phase = 2;
        if task.skill_time == 0 {
            self.on_magic_hit_timer(task);
        } else {
            // NOTE(java-quirk): the hit of a simultaneous cast is also put into `_skillCast`, so a
            // normal cast that begins meanwhile cancels it.
            self.schedule_magic_use(task, LAUNCH_BEFORE_HIT_MS, Slot::Normal);
        }
    }

    /// Java `Creature.onMagicHitTimer(mut)`: hate, costs, `callSkill`; the finalizer next.
    fn on_magic_hit_timer(&mut self, mut task: MagicUseTask) {
        let caster = task.caster;
        let Some(table) = self.skill_table() else {
            self.abort_cast(caster);
            return;
        };
        let Some(skill) = table.get(task.skill.id, task.skill.level) else {
            self.abort_cast(caster);
            return;
        };

        // Go through targets table.
        for &target in &task.targets {
            if self.is_playable(target) {
                if self.is_player(caster) && self.is_summon(target) {
                    self.update_and_broadcast_summon_status(target, 1);
                }
            } else if self.is_playable(caster) && self.is_attackable_npc(target) {
                if skill.effect_point > 0 {
                    // M5-EFX-03: `target.asAttackable().reduceHate(this, skill.getEffectPoint())`.
                } else if skill.effect_point < 0 {
                    self.add_damage_hate(target, caster, 0, -i64::from(skill.effect_point));
                }
            }
        }

        // The shots are charged again for this cast (`getAutoSoulShot`), before the skill lands
        // and uses them up.
        let (soul, spirit) = (
            self.skill_uses_soul_shot(skill),
            self.skill_uses_spirit_shot(skill),
        );
        self.recharge_shots(caster, soul, spirit);

        let mut su = StatusUpdate::new(caster.0);
        let mut send_status = false;

        // Consume MP of the Creature.
        let mp_consume = self.mp_consume(caster, skill);
        if mp_consume > 0 {
            let cur_mp = self.core_of(caster).map_or(0.0, |c| c.cur_mp);
            if f64::from(mp_consume) > cur_mp {
                self.send(caster, &SystemMessage::new(SystemMessageId::NotEnoughMp));
                self.abort_cast(caster);
                return;
            }
            self.reduce_mp(caster, f64::from(mp_consume));
            let mp = self.core_of(caster).map_or(0.0, |c| c.cur_mp);
            su.add_attribute(status::CUR_MP, mp as i32);
            send_status = true;
        }

        // Consume HP if necessary.
        if skill.hp_consume > 0 {
            let consume_hp = f64::from(skill.hp_consume);
            let cur_hp = self.core_of(caster).map_or(0.0, |c| c.cur_hp);
            if consume_hp >= cur_hp {
                self.send(caster, &SystemMessage::new(SystemMessageId::NotEnoughHp));
                self.abort_cast(caster);
                return;
            }
            // `_status.reduceHp(consumeHp, this, true)`: awake, not marked as HP consumption.
            if self.is_player(caster) {
                self.player_reduce_hp(caster, consume_hp, Some(caster));
            } else {
                self.reduce_hp(caster, consume_hp, Some(caster));
            }
            let hp = self.core_of(caster).map_or(0.0, |c| c.cur_hp);
            su.add_attribute(status::CUR_HP, hp as i32);
            send_status = true;
        }

        // Send a Server->Client packet StatusUpdate with MP modification to the Player.
        if send_status {
            self.send(caster, &su);
        }

        if self.is_player(caster) {
            // Consume Charges.
            if skill.charge_consume > 0 {
                self.decrease_charges(caster, skill.charge_consume);
            }
            // Consume Souls if necessary.
            if skill.soul_max_consume > 0 && !self.decrease_souls(caster, skill.soul_max_consume) {
                self.abort_cast(caster);
                return;
            }
        }

        // Launch the magic skill in order to calculate its effects.
        self.call_skill(caster, skill, task.targets.clone());

        if task.skill_time > 0 {
            task.count += 1;
        }
        task.phase = 3;
        if task.skill_time == 0 {
            self.on_magic_finalizer(task);
        } else {
            let slot = if task.simultaneous {
                Slot::Simultaneous
            } else {
                Slot::Normal
            };
            self.schedule_magic_use(task, 0, slot);
        }
    }

    /// Java `Creature.onMagicFinalizer(mut)`: the cast is over.
    fn on_magic_finalizer(&mut self, task: MagicUseTask) {
        let caster = task.caster;
        if task.simultaneous {
            let state = self.cast_state_mut(caster);
            state.skill_cast2 = None;
            state.casting_simultaneously_now = false;
            return;
        }
        // Cleanup.
        {
            let state = self.cast_state_mut(caster);
            state.skill_cast = None;
            state.cast_interrupt_time = 0;
        }
        // On each repeat recharge shots before cast.
        if task.count > 0
            && let Some(table) = self.skill_table()
            && let Some(skill) = table.get(task.skill.id, task.skill.level)
        {
            let (soul, spirit) = (
                self.skill_uses_soul_shot(skill),
                self.skill_uses_spirit_shot(skill),
            );
            self.recharge_shots(caster, soul, spirit);
        }

        // Stop casting.
        self.set_casting_now(caster, false);
        self.set_casting_simultaneously_now(caster, false);

        let Some(table) = self.skill_table() else {
            return;
        };
        let Some(skill) = table.get(task.skill.id, task.skill.level) else {
            return;
        };
        let target = task.targets.first().copied();

        // Attack target after skill use.
        let current = self.cast_selected_target(caster);
        if skill.next_action_is_attack
            && let Some(t) = target
            && current != Some(caster)
            && current == Some(t)
            && self.can_be_attacked(t)
        {
            // Auto play is not ported.
            let next = self.next_intention(caster);
            if next != Some(Intention::MoveTo) {
                if self.is_player(caster) {
                    if self.current_skill(caster).is_none_or(|c| !c.shift_pressed) {
                        let due = self.tick + TimerWheel::ms_to_ticks(NEXT_ACTION_DELAY_MS);
                        self.timers.schedule_at(
                            due,
                            TimerKind::Cast(CastTimer::AttackAfterCast { caster, target: t }),
                        );
                    }
                } else {
                    self.set_intention_attack(caster, t);
                }
            } else if self.is_player(caster) {
                // Player is moving: wait for skill land animation.
                let due = self.tick + TimerWheel::ms_to_ticks(NEXT_ACTION_DELAY_MS);
                self.timers.schedule_at(
                    due,
                    TimerKind::Cast(CastTimer::CompleteMagicFinalizer {
                        caster,
                        skill: task.skill,
                        target,
                    }),
                );
                return;
            }
        }
        self.complete_magic_finalizer(caster, task.skill, target);
    }

    /// Java `Creature.completeMagicFinalizer(skill, target)`.
    fn complete_magic_finalizer(&mut self, caster: ObjectId, key: SkillKey, _target: Option<ObjectId>) {
        let Some(table) = self.skill_table() else {
            return;
        };
        let Some(skill) = table.get(key.id, key.level) else {
            return;
        };
        if has_negative_effect(skill) && skill.target_type != TargetType::Unlockable {
            self.client_start_auto_attack(caster);
        }
        // Notify the AI of the Creature with FINISH_CASTING.
        self.notify_action_finish_casting(caster);
        // Notify DP Scripts (`notifyQuestEventSkillFinished`): D-05.

        // If character is a player, then wipe their current cast state and check if a skill is
        // queued. If there is a queued skill, launch it and wipe the queue.
        if self.is_player(caster) {
            let queued = self.queued_skill(caster);
            self.set_current_skill(caster, None);
            if let Some(queued) = queued {
                self.set_queued_skill(caster, None);
                // DO NOT USE: Recursive call to useMagic() method (Java runs it as a task).
                let due = self.tick + TimerWheel::ms_to_ticks(0);
                self.timers.schedule_at(
                    due,
                    TimerKind::Cast(CastTimer::QueuedMagicUse {
                        player: caster,
                        queued,
                    }),
                );
            }
        }
        if self.is_channeling(caster) {
            self.stop_channeling(caster);
        }
    }

    // ------------------------------------------------------------------ abort / break

    /// Java `Creature.abortCast()`: the running cast stops, `MagicSkillCanceled` to everybody,
    /// `ActionFailed` to the caster.
    pub fn abort_cast(&mut self, id: ObjectId) {
        if !(self.is_casting_now(id) || self.is_casting_simultaneously_now(id)) {
            return;
        }
        // Cancels the skill hit scheduled task.
        let (normal, simultaneous) = {
            let state = self.cast_state_mut(id);
            (state.skill_cast.take(), state.skill_cast2.take())
        };
        for timer in [normal, simultaneous].into_iter().flatten() {
            self.timers.cancel(timer);
        }
        // TODO in Java: handle removing the spawned npc.
        if self.is_channeling(id) {
            self.stop_channeling(id);
        }
        if self
            .cast
            .creatures
            .get(&id)
            .is_some_and(|c| c.reuse.all_skills_disabled)
        {
            // This remains for forced skill use, e.g. scroll of escape.
            self.enable_all_skills(id);
        }
        self.set_casting_now(id, false);
        self.set_casting_simultaneously_now(id, false);
        // Safeguard for cannot be interrupt any more.
        self.cast_state_mut(id).cast_interrupt_time = 0;
        if self.is_player(id) {
            // Setting back previous intention.
            self.notify_action_finish_casting(id);
        }
        // Broadcast packet to stop animations client-side.
        self.broadcast_creature_packet(id, &MagicSkillCanceled { object_id: id.0 });
        // Send an "action failed" packet to the caster.
        self.send(id, &ActionFailed);
    }

    /// Java `Creature.breakCast()`: damage interrupts a spell (or a static skill) that is still
    /// before its interrupt time.
    pub fn break_cast(&mut self, id: ObjectId) {
        let Some(last) = self.cast.creatures.get(&id).and_then(|c| c.last_skill_cast) else {
            return;
        };
        if !self.is_casting_now(id) || !self.can_abort_cast(id) {
            return;
        }
        // Damage can only cancel magical & static skills.
        let breakable = self
            .skill_table()
            .and_then(|t| t.get(last.id, last.level).map(|s| s.is_magic() || s.is_static()))
            .unwrap_or(false);
        if !breakable {
            return;
        }
        self.abort_cast(id);
        if self.is_player(id) {
            self.send(
                id,
                &SystemMessage::new(SystemMessageId::YourCastingHasBeenInterrupted),
            );
        }
    }

    // ------------------------------------------------------------------ callSkill

    /// Java `Creature.callSkill(skill, targets)`: the effects land, and everybody concerned
    /// learns about it (AI, attack-by lists, hate of the mobs that watch).
    pub fn call_skill(&mut self, caster: ObjectId, skill: &SkillTemplate, mut targets: Vec<ObjectId>) {
        let key = SkillKey::of(skill);
        // Check if the toggle skill effects are already in progress on the Creature.
        if skill.is_toggle() && self.is_affected_by_skill(caster, skill.id) {
            return;
        }
        if targets.is_empty() {
            return;
        }

        // Block RaidBoss from receiving heals/buffs from players (and their pet/summon).
        let settings = &self.settings.cast;
        let (block_heal, block_buff) = (
            settings.disable_raidboss_heal_from_players,
            settings.disable_raidboss_buff_from_players,
        );
        if (block_heal || block_buff) && self.is_player(caster) {
            let is_heal_skill = self.skill_has_effect_type(
                skill,
                &[
                    EffectType::Heal,
                    EffectType::Cpheal,
                    EffectType::ManahealByLevel,
                    EffectType::ManahealPercent,
                    EffectType::RebalanceHp,
                ],
            );
            let is_buff_skill = !is_heal_skill
                && (self.skill_has_effect_type(
                    skill,
                    &[
                        EffectType::Buff,
                        EffectType::NoblesseBlessing,
                        EffectType::CharmOfLuck,
                        EffectType::Relaxing,
                    ],
                ) || (is_continuous(skill) && !skill.is_debuff));
            let block_raid_boss = (is_heal_skill && block_heal) || (is_buff_skill && block_buff);
            // `obj instanceof RaidBoss`: raid bosses (grand bosses are their own class).
            let raid_boss = |w: &World, t: ObjectId| {
                w.npcs
                    .get(&t)
                    .is_some_and(|n| n.class == crate::npc::NpcClass::RaidBoss)
            };
            if block_raid_boss && targets.iter().any(|t| raid_boss(self, *t)) {
                // If the only target is a RaidBoss, nothing to apply.
                if targets.len() == 1 {
                    return;
                }
                targets.retain(|t| !raid_boss(self, *t));
                if targets.is_empty() {
                    return;
                }
            }
        }

        // Initial checks.
        let caster_level = self.stats(caster).map_or(0, |v| v.level());
        let negative = has_negative_effect(skill);
        for &target in &targets {
            if !self.objects.get(&target).is_some_and(|e| e.kind.is_creature()) {
                continue;
            }
            // Check raid monster attack and check buffing characters who attack raid monsters.
            let (attack_target, cast_target) = if self.has_ai(target) {
                (self.attack_target_of(target), self.cast_target_of(target))
            } else {
                (None, None)
            };
            let level_of = |w: &World, id: ObjectId| w.stats(id).map_or(0, |v| v.level());
            // `giveRaidCurse()` is true for every raid (scripts may turn it off: D-05).
            let raid_watch = |w: &World, other: Option<ObjectId>| {
                other.is_some_and(|o| {
                    w.is_raid(o)
                        && w.combat_state(o).is_some_and(|c| c.attack_by.contains(&target))
                        && caster_level > level_of(w, o) + 8
                })
            };
            let curse = !self.settings.cast.raid_disable_curse
                && ((self.is_raid(target) && caster_level > level_of(self, target) + 8)
                    || (!negative && raid_watch(self, attack_target))
                    || (!negative && raid_watch(self, cast_target)));
            if curse {
                let curse_skill = if skill.is_magic() { RAID_CURSE } else { RAID_CURSE2 };
                if self
                    .skill_table()
                    .is_some_and(|t| t.get(curse_skill.id, curse_skill.level).is_some())
                {
                    self.abort_attack(caster);
                    self.abort_cast(caster);
                    self.set_intention_idle(caster);
                    self.apply_effects(target, caster, curse_skill);
                } else {
                    warn!(
                        "Skill ID {} level {} is missing in DP!",
                        curse_skill.id, curse_skill.level
                    );
                }
                return;
            }
            // M5: over-hit (`overhitEnabled(true)` on an attackable) is not modelled.
            // Static skills not trigger any chance skills; the weapon's `castOnMagicSkill` and the
            // `_triggerSkills` of the options.
            self.trigger_skills_on_call_skill(caster, target, skill);
        }

        // Launch the magic skill and calculate its effects.
        self.activate_skill(caster, &targets, key);

        // Flag on Raid/Champion (player or owner of summon/pet).
        let pvp = &self.settings.pvp;
        let flag_actor = if self.is_player(caster) {
            Some(caster)
        } else if pvp.flag_player_on_summon_pet_raid_champion_attack && self.is_summon(caster) {
            self.summon_owner(caster)
        } else {
            None
        };
        if skill.effect_point <= 0
            && negative
            && (pvp.flag_player_on_raid_attack || pvp.flag_player_on_champion_attack)
            && let Some(acting_player) = flag_actor
        {
            let hits_raid_or_champion = targets.iter().any(|&t| {
                self.objects.get(&t).is_some_and(|e| e.kind.is_creature())
                    && ((self.is_raid(t) && self.settings.pvp.flag_player_on_raid_attack)
                        || (self.stats(t).is_some_and(|v| v.is_champion())
                            && self.settings.pvp.flag_player_on_champion_attack))
            });
            if hits_raid_or_champion {
                self.update_pvp_status(acting_player);
            }
        }

        // `asPlayer()`: a summon casts for its owner.
        if let Some(player) = self.acting_player(caster) {
            for &target in &targets {
                // ATTACKED and PvPStatus.
                if self.objects.get(&target).is_some_and(|e| e.kind.is_creature()) {
                    if skill.effect_point <= 0 {
                        if self.is_playable(target) && negative {
                            // Casted on target_self but don't harm self.
                            if target != caster {
                                // Combat-mode check.
                                if self.is_player(target) {
                                    self.client_start_auto_attack(target);
                                }
                                // The combat mode of a summon target is its owner's.
                                if let Some(owner) = self.summon_owner(target)
                                    && self.has_ai(target)
                                    && self.players.contains_key(&owner)
                                {
                                    self.client_start_auto_attack(owner);
                                }
                                // Attack of the own pet does not flag the player; triggering a
                                // trap does not flag its owner (no traps); an area skill of 0
                                // effect point does not flag either.
                                if self.summon_of(player) != Some(target)
                                    && !(skill.effect_point == 0 && skill.affect_range > 0)
                                {
                                    self.update_pvp_status_target(player, target);
                                }
                            }
                        } else if self.is_attackable_npc(target) && !matches!(skill.id, LURE | TEMPTATION) {
                            // Add attacker into list.
                            self.add_attacker_to_attack_by_list(target, caster);
                        }
                        // Notify target AI about the attack.
                        if self.has_ai(target)
                            && negative
                            && !self.skill_has_effect_type(skill, &[EffectType::Hate])
                            && skill.abnormal_type != AbnormalType::TurnPassive
                        {
                            self.notify_action_attacked(target, caster);
                        }
                    } else if self.is_player(target) {
                        // Casting non offensive skill on player with pvp flag set or with karma.
                        if target != caster
                            && (self.pvp_flag_of(target) > 0 || self.karma_of_player(target) > 0)
                        {
                            self.update_pvp_status(player);
                        }
                    } else if self.is_attackable_npc(target) {
                        self.update_pvp_status(player);
                    }
                }
            }

            // Mobs in range 1000 see spell.
            let watchers: Vec<ObjectId> = self
                .visible_objects_in_range(player, 1000)
                .into_iter()
                .filter(|o| self.npcs.contains_key(o))
                .collect();
            let not_static = !skill.is_static();
            for npc in watchers {
                // ON_NPC_SKILL_SEE: the clan hall scripts (M7-CHSIEGE-01) and the outpost's of
                // `TerritoryWarSuperClass` (M7-TW-01); the others are D-05.
                self.hall_script_on_skill_see(npc, caster, skill.id, &targets);
                self.territory_on_skill_see(npc, player, skill.id, &targets);
                if !self.is_attackable_npc(npc) {
                    continue;
                }
                // A skill on the own summon alone makes no hate.
                let own_summon_only = targets.len() == 1 && self.summon_of(player) == Some(targets[0]);
                let effect_point = if own_summon_only { 0 } else { skill.effect_point };
                if effect_point > 0 && self.has_ai(npc) && self.intention_of(npc) == Intention::Attack {
                    let npc_target = self.creature_target(npc);
                    for &skill_target in &targets {
                        if npc_target == Some(skill_target) || npc == skill_target {
                            let level = self.stats(npc).map_or(0, |v| v.level());
                            let base = effect_point.wrapping_mul(150) / level.wrapping_add(7);
                            // `originalCaster = isSummon() ? this : player`.
                            let original_caster = if self.is_summon(caster) { caster } else { player };
                            let ratio = self.hate_ratio(npc, original_caster);
                            self.add_damage_hate(npc, original_caster, 0, (f64::from(base) * ratio) as i64);
                        }
                    }
                }
                // Players which are 9 levels above a Raid Boss and cast a skill nearby, are
                // silenced with the Raid Curse skill. Static skills (recipe books, potions and
                // other item skills) are not casting.
                let npc_level = self.stats(npc).map_or(0, |v| v.level());
                if !self.settings.cast.raid_disable_curse
                    && not_static
                    && self.is_raid(npc)
                    && self.is_in_combat(npc)
                    && caster_level - npc_level > 8
                {
                    self.abort_attack(player);
                    self.abort_cast(player);
                    self.set_intention_idle(player);
                    let curse = if negative { RAID_CURSE2 } else { RAID_CURSE };
                    self.apply_effects(npc, player, curse);
                }
            }
        }

        // Notify AI.
        if negative && !self.skill_has_effect_type(skill, &[EffectType::Hate]) {
            for &target in &targets {
                // NOTE(java-quirk): a player's offensive skill already notified the target's AI
                // above, so it hears the attack twice.
                if self.objects.get(&target).is_some_and(|e| e.kind.is_creature()) && self.has_ai(target) {
                    // Notify target AI about the attack.
                    self.notify_action_attacked(target, caster);
                }
            }
        }
    }
}
