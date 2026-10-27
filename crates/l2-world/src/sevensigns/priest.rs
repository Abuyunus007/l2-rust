//! The priests of the Seven Signs: `SignsPriest` (the Mammon NPCs are of this type too),
//! `DawnPriest` and `DuskPriest`. Joining a cabal, the seal stones, Ancient Adena, the record.
// Java: entity/actor/instance/SignsPriest.java (onBypassFeedback, showChatWindow(player, value,
//       suffix, isDescription))
// Java: entity/actor/instance/{DawnPriest,DuskPriest}.java (onBypassFeedback, showChatWindow)
// Java: mechanics/sevensigns/SevenSigns.java (setPlayerInfo, getAncientAdenaReward,
//       addPlayerStoneContrib)
// Java: entity/actor/Player.java (addAncientAdena, reduceAncientAdena)

use l2_data::ClassId;
use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{ActionFailed, NpcHtmlMessage, SystemMessage, SystemMessageParam};
use l2_util::ObjectId;
use tracing::warn;

use super::{
    ANCIENT_ADENA_ID, BLUE_CONTRIB_POINTS, CABAL_DAWN, CABAL_DUSK, CABAL_NULL, GREEN_CONTRIB_POINTS,
    RECORD_SEVEN_SIGNS_COST, RECORD_SEVEN_SIGNS_ID, RED_CONTRIB_POINTS, SEAL_AVARICE, SEAL_GNOSIS,
    SEAL_STONE_BLUE_ID, SEAL_STONE_BLUE_VALUE, SEAL_STONE_GREEN_ID, SEAL_STONE_GREEN_VALUE,
    SEAL_STONE_RED_ID, SEAL_STONE_RED_VALUE, SEAL_STRIFE, SEVEN_SIGNS_HTML_PATH, SevenSignsWrite,
    SignsPlayer, cabal_name, cabal_short_name, calc_ancient_adena_reward, calc_contribution_score, seal_name,
};
use crate::outbox::PersistCommand;
use crate::world::World;

/// Which of the three priest classes an NPC is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PriestKind {
    /// `DawnPriest`.
    Dawn,
    /// `DuskPriest`.
    Dusk,
    /// `SignsPriest` itself (the Mammon NPCs): `this instanceof DawnPriest` is false.
    Signs,
}

impl PriestKind {
    fn of(npc_type: &str) -> Option<Self> {
        Some(match npc_type {
            "DawnPriest" => Self::Dawn,
            "DuskPriest" => Self::Dusk,
            "SignsPriest" => Self::Signs,
            _ => return None,
        })
    }

    /// `this instanceof DawnPriest ? "dawn..." : "dusk..."`.
    fn side(self) -> &'static str {
        if self == Self::Dawn { "dawn" } else { "dusk" }
    }
}

/// Java `Integer.parseInt(command.substring(begin, end).trim())`: `None` where Java throws.
fn parse_sub(command: &str, begin: usize, end: Option<usize>) -> Option<i64> {
    let text = match end {
        Some(end) => command.get(begin..end)?,
        None => command.get(begin..)?,
    };
    // `String.trim()` strips every char up to the space.
    text.trim_matches(|c: char| c <= ' ').parse::<i64>().ok()
}

/// `Integer.parseInt(command.substring(begin, end).trim())`: a text that is a long but not an int
/// throws too.
fn parse_int_sub(command: &str, begin: usize, end: Option<usize>) -> Option<i32> {
    parse_sub(command, begin, end).and_then(|v| i32::try_from(v).ok())
}

/// `Integer.parseInt(command.substring(begin, end))` without `trim()`: a space makes Java throw.
fn parse_int_raw(command: &str, begin: usize, end: Option<usize>) -> Option<i32> {
    let text = match end {
        Some(end) => command.get(begin..end)?,
        None => command.get(begin..)?,
    };
    text.parse::<i32>().ok()
}

impl World {
    fn priest_kind(&self, npc: ObjectId) -> Option<PriestKind> {
        let n = self.npcs.get(&npc)?;
        PriestKind::of(&self.npc_world.template(n.npc_id)?.npc_type)
    }

    /// `Npc.showChatWindow(player, filename)`: the page with `%objectId%`, then `ActionFailed`.
    pub(crate) fn ss_show_file(&mut self, npc: ObjectId, player: ObjectId, filename: &str) {
        if let Some(mut html) = self.html_from_file(player, npc.0, filename) {
            html.html = html.html.replace("%objectId%", &npc.0.to_string());
            self.send_html(player, &html);
        }
        self.send(player, &ActionFailed);
    }

    /// `SignsPriest.showChatWindow(player, value, suffix, isDescription)`.
    fn ss_show_signs(
        &mut self,
        npc: ObjectId,
        player: ObjectId,
        value: i64,
        suffix: Option<&str>,
        desc: bool,
    ) {
        let mut filename = format!(
            "{SEVEN_SIGNS_HTML_PATH}{}{value}",
            if desc { "desc_" } else { "signs_" }
        );
        match suffix {
            Some(suffix) => filename.push_str(&format!("_{suffix}.htm")),
            None => filename.push_str(".htm"),
        }
        self.ss_show_file(npc, player, &filename);
    }

    /// The click on a dawn or dusk priest (`DawnPriest.showChatWindow(player)`): `true` when the
    /// NPC is one.
    pub(crate) fn ss_priest_show_chat_window(&mut self, npc: ObjectId, player: ObjectId) -> bool {
        let kind = match self.priest_kind(npc) {
            Some(kind @ (PriestKind::Dawn | PriestKind::Dusk)) => kind,
            _ => return false,
        };
        self.send(player, &ActionFailed);
        let (own, other) = if kind == PriestKind::Dawn {
            (CABAL_DAWN, CABAL_DUSK)
        } else {
            (CABAL_DUSK, CABAL_DAWN)
        };
        let side = kind.side();
        let seal_gnosis_owner = self.ss_seal_owner(SEAL_GNOSIS);
        let player_cabal = self.ss_player_cabal(player.0);
        let validation = self.ss_is_seal_validation_period();
        let results = self.ss_is_comp_results_period();
        let recruit_period = self.ss_current_period();
        let comp_winner = self.ss_cabal_highest_score();
        let page = if player_cabal == own {
            if results {
                "5"
            } else if recruit_period == 0 {
                "6"
            } else if validation {
                if comp_winner == own {
                    if comp_winner != seal_gnosis_owner {
                        "2c"
                    } else {
                        "2a"
                    }
                } else if comp_winner == CABAL_NULL {
                    "2d"
                } else {
                    "2b"
                }
            } else {
                "1b"
            }
        } else if player_cabal == other {
            if validation { "3a" } else { "3b" }
        } else if results {
            "5"
        } else if recruit_period == 0 {
            "6"
        } else if validation {
            if comp_winner == own {
                "4"
            } else if comp_winner == CABAL_NULL {
                "2d"
            } else {
                "2b"
            }
        } else {
            "1a"
        };
        let filename = format!("{SEVEN_SIGNS_HTML_PATH}{side}_priest_{page}.htm");
        if let Some(mut html) = self.html_from_file(player, npc.0, &filename) {
            html.html = html.html.replace("%objectId%", &npc.0.to_string());
            self.send_html(player, &html);
        }
        true
    }

    /// `DawnPriest` / `DuskPriest` / `SignsPriest.onBypassFeedback`: `true` when the NPC is a
    /// priest (the command was handled or refused here).
    pub(crate) fn ss_priest_bypass_feedback(
        &mut self,
        npc: ObjectId,
        player: ObjectId,
        command: &str,
    ) -> bool {
        let Some(kind) = self.priest_kind(npc) else {
            return false;
        };
        // `DawnPriest.onBypassFeedback`: "Chat" shows the priest's own window.
        if kind != PriestKind::Signs && command.starts_with("Chat") {
            self.ss_priest_show_chat_window(npc, player);
            return true;
        }
        if self.players.get(&player).and_then(|p| p.talk.last_folk_npc) != Some(npc) {
            return true;
        }
        if command.starts_with("SevenSignsDesc") {
            match parse_int_raw(command, 15, None) {
                Some(value) => self.ss_show_signs(npc, player, i64::from(value), None, true),
                None => warn!("SignsPriest: bad bypass {command:?} of {player}"),
            }
        } else if command.starts_with("SevenSigns") {
            self.ss_signs_command(npc, player, kind, command);
        } else {
            self.generic_npc_bypass_feedback(npc, player, command);
        }
        true
    }

    /// The `SevenSigns <val> ...` commands.
    fn ss_signs_command(&mut self, npc: ObjectId, player: ObjectId, kind: PriestKind, command: &str) {
        let side = kind.side();
        let ancient_adena_amount = self.inventory_item_count(player, ANCIENT_ADENA_ID);
        // `Integer.parseInt(command.substring(11, 12).trim())`, then two digits when longer.
        let Some(mut val) = parse_int_sub(command, 11, Some(12)) else {
            warn!("SignsPriest: bad bypass {command:?} of {player}");
            return;
        };
        if command.len() > 12 {
            match parse_int_sub(command, 11, Some(13)) {
                Some(v) => val = v,
                None => {
                    warn!("SignsPriest: bad bypass {command:?} of {player}");
                    return;
                }
            }
        }
        let mut cabal = CABAL_NULL;
        if command.len() > 13 {
            cabal = match parse_int_sub(command, 14, Some(15))
                .or_else(|| parse_int_sub(command, 13, Some(14)))
            {
                Some(c) => c,
                None => {
                    // `StringTokenizer(command.trim())`: the second token.
                    match command
                        .split_whitespace()
                        .nth(1)
                        .and_then(|t| t.parse::<i32>().ok())
                    {
                        Some(c) => c,
                        None => {
                            let npc_id = self.npcs.get(&npc).map_or(0, |n| n.npc_id);
                            warn!(
                                "Failed to retrieve cabal from bypass command. NpcId: {npc_id}; Command: {command}"
                            );
                            CABAL_NULL
                        }
                    }
                }
            };
        }
        let class_level = self.players.get(&player).map_or(0, |p| {
            ClassId(u16::try_from(p.data.class_id).unwrap_or(0)).class_level()
        });
        let castle_id = {
            let clan_id = self.clan_id_of(player);
            self.clan(clan_id).map(|c| c.castle_id)
        };
        let settings = self.seven_signs.settings.clone();

        match val {
            // Purchase Record of the Seven Signs
            2 => {
                if !self.validate_capacity(player, 1, false) {
                    self.send(player, &SystemMessage::new(SystemMessageId::YourInventoryIsFull));
                    return;
                }
                // NOTE(java-quirk): `reduceAdena(.., true)` already says "not enough adena", so
                // a poor player is told twice.
                if !self.reduce_adena(player, RECORD_SEVEN_SIGNS_COST, true) {
                    self.send(
                        player,
                        &SystemMessage::new(SystemMessageId::YouDoNotHaveEnoughAdena),
                    );
                    return;
                }
                self.inventory_add_item_by_id(player, RECORD_SEVEN_SIGNS_ID, 1);
                self.send(
                    player,
                    &SystemMessage::new(SystemMessageId::YouHaveEarnedS1)
                        .with(SystemMessageParam::ItemName(RECORD_SEVEN_SIGNS_ID)),
                );
                self.ss_show_signs(npc, player, i64::from(val), Some(side), false);
            }
            // "I want to participate" request
            33 => {
                let old_cabal = self.ss_player_cabal(player.0);
                if old_cabal != CABAL_NULL {
                    self.ss_show_signs(npc, player, 33, Some(&format!("{side}_member")), false);
                    return;
                } else if class_level == 0 {
                    self.ss_show_signs(npc, player, 33, Some(&format!("{side}_firstclass")), false);
                    return;
                } else if cabal == CABAL_DUSK && settings.castle_for_dusk {
                    // Castle owners cannot participate with dusk side.
                    if castle_id.is_some_and(|c| c > 0) {
                        self.ss_show_file(
                            npc,
                            player,
                            &format!("{SEVEN_SIGNS_HTML_PATH}signs_33_dusk_no.htm"),
                        );
                        return;
                    }
                } else if cabal == CABAL_DAWN && settings.castle_for_dawn && class_level >= 2 {
                    // Clans without castle need to pay participation fee.
                    if castle_id.is_none_or(|c| c == 0) {
                        self.ss_show_file(
                            npc,
                            player,
                            &format!("{SEVEN_SIGNS_HTML_PATH}signs_33_dawn_fee.htm"),
                        );
                        return;
                    }
                }
                self.ss_show_signs(npc, player, 33, Some(side), false);
            }
            // Pay the participation fee request
            34 => {
                let page = if class_level > 0
                    && (self.adena_of(player) >= i64::from(settings.join_dawn_fee)
                        || self.inventory_item_count(player, settings.manors_agreement_id) > 0)
                {
                    "signs_33_dawn.htm"
                } else {
                    "signs_33_dawn_no.htm"
                };
                self.ss_show_file(npc, player, &format!("{SEVEN_SIGNS_HTML_PATH}{page}"));
            }
            // Join Cabal Intro 1, Festival of Darkness Intro
            3 | 8 => {
                self.ss_show_signs(npc, player, i64::from(val), Some(cabal_short_name(cabal)), false);
            }
            // Join a Cabal - SevenSigns 4 [0]1 x
            4 => self.ss_join_cabal(npc, player, kind, command, cabal, class_level, castle_id),
            5 => {
                let suffix = if self.ss_player_cabal(player.0) == CABAL_NULL {
                    format!("{side}_no")
                } else {
                    side.to_owned()
                };
                self.ss_show_signs(npc, player, 5, Some(&suffix), false);
            }
            21 => self.ss_contribute_amount(npc, player, side, command),
            // Contribute Seal Stones - SevenSigns 6 x
            6 => self.ss_contribute_stones(npc, player, side, command),
            // Exchange Ancient Adena for Adena - SevenSigns 7 xxxxxxx
            7 => {
                let page = match parse_sub(command, 13, None) {
                    None => "blkmrkt_3.htm",
                    Some(convert) if convert < 1 => "blkmrkt_3.htm",
                    Some(convert) if ancient_adena_amount < convert => "blkmrkt_4.htm",
                    Some(convert) => {
                        self.reduce_ancient_adena(player, convert, true);
                        self.add_adena(player, convert, true);
                        "blkmrkt_5.htm"
                    }
                };
                self.ss_show_file(npc, player, &format!("{SEVEN_SIGNS_HTML_PATH}{page}"));
            }
            // Receive Contribution Rewards
            9 => {
                let player_cabal = self.ss_player_cabal(player.0);
                let winning = self.ss_cabal_highest_score();
                if self.ss_is_seal_validation_period() && player_cabal == winning {
                    if !self.seven_signs.players.contains_key(&player.0) {
                        // NOTE(java-quirk): a tie and a player who never joined: Java's
                        // `getAncientAdenaReward` throws a NullPointerException; nothing happens.
                        warn!("SevenSigns: {player} has no Seven Signs data to collect a reward from.");
                        return;
                    }
                    let reward = self.ss_get_ancient_adena_reward(player.0, true);
                    if reward < 3 {
                        self.ss_show_signs(npc, player, 9, Some(&format!("{side}_b")), false);
                        return;
                    }
                    self.add_ancient_adena(player, i64::from(reward), true);
                    self.ss_show_signs(npc, player, 9, Some(&format!("{side}_a")), false);
                }
            }
            // Teleport to Hunting Grounds
            11 => {
                let parsed = command.get(14..).and_then(|rest| {
                    let mut st = rest.split_whitespace();
                    let x = st.next()?.parse::<i32>().ok()?;
                    let y = st.next()?.parse::<i32>().ok()?;
                    let z = st.next()?.parse::<i32>().ok()?;
                    let cost = st.next()?.parse::<i64>().ok()?;
                    Some((x, y, z, cost))
                });
                let Some((x, y, z, cost)) = parsed else {
                    warn!("SevenSigns: Error occurred while teleporting player: {command:?}");
                    return;
                };
                if cost > 0 && !self.reduce_ancient_adena(player, cost, true) {
                    return;
                }
                self.tele_to_location(player, crate::teleport::TeleportTarget::at(x, y, z));
            }
            16 => self.ss_show_signs(npc, player, 16, Some(side), false),
            // Exchange Seal Stones for Ancient Adena (Type Choice) - SevenSigns 17 x
            17 => self.ss_exchange_choice(npc, player, kind, command),
            // Exchange Seal Stones for Ancient Adena - SevenSigns 18 xxxx xxxxxx
            18 => self.ss_exchange_stones(npc, player, side, command),
            // Seal Information (for when joining a cabal)
            19 => match parse_int_raw(command, 16, None) {
                Some(chosen_seal) => {
                    let suffix = format!("{}_{}", seal_name(chosen_seal, true), cabal_short_name(cabal));
                    self.ss_show_signs(npc, player, 19, Some(&suffix), false);
                }
                None => warn!("SignsPriest: bad bypass {command:?} of {player}"),
            },
            // Seal Status (for when joining a cabal)
            20 => {
                let mut content = String::new();
                if kind == PriestKind::Dawn {
                    content.push_str(
                        "<html><body>Priest of Dawn:<br><font color=\"LEVEL\">[ Seal Status ]</font><br>",
                    );
                } else {
                    content.push_str(
                        "<html><body>Dusk Priestess:<br><font color=\"LEVEL\">[ Status of the Seals ]</font><br>",
                    );
                }
                for seal in 1..4 {
                    let owner = self.ss_seal_owner(seal);
                    if owner != CABAL_NULL {
                        content.push_str(&format!(
                            "[{}: {}]<br>",
                            seal_name(seal, false),
                            cabal_name(owner)
                        ));
                    } else {
                        content.push_str(&format!("[{}: Nothingness]<br>", seal_name(seal, false)));
                    }
                }
                content.push_str(&format!(
                    "<a action=\"bypass -h npc_{}_Chat 0\">Go back.</a></body></html>",
                    npc.0
                ));
                if let Some(html) = NpcHtmlMessage::new(npc.0, &content) {
                    self.send_html(player, &html);
                }
            }
            _ => self.ss_show_signs(npc, player, i64::from(val), None, false),
        }
    }

    /// `case 4`: joining a cabal.
    #[allow(clippy::too_many_arguments)]
    fn ss_join_cabal(
        &mut self,
        npc: ObjectId,
        player: ObjectId,
        kind: PriestKind,
        command: &str,
        cabal: i32,
        class_level: i32,
        castle_id: Option<i32>,
    ) {
        let side = kind.side();
        let Some(new_seal) = parse_int_raw(command, 15, None) else {
            warn!("SignsPriest: bad bypass {command:?} of {player}");
            return;
        };
        // The join dialog refuses characters without a class transfer, a direct bypass has to
        // refuse them as well.
        if class_level == 0 {
            self.ss_show_signs(npc, player, 33, Some(&format!("{side}_firstclass")), false);
            return;
        }
        // A crafted join has to match what the priest offers, its own side and one of the seals.
        let own = if kind == PriestKind::Dawn {
            CABAL_DAWN
        } else {
            CABAL_DUSK
        };
        if !(SEAL_AVARICE..=SEAL_STRIFE).contains(&new_seal) || cabal != own {
            return;
        }
        // Joining is only offered during the competition period.
        if !self.ss_is_competition_period() {
            self.show_chat_window_default(npc, player);
            return;
        }
        // A member rejoining would switch sides and keep the contribution score.
        if self.ss_player_cabal(player.0) != CABAL_NULL {
            self.ss_show_signs(npc, player, 33, Some(&format!("{side}_member")), false);
            return;
        }
        if class_level >= 1 {
            let settings = self.seven_signs.settings.clone();
            let has_castle = castle_id.is_some_and(|c| c > 0);
            // "even if in htmls is said that ally can have castle too, but it is not"
            if cabal == CABAL_DUSK && settings.castle_for_dusk && has_castle {
                self.ss_show_file(
                    npc,
                    player,
                    &format!("{SEVEN_SIGNS_HTML_PATH}signs_33_dusk_no.htm"),
                );
                return;
            }
            // The Lord's certificate or the fee, only after the second class transfer.
            if settings.castle_for_dawn && cabal == CABAL_DAWN && class_level >= 2 {
                let allow = has_castle
                    || self.destroy_item_by_item_id(player, settings.manors_agreement_id, 1, true)
                    || self.reduce_adena(player, i64::from(settings.join_dawn_fee), true);
                if !allow {
                    self.ss_show_file(
                        npc,
                        player,
                        &format!("{SEVEN_SIGNS_HTML_PATH}signs_33_dawn_fee.htm"),
                    );
                    return;
                }
            }
        }
        self.ss_set_player_info(player.0, cabal, new_seal);
        let joined = if cabal == CABAL_DAWN {
            SystemMessageId::YouWillParticipateInTheSevenSignsAsAMemberOfTheLordsOfDawn
        } else {
            SystemMessageId::YouWillParticipateInTheSevenSignsAsAMemberOfTheRevolutionariesOfDusk
        };
        self.send(player, &SystemMessage::new(joined));
        // Show a confirmation message to the user, indicating which seal they chose.
        let chosen = match new_seal {
            SEAL_AVARICE => SystemMessageId::YouVeChosenToFightForTheSealOfAvariceDuringThisQuestEventPeriod,
            SEAL_GNOSIS => SystemMessageId::YouVeChosenToFightForTheSealOfGnosisDuringThisQuestEventPeriod,
            _ => SystemMessageId::YouVeChosenToFightForTheSealOfStrifeDuringThisQuestEventPeriod,
        };
        self.send(player, &SystemMessage::new(chosen));
        self.ss_show_signs(npc, player, 4, Some(cabal_short_name(cabal)), false);
    }

    /// `S2_S1_HAS_DISAPPEARED` of a stone given to the priest.
    fn ss_stones_gone(&self, player: ObjectId, item_id: i32, count: i64) {
        self.send(
            player,
            &SystemMessage::new(SystemMessageId::S2S1HasDisappeared)
                .with(SystemMessageParam::ItemName(item_id))
                .with(SystemMessageParam::LongNumber(count)),
        );
    }

    /// `case 21`: contribute a number of stones of one colour.
    fn ss_contribute_amount(&mut self, npc: ObjectId, player: ObjectId, side: &str, command: &str) {
        let Some(stone_id) = parse_int_raw(command, 14, Some(18)) else {
            warn!("SignsPriest: bad bypass {command:?} of {player}");
            return;
        };
        let score = i64::from(self.ss_player_contrib_score(player.0));
        let Some(count) = parse_sub(command, 19, None) else {
            self.ss_show_signs(npc, player, 6, Some(&format!("{side}_failure")), false);
            return;
        };
        let room = i64::from(self.seven_signs.settings.max_player_contrib).wrapping_sub(score);
        let (mut red, mut green, mut blue) = (0, 0, 0);
        match stone_id {
            SEAL_STONE_BLUE_ID => blue = count.min(room / BLUE_CONTRIB_POINTS),
            SEAL_STONE_GREEN_ID => green = count.min(room / GREEN_CONTRIB_POINTS),
            SEAL_STONE_RED_ID => red = count.min(room / RED_CONTRIB_POINTS),
            _ => {}
        }
        let mut found = false;
        for (amount, id) in [
            (red, SEAL_STONE_RED_ID),
            (green, SEAL_STONE_GREEN_ID),
            (blue, SEAL_STONE_BLUE_ID),
        ] {
            if amount > 0 && self.destroy_item_by_item_id(player, id, amount, false) {
                found = true;
                self.ss_stones_gone(player, id, amount);
            }
        }
        if count > 0 && red.wrapping_add(green).wrapping_add(blue) <= 0 {
            // A requested amount that does not fit under the contribution limit.
            self.send(
                player,
                &SystemMessage::new(SystemMessageId::ContributionLevelHasExceededTheLimitYouMayNotContinue),
            );
        } else if !found {
            self.ss_show_signs(npc, player, 6, Some(&format!("{side}_low_stones")), false);
        } else {
            let gained = self.ss_add_player_stone_contrib(player.0, blue, green, red);
            self.send(
                player,
                &SystemMessage::new(SystemMessageId::YourContributionScoreHasIncreasedByS1)
                    .with(SystemMessageParam::LongNumber(gained)),
            );
            self.ss_show_signs(npc, player, 6, Some(side), false);
        }
    }

    /// `case 6`: the contribution page of one colour, or (4) everything that fits.
    fn ss_contribute_stones(&mut self, npc: ObjectId, player: ObjectId, side: &str, command: &str) {
        let Some(stone_type) = parse_int_raw(command, 13, None) else {
            warn!("SignsPriest: bad bypass {command:?} of {player}");
            return;
        };
        let blue_count = self.inventory_item_count(player, SEAL_STONE_BLUE_ID);
        let green_count = self.inventory_item_count(player, SEAL_STONE_GREEN_ID);
        let red_count = self.inventory_item_count(player, SEAL_STONE_RED_ID);
        let contrib_score = i64::from(self.ss_player_contrib_score(player.0));
        let max = i64::from(self.seven_signs.settings.max_player_contrib);
        if contrib_score >= max {
            self.send(
                player,
                &SystemMessage::new(SystemMessageId::ContributionLevelHasExceededTheLimitYouMayNotContinue),
            );
            return;
        }
        let (color, stone_color, stone_id, stone_count) = match stone_type {
            1 => ("Blue", "blue", SEAL_STONE_BLUE_ID, blue_count),
            2 => ("Green", "green", SEAL_STONE_GREEN_ID, green_count),
            3 => ("Red", "red", SEAL_STONE_RED_ID, red_count),
            4 => {
                let mut temp = contrib_score;
                let red = (max.wrapping_sub(temp) / RED_CONTRIB_POINTS).min(red_count);
                temp = temp.wrapping_add(red.wrapping_mul(RED_CONTRIB_POINTS));
                let green = (max.wrapping_sub(temp) / GREEN_CONTRIB_POINTS).min(green_count);
                temp = temp.wrapping_add(green.wrapping_mul(GREEN_CONTRIB_POINTS));
                let blue = (max.wrapping_sub(temp) / BLUE_CONTRIB_POINTS).min(blue_count);
                let mut found = false;
                for (amount, id) in [
                    (red, SEAL_STONE_RED_ID),
                    (green, SEAL_STONE_GREEN_ID),
                    (blue, SEAL_STONE_BLUE_ID),
                ] {
                    if amount > 0 && self.destroy_item_by_item_id(player, id, amount, false) {
                        found = true;
                        self.ss_stones_gone(player, id, amount);
                    }
                }
                if !found && red_count.wrapping_add(green_count).wrapping_add(blue_count) > 0 {
                    // Stones are carried, but none fit under the contribution limit.
                    self.send(
                        player,
                        &SystemMessage::new(
                            SystemMessageId::ContributionLevelHasExceededTheLimitYouMayNotContinue,
                        ),
                    );
                } else if !found {
                    self.ss_show_signs(npc, player, 6, Some(&format!("{side}_no_stones")), false);
                } else {
                    let gained = self.ss_add_player_stone_contrib(player.0, blue, green, red);
                    self.send(
                        player,
                        &SystemMessage::new(SystemMessageId::YourContributionScoreHasIncreasedByS1)
                            .with(SystemMessageParam::LongNumber(gained)),
                    );
                    self.ss_show_signs(npc, player, 6, Some(side), false);
                }
                return;
            }
            // NOTE(java-quirk): another stone type replaces `%contribStoneColor%` with null,
            // which throws in `String.replace`; nothing is shown.
            _ => return,
        };
        let path = format!("{SEVEN_SIGNS_HTML_PATH}signs_6_{side}_contribute.htm");
        match self.get_htm(player, &path) {
            Some(content) => {
                let content = content
                    .replace("%contribStoneColor%", color)
                    .replace("%stoneColor%", stone_color)
                    .replace("%stoneCount%", &stone_count.to_string())
                    .replace("%stoneItemId%", &stone_id.to_string())
                    .replace("%objectId%", &npc.0.to_string());
                if let Some(html) = NpcHtmlMessage::new(npc.0, &content) {
                    self.send_html(player, &html);
                }
            }
            None => warn!("Problem with HTML text {path}"),
        }
    }

    /// `case 17`: the exchange page of one colour, or (4) every stone at once.
    fn ss_exchange_choice(&mut self, npc: ObjectId, player: ObjectId, kind: PriestKind, command: &str) {
        let side = kind.side();
        let Some(stone_type) = parse_int_raw(command, 14, None) else {
            warn!("SignsPriest: bad bypass {command:?} of {player}");
            return;
        };
        let (stone_color, stone_id, stone_value) = match stone_type {
            1 => ("blue", SEAL_STONE_BLUE_ID, SEAL_STONE_BLUE_VALUE),
            2 => ("green", SEAL_STONE_GREEN_ID, SEAL_STONE_GREEN_VALUE),
            3 => ("red", SEAL_STONE_RED_ID, SEAL_STONE_RED_VALUE),
            4 => {
                let blue = self.inventory_item_count(player, SEAL_STONE_BLUE_ID);
                let green = self.inventory_item_count(player, SEAL_STONE_GREEN_ID);
                let red = self.inventory_item_count(player, SEAL_STONE_RED_ID);
                let reward = calc_ancient_adena_reward(blue, green, red);
                if reward == 0 {
                    self.ss_show_signs(npc, player, 18, Some(&format!("{side}_no_stones")), false);
                    return;
                }
                for (count, id) in [
                    (blue, SEAL_STONE_BLUE_ID),
                    (green, SEAL_STONE_GREEN_ID),
                    (red, SEAL_STONE_RED_ID),
                ] {
                    if count > 0 {
                        self.destroy_item_by_item_id(player, id, count, true);
                    }
                }
                self.add_ancient_adena(player, reward, true);
                self.ss_show_signs(npc, player, 18, Some(side), false);
                return;
            }
            // NOTE(java-quirk): another type replaces `%stoneColor%` with null, which throws.
            _ => return,
        };
        let stone_count = self.inventory_item_count(player, stone_id);
        let path = format!("{SEVEN_SIGNS_HTML_PATH}signs_17_{side}.htm");
        match self.get_htm(player, &path) {
            Some(content) => {
                let content = content
                    .replace("%stoneColor%", stone_color)
                    .replace("%stoneValue%", &stone_value.to_string())
                    .replace("%stoneCount%", &stone_count.to_string())
                    .replace("%stoneItemId%", &stone_id.to_string())
                    .replace("%objectId%", &npc.0.to_string());
                if let Some(html) = NpcHtmlMessage::new(npc.0, &content) {
                    self.send_html(player, &html);
                }
            }
            None => warn!("Problem with HTML text {SEVEN_SIGNS_HTML_PATH}signs_17.htm: {path}"),
        }
    }

    /// `case 18`: a number of stones of one colour for Ancient Adena.
    fn ss_exchange_stones(&mut self, npc: ObjectId, player: ObjectId, side: &str, command: &str) {
        let Some(stone_id) = parse_int_raw(command, 14, Some(18)) else {
            warn!("SignsPriest: bad bypass {command:?} of {player}");
            return;
        };
        let Some(count) = parse_sub(command, 19, None) else {
            self.ss_show_signs(npc, player, 18, Some(&format!("{side}_failed")), false);
            return;
        };
        if self.inventory_item_by_item_id(player, stone_id).is_none() {
            self.ss_show_signs(npc, player, 18, Some(&format!("{side}_no_stones")), false);
            return;
        }
        let total = self.inventory_item_count(player, stone_id);
        if count <= total && count > 0 {
            let reward = match stone_id {
                SEAL_STONE_BLUE_ID => calc_ancient_adena_reward(count, 0, 0),
                SEAL_STONE_GREEN_ID => calc_ancient_adena_reward(0, count, 0),
                SEAL_STONE_RED_ID => calc_ancient_adena_reward(0, 0, count),
                _ => 0,
            };
            if self.destroy_item_by_item_id(player, stone_id, count, true) {
                self.add_ancient_adena(player, reward, true);
                self.ss_show_signs(npc, player, 18, Some(side), false);
            }
        } else {
            self.ss_show_signs(npc, player, 18, Some(&format!("{side}_low_stones")), false);
        }
    }

    // ------------------------------------------------------------------ SevenSigns' bookkeeping

    /// Java `setPlayerInfo(objectId, chosenCabal, chosenSeal)`: the member joins (a new member
    /// gets its row, per player), the seal counts one more chooser.
    pub fn ss_set_player_info(&mut self, char_id: i32, chosen_cabal: i32, chosen_seal: i32) -> i32 {
        let name = cabal_short_name(chosen_cabal).to_owned();
        let is_dawn = match self.seven_signs.players.get_mut(&char_id) {
            Some(p) => {
                // "If the seal validation period has passed, cabal information was removed and so
                // "re-register" player"
                p.cabal = name;
                p.seal = chosen_seal;
                p.cabal == "dawn"
            }
            None => {
                let row = SignsPlayer {
                    char_id,
                    cabal: name.clone(),
                    seal: chosen_seal,
                    ..SignsPlayer::default()
                };
                let is_dawn = row.cabal == "dawn";
                self.seven_signs.players.insert(char_id, row);
                // "Update data in database, as we have a new player signing up."
                self.outbox
                    .persist(PersistCommand::SevenSigns(SevenSignsWrite::InsertPlayer {
                        char_id,
                        cabal: name,
                        seal: chosen_seal,
                    }));
                is_dawn
            }
        };
        // Increasing Seal total score for the player chosen Seal.
        // NOTE(java-quirk): an unknown seal makes Java's `get(seal) + 1` throw; here it counts
        // nowhere.
        let status = &mut self.seven_signs.status;
        let totals = if is_dawn {
            &mut status.dawn_seal_totals
        } else {
            &mut status.dusk_seal_totals
        };
        if let Some(total) = usize::try_from(chosen_seal).ok().and_then(|i| totals.get_mut(i)) {
            *total = total.wrapping_add(1);
        }
        if !self.seven_signs.settings.lazy_update {
            self.ss_save_status();
        }
        chosen_cabal
    }

    /// Java `getAncientAdenaReward(objectId, removeReward)`: what the member may collect; the
    /// stones and the Ancient Adena of its row go back to 0.
    // NOTE(java-quirk): the row is emptied even with `removeReward` false.
    pub fn ss_get_ancient_adena_reward(&mut self, char_id: i32, remove_reward: bool) -> i32 {
        let Some(p) = self.seven_signs.players.get_mut(&char_id) else {
            // Java: NullPointerException.
            return 0;
        };
        let reward = p.ancient_adena_amount as i32;
        p.red_stones = 0;
        p.green_stones = 0;
        p.blue_stones = 0;
        p.ancient_adena_amount = 0;
        if remove_reward && !self.seven_signs.settings.lazy_update {
            self.ss_save_data(char_id);
            self.ss_save_status();
        }
        reward
    }

    /// Java `addPlayerStoneContrib(objectId, blue, green, red)`: the stones count for the member
    /// and its cabal; -1 when the score would pass `AltMaxPlayerContrib`.
    pub fn ss_add_player_stone_contrib(&mut self, char_id: i32, blue: i64, green: i64, red: i64) -> i64 {
        let max = i64::from(self.seven_signs.settings.max_player_contrib);
        let contrib = calc_contribution_score(blue, green, red);
        let Some(p) = self.seven_signs.players.get_mut(&char_id) else {
            // Java: NullPointerException.
            return -1;
        };
        let total = p.contribution_score.wrapping_add(contrib);
        if total > max {
            return -1;
        }
        let total_aa = p
            .ancient_adena_amount
            .wrapping_add(calc_ancient_adena_reward(blue, green, red));
        p.red_stones = i64::from(p.red_stones as i32).wrapping_add(red);
        p.green_stones = i64::from(p.green_stones as i32).wrapping_add(green);
        p.blue_stones = i64::from(p.blue_stones as i32).wrapping_add(blue);
        p.ancient_adena_amount = total_aa;
        p.contribution_score = total;
        match p.cabal_id() {
            CABAL_DAWN => self.seven_signs.status.dawn_stone_score += contrib as f64,
            CABAL_DUSK => self.seven_signs.status.dusk_stone_score += contrib as f64,
            _ => {}
        }
        if !self.seven_signs.settings.lazy_update {
            self.ss_save_data(char_id);
            self.ss_save_status();
        }
        contrib
    }

    // ------------------------------------------------------------------ Ancient Adena

    /// Java `Player.addAncientAdena(process, count, reference, sendMessage)`.
    pub fn add_ancient_adena(&mut self, player: ObjectId, count: i64, send_message: bool) {
        if send_message {
            self.send(
                player,
                &SystemMessage::new(SystemMessageId::YouHaveEarnedS2S1S)
                    .with(SystemMessageParam::ItemName(ANCIENT_ADENA_ID))
                    .with(SystemMessageParam::LongNumber(count)),
            );
        }
        if count <= 0 {
            return;
        }
        // NOTE(java-quirk): `PlayerInventory.addItem` already sends the update; Java sends the
        // Ancient Adena again.
        if let Some(added) = self.inventory_add_item_by_id(player, ANCIENT_ADENA_ID, count) {
            let entry = self.update_entry(&added.item, added.change);
            self.send_inventory_update(player, entry);
        }
    }

    /// Java `Player.reduceAncientAdena(process, count, reference, sendMessage)`.
    pub fn reduce_ancient_adena(&mut self, player: ObjectId, count: i64, send_message: bool) -> bool {
        if count > self.inventory_item_count(player, ANCIENT_ADENA_ID) {
            if send_message {
                self.send(
                    player,
                    &SystemMessage::new(SystemMessageId::YouDoNotHaveEnoughAdena),
                );
            }
            return false;
        }
        if count > 0 {
            // `destroyItemByItemId` with Java's messages (`S2_S1_HAS_DISAPPEARED` for more
            // than one, `S1_HAS_DISAPPEARED` for one).
            return self.destroy_item_by_item_id(player, ANCIENT_ADENA_ID, count, send_message);
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_command_is_cut_like_javas_substrings() {
        // "SevenSigns 4 2 3": the value, then the cabal from index 13, the seal from 15.
        let c = "SevenSigns 4 2 3";
        assert_eq!(parse_int_sub(c, 11, Some(13)), Some(4));
        assert_eq!(parse_int_sub(c, 14, Some(15)), None);
        assert_eq!(parse_int_sub(c, 13, Some(14)), Some(2));
        assert_eq!(parse_int_sub(c, 15, None), Some(3));
        // "SevenSigns 6 4": index 15 is past the end, so Java falls back to 13..14.
        let c = "SevenSigns 6 4";
        assert_eq!(parse_int_sub(c, 14, Some(15)), None);
        assert_eq!(parse_int_sub(c, 13, Some(14)), Some(4));
        // "SevenSigns 21 6360 100": the stone and the amount.
        let c = "SevenSigns 21 6360 100";
        assert_eq!(parse_int_sub(c, 14, Some(18)), Some(6360));
        assert_eq!(parse_sub(c, 19, None), Some(100));
    }
}
