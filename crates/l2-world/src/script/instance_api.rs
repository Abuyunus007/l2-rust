//! What the instance scripts (S-INST-01) call and the world did not offer `pub` yet: small
//! wrappers over the world's own state, one Java call each.
// Java: entity/actor/Npc.java, entity/actor/Attackable.java, entity/WorldObject.java,
//       entity/spawns/Spawn.java, entity/instancezone/Instance.java

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

    /// Java `Attackable.clearAggroList()`.
    pub fn clear_aggro_list(&mut self, npc: ObjectId) {
        self.aggro_clear(npc);
    }

    /// Java `Creature.disableCoreAI(value)`.
    pub fn set_core_ai_disabled(&mut self, npc: ObjectId, value: bool) {
        self.disable_core_ai(npc, value);
    }

    /// Java `Creature.setRunning()` of an NPC.
    pub fn set_npc_running(&mut self, npc: ObjectId) {
        self.set_running(npc, true);
    }

    /// Java `npc.getAI().setIntentionMoveTo(location)`.
    pub fn npc_move_to_location(&mut self, npc: ObjectId, x: i32, y: i32, z: i32) {
        self.npc_set_intention_move_to(npc, x, y, z);
    }

    /// `(getCurrentHp(), getMaxHp())` of a creature.
    pub fn creature_hp(&self, id: ObjectId) -> Option<(f64, f64)> {
        self.hp_and_max(id)
    }

    /// Java `npc.isMonster()`.
    pub fn is_monster_npc(&self, id: ObjectId) -> bool {
        self.npcs.get(&id).is_some_and(|n| n.class.is_monster())
    }

    /// Java `npc.getSpawn().setLocation(location)` as far as an NPC's way home goes
    /// (`Npc::spawn_point`, what `returnHome` walks to).
    pub fn set_npc_spawn_point(&mut self, npc: ObjectId, x: i32, y: i32, z: i32) {
        if let Some(n) = self.npcs.get_mut(&npc) {
            n.spawn_point = (x, y, z);
        }
    }

    /// What a script does to a pet it takes out of an instance (Java `PailakaDevilsLegacy.
    /// onExitZone`): `stopFollow`, `abortAttack`, `abortCast`, `stopAITask`, `setTarget(null)`,
    /// `decayMe()` and `owner.setPet(null)`. The summon is not stored (not `unSummon`).
    pub fn discard_summon(&mut self, owner: ObjectId, summon: ObjectId) {
        if !self.is_dead(summon) {
            self.stop_follow(summon);
            self.abort_attack(summon);
            self.abort_cast(summon);
            self.stop_summon_ai_task(summon);
            self.set_creature_target(summon, None);
        }
        self.decay(summon);
        self.set_pet(owner, None);
    }
}
