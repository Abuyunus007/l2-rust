//! The siege itself: the start, the owner change in the middle (`midVictory`), the end, the
//! announcements to both sides, the siege state of the participants and the teleports out of the
//! siege zone.
// Java: mechanics/siege/Siege.java (startSiege, endSiege, midVictory, announceToPlayer,
//       updatePlayerSiegeStateFlags, teleportPlayer, getAttackersInZone, getPlayersInZone,
//       getOwnersInZone, getSpectatorsInZone, removeFlags, removeDefenderFlags, killedCT,
//       killedFlag, addAttacker, addDefender, removeAttacker, removeDefender)
// Java: entity/clan/Clan.java (increaseBloodAllianceCount, addSiegeKill, addSiegeDeath,
//       clearSiegeKills, clearSiegeDeaths)

use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{RelationChanged, SystemMessage, SystemMessageParam};
use l2_util::ObjectId;
use tracing::warn;

use super::{SiegeClan, SiegeClanType, SiegeTeleportWhoType, SiegeTimer};
use crate::residence::{CastleId, ResidenceRef, ResidenceWrite};
use crate::teleport::TeleportWhereType;
use crate::world::World;

/// `SiegeManager.getSiegeLength()` is in minutes.
const MINUTE_MS: i64 = 60_000;

impl World {
    // ------------------------------------------------------------------ start

    /// Java `Siege.startSiege()`: a siege without attackers is called off (and the next date
    /// set); otherwise the siege clans are loaded, the participants get their siege state, the
    /// others leave the siege zone, the towers, doors and guards stand, the tickets on the ground
    /// go, the zone is active and the end task runs.
    pub fn start_siege(&mut self, id: CastleId) {
        let Some(siege) = self.castle_siege(id) else {
            return;
        };
        if siege.in_progress {
            return;
        }
        let owner = self.castle(id).map_or(0, |c| c.owner_id);
        let no_attackers = siege.attacker_clans().is_empty();
        if let Some(siege) = self.castle_siege_mut(id) {
            siege.first_owner_clan_id = owner;
        }
        if no_attackers {
            let message = if owner <= 0 {
                SystemMessageId::TheSiegeOfS1HasBeenCanceledDueToLackOfInterest
            } else {
                self.clan_increase_blood_alliance_count(owner);
                SystemMessageId::S1SSiegeWasCanceledBecauseThereWereNoClansThatParticipated
            };
            self.broadcast_to_all_online_players(
                &SystemMessage::new(message).with(SystemMessageParam::CastleName(id.0)),
            );
            self.siege_save_castle_siege(id);
            return;
        }
        if let Some(siege) = self.castle_siege_mut(id) {
            // Atk is now atk.
            siege.is_normal_side = true;
            // Flag so that same siege instance cannot be started again.
            siege.in_progress = true;
        }
        // Load siege clan from db.
        self.siege_load_siege_clan(id);
        self.siege_update_player_siege_state_flags(id, false);
        // Teleport to the closest town.
        self.siege_teleport_player(id, SiegeTeleportWhoType::NotOwner, TeleportWhereType::Town);
        if let Some(siege) = self.castle_siege_mut(id) {
            siege.control_tower_count = 0;
        }
        self.siege_spawn_control_tower(id);
        self.siege_spawn_flame_tower(id);
        self.castle_spawn_door(id, false);
        self.siege_spawn_siege_guard(id);
        // Remove the tickets from the ground.
        self.merc_delete_tickets(id.0);
        self.castle_siege_zone_set_active(id, true);
        // Schedule a task to prepare auto siege end.
        let end = self.siege_now_ms() + i64::from(self.sieges.settings.siege_length) * MINUTE_MS;
        if let Some(siege) = self.castle_siege_mut(id) {
            siege.siege_end_date = end;
        }
        // Prepare auto end task.
        self.schedule_siege_timer(SiegeTimer::End(id), 1000);
        self.broadcast_to_all_online_players(
            &SystemMessage::new(SystemMessageId::TheS1SiegeHasStarted)
                .with(SystemMessageParam::CastleName(id.0)),
        );
        // Scripts: `OnCastleSiegeStart` (D-05).
    }

    // ------------------------------------------------------------------ end

    /// Java `Siege.endSiege()`.
    pub fn end_siege(&mut self, id: CastleId) {
        if !self.is_castle_siege_in_progress(id) {
            return;
        }
        self.broadcast_to_all_online_players(
            &SystemMessage::new(SystemMessageId::TheS1SiegeHasFinished)
                .with(SystemMessageParam::CastleName(id.0)),
        );
        let (owner, name) = self
            .castle(id)
            .map(|c| (c.owner_id, c.name.clone()))
            .unwrap_or_default();
        if owner > 0 {
            // NOTE(java-quirk): Java throws for an owner `ClanTable` does not know.
            if let Some(clan_name) = self.clan(owner).map(|c| c.name.clone()) {
                self.broadcast_to_all_online_players(
                    &SystemMessage::new(SystemMessageId::ClanS1IsVictoriousOverS2SCastleSiege)
                        .with(SystemMessageParam::Text(clan_name))
                        .with(SystemMessageParam::Text(name)),
                );
                let first_owner = self.castle_siege(id).map_or(-1, |s| s.first_owner_clan_id);
                if owner == first_owner {
                    // Owner is unchanged.
                    self.clan_increase_blood_alliance_count(owner);
                } else {
                    // `_castle.setTicketBuyCount(0)` (D-51: the row only with the switch).
                    if let Some(castle) = self.castle_mut(id) {
                        castle.ticket_buy_count = 0;
                    }
                    self.siege_job_write(ResidenceWrite::CastleTicketBuyCount {
                        castle_id: id.0,
                        count: 0,
                    });
                    // M8: `Hero.getInstance().setCastleTaken(objectId, castleId)` of every online
                    // noble member.
                }
            }
        } else {
            self.broadcast_to_all_online_players(
                &SystemMessage::new(SystemMessageId::TheSiegeOfS1HasEndedInADraw)
                    .with(SystemMessageParam::CastleName(id.0)),
            );
        }
        let clans: Vec<i32> = self.castle_siege(id).map_or_else(Vec::new, |s| {
            s.attacker_clans()
                .iter()
                .chain(s.defender_clans().iter())
                .map(|sc| sc.clan_id)
                .collect()
        });
        for clan_id in clans {
            if self.clan(clan_id).is_some() {
                self.clan_clear_siege_kills(clan_id);
            }
        }
        self.castle_update_clans_reputation(id);
        // Removes all flags. Note: Remove flag before teleporting players.
        self.siege_remove_flags(id, true);
        // Teleport to the second closest town.
        self.siege_teleport_player(id, SiegeTeleportWhoType::NotOwner, TeleportWhereType::Town);
        // Flag so that siege instance can be started.
        if let Some(siege) = self.castle_siege_mut(id) {
            siege.in_progress = false;
        }
        self.siege_update_player_siege_state_flags(id, true);
        // Save castle specific data.
        self.siege_save_castle_siege(id);
        // Clear siege clan from db.
        self.siege_clear_siege_clan(id);
        // Remove all towers from this castle.
        self.siege_remove_towers(id);
        // Remove all spawned siege guard from this castle.
        self.siege_unspawn_siege_guard(id);
        if self.castle(id).is_some_and(|c| c.owner_id > 0) {
            self.siege_remove_mercs(id);
        }
        // Respawn door to castle.
        self.castle_spawn_door(id, false);
        if let Some(castle) = self.castle_mut(id) {
            castle.first_mid_victory = false;
        }
        self.castle_siege_zone_set_active(id, false);
        // Scripts: `OnCastleSiegeFinish` (D-05).
    }

    // ------------------------------------------------------------------ midVictory

    /// Java `Siege.midVictory()`: the castle changed hands during the siege (`Castle.setOwner` /
    /// `removeOwner` call it). The new owner defends with its alliance; the old defenders attack;
    /// a siege with no defenders left and one attacker (or only the new owner's alliance) ends.
    pub fn siege_mid_victory(&mut self, id: CastleId) {
        if !self.is_castle_siege_in_progress(id) {
            return;
        }
        let owner = self.castle(id).map_or(0, |c| c.owner_id);
        if owner > 0 {
            // Remove all merc entry from db.
            self.siege_remove_mercs(id);
        }
        let Some(siege) = self.castle_siege(id) else {
            return;
        };
        // If defender doesn't exist (Pc vs Npc) and only 1 attacker.
        if siege.defender_clans().is_empty() && siege.attacker_clans().len() == 1 {
            self.siege_move_new_owner_to_defenders(id, owner);
            self.end_siege(id);
            return;
        }
        if owner <= 0 {
            return;
        }
        // NOTE(java-quirk): Java throws for an owner `ClanTable` does not know.
        let Some(ally_id) = self.clan(owner).map(|c| c.ally_id) else {
            return;
        };
        let Some(siege) = self.castle_siege(id) else {
            return;
        };
        // If defender doesn't exist (Pc vs Npc) and only an alliance attacks and the player's clan
        // is in an alliance.
        if siege.defender_clans().is_empty() && ally_id != 0 {
            let all_in_same_alliance = siege
                .attacker_clans()
                .iter()
                .all(|sc| self.clan(sc.clan_id).is_some_and(|c| c.ally_id == ally_id));
            if all_in_same_alliance {
                self.siege_move_new_owner_to_defenders(id, owner);
                self.end_siege(id);
                return;
            }
        }
        if let Some(siege) = self.castle_siege_mut(id) {
            let defenders = std::mem::take(siege.defender_clans_mut());
            for mut sc in defenders {
                sc.kind = SiegeClanType::Attacker;
                siege.attacker_clans_mut().push(sc);
            }
        }
        self.siege_move_new_owner_to_defenders(id, owner);
        // The player's clan is in an alliance.
        for clan in self.clan_allies(ally_id) {
            if let Some(siege) = self.castle_siege_mut(id)
                && let Some(pos) = siege.attacker_clans().iter().position(|sc| sc.clan_id == clan)
            {
                let mut sc = siege.attacker_clans_mut().remove(pos);
                sc.kind = SiegeClanType::Defender;
                siege.defender_clans_mut().push(sc);
            }
        }
        if let Some(castle) = self.castle_mut(id) {
            castle.first_mid_victory = true;
        }
        // Teleport to the second closest town.
        self.siege_teleport_player(id, SiegeTeleportWhoType::Attacker, TeleportWhereType::SiegeFlag);
        self.siege_teleport_player(id, SiegeTeleportWhoType::Spectator, TeleportWhereType::Town);
        // Removes defenders' flags.
        self.siege_remove_flags(id, false);
        // Remove all castle upgrade.
        self.castle_remove_upgrade(id);
        // Respawn door to castle but make them weaker (50% hp).
        self.castle_spawn_door(id, true);
        // Remove all towers from this castle.
        self.siege_remove_towers(id);
        // Each new siege midvictory CT are completely respawned.
        if let Some(siege) = self.castle_siege_mut(id) {
            siege.control_tower_count = 0;
        }
        self.siege_spawn_control_tower(id);
        self.siege_spawn_flame_tower(id);
        self.siege_update_player_siege_state_flags(id, false);
        // Scripts: `OnCastleSiegeOwnerChange` (D-05).
    }

    /// `removeAttacker(getAttackerClan(owner)); addDefender(sc, OWNER)`.
    // NOTE(java-quirk): `addDefender(null, ...)` does nothing, so an owner that was no attacker
    // (a GM `//setcastle`) is not added.
    fn siege_move_new_owner_to_defenders(&mut self, id: CastleId, owner: i32) {
        let Some(siege) = self.castle_siege_mut(id) else {
            return;
        };
        if let Some(pos) = siege.attacker_clans().iter().position(|sc| sc.clan_id == owner) {
            let mut sc = siege.attacker_clans_mut().remove(pos);
            sc.kind = SiegeClanType::Owner;
            siege.defender_clans_mut().push(sc);
        }
    }

    // ------------------------------------------------------------------ announcements

    /// Java `Siege.announceToPlayer(message, bothSides)`: the online members of the defenders,
    /// and of the attackers with `both_sides`.
    pub fn siege_announce_to_player(&self, id: CastleId, message: &SystemMessage, both_sides: bool) {
        let Some(siege) = self.castle_siege(id) else {
            return;
        };
        let bytes = l2_net::Encode::to_bytes(message);
        let mut clans: Vec<i32> = siege.defender_clans().iter().map(|sc| sc.clan_id).collect();
        if both_sides {
            clans.extend(siege.attacker_clans().iter().map(|sc| sc.clan_id));
        }
        for clan in clans {
            for member in self.clan_online_members(clan, 0) {
                self.send_bytes(member, bytes.clone());
            }
        }
    }

    /// Java `Siege.updatePlayerSiegeStateFlags(clear)`: the online members of the attackers
    /// (state 1) and defenders (state 2) get their siege state, `isInSiege` and the fame task in
    /// the zone, a new `UserInfo` and `RelationChanged` for everybody who sees them.
    pub fn siege_update_player_siege_state_flags(&mut self, id: CastleId, clear: bool) {
        let Some(siege) = self.castle_siege(id) else {
            return;
        };
        let sides: Vec<(i32, u8)> = siege
            .attacker_clans()
            .iter()
            .map(|sc| (sc.clan_id, 1))
            .chain(siege.defender_clans().iter().map(|sc| (sc.clan_id, 2)))
            .collect();
        let (freq, points) = (
            self.sieges.settings.castle_zone_fame_task_frequency,
            self.sieges.settings.castle_zone_fame_aquire_points,
        );
        for (clan, state) in sides {
            for member in self.clan_online_members(clan, 0) {
                if clear {
                    self.set_siege_state(member, 0);
                    self.set_siege_side(member, 0);
                    self.set_in_siege(member, false);
                    self.stop_fame_task(member);
                } else {
                    self.set_siege_state(member, state);
                    self.set_siege_side(member, id.0);
                    if self.siege_check_if_in_zone(id, member) {
                        self.set_in_siege(member, true);
                        self.start_fame_task(member, i64::from(freq) * 1000, points);
                    }
                }
                self.send_user_info(member);
                self.siege_send_relation_to_visible_players(member);
            }
        }
    }

    /// `World.forEachVisibleObject(member, Player.class, ...)`: `RelationChanged` of the member
    /// (and its summon) for every player who sees it.
    pub(crate) fn siege_send_relation_to_visible_players(&self, member: ObjectId) {
        for viewer in self.visible_players(member) {
            if !self.is_visible_for(member, viewer) {
                continue;
            }
            self.send(
                viewer,
                &RelationChanged::single(self.relation_entry(member, viewer)),
            );
            if let Some(summon) = self.summon_of(member) {
                let auto = self.is_auto_attackable(member, viewer);
                let entry = self.summon_relation_entry(summon, viewer, Some(auto));
                self.send(viewer, &RelationChanged::single(entry));
            }
        }
    }

    /// Java `Siege.checkIfInZone(object)`.
    pub fn siege_check_if_in_zone(&self, id: CastleId, object: ObjectId) -> bool {
        self.position_of(object)
            .is_some_and(|(x, y, z)| self.siege_contains(id, x, y, z))
    }

    // ------------------------------------------------------------------ teleports

    /// Java `Siege.getPlayersInZone()`: the players the castle's siege zone holds.
    pub fn siege_players_in_zone(&self, id: CastleId) -> Vec<ObjectId> {
        let Some(zone) = self.castle(id).and_then(|c| c.siege_zone) else {
            return Vec::new();
        };
        self.siege_zone_players(zone)
    }

    /// Java `Siege.getAttackersInZone()`: the attackers' online members that are in the siege.
    pub fn siege_attackers_in_zone(&self, id: CastleId) -> Vec<ObjectId> {
        let Some(siege) = self.castle_siege(id) else {
            return Vec::new();
        };
        siege
            .attacker_clans()
            .iter()
            .flat_map(|sc| self.clan_online_members(sc.clan_id, 0))
            .filter(|p| self.is_in_siege(*p))
            .collect()
    }

    /// Java `Siege.getOwnersInZone()`: the owner's online members that are in the siege.
    pub fn siege_owners_in_zone(&self, id: CastleId) -> Vec<ObjectId> {
        let (Some(siege), Some(owner)) = (self.castle_siege(id), self.castle(id).map(|c| c.owner_id)) else {
            return Vec::new();
        };
        siege
            .defender_clans()
            .iter()
            .filter(|sc| sc.clan_id == owner)
            .flat_map(|sc| self.clan_online_members(sc.clan_id, 0))
            .filter(|p| self.is_in_siege(*p))
            .collect()
    }

    /// Java `Siege.getSpectatorsInZone()`: the players in the zone that are not in the siege.
    pub fn siege_spectators_in_zone(&self, id: CastleId) -> Vec<ObjectId> {
        self.siege_players_in_zone(id)
            .into_iter()
            .filter(|p| !self.is_in_siege(*p))
            .collect()
    }

    /// Java `Siege.teleportPlayer(teleportWho, teleportWhere)`. GMs and jailed players stay.
    pub fn siege_teleport_player(
        &mut self,
        id: CastleId,
        who: SiegeTeleportWhoType,
        place: TeleportWhereType,
    ) {
        let players = match who {
            SiegeTeleportWhoType::Owner => self.siege_owners_in_zone(id),
            SiegeTeleportWhoType::NotOwner => {
                let owner = self.castle(id).map_or(0, |c| c.owner_id);
                // `player.inObserverMode()` (M8-OLY-01).
                self.siege_players_in_zone(id)
                    .into_iter()
                    .filter(|p| {
                        let clan = self.clan_id_of(*p);
                        !self.in_observer_mode(*p) && !(clan > 0 && clan == owner)
                    })
                    .collect()
            }
            SiegeTeleportWhoType::Attacker => self.siege_attackers_in_zone(id),
            SiegeTeleportWhoType::Spectator => self.siege_spectators_in_zone(id),
        };
        for player in players {
            if self.is_gm(player) || self.is_jailed(player) {
                continue;
            }
            self.tele_to_location_where(player, place);
        }
    }

    // ------------------------------------------------------------------ flags

    /// Java `removeFlags()` (`defenders_only = false`: `removeDefenderFlags()`): the headquarters
    /// of the attackers (with `all`) and defenders vanish.
    fn siege_remove_flags(&mut self, id: CastleId, all: bool) {
        let Some(siege) = self.castle_siege_mut(id) else {
            return;
        };
        let mut flags = Vec::new();
        if all {
            for sc in siege.attacker_clans_mut().iter_mut() {
                flags.append(&mut sc.flags);
            }
        }
        for sc in siege.defender_clans_mut().iter_mut() {
            flags.append(&mut sc.flags);
        }
        for flag in flags {
            self.delete_npc(flag);
        }
    }

    /// Java `Siege.killedFlag(flag)`: the first attacker clan's `removeFlag(flag)`.
    // NOTE(java-quirk): `SiegeClan.removeFlag` deletes the flag even when it is not the clan's,
    // so the loop always ends at the first attacker clan.
    pub fn siege_killed_flag(&mut self, id: CastleId, flag: ObjectId) {
        let Some(siege) = self.castle_siege_mut(id) else {
            return;
        };
        if let Some(sc) = siege.attacker_clans_mut().first_mut() {
            sc.flags.retain(|f| *f != flag);
            self.delete_npc(flag);
        }
    }

    /// Java `Siege.killedCT()`.
    pub(crate) fn siege_killed_ct(&mut self, id: CastleId) {
        if let Some(siege) = self.castle_siege_mut(id) {
            siege.control_tower_count = (siege.control_tower_count - 1).max(0);
        }
    }

    // ------------------------------------------------------------------ clans

    /// Java `Clan.increaseBloodAllianceCount()`: `BloodAllianceReward` more, written as a siege
    /// job (D-51).
    pub(crate) fn clan_increase_blood_alliance_count(&mut self, clan_id: i32) {
        let reward = self.sieges.settings.blood_alliance_reward;
        let Some(clan) = self.clan_mut(clan_id) else {
            warn!("Blood alliance reward for a clan that does not exist: {clan_id}");
            return;
        };
        clan.blood_alliance_count += reward;
        let count = clan.blood_alliance_count;
        self.siege_job_write(ResidenceWrite::ClanBloodAllianceCount { clan_id, count });
    }

    /// Java `Clan.getSiegeKills()`.
    pub fn clan_siege_kills(&self, clan_id: i32) -> i32 {
        self.sieges.clan_kills.get(&clan_id).map_or(0, |k| k.0)
    }

    /// Java `Clan.getSiegeDeaths()`.
    pub fn clan_siege_deaths(&self, clan_id: i32) -> i32 {
        self.sieges.clan_kills.get(&clan_id).map_or(0, |k| k.1)
    }

    /// Java `Clan.addSiegeKill()` and the killed clan's `addSiegeDeath()`.
    pub(crate) fn clan_add_siege_kill(&mut self, killer_clan: i32, killed_clan: i32) {
        self.sieges.clan_kills.entry(killer_clan).or_default().0 += 1;
        self.sieges.clan_kills.entry(killed_clan).or_default().1 += 1;
    }

    /// Java `Clan.clearSiegeKills()` and `clearSiegeDeaths()`.
    pub(crate) fn clan_clear_siege_kills(&mut self, clan_id: i32) {
        self.sieges.clan_kills.remove(&clan_id);
    }

    /// The attacker clan entry of a residence's siege (Java `Siegable.getAttackerClan(clan)`).
    pub(crate) fn siege_attacker_clan_mut(
        &mut self,
        residence: ResidenceRef,
        clan_id: i32,
    ) -> Option<&mut SiegeClan> {
        match residence {
            ResidenceRef::Castle(id) => self
                .castle_siege_mut(id)?
                .attacker_clans_mut()
                .iter_mut()
                .find(|sc| sc.clan_id == clan_id),
            // The fortress siege clans are `fort_attacker_clan`'s (M7-FORT-01).
            ResidenceRef::Fort(_) => None,
            ResidenceRef::ClanHall(hall) => self.hall_siege_attacker_clan_mut(hall, clan_id),
        }
    }
}
