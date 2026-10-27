//! The life of an instance: made from a template (doors and the "general" spawn group), its
//! duration and empty-destroy countdown, and its destruction.
// Java: managers/InstanceManager.java (createInstanceFromTemplate, createDynamicInstance,
//       destroyInstance)
// Java: entity/instancezone/Instance.java (loadInstanceTemplate, spawnDoors, spawnGroup,
//       removePlayer, removePlayers, removeNpcs, removeDoors, setDuration, doCheckTimeUp,
//       cancelTimer, CheckTimeUp, TimeUp)
// Java: entity/instancezone/InstanceWorld.java (destroy, broadcastPacket, openDoor, closeDoor)

use std::sync::Arc;

use l2_net::Encode;
use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{
    ChatType, CreatureSay, SayBody, SenderField, SystemMessage, SystemMessageParam,
};
use l2_util::ObjectId;
use tracing::warn;

use super::{FIRST_DYNAMIC_INSTANCE_ID, Instance, InstanceSpawnInfo, InstanceTask, InstanceTimer};
use crate::door::Door;
use crate::objects::ObjectKind;
use crate::teleport::{TeleportTarget, TeleportWhereType};
use crate::timer::{TimerKind, TimerWheel};
use crate::world::World;

/// `ThreadPool.schedule(new CheckTimeUp(duration), 500)` of `setDuration`.
const DURATION_START_DELAY_MS: i32 = 500;
/// `ThreadPool.schedule(new CheckTimeUp(...), 15000)` of `<activityTime>`.
const ACTIVITY_START_DELAY_MS: i64 = 15_000;

impl World {
    // ------------------------------------------------------------------ creation

    /// Java `InstanceManager.createInstanceFromTemplate(id, templateId)`: `false` when the id is
    /// taken. The doors and the "general" group are spawned.
    pub fn create_instance_from_template(&mut self, id: i32, template_id: i32) -> bool {
        if self.instances.instances.contains_key(&id) {
            return false;
        }
        self.insert_instance(id);
        self.load_instance_template(id, template_id);
        self.spawn_instance_doors(id);
        self.spawn_instance_group(id, "general");
        true
    }

    /// Java `InstanceManager.createDynamicInstance(templateId)`: the next free id from 300000 on;
    /// a template id above 0 loads the template, its doors and its "general" group.
    pub fn create_dynamic_instance(&mut self, template_id: i32) -> i32 {
        while self.instances.instances.contains_key(&self.instances.dynamic) {
            self.instances.dynamic += 1;
            if self.instances.dynamic == i32::MAX {
                warn!(
                    "InstanceManager: More then {} instances created",
                    i32::MAX - FIRST_DYNAMIC_INSTANCE_ID
                );
                self.instances.dynamic = FIRST_DYNAMIC_INSTANCE_ID;
            }
        }
        let id = self.instances.dynamic;
        self.insert_instance(id);
        if template_id > 0 {
            self.load_instance_template(id, template_id);
            self.spawn_instance_doors(id);
            self.spawn_instance_group(id, "general");
        }
        id
    }

    /// `INSTANCES.put(id, new Instance(id))`.
    fn insert_instance(&mut self, id: i32) {
        let instance = Instance::new(
            id,
            self.instances.settings.eject_dead_player_time_ms,
            self.instance_now_ms(),
        );
        self.instances.instances.insert(id, instance);
    }

    /// Java `Instance.loadInstanceTemplate(templateId)` (the parse is done at load time, see
    /// `l2_data::instance`): the template's values go to the instance, and an `<activityTime>`
    /// starts the countdown 15 seconds later.
    fn load_instance_template(&mut self, id: i32, template_id: i32) {
        let templates = Arc::clone(&self.instances.templates);
        let Some(template) = templates.get(template_id) else {
            // Java: the file name is `null`, the read fails with an IOException.
            warn!("Instance: can not find ./data/instances/null ! (template {template_id})");
            return;
        };
        let now = self.instance_now_ms();
        let Some(instance) = self.instances.instances.get_mut(&id) else {
            return;
        };
        instance.template_id = template.id;
        if let Some(eject) = template.eject_time_ms {
            instance.eject_time_ms = eject;
        }
        instance.allow_random_walk = template.allow_random_walk;
        instance.allow_summon = template.allow_summon;
        instance.empty_destroy_time_ms = template.empty_destroy_time_ms;
        instance.show_timer = template.show_timer;
        instance.timer_increase = template.timer_increase;
        instance.timer_text = template.timer_text.clone();
        instance.pvp = template.pvp;
        instance.door_templates = template.doors.clone();
        instance.spawn_templates = template.spawns.clone();
        instance.exit_location = template.exit_point;
        if let Some(locations) = &template.enter_locations {
            instance.enter_locations = locations.clone();
        }
        instance.reenter_type = template.reenter_type;
        instance.reenter_data = template.reenter_data.clone();
        instance.remove_buff_type = template.remove_buff_type;
        instance.buff_exceptions = template.buff_exceptions.clone();
        if let Some(minutes) = template.activity_time_minutes {
            // `new CheckTimeUp(Integer.parseInt(val) * 60000)`: int arithmetic; the end time is
            // `Long.parseLong(val) * 60000 + 15000`.
            // NOTE(java-quirk): this does not cancel a countdown that already runs.
            let remaining = minutes.wrapping_mul(60_000);
            instance.end_time_ms = now + i64::from(minutes) * 60_000 + ACTIVITY_START_DELAY_MS;
            let timer = self.schedule_instance_task(
                id,
                InstanceTask::CheckTimeUp(remaining),
                ACTIVITY_START_DELAY_MS,
            );
            if let Some(instance) = self.instances.instances.get_mut(&id) {
                instance.check_time_up = Some(timer);
            }
        }
    }

    /// Java `Instance.spawnDoors()`: a new door for every door id of the template, in the
    /// instance, at full hit points.
    pub(crate) fn spawn_instance_doors(&mut self, id: i32) {
        let Some(door_ids) = self
            .instances
            .instances
            .get(&id)
            .map(|i| i.door_templates.clone())
        else {
            return;
        };
        let Some(ids) = self.npc_world.ids.clone() else {
            warn!("Instance {id}: no object id factory, the doors are not spawned.");
            return;
        };
        for door_id in door_ids {
            // `DoorData.getDoorTemplate(doorId)`: the template of the door of the normal world.
            let Some(template) = self.door_by_door_id(door_id).map(|d| Arc::clone(&d.template)) else {
                // NOTE(java-quirk): Java gets `null` and the `DoorTemplate` constructor throws, which
                // ends the creation of the instance; here the door is left out.
                warn!("Instance {id}: door {door_id} has no template.");
                continue;
            };
            let object_id = match ids.allocate() {
                Ok(object_id) => object_id,
                Err(e) => {
                    warn!("Instance {id}: door {door_id}: {e}");
                    continue;
                }
            };
            let (x, y, z) = (template.pos_x, template.pos_y, template.pos_z);
            // `new Door(new DoorTemplate(...))` starts at full hit points (`setCurrentHp(getMaxHp())`).
            self.doors.insert(object_id, Door::new(object_id, template));
            self.register_object(object_id, ObjectKind::Door);
            self.set_instance_id(object_id, id);
            self.spawn_at(object_id, x, y, z);
            self.door_snapshot = None;
            if let Some(instance) = self.instances.instances.get_mut(&id) {
                // `_doors.put(doorId, newdoor)`: a door id listed twice keeps the last door.
                instance.doors.insert(door_id, object_id);
            }
        }
    }

    /// Java `Instance.spawnGroup(groupName)`: one spawn per `<npc>` of the group, with the
    /// instance's id, its respawn delay and random walk; the NPCs it made.
    pub fn spawn_instance_group(&mut self, id: i32, group: &str) -> Vec<ObjectId> {
        let Some(spawns) = self
            .instances
            .instances
            .get(&id)
            .map(|i| i.spawn_templates.clone())
        else {
            return Vec::new();
        };
        let mut spawned = Vec::new();
        for set in spawns.iter().filter(|s| s.group == group) {
            // `new Spawn(npcId)` throws for an unknown template or a class Java cannot build.
            let constructible = self
                .npc_world
                .template(set.npc_id)
                .is_some_and(|t| crate::npc::NpcClass::from_type(&t.npc_type).is_some());
            if !constructible {
                warn!("Problem with Instance: no spawnable NPC {}", set.npc_id);
                continue;
            }
            // `setRespawnDelay(respawn, respawnRandom)`.
            let (min_ms, max_ms) = if set.respawn != 0 {
                let min = set.respawn.saturating_sub(set.respawn_random).max(1);
                let max = set.respawn.saturating_add(set.respawn_random).max(1);
                (min.saturating_mul(1000), max.saturating_mul(1000))
            } else {
                (0, 0)
            };
            let index = self.add_runtime_spawn(l2_data::spawn::NpcSpawn {
                npc_id: set.npc_id,
                amount: 1,
                location: l2_data::Location {
                    x: set.x,
                    y: set.y,
                    z: set.z,
                },
                heading: set.heading,
                respawn_min_ms: min_ms,
                respawn_max_ms: max_ms,
                // A new `Spawn` has `_doRespawn = true`.
                respawn_enabled: true,
                chase_range: 0,
                name: None,
                territory: None,
                period_of_day: None,
                ai_data: None,
                group: None,
                source: "instance".to_owned(),
            });
            // `spawnDat.setInstanceId(_id)`, `setRandomWalking(...)`; the NPC is put into the
            // instance when it is created (`npc.rs`).
            self.instances.spawns.insert(
                index,
                InstanceSpawnInfo {
                    instance_id: id,
                    random_walk: set.random_walk,
                    on_kill_delay: set.on_kill_delay,
                },
            );
            let mut report = crate::npc::SpawnReport::default();
            let Some(npc) = self.initialize_npc(index, None, &mut report) else {
                // `spawned.isAttackable()` on null: the exception skips the rest of the entry.
                warn!("Problem with Instance: NPC {} was not spawned", set.npc_id);
                continue;
            };
            if set.respawn == 0 {
                self.stop_respawn(index);
            } else {
                self.start_respawn(index);
            }
            spawned.push(npc);
        }
        spawned
    }

    // ------------------------------------------------------------------ players and timers

    /// Java `Instance.removePlayer(objectId)`: out of the list; the last one out starts the
    /// empty-destroy countdown.
    pub(crate) fn instance_remove_player(&mut self, id: i32, player: ObjectId) {
        let now = self.instance_now_ms();
        let Some(instance) = self.instances.instances.get_mut(&id) else {
            return;
        };
        instance.players.remove(&player);
        if !instance.players.is_empty() || instance.empty_destroy_time_ms < 0 {
            return;
        }
        instance.last_left_ms = now;
        // NOTE(java-quirk): `(int) (_instanceEndTime - now - 500)`: for an instance without an end
        // time (-1) the long is truncated to an int, so the "duration" is whatever the low 32 bits
        // of a large negative number are.
        let duration = (instance.end_time_ms - now - i64::from(DURATION_START_DELAY_MS)) as i32;
        self.set_instance_duration(id, duration);
    }

    /// Java `Instance.setDuration(duration)`: the countdown starts again in 500 ms.
    pub fn set_instance_duration(&mut self, id: i32, duration_ms: i32) {
        let now = self.instance_now_ms();
        if let Some(timer) = self
            .instances
            .instances
            .get_mut(&id)
            .and_then(|i| i.check_time_up.take())
        {
            self.timers.cancel(timer);
        }
        if !self.instances.instances.contains_key(&id) {
            return;
        }
        let timer = self.schedule_instance_task(
            id,
            InstanceTask::CheckTimeUp(duration_ms),
            i64::from(DURATION_START_DELAY_MS),
        );
        if let Some(instance) = self.instances.instances.get_mut(&id) {
            instance.check_time_up = Some(timer);
            instance.end_time_ms = now + i64::from(duration_ms) + i64::from(DURATION_START_DELAY_MS);
        }
    }

    /// Java `Instance.setEmptyDestroyTime(time)`.
    pub fn set_instance_empty_destroy_time(&mut self, id: i32, time_ms: i64) {
        if let Some(instance) = self.instances.instances.get_mut(&id) {
            instance.empty_destroy_time_ms = time_ms;
        }
    }

    /// Java `Instance.cancelTimer()`.
    fn cancel_instance_timer(&mut self, id: i32) {
        // NOTE(java-quirk): Java cancels `_checkTimeUpTask` but keeps the reference, which does
        // not matter: every later schedule replaces it.
        if let Some(timer) = self
            .instances
            .instances
            .get_mut(&id)
            .and_then(|i| i.check_time_up.take())
        {
            self.timers.cancel(timer);
        }
    }

    pub(crate) fn schedule_instance_task(
        &mut self,
        instance_id: i32,
        task: InstanceTask,
        delay_ms: i64,
    ) -> crate::timer::TimerId {
        let due = self.tick + TimerWheel::ms_to_ticks(u64::try_from(delay_ms).unwrap_or(0));
        self.timers
            .schedule_at(due, TimerKind::Instance(InstanceTimer { instance_id, task }))
    }

    /// Java `CheckTimeUp.run`, `TimeUp.run` and the eject task of `notifyDeath`.
    pub(crate) fn on_instance_timer(&mut self, timer: InstanceTimer) {
        match timer.task {
            InstanceTask::CheckTimeUp(remaining) => {
                if let Some(instance) = self.instances.instances.get_mut(&timer.instance_id) {
                    instance.check_time_up = None;
                    self.do_check_time_up(timer.instance_id, remaining);
                }
            }
            InstanceTask::TimeUp => {
                if let Some(instance) = self.instances.instances.get_mut(&timer.instance_id) {
                    instance.check_time_up = None;
                }
                self.destroy_instance(timer.instance_id);
            }
            InstanceTask::EjectDead { player, exit } => {
                self.eject_dead_player(timer.instance_id, player, exit)
            }
        }
    }

    /// Java `Instance.doCheckTimeUp(value)`: the "expires in N minutes" messages, the "N seconds
    /// left" notices, the next check or the end.
    fn do_check_time_up(&mut self, id: i32, value: i32) {
        let now = self.instance_now_ms();
        let Some(instance) = self.instances.instances.get(&id) else {
            return;
        };
        let empty = instance.players.is_empty();
        let empty_destroy = instance.empty_destroy_time_ms;
        let last_left = instance.last_left_ms;
        let members: Vec<ObjectId> = instance.players.iter().copied().collect();
        let mut remaining = value;
        let interval: i32;
        let mut notice: Option<String> = None;
        let mut expire_minutes: Option<i32> = None;
        if empty && empty_destroy == 0 {
            remaining = 0;
            interval = 500;
        } else if empty && empty_destroy > 0 {
            let empty_time_left = (last_left + empty_destroy) - now;
            if empty_time_left <= 0 {
                interval = 0;
                remaining = 0;
            } else if remaining > 300_000 && empty_time_left > 300_000 {
                interval = 300_000;
                remaining -= 300_000;
            } else if remaining > 60_000 && empty_time_left > 60_000 {
                interval = 60_000;
                remaining -= 60_000;
            } else if remaining > 30_000 && empty_time_left > 30_000 {
                interval = 30_000;
                remaining -= 30_000;
            } else {
                interval = 10_000;
                remaining = remaining.wrapping_sub(10_000);
            }
        } else if remaining > 300_000 {
            expire_minutes = Some(remaining / 60_000);
            interval = 300_000;
            remaining -= 300_000;
        } else if remaining > 60_000 {
            expire_minutes = Some(remaining / 60_000);
            interval = 60_000;
            remaining -= 60_000;
        } else if remaining > 30_000 {
            notice = Some(format!("{} seconds left.", remaining / 1000));
            interval = 30_000;
            remaining -= 30_000;
        } else {
            notice = Some(format!("{} seconds left.", remaining / 1000));
            interval = 10_000;
            remaining = remaining.wrapping_sub(10_000);
        }

        if let Some(minutes) = expire_minutes {
            let sm = SystemMessage::new(
                SystemMessageId::ThisDungeonWillExpireInS1MinuteSYouWillBeForcedOutOfTheDungeonWhenTheTimeExpires,
            )
            .with(SystemMessageParam::Text(minutes.to_string()));
            let bytes = sm.to_bytes();
            for player in &members {
                // `player.isOnline() && player.getInstanceId() == _id`.
                if self.players.contains_key(player) && self.instance_id_of(*player) == id {
                    self.send_bytes(*player, bytes.clone());
                }
            }
        }
        if let Some(text) = notice {
            // `new CreatureSay(null, ChatType.ALLIANCE, "Notice", text)`.
            let cs = CreatureSay {
                sender_object_id: 0,
                chat_type: ChatType::Alliance,
                sender: SenderField::Name("Notice".to_owned()),
                message_id: -1,
                body: SayBody::Text(text),
            };
            let bytes = cs.to_bytes();
            for player in &members {
                if self.players.contains_key(player) && self.instance_id_of(*player) == id {
                    self.send_bytes(*player, bytes.clone());
                }
            }
        }

        self.cancel_instance_timer(id);
        let task = if remaining >= 10_000 {
            InstanceTask::CheckTimeUp(remaining)
        } else {
            InstanceTask::TimeUp
        };
        let timer = self.schedule_instance_task(id, task, i64::from(interval));
        if let Some(instance) = self.instances.instances.get_mut(&id) {
            instance.check_time_up = Some(timer);
        }
    }

    // ------------------------------------------------------------------ destruction

    /// Java `InstanceManager.destroyInstance(instanceId)`: the NPCs go, the players are put out
    /// (to the exit point or the nearest town), the doors go, the countdown stops. Ids of 0 and
    /// below are never destroyed.
    pub fn destroy_instance(&mut self, id: i32) {
        if id <= 0 || !self.instances.instances.contains_key(&id) {
            return;
        }
        self.remove_instance_npcs(id);
        self.remove_instance_players(id);
        self.remove_instance_doors(id);
        self.cancel_instance_timer(id);
        self.instances.instances.remove(&id);
        self.instances.worlds.remove(&id);
        self.instances.spawns.retain(|_, s| s.instance_id != id);
    }

    /// Java `Instance.removeNpcs()`.
    fn remove_instance_npcs(&mut self, id: i32) {
        let npcs: Vec<(ObjectId, Option<usize>)> = self
            .instances
            .instances
            .get(&id)
            .map(|i| i.npcs.iter().map(|(n, s)| (*n, *s)).collect())
            .unwrap_or_default();
        for (npc, spawn) in npcs {
            if let Some(spawn) = spawn {
                self.stop_respawn(spawn);
            }
            self.delete_npc(npc);
        }
        if let Some(instance) = self.instances.instances.get_mut(&id) {
            instance.npcs.clear();
        }
    }

    /// Java `Instance.removePlayers()`.
    fn remove_instance_players(&mut self, id: i32) {
        let (players, exit) = match self.instances.instances.get(&id) {
            Some(i) => (i.players.iter().copied().collect::<Vec<_>>(), i.exit_location),
            None => return,
        };
        for player in players {
            if self.players.contains_key(&player) && self.instance_id_of(player) == id {
                self.set_instance_id(player, 0);
                self.teleport_out_of_instance(player, exit);
            }
        }
        if let Some(instance) = self.instances.instances.get_mut(&id) {
            instance.players.clear();
        }
    }

    /// `player.teleToLocation(_exitLocation, true)`, else `teleToLocation(TeleportWhereType.TOWN)`.
    /// The exit `Location` has instance id 0.
    pub(crate) fn teleport_out_of_instance(&mut self, player: ObjectId, exit: Option<(i32, i32, i32)>) {
        match exit {
            Some((x, y, z)) => {
                let offset = self.settings.teleport.max_offset_on_teleport;
                self.tele_to_location(
                    player,
                    TeleportTarget::at(x, y, z)
                        .with_instance(0)
                        .with_random_offset(offset),
                );
            }
            None => self.tele_to_location_where(player, TeleportWhereType::Town),
        }
    }

    /// Java `Instance.removeDoors()`.
    fn remove_instance_doors(&mut self, id: i32) {
        let doors: Vec<ObjectId> = self
            .instances
            .instances
            .get(&id)
            .map(|i| i.doors.values().copied().collect())
            .unwrap_or_default();
        for door in doors {
            // `door.decayMe()`, `region.removeVisibleObject(door)`, `World.removeObject(door)`.
            self.decay(door);
            self.doors.remove(&door);
        }
        self.door_snapshot = None;
        if let Some(instance) = self.instances.instances.get_mut(&id) {
            instance.doors.clear();
        }
    }

    // ------------------------------------------------------------------ InstanceWorld helpers

    /// Java `InstanceWorld.destroy()`: "Use this method to destroy instance world properly": no
    /// grace for an empty instance, and the end in one second.
    pub fn destroy_instance_world(&mut self, instance_id: i32) {
        self.set_instance_empty_destroy_time(instance_id, 0);
        self.set_instance_duration(instance_id, 1000);
    }

    /// Java `InstanceWorld.broadcastPacket(packets)`: to every player of the instance who is in it.
    pub fn instance_broadcast_packet(&self, instance_id: i32, packet: &impl Encode) {
        let Some(instance) = self.instances.instances.get(&instance_id) else {
            return;
        };
        let bytes = packet.to_bytes();
        for player in &instance.players {
            if self.players.contains_key(player) && self.instance_id_of(*player) == instance_id {
                self.send_bytes(*player, bytes.clone());
            }
        }
    }

    /// Java `Instance.getDoor(doorId)`.
    pub fn instance_door(&self, instance_id: i32, door_id: i32) -> Option<ObjectId> {
        self.instances
            .instances
            .get(&instance_id)
            .and_then(|i| i.doors.get(&door_id).copied())
    }

    /// Java `InstanceWorld.openDoor(doorId)`: opens the door of the instance if it is closed.
    pub fn instance_open_door(&mut self, instance_id: i32, door_id: i32) {
        if let Some(door) = self.instance_door(instance_id, door_id)
            && self.doors.get(&door).is_some_and(|d| !d.open)
        {
            self.open_door(door);
        }
    }

    /// Java `InstanceWorld.closeDoor(doorId)`.
    pub fn instance_close_door(&mut self, instance_id: i32, door_id: i32) {
        if let Some(door) = self.instance_door(instance_id, door_id)
            && self.doors.get(&door).is_some_and(|d| d.open)
        {
            self.close_door(door);
        }
    }

    /// Java `Instance.addEnterLoc` / `setExitLoc` / `setAllowSummon` / `setPvP` /
    /// `setEjectTime` / `setReenterType` for the scripts: the instance to change.
    pub fn instance_mut(&mut self, id: i32) -> Option<&mut Instance> {
        self.instances.instances.get_mut(&id)
    }
}
