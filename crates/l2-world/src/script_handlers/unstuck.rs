//! The user commands `/unstuck` (52) and `/mybirthday` (126).
// Java: dist/game/data/scripts/handlers/chat/commands/user/Unstuck.java
// Java: dist/game/data/scripts/handlers/chat/commands/user/MyBirthday.java
// Java: entity/actor/Creature.java (forceIsCasting)
//
// `/unstuck` is the Soul of Escape animation without a scroll: the long escape skill (2099, five
// minutes; 2100 for a GM, one second) is cast, or, when the interval is not the retail 300 s,
// the animation is faked and a timer teleports to the nearest town.

use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{
    ActionFailed, MagicSkillUse, SetupGauge, SkillPosition, SystemMessage, SystemMessageParam,
};
use l2_util::ObjectId;

use crate::effects::SkillKey;
use crate::teleport::TeleportWhereType;
use crate::timer::{TimerKind, TimerWheel};
use crate::world::World;

/// `Unstuck.COMMAND_IDS`.
pub const UNSTUCK_COMMAND: i32 = 52;
/// `MyBirthday.COMMAND_IDS`.
pub const MY_BIRTHDAY_COMMAND: i32 = 126;

/// The Soul of Escape skill whose animation the fake escape shows.
const SOE_SKILL: i32 = 1050;
/// "5 minutes escape" and "1 second escape".
const ESCAPE_SKILL: SkillKey = SkillKey { id: 2099, level: 1 };
const GM_ESCAPE_SKILL: SkillKey = SkillKey { id: 2100, level: 1 };

/// `GameTimeTaskManager.MILLIS_IN_TICK`.
const MILLIS_IN_TICK: i32 = 100;

/// The timers of the script handlers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScriptHandlerTimer {
    /// Java `Unstuck.EscapeFinalizer.run()`.
    UnstuckEscape { player: ObjectId },
}

/// What the script handlers keep in the world.
#[derive(Debug, Default)]
pub struct ScriptHandlerWorld {
    pub(crate) settings: ScriptHandlerSettings,
}

/// The config values the script handlers read. `Default` is the Java default.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScriptHandlerSettings {
    /// `PlayerConfig.UNSTUCK_INTERVAL` (`UnstuckInterval`, seconds).
    pub unstuck_interval: i32,
    /// `GeneralConfig.ALLOW_WEAR` (`AllowWear`).
    pub allow_wear: bool,
    /// `GeneralConfig.ALLOW_RENTPET` (`AllowRentPet`).
    pub allow_rentpet: bool,
    /// `FindPvpConfig.ENABLE_FIND_PVP` (`EnableFindPvP`).
    pub enable_find_pvp: bool,
}

impl Default for ScriptHandlerSettings {
    fn default() -> Self {
        Self {
            unstuck_interval: 300,
            allow_wear: true,
            allow_rentpet: false,
            enable_find_pvp: false,
        }
    }
}

/// The civil date of a day count since 1970-01-01 (Hinnant's algorithm).
fn civil_from_millis(millis: i64) -> (i32, u8, u8) {
    let z = millis.div_euclid(86_400_000) + 719_468;
    let era = z.div_euclid(146_097);
    let day_of_era = z.rem_euclid(146_097);
    let year_of_era = (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let mp = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    (year as i32, month as u8, day as u8)
}

impl World {
    /// Installs `UnstuckInterval`.
    pub fn set_script_handler_settings(&mut self, settings: ScriptHandlerSettings) {
        self.script_handlers.settings = settings;
    }

    /// The commands of this module: `true` when `command` is one of them.
    pub(crate) fn user_command_scripts(&mut self, id: ObjectId, command: i32) -> bool {
        match command {
            UNSTUCK_COMMAND => {
                self.user_command_unstuck(id);
                true
            }
            MY_BIRTHDAY_COMMAND => {
                self.user_command_my_birthday(id);
                true
            }
            _ => false,
        }
    }

    /// Java `MyBirthday.onCommand`: the date the character was made (`createDate`; Java's default
    /// for a character without one is the moment the object was made).
    fn user_command_my_birthday(&mut self, id: ObjectId) {
        let Some(player) = self.players.get(&id) else {
            return;
        };
        let name = player.data.name.clone();
        let (year, month, day) = player.data.profile.create_date.unwrap_or_else(|| {
            let now = self.world_ms();
            civil_from_millis(now.wrapping_add(l2_util::time::local_offset_millis(now)))
        });
        let message = SystemMessage::new(SystemMessageId::C1SBirthdayIsS3S4S2)
            .with(SystemMessageParam::PlayerName(name))
            .with(SystemMessageParam::Text(year.to_string()))
            .with(SystemMessageParam::Text(month.to_string()))
            .with(SystemMessageParam::Text(day.to_string()));
        self.send(id, &message);
    }

    /// Java `Unstuck.onCommand`.
    fn user_command_unstuck(&mut self, id: ObjectId) -> bool {
        if !self.players.contains_key(&id) {
            return false;
        }
        if self.is_registered_on_event(id) {
            self.send(id, &ActionFailed);
            return false;
        }
        if self.is_jailed(id) {
            self.send_message(id, "You cannot use this function while you are jailed.");
            return false;
        }
        // (`FactionSystemConfig.FACTION_SYSTEM_ENABLED` is off by default and not modelled.)
        let is_gm = self.is_gm(id);
        let interval = self.script_handlers.settings.unstuck_interval;
        let unstuck_timer = if is_gm { 1000 } else { interval.wrapping_mul(1000) };
        if self.is_in_olympiad_mode(id) {
            self.send(
                id,
                &SystemMessage::new(SystemMessageId::YouCannotUseThatSkillInAGrandOlympiadMatch),
            );
            return false;
        }
        if self.is_casting_now(id)
            || self.is_movement_disabled(id)
            || self.is_muted(id)
            || self.is_alike_dead(id)
            || self.in_observer_mode(id)
            || self.is_combat_flag_equipped(id)
        {
            return false;
        }

        // `forceIsCasting(ticks + unstuckTimer / MILLIS_IN_TICK)`.
        self.set_casting_now(id, true);
        let end = self.game_ticks().wrapping_add(unstuck_timer / MILLIS_IN_TICK);
        self.cast_state_mut(id).cast_interrupt_time = end.wrapping_sub(4);

        let table = self.skill_table();
        let escape_exists = table
            .as_ref()
            .is_some_and(|t| t.get(ESCAPE_SKILL.id, ESCAPE_SKILL.level).is_some());
        let gm_escape_exists = table
            .as_ref()
            .is_some_and(|t| t.get(GM_ESCAPE_SKILL.id, GM_ESCAPE_SKILL.level).is_some());
        if is_gm {
            if gm_escape_exists {
                self.do_cast(id, GM_ESCAPE_SKILL);
                return true;
            }
            self.send_message(id, "You use Escape: 1 second.");
        } else if interval == 300 && escape_exists {
            // If unstuck is default (5min), send retail system message.
            self.send(
                id,
                &SystemMessage::new(
                    SystemMessageId::YouAreStuckYouWillBeTransportedToTheNearestVillageInFiveMinutes,
                ),
            );
            self.stop_move(id, None);
            self.abort_cast(id);
            self.do_cast(id, ESCAPE_SKILL);
            return true;
        } else if interval > 100 {
            self.send_message(
                id,
                &format!("You use Escape: {} minutes.", unstuck_timer / 60_000),
            );
        } else {
            self.send_message(id, &format!("You use Escape: {} seconds.", unstuck_timer / 1000));
        }

        self.set_intention_idle(id);

        // SoE Animation section.
        self.set_target(id, Some(id));
        self.disable_all_skills(id);
        if let Some((x, y, z)) = self.position_of(id) {
            let at = SkillPosition { x, y, z };
            self.broadcast_creature_packet(
                id,
                &MagicSkillUse {
                    creature_id: id.0,
                    target_id: id.0,
                    skill_id: SOE_SKILL,
                    skill_level: 1,
                    hit_time: unstuck_timer,
                    reuse_delay: 0,
                    creature: at,
                    ground_location: None,
                    target: at,
                },
            );
        }
        self.send(id, &SetupGauge::new(id.0, SetupGauge::BLUE, unstuck_timer));
        // End SoE Animation section.

        // Continue execution later: `setSkillCast(ThreadPool.schedule(new EscapeFinalizer(player),
        // unstuckTimer))`.
        let due = self.tick + TimerWheel::ms_to_ticks(u64::try_from(unstuck_timer).unwrap_or(0));
        let timer = self.timers.schedule_at(
            due,
            TimerKind::ScriptHandler(ScriptHandlerTimer::UnstuckEscape { player: id }),
        );
        self.cast_state_mut(id).skill_cast = Some(timer);
        true
    }

    /// The timers of this module.
    pub(crate) fn on_script_handler_timer(&mut self, timer: ScriptHandlerTimer) {
        match timer {
            ScriptHandlerTimer::UnstuckEscape { player } => self.unstuck_escape_finalizer(player),
        }
    }

    /// Java `Unstuck.EscapeFinalizer.run()`.
    fn unstuck_escape_finalizer(&mut self, player: ObjectId) {
        if !self.players.contains_key(&player) {
            return;
        }
        // The task is over; `setSkillCast` keeps the finished future in Java.
        if let Some(state) = self.cast.creatures.get_mut(&player) {
            state.skill_cast = None;
        }
        if self.is_dead(player) {
            return;
        }
        self.set_in_7s_dungeon(player, false);
        self.enable_all_skills(player);
        self.set_casting_now(player, false);
        self.set_instance_id(player, 0);
        self.tele_to_location_where(player, TeleportWhereType::Town);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn civil_dates() {
        assert_eq!(civil_from_millis(0), (1970, 1, 1));
        assert_eq!(civil_from_millis(951_782_400_000), (2000, 2, 29));
        assert_eq!(civil_from_millis(-86_400_000), (1969, 12, 31));
    }
}
