//! [`Qs`]: a handle on one quest state of one player (Java `QuestState`).
// Java: mechanics/script/QuestState.java
//
// Java scripts hold a `QuestState` object and call `qs.setCond(2, true)` next to
// `giveItems(player, ...)`. A Rust handle cannot hold `&mut World` while the script also calls
// the helpers, so `Qs` is a small `Copy` value (player + script) and every method takes the
// context: `qs.set_cond(ctx, 2, true)`. A handle whose state was removed (`exitQuest(true)`)
// reads like an empty state and writes nothing.

use l2_protocol_game::server::{ExShowQuestMark, TutorialShowQuestionMark};
use l2_util::ObjectId;

use super::ctx::ScriptCtx;
use super::setup::ScriptId;
use super::sound::QuestSound;
use super::state::{
    COMPLETED, COMPLETED_STATE_FLAGS_VAR, COND_VAR, CREATED, MEMO_EX_VAR, MEMO_VAR, QuestState, QuestType,
    QuestWrite, RESTART_VAR, STARTED, STATE_VAR, state_name,
};
use crate::world::World;

/// Java `QuestState.RESET_HOUR` / `RESET_MINUTES`: when a daily quest can be taken again.
const RESET_HOUR: i64 = 6;
const RESET_MINUTES: i64 = 30;

/// A player's state of one quest. Get one with [`ScriptCtx::get_quest_state`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Qs {
    pub(crate) player: ObjectId,
    pub(crate) script: ScriptId,
}

impl Qs {
    /// Java `getPlayer()`.
    pub fn player(self) -> ObjectId {
        self.player
    }

    /// Java `getQuestName()`.
    pub fn quest_name(self, ctx: &ScriptCtx<'_>) -> &'static str {
        ctx.world.scripts.name_of(self.script)
    }

    fn read<R>(self, world: &World, f: impl FnOnce(&QuestState) -> R) -> Option<R> {
        let name = world.scripts.name_of(self.script);
        world.scripts.players.get(&self.player)?.get(name).map(f)
    }

    fn write<R>(self, world: &mut World, f: impl FnOnce(&mut QuestState) -> R) -> Option<R> {
        let name = world.scripts.name_of(self.script);
        world.scripts.players.get_mut(&self.player)?.get_mut(name).map(f)
    }

    /// A copy of the state's values (for reads the methods below do not cover).
    pub fn snapshot(self, ctx: &ScriptCtx<'_>) -> Option<QuestState> {
        self.read(ctx.world, Clone::clone)
    }

    /// Java `getState()`.
    pub fn state(self, ctx: &ScriptCtx<'_>) -> u8 {
        self.read(ctx.world, QuestState::state).unwrap_or(CREATED)
    }

    /// Java `isCreated()`.
    pub fn is_created(self, ctx: &ScriptCtx<'_>) -> bool {
        self.state(ctx) == CREATED
    }

    /// Java `isStarted()`.
    pub fn is_started(self, ctx: &ScriptCtx<'_>) -> bool {
        self.state(ctx) == STARTED
    }

    /// Java `isCompleted()`.
    pub fn is_completed(self, ctx: &ScriptCtx<'_>) -> bool {
        self.state(ctx) == COMPLETED
    }

    /// Java `get(variable)`.
    pub fn get(self, ctx: &ScriptCtx<'_>, variable: &str) -> Option<String> {
        self.read(ctx.world, |qs| qs.get(variable).map(str::to_owned))
            .flatten()
    }

    /// Java `getInt(variable)`.
    pub fn get_int(self, ctx: &ScriptCtx<'_>, variable: &str) -> i32 {
        self.read(ctx.world, |qs| qs.get_int(variable)).unwrap_or(0)
    }

    /// Java `isSet(variable)`.
    pub fn is_set(self, ctx: &ScriptCtx<'_>, variable: &str) -> bool {
        self.read(ctx.world, |qs| qs.is_set(variable)).unwrap_or(false)
    }

    /// Java `getCond()`.
    pub fn get_cond(self, ctx: &ScriptCtx<'_>) -> i32 {
        self.read(ctx.world, QuestState::cond).unwrap_or(0)
    }

    /// Java `isCond(condition)`. A handle whose state was removed reads like an empty state
    /// (`_cond` 0), so `is_cond(ctx, 0)` is true for it as for a fresh state.
    pub fn is_cond(self, ctx: &ScriptCtx<'_>, condition: i32) -> bool {
        self.read(ctx.world, |qs| qs.is_cond(condition))
            .unwrap_or(condition == 0)
    }

    /// Java `getMemoState()`.
    pub fn get_memo_state(self, ctx: &ScriptCtx<'_>) -> i32 {
        self.read(ctx.world, QuestState::memo_state).unwrap_or(0)
    }

    /// Java `isMemoState(memoState)`.
    pub fn is_memo_state(self, ctx: &ScriptCtx<'_>, memo_state: i32) -> bool {
        self.read(ctx.world, |qs| qs.is_memo_state(memo_state))
            .unwrap_or(memo_state == 0)
    }

    /// Java `getMemoStateEx(slot)`.
    pub fn get_memo_state_ex(self, ctx: &ScriptCtx<'_>, slot: i32) -> i32 {
        self.read(ctx.world, |qs| qs.memo_state_ex(slot)).unwrap_or(0)
    }

    /// Java `isMemoStateEx(slot, memoStateEx)`.
    pub fn is_memo_state_ex(self, ctx: &ScriptCtx<'_>, slot: i32, memo_state_ex: i32) -> bool {
        self.get_memo_state_ex(ctx, slot) == memo_state_ex
    }

    /// Java `isExitQuestOnCleanUp()`.
    pub fn is_exit_quest_on_clean_up(self, ctx: &ScriptCtx<'_>) -> bool {
        self.read(ctx.world, QuestState::is_exit_quest_on_clean_up)
            .unwrap_or(false)
    }

    /// Java `setIsExitQuestOnCleanUp(value)`.
    pub fn set_exit_quest_on_clean_up(self, ctx: &mut ScriptCtx<'_>, value: bool) {
        self.write(ctx.world, |qs| qs.exit_quest_on_clean_up = value);
    }

    /// Java `setState(state)`: the state row is written and the journal sent.
    pub fn set_state(self, ctx: &mut ScriptCtx<'_>, state: u8) {
        set_state(ctx.world, self, state, true);
    }

    /// Java `setState(state, saveInDb)`.
    pub fn set_state_with(self, ctx: &mut ScriptCtx<'_>, state: u8, save_in_db: bool) {
        set_state(ctx.world, self, state, save_in_db);
    }

    /// Java `set(variable, value)`: the row is inserted or updated; `cond` also updates the
    /// condition, the step flags and the journal.
    pub fn set(self, ctx: &mut ScriptCtx<'_>, variable: &str, value: &str) {
        set(ctx.world, self, variable, value);
    }

    /// Java `set(variable, int value)`.
    pub fn set_int(self, ctx: &mut ScriptCtx<'_>, variable: &str, value: i32) {
        set(ctx.world, self, variable, &value.to_string());
    }

    /// Java `unset(variable)`.
    pub fn unset(self, ctx: &mut ScriptCtx<'_>, variable: &str) {
        unset(ctx.world, self, variable);
    }

    /// Java `setCond(value)` (`play_quest_middle = false`) and `setCond(value, playQuestMiddle)`:
    /// only while the quest is started.
    pub fn set_cond(self, ctx: &mut ScriptCtx<'_>, value: i32, play_quest_middle: bool) {
        if !self.is_started(ctx) {
            return;
        }
        set(ctx.world, self, COND_VAR, &value.to_string());
        if play_quest_middle {
            ctx.world
                .send(self.player, &QuestSound::ItemsoundQuestMiddle.packet());
        }
    }

    /// Java `setMemoState(value)`.
    pub fn set_memo_state(self, ctx: &mut ScriptCtx<'_>, value: i32) {
        set(ctx.world, self, MEMO_VAR, &value.to_string());
    }

    /// Java `setMemoStateEx(slot, value)`.
    pub fn set_memo_state_ex(self, ctx: &mut ScriptCtx<'_>, slot: i32, value: i32) {
        set(
            ctx.world,
            self,
            &format!("{MEMO_EX_VAR}{slot}"),
            &value.to_string(),
        );
    }

    /// Java `addNotifyOfDeath(creature)`: the quest's `on_death` runs when that player dies.
    pub fn add_notify_of_death(self, ctx: &mut ScriptCtx<'_>, creature: ObjectId) {
        if !ctx.world.players.contains_key(&creature) {
            return;
        }
        let name = ctx.world.scripts.name_of(self.script).to_owned();
        let quests = ctx.world.scripts.players.entry(creature).or_default();
        if !quests.notify_of_death.contains(&name) {
            quests.notify_of_death.push(name);
        }
    }

    /// Java `removeNotifyOfDeath(creature)` (`Player.removeNotifyQuestOfDeath(qs)`): the quest's
    /// `on_death` no longer runs when that player dies.
    pub fn remove_notify_of_death(self, ctx: &mut ScriptCtx<'_>, creature: ObjectId) {
        let name = ctx.world.scripts.name_of(self.script).to_owned();
        if let Some(quests) = ctx.world.scripts.players.get_mut(&creature) {
            quests.notify_of_death.retain(|n| *n != name);
        }
    }

    /// Java `startQuest()`: `cond` 1, the state `Started`, the accept sound. Only for a created
    /// state of a quest that is not custom.
    pub fn start_quest(self, ctx: &mut ScriptCtx<'_>) {
        let custom = ctx
            .world
            .scripts
            .script(self.script)
            .is_some_and(|s| s.is_custom());
        if self.is_created(ctx) && !custom {
            set(ctx.world, self, COND_VAR, "1");
            set_state(ctx.world, self, STARTED, true);
            ctx.world
                .send(self.player, &QuestSound::ItemsoundQuestAccept.packet());
        }
    }

    /// Java `exitQuest(repeatable)` and `exitQuest(repeatable, playExitQuest)`: the quest items
    /// go, a repeatable quest loses its rows and state, a one-time quest keeps `Completed`.
    pub fn exit_quest(self, ctx: &mut ScriptCtx<'_>, repeatable: bool, play_exit_quest: bool) {
        exit_quest(ctx, self, repeatable);
        if play_exit_quest {
            ctx.world
                .send(self.player, &QuestSound::ItemsoundQuestFinish.packet());
        }
    }

    /// Java `exitQuest(QuestType)` and `exitQuest(QuestType, playExitQuest)`.
    pub fn exit_quest_type(self, ctx: &mut ScriptCtx<'_>, kind: QuestType, play_exit_quest: bool) {
        match kind {
            QuestType::Daily => {
                exit_quest(ctx, self, false);
                self.set_restart_time(ctx);
            }
            QuestType::OneTime | QuestType::Repeatable => {
                exit_quest(ctx, self, kind == QuestType::Repeatable);
            }
        }
        if play_exit_quest {
            ctx.world
                .send(self.player, &QuestSound::ItemsoundQuestFinish.packet());
        }
    }

    /// Java `showQuestionMark(number)`.
    pub fn show_question_mark(self, ctx: &mut ScriptCtx<'_>, number: i32) {
        ctx.world
            .send(self.player, &TutorialShowQuestionMark { mark_id: number });
    }

    /// Java `setRestartTime()`: the next 6:30 in the server's zone (D-48).
    pub fn set_restart_time(self, ctx: &mut ScriptCtx<'_>) {
        let now = ctx.now_ms();
        let value = restart_time(now, l2_util::time::local_offset_millis(now));
        set(ctx.world, self, RESTART_VAR, &value.to_string());
    }

    /// Java `isNowAvailable()`: the restart time has passed.
    pub fn is_now_available(self, ctx: &ScriptCtx<'_>) -> bool {
        // NOTE(java-quirk): `Long.parseLong` of a bad value throws in Java; it counts as "not
        // available" here.
        self.get(ctx, RESTART_VAR)
            .and_then(|v| v.parse::<i64>().ok())
            .is_some_and(|time| time <= ctx.now_ms())
    }
}

/// Java `setRestartTime`'s calendar: today's (or, from 6:00 on, tomorrow's) 6:30 local time.
/// `Calendar.set` leaves the seconds and milliseconds of now.
pub(crate) fn restart_time(now_utc_ms: i64, offset_ms: i64) -> i64 {
    const DAY: i64 = 86_400_000;
    const HOUR: i64 = 3_600_000;
    const MINUTE: i64 = 60_000;
    let local = now_utc_ms + offset_ms;
    let day_start = local.div_euclid(DAY) * DAY;
    let in_day = local - day_start;
    let seconds_and_millis = in_day % MINUTE;
    let day = if in_day / HOUR >= RESET_HOUR {
        day_start + DAY
    } else {
        day_start
    };
    day + RESET_HOUR * HOUR + RESET_MINUTES * MINUTE + seconds_and_millis - offset_ms
}

/// Java `Quest.createQuestVarInDb` / `updateQuestVarInDb` / ... : one statement for the
/// persistence task.
pub(crate) fn quest_write(world: &mut World, write: QuestWrite) {
    world.outbox.persist(crate::outbox::PersistCommand::Quest(write));
}

fn char_id(qs: Qs) -> i32 {
    qs.player.0
}

/// Java `QuestState.setState(state, saveInDb)`.
pub(crate) fn set_state(world: &mut World, qs: Qs, state: u8, save_in_db: bool) {
    let Some((old, name)) = qs.write(world, |s| {
        let old = s.state;
        if old != state {
            s.state = state;
        }
        (old, s.quest_name.clone())
    }) else {
        return;
    };
    if old == state {
        return;
    }
    if save_in_db {
        let write = if old == CREATED {
            // Java `createQuestInDb`.
            QuestWrite::InsertVar {
                char_id: char_id(qs),
                name,
                var: STATE_VAR.to_owned(),
                value: state_name(state).to_owned(),
            }
        } else {
            // Java `updateQuestInDb`.
            QuestWrite::UpdateVar {
                char_id: char_id(qs),
                name,
                var: STATE_VAR.to_owned(),
                value: state_name(state).to_owned(),
            }
        };
        quest_write(world, write);
    }
    world.send_quest_list(qs.player);
}

/// Java `QuestState.set(variable, value)`.
pub(crate) fn set(world: &mut World, qs: Qs, variable: &str, value: &str) {
    let Some((old, name)) = qs.write(world, |s| {
        (
            s.vars.insert(variable.to_owned(), value.to_owned()),
            s.quest_name.clone(),
        )
    }) else {
        return;
    };
    let write = if old.is_some() {
        QuestWrite::UpdateVar {
            char_id: char_id(qs),
            name,
            var: variable.to_owned(),
            value: value.to_owned(),
        }
    } else {
        QuestWrite::InsertVar {
            char_id: char_id(qs),
            name,
            var: variable.to_owned(),
            value: value.to_owned(),
        }
    };
    quest_write(world, write);

    if variable == COND_VAR {
        let previous = old.and_then(|o| o.parse::<i32>().ok()).unwrap_or(0);
        let new_cond = value.parse::<i32>().unwrap_or(0);
        qs.write(world, |s| s.cond = new_cond);
        set_cond_flags(world, qs, new_cond, previous);
    }
}

/// Java `QuestState.setCond(cond, old)` (private): the skipped-step flags, the journal and the
/// quest mark.
fn set_cond_flags(world: &mut World, qs: Qs, cond: i32, old: i32) {
    if cond == old {
        return;
    }
    let mut flags: i32 = 0;
    // cond 0 and 1 do not need completedStateFlags. Also, if cond > 1, the 1st step must always
    // exist (i.e. it can never be skipped). So if cond is 2, we can still safely assume no steps
    // have been skipped. Finally, more than 31 steps CANNOT be supported in any way with
    // skipping.
    if !(3..=31).contains(&cond) {
        unset(world, qs, COMPLETED_STATE_FLAGS_VAR);
    } else {
        flags = qs
            .read(world, |s| s.get_int(COMPLETED_STATE_FLAGS_VAR))
            .unwrap_or(0);
    }
    // Java's `1 << n` uses the low five bits of `n`.
    let bit = |n: i32| 1i32.wrapping_shl(n as u32);
    if flags == 0 {
        // case 1: No steps have been skipped so far...
        if cond > old + 1 {
            // Set the most significant bit to 1 (indicates that there exist skipped states).
            // Also, ensure that the least significant bit is an 1 (the first step is never
            // skipped, no matter what the cond says).
            flags = 0x8000_0001_u32 as i32;
            // Since no flag had been skipped until now, the least significant bits must all be
            // set to 1, up until "old" number of bits.
            flags |= bit(old).wrapping_sub(1);
            // Now, just set the bit corresponding to the passed cond to 1 (current step).
            flags |= bit(cond - 1);
            set(world, qs, COMPLETED_STATE_FLAGS_VAR, &flags.to_string());
        }
    } else if cond < old {
        // case 2: There were exist previously skipped steps; a push back to a previous step
        // clears all completion flags ahead.
        flags &= bit(cond).wrapping_sub(1);
        // now, check if this resulted in no steps being skipped any more
        if flags == bit(cond).wrapping_sub(1) {
            unset(world, qs, COMPLETED_STATE_FLAGS_VAR);
        } else {
            // set the most significant bit back to 1 again, to correctly indicate that this
            // skips states.
            flags |= 0x8000_0001_u32 as i32;
            set(world, qs, COMPLETED_STATE_FLAGS_VAR, &flags.to_string());
        }
    } else {
        // If this moves forward, it changes nothing on previously skipped steps. Just mark this
        // state and we are done.
        flags |= bit(cond - 1);
        set(world, qs, COMPLETED_STATE_FLAGS_VAR, &flags.to_string());
    }

    // Send a packet to the client to inform it of the quest progress (step change).
    world.send_quest_list(qs.player);
    let (custom, quest_id) = world
        .scripts
        .script(qs.script)
        .map_or((true, 0), |s| (s.is_custom(), s.id()));
    if !custom && cond > 0 {
        world.send(qs.player, &ExShowQuestMark { quest_id });
    }
}

/// Java `QuestState.unset(variable)`.
pub(crate) fn unset(world: &mut World, qs: Qs, variable: &str) {
    let Some(Some(name)) = qs.write(world, |s| {
        let old = s.vars.remove(variable)?;
        if variable == COND_VAR {
            s.cond = 0;
        }
        let _ = old;
        Some(s.quest_name.clone())
    }) else {
        return;
    };
    quest_write(
        world,
        QuestWrite::DeleteVar {
            char_id: char_id(qs),
            name,
            var: variable.to_owned(),
        },
    );
}

/// Java `QuestState.exitQuest(repeatable)`.
fn exit_quest(ctx: &mut ScriptCtx<'_>, qs: Qs, repeatable: bool) {
    let name = ctx.world.scripts.name_of(qs.script).to_owned();
    // `_player.removeNotifyQuestOfDeath(this)`.
    if let Some(quests) = ctx.world.scripts.players.get_mut(&qs.player) {
        quests.notify_of_death.retain(|n| *n != name);
    }
    if !qs.is_started(ctx) {
        return;
    }
    // Clean registered quest items (`removeRegisteredQuestItems`: `takeItems(player, -1, ids)`).
    let items = ctx.world.scripts.quest_items(qs.script).to_vec();
    ctx.take_items_many(qs.player, -1, &items);
    let quest_name = qs.read(ctx.world, |s| s.quest_name.clone()).unwrap_or(name);
    quest_write(
        ctx.world,
        QuestWrite::DeleteQuest {
            char_id: char_id(qs),
            name: quest_name.clone(),
            repeatable,
        },
    );
    if repeatable {
        if let Some(quests) = ctx.world.scripts.players.get_mut(&qs.player) {
            quests.remove(&quest_name);
        }
        ctx.world.send_quest_list(qs.player);
    } else {
        set_state(ctx.world, qs, COMPLETED, true);
    }
    qs.write(ctx.world, |s| s.vars.clear());
}

#[cfg(test)]
mod tests {
    use super::*;

    const HOUR: i64 = 3_600_000;

    #[test]
    fn restart_time_is_the_next_half_past_six_local() {
        // 2026-01-01 05:00:07.250 UTC, zone UTC: today 06:30:07.250.
        let day = 1_767_225_600_000;
        let now = day + 5 * HOUR + 7_250;
        assert_eq!(restart_time(now, 0), day + 6 * HOUR + 30 * 60_000 + 7_250);
        // From 06:00 on it is tomorrow.
        let now = day + 6 * HOUR + 1;
        assert_eq!(
            restart_time(now, 0),
            day + 86_400_000 + 6 * HOUR + 30 * 60_000 + 1
        );
        // UTC+2: 05:00 UTC is 07:00 local, so tomorrow 06:30 local = 04:30 UTC.
        let now = day + 5 * HOUR;
        assert_eq!(
            restart_time(now, 2 * HOUR),
            day + 86_400_000 + 4 * HOUR + 30 * 60_000
        );
    }
}
