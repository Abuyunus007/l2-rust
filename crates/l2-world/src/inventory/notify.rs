//! What Java sends around an inventory change: `broadcastUserInfo`, `broadcastModifiedStats`,
//! the delayed `sendSkillList` / `sendItemList`, and `refreshWeight`.
// Java: entity/actor/Player.java (broadcastUserInfo, updateUserInfo, broadcastCharInfo,
//       sendSkillList, sendItemList, refreshOverloaded)
// Java: entity/actor/Creature.java (broadcastModifiedStats)

use l2_data::Stat;
use l2_protocol_game::server::enterworld::{SkillList, SkillListEntry};
use l2_protocol_game::server::visibility::status;
use l2_protocol_game::server::{ExQuestItemList, StatusUpdate};
use l2_util::ObjectId;

use crate::enter::{item_list, quest_item_list};
use crate::timer::{TimerKind, TimerWheel};
use crate::world::{World, unix_ms_now};

/// `Player.broadcastCharInfo` waits 100 ms.
const CHAR_INFO_DELAY_MS: u64 = 100;
/// `Player.sendSkillList` waits 300 ms.
const SKILL_LIST_DELAY_MS: u64 = 300;
/// `Player.sendItemList` waits 250 ms.
const ITEM_LIST_DELAY_MS: u64 = 250;

impl World {
    /// Java `Player.broadcastUserInfo()`: `UserInfo` and `ExBrExtraUserInfo` to the player now, the
    /// `CharInfo` of everybody who sees the player 100 ms later (one pending broadcast at a time).
    pub(crate) fn broadcast_user_info(&mut self, id: ObjectId) {
        // Send user info to the current player.
        self.send_user_info(id);
        // Broadcast char info to known players.
        self.broadcast_char_info(id);
    }

    /// Java `Player.broadcastCharInfo()`.
    fn broadcast_char_info(&mut self, id: ObjectId) {
        let due = self.tick + TimerWheel::ms_to_ticks(CHAR_INFO_DELAY_MS);
        let Some(player) = self.players.get(&id) else {
            return;
        };
        if player.inv_tasks.char_info.is_some() {
            return;
        }
        let timer = self.timers.schedule_at(due, TimerKind::CharInfo(id));
        if let Some(player) = self.players.get_mut(&id) {
            player.inv_tasks.char_info = Some(timer);
        }
    }

    /// The task of `broadcastCharInfo`: `CharInfo` (and the relation) to every viewer.
    pub(crate) fn on_char_info_timer(&mut self, id: ObjectId) {
        let Some(player) = self.players.get_mut(&id) else {
            return;
        };
        player.inv_tasks.char_info = None;
        // Java's task sends the `CharInfo` (and the relation) to everyone who sees the player;
        // unlike `sendInfo` it has no second `CharInfo` for a transformed player.
        for viewer in self.visible_players(id) {
            if self.is_visible_for(id, viewer) {
                self.send_player_info_as(id, viewer, false);
            }
        }
    }

    /// Java `Creature.broadcastModifiedStats(stats)` for a player, called by `addStatFuncs` /
    /// `removeStatsOwner`: `UserInfo` for the player and a `StatusUpdate` with the new attack and
    /// cast speed for everybody, or the whole `broadcastUserInfo` when the movement speed changed.
    // NOTE: Java also runs this for the stat functions of every passive skill an item or armor set
    // brings, once per skill. The stat engine's `add_skill` does not report its stats, so only the
    // item's own functions are broadcast here; the `broadcastUserInfo` that ends every equip
    // sends the final state anyway.
    pub(crate) fn broadcast_modified_stats(&mut self, id: ObjectId, stats: &[Stat]) {
        if !self.objects.get(&id).is_some_and(|e| e.spawned) {
            return;
        }
        if stats.is_empty() {
            return;
        }
        let mut broadcast_full = false;
        let mut update = StatusUpdate::new(id.0);
        {
            let Some(view) = self.stats(id) else {
                return;
            };
            for stat in stats {
                match stat {
                    Stat::PowerAttackSpeed => update.add_attribute(status::ATK_SPD, view.p_atk_spd() as i32),
                    Stat::MagicAttackSpeed => update.add_attribute(status::CAST_SPD, view.m_atk_spd()),
                    Stat::MoveSpeed => broadcast_full = true,
                    _ => {}
                }
            }
        }
        if broadcast_full {
            self.broadcast_user_info(id);
        } else {
            self.send_user_info(id);
            if update.has_attributes() {
                self.broadcast_to_self_and_visible(id, &update);
            }
        }
    }

    /// Java `Player.sendSkillList()`: `SkillList` of everything the player knows, 300 ms later.
    /// A request while one is waiting changes nothing.
    pub(crate) fn send_skill_list(&mut self, id: ObjectId) {
        let due = self.tick + TimerWheel::ms_to_ticks(SKILL_LIST_DELAY_MS);
        let Some(player) = self.players.get(&id) else {
            return;
        };
        if player.inv_tasks.skill_list.is_some() {
            return;
        }
        let timer = self.timers.schedule_at(due, TimerKind::SkillList(id));
        if let Some(player) = self.players.get_mut(&id) {
            player.inv_tasks.skill_list = Some(timer);
        }
    }

    pub(crate) fn on_skill_list_timer(&mut self, id: ObjectId) {
        let Some(player) = self.players.get_mut(&id) else {
            return;
        };
        player.inv_tasks.skill_list = None;
        // `getAllSkills()` is a `ConcurrentHashMap` of skill id to skill.
        let known = &player.core.skills;
        let transformed = player.transform.transformation.is_some();
        let keys: Vec<i32> = known.iter().map(|(skill_id, _)| *skill_id).collect();
        let mut skills: Vec<SkillListEntry> = java_concurrent_map_order(&keys)
            .into_iter()
            .filter_map(|skill_id| known.iter().find(|(s, _)| *s == skill_id).copied())
            .map(|(skill_id, level)| (skill_id, level, self.data.skill_is_passive(skill_id)))
            // A transformed player keeps its passive skills only, and not those the form also has.
            .filter(|&(skill_id, _, passive)| {
                !((transformed && !passive) || (passive && player.transform.skills.contains_key(&skill_id)))
            })
            .map(|(skill_id, level, passive)| SkillListEntry {
                id: skill_id,
                level,
                passive,
                // Clan skills of a clan with negative reputation and enchantable skills need the
                // clan and the skill enchant tables.
                disabled: false,
                enchanted: false,
            })
            .collect();
        // The skills of the form, by skill id.
        skills.extend(
            self.transformation_skill_list(id)
                .into_iter()
                .map(|(skill_id, level)| SkillListEntry {
                    id: skill_id,
                    level,
                    passive: false,
                    disabled: false,
                    enchanted: false,
                }),
        );
        self.send(id, &SkillList { skills });
    }

    /// Java `Player.sendItemList(open)`: `ItemList` (then `ExQuestItemList`) 250 ms later; a new
    /// request replaces the waiting one.
    pub(crate) fn send_item_list(&mut self, id: ObjectId, open: bool) {
        let due = self.tick + TimerWheel::ms_to_ticks(ITEM_LIST_DELAY_MS);
        let Some(player) = self.players.get_mut(&id) else {
            return;
        };
        let old = player.inv_tasks.item_list.take();
        if let Some((timer, _)) = old {
            self.timers.cancel(timer);
        }
        let timer = self.timers.schedule_at(due, TimerKind::ItemList(id));
        if let Some(player) = self.players.get_mut(&id) {
            player.inv_tasks.item_list = Some((timer, open));
        }
    }

    pub(crate) fn on_item_list_timer(&mut self, id: ObjectId) {
        let Some(player) = self.players.get_mut(&id) else {
            return;
        };
        let Some((_, open)) = player.inv_tasks.item_list.take() else {
            return;
        };
        let now_ms = unix_ms_now();
        let Some(player) = self.players.get(&id) else {
            return;
        };
        let mut list = item_list(&player.data, &*self.data, now_ms);
        list.show_window = open;
        list.block = self.inventory_block_packet(id);
        let mut quest: ExQuestItemList = quest_item_list(&player.data, &*self.data, now_ms);
        quest.block = self.inventory_block_packet(id);
        self.send(id, &list);
        // `ItemList.runImpl` sends the quest items after the list.
        self.send(id, &quest);
    }

    /// Java `PlayerInventory.refreshWeight()` -> `Player.refreshOverloaded()`: when the weight
    /// penalty level changed, the player's `UserInfo` and `EtcStatusUpdate` follow.
    pub(crate) fn refresh_weight(&mut self, id: ObjectId) {
        if self.refresh_overloaded(id) {
            self.broadcast_user_info(id);
            self.send_etc_status_update(id);
        }
    }

    /// Java `Player.refreshExpertisePenalty()` with its `EtcStatusUpdate` when a penalty changed.
    pub(crate) fn refresh_expertise(&mut self, id: ObjectId) {
        if self.refresh_expertise_penalty(id) {
            self.send_etc_status_update(id);
        }
    }
}

/// The iteration order of a `ConcurrentHashMap<Integer, _>` holding `keys` (in insertion order,
/// no duplicates). The table starts at 16 and doubles as soon as the count reaches three
/// quarters of it; a key sits in bucket `(h ^ (h >>> 16)) & (capacity - 1)` and a bucket keeps
/// insertion order.
pub(crate) fn java_concurrent_map_order(keys: &[i32]) -> Vec<i32> {
    let mut capacity = 16usize;
    while keys.len() >= capacity * 3 / 4 {
        capacity *= 2;
    }
    bucket_order(keys, capacity)
}

fn bucket_order(keys: &[i32], capacity: usize) -> Vec<i32> {
    let bucket = |k: i32| {
        let h = k as u32;
        ((h ^ (h >> 16)) as usize) & (capacity - 1)
    };
    let mut indexed: Vec<(usize, usize, i32)> =
        keys.iter().enumerate().map(|(i, &k)| (bucket(k), i, k)).collect();
    indexed.sort_unstable();
    indexed.into_iter().map(|(_, _, k)| k).collect()
}
