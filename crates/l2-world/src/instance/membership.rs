//! Who is in which instance: Java `WorldObject.setInstanceId` and the places that move a player
//! in or out (enter world, logout, a death inside, the restart point).
// Java: entity/WorldObject.java (setInstanceId, sendInstanceUpdate)
// Java: entity/instancezone/Instance.java (addPlayer, addNpc, removeNpc, notifyDeath,
//       cancelEjectDeadPlayer)
// Java: entity/spawns/Spawn.java (doSpawn: `npc.setInstanceId(getInstanceId())`, the
//       `broadcastInfo` of `initializeNpc`), entity/actor/holders/npc/MinionList.java
//       (`minion.setInstanceId(master.getInstanceId())`)
// Java: network/clientpackets/{EnterWorld,Logout,RequestRestart}.java (the instance blocks)
// Java: entity/actor/Player.java (deleteMe's instance block, doRevive), Playable.java (doDie)

use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{ExSendUiEvent, SystemMessage, SystemMessageParam};
use l2_util::ObjectId;

use super::{InstanceTask, InstanceTimeWrite};
use crate::objects::ObjectKind;
use crate::outbox::PersistCommand;
use crate::teleport::TeleportWhereType;
use crate::world::World;

/// `PlayerVariables.RESTORE_LOCATION`.
const RESTORE_LOCATION: &str = "RESTORE_LOCATION";

impl World {
    /// Java `WorldObject.setInstanceId(instanceId)`: a negative id or the current one does
    /// nothing, nor does an id with no instance behind it. A player leaves the old instance's
    /// list (and its timer) and joins the new one's, its summon follows; an NPC does the same with
    /// the NPC lists. Everything else only changes its id.
    ///
    /// The object stays where it is in the region grid: callers that want the change seen
    /// (everybody of the old instance forgets it, everybody of the new one learns about it)
    /// teleport it, which takes it out of view and back (`teleport.rs`), as Java's callers do.
    pub fn set_instance_id(&mut self, id: ObjectId, instance_id: i32) {
        let Some(entry) = self.objects.get(&id) else {
            return;
        };
        let old = entry.instance_id;
        let kind = entry.kind;
        if instance_id < 0 || old == instance_id {
            return;
        }
        if !self.instances.instances.contains_key(&instance_id) {
            return;
        }
        let old_exists = self.instances.instances.contains_key(&old);
        match kind {
            ObjectKind::Player => {
                if old > 0 && old_exists {
                    self.instance_remove_player(old, id);
                    if self.instances.instances.get(&old).is_some_and(|i| i.show_timer) {
                        self.send_instance_update(id, old, true);
                    }
                }
                if instance_id > 0 {
                    if let Some(instance) = self.instances.instances.get_mut(&instance_id) {
                        instance.players.insert(id);
                    }
                    if self
                        .instances
                        .instances
                        .get(&instance_id)
                        .is_some_and(|i| i.show_timer)
                    {
                        self.send_instance_update(id, instance_id, false);
                    }
                }
                if let Some(summon) = self.summon_of(id) {
                    self.set_instance_id(summon, instance_id);
                }
            }
            ObjectKind::Npc => {
                let spawn = self.npcs.get(&id).and_then(|n| n.spawn);
                if old > 0 && old_exists {
                    // `removeNpc`: the spawn stops respawning.
                    if let Some(spawn) = spawn {
                        self.stop_respawn(spawn);
                    }
                    if let Some(instance) = self.instances.instances.get_mut(&old) {
                        instance.npcs.remove(&id);
                    }
                }
                if instance_id > 0
                    && let Some(instance) = self.instances.instances.get_mut(&instance_id)
                {
                    instance.npcs.insert(id, spawn);
                }
            }
            _ => {}
        }
        if let Some(entry) = self.objects.get_mut(&id) {
            entry.instance_id = instance_id;
        }
    }

    /// Java `WorldObject.sendInstanceUpdate(instance, hide)`: the instance timer of the client.
    fn send_instance_update(&self, player: ObjectId, instance_id: i32, hide: bool) {
        let Some(instance) = self.instances.instances.get(&instance_id) else {
            return;
        };
        let start_time = ((self.instance_now_ms() - instance.start_time_ms) / 1000) as i32;
        let end_time = ((instance.end_time_ms - instance.start_time_ms) / 1000) as i32;
        // `new ExSendUIEvent(player, hide, countUp, start, end, text)`: the text is the one
        // parameter, the NPC string id -1.
        let packet = if instance.timer_increase {
            ExSendUiEvent {
                object_id: player.0,
                hide,
                count_up: true,
                start_time,
                end_time,
                npc_string_id: -1,
                params: vec![instance.timer_text.clone()],
            }
        } else {
            ExSendUiEvent {
                object_id: player.0,
                hide,
                count_up: false,
                start_time: end_time.wrapping_sub(start_time),
                end_time: 0,
                npc_string_id: -1,
                params: vec![instance.timer_text.clone()],
            }
        };
        self.send(player, &packet);
    }

    /// The instance part of a new NPC, called by `npc.rs` before the NPC is spawned: Java's
    /// `Spawn.doSpawn` puts it into its spawn's instance ("Must be done before object is spawned
    /// into visible world"), `MinionList.initializeNpc` into its master's. A respawn (the same
    /// object id) gets its spawn's instance again; Java's object kept it. Returns the instance.
    pub(crate) fn instance_on_npc_created(
        &mut self,
        id: ObjectId,
        spawn: Option<usize>,
        leader: Option<ObjectId>,
    ) -> i32 {
        let instance_id = match leader {
            Some(master) => self.instance_id_of(master),
            None => spawn
                .and_then(|s| self.instances.spawns.get(&s))
                .map_or(0, |s| s.instance_id),
        };
        self.set_instance_id(id, instance_id);
        self.instance_id_of(id)
    }

    // ------------------------------------------------------------------ enter and leave

    /// Java `EnterWorld`: with `RestorePlayerInstance` the player goes back into the instance
    /// whose list still holds him; without it he is taken off that list. Also the re-entry times
    /// the session read at `CharacterSelect` (`times.rs`).
    pub(crate) fn instance_on_enter_world(&mut self, id: ObjectId) {
        self.instance_times_on_enter(id);
        let instance_id = self.instance_with_player(id);
        if self.instances.settings.restore_player_instance {
            self.set_instance_id(id, instance_id);
        } else if instance_id > 0 {
            self.instance_remove_player(instance_id, id);
        }
    }

    /// Java `Logout` / `RequestRestart` without `RestorePlayerInstance`: a player inside an
    /// instance leaves it, and the next login starts at the exit point (or the nearest town),
    /// through the `RESTORE_LOCATION` variable.
    pub(crate) fn instance_on_logout(&mut self, id: ObjectId) {
        if self.instances.settings.restore_player_instance {
            return;
        }
        let instance_id = self.instance_id_of(id);
        if instance_id <= 0 {
            return;
        }
        let Some(exit) = self
            .instances
            .instances
            .get(&instance_id)
            .map(|i| i.exit_location)
        else {
            return;
        };
        self.set_instance_id(id, 0);
        let location = match exit {
            Some(location) => Some(location),
            None => self.get_tele_to_location(id, TeleportWhereType::Town),
        };
        // Java: `location.getX() + ";" + ...` (a missing town is a NullPointerException there).
        if let Some((x, y, z)) = location
            && let Some(player) = self.players.get_mut(&id)
        {
            let profile = &mut player.data.profile;
            profile
                .variables
                .insert(RESTORE_LOCATION.to_owned(), format!("{x};{y};{z}"));
            profile.variables_removed.remove(RESTORE_LOCATION);
            profile.variables_changed.insert(RESTORE_LOCATION.to_owned());
        }
        // NOTE(java-quirk): `setInstanceId(0)` already took the player off the list; Java calls
        // `removePlayer` a second time, which starts the empty countdown again.
        self.instance_remove_player(instance_id, id);
    }

    /// Java `Player.deleteMe`'s instance block (after the store, so the position it changes is
    /// not saved): without `RestorePlayerInstance` the player leaves the instance's list, and a
    /// summon still out (a dead pet) is sent to the exit point, out of the instance.
    pub(crate) fn instance_on_delete(&mut self, id: ObjectId) {
        // The re-entry times belong to the online character (they are read again at the next
        // `CharacterSelect`).
        self.instances.player_times.remove(&id);
        let instance_id = self.instance_id_of(id);
        if instance_id == 0 || self.instances.settings.restore_player_instance {
            return;
        }
        if !self.instances.instances.contains_key(&instance_id) {
            return;
        }
        self.instance_remove_player(instance_id, id);
        let exit = self
            .instances
            .instances
            .get(&instance_id)
            .and_then(|i| i.exit_location);
        if let Some((x, y, z)) = exit {
            // `setXYZInvisible(x + Rnd.get(-30, 30), y + Rnd.get(-30, 30), z)`: the player is about
            // to leave the tables, so only the draws are kept.
            let _ = self.rng().get(-30, 30);
            let _ = self.rng().get(-30, 30);
            if let Some(summon) = self.summon_of(id) {
                let offset = self.settings.teleport.max_offset_on_teleport;
                self.tele_to_location(
                    summon,
                    crate::teleport::TeleportTarget::at(x, y, z)
                        .with_instance(0)
                        .with_random_offset(offset),
                );
                self.set_instance_id(summon, 0);
            }
        }
    }

    // ------------------------------------------------------------------ death

    /// Java `Playable.doDie`: "Notify instance." A player who dies in an instance is told how
    /// long he may stay dead, and put out after that time if he is still dead and still inside.
    pub(crate) fn instance_on_player_death(&mut self, id: ObjectId) {
        let instance_id = self.instance_id_of(id);
        if !self.is_player(id) || instance_id <= 0 {
            return;
        }
        let Some((eject_time, exit)) = self
            .instances
            .instances
            .get(&instance_id)
            .map(|i| (i.eject_time_ms, i.exit_location))
        else {
            return;
        };
        // `!player.isOnEvent()`: the custom events are not ported.
        if eject_time <= 0 {
            return;
        }
        self.send(
            id,
            &SystemMessage::new(
                SystemMessageId::IfYouAreNotResurrectedWithinS1MinutesYouWillBeExpelledFromTheInstantZone,
            )
            .with(SystemMessageParam::IntNumber(eject_time / 60 / 1000)),
        );
        let timer = self.schedule_instance_task(
            instance_id,
            InstanceTask::EjectDead { player: id, exit },
            i64::from(eject_time),
        );
        // NOTE(java-quirk): `_ejectDeadTasks.put` replaces an older task of the player without
        // cancelling it; that one still runs.
        if let Some(instance) = self.instances.instances.get_mut(&instance_id) {
            instance.eject_dead.insert(id, timer);
        }
    }

    /// The eject task of `notifyDeath`.
    pub(crate) fn eject_dead_player(
        &mut self,
        instance_id: i32,
        player: ObjectId,
        exit: Option<(i32, i32, i32)>,
    ) {
        if let Some(instance) = self.instances.instances.get_mut(&instance_id) {
            instance.eject_dead.remove(&player);
        }
        if self.players.contains_key(&player)
            && self.is_dead(player)
            && self.instance_id_of(player) == instance_id
        {
            self.set_instance_id(player, 0);
            self.teleport_out_of_instance(player, exit);
        }
    }

    /// Java `Player.doRevive`: `instance.cancelEjectDeadPlayer(this)`.
    pub(crate) fn instance_on_player_revive(&mut self, id: ObjectId) {
        let instance_id = self.instance_id_of(id);
        if instance_id <= 0 {
            return;
        }
        let timer = self
            .instances
            .instances
            .get_mut(&instance_id)
            .and_then(|i| i.eject_dead.remove(&id));
        if let Some(timer) = timer {
            self.timers.cancel(timer);
        }
    }

    // ------------------------------------------------------------------ re-entry time writes

    pub(crate) fn persist_instance_time(&mut self, write: InstanceTimeWrite) {
        self.outbox.persist(PersistCommand::InstanceTime(write));
    }
}
