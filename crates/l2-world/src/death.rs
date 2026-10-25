//! Dying and coming back: `doDie` of players and NPCs, the `Die` packet, `doRevive`, and the
//! restart point a dead player picks (`RequestRestartPoint`).
// Java: entity/actor/Creature.java (doDie, doRevive, isPendingRevive, canRevive)
// Java: entity/actor/Playable.java (doDie)
// Java: entity/actor/Player.java (doDie, doRevive, onDieDropItem's call site)
// Java: entity/actor/Npc.java (doDie)
// Java: entity/actor/Attackable.java (doDie)
// Java: network/clientpackets/RequestRestartPoint.java
// Java: network/serverpackets/{Die,Revive}.java
//
// # The order of Java's `doDie`
// `World::do_die` (combat/damage.rs) runs the class chain the way Java's `super.doDie` calls do:
// - a player: `Playable.doDie` ([`World::playable_do_die`]; it does not call `Creature.doDie`),
//   then the rest of `Player.doDie` ([`World::player_do_die`]);
// - an NPC: `Creature.doDie` ([`World::creature_do_die`]), then `Npc.doDie`
//   ([`World::npc_do_die`]: the corpse timer), then, for an attackable, `Attackable.doDie`
//   ([`World::attackable_do_die`]).
// The `Die` packet is `notifyActionDeath`'s (`ai/mod.rs`), as in Java.
//
// # Hooks for the other M4 cards (grep the markers)
// - `// M4-EXP-01:` [`World::calculate_rewards`] (exp and sp of the main damage dealer),
//   [`World::calculate_death_exp_penalty`], [`World::restore_exp`].
// - `// M4-DROP-01:` [`World::calculate_rewards`] (`doItemDrop`), [`World::on_die_drop_item`].
// - What an attackable NPC's AI does around its death (clan help call, the attack-by list, the
//   minions and their respawn) is `ai/attackable.rs` and `ai/minions.rs`.
// - The PvP kill and karma gain of the killer is `pvp.rs`; the siege parts (the clan buttons of
//   `Die`, the restart points of the residences, the attackers' respawn delay, the siege NPCs'
//   deaths and the siege zone's death penalty) are `siege/` (M7-SIEGE-01).

use std::collections::{HashMap, HashSet};

use l2_protocol_game::server::{Die, Revive};
use l2_util::ObjectId;
use tracing::{debug, warn};

use crate::respawn::{PendingRespawn, SpawnRuntime};
use crate::teleport::{TeleportTarget, TeleportWhereType};
use crate::timer::TimerId;
use crate::world::World;
use crate::zones::ZoneId;

/// The `.ini` values dying and reviving read. `Default` is the Java default of each key.
#[derive(Debug, Clone, PartialEq)]
pub struct DeathSettings {
    /// `PlayerConfig.RESPAWN_RESTORE_CP` (`RespawnRestoreCP` / 100): the share of the maximum a
    /// revived creature gets back if it has less.
    pub respawn_restore_cp: f64,
    /// `PlayerConfig.RESPAWN_RESTORE_HP` (`RespawnRestoreHP` / 100, default 65).
    pub respawn_restore_hp: f64,
    /// `PlayerConfig.RESPAWN_RESTORE_MP` (`RespawnRestoreMP` / 100).
    pub respawn_restore_mp: f64,
    /// `PlayerConfig.DISCONNECT_AFTER_DEATH` (`DisconnectAfterDeath`; the code default is true, the
    /// shipped `Player.ini` says false).
    pub disconnect_after_death: bool,
    /// `NpcConfig.DEFAULT_CORPSE_TIME` (`DefaultCorpseTime`), seconds: the corpse time of a
    /// creature that has no NPC template.
    pub default_corpse_time: i32,
    /// `NpcConfig.SPOILED_CORPSE_EXTEND_TIME` (`SpoiledCorpseExtendTime`, default 10), seconds
    /// added to the corpse time of a spoiled (or seeded) monster.
    pub spoiled_corpse_extend_time: i32,
    /// `NpcConfig.CORPSE_CONSUME_SKILL_ALLOWED_TIME_BEFORE_DECAY`
    /// (`CorpseConsumeSkillAllowedTimeBeforeDecay`, default 2000), milliseconds: a corpse with
    /// less time left than this is too old to sweep.
    pub corpse_consume_skill_allowed_time_before_decay: i32,
}

impl Default for DeathSettings {
    fn default() -> Self {
        Self {
            respawn_restore_cp: 0.0,
            respawn_restore_hp: 0.65,
            respawn_restore_mp: 0.0,
            disconnect_after_death: true,
            default_corpse_time: 7,
            spoiled_corpse_extend_time: 10,
            corpse_consume_skill_allowed_time_before_decay: 2000,
        }
    }
}

/// Everything death keeps in the world, outside the creatures themselves.
#[derive(Debug, Default)]
pub struct DeathWorld {
    /// The spawns that admin commands made (`//spawn`, `//spawnat`): spawn index
    /// `static count + position` (see `respawn.rs`, `World::spawn_def`).
    pub(crate) runtime_spawns: Vec<l2_data::spawn::NpcSpawn>,
    /// The runtime fields of each spawn (Java `Spawn`), by index into the spawn data.
    pub(crate) spawns: HashMap<usize, SpawnRuntime>,
    /// The census of each spawn group that rolled units (Java `SpawnGroup._aliveOrReserved`).
    pub(crate) groups: HashMap<usize, crate::npc::GroupState>,
    /// Java `DecayTaskManager.DECAY_SCHEDULES`.
    pub(crate) decay: HashMap<ObjectId, TimerId>,
    /// Java `RespawnTaskManager.PENDING_RESPAWNS`.
    pub(crate) respawns: HashMap<ObjectId, PendingRespawn>,
    /// Java `Creature._isPendingRevive`.
    pub(crate) pending_revive: HashSet<ObjectId>,
    /// The world tick each entry of `decay` is due at: the timer wheel does not say, and
    /// `DecayTaskManager.getRemainingTime` (the sweep's old corpse check) needs it.
    pub(crate) decay_due: HashMap<ObjectId, u64>,
    /// Java `Attackable._spoilerObjectId` and `_sweepItems` of the monsters that have either (see
    /// `rewards/drop.rs`). A monster without an entry is not spoiled and has nothing to sweep.
    pub(crate) spoil: HashMap<ObjectId, crate::rewards::drop::SpoilState>,
}

impl World {
    // ------------------------------------------------------------------ doDie

    /// Dying creatures drop their state when they leave the world.
    pub(crate) fn forget_death_state(&mut self, id: ObjectId) {
        self.cancel_decay_task(id);
        self.death.pending_revive.remove(&id);
    }

    /// The part of every `doDie` that is the same: HP to 0, dead. `false` when the creature was
    /// dead already ("Killing is only possible one time").
    fn kill(&mut self, id: ObjectId) -> bool {
        if self.is_dead(id) {
            return false;
        }
        // Now reset currentHp to zero (the dead flag is `status.rs`'s, the only one).
        self.set_current_hp(id, 0.0, true);
        self.set_dead(id, true);
        // The attack in its animation ends with the creature.
        if let Some(state) = self.combat.creatures.get_mut(&id) {
            state.attack_end_ms = 0;
        }
        true
    }

    /// Java `Creature.stopMove(null)` for a creature that is dead: the movement ends and every
    /// player who sees it gets `StopMove`.
    fn stop_move_on_death(&mut self, id: ObjectId) {
        // `stop_move` clears the movement state of a player or an NPC and broadcasts `StopMove`.
        self.stop_move(id, None);
    }

    /// Java `Creature.doDie(killer)` for an NPC. `false` when it was dead already.
    fn creature_do_die(&mut self, id: ObjectId, killer: Option<ObjectId>) -> bool {
        if !self.kill(id) {
            return false;
        }
        // ON_CREATURE_DEATH / ON_CREATURE_KILLED: script events (D-05).

        // Calculate rewards for main damage dealer.
        let main_damage_dealer = if self.is_monster(id) {
            self.get_main_damage_dealer(id)
        } else {
            None
        };
        self.calculate_rewards(id, main_damage_dealer.or(killer));

        // Set target to null and cancel Attack or Cast: an NPC's target belongs to its AI
        // (`notify_action_death`).
        self.stop_move_on_death(id);

        // Stop HP/MP/CP Regeneration task.
        self.stop_hp_mp_regeneration(id);

        // An attackable whose spawn respawns loses its effects (`stopAllEffects()`), one that
        // does not respawn every effect (`stopAllEffectsWithoutExclusions(true, true)`); any
        // other creature keeps those that last through death.
        self.stop_effects_on_death(id);
        // Clan help range aggro on kill.
        self.clan_help_on_kill(id, killer);

        // Send the Server->Client packet StatusUpdate with current HP and MP to all other Player
        // to inform.
        self.broadcast_status_update(id);

        // Notify Creature AI (`Die` is its packet).
        if self.has_ai(id) {
            self.notify_action_death(id);
        }

        // `ZoneManager.getRegion(this).onDeath(this)`: no zone type reacts to a death.
        self.attack_by_clear(id);
        // `isChannelized()` (M5) and the boss defeat announcements (Custom/BossAnnouncements.ini,
        // off by default) are not ported.
        true
    }

    /// Java `Playable.doDie(killer)` for a player.
    fn playable_do_die(&mut self, id: ObjectId, killer: Option<ObjectId>) -> bool {
        // ON_CREATURE_DEATH: script event (D-05).
        if !self.kill(id) {
            return false;
        }

        // Set target to null and cancel Attack or Cast.
        self.set_creature_target(id, None);

        // Stop movement.
        self.stop_move_on_death(id);

        // Stop HP/MP/CP Regeneration task.
        self.stop_hp_mp_regeneration(id);

        // Noblesse blessing and resurrection special keep the buffs (and end themselves); the
        // charm of courage is not modelled.
        let mut delete_buffs = true;
        if self.is_affected(id, crate::effects::effect_flag::NOBLESS_BLESSING) {
            self.stop_effects(id, crate::effects::EffectType::NoblesseBlessing);
            delete_buffs = false;
        }
        if self.is_affected(id, crate::effects::effect_flag::RESURRECTION_SPECIAL) {
            self.stop_effects(id, crate::effects::EffectType::ResurrectionSpecial);
            delete_buffs = false;
        }
        if delete_buffs {
            self.stop_all_effects_except_those_that_last_through_death(id);
        }

        // Send the Server->Client packet StatusUpdate with current HP and MP to all other Player
        // to inform.
        self.broadcast_status_update(id);

        // `ZoneManager.getRegion(this).onDeath(this)`: the siege zone's Battleground Death
        // Syndrome (M7-SIEGE-01); `FlagPvPZone.onDieInside` is not ported. Quests that asked to
        // hear about the death (scripts: D-05), the instance (no instances yet).
        if self.is_player(id) {
            self.on_siege_zone_die_inside(id);
        }

        // `killerPlayer.onKillUpdatePvPKarma(this)`: PvP kills, PK kills and the karma the killer
        // gains (`pvp.rs`). `killer.asPlayer()`: a summon's owner is the killer player.
        if let Some(killer) = killer.and_then(|k| self.acting_player(k)) {
            self.on_kill_update_pvp_karma(killer, id);
        }

        // Notify Creature AI (`Die` is its packet).
        self.notify_action_death(id);
        // `updateEffectIcons()`: there is no effect list yet.
        true
    }

    /// Java `Npc.doDie(killer)` after `Creature.doDie`.
    fn npc_do_die(&mut self, id: ObjectId, killer: Option<ObjectId>) {
        // "Normally this wouldn't really be needed, but for those few exceptions, we do need to
        // reset the weapons back to the initial template weapon."
        let (right, left) = self
            .npcs
            .get(&id)
            .and_then(|n| self.npc_world.template(n.npc_id))
            .map_or((0, 0), |t| (t.rhand_id, t.lhand_id));
        if let Some(npc) = self.npcs.get_mut(&id) {
            npc.right_hand = right;
            npc.left_hand = left;
        }
        // `_currentCollisionRadius` goes back to the template's (`Grow` may have changed it).
        // `_currentCollisionHeight` is the template's already. `_killingBlowWeaponId` is read by
        // scripts only (D-05). Fake players are not ported.
        let radius = self
            .npcs
            .get(&id)
            .and_then(|n| self.npc_world.template(n.npc_id))
            .map(|t| f64::from(t.collision_radius));
        if let (Some(npc), Some(radius)) = (self.npcs.get_mut(&id), radius) {
            npc.collision_radius = radius;
        }
        let _ = killer;
        self.add_decay_task(id);
    }

    /// Java `Attackable.doDie(killer)` after `Npc.doDie`.
    fn attackable_do_die(&mut self, id: ObjectId, killer: Option<ObjectId>) {
        // ON_ATTACKABLE_KILL: script event (D-05).
        let _ = killer;
        // Notify to minions if there are.
        self.minions_on_die(id);
    }

    /// Java `Player.doDie(killer)` after `Playable.doDie`.
    fn player_do_die(&mut self, id: ObjectId, killer: Option<ObjectId>) {
        // Krateis arenas: not modelled.
        // `if (isMounted()) stopFeed()`
        self.mount_on_die(id);
        if self.is_fake_death(id) {
            self.stop_fake_death(id, true);
        }
        if let Some(killer) = killer {
            // Ultimate Combat teams and ON_PLAYER_PVP_KILL (D-05); the PvP/PK reward items and the
            // kill announcements (Custom configs, off by default); fake player karma.
            self.broadcast_status_update(id);

            // Clear resurrect xp calculation.
            self.set_exp_before_death(id, 0);

            // Cursed weapons (M8) are not modelled. A ward or the combat flag of a fortress siege
            // is dropped and nothing else happens (the `else if (_combatFlagEquippedId)` of Java):
            // during Territory War the war's `dropCombatFlag(this, true, false)` (M7-TW-01), else
            // the fortress's (M7-FORT-01).
            if self.is_combat_flag_equipped(id) {
                // NOTE(java-quirk): "TODO: Fort siege during TW??" - a fortress flag stays on its
                // carrier while the war runs.
                if self.is_tw_in_progress() {
                    self.territory_drop_combat_flag(id, true, false);
                } else {
                    self.fort_remove_combat_flag(id);
                }
            } else {
                let inside_pvp_zone = self.is_inside_zone(id, ZoneId::Pvp);
                let inside_siege_zone = self.is_inside_zone(id, ZoneId::Siege);
                // `pk == null || !pk.isCursedWeaponEquipped()`: nobody has a cursed weapon.
                // Check if any item should be dropped.
                self.on_die_drop_item(id, killer);
                // Clan reputation for a kill in a clan war (`clan/war.rs`).
                if !inside_pvp_zone && !inside_siege_zone {
                    self.clan_war_kill_reputation(id, killer);
                }

                // Should not penalize player when lucky, in a non siege PvP zone, has advent
                // blessing or is in an event.
                if inside_siege_zone || !inside_pvp_zone {
                    self.calculate_death_exp_penalty(id, killer);
                }
            }
        }

        // Unsummon Cubics (no `UserInfo`: the player's death sends its own updates).
        self.clear_cubics(id);
        // Channeling, dimensional rift, agathion: not modelled.
        // M5: `calculateDeathPenaltyBuffLevel(killer)` (the death penalty buff is a skill).
        if let Some(summon) = self.summon_of(id) {
            if self.is_betrayed(summon) {
                self.send(
                    id,
                    &l2_protocol_game::server::SystemMessage::new(
                        l2_protocol_game::SystemMessageId::YourPetServitorIsUnresponsiveAndWillNotObeyAnyOrders,
                    ),
                );
            }
            self.summon_cancel_action(summon);
        }
        // Rent pets, the water task, `AntiFeedManager`: not modelled.

        // `AntiFeedManager.setLastDeathTime(getObjectId())`.
        self.set_last_death_time(id);

        // FIXME: Karma reduction tempfix.
        let karma = self.players.get(&id).map_or(0, |p| p.data.karma);
        if karma > 0 {
            self.set_karma(id, if karma < 200 { 0 } else { karma - (karma / 4) });
        }

        if self.settings.death.disconnect_after_death {
            self.add_decay_task(id);
        }
    }

    /// Java `Creature.doDie(killer)` through the class chain: `false` when the creature was dead
    /// already. `killer` is whoever dealt the last hit (a summon's owner counts as the killer in
    /// Java's `asPlayer()` places; summons do not exist yet).
    pub(crate) fn do_die_chain(&mut self, id: ObjectId, killer: Option<ObjectId>) -> bool {
        if self.is_summon(id) {
            // `Summon.doDie` / `Servitor.doDie` around `Playable.doDie`.
            self.summon_do_die_before(id);
            if !self.playable_do_die(id, killer) {
                return false;
            }
            self.summon_do_die_after(id, killer);
            return true;
        }
        if self.is_player(id) {
            if !self.playable_do_die(id, killer) {
                return false;
            }
            self.player_do_die(id, killer);
            return true;
        }
        if !self.creature_do_die(id, killer) {
            return false;
        }
        // `Door.doDie`: "The castle gate has been destroyed." in a siege (M7-SIEGE-02).
        if self.doors.contains_key(&id) {
            self.door_do_die(id);
            return true;
        }
        if self.is_npc(id) {
            self.npc_do_die(id, killer);
            if self.is_attackable_npc(id) {
                self.attackable_do_die(id, killer);
            }
            // `ControlTower`, `FlameTower` and `SiegeFlag.doDie` (M7-SIEGE-01).
            self.siege_npc_do_die(id);
            // `FortCommander.doDie` (M7-FORT-01).
            self.fort_commander_died(id);
            // `TerritoryWard.doDie` (M7-TW-01).
            self.territory_ward_do_die(id, killer);
        }
        true
    }

    // ------------------------------------------------------------------ hooks of other cards

    /// Java `Creature.calculateRewards(killer)`, overridden by `Attackable` (exp, sp and the
    /// drops of the monster that just died). `killer` is Java's argument, which for a monster is
    /// `getMainDamageDealer()` when there is one (`creature_do_die` picks it).
    // M4-DROP-01: `doItemDrop(maxDealer or lastAttacker)` and `EventDropManager.doEventDrop`, in
    //             the same method, before the exp.
    pub(crate) fn calculate_rewards(&mut self, dying: ObjectId, killer: Option<ObjectId>) {
        // M4-DROP-01: the drop half (`rewards/drop.rs`). Java returns early when the aggro list is
        // empty, which skips the exp as well (`reward_exp_and_sp` checks it too).
        self.drop_rewards(dying, killer);
        // Only the drops use the killer; the exp goes to everybody in the aggro list.
        // M4-EXP-01: the exp and SP of the damage shares (`rewards/exp.rs`).
        self.reward_exp_and_sp(dying);
    }

    /// Java `Player.onDieDropItem(killer)`: karma players and players killed by monsters may drop
    /// items.
    // M4-DROP-01: `rewards/drop.rs` (`player_on_die_drop_item`). The chances (`PlayerRateDrop`
    //             ...) are 0 in the shipped `Rates.ini`.
    pub(crate) fn on_die_drop_item(&mut self, dying: ObjectId, killer: ObjectId) {
        self.player_on_die_drop_item(dying, killer);
    }

    /// Java `Player.calculateDeathExpPenalty(killer, atWar)`, called when `PLAYER_DELEVEL`.
    // M4-EXP-01: the exp loss, `_expBeforeDeath`, the lucky/advent/event checks (the zone check is
    //            the caller's): `rewards/exp.rs`.
    pub(crate) fn calculate_death_exp_penalty(&mut self, dying: ObjectId, killer: ObjectId) {
        self.player_calculate_death_exp_penalty(dying, killer);
    }

    /// Java `Player.restoreExp(percent)`: gives back the exp lost at death (`doRevive(power)`).
    // M4-EXP-01: `rewards/exp.rs`.
    pub(crate) fn restore_exp(&mut self, player: ObjectId, percent: f64) {
        self.player_restore_exp(player, percent);
    }

    // ------------------------------------------------------------------ the Die packet

    /// Java `Creature.canRevive()`: always true for an NPC; a player's `_canRevive` is only
    /// cleared by scripts.
    pub fn can_revive(&self, _id: ObjectId) -> bool {
        true
    }

    /// Java `Creature.isPendingRevive()`: `_isDead && _isPendingRevive`.
    pub fn is_pending_revive(&self, id: ObjectId) -> bool {
        self.is_dead(id) && self.death.pending_revive.contains(&id)
    }

    /// Java `setIsPendingRevive(value)`.
    pub(crate) fn set_pending_revive(&mut self, id: ObjectId, value: bool) {
        if value {
            self.death.pending_revive.insert(id);
        } else {
            self.death.pending_revive.remove(&id);
        }
    }

    /// Java `ItemContainer.haveItemForSelfResurrection()` on the player's inventory (worn items
    /// count, the warehouse does not).
    pub(crate) fn have_item_for_self_resurrection(&self, player: ObjectId) -> bool {
        self.players.get(&player).is_some_and(|p| {
            p.data.items.iter().any(|item| {
                (item.loc == "INVENTORY" || item.loc == "PAPERDOLL")
                    && self.data.item_allows_self_resurrection(item.item_id)
            })
        })
    }

    /// `new Die(creature)` as Java builds and writes it.
    ///
    /// The four clan buttons (clan hall, castle, siege headquarters, fortress) are the clan's and
    /// the siege's (`siege/restart.rs`, M7-SIEGE-01); Java writes zeros for a creature that cannot
    /// teleport, has no clan or is jailed. The blue sweep glow is `isSweepActive` (M5-SPOIL-01).
    pub fn die_packet(&self, id: ObjectId) -> Die {
        let can_teleport = self.can_revive(id) && !self.is_pending_revive(id);
        // `!OlympiadManager.isRegistered(player) && !player.isOnEvent()`: neither exists yet.
        let fixed_res = self.players.get(&id).is_some_and(|p| {
            self.have_item_for_self_resurrection(id)
                || self.data.access_level_allows_fixed_res(p.data.access_level)
        });
        let (to_clan_hall, to_castle, to_siege_hq, to_fortress) = if can_teleport {
            self.die_clan_buttons(id)
        } else {
            (false, false, false, false)
        };
        Die {
            object_id: id.0,
            can_teleport,
            to_clan_hall,
            to_castle,
            to_siege_hq,
            // Java `creature.isSweepActive()`: only an attackable with something left to sweep.
            sweepable: self.is_sweep_active(id),
            fixed_res,
            to_fortress,
        }
    }

    // ------------------------------------------------------------------ doRevive

    /// Java `Creature.doRevive()` (with `Player.doRevive()` for a player): the dead flag goes, the
    /// vitals come back to the configured share, everybody who sees the creature gets `Revive`.
    /// A creature that is teleporting waits (`isPendingRevive`) and is revived by the next
    /// `teleToLocation`.
    pub fn do_revive(&mut self, id: ObjectId) {
        if self.is_dead(id) {
            if !self.is_teleporting(id) {
                self.set_pending_revive(id, false);
                self.set_dead(id, false);

                let settings = self.settings.death.clone();
                if let Some(view) = self.stats(id) {
                    let max = (
                        f64::from(view.max_cp()),
                        f64::from(view.max_hp()),
                        f64::from(view.max_mp()),
                    );
                    let cur = self.core_of(id).map(|c| (c.cur_cp, c.cur_hp, c.cur_mp));
                    if let Some((cp, hp, mp)) = cur {
                        if settings.respawn_restore_cp > 0.0 && cp < max.0 * settings.respawn_restore_cp {
                            self.set_current_cp(id, max.0 * settings.respawn_restore_cp, true);
                        }
                        if settings.respawn_restore_hp > 0.0 && hp < max.1 * settings.respawn_restore_hp {
                            self.set_current_hp(id, max.1 * settings.respawn_restore_hp, true);
                        }
                        if settings.respawn_restore_mp > 0.0 && mp < max.2 * settings.respawn_restore_mp {
                            self.set_current_mp(id, max.2 * settings.respawn_restore_mp, true);
                        }
                    }
                }

                // Start broadcast status.
                self.broadcast_creature_packet(id, &Revive { object_id: id.0 });
                // `ZoneManager.getRegion(this).onRevive(this)`: no zone type reacts.
            } else {
                self.set_pending_revive(id, true);
            }
        }
        if self.is_player(id) {
            // Player.doRevive: stop decay task, `updateEffectIcons()`, and the death penalty
            // icon.
            self.cancel_decay_task(id);
            if self.effect_list(id).is_some() {
                self.update_effect_icons(id, false);
            }
            self.send_etc_status_update(id);
            // `_revivePet = false; _reviveRequested = 0; _revivePower = 0;`
            if let Some(control) = self.effects.control.get_mut(&id) {
                control.revive_pet = false;
                control.revive_requested = false;
                control.revive_power = 0.0;
            }
            // The pet revive request, a summon next to its owner in a peace zone, the dimensional
            // rift and the instance's eject timer: not modelled.
            // `if (isMounted()) startFeed(_mountNpcId)`
            self.mount_on_revive(id);
        }
    }

    /// Java `Player.doRevive(revivePower)`: revive, then take back `revivePower` percent of the
    /// exp lost at death.
    pub fn do_revive_with_power(&mut self, id: ObjectId, revive_power: f64) {
        self.do_revive(id);
        self.restore_exp(id, revive_power);
    }

    // ------------------------------------------------------------------ RequestRestartPoint

    /// Java `RequestRestartPoint.runImpl` + `portPlayer`.
    ///
    /// The clan hall, castle, fortress and siege headquarters restart points and the attackers'
    /// wait are the siege's (`siege/restart.rs`, M7-SIEGE-01). Not modelled: events, festival
    /// participants.
    pub(crate) fn on_request_restart_point(&mut self, id: ObjectId, point_type: i32) {
        if !self.players.contains_key(&id) {
            return;
        }
        if !self.can_revive(id) {
            return;
        }
        if self.is_fake_death(id) {
            self.stop_fake_death(id, true);
            return;
        }
        if !self.is_dead(id) {
            return;
        }
        // Custom event resurrection: events are not modelled.
        // An attacker of the siege the player lies in waits `AttackerRespawn` (M7-SIEGE-01).
        if self.siege_delay_attacker_restart(id, point_type) {
            return;
        }
        self.port_player(id, point_type);
    }

    /// Java `RequestRestartPoint.portPlayer` (also the attacker's `DeathTask`).
    pub(crate) fn port_player(&mut self, id: ObjectId, point_type: i32) {
        let name = self
            .players
            .get(&id)
            .map(|p| p.data.name.clone())
            .unwrap_or_default();
        // `player.isJailed()` forces 27; `isFestivalParticipant()` would force 5 (not modelled).
        let point_type = if self.is_jailed(id) { 27 } else { point_type };
        let mut instance = 0;
        let location: Option<(i32, i32, i32)> = match point_type {
            // To clanhall.
            1 => match self.restart_to_clan_hall(id) {
                Some(location) => location,
                None => {
                    warn!(
                        "Player [{name}] called RestartPointPacket - To Clanhall and he doesn't have Clanhall!"
                    );
                    return;
                }
            },
            // To castle: the castle defended in a siege, else the clan's castle.
            2 => match self.restart_to_castle(id) {
                Some(location) => location,
                None => return,
            },
            // To fortress.
            3 => match self.restart_to_fortress(id) {
                Some(location) => location,
                None => {
                    warn!(
                        "Player [{name}] called RestartPointPacket - To Fortress and he doesn't have Fortress!"
                    );
                    return;
                }
            },
            // To siege HQ.
            4 => match self.restart_to_siege_hq(id) {
                Some(location) => location,
                None => {
                    warn!(
                        "Player [{name}] called RestartPointPacket - To Siege HQ and he doesn't have Siege HQ!"
                    );
                    return;
                }
            },
            // Fixed or Player is a festival participant.
            5 => {
                let is_gm = self.is_gm(id);
                if !is_gm && !self.have_item_for_self_resurrection(id) {
                    warn!(
                        "Player [{name}] called RestartPointPacket - Fixed and he isn't festival participant!"
                    );
                    return;
                }
                // `destroyItemByItemId(FEE, ...)`: the three self-resurrection items, in order.
                if is_gm
                    || self.destroy_item_by_item_id(id, 10649, 1, false)
                    || self.destroy_item_by_item_id(id, 13300, 1, false)
                    || self.destroy_item_by_item_id(id, 13128, 1, false)
                {
                    self.do_revive_with_power(id, 100.0);
                    None
                } else {
                    // Festival Participant.
                    // NOTE(java-quirk): an item that allows self-resurrection but is none of the
                    // three above is not consumed, and the player is revived where he stands.
                    instance = self.objects.get(&id).map_or(0, |e| e.instance_id);
                    self.position_of(id)
                }
            }
            // TODO(java): agathion ress.
            6 => None,
            // To jail: only a jailed player.
            27 => {
                if !self.is_jailed(id) {
                    return;
                }
                Some(crate::punishments::JAIL_IN)
            }
            // To town.
            _ => match self.get_tele_to_location(id, TeleportWhereType::Town) {
                Some(location) => Some(location),
                None => {
                    // Java: NullPointerException.
                    warn!("{id}: no restart point; not teleporting.");
                    return;
                }
            },
        };

        // Teleport and revive.
        if let Some((x, y, z)) = location {
            // `setIn7sDungeon(false)`: the Seven Signs dungeon is not modelled.
            self.set_pending_revive(id, true);
            let offset = self.settings.teleport.max_offset_on_teleport;
            self.tele_to_location(
                id,
                TeleportTarget::at(x, y, z)
                    .with_instance(instance)
                    .with_random_offset(offset),
            );
            debug!("{id}: restart point {point_type} -> {x} {y} {z}");
        }
    }
}
