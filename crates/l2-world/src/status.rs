//! HP, MP and CP: the vitals setters, the regeneration task, the status listeners and the
//! `StatusUpdate` broadcast.
// Java: entity/actor/status/CreatureStatus.java (setCurrentHp/Mp, start/stopHpMpRegeneration,
//       doRegeneration, add/removeStatusListener)
// Java: entity/actor/status/PlayerStatus.java (setCurrentCp, doRegeneration, reduceHp)
// Java: entity/actor/status/{NpcStatus,AttackableStatus}.java (reduceHp)
// Java: entity/actor/Creature.java (broadcastStatusUpdate, needHpUpdate)
// Java: entity/actor/Player.java (broadcastStatusUpdate, needCpUpdate, needMpUpdate)
// Java: entity/WorldRegion.java (switchAI: the regeneration part)
// Java: network/serverpackets/StatusUpdate.java
//
// Java keeps this state in each creature's status object. Here it is one `CreatureStatus` per
// creature in `World::statuses` (D-17: data, not a class tree). An entry exists while its creature
// needs one; `sweep_statuses` drops the entries of creatures that have left the world.
//
// The damage path (`reduceHp`, `reduceMp`) is here too, called by `combat/damage.rs`
// (`World::reduce_current_hp`, M4-COMBAT-01). The dead flag ([`World::is_dead`] /
// [`World::set_dead`]) is the only one in the world: `World::do_die` sets it, `World::do_revive`
// clears it (both in `death.rs`, M4-DEATH-01).
//
// Left out, with the card that brings it: the olympiad
// part of the player's status broadcast, the tutorial quest hooks, summons and pets (M5), the
// festival, siege and clan hall regeneration modifiers (no such state yet), `isSitting()` (sitting
// is not modelled), and the champion regeneration (champions are not rolled).

use std::collections::BTreeSet;

use l2_net::Encode;
use l2_protocol_game::server::StatusUpdate;
use l2_protocol_game::server::visibility::status as attr;
use l2_util::ObjectId;

use crate::formulas::{
    CpRegenInput, HpRegenInput, MpRegenInput, RegenKind, RegenPosture, calc_cp_regen, calc_hp_regen,
    calc_mp_regen, get_regenerate_period,
};
use crate::npc::NpcClass;
use crate::player::CreatureCore;
use crate::region::RegionId;
use crate::stats::{BaseStat, Stat, StatView};
use crate::timer::{TimerId, TimerKind, TimerWheel};
use crate::world::World;

/// Java `CreatureStatus.REGEN_FLAG_HP`: the HP is below its maximum and wants regeneration.
const REGEN_FLAG_HP: u8 = 1;
/// Java `CreatureStatus.REGEN_FLAG_MP`.
const REGEN_FLAG_MP: u8 = 2;
/// Java `PlayerStatus.REGEN_FLAG_CP` (`CreatureStatus.REGEN_FLAG_CP`).
const REGEN_FLAG_CP: u8 = 4;

/// Java `Creature.MAX_HP_BAR_PX`: a bar shorter than this reports every change.
const MAX_HP_BAR_PX: f64 = 352.0;

/// Java `Player.broadcastStatusUpdate`: `ThreadPool.schedule(.., 50)`.
const SELF_UPDATE_MS: u64 = 50;

/// How often the world drops the status entries of creatures that left (one minute). Java has no
/// such table; here an entry is only kept while its creature is in the world.
pub(crate) const STATUS_SWEEP_TICKS: u64 = 20 * 60;

/// The multipliers the regeneration formulas read: Java's `PlayerConfig` and `NpcConfig` values
/// after their `/ 100` (the `.ini` value 100 is 1.0). The defaults are Java's defaults.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RegenSettings {
    /// `PlayerConfig.HP_REGEN_MULTIPLIER` (`HpRegenMultiplier`).
    pub hp_regen_multiplier: f64,
    /// `PlayerConfig.MP_REGEN_MULTIPLIER` (`MpRegenMultiplier`).
    pub mp_regen_multiplier: f64,
    /// `PlayerConfig.CP_REGEN_MULTIPLIER` (`CpRegenMultiplier`).
    pub cp_regen_multiplier: f64,
    /// `NpcConfig.RAID_HP_REGEN_MULTIPLIER` (`RaidHpRegenMultiplier`).
    pub raid_hp_regen_multiplier: f64,
    /// `NpcConfig.RAID_MP_REGEN_MULTIPLIER` (`RaidMpRegenMultiplier`).
    pub raid_mp_regen_multiplier: f64,
}

impl Default for RegenSettings {
    fn default() -> Self {
        Self {
            hp_regen_multiplier: 1.0,
            mp_regen_multiplier: 1.0,
            cp_regen_multiplier: 1.0,
            raid_hp_regen_multiplier: 1.0,
            raid_mp_regen_multiplier: 1.0,
        }
    }
}

/// Java `Creature._hpUpdateIncCheck`, `_hpUpdateDecCheck` and `_hpUpdateInterval` (the MP and CP
/// ones of `Player`): the window around the value the last status update was sent for.
#[derive(Debug, Clone, Copy, PartialEq)]
struct BarCheck {
    inc: f64,
    dec: f64,
    interval: f64,
}

impl BarCheck {
    /// Java's constructor values: the window starts at the maximum.
    fn new(max: f64) -> Self {
        let interval = max / MAX_HP_BAR_PX;
        let inc = max;
        Self {
            inc,
            dec: inc - interval,
            interval,
        }
    }
}

/// Java `CreatureStatus.needHpUpdate` (and `Player.needCpUpdate` / `needMpUpdate`, the same
/// logic): `true` when the change is big enough to send a status update.
fn need_update(bar: &mut Option<BarCheck>, cur: f64, max: f64) -> bool {
    if cur <= 1.0 || max < MAX_HP_BAR_PX {
        return true;
    }
    let bar = bar.get_or_insert_with(|| BarCheck::new(max));
    if cur <= bar.dec || cur >= bar.inc {
        if cur == max {
            bar.inc = cur + 1.0;
            bar.dec = cur - bar.interval;
        } else {
            let multi = cur / bar.interval;
            // Java `(int)` cast of a positive value: the truncation is the floor.
            let int_multi = multi as i64;
            let steps = if multi < int_multi as f64 {
                int_multi - 1
            } else {
                int_multi
            };
            bar.dec = bar.interval * steps as f64;
            bar.inc = bar.dec + bar.interval;
        }
        return true;
    }
    false
}

/// Java `Formulas.getRegeneratePeriod` (3 s for a creature, 5 minutes for a door) in world
/// ticks.
fn regen_period_ticks(is_door: bool) -> u64 {
    TimerWheel::ms_to_ticks(u64::try_from(get_regenerate_period(is_door)).unwrap_or(0))
}

/// Java's movement branches of `calcHpRegen` and `calcCpRegen` after `isSitting()` (see
/// `regen_posture`): `!isMoving()`, then `isRunning()`.
fn posture_of(moving: bool, running: bool) -> RegenPosture {
    match (moving, running) {
        (false, _) => RegenPosture::Standing,
        (true, true) => RegenPosture::Running,
        (true, false) => RegenPosture::Walking,
    }
}

/// Java `Npc.getTemplate().getType()` as far as the regeneration task cares: attackables
/// (`Attackable` and its subclasses) start regenerating when their region activates.
fn is_attackable(class: NpcClass) -> bool {
    matches!(
        class,
        NpcClass::Monster
            | NpcClass::RaidBoss
            | NpcClass::GrandBoss
            | NpcClass::Block
            | NpcClass::FriendlyMob
            | NpcClass::Guard
            | NpcClass::QuestGuard
            | NpcClass::Defender
            | NpcClass::FortCommander
    )
}

/// A per-template value of a player (`getTemplate().getBaseHpRegen(level)` ...). Java indexes the
/// level table directly; a level past it reads as 0 here, like `StatView::player_level_value`.
fn player_base(view: &StatView<'_>, value: impl Fn(&l2_data::PlayerTemplate, usize) -> Option<f64>) -> f64 {
    let level = usize::try_from(view.level()).unwrap_or(0);
    view.player_template()
        .and_then(|t| value(t, level))
        .unwrap_or(0.0)
}

/// Java `CreatureStatus` fields that are not HP/MP/CP values (the values live in `CreatureCore`).
#[derive(Debug, Default)]
pub struct CreatureStatus {
    /// Java `_regTask != null`: the regeneration task is running.
    regen_active: bool,
    /// The timer of the next regeneration tick of that task.
    regen_timer: Option<TimerId>,
    /// Java `_flagsRegenActive`: which vitals are below their maximum.
    regen_flags: u8,
    /// Java `isDead()`'s flag. Death (`death.rs`) sets it through [`World::set_dead`] and
    /// `do_revive` clears it.
    dead: bool,
    /// Java `Creature._isInvul` (`setInvul`): set by the GM `//invul` commands. Read through
    /// [`World::is_invul`], which also reads the teleporting state as Java's `isInvul()` does.
    invul: bool,
    /// Java `_statusListener`: the players who target this creature, sorted for determinism.
    listeners: BTreeSet<ObjectId>,
    /// Java `Player._broadcastStatusUpdateTask`: the pending self update of a player.
    self_update_timer: Option<TimerId>,
    /// The update windows of HP, CP and MP (Java `_hpUpdateIncCheck` ...). Created on first use
    /// with the maximum at that time, where Java creates them with the constructor. The MP window
    /// is read only by the party part of the status broadcast (`needMpUpdate`).
    hp_bar: Option<BarCheck>,
    cp_bar: Option<BarCheck>,
    mp_bar: Option<BarCheck>,
}

impl World {
    /// Installs the regeneration multipliers. Call before players enter.
    pub fn set_regen_settings(&mut self, settings: RegenSettings) {
        self.regen = settings;
    }

    /// Whether `id` is one of the creatures this module tracks: a player, an NPC or a door
    /// (Java `Door extends Creature`, M7-SIEGE-02) in the world.
    fn is_tracked(&self, id: ObjectId) -> bool {
        self.players.contains_key(&id) || self.npcs.contains_key(&id) || self.doors.contains_key(&id)
    }

    /// Java `isPlayer()`.
    pub(crate) fn is_player(&self, id: ObjectId) -> bool {
        self.players.contains_key(&id)
    }

    /// The `CreatureCore` of a player, NPC or door.
    pub(crate) fn core_of(&self, id: ObjectId) -> Option<&CreatureCore> {
        self.players
            .get(&id)
            .map(|p| &p.core)
            .or_else(|| self.npcs.get(&id).map(|n| &n.core))
            .or_else(|| self.doors.get(&id).map(|d| &d.core))
    }

    pub(crate) fn core_of_mut(&mut self, id: ObjectId) -> Option<&mut CreatureCore> {
        if let Some(p) = self.players.get_mut(&id) {
            return Some(&mut p.core);
        }
        if let Some(n) = self.npcs.get_mut(&id) {
            return Some(&mut n.core);
        }
        self.doors.get_mut(&id).map(|d| &mut d.core)
    }

    fn status_mut(&mut self, id: ObjectId) -> &mut CreatureStatus {
        self.statuses.entry(id).or_default()
    }

    fn dead_flag(&self, id: ObjectId) -> bool {
        // A door keeps its flag itself: the geodata checks read it without the world.
        if let Some(door) = self.doors.get(&id) {
            return door.dead;
        }
        self.statuses.get(&id).is_some_and(|s| s.dead)
    }

    /// Java `Creature.isDead()`: the one dead flag of the world (see [`Self::set_dead`]).
    pub fn is_dead(&self, id: ObjectId) -> bool {
        self.dead_flag(id)
    }

    /// The NPC template of an NPC (`None` for a player or an unknown id).
    fn npc_template_of(&self, id: ObjectId) -> Option<&l2_data::NpcTemplate> {
        let npc = self.npcs.get(&id)?;
        self.npc_world.template(npc.npc_id)
    }

    /// `(cur, max)` of HP: the stored value and `getMaxHp()`.
    fn hp_and_max(&self, id: ObjectId) -> Option<(f64, f64)> {
        let cur = self.core_of(id)?.cur_hp;
        Some((cur, f64::from(self.stats(id)?.max_hp())))
    }

    fn mp_and_max(&self, id: ObjectId) -> Option<(f64, f64)> {
        let cur = self.core_of(id)?.cur_mp;
        Some((cur, f64::from(self.stats(id)?.max_mp())))
    }

    fn cp_and_max(&self, id: ObjectId) -> Option<(f64, f64)> {
        if !self.is_player(id) {
            return None;
        }
        let cur = self.core_of(id)?.cur_cp;
        Some((cur, f64::from(self.stats(id)?.max_cp())))
    }

    // ------------------------------------------------------------------ setters

    /// Java `CreatureStatus.setCurrentHp(newHp, broadcastPacket)`: sets the HP (capped at the
    /// maximum), starts the regeneration when it is below the maximum, stops it when the last
    /// vital reaches the maximum. `true` when the integer HP changed.
    pub fn set_current_hp(&mut self, id: ObjectId, new_hp: f64, broadcast: bool) -> bool {
        let Some((cur, max)) = self.hp_and_max(id) else {
            return false;
        };
        // Java reads the old value as an `int`; the UserInfo refresh at 30% HP is for players.
        let old = cur as i32;
        let third = max * 0.3;
        let user_info = self.is_player(id)
            && ((f64::from(old) < third && new_hp > third) || (f64::from(old) > third && new_hp < third));
        if self.dead_flag(id) {
            return false;
        }
        let stored = if new_hp >= max {
            self.finish_regen_flag(id, REGEN_FLAG_HP);
            max
        } else {
            self.raise_regen_flag(id, REGEN_FLAG_HP);
            new_hp
        };
        if let Some(core) = self.core_of_mut(id) {
            core.cur_hp = stored;
        }
        let changed = f64::from(old) != stored;
        if changed && broadcast {
            self.broadcast_status_update(id);
        }
        if user_info {
            self.send_user_info(id);
        }
        changed
    }

    /// Java `CreatureStatus.setCurrentMp(newMp, broadcastPacket)`. `true` when the integer MP
    /// changed.
    pub fn set_current_mp(&mut self, id: ObjectId, new_mp: f64, broadcast: bool) -> bool {
        let Some((cur, max)) = self.mp_and_max(id) else {
            return false;
        };
        let old = cur as i32;
        if self.dead_flag(id) {
            return false;
        }
        let stored = if new_mp >= max {
            self.finish_regen_flag(id, REGEN_FLAG_MP);
            max
        } else {
            self.raise_regen_flag(id, REGEN_FLAG_MP);
            new_mp
        };
        if let Some(core) = self.core_of_mut(id) {
            core.cur_mp = stored;
        }
        let changed = f64::from(old) != stored;
        if changed && broadcast {
            self.broadcast_status_update(id);
        }
        changed
    }

    /// Java `PlayerStatus.setCurrentCp(value, broadcastPacket)`: CP is only a player's. `true`
    /// when the integer CP changed.
    pub fn set_current_cp(&mut self, id: ObjectId, value: f64, broadcast: bool) -> bool {
        let Some((cur, max)) = self.cp_and_max(id) else {
            return false;
        };
        let old = cur as i32;
        if self.dead_flag(id) {
            return false;
        }
        let new_cp = value.max(0.0);
        let stored = if new_cp >= max {
            self.finish_regen_flag(id, REGEN_FLAG_CP);
            max
        } else {
            self.raise_regen_flag(id, REGEN_FLAG_CP);
            new_cp
        };
        if let Some(core) = self.core_of_mut(id) {
            core.cur_cp = stored;
        }
        let changed = f64::from(old) != stored;
        if changed && broadcast {
            self.broadcast_status_update(id);
        }
        changed
    }

    /// Java `CreatureStatus.setCurrentHpMp(newHp, newMp)`: both without a broadcast, then one
    /// broadcast if either changed.
    pub fn set_current_hp_mp(&mut self, id: ObjectId, new_hp: f64, new_mp: f64) {
        let hp_changed = self.set_current_hp(id, new_hp, false);
        let mp_changed = self.set_current_mp(id, new_mp, false);
        if hp_changed || mp_changed {
            self.broadcast_status_update(id);
        }
    }

    /// Java `_flagsRegenActive |= flag` and `startHpMpRegeneration()`.
    fn raise_regen_flag(&mut self, id: ObjectId, flag: u8) {
        self.status_mut(id).regen_flags |= flag;
        self.start_hp_mp_regeneration(id);
    }

    /// Java `_flagsRegenActive &= ~flag`, then `stopHpMpRegeneration()` when no flag is left.
    fn finish_regen_flag(&mut self, id: ObjectId, flag: u8) {
        let none_left = {
            let st = self.status_mut(id);
            st.regen_flags &= !flag;
            st.regen_flags == 0
        };
        if none_left {
            self.stop_hp_mp_regeneration(id);
        }
    }

    /// Java `Creature.setInvul(value)`: only the flag. [`World::is_invul`] is what the damage
    /// code reads. A creature that is not in the world keeps no flag.
    pub fn set_invul(&mut self, id: ObjectId, value: bool) {
        if !self.is_tracked(id) {
            return;
        }
        self.status_mut(id).invul = value;
    }

    /// Java `Creature.isInvul()`: `_isInvul || _isTeleporting || isAffected(EffectFlag.INVUL)`.
    /// `Player.isInvul()` also adds `isTeleportProtected()`; the spawn protection is not modelled.
    pub fn is_invul(&self, id: ObjectId) -> bool {
        self.invul_flag(id)
            || self.is_teleport_protected(id)
            || self.is_teleporting(id)
            || self.is_affected(id, crate::effects::effect_flag::INVUL)
    }

    /// Java `_isInvul` alone (the field that `setInvul` writes).
    pub fn invul_flag(&self, id: ObjectId) -> bool {
        self.statuses.get(&id).is_some_and(|s| s.invul)
    }

    /// Java `Creature.setDead`. `World::do_die` sets it, `World::do_revive` clears it (both in
    /// `death.rs`). While it is set, the vitals do not change and no regeneration starts. It does
    /// not stop a running task: Java's `doDie` does that itself.
    pub fn set_dead(&mut self, id: ObjectId, dead: bool) {
        if !self.is_tracked(id) {
            return;
        }
        if self.doors.contains_key(&id) {
            // `Door::dead`, which also decides whether the door blocks.
            self.set_door_dead(id, dead);
            return;
        }
        if dead {
            self.status_mut(id).dead = true;
        } else if let Some(st) = self.statuses.get_mut(&id) {
            st.dead = false;
        }
    }

    /// Java `CreatureStatus.reduceMp(value)`.
    pub fn reduce_mp(&mut self, id: ObjectId, value: f64) {
        let Some((cur, _)) = self.mp_and_max(id) else {
            return;
        };
        self.set_current_mp(id, (cur - value).max(0.0), true);
    }

    // ------------------------------------------------------------------ damage

    /// Java `CreatureStatus.reduceHp(value, attacker, awake = true, isDOT = false,
    /// isHPConsumption = false)` with `NpcStatus` / `AttackableStatus` (their extras are duels,
    /// the attack-by list and over-hit; see the comments). The attackable's aggro list is
    /// `Attackable.reduceCurrentHp`'s, in `combat/damage.rs`.
    pub(crate) fn reduce_hp(&mut self, id: ObjectId, value: f64, attacker: Option<ObjectId>) {
        if self.is_dead(id) {
            return;
        }
        // NpcStatus.reduceHp: a duelist who hits an NPC leaves the duel, then the attacker is added
        // to the attack-by list before CreatureStatus.reduceHp, so this comes before the
        // invulnerability check.
        // A door's status is the plain `CreatureStatus` (`DoorStatus` adds nothing), without
        // `NpcStatus`'s part.
        if let Some(attacker) = attacker.filter(|_| !self.doors.contains_key(&id)) {
            self.duel_interrupt_on_hit(id, attacker);
            self.add_attacker_to_attack_by_list(id, attacker);
        }
        // "invul handling": `isInvul() && !(isDOT || isHPConsumption)`. DOT and HP consumption do
        // not reach this function yet (they are effects, M5).
        if self.is_invul(id) {
            return;
        }
        // M5: a GM without `giveDamage` rights (access level flags are not loaded: every access
        // level may give damage). AttackableStatus: over-hit is a skill effect (M5).
        // `!isDOT && !isHPConsumption`: neither reaches this function yet.
        self.stop_effects_on_damage(id, true);
        // The stun break (`isStunned() && Rnd.get(10) == 0` -> `stopStunning(true)`).
        self.break_states_on_damage(id);
        if value > 0.0 {
            let current = self.core_of(id).map_or(0.0, |c| c.cur_hp);
            self.set_current_hp(id, (current - value).max(0.0), true);
        }
        // `isMortal()`: nothing makes a creature immortal before scripts.
        if self.core_of(id).is_some_and(|c| c.cur_hp < 0.5) {
            self.abort_attack(id);
            self.abort_cast(id);
            self.do_die(id, attacker);
        }
    }

    /// Java `PlayerStatus.reduceHp(value, attacker, awake = true, isDOT = false,
    /// isHPConsumption = false, ignoreCP = false)`: CP takes a playable's hit first, the damage
    /// message, then HP.
    pub(crate) fn player_reduce_hp(&mut self, id: ObjectId, value: f64, attacker: Option<ObjectId>) {
        if self.is_dead(id) {
            return;
        }
        // Java: `player.isInvul() && !(isDOT || isHPConsumption)`, before anything else. DOT and HP
        // consumption do not reach this function yet (M5).
        if self.is_invul(id) {
            return;
        }
        // "If OFFLINE_MODE_NO_DAMAGE is enabled and player is offline and he is in store/craft
        // mode, no damage is taken."
        if self.offline_takes_no_damage(id) {
            return;
        }
        // `!isHPConsumption`: HP consumption does not reach this function yet.
        self.stop_effects_on_damage(id, true);
        // Crafting and private stores (stand up): Sitting players stand up,
        // a stun may break.
        self.break_states_on_damage(id);
        let mut amount = value;
        let mut full_value = amount as i32;
        if let Some(attacker) = attacker.filter(|a| *a != id) {
            // GM `giveDamage` rights: M5.
            // A duelist who is beaten or has won takes no damage; a hit by somebody outside the
            // duel interrupts it.
            if self.duel_ignores_damage(id, attacker) {
                return;
            }
            // Summon damage transfer and mana shield: M5 (effects and summons; their stats are 0
            // without them).
            // Check and calculate the damage transferred to a party member (`TransferDamage`).
            if let Some(rest) = self.party_transfer_damage(id, attacker, amount) {
                amount = rest;
                full_value = rest as i32;
            }
            if self.is_playable(attacker) {
                // `attacker.isPlayable() || attacker.isFakePlayer()`: CP takes the hit first.
                let cp = self.players.get(&id).map_or(0.0, |p| p.core.cur_cp);
                if cp >= amount {
                    // Set Cp to diff of Cp vs value.
                    self.set_current_cp(id, cp - amount, true);
                    // No need to subtract anything from Hp.
                    amount = 0.0;
                } else {
                    // Get diff from value vs Cp; will apply diff to Hp.
                    amount -= cp;
                    self.set_current_cp(id, 0.0, false);
                }
            }
            if full_value > 0 {
                // Send a System Message to the Player.
                let message = l2_protocol_game::server::SystemMessage::new(
                    l2_protocol_game::SystemMessageId::C1HasReceivedS3DamageFromC2,
                )
                .with(l2_protocol_game::server::SystemMessageParam::Text(
                    self.creature_name(id),
                ))
                .with(l2_protocol_game::server::SystemMessageParam::Text(
                    self.creature_name(attacker),
                ))
                .with(l2_protocol_game::server::SystemMessageParam::IntNumber(
                    full_value,
                ));
                self.send(id, &message);
            }
        }

        if amount > 0.0 {
            let current = self.core_of(id).map_or(0.0, |c| c.cur_hp);
            let mut new_hp = (current - amount).max(0.0);
            // Duels end at 1 HP: the duelist is beaten, not killed.
            if current - amount <= 0.0 && self.duel_defeats_instead_of_dying(id, attacker) {
                new_hp = 1.0;
            }
            self.set_current_hp(id, new_hp, true);
        }

        if self.core_of(id).is_some_and(|c| c.cur_hp < 0.5) {
            self.abort_attack(id);
            self.abort_cast(id);
            // A fighter in the olympiad does not die (M8-OLY-01).
            if self.olympiad_fighter_falls(id) {
                return;
            }
            self.do_die(id, attacker);
        }
    }

    // ------------------------------------------------------------------ regeneration task

    /// Java `CreatureStatus.startHpMpRegeneration`: starts the regeneration task (every 3 s) unless
    /// it runs already or the creature is dead. Nothing checks that a vital is below its maximum:
    /// Java does not either, and the first tick stops the task when nothing needs regeneration.
    pub fn start_hp_mp_regeneration(&mut self, id: ObjectId) {
        if !self.is_tracked(id) {
            return;
        }
        if self.statuses.get(&id).is_some_and(|s| s.regen_active || s.dead) {
            return;
        }
        let due = self.tick + regen_period_ticks(self.doors.contains_key(&id));
        let timer = self.timers.schedule_at(due, TimerKind::Regeneration(id));
        let st = self.status_mut(id);
        st.regen_active = true;
        st.regen_timer = Some(timer);
    }

    /// Java `CreatureStatus.stopHpMpRegeneration`: cancels the task and clears the flags.
    pub fn stop_hp_mp_regeneration(&mut self, id: ObjectId) {
        let Some(st) = self.statuses.get_mut(&id) else {
            return;
        };
        if !st.regen_active {
            return;
        }
        st.regen_active = false;
        st.regen_flags = 0;
        let timer = st.regen_timer.take();
        if let Some(timer) = timer {
            self.timers.cancel(timer);
        }
    }

    /// Whether the regeneration task of `id` is running. For tests and tools.
    pub fn is_hp_mp_regenerating(&self, id: ObjectId) -> bool {
        self.statuses.get(&id).is_some_and(|s| s.regen_active)
    }

    /// Java `WorldRegion.switchAI(true)`, the regeneration part: every attackable in the region
    /// starts its regeneration task. The AI and random animation parts come with M4-AI-01.
    pub(crate) fn start_regeneration_in_region(&mut self, region: RegionId) {
        let attackables: Vec<ObjectId> = self
            .regions
            .objects(region)
            .filter(|id| self.npcs.get(id).is_some_and(|n| is_attackable(n.class)))
            .collect();
        for id in attackables {
            self.start_hp_mp_regeneration(id);
        }
    }

    /// A regeneration timer fired: Java's `doRegeneration`, then the next run of the fixed-rate
    /// task, unless `doRegeneration` stopped it.
    pub(crate) fn on_regeneration_timer(&mut self, id: ObjectId) {
        let Some(st) = self.statuses.get_mut(&id) else {
            return;
        };
        st.regen_timer = None;
        if !st.regen_active {
            return;
        }
        if !self.is_tracked(id) {
            // The creature left the world; its task must not outlive it.
            self.forget_status(id);
            return;
        }
        if self.is_player(id) {
            self.player_regeneration(id);
        } else {
            self.creature_regeneration(id);
        }
        // The fixed rate: the next run is one period after this tick. Timers fire on the tick they
        // are due, so this matches Java's `scheduleAtFixedRate` without a catch-up burst.
        if self.statuses.get(&id).is_some_and(|s| s.regen_active) {
            let due = self.tick + regen_period_ticks(self.doors.contains_key(&id));
            let timer = self.timers.schedule_at(due, TimerKind::Regeneration(id));
            self.status_mut(id).regen_timer = Some(timer);
        }
    }

    /// Java `PlayerStatus.doRegeneration`: CP, HP and MP, each only below its recoverable maximum,
    /// then the status broadcast, which goes out even when nothing changed.
    fn player_regeneration(&mut self, id: ObjectId) {
        if let Some((cur, _)) = self.cp_and_max(id)
            && self
                .stats(id)
                .is_some_and(|v| f64::from(v.max_recoverable_cp()) > cur)
        {
            let gain = self.player_cp_regen(id);
            self.set_current_cp(id, cur + gain, false);
        }
        if let Some((cur, _)) = self.hp_and_max(id)
            && self
                .stats(id)
                .is_some_and(|v| f64::from(v.max_recoverable_hp()) > cur)
        {
            let gain = self.player_hp_regen(id);
            self.set_current_hp(id, cur + gain, false);
        }
        if let Some((cur, _)) = self.mp_and_max(id)
            && self
                .stats(id)
                .is_some_and(|v| f64::from(v.max_recoverable_mp()) > cur)
        {
            let gain = self.player_mp_regen(id);
            self.set_current_mp(id, cur + gain, false);
        }
        self.broadcast_status_update(id);
    }

    /// Java `CreatureStatus.doRegeneration` (NPCs and attackables): HP and MP together, computed
    /// before either is set, then one broadcast if either changed. Stops when nothing needs it.
    fn creature_regeneration(&mut self, id: ObjectId) {
        let Some(view) = self.stats(id) else {
            return;
        };
        let (cur_hp, cur_mp) = match self.core_of(id) {
            Some(core) => (core.cur_hp, core.cur_mp),
            None => return,
        };
        let needs =
            cur_hp < f64::from(view.max_recoverable_hp()) || cur_mp < f64::from(view.max_recoverable_mp());
        if self.dead_flag(id) || !needs {
            self.stop_hp_mp_regeneration(id);
            return;
        }
        let new_hp = cur_hp + self.npc_hp_regen(id);
        let new_mp = cur_mp + self.npc_mp_regen(id);
        self.set_current_hp_mp(id, new_hp, new_mp);
    }

    // ------------------------------------------------------------------ regeneration per period

    /// Sitting, else standing, walking or running.
    fn regen_posture(&self, id: ObjectId) -> RegenPosture {
        if self.is_sitting(id) {
            return RegenPosture::Sitting;
        }
        self.stats(id).map_or(RegenPosture::Standing, |v| {
            posture_of(v.is_moving(), v.is_running())
        })
    }

    /// Java `Formulas.calcHpRegen(player)`. The siege modifier and the castle and fort factors
    /// are the siege module's (M7-SIEGE-01), the clan hall's factor the hall's (M7-CH-01); the
    /// festival (M8) and the mother tree bonus are not modelled yet.
    fn player_hp_regen(&self, id: ObjectId) -> f64 {
        let Some(view) = self.stats(id) else {
            return 0.0;
        };
        let kind = RegenKind::Player {
            base_regen: player_base(&view, |t, level| t.base_hp_regen(level)),
            level_mod: view.level_mod(),
            stat_bonus: view.bonus(BaseStat::Con),
            posture: self.regen_posture(id),
        };
        let rate = |base: f64| view.calc_stat(Stat::RegenerateHpRate, base, None, None);
        // Java's order: festival or siege, clan hall, castle, fort.
        // `calcSiegeRegenModifier(player)` when positive (the festival comes first: M8).
        let siege = self.siege_regen_modifier(id);
        let clan_hall = self.clan_hall_regen_factor(id, crate::residence::HALL_FUNC_RESTORE_HP);
        let multipliers: Vec<f64> = (siege > 0.0)
            .then_some(siege)
            .into_iter()
            .chain(clan_hall)
            .chain(self.residence_regen_factors(id, crate::residence::CASTLE_FUNC_RESTORE_HP))
            .collect();
        calc_hp_regen(&HpRegenInput {
            kind,
            is_raid: view.is_raid(),
            hp_regen_multiplier: self.regen.hp_regen_multiplier,
            raid_hp_regen_multiplier: self.regen.raid_hp_regen_multiplier,
            champion_hp_regen: None,
            multipliers: &multipliers,
            mother_tree_hp_regen_bonus: 0,
            regenerate_hp_rate: &rate,
        })
    }

    /// Java `Formulas.calcMpRegen(player)` (see [`Self::player_hp_regen`] for what is not here).
    fn player_mp_regen(&self, id: ObjectId) -> f64 {
        let Some(view) = self.stats(id) else {
            return 0.0;
        };
        let kind = RegenKind::Player {
            base_regen: player_base(&view, |t, level| t.base_mp_regen(level)),
            level_mod: view.level_mod(),
            stat_bonus: view.bonus(BaseStat::Men),
            posture: self.regen_posture(id),
        };
        let rate = |base: f64| view.calc_stat(Stat::RegenerateMpRate, base, None, None);
        let clan_hall = self.clan_hall_regen_factor(id, crate::residence::HALL_FUNC_RESTORE_MP);
        // Then the castle's and the fortress's (M7-SIEGE-01).
        let multipliers: Vec<f64> = clan_hall
            .into_iter()
            .chain(self.residence_regen_factors(id, crate::residence::CASTLE_FUNC_RESTORE_MP))
            .collect();
        calc_mp_regen(&MpRegenInput {
            kind,
            is_raid: view.is_raid(),
            mp_regen_multiplier: self.regen.mp_regen_multiplier,
            raid_mp_regen_multiplier: self.regen.raid_mp_regen_multiplier,
            multipliers: &multipliers,
            mother_tree_mp_regen_bonus: 0,
            regenerate_mp_rate: &rate,
        })
    }

    /// Java `Formulas.calcCpRegen(player)`.
    fn player_cp_regen(&self, id: ObjectId) -> f64 {
        let Some(view) = self.stats(id) else {
            return 0.0;
        };
        let rate = |base: f64| view.calc_stat(Stat::RegenerateCpRate, base, None, None);
        calc_cp_regen(&CpRegenInput {
            base_cp_regen: player_base(&view, |t, level| t.base_cp_regen(level)),
            level_mod: view.level_mod(),
            con_bonus: view.bonus(BaseStat::Con),
            posture: self.regen_posture(id),
            cp_regen_multiplier: self.regen.cp_regen_multiplier,
            regenerate_cp_rate: &rate,
        })
    }

    /// Java `Formulas.calcHpRegen(creature)`: HP regained per period, a player's or an NPC's
    /// (the `//fight_calculator` page prints it for either).
    pub(crate) fn hp_regen_per_period(&self, id: ObjectId) -> f64 {
        if self.players.contains_key(&id) {
            self.player_hp_regen(id)
        } else {
            self.npc_hp_regen(id)
        }
    }

    /// Java `Formulas.calcHpRegen(npc)`: the template value, and the player multiplier unless raid.
    fn npc_hp_regen(&self, id: ObjectId) -> f64 {
        let Some(view) = self.stats(id) else {
            return 0.0;
        };
        let base_reg = self.npc_template_of(id).map_or(0.0, |t| t.base_hp_reg);
        let rate = |base: f64| view.calc_stat(Stat::RegenerateHpRate, base, None, None);
        // `creature.isPet()`: the level row's regeneration times `PetHpRegenMultiplier`.
        let kind = match self.pet_level_data_of(id) {
            Some(level) => RegenKind::Pet {
                pet_regen: level.regen_hp,
                pet_regen_multiplier: self.summons.pet_world.settings.pet_hp_regen_multiplier,
            },
            None => RegenKind::Npc { base_reg },
        };
        calc_hp_regen(&HpRegenInput {
            kind,
            is_raid: view.is_raid(),
            hp_regen_multiplier: self.regen.hp_regen_multiplier,
            raid_hp_regen_multiplier: self.regen.raid_hp_regen_multiplier,
            champion_hp_regen: None,
            multipliers: &[],
            mother_tree_hp_regen_bonus: 0,
            regenerate_hp_rate: &rate,
        })
    }

    /// Java `Formulas.calcMpRegen(npc)`.
    fn npc_mp_regen(&self, id: ObjectId) -> f64 {
        let Some(view) = self.stats(id) else {
            return 0.0;
        };
        let base_reg = self.npc_template_of(id).map_or(0.0, |t| t.base_mp_reg);
        let rate = |base: f64| view.calc_stat(Stat::RegenerateMpRate, base, None, None);
        let kind = match self.pet_level_data_of(id) {
            Some(level) => RegenKind::Pet {
                pet_regen: level.regen_mp,
                pet_regen_multiplier: self.summons.pet_world.settings.pet_mp_regen_multiplier,
            },
            None => RegenKind::Npc { base_reg },
        };
        calc_mp_regen(&MpRegenInput {
            kind,
            is_raid: view.is_raid(),
            mp_regen_multiplier: self.regen.mp_regen_multiplier,
            raid_mp_regen_multiplier: self.regen.raid_mp_regen_multiplier,
            multipliers: &[],
            mother_tree_mp_regen_bonus: 0,
            regenerate_mp_rate: &rate,
        })
    }

    // ------------------------------------------------------------------ status listeners

    /// Java `Creature.addStatusListener`: `listener` (a player) wants `target`'s HP updates. A
    /// creature is never its own listener.
    pub fn add_status_listener(&mut self, target: ObjectId, listener: ObjectId) {
        if target == listener || !self.is_tracked(target) {
            return;
        }
        self.status_mut(target).listeners.insert(listener);
    }

    /// Java `Creature.removeStatusListener`.
    pub fn remove_status_listener(&mut self, target: ObjectId, listener: ObjectId) {
        if let Some(st) = self.statuses.get_mut(&target) {
            st.listeners.remove(&listener);
        }
    }

    // ------------------------------------------------------------------ broadcasts

    /// Java `Creature.broadcastStatusUpdate` for an NPC: the HP to its listeners, when there are
    /// any and the change is big enough. Java `Player.broadcastStatusUpdate`: the player's own
    /// vitals 50 ms later, once however many changes come in between.
    ///
    // NOTE(java-quirk): `Player` overrides `broadcastStatusUpdate` and sends nothing to the
    // status listeners, so a player's target sees its HP only when it is re-targeted.
    pub fn broadcast_status_update(&mut self, id: ObjectId) {
        if self.is_player(id) {
            if self
                .statuses
                .get(&id)
                .is_some_and(|s| s.self_update_timer.is_some())
            {
                return;
            }
            let due = self.tick + TimerWheel::ms_to_ticks(SELF_UPDATE_MS);
            let timer = self.timers.schedule_at(due, TimerKind::SelfStatusUpdate(id));
            self.status_mut(id).self_update_timer = Some(timer);
            return;
        }
        // `Door.broadcastStatusUpdate`: `StaticObjectInfo`, `DoorStatusUpdate` (M7-SIEGE-02).
        if self.doors.contains_key(&id) {
            self.broadcast_door_status(id);
            return;
        }
        if !self.npcs.contains_key(&id) {
            return;
        }
        self.creature_broadcast_status_update(id);
        // `Summon.broadcastStatusUpdate()`: then `updateAndBroadcastStatus(1)`.
        if self.is_summon(id) {
            self.update_and_broadcast_summon_status(id, 1);
        }
    }

    /// Java `Creature.broadcastStatusUpdate()`: the HP to the creature's status listeners when
    /// the bar moved.
    fn creature_broadcast_status_update(&mut self, id: ObjectId) {
        let has_listeners = self.statuses.get(&id).is_some_and(|s| !s.listeners.is_empty());
        if !has_listeners || !self.need_hp_update(id) {
            return;
        }
        let Some(view) = self.stats(id) else {
            return;
        };
        let cur_hp = self.core_of(id).map_or(0.0, |c| c.cur_hp);
        let mut update = StatusUpdate::new(id.0);
        update.add_attribute(attr::MAX_HP, view.max_hp());
        update.add_attribute(attr::CUR_HP, cur_hp as i32);
        let bytes = update.to_bytes();
        let listeners: Vec<ObjectId> = self
            .statuses
            .get(&id)
            .map(|s| s.listeners.iter().copied().collect())
            .unwrap_or_default();
        for listener in listeners {
            self.send_bytes(listener, bytes.clone());
        }
    }

    /// The self update of a player (Java `Player.broadcastStatusUpdate`'s task): the six vitals,
    /// then the party window of the others, the duel window of the opponents, and the olympiad part
    /// (not ported, see the module comment). Those parts read the update windows too, so the windows move as they do
    /// in Java.
    pub(crate) fn on_self_update_timer(&mut self, id: ObjectId) {
        if let Some(st) = self.statuses.get_mut(&id) {
            st.self_update_timer = None;
        }
        if !self.is_player(id) {
            return;
        }
        let Some(view) = self.stats(id) else {
            return;
        };
        let Some(core) = self.core_of(id) else {
            return;
        };
        let mut update = StatusUpdate::new(id.0);
        update.add_attribute(attr::MAX_HP, view.max_hp());
        update.add_attribute(attr::CUR_HP, core.cur_hp as i32);
        update.add_attribute(attr::MAX_MP, view.max_mp());
        update.add_attribute(attr::CUR_MP, core.cur_mp as i32);
        update.add_attribute(attr::MAX_CP, view.max_cp());
        update.add_attribute(attr::CUR_CP, core.cur_cp as i32);
        self.send(id, &update);
        // Java computes needCpUpdate and needHpUpdate for its party and duel parts; needMpUpdate
        // only runs when there is a party and neither of the others asked for the update.
        let need_cp = self.need_cp_update(id);
        let need_hp = self.need_hp_update(id);
        // Check if a party is in progress and the party window update is useful.
        if self.is_in_party(id) && (need_cp || need_hp || self.need_mp_update(id)) {
            self.send_party_small_window_update(id);
        }
        // In duel MP updated only with CP or HP.
        // The olympiad part: the fighter's HP and CP to the observers and the other side
        // (M8-OLY-01).
        if need_cp || need_hp {
            self.olympiad_broadcast_status_update(id);
        }
        if self.is_in_duel(id) && (need_cp || need_hp) {
            if let Some(info) = self.duel_update_user_info(id) {
                self.duel_broadcast_to_opposite_team(id, &info);
            }
        }
    }

    fn need_mp_update(&mut self, id: ObjectId) -> bool {
        let Some((cur, max)) = self.mp_and_max(id) else {
            return false;
        };
        let st = self.status_mut(id);
        need_update(&mut st.mp_bar, cur, max)
    }

    fn need_hp_update(&mut self, id: ObjectId) -> bool {
        let Some((cur, max)) = self.hp_and_max(id) else {
            return false;
        };
        let st = self.status_mut(id);
        need_update(&mut st.hp_bar, cur, max)
    }

    fn need_cp_update(&mut self, id: ObjectId) -> bool {
        let Some((cur, max)) = self.cp_and_max(id) else {
            return false;
        };
        let st = self.status_mut(id);
        need_update(&mut st.cp_bar, cur, max)
    }

    // ------------------------------------------------------------------ lifetime

    /// Java `Player.restore`: `setCurrentCp`, `setCurrentHp` and `setCurrentMp` with the restored
    /// values, then the dead check. Java compares with the zero the constructor left, so it always
    /// broadcasts there; here the restored values are already in place, so only a fractional HP
    /// can send anything.
    pub(crate) fn restore_player_vitals(&mut self, id: ObjectId) {
        let Some(core) = self.core_of(id) else {
            return;
        };
        let (cp, hp, mp) = (core.cur_cp, core.cur_hp, core.cur_mp);
        self.set_current_cp(id, cp, true);
        self.set_current_hp(id, hp, true);
        self.set_current_mp(id, mp, true);
        if hp < 0.5 {
            self.set_dead(id, true);
            self.stop_hp_mp_regeneration(id);
        }
    }

    /// Drops the status of `id` and cancels its timers. A creature that leaves or enters again
    /// starts with no status, as a new Java object does.
    pub(crate) fn forget_status(&mut self, id: ObjectId) {
        let Some(st) = self.statuses.remove(&id) else {
            return;
        };
        for timer in [st.regen_timer, st.self_update_timer].into_iter().flatten() {
            self.timers.cancel(timer);
        }
    }

    /// Drops the entries of creatures that left the world and the listeners that left. See
    /// [`STATUS_SWEEP_TICKS`].
    pub(crate) fn sweep_statuses(&mut self) {
        let gone: Vec<ObjectId> = self
            .statuses
            .keys()
            .copied()
            .filter(|id| !self.is_tracked(*id))
            .collect();
        for id in gone {
            self.forget_status(id);
        }
        for st in self.statuses.values_mut() {
            st.listeners.retain(|l| self.players.contains_key(l));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn posture_follows_the_java_branches() {
        assert_eq!(posture_of(false, false), RegenPosture::Standing);
        assert_eq!(posture_of(false, true), RegenPosture::Standing);
        assert_eq!(posture_of(true, true), RegenPosture::Running);
        assert_eq!(posture_of(true, false), RegenPosture::Walking);
    }

    #[test]
    fn small_bars_always_report() {
        let mut bar = None;
        assert!(need_update(&mut bar, 100.0, 200.0));
        assert!(need_update(&mut bar, 1.0, 5000.0));
        assert!(bar.is_none(), "the window is not touched below the bar size");
    }

    #[test]
    fn bar_windows_move_like_java() {
        // max 3520: 352 bars of 10 HP each.
        let max = 3520.0;
        let mut bar = None;
        // The window starts at the maximum: a value at the top reports and moves the window up.
        assert!(need_update(&mut bar, max, max));
        assert_eq!(
            bar,
            Some(BarCheck {
                inc: 3521.0,
                dec: 3510.0,
                interval: 10.0
            })
        );
        // Inside the window: no update.
        assert!(!need_update(&mut bar, 3519.0, max));
        // Below the window: report, and the window moves to the bar below the value.
        assert!(need_update(&mut bar, 3509.0, max));
        assert_eq!(
            bar,
            Some(BarCheck {
                inc: 3510.0,
                dec: 3500.0,
                interval: 10.0
            })
        );
    }

    #[test]
    fn regen_period_is_three_seconds_of_ticks() {
        assert_eq!(regen_period_ticks(false), 60);
        // A door regenerates every 5 minutes (`HP_REGENERATE_PERIOD * 100`).
        assert_eq!(regen_period_ticks(true), 6000);
    }
}
