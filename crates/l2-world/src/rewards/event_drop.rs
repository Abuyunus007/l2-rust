//! `EventDropManager`: the extra drops an event adds to every kill (M8-MISC-01).
// Java: managers/EventDropManager.java
// Java: entity/actor/holders/npc/EventDropHolder.java (and `DropHolder`)
// Java: entity/actor/Attackable.java (calculateRewards: the two `doEventDrop` calls)
//
// The events themselves are scripts (`LongTimeEvent`, D-05): they register their list with
// [`World::add_event_drops`] when they start and take it back with [`World::remove_event_drops`]
// when they end. Until the script milestone nothing registers a list, so a kill rolls nothing
// extra, as in Java without an event.

use std::collections::{BTreeMap, BTreeSet};

use l2_util::ObjectId;

use crate::formulas::rng::Rnd;
use crate::world::World;

/// Java `EventDropHolder`: one extra drop of an event.
#[derive(Debug, Clone, PartialEq)]
pub struct EventDrop {
    /// `getItemId()`.
    pub item_id: i32,
    /// `getMin()`.
    pub min: i64,
    /// `getMax()`.
    pub max: i64,
    /// `getChance()`: percent, rolled with `Rnd.get(100d) < chance`.
    pub chance: f64,
    /// `getMinLevel()`: the lowest monster level.
    pub min_level: i32,
    /// `getMaxLevel()`: the highest monster level.
    pub max_level: i32,
    /// `getMonsterIds()`: empty means every monster.
    pub monster_ids: BTreeSet<i32>,
}

/// Java `EventDropManager.EVENT_DROPS`: the lists by the event that registered them.
///
/// Java keys the map by the `LongTimeEvent` object (a `ConcurrentHashMap`, so no order); a
/// `BTreeMap` by the event's name keeps a seeded run reproducible (D-13).
#[derive(Debug, Clone, Default)]
pub struct EventDropWorld {
    drops: BTreeMap<String, Vec<EventDrop>>,
}

impl EventDropWorld {
    /// Java `isEmpty()`.
    pub fn is_empty(&self) -> bool {
        self.drops.is_empty()
    }
}

impl World {
    /// Java `EventDropManager.addDrops(longTimeEvent, dropList)`: a second call with the same
    /// event replaces its list (`Map.put`).
    pub fn add_event_drops(&mut self, event: &str, drops: Vec<EventDrop>) {
        self.event_drops.drops.insert(event.to_owned(), drops);
    }

    /// Java `EventDropManager.removeDrops(longTimeEvent)`.
    pub fn remove_event_drops(&mut self, event: &str) {
        self.event_drops.drops.remove(event);
    }

    /// Java `EventDropManager.doEventDrop(attacker, attackable)`.
    pub fn do_event_drop(&mut self, attacker: Option<ObjectId>, attackable: ObjectId) {
        if self.event_drops.is_empty() {
            return;
        }
        // Event items drop only for players. (`attackable.isFakePlayer()`: fake players are not
        // ported, so none is one.)
        let Some(player) = attacker.and_then(|a| self.acting_player(a)) else {
            return;
        };
        // Event items drop only within a default 9 level difference.
        let (Some(player_level), Some((monster_id, monster_level))) = (
            self.players.get(&player).map(|p| p.core.level),
            self.npcs.get(&attackable).map(|n| (n.npc_id, n.core.level)),
        ) else {
            return;
        };
        if player_level - monster_level > self.settings.drops.event_item_max_level_difference {
            return;
        }

        // Cloned: the rolls and the item hand-out borrow the world.
        let lists: Vec<Vec<EventDrop>> = self.event_drops.drops.values().cloned().collect();
        for list in lists {
            for drop in list {
                if !drop.monster_ids.is_empty() && !drop.monster_ids.contains(&monster_id) {
                    continue;
                }
                if monster_level < drop.min_level
                    || monster_level > drop.max_level
                    || self.rng().get_double(100.0) >= drop.chance
                {
                    continue;
                }
                let count = self.rng().get_long_range(drop.min, drop.max);
                let ground = &self.settings.ground;
                if ground.auto_loot_item_ids.contains(&drop.item_id)
                    || ground.auto_loot
                    || self.is_flying(attackable)
                {
                    // Give the item to the player that has killed the attackable.
                    self.do_auto_loot(player, attackable, drop.item_id, count);
                } else {
                    // Drop the item on the ground.
                    self.npc_drop_item(attackable, Some(player), drop.item_id, count);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn drop(item_id: i32) -> EventDrop {
        EventDrop {
            item_id,
            min: 1,
            max: 3,
            chance: 100.0,
            min_level: 1,
            max_level: 99,
            monster_ids: BTreeSet::new(),
        }
    }

    #[test]
    fn the_registry_replaces_and_removes_by_event() {
        let mut registry = EventDropWorld::default();
        assert!(registry.is_empty());
        registry.drops.insert("Halloween".into(), vec![drop(1)]);
        registry.drops.insert("Halloween".into(), vec![drop(2), drop(3)]);
        assert_eq!(registry.drops["Halloween"].len(), 2);
        registry.drops.remove("Halloween");
        assert!(registry.is_empty());
    }
}
