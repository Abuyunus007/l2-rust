//! What Java keeps on an `Npc` for the scripts: the weapon of the killing blow and the NPCs it
//! summoned with `addSpawn(summoner, ...)` (S-DATA-01).
// Java: entity/actor/Npc.java (_killingBlowWeaponId, _summonedNpcs, addSummonedNpc,
//       removeSummonedNpc, getSummonedNpcCount, onDecay, onSpawn, doDie),
//       entity/actor/Creature.java (getSummoner), entity/spawns/Spawn.java (doSpawn),
//       mechanics/script/Quest.java (addSpawn with a summoner)

use std::collections::{BTreeSet, HashMap};

use l2_util::ObjectId;

use super::ScriptNpc;
use super::ctx::ScriptCtx;
use crate::world::World;

/// The per-NPC data, by object id (D-10: ids, not references).
#[derive(Debug, Default)]
pub(crate) struct NpcLinks {
    /// `_summonedNpcs`: the NPCs each summoner made and that are not decayed yet.
    summoned: HashMap<ObjectId, BTreeSet<ObjectId>>,
    /// `getSummoner()` of an NPC.
    summoner: HashMap<ObjectId, ObjectId>,
    /// `_killingBlowWeaponId`.
    killing_blow: HashMap<ObjectId, i32>,
}

impl World {
    /// Java `Npc.doDie`: `_killingBlowWeaponId` is the active weapon item of the killer, 0 for
    /// none.
    pub(crate) fn script_record_killing_blow(&mut self, npc: ObjectId, killer: Option<ObjectId>) {
        let weapon = killer
            .and_then(|k| self.stats(k))
            .and_then(|s| s.active_weapon_item())
            .map_or(0, |t| t.id);
        self.scripts.npc_links.killing_blow.insert(npc, weapon);
    }

    /// Java `Npc.onSpawn` (`_killingBlowWeaponId = 0`) and `Spawn.doSpawn` (`setSummoner(null)`,
    /// `resetSummonedNpcs()`).
    pub(crate) fn script_reset_npc_links(&mut self, npc: ObjectId) {
        let links = &mut self.scripts.npc_links;
        links.killing_blow.remove(&npc);
        links.summoner.remove(&npc);
        links.summoned.remove(&npc);
    }

    /// Java `Npc.onDecay`: the NPC leaves its summoner's list.
    pub(crate) fn script_unlink_summoned(&mut self, npc: ObjectId) {
        let links = &mut self.scripts.npc_links;
        if let Some(summoner) = links.summoner.get(&npc).copied()
            && let Some(set) = links.summoned.get_mut(&summoner)
        {
            set.remove(&npc);
        }
    }

    /// Java `creature.setSummoner(summoner)` of a creature whose summoner is no NPC (a decoy's
    /// player): only `getSummoner()` answers it.
    pub(crate) fn set_npc_summoner(&mut self, npc: ObjectId, summoner: ObjectId) {
        self.scripts.npc_links.summoner.insert(npc, summoner);
    }

    /// Java `Npc.addSummonedNpc(npc)`: also `npc.setSummoner(this)`.
    pub fn add_summoned_npc(&mut self, summoner: ObjectId, npc: ObjectId) {
        let links = &mut self.scripts.npc_links;
        links.summoned.entry(summoner).or_default().insert(npc);
        links.summoner.insert(npc, summoner);
    }

    /// Java `Npc.getSummonedNpcCount()`.
    pub fn summoned_npc_count(&self, summoner: ObjectId) -> usize {
        self.scripts
            .npc_links
            .summoned
            .get(&summoner)
            .map_or(0, BTreeSet::len)
    }

    /// Java `Npc.getKillingBlowWeapon()`: the item id, 0 for none (and for an NPC that is alive).
    pub fn killing_blow_weapon(&self, npc: ObjectId) -> i32 {
        self.scripts
            .npc_links
            .killing_blow
            .get(&npc)
            .copied()
            .unwrap_or(0)
    }

    /// Java `Creature.getSummoner()` of an NPC made with `addSpawn(summoner, ...)`.
    pub fn summoner_of(&self, npc: ObjectId) -> Option<ObjectId> {
        self.scripts.npc_links.summoner.get(&npc).copied()
    }
}

impl ScriptCtx<'_> {
    /// Java `npc.getKillingBlowWeapon()`.
    pub fn killing_blow_weapon(&self, npc: ScriptNpc) -> i32 {
        self.world().killing_blow_weapon(npc.object_id)
    }

    /// Java `npc.getSummonedNpcCount()`.
    pub fn summoned_npc_count(&self, npc: ScriptNpc) -> i32 {
        i32::try_from(self.world().summoned_npc_count(npc.object_id)).unwrap_or(i32::MAX)
    }

    /// Java `npc.getSummoner()`.
    pub fn summoner(&self, npc: ScriptNpc) -> Option<ObjectId> {
        self.world().summoner_of(npc.object_id)
    }

    /// Java `addSpawn(summoner, npcId, x, y, z, heading, randomOffset, despawnDelay, ...)`: the
    /// new NPC joins the summoner's list and gets it as its summoner.
    #[allow(clippy::too_many_arguments)]
    pub fn add_spawn_summoned(
        &mut self,
        summoner: ScriptNpc,
        npc_id: i32,
        x: i32,
        y: i32,
        z: i32,
        heading: i32,
        random_offset: bool,
        despawn_delay_ms: i64,
        instance_id: i32,
    ) -> Option<ScriptNpc> {
        let npc = self.add_spawn(
            npc_id,
            x,
            y,
            z,
            heading,
            random_offset,
            despawn_delay_ms,
            instance_id,
        )?;
        self.world_mut()
            .add_summoned_npc(summoner.object_id, npc.object_id);
        Some(npc)
    }
}
