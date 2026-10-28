//! What the instance scripts (S-INST-01) call and the world did not offer `pub` yet: small
//! wrappers over the world's own state, one Java call each.
// Java: entity/actor/Npc.java, entity/WorldObject.java, entity/spawns/Spawn.java

use l2_util::ObjectId;

use crate::world::World;

impl World {
    /// Java `WorldObject.isInvisible()`.
    pub fn is_object_invisible(&self, id: ObjectId) -> bool {
        self.objects.get(&id).is_some_and(|e| e.invisible)
    }

    /// `PlayerConfig.MAX_OFFSET_ON_TELEPORT`, what `Quest.teleportPlayer(.., true)` randomizes by.
    pub fn max_offset_on_teleport(&self) -> i32 {
        self.settings.teleport.max_offset_on_teleport
    }
}
