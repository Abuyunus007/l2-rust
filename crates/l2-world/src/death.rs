//! Dying and coming back: `doDie` of players and NPCs, the `Die` packet, `doRevive`, and the
//! restart point a dead player picks (`RequestRestartPoint`).
// Java: entity/actor/Creature.java (doDie, doRevive, isPendingRevive, canRevive)
// Java: entity/actor/Playable.java (doDie)
// Java: entity/actor/Player.java (doDie, doRevive, onDieDropItem's call site, setKarma)
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
// - `// M4-AI-01:` what an attackable NPC's AI does around its death (clan help call, the
//   attack-by list, minions).
// - `// M5:` effects, PvP flags and karma gain of the killer, clans, sieges.

use std::collections::{HashMap, HashSet};

use l2_protocol_game::server::visibility::status as attr;
use l2_protocol_game::server::{Die, Revive, StatusUpdate};
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
}

impl Default for DeathSettings {
    fn default() -> Self {
        Self {
            respawn_restore_cp: 0.0,
            respawn_restore_hp: 0.65,
            respawn_restore_mp: 0.0,
            disconnect_after_death: true,
            default_corpse_time: 7,
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
        self.calculate_rewards(id, killer);

        // Set target to null and cancel Attack or Cast: an NPC's target belongs to its AI
        // (M4-AI-01).
        self.stop_move_on_death(id);

        // Stop HP/MP/CP Regeneration task.
        self.stop_hp_mp_regeneration(id);

        // M5: `stopAllEffects()` (a spawn that respawns) or `stopAllEffectsWithoutExclusions`.
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
        self.set_target(id, None);

        // Stop movement.
        self.stop_move_on_death(id);

        // Stop HP/MP/CP Regeneration task.
        self.stop_hp_mp_regeneration(id);

        // M5: noblesse blessing, resurrection special, the charm of courage, and
        // `stopAllEffectsExceptThoseThatLastThroughDeath()`.

        // Send the Server->Client packet StatusUpdate with current HP and MP to all other Player
        // to inform.
        self.broadcast_status_update(id);

        // `ZoneManager.getRegion(this).onDeath(this)`: no zone type reacts to a death. Quests
        // that asked to hear about the death (scripts: D-05), the instance (no instances yet).

        // M5: `killerPlayer.onKillUpdatePvPKarma(this)`: PvP kills, PK kills and the karma the
        // killer gains. It needs the PvP flag state, which does not exist yet.
        let _ = killer;

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
        // `_currentCollisionHeight/Radius` are the template's already (no transformation of NPCs
        // yet). `_killingBlowWeaponId` is read by scripts only (D-05). Fake players are not
        // ported.
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
        // Mounts (`stopFeed`), fake death, Krateis arenas: not modelled.
        if let Some(killer) = killer {
            // Ultimate Combat teams and ON_PLAYER_PVP_KILL (D-05); the PvP/PK reward items and the
            // kill announcements (Custom configs, off by default); fake player karma.
            self.broadcast_status_update(id);

            // Clear resurrect xp calculation.
            // M4-EXP-01: `setExpBeforeDeath(0)`.

            // Cursed weapons and combat flags (territory wars, fort sieges) are not modelled, so
            // the `else` branch of Java's cursed/flag checks always runs.
            let inside_pvp_zone = self.is_inside_zone(id, ZoneId::Pvp);
            let inside_siege_zone = self.is_inside_zone(id, ZoneId::Siege);
            // `pk == null || !pk.isCursedWeaponEquipped()`: nobody has a cursed weapon.
            // Check if any item should be dropped.
            self.on_die_drop_item(id, killer);
            // Clan reputation for a kill in a clan war: clans are M5.

            // Should not penalize player when lucky, in a non siege PvP zone, has advent blessing
            // or is in an event.
            if inside_siege_zone || !inside_pvp_zone {
                self.calculate_death_exp_penalty(id, killer);
            }
        }

        // Cubics, channeling, dimensional rift, agathion: not modelled.
        // M5: `calculateDeathPenaltyBuffLevel(killer)` (the death penalty buff is a skill).
        // Summons (`cancelAction`), rent pets, the water task, `AntiFeedManager`: not modelled.

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
        if self.is_npc(id) {
            self.npc_do_die(id, killer);
            if self.is_attackable_npc(id) {
                self.attackable_do_die(id, killer);
            }
        }
        true
    }

    // ------------------------------------------------------------------ hooks of other cards

    /// Java `Creature.calculateRewards(killer)`, overridden by `Attackable` (exp, sp and the
    /// drops of the monster that just died). `killer` is Java's argument, which for a monster is
    /// `getMainDamageDealer()` when there is one: the caller does not pick it yet.
    // M4-EXP-01: `Attackable.calculateRewards` (the damage shares of the aggro list, party
    //            shares, `addExpAndSp`), `getMustRewardExpSP()`.
    // M4-DROP-01: `doItemDrop(maxDealer or lastAttacker)` and `EventDropManager.doEventDrop`, in
    //             the same method, before the exp.
    pub(crate) fn calculate_rewards(&mut self, dying: ObjectId, killer: Option<ObjectId>) {
        let _ = (dying, killer);
    }

    /// Java `Player.onDieDropItem(killer)`: karma players and players killed by monsters may drop
    /// items.
    // M4-DROP-01: needs items on the ground (`dropItem(ItemProcessType.DEATH, ...)`). The chances
    //             (`PlayerRateDrop` ...) are 0 in the shipped `Rates.ini`.
    pub(crate) fn on_die_drop_item(&mut self, dying: ObjectId, killer: ObjectId) {
        let _ = (dying, killer);
    }

    /// Java `Player.calculateDeathExpPenalty(killer, atWar)`, called when `PLAYER_DELEVEL`.
    // M4-EXP-01: the exp loss, `_expBeforeDeath`, the lucky/advent/event checks (the zone check is
    //            the caller's).
    pub(crate) fn calculate_death_exp_penalty(&mut self, dying: ObjectId, killer: ObjectId) {
        let _ = (dying, killer);
    }

    /// Java `Player.restoreExp(percent)`: gives back the exp lost at death (`doRevive(power)`).
    // M4-EXP-01: needs `_expBeforeDeath`.
    pub(crate) fn restore_exp(&mut self, player: ObjectId, percent: f64) {
        let _ = (player, percent);
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
    /// The four clan buttons (clan hall, castle, siege headquarters, fortress) need clans and
    /// sieges (M5); Java writes zeros for a creature without them. The blue sweep glow is the
    /// spoil of M4-DROP-01 (`isSweepActive`).
    pub fn die_packet(&self, id: ObjectId) -> Die {
        let can_teleport = self.can_revive(id) && !self.is_pending_revive(id);
        // `!OlympiadManager.isRegistered(player) && !player.isOnEvent()`: neither exists yet.
        let fixed_res = self.players.get(&id).is_some_and(|p| {
            self.have_item_for_self_resurrection(id)
                || self.data.access_level_allows_fixed_res(p.data.access_level)
        });
        Die {
            object_id: id.0,
            can_teleport,
            to_clan_hall: false,
            to_castle: false,
            to_siege_hq: false,
            sweepable: false,
            fixed_res,
            to_fortress: false,
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
            // Player.doRevive: stop decay task, `updateEffectIcons()` (no effect list yet), and
            // the death penalty icon.
            self.cancel_decay_task(id);
            self.send_etc_status_update(id);
            // The pet revive request, a summon next to its owner in a peace zone, the mount's
            // food, the dimensional rift and the instance's eject timer: not modelled.
        }
    }

    /// Java `Player.doRevive(revivePower)`: revive, then take back `revivePower` percent of the
    /// exp lost at death.
    pub fn do_revive_with_power(&mut self, id: ObjectId, revive_power: f64) {
        self.do_revive(id);
        self.restore_exp(id, revive_power);
    }

    // ------------------------------------------------------------------ karma

    /// Java `Player.setKarma(value)`: the value (never negative), then `broadcastKarma()`.
    // M5: the guards waking up when a player turns chaotic, and the `RelationChanged` to the
    // players around (relations are not modelled, see `player_view.rs`); `setKarmaFlag()` when
    // the karma ends is `UserInfo`.
    pub(crate) fn set_karma(&mut self, id: ObjectId, value: i32) {
        let karma = value.max(0);
        let old = self.players.get(&id).map_or(0, |p| p.data.karma);
        if let Some(player) = self.players.get_mut(&id) {
            player.data.karma = karma;
        }
        if old > 0 && karma == 0 {
            // Send a Server->Client StatusUpdate packet with Karma and PvP Flag to the Player.
            self.send_user_info(id);
        }
        let mut update = StatusUpdate::new(id.0);
        update.add_attribute(attr::KARMA, karma);
        self.send(id, &update);
    }

    // ------------------------------------------------------------------ RequestRestartPoint

    /// Java `RequestRestartPoint.runImpl` + `portPlayer`.
    ///
    /// Not modelled, so those branches do what Java does for a player without them: clans,
    /// castles, fortresses, sieges, clan halls (the "To clanhall / castle / fortress / siege HQ"
    /// buttons are refused), events, fake death, festival participants, jail.
    pub(crate) fn on_request_restart_point(&mut self, id: ObjectId, point_type: i32) {
        if !self.players.contains_key(&id) {
            return;
        }
        if !self.can_revive(id) {
            return;
        }
        // `player.isFakeDeath()` -> `stopFakeDeath(true)`: fake death is an effect (M5).
        if !self.is_dead(id) {
            return;
        }
        // Custom event resurrection, and an attacker's wait in a siege (castle sieges: M5).
        self.port_player(id, point_type);
    }

    fn port_player(&mut self, id: ObjectId, point_type: i32) {
        let name = self
            .players
            .get(&id)
            .map(|p| p.data.name.clone())
            .unwrap_or_default();
        // `player.isJailed()` forces 27 and `isFestivalParticipant()` forces 5: neither is
        // modelled.
        let mut instance = 0;
        let location: Option<(i32, i32, i32)> = match point_type {
            // To clanhall.
            1 => {
                warn!(
                    "Player [{name}] called RestartPointPacket - To Clanhall and he doesn't have Clanhall!"
                );
                return;
            }
            // To castle: without a siege in progress it needs a clan with a castle.
            2 => return,
            // To fortress.
            3 => {
                warn!(
                    "Player [{name}] called RestartPointPacket - To Fortress and he doesn't have Fortress!"
                );
                return;
            }
            // To siege HQ.
            4 => {
                warn!(
                    "Player [{name}] called RestartPointPacket - To Siege HQ and he doesn't have Siege HQ!"
                );
                return;
            }
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
            // To jail: only a jailed player (jail is not modelled).
            27 => return,
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
