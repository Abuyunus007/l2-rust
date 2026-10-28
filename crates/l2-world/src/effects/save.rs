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
        self.store_snapshot_with(id, true)
    }

    /// [`World::store_snapshot`] with Java's `store(storeActiveEffects)` flag: `false` keeps only
    /// the reuse timers (`setActiveClass` stores with `SubclassStoreSkillCooltime`, S-SUB-01).
    pub(crate) fn store_snapshot_with(&self, id: ObjectId, store_effects: bool) -> Option<PlayerData> {
        let player = self.players.get(&id)?;
        let mut snapshot = snapshot_for_store(player, self.tick);
        snapshot.profile.effects_to_store = self.saved_effects_with(id, store_effects);
        // Java `Player.store` -> `storeItemReuseDelay()` runs on every store, with no setting.
        snapshot.profile.item_reuse_to_store = Some(self.saved_item_reuse(id));
        // `storeRecipeShopList()` (only with `StoreRecipeShopList` and a shop).
        snapshot.profile.recipe_shop_to_store = self.recipe_shop_to_store(id);
        // `SevenSigns.getInstance().saveSevenSignsData(getObjectId())` (M8-SS-01).
        snapshot.profile.seven_signs_to_store = self.ss_row_to_store(id.0);
        // `_clanPrivileges.getMask()`: the mask of the moment, also of a shared rank object.
        if let Some(clan) = snapshot.profile.clan.as_mut() {
            clan.privileges = crate::clan::ClanPrivilegesRef::Own(self.clan_privileges_of(id));
        }
        // `isOnlineInt()`: 2 for a detached client (an offline trader).
        if self.is_offline_trader(id) {
            snapshot.profile.online_status = 2;
        }
        // `_observerMode ? _lastLoc.getX() : getX()` (M8-OLY-01).
        if player.olympiad.observer_mode {
            (snapshot.x, snapshot.y, snapshot.z) = player.olympiad.last_loc;
        }
        Some(snapshot)
    }

    /// Java `Player.storeEffect(true)`: the rows the character's buffs are saved as, in Java's
    /// order (buffs, triggered, dances, toggles, debuffs). `None` when `StoreSkillCooltime` is off:
    /// Java then returns before it touches the table.
    ///
    /// With `store_effects` false (`storeEffect(false)`) the buffs are skipped and only the reuse
    /// timers are kept (`restore_type` 1).
    // Java skips a buff whose reuse hash (`SkillData.getSkillHashCode`: the skill id and level) is
    // already stored, so one row per skill level.
    pub(crate) fn saved_effects_with(&self, id: ObjectId, store_effects: bool) -> Option<Vec<SavedEffect>> {
        let settings = self.settings.effects;
        if !settings.store_skill_cooltime {
            return None;
        }
        let mut rows = Vec::new();
        let mut stored: HashSet<SkillKey> = HashSet::new();
        // The reuse timers live on the world clock; the row keeps Java's wall-clock stamp
        // (`currentTime < getStamp()` is "time left").
        let now_wall = crate::world::unix_ms_now();
        let reuse = self.skill_reuse_time_stamps(id);
        let running = |key: SkillKey| {
            let hash = l2_data::SkillTemplates::hash_code(key.id, key.level);
            reuse
                .iter()
                .find(|t| t.hash_code == hash && t.remaining_ms > 0)
                .map_or((0, 0), |t| (t.reuse_ms, now_wall + t.remaining_ms))
        };
        if let Some(list) = self.effect_list(id).filter(|_| store_effects) {
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
                let (reuse_delay, systime) = running(info.skill);
                rows.push(SavedEffect {
                    skill_id: info.skill.id,
                    skill_level: info.skill.level,
                    remaining_time: info.time(self.tick),
                    reuse_delay,
                    systime,
                    restore_type: 0,
                });
            }
        }
        // Skills under reuse ('restore_type' = 1), in the reuse map's order.
        for t in &reuse {
            let key = SkillKey::new(t.skill_id, t.skill_level);
            if stored.contains(&key) || t.remaining_ms <= 0 {
                continue;
            }
            stored.insert(key);
            rows.push(SavedEffect {
                skill_id: t.skill_id,
                skill_level: t.skill_level,
                remaining_time: -1,
                reuse_delay: t.reuse_ms,
                systime: now_wall + t.remaining_ms,
                restore_type: 1,
            });
        }
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
        self.apply_saved_effect_rows(id, rows);
    }

    /// The loop of Java `Player.restoreEffects`: the reuse timers and the buffs of `rows`. The
    /// subclass switch calls it without the `StoreSkillCooltime` check (Java's `setActiveClass`
    /// calls `restoreEffects()` directly, S-SUB-01).
    pub(crate) fn apply_saved_effect_rows(&mut self, id: ObjectId, rows: Vec<SavedEffect>) {
        let Some(skills) = self.skill_table() else {
            return;
        };
        let now_wall = crate::world::unix_ms_now();
        for row in rows {
            // A skill the datapack does not have any more is dropped (Java `skill == null`).
            let Some(skill) = skills.get(row.skill_id, row.skill_level) else {
                continue;
            };
            // The reuse timer comes back when it has more than 10 ms left, for rows of both
            // restore types.
            let time = row.systime - now_wall;
            if time > 10 {
                self.disable_skill(id, skill, time);
                self.restore_time_stamp(id, skill, row.reuse_delay, time);
            }
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
