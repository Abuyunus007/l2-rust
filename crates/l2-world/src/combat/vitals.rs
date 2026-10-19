//! Setting HP/MP/CP after damage and the `StatusUpdate` broadcasts that follow.
// Java: entity/actor/status/{CreatureStatus,PlayerStatus}.java (setCurrentHp, setCurrentMp,
//       setCurrentCp, reduceMp)
// Java: entity/actor/Creature.java (broadcastStatusUpdate, needHpUpdate, MAX_HP_BAR_PX,
//       initCharStatusUpdateValues), entity/actor/Player.java (broadcastStatusUpdate,
//       updateUserInfo)
//
// M4-REGEN-01 owns `status.rs` (the regeneration task and its start/stop rules). These setters
// are the parts combat needs; the places where Java starts or stops the regeneration task are
// marked `// M4-REGEN-01:` so the two can be merged into one setter when both cards are in.
// The names are prefixed `vitals_` so they cannot clash with the status module's.

use l2_protocol_game::server::StatusUpdate;
use l2_protocol_game::server::visibility::status;
use l2_util::ObjectId;

use super::HpUpdateChecks;
use crate::timer::{TimerKind, TimerWheel};
use crate::world::World;

/// `Creature.MAX_HP_BAR_PX`.
const MAX_HP_BAR_PX: f64 = 352.0;
/// `Player.broadcastStatusUpdate` sends 50 ms later.
const PLAYER_STATUS_UPDATE_DELAY_MS: u64 = 50;

impl World {
    /// `getMaxHp()` of the stat engine (the core's cached value when the creature has no view).
    fn vitals_max_hp(&self, id: ObjectId) -> f64 {
        match self.stats(id) {
            Some(s) => f64::from(s.max_hp()),
            None => self.core_of(id).map_or(0.0, |c| c.max_hp),
        }
    }

    /// Java `CreatureStatus.setCurrentHp(newHp, broadcastPacket)` (with `PlayerStatus`'s
    /// override, whose extra is the tutorial quest): `true` when the integer HP changed.
    pub(crate) fn vitals_set_hp(&mut self, id: ObjectId, new_hp: f64, broadcast: bool) -> bool {
        let max_hp = self.vitals_max_hp(id);
        let is_player = self.is_player(id);
        let dead = self.is_dead(id);
        let Some(core) = self.core_of_mut(id) else {
            return false;
        };
        let current_hp = core.cur_hp as i32;
        // Tempfix for updating UserInfo when skills like Final Frenzy and Final Fortress
        // conditions activate.
        let thirty_percent = max_hp * 0.3;
        let current = f64::from(current_hp);
        let update_user_info = is_player
            && ((current < thirty_percent && new_hp > thirty_percent)
                || (current > thirty_percent && new_hp < thirty_percent));
        if dead {
            return false;
        }
        if new_hp >= max_hp {
            core.cur_hp = max_hp;
            // M4-REGEN-01: clear REGEN_FLAG_HP; stop the regeneration task when no flag is left.
        } else {
            core.cur_hp = new_hp;
            // M4-REGEN-01: set REGEN_FLAG_HP and start the regeneration task.
        }
        let changed = current != core.cur_hp;
        if changed && broadcast {
            self.vitals_broadcast_status_update(id);
        }
        if update_user_info {
            // Java `Player.updateUserInfo()`: `UserInfo` and `ExBrExtraUserInfo` to the player.
            self.send_user_info(id);
        }
        changed
    }

    /// Java `CreatureStatus.setCurrentMp(newMp, broadcastPacket)`.
    pub(crate) fn vitals_set_mp(&mut self, id: ObjectId, new_mp: f64, broadcast: bool) -> bool {
        let max_mp = match self.stats(id) {
            Some(s) => f64::from(s.max_mp()),
            None => self.core_of(id).map_or(0.0, |c| c.max_mp),
        };
        if self.is_dead(id) {
            return false;
        }
        let Some(core) = self.core_of_mut(id) else {
            return false;
        };
        let current_mp = core.cur_mp as i32;
        if new_mp >= max_mp {
            core.cur_mp = max_mp;
            // M4-REGEN-01: clear REGEN_FLAG_MP; stop the regeneration task when no flag is left.
        } else {
            core.cur_mp = new_mp;
            // M4-REGEN-01: set REGEN_FLAG_MP and start the regeneration task.
        }
        let changed = f64::from(current_mp) != core.cur_mp;
        if changed && broadcast {
            self.vitals_broadcast_status_update(id);
        }
        changed
    }

    /// Java `CreatureStatus.reduceMp(value)`.
    pub(crate) fn vitals_reduce_mp(&mut self, id: ObjectId, value: f64) {
        let current = self.core_of(id).map_or(0.0, |c| c.cur_mp);
        self.vitals_set_mp(id, (current - value).max(0.0), true);
    }

    /// Java `PlayerStatus.setCurrentCp(value, broadcastPacket)`.
    pub(crate) fn vitals_set_cp(&mut self, id: ObjectId, value: f64, broadcast: bool) {
        let max_cp = match self.stats(id) {
            Some(s) => f64::from(s.max_cp()),
            None => self.core_of(id).map_or(0.0, |c| c.max_cp),
        };
        if self.is_dead(id) {
            return;
        }
        let Some(p) = self.players.get_mut(&id) else {
            return;
        };
        let current_cp = p.core.cur_cp as i32;
        let new_cp = value.max(0.0);
        if new_cp >= max_cp {
            p.core.cur_cp = max_cp;
            // M4-REGEN-01: clear REGEN_FLAG_CP; stop the regeneration task when no flag is left.
        } else {
            p.core.cur_cp = new_cp;
            // M4-REGEN-01: set REGEN_FLAG_CP and start the regeneration task.
        }
        if f64::from(current_cp) != p.core.cur_cp && broadcast {
            self.vitals_broadcast_status_update(id);
        }
    }

    /// Java `broadcastStatusUpdate()`: a player's goes to itself 50 ms later (one pending at a
    /// time), anybody else's to its status listeners (the players that target it) when the HP bar
    /// would move (`needHpUpdate`).
    pub(crate) fn vitals_broadcast_status_update(&mut self, id: ObjectId) {
        if self.is_player(id) {
            if self.combat.status_update_timers.contains_key(&id) {
                return;
            }
            let due = self.tick + TimerWheel::ms_to_ticks(PLAYER_STATUS_UPDATE_DELAY_MS);
            let timer = self.timers.schedule_at(due, TimerKind::PlayerStatusUpdate(id));
            self.combat.status_update_timers.insert(id, timer);
            return;
        }
        let listeners = self.status_listeners(id);
        if listeners.is_empty() || !self.need_hp_update(id) {
            return;
        }
        let Some(core) = self.core_of(id) else {
            return;
        };
        let mut update = StatusUpdate::new(id.0);
        update.add_attribute(status::MAX_HP, self.vitals_max_hp(id) as i32);
        update.add_attribute(status::CUR_HP, core.cur_hp as i32);
        for listener in listeners {
            self.send(listener, &update);
        }
    }

    /// The `Player.broadcastStatusUpdate` task: the six values to the player. (Party windows,
    /// the olympiad and duels arrive with their systems.)
    pub(crate) fn on_player_status_update_timer(&mut self, id: ObjectId) {
        self.combat.status_update_timers.remove(&id);
        let Some(view) = self.stats(id) else {
            return;
        };
        let core = view.core();
        let mut update = StatusUpdate::new(id.0);
        update.add_attribute(status::MAX_HP, view.max_hp());
        update.add_attribute(status::CUR_HP, core.cur_hp as i32);
        update.add_attribute(status::MAX_MP, view.max_mp());
        update.add_attribute(status::CUR_MP, core.cur_mp as i32);
        update.add_attribute(status::MAX_CP, view.max_cp());
        update.add_attribute(status::CUR_CP, core.cur_cp as i32);
        self.send(id, &update);
    }

    /// Java `CreatureStatus.getStatusListener()`: the players whose target is `id` (Java adds a
    /// player in `Player.setTarget` and removes it when the target changes). Sorted by id, so the
    /// order is stable (Java's set has no order).
    pub(crate) fn status_listeners(&self, id: ObjectId) -> Vec<ObjectId> {
        let mut listeners: Vec<ObjectId> = self
            .players
            .iter()
            .filter(|(_, p)| p.target == Some(id))
            .map(|(listener, _)| *listener)
            .collect();
        listeners.sort_unstable();
        listeners
    }

    /// Java `Creature.needHpUpdate()`: `true` when the HP bar (352 px) would move, and moves the
    /// checks along.
    // Java fills the checks in the constructor (`initCharStatusUpdateValues`) from the max HP of
    // then; here they are filled the first time they are needed. An NPC's max HP does not change
    // before effects (M5), so the value is the same.
    fn need_hp_update(&mut self, id: ObjectId) -> bool {
        let max_hp = self.vitals_max_hp(id);
        let current_hp = self.core_of(id).map_or(0.0, |c| c.cur_hp);
        let state = self.combat_state_mut(id);
        let checks = state.hp_update.get_or_insert_with(|| {
            let interval = max_hp / MAX_HP_BAR_PX;
            HpUpdateChecks {
                inc_check: max_hp,
                dec_check: max_hp - interval,
                interval,
            }
        });
        if current_hp <= 1.0 || max_hp < MAX_HP_BAR_PX {
            return true;
        }
        if current_hp <= checks.dec_check || current_hp >= checks.inc_check {
            if current_hp == max_hp {
                checks.inc_check = current_hp + 1.0;
                checks.dec_check = current_hp - checks.interval;
            } else {
                let double_multi = current_hp / checks.interval;
                let int_multi = double_multi as i32;
                // NOTE(java-quirk): `doubleMulti < intMulti` is never true for a positive value
                // (the cast truncates towards zero), so this is always `intMulti`.
                let multi = if double_multi < f64::from(int_multi) {
                    int_multi - 1
                } else {
                    int_multi
                };
                checks.dec_check = checks.interval * f64::from(multi);
                checks.inc_check = checks.dec_check + checks.interval;
            }
            return true;
        }
        false
    }
}
