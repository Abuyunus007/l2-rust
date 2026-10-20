//! The buffs of a character kept over a logout: what the snapshot stores (Java
//! `Player.storeEffect`) and what comes back at enter world (Java `Player.restoreEffects`).
//! Design: `docs/design/m5-skills.md`.
// Java: entity/actor/Player.java (storeEffect, restoreEffects, STORE_SKILL_SAVE,
//       RESTORE_SKILL_SAVE, DELETE_SKILL_SAVE, onPlayerEnter)
// Java: config/PlayerConfig.java (STORE_SKILL_COOLTIME, ALT_STORE_TOGGLES, ALT_STORE_DANCES)
//
// The rows are `character_skills_save` (D-39: the Java game server reads the same rows). The
// world turns its effect lists into [`SavedEffect`]s when a snapshot is taken; the persistence
// task writes them (`PlayerProfile::effects_to_store`) and the database loader hands the rows of
// the last logout back in `PlayerProfile::effects_to_restore`, which the world applies once, when
// the character enters.

use std::collections::HashSet;

use l2_data::stats::AbnormalType;
use l2_util::ObjectId;

use super::SkillKey;
use crate::player::PlayerData;
use crate::snapshot::snapshot_for_store;
use crate::world::World;

/// One row of `character_skills_save`. The row's `buff_index` is not kept: the rows are written
/// and read in order, so the index is the position in the list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SavedEffect {
    /// `skill_id`.
    pub skill_id: i32,
    /// `skill_level`.
    pub skill_level: i32,
    /// `remaining_time`: the seconds the buff had left (Java `BuffInfo.getTime()`).
    pub remaining_time: i32,
    /// `reuse_delay`: the length of the reuse timer in milliseconds, 0 when there is none.
    pub reuse_delay: i64,
    /// `systime`: the wall clock millisecond at which the reuse timer ends, 0 when there is none.
    pub systime: i64,
    /// `restore_type`: 0 for a buff that was on the character, 1 for a reuse timer only.
    pub restore_type: i32,
}

impl World {
    /// The snapshot of a player in the world for the persistence task: its data with the live
    /// values (`snapshot_for_store`) and its buffs (Java `storeMe` -> `store(true)`).
    pub(crate) fn store_snapshot(&self, id: ObjectId) -> Option<PlayerData> {
        let player = self.players.get(&id)?;
        let mut snapshot = snapshot_for_store(player, self.tick);
        snapshot.profile.effects_to_store = self.saved_effects(id);
        Some(snapshot)
    }

    /// Java `Player.storeEffect(true)`: the rows the character's buffs are saved as, in Java's
    /// order (buffs, triggered, dances, toggles, debuffs). `None` when `StoreSkillCooltime` is off:
    /// Java then returns before it touches the table.
    // Java skips a buff whose reuse hash (`SkillData.getSkillHashCode`: the skill id and level) is
    // already stored, so one row per skill level.
    pub(crate) fn saved_effects(&self, id: ObjectId) -> Option<Vec<SavedEffect>> {
        let settings = self.settings.effects;
        if !settings.store_skill_cooltime {
            return None;
        }
        let mut rows = Vec::new();
        let mut stored: HashSet<SkillKey> = HashSet::new();
        if let Some(list) = self.effect_list(id) {
            for info in list.effects() {
                // Do not save heals.
                if info.abnormal_type() == AbnormalType::LifeForceOthers {
                    continue;
                }
                // Toggles are skipped, unless they are necessary to be always on.
                if info.facts.toggle && !settings.alt_store_toggles {
                    continue;
                }
                // Dances and songs are not kept in retail.
                if info.facts.dance && !settings.alt_store_dances {
                    continue;
                }
                if !stored.insert(info.skill) {
                    continue;
                }
                rows.push(SavedEffect {
                    skill_id: info.skill.id,
                    skill_level: info.skill.level,
                    remaining_time: info.time(self.tick),
                    reuse_delay: 0,
                    systime: 0,
                    restore_type: 0,
                });
            }
        }
        // M5-CAST-01: Java then writes every reuse timer that still runs and whose skill is not
        // stored above, as a row with `remaining_time` -1, `restore_type` 1 and the timer's
        // `reuse`/`stamp` (`getSkillReuseTimeStamps`). The reuse timers come with casting.
        Some(rows)
    }

    /// Java `Player.restoreEffects`: the rows the character had at its last logout are applied
    /// with the time they had left (`restore_type` 0). Called once when the character enters;
    /// the rows are taken from the profile.
    // Java: `if (PlayerConfig.STORE_SKILL_COOLTIME) restoreEffects();` in `onPlayerEnter`.
    pub(crate) fn restore_saved_effects(&mut self, id: ObjectId) {
        let Some(player) = self.players.get_mut(&id) else {
            return;
        };
        let rows = std::mem::take(&mut player.data.profile.effects_to_restore);
        if !self.settings.effects.store_skill_cooltime {
            return;
        }
        let Some(skills) = self.skill_table() else {
            return;
        };
        for row in rows {
            // A skill the datapack does not have any more is dropped (Java `skill == null`).
            if skills.get(row.skill_id, row.skill_level).is_none() {
                continue;
            }
            // M5-CAST-01: Java first restores the reuse timer when it has more than 10 ms left
            // (`disableSkill` and `addTimeStamp`), for rows of both restore types.
            // A reuse-only row ends there: the buff itself was gone at logout.
            if row.restore_type > 0 {
                continue;
            }
            // Java `skill.applyEffects(this, this, false, rs.getInt("remaining_time"))`.
            // NOTE(java-quirk): a buff with no time left at logout (remaining 0) comes back with the
            // skill's full time, because Java ignores a custom abnormal time of 0 or less.
            self.apply_effects_timed(
                id,
                id,
                SkillKey::new(row.skill_id, row.skill_level),
                false,
                row.remaining_time,
            );
        }
    }
}
