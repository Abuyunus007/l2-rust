//! The spawn table reads the area AI scripts of S-AI-03 need (`SelMahumDrill`).
// Java: entity/spawns/Spawn.java (getName, getLastSpawn), data/SpawnTable.java (getSpawns)

use l2_util::ObjectId;

use crate::world::World;

/// One row of `ScriptCtx::spawns_named`: a spawn of the spawn table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NamedSpawn {
    /// Java `spawn.getLastSpawn()`.
    pub last_spawn: Option<ObjectId>,
    /// Java `spawn.getX()`, `getY()`, `getZ()`.
    pub location: (i32, i32, i32),
    /// Java `spawn.getHeading()`.
    pub heading: i32,
}

impl World {
    /// Java `spawn.getName()` of spawn `index` (`None` for a spawn without a name).
    pub(crate) fn spawn_def_name(&self, index: usize) -> Option<String> {
        self.spawn_def(index).and_then(|s| s.name.clone())
    }

    /// Java `SpawnTable.getInstance().getSpawns(npcId)` filtered by the name prefix.
    pub(crate) fn named_spawns_of(&self, npc_id: i32, prefix: &str) -> Vec<NamedSpawn> {
        let Some(data) = self.npc_world.spawn_data() else {
            return Vec::new();
        };
        data.spawns()
            .iter()
            .enumerate()
            .filter(|(_, s)| s.npc_id == npc_id && s.name.as_deref().is_some_and(|n| n.starts_with(prefix)))
            .map(|(index, s)| NamedSpawn {
                last_spawn: self.last_spawn(index),
                location: (s.location.x, s.location.y, s.location.z),
                heading: s.heading,
            })
            .collect()
    }
}
