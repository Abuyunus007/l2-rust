//! The siege zones: active while a siege runs, they set the PvP and siege flags of the players
//! inside, put the registered ones in the siege (with the zone's fame), keep wyverns and mounts
//! out, and punish deaths of the participants with the Battleground Death Syndrome.
// Java: entity/zone/type/SiegeZone.java (Settings, onEnter, onExit, onDieInside,
//       onPlayerLogoutInside, updateZoneStatusForCharactersInside, setActive, setSiegeInstance)
// Java: mechanics/siege/Siegable.java (giveFame, getFameFrequency, getFameAmount)
//
// The zone's settings (`SiegeZone.Settings`: the siege object id, the siege, the active flag) live
// in `SiegeWorld::zones` by zone id. The fortress and conquerable hall cards activate their zones
// with [`World::set_siege_zone_siege`] and [`World::set_siege_zone_active`].

use l2_data::Zone;
use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::SystemMessage;
use l2_util::ObjectId;

use crate::effects::SkillKey;
use crate::mount::MountType;
use crate::residence::{CastleId, ResidenceRef};
use crate::world::World;
use crate::zones::ZoneId;

/// `SiegeZone.DISMOUNT_DELAY` (seconds).
const DISMOUNT_DELAY: i32 = 5;
/// Battleground Death Syndrome (`onDieInside`).
const DEATH_SYNDROME: i32 = 5660;
/// The combat flag of the fortress sieges (`SiegeZone.onExit`).
const COMBAT_FLAG: i32 = 9819;

/// The siege of an active siege zone (Java `Settings.getSiege()`, a `Siegable`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SiegeZoneSiege {
    pub siegable: ResidenceRef,
    /// `giveFame() && getFameFrequency() > 0`: the frequency (seconds) and the amount.
    pub fame: Option<(i32, i32)>,
}

/// Java `SiegeZone.Settings`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct SiegeZoneState {
    /// `_siege`.
    pub(crate) siege: Option<SiegeZoneSiege>,
    /// `_isActiveSiege`.
    pub(crate) active: bool,
}

impl World {
    /// Java `SiegeZone.isActive()`.
    pub fn is_siege_zone_active(&self, zone_id: i32) -> bool {
        self.sieges.zones.get(&zone_id).is_some_and(|z| z.active)
    }

    /// Java `SiegeZone.setSiegeInstance(siege)`.
    pub fn set_siege_zone_siege(&mut self, zone_id: i32, siege: Option<SiegeZoneSiege>) {
        self.sieges.zones.entry(zone_id).or_default().siege = siege;
    }

    /// Java `SiegeZone.setActive(value)` followed by `updateZoneStatusForCharactersInside()`.
    pub fn set_siege_zone_active(&mut self, zone_id: i32, active: bool) {
        self.sieges.zones.entry(zone_id).or_default().active = active;
        self.siege_zone_update_status_for_characters_inside(zone_id);
    }

    /// The castle's zone at the start and the end of its siege (`setSiegeInstance`, `setActive`,
    /// `updateZoneStatusForCharactersInside`, in Java's order for each).
    pub(crate) fn castle_siege_zone_set_active(&mut self, id: CastleId, active: bool) {
        let Some(zone) = self.castle(id).and_then(|c| c.siege_zone) else {
            return;
        };
        let settings = &self.sieges.settings;
        let fame = (settings.castle_zone_fame_task_frequency > 0).then_some((
            settings.castle_zone_fame_task_frequency,
            settings.castle_zone_fame_aquire_points,
        ));
        let siege = SiegeZoneSiege {
            siegable: ResidenceRef::Castle(id),
            fame,
        };
        if active {
            self.set_siege_zone_siege(zone, Some(siege));
            self.set_siege_zone_active(zone, true);
        } else {
            self.set_siege_zone_active(zone, false);
            self.set_siege_zone_siege(zone, None);
        }
        self.castle_doors_set_enemy(id, active);
    }

    /// The players a siege zone holds (its `_characterList`), by object id.
    pub fn siege_zone_players(&self, zone_id: i32) -> Vec<ObjectId> {
        let mut inside: Vec<ObjectId> = self
            .players
            .iter()
            .filter(|(_, p)| {
                p.zones
                    .members()
                    .iter()
                    .any(|(id, ty)| *id == zone_id && ty == "SiegeZone")
            })
            .map(|(id, _)| *id)
            .collect();
        inside.sort_unstable();
        inside
    }

    /// Java `SiegeZone.updateZoneStatusForCharactersInside()`.
    pub fn siege_zone_update_status_for_characters_inside(&mut self, zone_id: i32) {
        let Some(zone) = self
            .zoning
            .zones
            .as_ref()
            .and_then(|z| z.get_of_type(zone_id, "SiegeZone"))
            .cloned()
        else {
            return;
        };
        let players = self.siege_zone_players(zone_id);
        if self.is_siege_zone_active(zone_id) {
            for player in players {
                self.on_siege_zone_enter(player, &zone);
            }
        } else {
            for player in players {
                self.set_zone_flag(player, ZoneId::Pvp, false);
                self.set_zone_flag(player, ZoneId::Siege, false);
                self.set_zone_flag(player, ZoneId::NoSummonFriend, false);
                self.send(
                    player,
                    &SystemMessage::new(SystemMessageId::YouHaveLeftACombatZone),
                );
                self.stop_fame_task(player);
                if self.mount_type_of(player) == MountType::Wyvern {
                    self.exited_no_landing(player);
                }
            }
        }
    }

    /// Java `SiegeZone.onEnter(creature)` for a player.
    pub(crate) fn on_siege_zone_enter(&mut self, id: ObjectId, zone: &Zone) {
        let state = self.sieges.zones.get(&zone.id).copied().unwrap_or_default();
        if !state.active {
            return;
        }
        self.set_zone_flag(id, ZoneId::Pvp, true);
        self.set_zone_flag(id, ZoneId::Siege, true);
        // FIXME: Custom ? (Java's own comment)
        self.set_zone_flag(id, ZoneId::NoSummonFriend, true);
        let siegable_id = l2_data::residence::siege_object_id(zone);
        if self.is_registered_on_this_siege_field(id, siegable_id) {
            // In siege.
            self.set_in_siege(id, true);
            // NOTE(java-quirk): Java reads `getSiege()` without a null check; a zone that is active
            // without a siege throws there. Here such a zone gives no fame.
            if let Some((freq, amount)) = state.siege.and_then(|s| s.fame) {
                self.start_fame_task(id, i64::from(freq) * 1000, amount);
            }
        }
        self.send(
            id,
            &SystemMessage::new(SystemMessageId::YouHaveEnteredACombatZone),
        );
        if self.sieges.settings.allow_mounts_during_siege {
            return;
        }
        if self.is_gm(id) {
            self.send_message(
                id,
                "You have entered a siege zone. GM dismount restrictions are ignored.",
            );
            return;
        }
        // Check if wyvern riding is disallowed during siege and player is currently mounted on a
        // wyvern.
        let is_castle_lord = self
            .castle(CastleId(siegable_id))
            .is_some_and(|c| self.is_clan_leader(id) && self.clan_id_of(id) == c.owner_id);
        let mount = self.mount_type_of(id);
        if mount == MountType::Wyvern {
            // Only allow castle lord to ride wyvern inside siege zone.
            if !is_castle_lord {
                self.send(
                    id,
                    &SystemMessage::new(
                        SystemMessageId::ThisAreaCannotBeEnteredWhileMountedAtopOfAWyvernYouWillBeDismountedFromYourWyvernIfYouDoNotLeave,
                    ),
                );
                self.entered_no_landing(id, DISMOUNT_DELAY);
            }
        } else if self.is_mounted(id) {
            // Allow castle lord to be mounted on a strider during siege, disallow others.
            let is_castle_lord_on_strider = mount == MountType::Strider && is_castle_lord;
            if !is_castle_lord_on_strider {
                self.dismount(id);
            }
        }
        let riding = self
            .stats(id)
            .and_then(|v| v.transformation().map(|t| t.is_riding()))
            .unwrap_or(false);
        if riding {
            self.untransform(id);
        }
    }

    /// Java `SiegeZone.onExit(creature)` for a player.
    pub(crate) fn on_siege_zone_exit(&mut self, id: ObjectId, zone: &Zone) {
        self.set_zone_flag(id, ZoneId::Pvp, false);
        self.set_zone_flag(id, ZoneId::Siege, false);
        self.set_zone_flag(id, ZoneId::NoSummonFriend, false);
        let state = self.sieges.zones.get(&zone.id).copied().unwrap_or_default();
        if state.active {
            self.send(id, &SystemMessage::new(SystemMessageId::YouHaveLeftACombatZone));
            if self.mount_type_of(id) == MountType::Wyvern {
                self.exited_no_landing(id);
            }
            // Set pvp flag.
            if self.pvp_flag_of(id) == 0 {
                self.start_pvp_flag(id);
            }
        }
        self.stop_fame_task(id);
        self.set_in_siege(id, false);
        let fort_siege = matches!(state.siege.map(|s| s.siegable), Some(ResidenceRef::Fort(_)));
        if !fort_siege || self.inventory_item_by_item_id(id, COMBAT_FLAG).is_none() {
            return;
        }
        // M7-FORT-01: drop the combat flag (`FortSiegeManager.dropCombatFlag`) and unsummon the
        // summon.
    }

    /// Java `SiegeZone.onDieInside(creature)`: a participant who dies in an active siege zone gets
    /// the Battleground Death Syndrome, one level more each time (at most 5).
    pub(crate) fn on_siege_zone_die_inside(&mut self, id: ObjectId) {
        let zones: Vec<i32> = self
            .players
            .get(&id)
            .map(|p| {
                p.zones
                    .members()
                    .iter()
                    .filter(|(_, ty)| ty == "SiegeZone")
                    .map(|(z, _)| *z)
                    .collect()
            })
            .unwrap_or_default();
        for zone_id in zones {
            if !self.is_siege_zone_active(zone_id) {
                continue;
            }
            let Some(siegable_id) = self
                .zoning
                .zones
                .as_ref()
                .and_then(|z| z.get_of_type(zone_id, "SiegeZone"))
                .map(l2_data::residence::siege_object_id)
            else {
                continue;
            };
            if !self.is_registered_on_this_siege_field(id, siegable_id) {
                continue;
            }
            let mut level = 1;
            if let Some(info) = self
                .effect_list(id)
                .and_then(|l| l.info_by_skill_id(DEATH_SYNDROME))
            {
                level = (level + info.skill.level).min(5);
            }
            self.apply_effects(id, id, SkillKey::new(DEATH_SYNDROME, level));
        }
    }

    // NOTE: `SiegeZone.onPlayerLogoutInside` teleports a leaving player who is not of the zone's
    // clan id to the town, after `storeMe()` has written the position: nothing of it lasts (the
    // next `EnterWorld` teleports a player in a siege zone itself), so it is left out.
}
