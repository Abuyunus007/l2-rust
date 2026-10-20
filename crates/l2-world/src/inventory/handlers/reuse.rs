//! Item reuse: the stamps of items that were used, the shared reuse groups, and what the client is
//! told about them.
// Java: entity/actor/Creature.java (addTimeStampItem, getItemRemainingReuseTime,
//       getReuseDelayOnGroup)
// Java: entity/actor/holders/creature/TimeStamp.java (the item constructor, getRemaining)
// Java: network/clientpackets/UseItem.java (reuseData, sendSharedGroupUpdate and the reuse check)
// Java: network/serverpackets/ExUseSharedGroupItem.java
// Java: entity/actor/Player.java (storeItemReuseDelay, restoreItemReuse), M5-FIX-03
//
// # Time
// Like the skill stamps (`skills/reuse.rs`), item stamps use the world clock (`World::world_ms`)
// instead of `System.currentTimeMillis()`. What the client sees is the time left, which is the
// same on both clocks.

use l2_data::item::ItemTemplate;
use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{ExUseSharedGroupItem, SystemMessage, SystemMessageParam};
use l2_util::ObjectId;

use crate::inventory::item_name_param;
use crate::player::ItemData;
use crate::world::World;

/// Java `TimeStamp` of an item. `stamp_ms` is on the world clock; 0 means "passed".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ItemStamp {
    object_id: ObjectId,
    /// `getItemId()`.
    item_id: i32,
    /// `getReuse()`: the full delay in milliseconds.
    reuse_ms: i64,
    /// `getSharedReuseGroup()` of the item when the stamp was made.
    group: i32,
    stamp_ms: i64,
}

impl ItemStamp {
    /// `getRemaining()` at `now`.
    fn remaining(&self, now: i64) -> i64 {
        if self.stamp_ms == 0 {
            return 0;
        }
        (self.stamp_ms - now).max(0)
    }
}

/// One row of `character_item_reuse_save` as the world keeps it (Java `storeItemReuseDelay`
/// writes `TimeStamp.getItemId/getItemObjectId/getReuse/getStamp`). `systime` is on the wall clock,
/// like the column.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SavedItemReuse {
    /// `itemId`.
    pub item_id: i32,
    /// `itemObjId`.
    pub item_obj_id: i32,
    /// `reuseDelay`: the full delay in milliseconds.
    pub reuse_delay: i64,
    /// `systime`: the wall clock millisecond at which the timer ends.
    pub systime: i64,
}

/// The "re-use time" message of `UseItem.reuseData` and `ItemSkillsTemplate.checkReuse`: `name`
/// is the item (or the skill) the message is about.
pub(super) fn reuse_message(name: SystemMessageParam, remaining_ms: i64) -> SystemMessage {
    let hours = (remaining_ms / 3_600_000) as i32;
    let minutes = ((remaining_ms % 3_600_000) as i32) / 60_000;
    let seconds = ((remaining_ms / 1000) % 60) as i32;
    let message = if hours > 0 {
        SystemMessage::new(SystemMessageId::ThereAreS2HourSS3MinuteSAndS4SecondSRemainingInS1SReUseTime)
            .with(name)
            .with(SystemMessageParam::IntNumber(hours))
            .with(SystemMessageParam::IntNumber(minutes))
    } else if minutes > 0 {
        SystemMessage::new(SystemMessageId::ThereAreS2MinuteSS3SecondSRemainingInS1SReUseTime)
            .with(name)
            .with(SystemMessageParam::IntNumber(minutes))
    } else {
        SystemMessage::new(SystemMessageId::ThereAreS2SecondSRemainingInS1SReUseTime).with(name)
    };
    message.with(SystemMessageParam::IntNumber(seconds))
}

impl World {
    /// Java `Creature.addTimeStampItem(item, reuse)`: a known item keeps its place.
    pub(crate) fn add_time_stamp_item(
        &mut self,
        id: ObjectId,
        item: &ItemData,
        template: &ItemTemplate,
        reuse_ms: i64,
    ) {
        let now = self.world_ms();
        let stamp = ItemStamp {
            object_id: item.object_id,
            item_id: item.item_id,
            reuse_ms,
            group: template.shared_reuse_group,
            stamp_ms: if reuse_ms != 0 { now + reuse_ms } else { 0 },
        };
        self.put_item_stamp(id, stamp);
    }

    /// `_reuseTimeStampsItems.put(objectId, stamp)`: a known item keeps its place.
    fn put_item_stamp(&mut self, id: ObjectId, stamp: ItemStamp) {
        let stamps = &mut self.cast_state_mut(id).reuse.item_stamps;
        match stamps.iter_mut().find(|s| s.object_id == stamp.object_id) {
            Some(existing) => *existing = stamp,
            None => stamps.push(stamp),
        }
    }

    /// Java `Player.storeItemReuseDelay`: the rows the item reuse stamps are saved as, the ones
    /// that still have time left (`currentTime < ts.getStamp()`). The world clock stamp becomes
    /// the wall clock one Java keeps in `systime`.
    pub(crate) fn saved_item_reuse(&self, id: ObjectId) -> Vec<SavedItemReuse> {
        let now = self.world_ms();
        let now_wall = crate::world::unix_ms_now();
        let Some(state) = self.cast_state(id) else {
            return Vec::new();
        };
        state
            .reuse
            .item_stamps
            .iter()
            .filter(|s| now < s.stamp_ms)
            .map(|s| SavedItemReuse {
                item_id: s.item_id,
                item_obj_id: s.object_id.0,
                reuse_delay: s.reuse_ms,
                systime: now_wall + (s.stamp_ms - now),
            })
            .collect()
    }

    /// Java `Player.restoreItemReuse`: the rows of the last logout become stamps again. Called
    /// once when the character enters; the rows are taken from the profile.
    // NOTE(java-quirk): the item is looked up by its template id, not by the saved object id, so
    // two saved stamps of one template (two objects) both land on the first such item, the later
    // row replacing the earlier.
    // The warehouse fallback of Java (`getWarehouse().getItemByItemId`) is left out: the world
    // does not hold the warehouse, so a timer of an item stored there is dropped.
    pub(crate) fn restore_item_reuse(&mut self, id: ObjectId) {
        let Some(player) = self.players.get_mut(&id) else {
            return;
        };
        let rows = std::mem::take(&mut player.data.profile.item_reuse_to_restore);
        if rows.is_empty() {
            return;
        }
        let Some(templates) = self.item_templates() else {
            return;
        };
        let now_wall = crate::world::unix_ms_now();
        let now = self.world_ms();
        for row in rows {
            let item = self.players.get(&id).and_then(|p| {
                p.data
                    .items
                    .iter()
                    .find(|i| i.item_id == row.item_id && (i.loc == "INVENTORY" || i.loc == "PAPERDOLL"))
                    .cloned()
            });
            let Some(item) = item else {
                continue;
            };
            let Some(template) = templates.get(item.item_id) else {
                continue;
            };
            if template.reuse_delay <= 0 {
                continue;
            }
            let remaining = row.systime - now_wall;
            if remaining <= 10 {
                continue;
            }
            // `addTimeStampItem(item, reuseDelay, systime)`: the stamp ends `remaining` from now.
            self.put_item_stamp(
                id,
                ItemStamp {
                    object_id: item.object_id,
                    item_id: item.item_id,
                    reuse_ms: row.reuse_delay,
                    group: template.shared_reuse_group,
                    stamp_ms: now + remaining,
                },
            );
            if template.etc().is_some() {
                // `ExUseSharedGroupItem` (nothing is sent when the group is 0).
                self.send_shared_group_update(
                    id,
                    item.item_id,
                    template.shared_reuse_group,
                    remaining,
                    i32::try_from(row.reuse_delay).unwrap_or(i32::MAX),
                );
            }
        }
    }

    /// Java `Creature.getItemRemainingReuseTime(itemObjId)`: milliseconds left, -1 without a stamp.
    pub(crate) fn item_remaining_reuse_time(&self, id: ObjectId, object_id: ObjectId) -> i64 {
        let now = self.world_ms();
        self.cast_state(id)
            .and_then(|c| c.reuse.item_stamps.iter().find(|s| s.object_id == object_id))
            .map_or(-1, |s| s.remaining(now))
    }

    /// Java `Creature.getReuseDelayOnGroup(group)`: the time left of the first stamp of the
    /// group that has not passed, -1 when none.
    // NOTE: Java walks a `ConcurrentHashMap` keyed by object id; here the stamps are in the
    // order they were made. Items of one group share their delay, so it only matters for the
    // few that do not.
    pub(crate) fn reuse_delay_on_group(&self, id: ObjectId, group: i32) -> i64 {
        if group > 0 {
            let now = self.world_ms();
            let found = self.cast_state(id).and_then(|c| {
                c.reuse
                    .item_stamps
                    .iter()
                    .find(|s| s.group == group && now < s.stamp_ms)
            });
            if let Some(stamp) = found {
                return (stamp.stamp_ms - now).max(0);
            }
        }
        -1
    }

    /// Java `UseItem.sendSharedGroupUpdate(player, group, remaining, reuse)`.
    pub(crate) fn send_shared_group_update(
        &self,
        id: ObjectId,
        item_id: i32,
        group: i32,
        remaining_ms: i64,
        reuse_ms: i32,
    ) {
        if group > 0 {
            self.send(
                id,
                &ExUseSharedGroupItem::new(item_id, group, remaining_ms, reuse_ms),
            );
        }
    }

    /// The reuse part of `UseItem.runImpl`: `true` when the item (or its shared group) is still in
    /// reuse. The player is told, and the client's group timer is refreshed.
    pub(crate) fn item_reuse_blocks_use(
        &mut self,
        id: ObjectId,
        item: &ItemData,
        template: &ItemTemplate,
    ) -> bool {
        let reuse_delay = template.reuse_delay;
        let group = template.shared_reuse_group;
        if reuse_delay <= 0 {
            return false;
        }
        let templates = self.item_templates();
        let name = item_name_param(templates.as_deref(), item.item_id);

        let reuse = self.item_remaining_reuse_time(id, item.object_id);
        if reuse > 0 {
            self.send(id, &reuse_message(name, reuse));
            self.send_shared_group_update(id, item.item_id, group, reuse, reuse_delay);
            return true;
        }
        let reuse_on_group = self.reuse_delay_on_group(id, group);
        if reuse_on_group > 0 {
            self.send(id, &reuse_message(name, reuse_on_group));
            self.send_shared_group_update(id, item.item_id, group, reuse_on_group, reuse_delay);
            return true;
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ints(message: &SystemMessage) -> Vec<i32> {
        message
            .params
            .iter()
            .filter_map(|p| match p {
                SystemMessageParam::IntNumber(n) => Some(*n),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn the_message_depends_on_the_largest_unit() {
        let name = SystemMessageParam::ItemName(1060);
        let hours = reuse_message(name.clone(), 3_600_000 + 2 * 60_000 + 5_000);
        assert_eq!(
            hours.id,
            SystemMessageId::ThereAreS2HourSS3MinuteSAndS4SecondSRemainingInS1SReUseTime
        );
        assert_eq!(ints(&hours), vec![1, 2, 5]);
        let minutes = reuse_message(name.clone(), 90_000);
        assert_eq!(
            minutes.id,
            SystemMessageId::ThereAreS2MinuteSS3SecondSRemainingInS1SReUseTime
        );
        assert_eq!(ints(&minutes), vec![1, 30]);
        let seconds = reuse_message(name, 9_500);
        assert_eq!(
            seconds.id,
            SystemMessageId::ThereAreS2SecondSRemainingInS1SReUseTime
        );
        assert_eq!(ints(&seconds), vec![9]);
    }

    #[test]
    fn a_passed_stamp_has_nothing_left() {
        let stamp = ItemStamp {
            object_id: ObjectId(1),
            item_id: 0,
            reuse_ms: 0,
            group: 0,
            stamp_ms: 5_000,
        };
        assert_eq!(stamp.remaining(4_000), 1_000);
        assert_eq!(stamp.remaining(6_000), 0);
    }
}
