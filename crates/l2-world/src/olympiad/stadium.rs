//! Java `OlympiadStadiumZone` and the stadium half of `OlympiadGameManager`: one stadium per
//! zone, each in its own copy of the arena (an instance), the players it holds, its broadcasts,
//! its doors and buffers.
// Java: entity/zone/type/OlympiadStadiumZone.java
// Java: mechanics/olympiad/OlympiadGameManager.java (the constructor)
//
// Java gives every stadium zone the instance of its arena (`_zone.setInstanceId`), and the zone
// only takes creatures of that instance (`isAffected`). The world keeps the zone shapes
// immutable, so the zone code asks [`World::stadium_takes`] for that check.
// NOTE: before `OlympiadGameManager` exists Java's stadium zones have the instance id 0 and take
// the players of the normal world at the arenas' coordinates; here a stadium zone takes nobody
// until the manager made the stadiums (the Olympiad loads them at start, Java at the first
// competition).

use l2_data::Zone;
use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{
    ExOlympiadMatchEnd, ExOlympiadSpelledInfo, ExOlympiadUserInfo, OlympiadSpelledEffect, SystemMessage,
};
use l2_util::ObjectId;
use tracing::{info, warn};

use super::task::GameTask;
use super::{OlympiadTimer, Stadium};
use crate::teleport::TeleportWhereType;
use crate::world::World;
use crate::zones::ZoneId;

/// The zone type of the stadiums.
pub(crate) const STADIUM_ZONE: &str = "OlympiadStadiumZone";

impl World {
    /// `OlympiadGameManager.getInstance()`: the stadiums, made once (one arena instance per
    /// `OlympiadStadiumZone`).
    pub(crate) fn olympiad_game_manager(&mut self) {
        if self.olympiad.stadiums.is_some() {
            return;
        }
        let zones: Vec<Zone> = self
            .zoning
            .zones
            .as_ref()
            .map(|z| z.all_of_type(STADIUM_ZONE).into_iter().cloned().collect())
            .unwrap_or_default();
        if zones.is_empty() {
            // Java throws `new Error("No olympiad stadium zones defined !")`.
            warn!("No olympiad stadium zones defined !");
            self.olympiad.stadiums = Some(Vec::new());
            return;
        }
        let mut stadiums = Vec::with_capacity(zones.len());
        // NOTE(java-quirk): the id lives outside the loop, so a zone with another name gets the
        // instance of the zone before it (0 for the first).
        let mut instance_id = 0;
        for zone in zones {
            let name = zone.name.clone().unwrap_or_default();
            let template = match name.as_str() {
                "Grassy Arena" => Some(147),
                "Three Bridges Arena" => Some(148),
                "Heros's Vestiges Arena" => Some(149),
                "Orbis Arena" => Some(150),
                _ => None,
            };
            if let Some(template) = template {
                instance_id = self.create_dynamic_instance(template);
            }
            stadiums.push(Stadium {
                zone_id: zone.id,
                name,
                instance_id,
                spawns: zone.spawns.spawns.iter().map(|l| (l.x, l.y, l.z)).collect(),
                spectator_spawns: zone.spawns.spectator.iter().map(|l| (l.x, l.y, l.z)).collect(),
                task: GameTask::default(),
            });
        }
        info!("Olympiad System: Loaded {} stadiums.", stadiums.len());
        self.olympiad.stadiums = Some(stadiums);
    }

    /// The stadium of an `OlympiadStadiumZone`.
    pub(crate) fn stadium_of_zone(&self, zone_id: i32) -> Option<usize> {
        self.olympiad.stadiums().iter().position(|s| s.zone_id == zone_id)
    }

    /// `OlympiadStadiumZone.isAffected`'s own part: the creature is in the stadium's instance.
    pub(crate) fn stadium_takes(&self, zone_id: i32, id: ObjectId) -> bool {
        self.stadium_of_zone(zone_id)
            .and_then(|index| self.olympiad.stadiums().get(index))
            .is_some_and(|s| self.instance_id_of(id) == s.instance_id)
    }

    /// The players the stadium zone holds (`getPlayersInside()`) that are in its instance.
    fn stadium_players(&self, index: usize) -> Vec<ObjectId> {
        let Some(stadium) = self.olympiad.stadiums().get(index) else {
            return Vec::new();
        };
        let mut players: Vec<ObjectId> = self
            .players
            .iter()
            .filter(|(_, p)| {
                p.zones
                    .members()
                    .iter()
                    .any(|(zid, ty)| *zid == stadium.zone_id && ty == STADIUM_ZONE)
            })
            .map(|(id, _)| *id)
            .filter(|id| self.instance_id_of(*id) == stadium.instance_id)
            .collect();
        players.sort_unstable();
        players
    }

    /// `OlympiadStadiumZone.broadcastPacket(packet)`.
    pub(crate) fn stadium_broadcast(&mut self, index: usize, packet: &impl l2_net::Encode) {
        let bytes = packet.to_bytes();
        for player in self.stadium_players(index) {
            self.send_bytes(player, bytes.clone());
        }
    }

    /// `broadcastPacketToObservers(packet)`.
    fn stadium_broadcast_to_observers(&self, index: usize, packet: &impl l2_net::Encode) {
        let bytes = packet.to_bytes();
        for player in self.stadium_players(index) {
            if self.in_observer_mode(player) {
                self.send_bytes(player, bytes.clone());
            }
        }
    }

    /// `openDoors()`: the doors of the stadium's instance.
    pub(crate) fn stadium_open_doors(&mut self, index: usize) {
        let Some(instance_id) = self.olympiad.stadiums().get(index).map(|s| s.instance_id) else {
            return;
        };
        let doors: Vec<i32> = self
            .instance(instance_id)
            .map(|i| i.doors.keys().copied().collect())
            .unwrap_or_default();
        for door in doors {
            self.instance_open_door(instance_id, door);
        }
    }

    /// `closeDoors()`.
    pub(crate) fn stadium_close_doors(&mut self, index: usize) {
        let Some(instance_id) = self.olympiad.stadiums().get(index).map(|s| s.instance_id) else {
            return;
        };
        let doors: Vec<i32> = self
            .instance(instance_id)
            .map(|i| i.doors.keys().copied().collect())
            .unwrap_or_default();
        for door in doors {
            self.instance_close_door(instance_id, door);
        }
    }

    /// The `OlympiadManager` NPCs (the buffers) of the stadium's instance.
    fn stadium_buffers(&self, index: usize) -> Vec<ObjectId> {
        let Some(instance_id) = self.olympiad.stadiums().get(index).map(|s| s.instance_id) else {
            return Vec::new();
        };
        let Some(instance) = self.instance(instance_id) else {
            return Vec::new();
        };
        instance
            .npcs
            .keys()
            .copied()
            .filter(|npc| {
                self.npcs
                    .get(npc)
                    .and_then(|n| self.npc_world.template(n.npc_id))
                    .is_some_and(|t| t.npc_type == "OlympiadManager")
            })
            .collect()
    }

    /// `spawnBuffers()`.
    pub(crate) fn stadium_spawn_buffers(&mut self, index: usize) {
        for buffer in self.stadium_buffers(index) {
            if self.objects.get(&buffer).is_some_and(|e| !e.spawned) {
                self.spawn(buffer);
            }
        }
    }

    /// `deleteBuffers()`.
    pub(crate) fn stadium_delete_buffers(&mut self, index: usize) {
        for buffer in self.stadium_buffers(index) {
            if self.objects.get(&buffer).is_some_and(|e| e.spawned) {
                self.decay_keep_entry(buffer);
            }
        }
    }

    /// `OlympiadStadiumZone.onEnter(creature)` for a player.
    pub(crate) fn stadium_on_enter(&mut self, id: ObjectId, zone: &Zone) {
        self.set_inside(id, ZoneId::Pvp, true);
        let Some(index) = self.stadium_of_zone(zone.id) else {
            return;
        };
        if self
            .olympiad
            .stadiums()
            .get(index)
            .is_some_and(|s| s.task.game.is_some() && s.task.is_battle_started())
        {
            self.send(
                id,
                &SystemMessage::new(SystemMessageId::YouHaveEnteredACombatZone),
            );
            self.game_send_olympiad_info(index, id);
        }
        // "only participants, observers and GMs allowed"
        if !self.is_gm(id) && !self.is_in_olympiad_mode(id) && !self.in_observer_mode(id) {
            self.schedule_olympiad_timer(OlympiadTimer::KickPlayer(id), 0);
        } else if let Some(summon) = self.summon_of(id)
            && self.is_pet(summon)
        {
            // "check for pet"
            self.unsummon(summon, Some(id));
        }
    }

    /// `OlympiadStadiumZone.onExit(creature)` for a player.
    pub(crate) fn stadium_on_exit(&mut self, id: ObjectId, zone: &Zone) {
        self.set_inside(id, ZoneId::Pvp, false);
        let Some(index) = self.stadium_of_zone(zone.id) else {
            return;
        };
        if self
            .olympiad
            .stadiums()
            .get(index)
            .is_some_and(|s| s.task.game.is_some() && s.task.is_battle_started())
        {
            self.send(id, &SystemMessage::new(SystemMessageId::YouHaveLeftACombatZone));
            self.send(id, &ExOlympiadMatchEnd);
        }
    }

    /// `KickPlayer.run()`.
    pub(crate) fn stadium_kick_player(&mut self, id: ObjectId) {
        if !self.players.contains_key(&id) {
            return;
        }
        if let Some(summon) = self.summon_of(id) {
            self.unsummon(summon, Some(id));
        }
        self.tele_to_location_where(id, TeleportWhereType::Town);
        self.set_instance_id(id, 0);
    }

    /// `updateZoneStatusForCharactersInside()`.
    pub(crate) fn stadium_update_zone_status(&mut self, index: usize) {
        let Some(stadium) = self.olympiad.stadiums().get(index) else {
            return;
        };
        let battle_started = stadium.task.is_battle_started();
        let message = if battle_started {
            SystemMessage::new(SystemMessageId::YouHaveEnteredACombatZone)
        } else {
            SystemMessage::new(SystemMessageId::YouHaveLeftACombatZone)
        };
        // NPCs keep no zone state in this world (`zones.rs`), so only the players change.
        for player in self.stadium_players(index) {
            if battle_started {
                self.set_inside(player, ZoneId::Pvp, true);
                self.send(player, &message);
            } else {
                self.set_inside(player, ZoneId::Pvp, false);
                self.send(player, &message);
                self.send(player, &ExOlympiadMatchEnd);
            }
            self.broadcast_info(player);
        }
    }

    /// `broadcastStatusUpdate(player)`: the HP and CP of a fighter to the observers and the other
    /// side (the `Player.broadcastStatusUpdate` part of a running battle).
    pub(crate) fn olympiad_broadcast_status_update(&mut self, id: ObjectId) {
        let Some(state) = self.player_olympiad(id) else {
            return;
        };
        if !state.in_olympiad_mode || !state.olympiad_start {
            return;
        }
        let Ok(index) = usize::try_from(state.game_id) else {
            return;
        };
        if !self
            .olympiad
            .stadiums()
            .get(index)
            .is_some_and(|s| s.task.is_battle_started())
        {
            return;
        }
        let (Some(p), Some(view)) = (self.players.get(&id), self.stats(id)) else {
            return;
        };
        let packet = ExOlympiadUserInfo {
            side: u8::try_from(state.side).unwrap_or(0),
            object_id: id.0,
            name: p.data.name.clone(),
            class_id: p.data.class_id,
            cur_hp: p.core.cur_hp as i32,
            max_hp: view.max_hp(),
            cur_cp: p.core.cur_cp as i32,
            max_cp: view.max_cp(),
        };
        let instance = self.instance_id_of(id);
        for target in self.stadium_players(index) {
            if (self.in_observer_mode(target) || self.olympiad_side(target) != state.side)
                && self.instance_id_of(target) == instance
            {
                self.send(target, &packet);
            }
        }
    }

    /// The `ExOlympiadSpelledInfo` part of `EffectList.updateEffectIcons()`: the buffs of a
    /// fighter to the observers of a running battle.
    pub(crate) fn olympiad_spelled_info(&mut self, id: ObjectId) {
        let Some(state) = self.player_olympiad(id) else {
            return;
        };
        if !state.in_olympiad_mode || !state.olympiad_start {
            return;
        }
        let Ok(index) = usize::try_from(state.game_id) else {
            return;
        };
        if !self
            .olympiad
            .stadiums()
            .get(index)
            .is_some_and(|s| s.task.is_battle_started())
        {
            return;
        }
        let tick = self.tick;
        let effects = self
            .effect_list(id)
            .map(|list| {
                list.effects()
                    .filter(|i| i.in_use)
                    .map(|i| OlympiadSpelledEffect {
                        skill_id: i.facts.display_id,
                        level: i16::try_from(i.facts.display_level).unwrap_or(0),
                        time: i.time(tick),
                    })
                    .collect()
            })
            .unwrap_or_default();
        self.stadium_broadcast_to_observers(
            index,
            &ExOlympiadSpelledInfo {
                player_id: id.0,
                effects,
            },
        );
    }
}
