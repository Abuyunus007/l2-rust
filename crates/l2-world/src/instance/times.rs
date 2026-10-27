//! Re-entry times (`character_instance_time`): when a character may enter an instance zone again.
// Java: managers/InstanceManager.java (getInstanceTime, getAllInstanceTimes, setInstanceTime,
//       deleteInstanceTime, restoreInstanceTimes)
// Java: dist/game/data/scripts/handlers/chat/commands/user/InstanceZone.java (`/instancezone`)
//
// Java reads a character's rows the first time something asks for them and keeps them in
// `_playerTimes`. Here the session reads them at `CharacterSelect` (D-15: no database in a tick,
// `PlayerProfile::instance_times`) and the world takes them when the character enters. The rows
// that are over are deleted then, as `restoreInstanceTimes` does. Every change is written at
// once through the outbox ([`InstanceTimeWrite`]). The rows are the character's own, so they are
// written side by side with the Java game server as usual (D-53).

use std::collections::BTreeMap;

use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{SystemMessage, SystemMessageParam};
use l2_util::ObjectId;

use crate::world::World;

/// User command 114 (`InstanceZone.java`, `/instancezone`).
pub const INSTANCE_ZONE_COMMAND: i32 = 114;

/// A `character_instance_time` write with Java's statement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstanceTimeWrite {
    /// `INSERT INTO character_instance_time (charId,instanceId,time) values (?,?,?) ON DUPLICATE
    /// KEY UPDATE time=?`.
    Set {
        char_id: i32,
        instance_id: i32,
        time: i64,
    },
    /// `DELETE FROM character_instance_time WHERE charId=? AND instanceId=?`.
    Delete { char_id: i32, instance_id: i32 },
}

impl World {
    /// Takes the rows the session read (`restoreInstanceTimes`): the ones that are over are
    /// deleted, the rest kept.
    pub(crate) fn instance_times_on_enter(&mut self, id: ObjectId) {
        let Some(rows) = self
            .players
            .get_mut(&id)
            .map(|p| std::mem::take(&mut p.data.profile.instance_times))
        else {
            return;
        };
        let now = self.instance_now_ms();
        let mut times = BTreeMap::new();
        for (instance_id, time) in rows {
            if time < now {
                self.persist_instance_time(InstanceTimeWrite::Delete {
                    char_id: id.0,
                    instance_id,
                });
            } else {
                times.insert(instance_id, time);
            }
        }
        self.instances.player_times.insert(id, times);
    }

    /// Java `InstanceManager.getInstanceTime(playerObjId, id)`: -1 when there is none.
    pub fn instance_time(&self, player: ObjectId, template_id: i32) -> i64 {
        self.instances
            .player_times
            .get(&player)
            .and_then(|t| t.get(&template_id))
            .copied()
            .unwrap_or(-1)
    }

    /// Java `getAllInstanceTimes(playerObjId)`, by template id.
    pub fn instance_times(&self, player: ObjectId) -> BTreeMap<i32, i64> {
        self.instances
            .player_times
            .get(&player)
            .cloned()
            .unwrap_or_default()
    }

    /// Java `setInstanceTime(playerObjId, id, time)`.
    // NOTE: Java puts the time into its map only after the statement succeeded; here the world
    // does not wait for the database (D-15), so the map has it at once.
    pub fn set_instance_time(&mut self, player: ObjectId, template_id: i32, time: i64) {
        self.persist_instance_time(InstanceTimeWrite::Set {
            char_id: player.0,
            instance_id: template_id,
            time,
        });
        self.instances
            .player_times
            .entry(player)
            .or_default()
            .insert(template_id, time);
    }

    /// Java `deleteInstanceTime(playerObjId, id)`.
    pub fn delete_instance_time(&mut self, player: ObjectId, template_id: i32) {
        self.persist_instance_time(InstanceTimeWrite::Delete {
            char_id: player.0,
            instance_id: template_id,
        });
        if let Some(times) = self.instances.player_times.get_mut(&player) {
            times.remove(&template_id);
        }
    }

    /// Java `InstanceZone.onCommand` (`/instancezone`): the instance the player is allowed in,
    /// and the time left of every re-entry restriction over a minute (shorter ones are deleted).
    pub(crate) fn user_command_instance_zone(&mut self, id: ObjectId) {
        if let Some(world) = self.player_instance_world(id)
            && world.template_id >= 0
        {
            let template_id = world.template_id;
            self.send(
                id,
                &SystemMessage::new(SystemMessageId::InstantZoneCurrentlyInUseS1)
                    .with(SystemMessageParam::InstanceName(template_id)),
            );
        }
        let now = self.instance_now_ms();
        let mut first_message = true;
        for (template_id, time) in self.instance_times(id) {
            let remaining = (time - now) / 1000;
            if remaining > 60 {
                if first_message {
                    first_message = false;
                    self.send(id, &SystemMessage::new(SystemMessageId::InstanceZoneTimeLimit));
                }
                let hours = (remaining / 3600) as i32;
                let minutes = ((remaining % 3600) / 60) as i32;
                self.send(
                    id,
                    &SystemMessage::new(SystemMessageId::S1WillBeAvailableForReUseAfterS2HourSS3MinuteS)
                        .with(SystemMessageParam::InstanceName(template_id))
                        .with(SystemMessageParam::IntNumber(hours))
                        .with(SystemMessageParam::IntNumber(minutes)),
                );
            } else {
                self.delete_instance_time(id, template_id);
            }
        }
        if first_message {
            self.send(
                id,
                &SystemMessage::new(SystemMessageId::ThereIsNoInstanceZoneUnderATimeLimit),
            );
        }
    }
}
