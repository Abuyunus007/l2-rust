//! `CreatureSeeTaskManager`: the NPCs of `add_creature_see_id` look around once a second and tell
//! their scripts about every creature they did not see before (S-API-03).
// Java: taskmanagers/CreatureSeeTaskManager.java
// Java: entity/actor/Creature.java (initSeenCreatures, updateSeenCreatures, removeSeenCreature,
//       deleteMe), entity/actor/Npc.java (onSpawn: CREATURE_SEE_IDS), ai/CreatureAI.java
//       (notifyActionForgetObject)

use std::collections::{BTreeMap, HashSet};

use l2_util::ObjectId;

use super::setup::EventKind;
use crate::timer::TimerWheel;
use crate::world::World;

/// Java `CreatureSeeTaskManager` runs every 1000 ms.
const SEE_INTERVAL_MS: u64 = 1000;

/// Java `Creature._seenCreatures` and `_seenCreatureRange` of one NPC.
#[derive(Debug, Default)]
pub(crate) struct SeenCreatures {
    range: i32,
    seen: HashSet<ObjectId>,
}

/// The NPCs that look around, by object id (sorted: the order of Java's `ConcurrentHashMap` set
/// is not defined).
#[derive(Debug, Default)]
pub(crate) struct CreatureSee {
    npcs: BTreeMap<ObjectId, SeenCreatures>,
}

impl CreatureSee {
    /// The number of NPCs looking around (tests).
    pub(crate) fn len(&self) -> usize {
        self.npcs.len()
    }
}

impl World {
    /// Java `Npc.onSpawn`: `if (CREATURE_SEE_IDS.contains(getId())) initSeenCreatures()`.
    pub(crate) fn script_init_seen_creatures(&mut self, npc: ObjectId) {
        let Some(n) = self.npcs.get(&npc) else {
            return;
        };
        if !self.scripts.listeners.has(EventKind::CreatureSee, n.npc_id) {
            return;
        }
        // `_seenCreatureRange = PlayerConfig.ALT_PARTY_RANGE`, the aggro range when it has one.
        let aggro = self.npc_world.template(n.npc_id).map_or(0, |t| t.aggro_range);
        let range = if aggro > 0 {
            aggro
        } else {
            self.settings.party.alt_party_range
        };
        let entry = self.scripts.see.npcs.entry(npc).or_default();
        entry.range = range;
    }

    /// Java `Creature.deleteMe`: "Forget all seen creatures."
    pub(crate) fn script_forget_all_seen(&mut self, npc: ObjectId) {
        self.scripts.see.npcs.remove(&npc);
    }

    /// Java `CreatureAI.notifyActionForgetObject` -> `removeSeenCreature`: a creature that left
    /// the NPC's view is new to it when it comes back.
    pub(crate) fn script_forget_seen(&mut self, npc: ObjectId, creature: ObjectId) {
        if let Some(entry) = self.scripts.see.npcs.get_mut(&npc) {
            entry.seen.remove(&creature);
        }
    }

    /// The task: runs every second of the world (called from the tick).
    pub(crate) fn update_seen_creatures(&mut self) {
        if self.scripts.see.npcs.is_empty()
            || self.tick % TimerWheel::ms_to_ticks(SEE_INTERVAL_MS).max(1) != 0
        {
            return;
        }
        let ids: Vec<ObjectId> = self.scripts.see.npcs.keys().copied().collect();
        for npc in ids {
            // `updateSeenCreatures()`: not for the dead, the unspawned, or a sleeping region.
            if self.is_dead(npc)
                || !self.objects.get(&npc).is_some_and(|e| e.spawned)
                || !self.are_region_neighbors_active(npc)
            {
                continue;
            }
            let Some(range) = self.scripts.see.npcs.get(&npc).map(|e| e.range) else {
                continue;
            };
            for creature in self.visible_objects_in_range(npc, range) {
                if !self.objects.get(&creature).is_some_and(|e| e.kind.is_creature()) {
                    continue;
                }
                if self.objects.get(&creature).is_some_and(|e| e.invisible) {
                    continue;
                }
                let new = self
                    .scripts
                    .see
                    .npcs
                    .get_mut(&npc)
                    .is_some_and(|e| e.seen.insert(creature));
                if new {
                    self.script_on_creature_see(npc, creature);
                }
            }
        }
    }

    /// The number of NPCs that look around (tests).
    pub fn creature_see_count(&self) -> usize {
        self.scripts.see.len()
    }
}
