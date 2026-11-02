//! `config/Custom/Captcha.ini`: after a number of monster kills the player must read a code from a
//! picture, or is punished (moved to town, kicked, jailed or banned).
// Java: gameserver/managers/CaptchaManager.java
// Java: gameserver/mechanics/captcha/Captcha.java (the code and the id of the picture)
// Java: gameserver/entity/actor/Creature.java (`doDie`: `updateCounter`)
// Java: gameserver/network/clientpackets/RequestBypassToServer.java (`report...` -> `analyseBypass`)
//
// Differences, all in how the pictures are made: Java renders 1000 pictures when the server starts
// and hands them out in turn; here one picture is made per challenge (same code alphabet, same id
// range). Java also sends a first `PledgeCrest` with a different random id and picture before the
// one the window shows; that packet is never referred to and is not sent here.
// The kills of a player who is in auto play are not counted, as in Java (M9-L01).

use std::collections::HashMap;

use l2_protocol_game::server::{ChatType, CreatureSay, NpcHtmlMessage, PledgeCrest, SayBody, SenderField};
use l2_util::ObjectId;
use tracing::warn;

use super::captcha_image::{LETTERS, captcha_dds};
use super::misc::CustomMiscTimer;
use crate::command::LeaveReason;
use crate::punishments::{NewPunishment, PunishmentAffect, PunishmentType};
use crate::script::QuestSound;
use crate::teleport::TeleportWhereType;
use crate::timer::{TimerId, TimerKind, TimerWheel};
use crate::world::{World, unix_ms_now};

/// Java `MAX_JAIL_TIME`: the longest (doubled) jail time, 24 hours in minutes.
const MAX_JAIL_TIME: i32 = 24 * 60;
/// Java `WINDOW_DELAY`: seconds between two shows of the first window.
const WINDOW_DELAY: i32 = 3;
/// Java `CAPTCHA_MIN_ID` and `CAPTCHA_MAX_ID`.
const CAPTCHA_MIN_ID: i32 = 19000;
const CAPTCHA_MAX_ID: i32 = 25000;

/// Java `CaptchaManager.PlayerData`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ChallengeData {
    pub first_window: bool,
    pub captcha_id: i32,
    pub captcha_text: String,
}

/// The maps of `CaptchaManager`.
#[derive(Debug, Default)]
pub struct CaptchaState {
    jail_count: HashMap<ObjectId, i32>,
    retries: HashMap<ObjectId, i32>,
    last_jail_time: HashMap<ObjectId, i64>,
    monster_counter: HashMap<ObjectId, i32>,
    last_kill_time: HashMap<ObjectId, i64>,
    begin_validation: HashMap<ObjectId, TimerId>,
    pub(crate) validation: HashMap<ObjectId, ChallengeData>,
}

impl World {
    /// `CaptchaManager.isAlredyInReportMode(player)`.
    pub fn captcha_is_in_report_mode(&self, player: ObjectId) -> bool {
        self.custom_misc.captcha.validation.contains_key(&player)
    }

    /// `Creature.doDie`: `CaptchaManager.updateCounter(killer, this)`.
    pub(crate) fn captcha_update_counter(&mut self, killer: Option<ObjectId>, victim: ObjectId) {
        let settings = self.settings.custom_misc.captcha.clone();
        if !settings.enable {
            return;
        }
        let Some(killer) = killer else {
            return;
        };
        if !self.is_player(killer) || !self.is_monster(victim) {
            return;
        }
        // "Check if auto-play is enabled and player is auto-playing."
        if self.auto_play_enabled() && self.is_auto_playing(killer) {
            return;
        }
        if self.custom_misc.captcha.validation.contains_key(&killer) {
            return;
        }
        let state = &mut self.custom_misc.captcha;
        if settings.kill_counter_reset {
            let now = unix_ms_now();
            let previous = state.last_kill_time.get(&killer).copied().unwrap_or(now);
            if now - previous > i64::from(settings.kill_counter_reset_time_ms) {
                state.monster_counter.insert(killer, 0);
            }
            state.last_kill_time.insert(killer, now);
        }
        let count = state.monster_counter.get(&killer).map_or(1, |c| c + 1);
        let next = self.rng().below(settings.kill_counter_randomization);
        if settings.kill_counter.wrapping_add(next) < count {
            self.captcha_validation_tasks(killer);
            self.custom_misc.captcha.monster_counter.remove(&killer);
        } else {
            self.custom_misc.captcha.monster_counter.insert(killer, count);
        }
    }

    /// `CaptchaManager.validationTasks(player)`: makes the challenge, sends the picture and starts
    /// the countdown and the time limit.
    pub fn captcha_validation_tasks(&mut self, player: ObjectId) {
        let settings = self.settings.custom_misc.captcha.clone();
        let mut text = ['A'; 5];
        for letter in &mut text {
            *letter = LETTERS[self.rng().below(LETTERS.len() as i32) as usize];
        }
        let captcha_id = self.rng().get(CAPTCHA_MIN_ID, CAPTCHA_MAX_ID);
        let dds = captcha_dds(&text, self.rng());
        self.send(
            player,
            &PledgeCrest {
                crest_id: captcha_id,
                data: dds,
            },
        );
        self.custom_misc.captcha.validation.insert(
            player,
            ChallengeData {
                first_window: true,
                captcha_id,
                captcha_text: text.iter().collect(),
            },
        );

        let limit = u64::try_from(settings.validation_time)
            .unwrap_or(0)
            .saturating_mul(1000);
        let due = self.tick + TimerWheel::ms_to_ticks(limit);
        let timer = self
            .timers
            .schedule_at(due, TimerKind::CustomMisc(CustomMiscTimer::CaptchaReport(player)));
        // `ThreadPool.schedule(new countdown(player, VALIDATION_TIME), 0)`.
        let due = self.tick + TimerWheel::ms_to_ticks(0);
        self.timers.schedule_at(
            due,
            TimerKind::CustomMisc(CustomMiscTimer::CaptchaCountdown(
                player,
                settings.validation_time,
            )),
        );
        self.custom_misc.captcha.begin_validation.insert(player, timer);
    }

    /// `CaptchaManager.preValidationWindow(player)`.
    fn captcha_pre_validation_window(&mut self, player: ObjectId) {
        self.custom_misc.captcha.retries.insert(player, 0);
        let html = concat!(
            "<html>",
            "<title>Bot Protection</title>",
            "<body><center><br><br><img src=\"L2UI_CH3.herotower_deco\" width=\"256\" height=\"32\">",
            "<br><br><font color=\"a2a0a2\">If such window appears it means server suspect,<br1>that you may using cheating software.</font>",
            "<br><br><font color=\"b09979\">If given answer results are incorrect or no action is made character will be punished instantly.</font>",
            "<br><br><button value=\"CONTINUE\" action=\"bypass report_continue\" width=\"100\" height=\"27\" back=\"L2UI_CT1.Button_DF_Down\" fore=\"L2UI_CT1.Button_DF\">",
            "</center></body>",
            "</html>"
        );
        self.captcha_send_html(player, html);
    }

    /// `CaptchaManager.validationWindow(player)`.
    fn captcha_validation_window(&mut self, player: ObjectId) {
        let Some(data) = self.custom_misc.captcha.validation.get(&player).cloned() else {
            return;
        };
        let server_id = self.settings.custom_misc.captcha.server_id;
        let html = format!(
            "<html><title>Bot Protection</title><body><center><br><br><img src=\"L2UI_CH3.herotower_deco\" width=\"256\" height=\"32\">\
             <br><br><font color=\"a2a0a2\">in order to prove you are a human being<br1>you've to</font> <font color=\"b09979\">enter the code from picture:</font>\
             <br><br><img src=\"Crest.crest_{server_id}_{}\" width=256 height=64></td></tr>\
             <br>\
             <br><br><edit var=\"answer\" width=110>\
             <br><button value=\"Confirm\" action=\"bypass -h report_ $answer\" width=\"100\" height=\"27\" back=\"L2UI_CT1.Button_DF_Down\" fore=\"L2UI_CT1.Button_DF\"><br/>\
             </center></body></html>",
            data.captcha_id
        );
        self.captcha_send_html(player, &html);
        self.send(player, &QuestSound::SkillsoundHorror1.packet());
    }

    /// `CaptchaManager.punishmentWindow(player)`.
    fn captcha_punishment_window(&mut self, player: ObjectId) {
        let html = concat!(
            "<html>",
            "<title>Bot Protection</title>",
            "<body><center><br><br><img src=\"L2UI_CH3.herotower_deco\" width=\"256\" height=\"32\">",
            "<br><br><font color=\"a2a0a2\">If such window appears, it means character haven't passed through prevention system.",
            "<br><br><font color=\"b09979\">In such case character get moved to nearest town.</font>",
            "</center></body>",
            "</html>"
        );
        self.captcha_send_html(player, html);
    }

    /// `new NpcHtmlMessage(1)` + `setHtml` + `sendPacket`.
    fn captcha_send_html(&mut self, player: ObjectId, html: &str) {
        if let Some(message) = NpcHtmlMessage::new(1, html) {
            self.send_html(player, &message);
        }
    }

    /// `AdminData.broadcastToGMs(new CreatureSay(null, ChatType.ALLIANCE, "Punishment", text))`.
    fn captcha_broadcast_to_gms(&self, text: &str) {
        let mut gms: Vec<ObjectId> = self.players.keys().copied().filter(|p| self.is_gm(*p)).collect();
        gms.sort_unstable_by_key(|p| p.0);
        let say = CreatureSay {
            sender_object_id: 0,
            chat_type: ChatType::Alliance,
            sender: SenderField::Name("Punishment".to_owned()),
            message_id: -1,
            body: SayBody::Text(text.to_owned()),
        };
        for gm in gms {
            self.send(gm, &say);
        }
    }

    /// `CaptchaManager.banPunishment(player)`.
    fn captcha_ban_punishment(&mut self, player: ObjectId) {
        let punishment = self.settings.custom_misc.captcha.punishment;
        self.custom_misc.captcha.validation.remove(&player);
        if let Some(timer) = self.custom_misc.captcha.begin_validation.remove(&player) {
            self.timers.cancel(timer);
        }
        let name = self
            .players
            .get(&player)
            .map(|p| p.data.name.clone())
            .unwrap_or_default();
        let who = self.player_text(player);
        match punishment {
            // Move the character to the closest village.
            0 => {
                self.stop_move(player, None);
                self.tele_to_location_where(player, TeleportWhereType::Town);
                self.captcha_punishment_window(player);
                self.send(player, &QuestSound::ItemsoundQuestGiveup.packet());
                self.captcha_broadcast_to_gms(&format!("{name} has been teleported."));
                warn!("{who}[TELEPORT] Wrong Captcha maybe boting?");
            }
            // Kick the character from the server.
            1 => {
                if self.players.contains_key(&player) {
                    self.leave(player, LeaveReason::Logout);
                    self.captcha_broadcast_to_gms(&format!("{name} has been kicked."));
                    warn!("{who}[KICK] Wrong Captcha maybe boting?");
                    // Java returns here: no "code doesn't match" message.
                    return;
                }
            }
            // Put the character to jail.
            2 => {
                self.captcha_apply_jail(player);
                self.send(player, &QuestSound::EtcsoundElrokiSongFull.packet());
                self.captcha_broadcast_to_gms(&format!("{name} has been jailed."));
                warn!("{who}[JAIL] Wrong Captcha maybe boting?");
            }
            // Ban the character from the server.
            3 => {
                // `changeAccessLevel`: only for a player who is online.
                if self.players.contains_key(&player) {
                    self.start_punishment(NewPunishment::new(
                        player.0,
                        PunishmentAffect::Character,
                        PunishmentType::Ban,
                        0,
                        "Bot Protection",
                        "System",
                    ));
                }
                self.captcha_broadcast_to_gms(&format!("{name} has been banned."));
                warn!("{who}[BAN] Wrong Captcha maybe boting?");
            }
            _ => {}
        }
        self.send_message(player, "Unfortunately, code doesn't match.");
    }

    /// `CaptchaManager.applyJailPunishment(player)`.
    fn captcha_apply_jail(&mut self, player: ObjectId) {
        if !self.players.contains_key(&player) {
            return;
        }
        let settings = self.settings.custom_misc.captcha.clone();
        let now = unix_ms_now();
        let jail_time;
        if settings.double_jail_time {
            let state = &mut self.custom_misc.captcha;
            let last = state.last_jail_time.get(&player).copied().unwrap_or(0);
            let previous_count = state.jail_count.get(&player).copied().unwrap_or(0);
            // Use the multiplier if enabled.
            jail_time = if now - last >= 24 * 60 * 60 * 1000 {
                settings.jail_time
            } else {
                // Double the previous jail time, capped at the maximum.
                // `(int) MathUtil.pow(2, count) * JAIL_TIME`: the power is a double cut to `int`.
                let power = 2f64
                    .powi(previous_count)
                    .clamp(f64::from(i32::MIN), f64::from(i32::MAX)) as i32;
                power.wrapping_mul(settings.jail_time).min(MAX_JAIL_TIME)
            };
            *state.jail_count.entry(player).or_insert(0) += 1;
            // A message about the longer next time, two seconds later.
            let due = self.tick + TimerWheel::ms_to_ticks(2000);
            self.timers.schedule_at(
                due,
                TimerKind::CustomMisc(CustomMiscTimer::CaptchaJailWarning(player)),
            );
        } else {
            jail_time = settings.jail_time;
        }
        let punishment_time = if jail_time == 0 {
            0
        } else {
            now + i64::from(jail_time) * 60_000
        };
        self.start_punishment(NewPunishment::new(
            player.0,
            PunishmentAffect::Character,
            PunishmentType::Jail,
            punishment_time,
            "Bot Protection",
            "System",
        ));
        if jail_time != 0 {
            self.custom_misc.captcha.last_jail_time.insert(player, now);
        }
    }

    /// `CaptchaManager.analyseBypass(command, player)`: the `report...` bypass.
    pub(crate) fn captcha_analyse_bypass(&mut self, player: ObjectId, command: &str) {
        let Some(data) = self.custom_misc.captcha.validation.get(&player).cloned() else {
            return;
        };
        // `command.substring(command.indexOf("_") + 1)`; no `_` is index -1, the whole command.
        let params = command.split_once('_').map_or(command, |(_, rest)| rest);
        if params.starts_with("continue") {
            self.captcha_validation_window(player);
            if let Some(d) = self.custom_misc.captcha.validation.get_mut(&player) {
                d.first_window = false;
            }
            return;
        }
        // `trim().equalsIgnoreCase(text)`
        let answer = params.trim_matches(|c: char| c <= ' ');
        if !answer.eq_ignore_ascii_case(&data.captcha_text) {
            let retries = self
                .custom_misc
                .captcha
                .retries
                .get(&player)
                .copied()
                .unwrap_or(0)
                + 1;
            self.custom_misc.captcha.retries.insert(player, retries);
            let attempts = self.settings.custom_misc.captcha.captcha_attempts;
            if retries >= attempts {
                self.captcha_ban_punishment(player);
            } else {
                self.send_message(
                    player,
                    &format!(
                        "Incorrect code. You have {} more attempt(s) left.",
                        attempts - retries
                    ),
                );
                // Provide another chance.
                self.captcha_validation_window(player);
                self.send(player, &QuestSound::ItemsoundBrokenKey.packet());
            }
        } else {
            self.send_message(player, "Congratulations, code matches!");
            self.custom_misc.captcha.validation.remove(&player);
            if let Some(timer) = self.custom_misc.captcha.begin_validation.remove(&player) {
                self.timers.cancel(timer);
            }
            self.send(player, &QuestSound::ItemsoundSowSuccess.packet());
        }
    }

    /// `ReportCheckTask.run`: the time to answer is over.
    pub(crate) fn on_captcha_report(&mut self, player: ObjectId) {
        if self.custom_misc.captcha.validation.contains_key(&player) {
            self.captcha_ban_punishment(player);
        }
    }

    /// `countdown.run`: one second of the countdown for a player who is online.
    pub(crate) fn on_captcha_countdown(&mut self, player: ObjectId, time: i32) {
        if !self.players.contains_key(&player) {
            return;
        }
        let first_window = self
            .custom_misc
            .captcha
            .validation
            .get(&player)
            .is_some_and(|d| d.first_window);
        if first_window && time % WINDOW_DELAY == 0 {
            self.captcha_pre_validation_window(player);
        }
        match time {
            300 | 240 | 180 | 120 | 60 => {
                self.send_message(player, &format!("{} minute(s) to enter the code.", time / 60));
            }
            30 | 10 | 5 | 4 | 3 | 2 | 1 => {
                self.send_message(player, &format!("{time} second(s) to enter the code!"));
            }
            _ => {}
        }
        if time > 1 && self.custom_misc.captcha.validation.contains_key(&player) {
            let due = self.tick + TimerWheel::ms_to_ticks(1000);
            self.timers.schedule_at(
                due,
                TimerKind::CustomMisc(CustomMiscTimer::CaptchaCountdown(player, time - 1)),
            );
        }
    }

    /// The code the player has to enter, while a challenge is open (for tools and tests).
    pub fn captcha_code_of(&self, player: ObjectId) -> Option<String> {
        self.custom_misc
            .captcha
            .validation
            .get(&player)
            .map(|d| d.captcha_text.clone())
    }

    /// `CaptchaManager.captchaSuccessfull(player)`.
    pub fn captcha_successful(&mut self, player: ObjectId) {
        self.custom_misc.captcha.validation.remove(&player);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_challenge_is_remembered_with_its_text() {
        let mut state = CaptchaState::default();
        state.validation.insert(
            ObjectId(1),
            ChallengeData {
                first_window: true,
                captcha_id: 20000,
                captcha_text: "ABCDE".to_owned(),
            },
        );
        assert!(state.validation.contains_key(&ObjectId(1)));
    }
}
