//! Java `Quest.addSpawn`: an NPC a script places, with no respawn.
// Java: mechanics/script/Quest.java (addSpawn(summoner, npcId, x, y, z, heading, randomOffset,
//       despawnDelay, isSummonSpawn, instanceId))
// Java: entity/Spawn.java (doSpawn, stopRespawn), entity/actor/Npc.java (scheduleDespawn)

use l2_util::ObjectId;
use tracing::{error, warn};

use crate::world::World;

impl World {
    /// Java `Quest.addSpawn(...)`: a new `Spawn` of the NPC at the place (shifted by 50 to 100
    /// in x and y with `random_offset`), in `instance_id`, that does not respawn; with a
    /// `despawn_delay_ms` above 0 it is deleted after that time.
    ///
    /// The summoner and `isSummonSpawn` (the spawn animation) are not modelled; Java's
    /// `RandomSpawnsConfig.ENABLE_RANDOM_MONSTER_SPAWNS` branch is a custom that is off by default.
    pub fn script_add_spawn(
        &mut self,
        npc_id: i32,
        (x, y, z): (i32, i32, i32),
        heading: i32,
        random_offset: bool,
        despawn_delay_ms: i64,
        instance_id: i32,
    ) -> Option<ObjectId> {
        if x == 0 && y == 0 {
            error!("addSpawn(): invalid spawn coordinates for NPC #{npc_id}!");
            return None;
        }
        let (mut x, mut y) = (x, y);
        if random_offset {
            let mut offset = self.rng().get(50, 100);
            if self.rng().next_bool() {
                offset *= -1;
            }
            x += offset;
            let mut offset = self.rng().get(50, 100);
            if self.rng().next_bool() {
                offset *= -1;
            }
            y += offset;
        }
        let template = self
            .npc_world
            .template(npc_id)
            .filter(|t| crate::npc::NpcClass::from_type(&t.npc_type).is_some())
            .map(|t| t.is_random_walk_enabled());
        let Some(template_random_walk) = template else {
            warn!("Could not spawn NPC #{npc_id}; error: no such template");
            return None;
        };
        let index = self.add_runtime_spawn(l2_data::spawn::NpcSpawn {
            npc_id,
            amount: 1,
            location: l2_data::Location { x, y, z },
            heading,
            respawn_min_ms: 0,
            respawn_max_ms: 0,
            // `spawn.stopRespawn()`.
            respawn_enabled: false,
            chase_range: 0,
            name: None,
            territory: None,
            period_of_day: None,
            ai_data: None,
            group: None,
            source: "script".to_owned(),
        });
        if instance_id != 0 {
            // `spawn.setInstanceId(instanceId)`: the NPC is put into the instance when it is
            // created (`npc.rs`). The spawn's random walk flag is `Npc.onSpawn`'s, the
            // template's: nothing sets it on a script spawn.
            self.instances.spawns.insert(
                index,
                crate::instance::InstanceSpawnInfo {
                    instance_id,
                    random_walk: template_random_walk,
                    on_kill_delay: -1,
                },
            );
        }
        let mut report = crate::npc::SpawnReport::default();
        let Some(npc) = self.initialize_npc(index, None, &mut report) else {
            warn!("Could not spawn NPC #{npc_id}");
            return None;
        };
        if despawn_delay_ms > 0 {
            self.schedule_script_despawn(npc, despawn_delay_ms);
        }
        Some(npc)
    }
}
