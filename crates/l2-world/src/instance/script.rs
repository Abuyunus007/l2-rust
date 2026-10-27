//! Java `InstanceScript`: what the instance scripts call to put a player into an instance, finish
//! it, restrict the re-entry and take buffs away. The scripts themselves come with the script
//! milestone (D-05); the hooks a script implements (`onEnterInstance`, `checkConditions`) are the
//! closures of [`World::enter_instance`].
// Java: mechanics/script/InstanceScript.java
// Java: mechanics/script/Quest.java (teleportPlayer)

use l2_data::instance::{DayOfWeek, InstanceReenterTime};
use l2_data::{InstanceReenterType, InstanceRemoveBuffType};
use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{SystemMessage, SystemMessageParam};
use l2_util::ObjectId;
use tracing::warn;

use super::InstanceWorld;
use crate::effects::SkillFinishType;
use crate::teleport::TeleportTarget;
use crate::world::World;

const DAY_MS: i64 = 86_400_000;

/// What [`World::enter_instance`] did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnterOutcome {
    /// The player's world is of another template and has players: "You have entered another
    /// instance zone, therefore you cannot enter corresponding dungeon."
    OtherInstanceBusy,
    /// The player went (back) into the world he is allowed in (`firstEntrance` false).
    Existing(i32),
    /// A new instance was made for him (`firstEntrance` true).
    New(i32),
    /// `checkConditions` said no.
    Refused,
}

impl World {
    /// Java `InstanceScript.enterInstance(player, templateId)`. `check_conditions` is the
    /// script's `checkConditions(player)`, `on_enter` its `onEnterInstance(player, world,
    /// firstEntrance)` (with the instance id of the world); it typically adds the allowed players.
    pub fn enter_instance(
        &mut self,
        player: ObjectId,
        template_id: i32,
        check_conditions: impl FnOnce(&mut World, ObjectId) -> bool,
        on_enter: impl FnOnce(&mut World, ObjectId, i32, bool),
    ) -> EnterOutcome {
        if let Some((instance_id, world_template)) = self
            .player_instance_world(player)
            .map(|w| (w.instance_id, w.template_id))
        {
            if world_template != template_id {
                let count = self.instance(instance_id).map_or(0, |i| i.players.len());
                if count > 0 {
                    self.send(
                        player,
                        &SystemMessage::new(
                            SystemMessageId::YouHaveEnteredAnotherInstanceZoneThereforeYouCannotEnterCorrespondingDungeon,
                        ),
                    );
                    return EnterOutcome::OtherInstanceBusy;
                }
                // NOTE(java-quirk): the old world is destroyed (in a second), and the player is
                // still put into it below.
                self.destroy_instance_world(instance_id);
            }
            on_enter(self, player, instance_id, false);
            if self
                .instance(instance_id)
                .is_some_and(|i| i.is_remove_buff_enabled())
            {
                self.instance_remove_buffs_of(player, instance_id);
            }
            self.teleport_to_enter_location(player, instance_id);
            return EnterOutcome::Existing(instance_id);
        }
        if !check_conditions(self, player) {
            return EnterOutcome::Refused;
        }
        let instance_id = self.create_dynamic_instance(template_id);
        let template = self.instance(instance_id).map_or(-1, |i| i.template_id);
        self.add_instance_world(InstanceWorld::new(instance_id, template));
        on_enter(self, player, instance_id, true);
        if self
            .instance(instance_id)
            .is_some_and(|i| i.reenter_type == InstanceReenterType::OnEnter)
        {
            self.handle_reenter_time(instance_id);
        }
        if self
            .instance(instance_id)
            .is_some_and(|i| i.is_remove_buff_enabled())
        {
            self.instance_remove_buffs(instance_id);
        }
        self.teleport_to_enter_location(player, instance_id);
        EnterOutcome::New(instance_id)
    }

    /// `teleportPlayer(player, instance.getEnterLocs().stream().findAny().get(), instanceId,
    /// false)` when the instance has enter locations.
    fn teleport_to_enter_location(&mut self, player: ObjectId, instance_id: i32) {
        // `findAny()` on the list of `<spawnPoints>`: the first one.
        let Some((x, y, z)) = self
            .instance(instance_id)
            .and_then(|i| i.enter_locations.first().copied())
        else {
            return;
        };
        self.tele_to_location(player, TeleportTarget::at(x, y, z).with_instance(instance_id));
    }

    /// Java `finishInstance(world)`: with `DefaultFinishTime`.
    pub fn finish_instance(&mut self, instance_id: i32) {
        let duration = self.instances.settings.instance_finish_time_ms;
        self.finish_instance_in(instance_id, duration);
    }

    /// Java `finishInstance(world, duration)`: the re-entry time of an `ON_FINISH` template, then
    /// the end: at once for 0, after `duration_ms` (and no grace for an empty instance) above.
    pub fn finish_instance_in(&mut self, instance_id: i32, duration_ms: i32) {
        let Some(reenter_type) = self.instance(instance_id).map(|i| i.reenter_type) else {
            return;
        };
        if reenter_type == InstanceReenterType::OnFinish {
            self.handle_reenter_time(instance_id);
        }
        if duration_ms == 0 {
            self.destroy_instance(instance_id);
        } else if duration_ms > 0 {
            self.set_instance_duration(instance_id, duration_ms);
            self.set_instance_empty_destroy_time(instance_id, 0);
        }
    }

    /// Java `handleReenterTime(world)`: the earliest of the template's resets (a fixed delay
    /// wins at once), written for every allowed player.
    pub fn handle_reenter_time(&mut self, instance_id: i32) {
        let Some(data) = self.instance(instance_id).map(|i| i.reenter_data.clone()) else {
            return;
        };
        let now = self.instance_now_ms();
        let offset = l2_util::time::local_offset_millis(now);
        let mut earliest: i64 = -1;
        for entry in data {
            match entry {
                InstanceReenterTime::Delay(time) => {
                    if time > 0 {
                        earliest = now + time;
                        break;
                    }
                }
                InstanceReenterTime::At { day, hour, minute } => {
                    let Some(at) = next_reset_time(now, offset, day, hour, minute) else {
                        warn!("Instance {instance_id}: the reset on {day:?} never comes; skipped.");
                        continue;
                    };
                    if earliest == -1 || at < earliest {
                        earliest = at;
                    }
                }
            }
        }
        if earliest > 0 {
            self.set_reenter_time(instance_id, earliest);
        }
    }

    /// Java `setReenterTime(world, time)`: the time for every allowed player who is in the world,
    /// and the message to those online.
    pub fn set_reenter_time(&mut self, instance_id: i32, time: i64) {
        let Some((template_id, allowed)) = self
            .instance_world(instance_id)
            .map(|w| (w.template_id, w.allowed.clone()))
        else {
            return;
        };
        let name = self.instance_name(template_id);
        for player in allowed {
            // `getAllowed()`: only the players `World.getPlayer` finds.
            if !self.players.contains_key(&player) {
                continue;
            }
            self.set_instance_time(player, template_id, time);
            if self.sinks.contains_key(&player) || self.is_offline_trader(player) {
                self.send(
                    player,
                    &SystemMessage::new(
                        SystemMessageId::InstantZoneS1SEntryHasBeenRestrictedYouCanCheckTheNextPossibleEntryTimeByUsingTheCommandInstancezone,
                    )
                    .with(SystemMessageParam::Text(name.clone())),
                );
            }
        }
    }

    /// Java `handleRemoveBuffs(world)`: every allowed player who is in the world.
    pub fn instance_remove_buffs(&mut self, instance_id: i32) {
        let allowed = self
            .instance_world(instance_id)
            .map(|w| w.allowed.clone())
            .unwrap_or_default();
        for player in allowed {
            if self.players.contains_key(&player) {
                self.instance_remove_buffs_of(player, instance_id);
            }
        }
    }

    /// Java `handleRemoveBuffs(player, world)`: by the template's `removeBuffs` type, the player's
    /// and the summon's buffs go (all, all but the listed skills, or only the listed ones).
    fn instance_remove_buffs_of(&mut self, player: ObjectId, instance_id: i32) {
        let Some((kind, exceptions)) = self
            .instance(instance_id)
            .map(|i| (i.remove_buff_type, i.buff_exceptions.clone()))
        else {
            return;
        };
        let summon = self.summon_of(player);
        match kind {
            InstanceRemoveBuffType::All => {
                self.stop_all_effects_except_those_that_last_through_death(player);
                if let Some(summon) = summon {
                    self.stop_all_effects_except_those_that_last_through_death(summon);
                }
            }
            InstanceRemoveBuffType::Whitelist | InstanceRemoveBuffType::Blacklist => {
                let whitelist = kind == InstanceRemoveBuffType::Whitelist;
                for owner in std::iter::once(player).chain(summon) {
                    let buffs: Vec<(ObjectId, i32)> = self
                        .effect_list(owner)
                        .map(|l| l.buffs().iter().map(|b| (b.effected, b.skill().id)).collect())
                        .unwrap_or_default();
                    for (effected, skill_id) in buffs {
                        // Whitelist: everything not listed goes; blacklist: what is listed goes.
                        if exceptions.contains(&skill_id) != whitelist {
                            self.stop_skill_effects(effected, SkillFinishType::Removed, skill_id);
                        }
                    }
                }
            }
            InstanceRemoveBuffType::None => {}
        }
    }
}

/// The `Calendar` part of `handleReenterTime` for one `<reset day hour minute>`: today at
/// `hour:minute:00` local time (the milliseconds of now are kept, `Calendar.set` does not clear
/// them), a day later when that is not after now, then the next `day` of the week. `offset_ms` is
/// the local zone's offset (D-48), taken at now for the whole computation.
///
/// NOTE(java-quirk): Java compares `Calendar.DAY_OF_WEEK` (Sunday 1 .. Saturday 7) with
/// `DayOfWeek.getValue() + 1` (Monday 2 .. Sunday 8): Monday to Saturday match, a Sunday reset
/// never does and Java's loop does not end. `None` here for that case.
pub(crate) fn next_reset_time(
    now_utc: i64,
    offset_ms: i64,
    day: Option<DayOfWeek>,
    hour: i32,
    minute: i32,
) -> Option<i64> {
    let local_now = now_utc + offset_ms;
    let day_start = local_now.div_euclid(DAY_MS) * DAY_MS;
    let millis = local_now.rem_euclid(1000);
    // `set(HOUR_OF_DAY, hour)`, `set(MINUTE, minute)`, `set(SECOND, 0)`; the calendar is lenient,
    // so -1 (an attribute that is not there) rolls back into the hour or day before.
    let mut local = day_start + i64::from(hour) * 3_600_000 + i64::from(minute) * 60_000 + millis;
    if local - offset_ms <= now_utc {
        local += DAY_MS;
    }
    if let Some(day) = day {
        let wanted = day.value() + 1;
        let mut tries = 0;
        while calendar_day_of_week(local) != wanted {
            local += DAY_MS;
            tries += 1;
            if tries > 7 {
                return None;
            }
        }
    }
    Some(local - offset_ms)
}

/// `Calendar.DAY_OF_WEEK` of a local millisecond time: Sunday 1 to Saturday 7 (1970-01-01 was a
/// Thursday, 5).
fn calendar_day_of_week(local_ms: i64) -> i32 {
    let days = local_ms.div_euclid(DAY_MS);
    i32::try_from((days + 4).rem_euclid(7)).unwrap_or(0) + 1
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 2026-10-28 (a Wednesday) 10:15:20.250 UTC.
    const WED: i64 = 1_793_182_520_250;

    #[test]
    fn the_day_of_the_week() {
        assert_eq!(calendar_day_of_week(0), 5, "1970-01-01, a Thursday");
        assert_eq!(calendar_day_of_week(WED), 4, "Wednesday");
    }

    #[test]
    fn a_daily_reset_is_today_or_tomorrow() {
        let today_630 = WED - (10 * 3_600_000 + 15 * 60_000 + 20_000) + 6 * 3_600_000 + 30 * 60_000;
        // 06:30 has passed: tomorrow, with the milliseconds of now.
        assert_eq!(next_reset_time(WED, 0, None, 6, 30), Some(today_630 + DAY_MS));
        // 12:00 is still to come today.
        let noon = today_630 + 5 * 3_600_000 + 30 * 60_000;
        assert_eq!(next_reset_time(WED, 0, None, 12, 0), Some(noon));
        // Local time two hours ahead: 06:30 local is 04:30 UTC.
        assert_eq!(
            next_reset_time(WED, 2 * 3_600_000, None, 6, 30),
            Some(today_630 + DAY_MS - 2 * 3_600_000)
        );
    }

    #[test]
    fn a_weekly_reset_waits_for_its_day() {
        let today_630 = WED - (10 * 3_600_000 + 15 * 60_000 + 20_000) + 6 * 3_600_000 + 30 * 60_000;
        // Wednesday 06:30 has passed: next Wednesday.
        assert_eq!(
            next_reset_time(WED, 0, Some(DayOfWeek::Wednesday), 6, 30),
            Some(today_630 + 7 * DAY_MS)
        );
        assert_eq!(
            next_reset_time(WED, 0, Some(DayOfWeek::Saturday), 6, 30),
            Some(today_630 + 3 * DAY_MS)
        );
        assert_eq!(
            next_reset_time(WED, 0, Some(DayOfWeek::Monday), 6, 30),
            Some(today_630 + 5 * DAY_MS)
        );
        // Java never finds a Sunday.
        assert_eq!(next_reset_time(WED, 0, Some(DayOfWeek::Sunday), 6, 30), None);
    }
}
