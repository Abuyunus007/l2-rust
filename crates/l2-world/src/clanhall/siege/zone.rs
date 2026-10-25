//! The zones of a conquerable hall: its siege zone (active during the battle), its
//! `SiegableHallZone` (the inner spawn points, banishing) and the `ResidenceHallTeleportZone`s of
//! the flag war halls.
// Java: entity/zone/type/SiegeZone.java (setActive, setSiegeInstance, getSiegeObjectId)
// Java: entity/zone/type/SiegableHallZone.java (getChallengerSpawns, banishNonSiegeParticipants)
// Java: entity/zone/type/ResidenceHallTeleportZone.java (checkTeleportTask)
// Java: mechanics/siege/clanhalls/SiegableHall.java (updateSiegeZone)
// Java: entity/actor/instance/Door.java (isEnemy)

use l2_data::Location;

use super::HallSiegeTimer;
use crate::residence::{ClanHallId, ResidenceRef};
use crate::siege::SiegeZoneSiege;
use crate::teleport::TeleportTarget;
use crate::world::World;

/// The zone type of the hall teleport zones.
const TELEPORT_ZONE: &str = "ResidenceHallTeleportZone";
/// `ResidenceHallTeleportZone`'s `ThreadPool.schedule(new TeleportTask(), 30000)`.
const TELEPORT_DELAY_MS: i64 = 30_000;

impl World {
    /// The `SiegeZone` of the hall (Java `SiegableHall.getSiegeZone()`).
    pub(crate) fn hall_siege_zone(&self, id: ClanHallId) -> Option<i32> {
        self.hall_siege(id)?.siege_zone
    }

    /// `SiegableHall.setSiege(engine)`: the zone knows the siege (and its fame).
    pub(crate) fn hall_link_siege_zone(&mut self, id: ClanHallId) {
        let Some(zone) = self.hall_siege_zone(id) else {
            return;
        };
        let settings = &self.clanhall.siege.settings;
        // `giveFame() && getFameFrequency() > 0`.
        let fame = (settings.enable_fame && settings.fame_frequency > 0)
            .then_some((settings.fame_frequency, settings.fame_amount));
        self.set_siege_zone_siege(
            zone,
            Some(SiegeZoneSiege {
                siegable: ResidenceRef::ClanHall(id),
                fame,
            }),
        );
    }

    /// Java `SiegableHall.updateSiegeZone(active)`: `SiegeZone.setActive` only, the players inside
    /// are not touched until they cross the border (the castle sieges also call
    /// `updateZoneStatusForCharactersInside`). The doors with an HP bar become enemies.
    pub(crate) fn hall_update_siege_zone(&mut self, id: ClanHallId, active: bool) {
        if let Some(zone) = self.hall_siege_zone(id) {
            self.sieges.zones.entry(zone).or_default().active = active;
        }
        for door in self.residence_doors(ResidenceRef::ClanHall(id)) {
            if let Some(d) = self.doors.get_mut(&door) {
                d.siege_enemy = active && d.template.show_hp;
            }
        }
    }

    /// The hall's `SiegableHallZone`.
    fn hall_zone_data(&self, id: ClanHallId) -> Option<l2_data::Zone> {
        self.clan_hall_zone(id).cloned()
    }

    /// `getZone().getSpawns()`: the normal restart points of the hall's zone.
    pub(crate) fn hall_zone_spawns(&self, id: ClanHallId) -> Vec<Location> {
        self.hall_zone_data(id)
            .map(|z| z.spawns.spawns)
            .unwrap_or_default()
    }

    /// `getZone().getChallengerSpawns()`.
    pub(crate) fn hall_zone_challenger_spawns(&self, id: ClanHallId) -> Vec<Location> {
        self.hall_zone_data(id)
            .map(|z| z.spawns.challenger)
            .unwrap_or_default()
    }

    /// `RandomRespawnInTownEnabled`: a random point of the list, or the first.
    fn hall_pick(&mut self, len: usize) -> usize {
        if self.settings.teleport.random_respawn_in_town_enabled {
            usize::try_from(self.rng().below(i32::try_from(len).unwrap_or(i32::MAX))).unwrap_or(0)
        } else {
            0
        }
    }

    /// Java `SiegableHallZone.banishNonSiegeParticipants()`.
    // NOTE(java-quirk): despite its name it banishes the players with `isInHideoutSiege()` (the
    // participants); the others stay.
    pub(crate) fn hall_banish_non_siege_participants(&mut self, id: ClanHallId) {
        let Some((zone_id, zone_type)) = self.clan_hall(id).and_then(|h| h.zone.clone()) else {
            return;
        };
        let Some(zone) = self.hall_zone_data(id) else {
            return;
        };
        let offset = self.settings.teleport.max_offset_on_teleport;
        for player in self.players_inside_zone(zone_id, &zone_type) {
            if !self.is_in_hideout_siege(player) {
                continue;
            }
            let loc = {
                let mut pick = |len: usize| self.hall_pick(len);
                zone.banish_spawn_loc(&mut pick)
            };
            if let Some(loc) = loc {
                self.tele_to_location(
                    player,
                    TeleportTarget::at(loc.x, loc.y, loc.z).with_random_offset(offset),
                );
            }
        }
    }

    /// The teleport zone `residenceZoneId` of the hall (`BanditStronghold.TELE_ZONES`).
    fn hall_teleport_zone(&self, id: ClanHallId, index: i32) -> Option<i32> {
        let zones = self.zoning.zones.as_ref()?;
        zones
            .all_of_type(TELEPORT_ZONE)
            .into_iter()
            .find(|z| {
                z.int_param("residenceId") == Some(id.0) && z.int_param("residenceZoneId") == Some(index)
            })
            .map(|z| z.id)
    }

    /// Java `TELE_ZONES[index].checkTeleportTask()`: the players in the zone are moved in 30
    /// seconds, unless that is pending already. `false` where Java's `TELE_ZONES[index]` is null
    /// (a NullPointerException).
    pub(crate) fn hall_check_teleport_task(&mut self, id: ClanHallId, index: i32) -> bool {
        let Some(zone) = self.hall_teleport_zone(id, index) else {
            return false;
        };
        if self.clanhall.siege.tele_tasks.contains_key(&zone) {
            return true;
        }
        let task = self.hall_schedule(HallSiegeTimer::TeleportZone(zone), TELEPORT_DELAY_MS);
        self.clanhall.siege.tele_tasks.insert(zone, task);
        true
    }

    /// Java `ResidenceHallTeleportZone.TeleportTask`: everybody inside goes to one spawn point.
    pub(crate) fn hall_teleport_zone_due(&mut self, zone_id: i32) {
        self.clanhall.siege.tele_tasks.remove(&zone_id);
        let Some(zone) = self
            .zoning
            .zones
            .as_ref()
            .and_then(|z| z.get_of_type(zone_id, TELEPORT_ZONE))
            .cloned()
        else {
            return;
        };
        let spawns = &zone.spawns.spawns;
        let index = if spawns.len() > 1 {
            usize::try_from(self.rng().below(i32::try_from(spawns.len()).unwrap_or(i32::MAX))).unwrap_or(0)
        } else {
            0
        };
        // `getSpawns().get(index)` is null for a zone without points: a NullPointerException.
        let Some(loc) = spawns.get(index) else {
            tracing::warn!("ResidenceHallTeleportZone {zone_id} has no spawn point.");
            return;
        };
        for player in self.players_inside_zone(zone_id, TELEPORT_ZONE) {
            self.tele_to_location(player, TeleportTarget::at(loc.x, loc.y, loc.z));
        }
    }
}
