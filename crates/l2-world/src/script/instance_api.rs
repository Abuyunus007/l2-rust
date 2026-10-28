//! What the instance scripts (S-INST-01) call and the world did not offer `pub` yet: small
//! wrappers over the world's own state, one Java call each.
// Java: entity/actor/Npc.java, entity/actor/Attackable.java, entity/WorldObject.java,
//       entity/spawns/Spawn.java, entity/instancezone/Instance.java

use l2_util::ObjectId;

use crate::effects::SkillFinishType;
use crate::effects::list::ListKind;
use crate::respawn::RUNTIME_SPAWN_BASE;
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

    /// Java `npc.getSpawn()` as the index of the spawn in the world's spawn list.
    pub fn npc_spawn_index(&self, npc: ObjectId) -> Option<usize> {
        self.npcs.get(&npc).and_then(|n| n.spawn)
    }

    /// Java `spawn.setRespawnDelay(delay)` (seconds, no random interval).
    pub fn set_spawn_respawn_delay(&mut self, index: usize, delay_s: i32) {
        let Some(base) = index.checked_sub(RUNTIME_SPAWN_BASE) else {
            return;
        };
        if let Some(spawn) = self.death.runtime_spawns.get_mut(base) {
            let ms = if delay_s != 0 { delay_s.max(1).saturating_mul(1000) } else { 0 };
            spawn.respawn_min_ms = ms;
            spawn.respawn_max_ms = ms;
        }
    }

    /// Java `spawn.startRespawn()`.
    pub fn start_spawn_respawn(&mut self, index: usize) {
        self.start_respawn(index);
    }

    /// Java `spawn.stopRespawn()`.
    pub fn stop_spawn_respawn(&mut self, index: usize) {
        self.stop_respawn(index);
    }

    /// Java `npc.setRandomWalking(enabled)` of an instance NPC: the flag of its spawn.
    pub fn set_npc_random_walking(&mut self, npc: ObjectId, enabled: bool) {
        let Some(spawn) = self.npcs.get(&npc).and_then(|n| n.spawn) else {
            return;
        };
        if let Some(info) = self.instances.spawns.get_mut(&spawn) {
            info.random_walk = enabled;
        }
    }

    /// Kamaloka `removeBuffs(creature)` for one creature: every buff, triggered buff, toggle and
    /// debuff that is not `stayAfterDeath` and whose skill is not in `keep` goes (Java's
    /// `EffectList.forEach(function, false)` leaves the dances out).
    pub fn remove_buffs_except(&mut self, owner: ObjectId, keep: &[i32]) {
        let doomed: Vec<(ObjectId, i32)> = self
            .effect_list(owner)
            .map(|list| {
                [ListKind::Buffs, ListKind::Triggered, ListKind::Toggles, ListKind::Debuffs]
                    .into_iter()
                    .flat_map(|kind| list.queue(kind).iter())
                    .filter(|info| !info.facts.stay_after_death && !keep.contains(&info.skill().id))
                    .map(|info| (info.effected, info.skill().id))
                    .collect()
            })
            .unwrap_or_default();
        for (effected, skill_id) in doomed {
            self.stop_skill_effects(effected, SkillFinishType::Removed, skill_id);
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
