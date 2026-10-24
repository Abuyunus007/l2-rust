//! The siege zone of a fortress: whether it is active, what entering and leaving it does to a
//! player, and the players inside.
// Java: entity/zone/type/SiegeZone.java (onEnter, onExit, updateZoneStatusForCharactersInside,
//       banishForeigners, isActive, setActive)
// Java: entity/actor/Player.java (isRegisteredOnThisSiegeField)
//
// SiegeZone is one Java class for castles, fortresses and conquerable halls; its `fortId` parameter
// ties a zone to a fortress. This module handles the zones of fortresses only (M7-SIEGE-01 owns
// the castle ones and their `onEnter` / `onExit` hooks in `zones.rs` call this for a `fortId`
// zone). The activity of a zone is kept here, by zone id.

use l2_data::Zone;
use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::SystemMessage;
use l2_util::ObjectId;

use crate::mount::MountType;
use crate::residence::FortId;
use crate::teleport::TeleportWhereType;
use crate::world::World;
use crate::zones::ZoneId;

/// `SiegeZone.DISMOUNT_DELAY`.
const DISMOUNT_DELAY: i32 = 5;
/// The item id of the combat flag.
pub const COMBAT_FLAG_ITEM_ID: i32 = 9819;

/// The type name of the zone the fortress's siege zone is.
const SIEGE_ZONE: &str = "SiegeZone";

impl World {
    /// The id of the fortress's siege zone.
    fn fort_siege_zone_id(&self, fort: FortId) -> Option<i32> {
        self.fort(fort)?.siege_zone
    }

    /// Java `Fort.getZone().isActive()`.
    pub fn fort_zone_is_active(&self, fort: FortId) -> bool {
        self.fort_siege_zone_id(fort)
            .is_some_and(|zone| self.fort.active_zones.contains(&zone))
    }

    /// Java `Fort.getZone().setActive(value)`.
    pub(crate) fn fort_zone_set_active(&mut self, fort: FortId, active: bool) {
        let Some(zone) = self.fort_siege_zone_id(fort) else {
            return;
        };
        if active {
            self.fort.active_zones.insert(zone);
        } else {
            self.fort.active_zones.remove(&zone);
        }
    }

    /// Java `getPlayersInside()` of the fortress's siege zone.
    pub(crate) fn fort_players_in_zone(&self, fort: FortId) -> Vec<ObjectId> {
        self.fort_siege_zone_id(fort)
            .map(|zone| self.players_inside_zone(zone, SIEGE_ZONE))
            .unwrap_or_default()
    }

    /// Java `SiegeZone.updateZoneStatusForCharactersInside()`.
    pub(crate) fn fort_zone_update_status(&mut self, fort: FortId) {
        let active = self.fort_zone_is_active(fort);
        for player in self.fort_players_in_zone(fort) {
            if active {
                self.fort_zone_entered(player, fort);
            } else {
                self.fort_zone_left_quietly(player);
            }
        }
    }

    /// Java `SiegeZone.banishForeigners(owningClanId)`: everybody inside who is not of the clan
    /// goes to the nearest town.
    pub(crate) fn fort_zone_banish_foreigners(&mut self, fort: FortId, owning_clan_id: i32) {
        for player in self.fort_players_in_zone(fort) {
            if self.clan_id_of(player) == owning_clan_id {
                continue;
            }
            self.tele_to_location_where(player, TeleportWhereType::Town);
        }
    }

    /// Java `Player.isRegisteredOnThisSiegeField(value)`.
    pub fn is_registered_on_this_siege_field(&self, player: ObjectId, value: i32) -> bool {
        let side = self.fort.players.get(&player).map_or(0, |p| p.side);
        side == value || (81..=89).contains(&side)
    }

    // ------------------------------------------------------------------ onEnter / onExit

    /// The hook of `zones.rs`: a player entered a `SiegeZone`. `None` for a zone that is not a
    /// fortress's (castles and halls are other cards').
    pub(crate) fn fort_siege_zone_on_enter(&mut self, player: ObjectId, zone: &Zone) -> Option<()> {
        let fort = FortId(zone.int_param("fortId")?);
        self.fort(fort)?;
        if self.fort_zone_is_active(fort) {
            self.fort_zone_entered(player, fort);
        }
        Some(())
    }

    /// The hook of `zones.rs`: a player left a `SiegeZone` (see [`Self::fort_siege_zone_on_enter`]).
    pub(crate) fn fort_siege_zone_on_exit(&mut self, player: ObjectId, zone: &Zone) -> Option<()> {
        let fort = FortId(zone.int_param("fortId")?);
        self.fort(fort)?;
        self.set_inside(player, ZoneId::Pvp, false);
        self.set_inside(player, ZoneId::Siege, false);
        self.set_inside(player, ZoneId::NoSummonFriend, false);
        if self.fort_zone_is_active(fort) {
            self.send(
                player,
                &SystemMessage::new(SystemMessageId::YouHaveLeftACombatZone),
            );
            if self.mount_type_of(player) == MountType::Wyvern {
                self.exited_no_landing(player);
            }
            // Set pvp flag.
            if self.pvp_flag_of(player) == 0 {
                self.start_pvp_flag(player);
            }
        }
        self.fort_stop_fame_task(player);
        if let Some(info) = self.fort.players.get_mut(&player) {
            info.in_siege = false;
        }
        // "Drop combat flag." and the summon goes away.
        if self
            .inventory_item_by_item_id(player, COMBAT_FLAG_ITEM_ID)
            .is_some()
        {
            self.fort_drop_combat_flag(player, fort);
            if let Some(summon) = self.summon_of(player) {
                self.abort_attack(summon);
                self.abort_cast(summon);
                self.stop_all_effects(summon);
                self.unsummon(summon, Some(player));
            }
        }
        Some(())
    }

    /// `EnterWorld` for the fortress code: a clan in a siege that is going on gets its siege state,
    /// and a combat flag left in the inventory is removed.
    pub(crate) fn fort_player_entered(&mut self, player: ObjectId) {
        let clan = self.clan_id_of(player);
        if self.clan(clan).is_some() {
            let forts: Vec<FortId> = self.residences.forts.iter().map(|f| f.id).collect();
            for fort in forts {
                if !self.fort_siege_in_progress(fort) {
                    continue;
                }
                let state = if self.fort_check_is_attacker(fort, clan) {
                    1
                } else if self.fort_check_is_defender(fort, clan) {
                    2
                } else {
                    continue;
                };
                let info = self.fort.players.entry(player).or_default();
                info.state = state;
                info.side = fort.0;
            }
        }
        // Remove combat flag before teleporting.
        self.fort_remove_combat_flag(player);
    }

    /// The start of `Player.deleteMe` for the fortress code: `SiegeZone.onPlayerLogoutInside`,
    /// the combat flag, the siege state and the fame task.
    pub(crate) fn fort_player_leaving(&mut self, player: ObjectId) {
        // NOTE(java-quirk): `onPlayerLogoutInside` compares the player's clan id with the id of
        // the sieged fortress, which are never equal: whoever logs out inside the zone of a
        // fortress is brought to the nearest town.
        let inside: Vec<i32> = self
            .players
            .get(&player)
            .map(|p| {
                p.zones
                    .members()
                    .iter()
                    .filter(|(_, ty)| ty == SIEGE_ZONE)
                    .map(|(id, _)| *id)
                    .collect()
            })
            .unwrap_or_default();
        let in_fort_zone = inside
            .iter()
            .any(|zone| self.residences.forts.iter().any(|f| f.siege_zone == Some(*zone)));
        if in_fort_zone {
            self.tele_to_location_where(player, TeleportWhereType::Town);
        }
        // Remove combat flag.
        self.fort_remove_combat_flag(player);
        self.fort_player_left(player);
    }

    /// `SiegeZone.onEnter(creature)` of an active zone, for a player.
    fn fort_zone_entered(&mut self, player: ObjectId, fort: FortId) {
        self.set_inside(player, ZoneId::Pvp, true);
        self.set_inside(player, ZoneId::Siege, true);
        // FIXME in Java: custom?
        self.set_inside(player, ZoneId::NoSummonFriend, true);
        if self.is_registered_on_this_siege_field(player, fort.0) {
            if let Some(info) = self.fort.players.get_mut(&player) {
                info.in_siege = true;
            } else {
                self.fort.players.insert(
                    player,
                    super::PlayerFortSiege {
                        in_siege: true,
                        ..Default::default()
                    },
                );
            }
            // `giveFame()` is true for a fortress.
            let frequency = self.fort.settings.fortress_zone_fame_task_frequency;
            if frequency > 0 {
                let points = self.fort.settings.fortress_zone_fame_aquire_points;
                self.fort_start_fame_task(player, i64::from(frequency) * 1000, points);
            }
        }
        self.send(
            player,
            &SystemMessage::new(SystemMessageId::YouHaveEnteredACombatZone),
        );
        if crate::transform::ALLOW_MOUNTS_DURING_SIEGE {
            return;
        }
        if self.is_gm(player) {
            self.send_message(
                player,
                "You have entered a siege zone. GM dismount restrictions are ignored.",
            );
            return;
        }
        // A fortress's id is no castle id: nobody is "the castle lord" here.
        match self.mount_type_of(player) {
            MountType::Wyvern => {
                self.send(
                    player,
                    &SystemMessage::new(
                        SystemMessageId::ThisAreaCannotBeEnteredWhileMountedAtopOfAWyvernYouWillBeDismountedFromYourWyvernIfYouDoNotLeave,
                    ),
                );
                self.entered_no_landing(player, DISMOUNT_DELAY);
            }
            _ if self.is_mounted(player) => {
                self.dismount(player);
            }
            _ => {}
        }
        // `transform.isRiding()`.
        let riding = self
            .players
            .get(&player)
            .and_then(|p| p.transform.transformation)
            .is_some_and(|form| {
                self.transform_table()
                    .and_then(|t| t.get(form).map(|f| f.is_riding()))
                    .unwrap_or(false)
            });
        if riding {
            self.untransform(player);
        }
    }

    /// The `else` of `updateZoneStatusForCharactersInside()` for a player: the zone is not active.
    fn fort_zone_left_quietly(&mut self, player: ObjectId) {
        self.set_inside(player, ZoneId::Pvp, false);
        self.set_inside(player, ZoneId::Siege, false);
        self.set_inside(player, ZoneId::NoSummonFriend, false);
        self.send(
            player,
            &SystemMessage::new(SystemMessageId::YouHaveLeftACombatZone),
        );
        self.fort_stop_fame_task(player);
        if self.mount_type_of(player) == MountType::Wyvern {
            self.exited_no_landing(player);
        }
    }
}
