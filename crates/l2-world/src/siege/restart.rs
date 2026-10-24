//! Where a dead player may come back during and outside a siege: the clan buttons of `Die`, the
//! restart points of the residences and the headquarters (`MapRegionData.getTeleToLocation`), and
//! the respawn delay of a siege's attackers.
// Java: data/xml/MapRegionData.java (getTeleToLocation: the clan hall, castle, fortress and siege
//       flag branches, the Seven Signs branch)
// Java: network/serverpackets/Die.java (the clan buttons)
// Java: network/clientpackets/RequestRestartPoint.java (the attacker's DeathTask, portPlayer's
//       cases 1 to 4)
// Java: entity/zone/ZoneRespawn.java (getSpawnLoc, getChaoticSpawnLoc, getOtherSpawnLoc)

use l2_util::ObjectId;

use super::SiegeTimer;
use crate::residence::{CASTLE_FUNC_RESTORE_EXP, CastleId, FortId, ResidenceRef};
use crate::teleport::TeleportWhereType;
use crate::world::World;

/// `Fort.FUNC_RESTORE_EXP`.
const FORT_FUNC_RESTORE_EXP: i32 = 4;
/// `SevenSigns.CABAL_DAWN`.
const CABAL_DAWN: i32 = 2;

/// Which list of a residence zone's restart points (`ZoneRespawn`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ZoneSpawn {
    /// `getSpawnLoc()`.
    Normal,
    /// `getChaoticSpawnLoc()` (the normal ones when the zone has none).
    Chaotic,
    /// `getOtherSpawnLoc()` (the normal ones when the zone has none).
    Other,
}

impl World {
    /// A restart point of a residence zone (`RandomRespawnInTownEnabled` picks one at random).
    fn residence_zone_spawn(
        &mut self,
        zone_id: i32,
        zone_type: &str,
        kind: ZoneSpawn,
    ) -> Option<(i32, i32, i32)> {
        let zone = self
            .zoning
            .zones
            .as_ref()?
            .get_of_type(zone_id, zone_type)?
            .clone();
        let random = self.settings.teleport.random_respawn_in_town_enabled;
        let mut pick = |len: usize| {
            if random {
                usize::try_from(self.rng().below(i32::try_from(len).unwrap_or(i32::MAX))).unwrap_or(0)
            } else {
                0
            }
        };
        let loc = match kind {
            ZoneSpawn::Normal => zone.spawn_loc(&mut pick),
            ZoneSpawn::Chaotic => zone.chaotic_spawn_loc(&mut pick),
            ZoneSpawn::Other => zone.other_spawn_loc(&mut pick),
        }?;
        Some((loc.x, loc.y, loc.z))
    }

    fn residence_zone_spawn_for(&mut self, player: ObjectId, zone: (i32, &str)) -> Option<(i32, i32, i32)> {
        let karma = self.players.get(&player).map_or(0, |p| p.data.karma);
        let kind = if karma > 0 {
            ZoneSpawn::Chaotic
        } else {
            ZoneSpawn::Normal
        };
        self.residence_zone_spawn(zone.0, zone.1, kind)
    }

    /// The clan branches of Java `MapRegionData.getTeleToLocation(player, where)`: the clan's
    /// hall, castle (or the castle it defends in a siege), fortress, or the clan's headquarters
    /// in the siege the player stands in. `None` goes on with the town branches.
    pub(crate) fn residence_tele_to_location(
        &mut self,
        id: ObjectId,
        place: TeleportWhereType,
    ) -> Option<(i32, i32, i32)> {
        let clan_id = self.clan_of(id).map(|c| c.id)?;
        // Flying players in Gracia cannot use teleports to Aden continent.
        let flying = self.stats(id).is_some_and(|v| v.is_flying_mounted()) || self.is_flying(id);
        if flying {
            return None;
        }
        let (x, y, z) = self.position_of(id)?;
        match place {
            // Clan hall teleportation (`getAbstractHallByOwner`: an auctionable hall first).
            TeleportWhereType::ClanHall => {
                let hall = self.clan_hall_by_owner(clan_id)?;
                let (zone_id, zone_type) = self.clan_hall(hall)?.zone.clone()?;
                self.residence_zone_spawn_for(id, (zone_id, &zone_type))
            }
            // Castle teleportation: the clan's castle, or the castle it defends in a siege.
            TeleportWhereType::Castle => {
                let castle = self.castle_by_owner(clan_id).or_else(|| {
                    self.castle_at(x, y, z).filter(|c| {
                        self.castle_siege(*c)
                            .is_some_and(|s| s.in_progress && s.defender_clan(clan_id).is_some())
                    })
                })?;
                let zone = self.castle(castle).filter(|c| c.id.0 > 0)?.zone?;
                self.residence_zone_spawn_for(id, (zone, "CastleZone"))
            }
            // Fortress teleportation: the clan's fortress, or the fortress it owns in a siege.
            TeleportWhereType::Fortress => {
                // NOTE(java-quirk): the second test (the fortress here, in a siege, owned by the
                // clan) only finds what `getFortByOwner` found already.
                let fort = self.fort_by_owner(clan_id).or_else(|| {
                    self.fort_at(x, y, z).filter(|f| {
                        self.fort_siege_in_progress(*f)
                            && self.fort(*f).is_some_and(|fort| fort.owner_id == clan_id)
                    })
                })?;
                let zone = self.fort(fort).filter(|f| f.id.0 > 0)?.zone?;
                self.residence_zone_spawn_for(id, (zone, "FortZone"))
            }
            // Siege flag teleportation: a headquarters of the clan in the siege here.
            TeleportWhereType::SiegeFlag => {
                if let Some(castle) = self.castle_at(x, y, z) {
                    let siege = self.castle_siege(castle).filter(|s| s.in_progress)?;
                    let flag = siege.flags_of(clan_id).first().copied()?;
                    return self.position_of(flag);
                }
                // The headquarters of a fortress siege (`fort.getSiege().getFlag(clan)`).
                let fort = self
                    .fort_at(x, y, z)
                    .filter(|f| self.fort_siege_in_progress(*f))?;
                let flag = self.fort_siege_flags(fort, clan_id).first().copied()?;
                self.position_of(flag)
            }
            TeleportWhereType::Town => None,
        }
    }

    /// `getTeleToLocation`'s "Respawn far from castle during siege if clan is participating": with
    /// the Seal of Strife owned by Dawn, a participant of the siege here restarts at the castle
    /// zone's other points.
    pub(crate) fn siege_strife_restart_point(&mut self, id: ObjectId) -> Option<(i32, i32, i32)> {
        let (x, y, z) = self.position_of(id)?;
        let castle = self.castle_at(x, y, z)?;
        let clan = self.clan_of(id).map(|c| c.id);
        let participating = self
            .castle_siege(castle)
            .is_some_and(|s| s.in_progress && (s.is_defender(clan) || s.is_attacker(clan)));
        if !participating || self.seven_signs_strife_owner() != CABAL_DAWN {
            return None;
        }
        let zone = self.castle(castle)?.zone?;
        self.residence_zone_spawn(zone, "CastleZone", ZoneSpawn::Other)
    }

    /// The four clan buttons of `Die` (to the clan hall, the castle, the siege headquarters, the
    /// fortress), for a player who may teleport, has a clan and is not jailed.
    pub(crate) fn die_clan_buttons(&self, id: ObjectId) -> (bool, bool, bool, bool) {
        let Some(clan) = self.clan_of(id) else {
            return (false, false, false, false);
        };
        if self.is_jailed(id) {
            return (false, false, false, false);
        }
        let Some((x, y, z)) = self.position_of(id) else {
            return (false, false, false, false);
        };
        let mut in_castle_defense = false;
        let mut in_fort_defense = false;
        let mut has_flags = false;
        let fort = self.fort_at(x, y, z).filter(|f| self.fort_siege_in_progress(*f));
        if let Some(siege) = self
            .castle_at(x, y, z)
            .and_then(|c| self.castle_siege(c))
            .filter(|s| s.in_progress)
        {
            // Siege in progress.
            match siege.attacker_clan(clan.id) {
                Some(sc) => has_flags = !sc.flags.is_empty(),
                None => in_castle_defense = siege.is_defender(Some(clan.id)),
            }
        } else if let Some(fort) = fort {
            // Siege in progress (a fortress, M7-FORT-01).
            match self.fort_attacker_clan(fort, clan.id) {
                Some(sc) => has_flags = !sc.flags.is_empty(),
                None => in_fort_defense = self.fort_check_is_defender(fort, clan.id),
            }
        }
        // M7-TW-01: `TerritoryWarManager.getHQForClan(clan)`.
        // `getNearbyClanHall(creature).getSiege().checkIsAttacker(clan)` (M7-CHSIEGE-01).
        let hall_attacker = self
            .nearby_siegable_hall(x, y, 10_000)
            .is_some_and(|hall| self.hall_siege_is_attacker(hall, clan.id));
        (
            clan.hideout_id > 0,
            clan.castle_id > 0 || in_castle_defense,
            (has_flags && !in_castle_defense && !in_fort_defense) || hall_attacker,
            clan.fort_id > 0 || in_fort_defense,
        )
    }

    /// `RequestRestartPoint`: an attacker of the siege the player lies in waits
    /// `AttackerRespawn` (`DeathTask`). `true` when the restart is delayed.
    pub(crate) fn siege_delay_attacker_restart(&mut self, id: ObjectId, point_type: i32) -> bool {
        let Some((x, y, z)) = self.position_of(id) else {
            return false;
        };
        let Some(castle) = self.castle_at(x, y, z) else {
            return false;
        };
        let clan = self.clan_of(id).map(|c| c.id);
        let attacker = self
            .castle_siege(castle)
            .is_some_and(|s| s.in_progress && clan.is_some() && s.is_attacker(clan));
        if !attacker {
            return false;
        }
        // Schedule respawn delay for attacker.
        let delay = self.sieges.settings.attacker_respawn_delay;
        self.schedule_siege_timer(
            SiegeTimer::RestartPoint {
                player: id,
                point_type,
            },
            i64::from(delay),
        );
        if delay > 0 {
            self.send_message(id, &format!("You will be re-spawned in {} seconds", delay / 1000));
        }
        true
    }

    /// `portPlayer`'s case 1 (to the clan hall): the restart point and the exp the hall's
    /// function gives back. `None` refuses the restart.
    pub(crate) fn restart_to_clan_hall(&mut self, id: ObjectId) -> Option<Option<(i32, i32, i32)>> {
        let clan = self.clan_of(id).map(|c| (c.id, c.hideout_id));
        let (clan_id, hideout) = clan?;
        if hideout == 0 {
            return None;
        }
        let loc = self.get_tele_to_location(id, TeleportWhereType::ClanHall);
        // `ClanHallTable.getClanHallByOwner(clan).getFunction(FUNC_RESTORE_EXP)` (M7-CH-01).
        if let Some(level) = self.clan_hall_restore_exp_level(clan_id) {
            self.restore_exp(id, f64::from(level));
        }
        Some(loc)
    }

    /// `portPlayer`'s case 2 (to the castle). `None` refuses the restart.
    pub(crate) fn restart_to_castle(&mut self, id: ObjectId) -> Option<Option<(i32, i32, i32)>> {
        let (x, y, z) = self.position_of(id)?;
        let clan = self.clan_of(id).map(|c| (c.id, c.castle_id));
        let castle = self.castle_at(x, y, z);
        let siege = castle
            .and_then(|c| self.castle_siege(c))
            .filter(|s| s.in_progress)
            .cloned();
        let loc = if let Some(siege) = siege {
            // Siege in progress.
            let clan_id = clan.map(|c| c.0);
            if siege.is_defender(clan_id) {
                self.get_tele_to_location(id, TeleportWhereType::Castle)
            } else if siege.is_attacker(clan_id) {
                self.get_tele_to_location(id, TeleportWhereType::Town)
            } else {
                let name = self
                    .players
                    .get(&id)
                    .map(|p| p.data.name.clone())
                    .unwrap_or_default();
                tracing::warn!(
                    "Player [{name}] called RestartPointPacket - To Castle and he doesn't have Castle!"
                );
                return None;
            }
        } else {
            if clan.is_none_or(|(_, castle_id)| castle_id == 0) {
                return None;
            }
            self.get_tele_to_location(id, TeleportWhereType::Castle)
        };
        let level = clan
            .and_then(|(clan_id, _)| self.castle_by_owner(clan_id))
            .and_then(|c: CastleId| self.residence_function(ResidenceRef::Castle(c), CASTLE_FUNC_RESTORE_EXP))
            .map(|f| f.lvl);
        if let Some(level) = level {
            self.restore_exp(id, f64::from(level));
        }
        Some(loc)
    }

    /// `portPlayer`'s case 3 (to the fortress). `None` refuses the restart.
    pub(crate) fn restart_to_fortress(&mut self, id: ObjectId) -> Option<Option<(i32, i32, i32)>> {
        let clan = self.clan_of(id).map(|c| (c.id, c.fort_id));
        // NOTE(java-quirk): `isInDefense` is always false in Java.
        let (clan_id, fort_id) = clan.filter(|(_, fort)| *fort != 0)?;
        let _ = fort_id;
        let loc = self.get_tele_to_location(id, TeleportWhereType::Fortress);
        let level = self
            .fort_by_owner(clan_id)
            .and_then(|f: FortId| self.residence_function(ResidenceRef::Fort(f), FORT_FUNC_RESTORE_EXP))
            .map(|f| f.lvl);
        if let Some(level) = level {
            self.restore_exp(id, f64::from(level));
        }
        Some(loc)
    }

    /// `portPlayer`'s case 4 (to the siege headquarters): only with a headquarters of the clan in
    /// the siege here. `None` refuses the restart.
    pub(crate) fn restart_to_siege_hq(&mut self, id: ObjectId) -> Option<Option<(i32, i32, i32)>> {
        let (x, y, z) = self.position_of(id)?;
        let clan = self.clan_of(id).map(|c| c.id);
        // M7-TW-01: `TerritoryWarManager.getHQForClan(clan)` / `getFlagForClan(clan)`.
        let hall = self.nearby_siegable_hall(x, y, 10_000);
        let castle_siege = self
            .castle_at(x, y, z)
            .and_then(|c| self.castle_siege(c))
            .filter(|s| s.in_progress);
        let has_flag = if let Some(siege) = castle_siege {
            clan.and_then(|clan| siege.attacker_clan(clan))
                .is_some_and(|sc| !sc.flags.is_empty())
        } else if let Some(fort) = self.fort_at(x, y, z).filter(|f| self.fort_siege_in_progress(*f)) {
            clan.and_then(|clan| self.fort_attacker_clan(fort, clan))
                .is_some_and(|sc| !sc.flags.is_empty())
        } else if let Some(hall) = hall.filter(|h| self.hall_is_in_siege(*h)) {
            // `hall.isInSiege()`: the hall's attacker clan (M7-CHSIEGE-01).
            clan.and_then(|clan| self.hall_siege_attacker_clan(hall, clan))
                .is_some_and(|sc| !sc.flags.is_empty())
        } else {
            false
        };
        if !has_flag {
            // Check if the clan hall has inner spawns.
            if let Some(hall) = hall
                && let Some(loc) = self.hall_inner_spawn_loc(hall, id)
            {
                return Some(Some((loc.x, loc.y, loc.z)));
            }
            return None;
        }
        Some(self.get_tele_to_location(id, TeleportWhereType::SiegeFlag))
    }
}
