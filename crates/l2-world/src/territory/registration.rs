//! The registrations of the war: who is registered where (`getRegisteredTerritoryId`,
//! `checkIsRegistered`, `isAllyField`), `RequestJoinDominionWar` (after a re-read of the rows,
//! D-50), `RequestDominionInfo` and the registration window (`ExShowDominionRegistry`).
// Java: managers/TerritoryWarManager.java (getRegisteredTerritoryId, isAllyField,
//       checkIsRegistered, registerClan, registerMerc, removeClan, removeMerc, changeRegistration)
// Java: network/clientpackets/{RequestJoinDominionWar,RequestDominionInfo}.java
// Java: network/serverpackets/{ExShowDominionRegistry,ExReplyDominionInfo,ExShowOwnthingPos}.java

use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::SystemMessage;
use l2_protocol_game::server::territory::{
    DominionTerritory, ExReplyDominionInfo, ExShowDominionRegistry, ExShowOwnthingPos, NO_ALLY, NO_OWNER,
    TerritoryWards, WardPosition,
};
use l2_util::ObjectId;
use tracing::warn;

use super::{Registration, TERRITORY_ID_BASE, TerritoryInput, TerritoryThen, TerritoryWrite};
use crate::clan::model::ClanAccess;
use crate::residence::CastleId;
use crate::world::World;

impl World {
    // ------------------------------------------------------------------ who is registered

    /// Java `getRegisteredTerritoryId(player)`: while the channel is open, the territory of the
    /// player's castle owning clan, of his registered clan or of his mercenary registration; else
    /// 0.
    pub fn territory_registered_id(&self, player: ObjectId) -> i32 {
        if !self.players.contains_key(&player)
            || !self.territory.channel_open
            || self.tw_level_of(player) < self.territory.settings.player_min_level
        {
            return 0;
        }
        if let Some(clan) = self.clan_of(player) {
            if clan.castle_id > 0 {
                return clan.castle_id + TERRITORY_ID_BASE;
            }
            let id = clan.id;
            for (castle, list) in &self.territory.registered_clans {
                if list.iter().any(|r| r.id == id) {
                    return castle + TERRITORY_ID_BASE;
                }
            }
        }
        for (castle, list) in &self.territory.registered_mercenaries {
            if list.iter().any(|r| r.id == player.0) {
                return castle + TERRITORY_ID_BASE;
            }
        }
        0
    }

    /// Java `isAllyField(player, fieldId)`: the field is the player's territory's castle, or its
    /// fortress (`fieldId > 100`).
    pub fn territory_is_ally_field(&self, player: ObjectId, field_id: i32) -> bool {
        let side = self.siege_side_of(player);
        if side == 0 {
            return false;
        }
        if side - TERRITORY_ID_BASE == field_id {
            return true;
        }
        field_id > 100
            && self
                .territory
                .territories
                .get(&(side - TERRITORY_ID_BASE))
                .is_some_and(|t| t.fort_id == field_id)
    }

    /// Java `checkIsRegistered(castleId, clan)`: a castle owning clan is registered for its own
    /// territory (any with -1); other clans by their registration (-1: anywhere).
    pub(crate) fn territory_check_clan_registered(&self, castle_id: i32, clan_id: Option<i32>) -> bool {
        let Some(clan) = clan_id.and_then(|c| self.clan(c)) else {
            return false;
        };
        if clan.castle_id > 0 {
            return castle_id == -1 || clan.castle_id == castle_id;
        }
        let id = clan.id;
        if castle_id == -1 {
            return self
                .territory
                .registered_clans
                .values()
                .any(|list| list.iter().any(|r| r.id == id));
        }
        // NOTE(java-quirk): `_registeredClans.get(castleId).contains(clan)` throws for a castle
        // without a list; here that is "not registered".
        self.territory
            .registered_clans
            .get(&castle_id)
            .is_some_and(|list| list.iter().any(|r| r.id == id))
    }

    /// Java `checkIsRegistered(castleId, objId)` for a mercenary.
    pub(crate) fn territory_check_merc_registered(&self, castle_id: i32, object_id: i32) -> bool {
        if castle_id == -1 {
            return self
                .territory
                .registered_mercenaries
                .values()
                .any(|list| list.iter().any(|r| r.id == object_id));
        }
        self.territory
            .registered_mercenaries
            .get(&castle_id)
            .is_some_and(|list| list.iter().any(|r| r.id == object_id))
    }

    // ------------------------------------------------------------------ the lists

    /// Java `changeRegistration(castleId, objId, delete)`; returns whether the row went out.
    fn territory_change_registration(&mut self, castle_id: i32, registered_id: i32, delete: bool) -> bool {
        let write = if delete {
            TerritoryWrite::RegistrationDelete {
                castle_id,
                registered_id,
            }
        } else {
            TerritoryWrite::RegistrationInsert {
                castle_id,
                registered_id,
            }
        };
        self.territory_write(write)
    }

    /// Java `registerClan(castleId, clan)`.
    pub fn territory_register_clan(&mut self, castle_id: i32, clan_id: i32) {
        if self.clan(clan_id).is_none()
            || self
                .territory
                .registered_clans
                .get(&castle_id)
                .is_some_and(|l| l.iter().any(|r| r.id == clan_id))
        {
            return;
        }
        let written = self.territory_change_registration(castle_id, clan_id, false);
        self.territory
            .registered_clans
            .entry(castle_id)
            .or_default()
            .push(Registration {
                id: clan_id,
                unsaved: !written,
            });
    }

    /// Java `registerMerc(castleId, player)`.
    pub fn territory_register_merc(&mut self, castle_id: i32, player: ObjectId) {
        if !self.players.contains_key(&player)
            || self.tw_level_of(player) < self.territory.settings.player_min_level
            || self
                .territory
                .registered_mercenaries
                .get(&castle_id)
                .is_some_and(|l| l.iter().any(|r| r.id == player.0))
        {
            return;
        }
        let written = self.territory_change_registration(castle_id, player.0, false);
        self.territory
            .registered_mercenaries
            .entry(castle_id)
            .or_default()
            .push(Registration {
                id: player.0,
                unsaved: !written,
            });
    }

    /// Java `removeClan(castleId, clan)`.
    pub fn territory_remove_clan(&mut self, castle_id: i32, clan_id: i32) {
        let Some(list) = self.territory.registered_clans.get_mut(&castle_id) else {
            return;
        };
        let Some(index) = list.iter().position(|r| r.id == clan_id) else {
            return;
        };
        list.remove(index);
        self.territory_change_registration(castle_id, clan_id, true);
    }

    /// Java `removeMerc(castleId, player)`.
    pub fn territory_remove_merc(&mut self, castle_id: i32, player: ObjectId) {
        let Some(list) = self.territory.registered_mercenaries.get_mut(&castle_id) else {
            return;
        };
        let Some(index) = list.iter().position(|r| r.id == player.0) else {
            return;
        };
        list.remove(index);
        self.territory_change_registration(castle_id, player.0, true);
    }

    /// Java `addDisguisedPlayer(playerObjId)`.
    pub fn territory_add_disguised(&mut self, player: ObjectId) {
        self.territory.disguised.insert(player);
    }

    /// Java `Disguise.onItemUse`: the disguise scroll of the player's own territory (`regId ==
    /// itemId - 13596`), not for a member of a castle owning clan.
    // Java: dist/game/data/scripts/handlers/items/Disguise.java
    pub(crate) fn disguise_on_use(&mut self, id: ObjectId, item: &crate::player::ItemData) -> bool {
        if !self.is_player(id) {
            self.send(
                id,
                &SystemMessage::new(SystemMessageId::YourPetCannotCarryThisItem),
            );
            return false;
        }
        let reg_id = self.registered_territory_id(id);
        if reg_id > 0 && reg_id == item.item_id - 13596 {
            if self.clan_of(id).is_some_and(|c| c.castle_id > 0) {
                self.send(
                    id,
                    &SystemMessage::new(SystemMessageId::ATerritoryOwningClanMemberCannotUseADisguiseScroll),
                );
                return false;
            }
            self.territory_add_disguised(id);
            self.broadcast_user_info(id);
            self.destroy_item_by_object_id(id, item.object_id, 1, false);
            true
        } else if reg_id > 0 {
            self.send(
                id,
                &SystemMessage::new(
                    SystemMessageId::TheDisguiseScrollCannotBeUsedBecauseItIsMeantForUseInADifferentTerritory,
                ),
            );
            false
        } else {
            self.send(
                id,
                &SystemMessage::new(SystemMessageId::TheTerritoryWarExclusiveDisguiseAndTransformationCanBeUsed20MinutesBeforeTheStartOfTheTerritoryWarTo10MinutesAfterItsEnd),
            );
            false
        }
    }

    // ------------------------------------------------------------------ the client packets

    /// The war's client packets.
    pub(crate) fn on_territory_input(&mut self, player: ObjectId, input: TerritoryInput) {
        match input {
            TerritoryInput::JoinDominionWar {
                territory_id,
                is_clan,
                is_joining,
            } => {
                if !self.players.contains_key(&player) {
                    return;
                }
                // D-50: the registrations change on the Java game server too; they are read
                // again before the request acts on them.
                self.territory_request_reread(TerritoryThen::Join {
                    player,
                    territory_id,
                    is_clan,
                    is_joining,
                });
            }
            TerritoryInput::DominionInfo => {
                if !self.players.contains_key(&player) {
                    return;
                }
                let info = self.dominion_info_packet();
                self.send(player, &info);
                let positions = self.ownthing_pos_packet();
                self.send(player, &positions);
            }
        }
    }

    /// Java `RequestJoinDominionWar.runImpl` on the registrations as the database has them.
    pub(crate) fn territory_join_after_reread(
        &mut self,
        player: ObjectId,
        territory_id: i32,
        is_clan: i32,
        is_joining: i32,
    ) {
        if !self.players.contains_key(&player) {
            return;
        }
        if self.territory.registration_over {
            self.send(
                player,
                &SystemMessage::new(
                    SystemMessageId::ItIsNotATerritoryWarRegistrationPeriodSoARequestCannotBeMadeAtThisTime,
                ),
            );
            return;
        }
        let clan = self.clan_of(player).map(|c| c.id);
        let castle_id = territory_id - TERRITORY_ID_BASE;
        if let Some(clan) = clan {
            // `getTerritory(castleId).getOwnerClan()` throws for a territory that does not exist.
            let Some(territory) = self.territory.territories.get(&castle_id) else {
                warn!("RequestJoinDominionWar: NullPointerException: no territory {territory_id}");
                return;
            };
            if territory.owner_clan == Some(clan) {
                self.send(
                    player,
                    &SystemMessage::new(SystemMessageId::TheClanWhoOwnsTheTerritoryCannotParticipateInTheTerritoryWarAsMercenaries),
                );
                return;
            }
        }
        if is_clan == 0x01 {
            if !self.has_clan_access(player, ClanAccess::CastleSiege) {
                self.send(
                    player,
                    &SystemMessage::new(SystemMessageId::YouAreNotAuthorizedToDoThat),
                );
                return;
            }
            let Some(clan) = clan else {
                return;
            };
            if is_joining == 1 {
                let dissolving = self.clan(clan).map_or(0, |c| c.dissolving_expiry_time);
                if self.tw_now_ms() < dissolving {
                    self.send(
                        player,
                        &SystemMessage::new(SystemMessageId::YourClanMayNotRegisterToParticipateInASiegeWhileUnderAGracePeriodOfTheClanSDissolution),
                    );
                    return;
                } else if self.territory_check_clan_registered(-1, Some(clan)) {
                    self.send(
                        player,
                        &SystemMessage::new(
                            SystemMessageId::YouVeAlreadyRequestedATerritoryWarInAnotherTerritoryElsewhere,
                        ),
                    );
                    return;
                }
                self.territory_register_clan(castle_id, clan);
            } else {
                self.territory_remove_clan(castle_id, clan);
            }
        } else {
            if self.tw_level_of(player) < 40 || self.tw_class_level_of(player) < 2 {
                // TODO in Java: punish player.
                return;
            }
            if is_joining == 1 {
                if self.territory_check_merc_registered(-1, player.0)
                    || clan.is_some_and(|c| self.territory_check_clan_registered(-1, Some(c)))
                {
                    self.send(
                        player,
                        &SystemMessage::new(
                            SystemMessageId::YouVeAlreadyRequestedATerritoryWarInAnotherTerritoryElsewhere,
                        ),
                    );
                    return;
                }
                self.territory_register_merc(castle_id, player);
            } else {
                self.territory_remove_merc(castle_id, player);
            }
        }
        let packet = self.dominion_registry_packet(CastleId(castle_id), player);
        self.send(player, &packet);
    }

    // ------------------------------------------------------------------ the packets

    /// The territories with owners and their wards (`getAllTerritories()`), for the packets.
    fn owned_territory_wards(&self) -> Vec<TerritoryWards> {
        self.owned_territories()
            .iter()
            .map(|t| TerritoryWards {
                territory_id: t.territory_id,
                ward_ids: t.owned_ward_ids(),
            })
            .collect()
    }

    /// `new ExShowDominionRegistry(castleId, player)`: the registration window of a territory.
    pub fn dominion_registry_packet(&self, castle: CastleId, player: ObjectId) -> ExShowDominionRegistry {
        let now = i32::try_from(self.tw_now_ms() / 1000).unwrap_or(i32::MAX);
        let castle_id = castle.0;
        let mut clan_requests = 0;
        let mut clan_registered = false;
        if let Some(list) = self.territory.registered_clans.get(&castle_id) {
            clan_requests = i32::try_from(list.len()).unwrap_or(i32::MAX);
            if let Some(clan) = self.clan_of(player).map(|c| c.id) {
                clan_registered = list.iter().any(|r| r.id == clan);
            }
        }
        let mut merc_requests = 0;
        let mut merc_registered = false;
        if let Some(list) = self.territory.registered_mercenaries.get(&castle_id) {
            merc_requests = i32::try_from(list.len()).unwrap_or(i32::MAX);
            merc_registered = list.iter().any(|r| r.id == player.0);
        }
        let war_time = i32::try_from(self.territory.start_tw_date / 1000).unwrap_or(i32::MAX);
        let owner = self
            .territory
            .territories
            .get(&castle_id)
            .and_then(|t| t.owner_clan)
            .and_then(|c| self.clan(c));
        let (owner_clan_name, owner_leader_name, owner_ally_name) = match owner {
            Some(clan) => (
                clan.name.clone(),
                self.clan_leader_name(clan.id),
                clan.ally_name.clone().unwrap_or_default(),
            ),
            // "something is wrong"
            None => (NO_OWNER.to_owned(), NO_OWNER.to_owned(), NO_ALLY.to_owned()),
        };
        ExShowDominionRegistry {
            castle_id,
            owner_clan_name,
            owner_leader_name,
            owner_ally_name,
            clan_requests,
            merc_requests,
            war_time,
            current_time: now,
            clan_registered,
            merc_registered,
            territories: self.owned_territory_wards(),
        }
    }

    /// `ExReplyDominionInfo`: the owned territories, their owners, wards and the war start.
    pub fn dominion_info_packet(&self) -> ExReplyDominionInfo {
        let war_start_secs = i32::try_from(self.territory.start_tw_date / 1000).unwrap_or(i32::MAX);
        let territories = self
            .owned_territories()
            .iter()
            .map(|t| DominionTerritory {
                territory_id: t.territory_id,
                name: format!(
                    "{}_dominion",
                    self.castle(CastleId(t.castle_id))
                        .map(|c| c.name.to_lowercase())
                        .unwrap_or_default()
                ),
                owner_clan_name: t
                    .owner_clan
                    .and_then(|c| self.clan(c))
                    .map(|c| c.name.clone())
                    .unwrap_or_default(),
                ward_ids: t.owned_ward_ids(),
                war_start_secs,
            })
            .collect();
        ExReplyDominionInfo { territories }
    }

    /// `ExShowOwnthingPos`: during the war, where each ward is (its NPC, its carrier, or 0).
    pub fn ownthing_pos_packet(&self) -> ExShowOwnthingPos {
        if !self.territory.in_progress {
            return ExShowOwnthingPos::default();
        }
        let wards = self
            .territory
            .wards
            .iter()
            .map(|ward| {
                let at = ward
                    .npc
                    .and_then(|n| self.position_of(n))
                    .or_else(|| ward.player.and_then(|p| self.position_of(p)))
                    .unwrap_or((0, 0, 0));
                WardPosition {
                    territory_id: ward.territory_id,
                    x: at.0,
                    y: at.1,
                    z: at.2,
                }
            })
            .collect();
        ExShowOwnthingPos { wards }
    }
}
