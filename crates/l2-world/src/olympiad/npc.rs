//! The Olympiad NPC pages and bypasses (`OlympiadManagerLink`, `OlympiadObservation`), the
//! buffer's chat window and `/olympiadstat`.
// Java: dist/game/data/scripts/handlers/bypass/npc/{OlympiadManagerLink,OlympiadObservation}.java
// Java: dist/game/data/scripts/handlers/chat/commands/user/OlympiadStat.java
// Java: entity/actor/instance/OlympiadManager.java (showChatWindow(player, val, suffix))
// Java: entity/actor/Npc.java (showChatWindow: the buffer 36402)
// Java: network/clientpackets/RequestBypassToServer.java (`_olympiad?command`)

use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{
    ActionFailed, ExHeroList, MagicSkillUse, SkillPosition, SystemMessage, SystemMessageParam,
};
use l2_util::ObjectId;
use tracing::warn;

use super::game::CompetitionType;
use super::{
    OLYMPIAD_HTML_PATH, OlympiadRereadAction, OlympiadRereadRequest, OlympiadRereadRows, OlympiadRereadThen,
    UNCLAIMED_OLYMPIAD_PASSES_VAR,
};
use crate::bypass::BypassHandlers;
use crate::effects::SkillKey;
use crate::outbox::PersistCommand;
use crate::world::World;

/// `OlympiadManagerLink.ALLOWED_BUFFS`.
const ALLOWED_BUFFS: [(i32, i32); 10] = [
    (4357, 2), // Haste Lv2
    (4342, 2), // Wind Walk Lv2
    (4356, 3), // Empower Lv3
    (4355, 3), // Acumen Lv3
    (4351, 6), // Concentration Lv6
    (4345, 3), // Might Lv3
    (4358, 3), // Guidance Lv3
    (4359, 3), // Focus Lv3
    (4360, 3), // Death Whisper Lv3
    (4352, 2), // Berserker Spirit Lv2
];

/// The classes of the ranking page (`OlympiadManagerLink`'s `switch (playerClass)`).
const RANKING_CLASSES: [i32; 36] = [
    88, 89, 90, 91, 92, 93, 94, 95, 96, 97, 98, 99, 100, 101, 102, 103, 104, 105, 106, 107, 108, 109, 110,
    111, 112, 113, 114, 115, 116, 117, 118, 131, 132, 133, 134, 136,
];

/// The multisells of the rewards (`olympiadnoble 7` and `8`).
const EQUIPMENT_REWARDS: i32 = 102;
const MISC_REWARDS: i32 = 103;

/// The buffer of the arenas (`Npc.showChatWindow`'s `case 36402`).
pub(crate) const OLYMPIAD_HOST: i32 = 36402;

/// Registers the Olympiad bypass handlers.
pub fn register_bypass_handlers(handlers: &mut BypassHandlers) {
    for command in ["olympiaddesc", "olympiadnoble", "olybuff", "olympiad"] {
        handlers.register(command, manager_link_handler);
    }
    for command in ["watchmatch", "arenachange"] {
        handlers.register(command, observation_handler);
    }
}

/// `RequestBypassToServer`'s `_olympiad?command` branch: the arena of the observer window
/// (`arenachange <arena - 1>`).
pub(crate) fn olympiad_command_bypass(world: &mut World, player: ObjectId, command: &str) {
    // `Integer.parseInt(_command.split("=")[2])`
    let Some(arena) = command.split('=').nth(2).and_then(|t| t.parse::<i32>().ok()) else {
        warn!("{player}: bad olympiad bypass [{command}]");
        return;
    };
    let arena = arena.wrapping_sub(1);
    observation_handler(world, player, &format!("arenachange {arena}"), None);
}

fn is_olympiad_manager(world: &World, npc: ObjectId) -> bool {
    world
        .npcs
        .get(&npc)
        .and_then(|n| world.npc_world.template(n.npc_id))
        .is_some_and(|t| t.npc_type == "OlympiadManager")
}

/// Java `OlympiadManagerLink.onCommand`.
fn manager_link_handler(
    world: &mut World,
    player: ObjectId,
    command: &str,
    target: Option<ObjectId>,
) -> bool {
    let Some(npc) = target.filter(|t| is_olympiad_manager(world, *t)) else {
        return false;
    };
    let lower = command.to_lowercase();
    // Java catches every exception of the handler, logs it and answers `true`.
    let done = if lower.starts_with("olympiaddesc") {
        world.olympiad_desc(player, npc, command)
    } else if lower.starts_with("olympiadnoble") {
        return world.olympiad_noble(player, npc, command);
    } else if lower.starts_with("olybuff") {
        return world.olympiad_buff(player, npc, command);
    } else if lower.starts_with("olympiad") {
        world.olympiad_command(player, npc, command)
    } else {
        Some(())
    };
    if done.is_none() {
        warn!("Exception in OlympiadManagerLink: [{command}] from {player}");
    }
    true
}

/// Java `OlympiadObservation.onCommand`.
fn observation_handler(
    world: &mut World,
    player: ObjectId,
    command: &str,
    _target: Option<ObjectId>,
) -> bool {
    let manager = world.players.get(&player).and_then(|p| p.talk.last_folk_npc);
    if command.starts_with("watchmatch") {
        if !world.olympiad.in_comp_period {
            world.send(
                player,
                &SystemMessage::new(SystemMessageId::TheGrandOlympiadGamesAreNotCurrentlyInProgress),
            );
            return false;
        }
        let list = world.olympiad_match_list();
        world.send(player, &list);
        return true;
    }
    let Some(manager) = manager.filter(|m| is_olympiad_manager(world, *m)) else {
        return false;
    };
    let near = match (world.position_of(player), world.position_of(manager)) {
        (Some(a), Some(b)) => crate::target::distance_2d(a, b) <= 300.0,
        _ => false,
    };
    if !world.in_observer_mode(player) && !near {
        return false;
    }
    if world.is_registered_in_comp(player) {
        world.send(
            player,
            &SystemMessage::new(
                SystemMessageId::YouMayNotObserveAGrandOlympiadGamesMatchWhileYouAreOnTheWaitingList,
            ),
        );
        return false;
    }
    if !world.olympiad.in_comp_period {
        world.send(
            player,
            &SystemMessage::new(SystemMessageId::TheGrandOlympiadGamesAreNotCurrentlyInProgress),
        );
        return false;
    }
    // `isRegisteredOnEvent()`: the events are scripts (D-05).
    let Some(arena) = command.get(12..).and_then(|t| t.trim().parse::<i32>().ok()) else {
        warn!("Exception in OlympiadObservation: [{command}] from {player}");
        return false;
    };
    world.olympiad_game_manager();
    let Some(stadium) = usize::try_from(arena)
        .ok()
        .and_then(|i| world.olympiad.stadiums().get(i))
    else {
        return true;
    };
    let instance_id = stadium.instance_id;
    let Some(spawn) = stadium.spectator_spawns.first().copied() else {
        warn!("Exception in OlympiadObservation: the arena {arena} has no spectator spawn");
        return false;
    };
    world.enter_olympiad_observer_mode(player, spawn, arena, instance_id);
    true
}

impl World {
    /// A page of the olympiad folder with `%objectId%` of the NPC.
    fn olympiad_page(&mut self, player: ObjectId, npc: ObjectId, file: &str, replace: &[(&str, String)]) {
        let Some(mut html) = self.html_from_file(player, npc.0, &format!("{OLYMPIAD_HTML_PATH}{file}"))
        else {
            return;
        };
        for (key, value) in replace {
            html.html = html.html.replace(key, value);
        }
        self.send_html(player, &html);
    }

    /// `olympiaddesc`: `OlympiadManager.showChatWindow(player, val, suffix)`.
    fn olympiad_desc(&mut self, player: ObjectId, npc: ObjectId, command: &str) -> Option<()> {
        let value: i32 = command.get(13..14)?.parse().ok()?;
        let suffix = command.get(14..)?;
        let mut filename = format!("{OLYMPIAD_HTML_PATH}noble_desc{value}{suffix}.htm");
        if filename == format!("{OLYMPIAD_HTML_PATH}noble_desc0.htm") {
            filename = format!("{OLYMPIAD_HTML_PATH}noble_main.htm");
        }
        // `Npc.showChatWindow(player, filename)`.
        self.send_npc_file(npc, player, &filename);
        self.send(player, &ActionFailed);
        Some(())
    }

    /// `olympiadnoble`.
    fn olympiad_noble(&mut self, player: ObjectId, npc: ObjectId, command: &str) -> bool {
        let object_id = ("%objectId%", npc.0.to_string());
        if self.is_cursed_weapon_equipped(player) {
            self.olympiad_page(player, npc, "noble_cursed_weapon.htm", &[]);
            return false;
        }
        if self
            .players
            .get(&player)
            .is_some_and(|p| p.data.profile.class_index != 0)
        {
            self.olympiad_page(player, npc, "noble_sub.htm", &[object_id]);
            return false;
        }
        let class_level = self.players.get(&player).map_or(0, |p| {
            l2_data::ClassId(u16::try_from(p.data.class_id).unwrap_or(0)).class_level()
        });
        if !self.is_noble(player) || class_level < 3 {
            self.olympiad_page(player, npc, "noble_thirdclass.htm", &[object_id]);
            return false;
        }
        let Some(value) = command.get(14..).and_then(|t| t.parse::<i32>().ok()) else {
            warn!("Exception in OlympiadManagerLink: [{command}] from {player}");
            return true;
        };
        match value {
            // H5 match selection
            0 => {
                if !self.olympiad_is_registered(player) {
                    let replace = [
                        object_id,
                        ("%olympiad_period%", self.olympiad.data.period.to_string()),
                        ("%olympiad_cycle%", self.olympiad.data.current_cycle.to_string()),
                        ("%olympiad_opponent%", self.olympiad_count_opponents().to_string()),
                    ];
                    self.olympiad_page(player, npc, "noble_desc2a.htm", &replace);
                } else {
                    self.olympiad_page(player, npc, "noble_unregister.htm", &[object_id]);
                }
            }
            // unregister
            1 => {
                self.olympiad_unregister_noble(player);
            }
            // show waiting list
            2 => {
                let non_classed = self.olympiad.non_class_based.len();
                let teams = self.olympiad.teams_based.len();
                let classed: usize = self.olympiad.class_based.values().map(Vec::len).sum();
                let display = usize::try_from(self.olympiad.settings.reg_display).unwrap_or(0);
                let shown = |count: usize| {
                    if display > 0 {
                        if count < display {
                            format!("Fewer than {}", self.olympiad.settings.reg_display)
                        } else {
                            format!("More than {}", self.olympiad.settings.reg_display)
                        }
                    } else {
                        count.to_string()
                    }
                };
                let replace = [
                    ("%listClassed%", shown(classed)),
                    ("%listNonClassedTeam%", shown(teams)),
                    ("%listNonClassed%", shown(non_classed)),
                    object_id,
                ];
                self.olympiad_page(player, npc, "noble_registered.htm", &replace);
            }
            // "There are %points% Grand Olympiad points granted for this event."
            3 => {
                let points = self.noble_points(player.0);
                self.olympiad_page(
                    player,
                    npc,
                    "noble_points1.htm",
                    &[("%points%", points.to_string()), object_id],
                );
            }
            4 => {
                self.olympiad_register_noble(player, CompetitionType::NonClassed);
            }
            5 => {
                self.olympiad_register_noble(player, CompetitionType::Classed);
            }
            // request tokens reward
            6 => self.olympiad_passes(player, npc, OlympiadRereadAction::SettlePage),
            // Equipment Rewards
            7 => self.multisell_separate_and_send(EQUIPMENT_REWARDS, player, Some(npc), false, 1.0, 1.0),
            // Misc. Rewards
            8 => self.multisell_separate_and_send(MISC_REWARDS, player, Some(npc), false, 1.0, 1.0),
            // "Your Grand Olympiad Score from the previous period is %points% point(s)"
            9 => self.olympiad_reread(
                OlympiadRereadRequest::LastPoints { char_id: player.0 },
                OlympiadRereadThen {
                    player,
                    npc,
                    action: OlympiadRereadAction::LastPoints,
                },
            ),
            // give tokens to player
            10 => self.olympiad_passes(player, npc, OlympiadRereadAction::GivePasses),
            // register team
            11 => {
                self.olympiad_register_noble(player, CompetitionType::Teams);
            }
            _ => warn!("Olympiad System: Could not send packet for request {value}"),
        }
        true
    }

    /// `olympiadnoble 6` and `10`: Java reads the variable of the player. With
    /// `CycleDatabaseWrites` off the Java game server writes it at the period end, so the row is
    /// read again first (D-50).
    fn olympiad_passes(&mut self, player: ObjectId, npc: ObjectId, action: OlympiadRereadAction) {
        if self.olympiad_writes_cycles() {
            let passes = self.player_variable_int(player, UNCLAIMED_OLYMPIAD_PASSES_VAR);
            self.olympiad_passes_with(player, npc, action, passes, false);
        } else {
            self.olympiad_reread(
                OlympiadRereadRequest::UnclaimedPasses { char_id: player.0 },
                OlympiadRereadThen { player, npc, action },
            );
        }
    }

    /// The variable as `getInt(name, 0)`.
    fn player_variable_int(&self, player: ObjectId, name: &str) -> i32 {
        self.players
            .get(&player)
            .and_then(|p| p.data.profile.variables.get(name))
            .and_then(|v| v.trim().parse::<i32>().ok())
            .unwrap_or(0)
    }

    /// The pages of `olympiadnoble 6` / `10` once the passes are known.
    fn olympiad_passes_with(
        &mut self,
        player: ObjectId,
        npc: ObjectId,
        action: OlympiadRereadAction,
        passes: i32,
        reread: bool,
    ) {
        let object_id = ("%objectId%", npc.0.to_string());
        match action {
            OlympiadRereadAction::SettlePage => {
                let file = if passes > 0 {
                    "noble_settle.htm"
                } else {
                    "noble_nopoints2.htm"
                };
                self.olympiad_page(player, npc, file, &[object_id]);
            }
            OlympiadRereadAction::GivePasses => {
                if passes <= 0 {
                    return;
                }
                self.remove_player_variable(player, UNCLAIMED_OLYMPIAD_PASSES_VAR);
                if reread {
                    // The row the Java game server wrote goes now, before the items are handed
                    // out, so neither server hands them out again.
                    self.outbox.persist(PersistCommand::PlayerVariables {
                        owner: player,
                        upserts: Vec::new(),
                        removed: vec![UNCLAIMED_OLYMPIAD_PASSES_VAR.to_owned()],
                    });
                }
                let item = self.olympiad.settings.comp_reward_item;
                let count = i64::from(passes) * i64::from(self.olympiad.settings.gp_per_point);
                self.add_item(player, item, count, true);
            }
            OlympiadRereadAction::Ranking | OlympiadRereadAction::LastPoints => {}
        }
    }

    /// Sends a read to the persistence task; the answer comes back as
    /// `WorldCommand::OlympiadReread`.
    fn olympiad_reread(&mut self, request: OlympiadRereadRequest, then: OlympiadRereadThen) {
        self.outbox
            .persist(PersistCommand::OlympiadRereadRequest { request, then });
    }

    /// The answer of an Olympiad read (D-15, D-50): a failed read shows nothing and hands nothing
    /// out.
    pub(crate) fn on_olympiad_reread(
        &mut self,
        result: Result<OlympiadRereadRows, ()>,
        then: OlympiadRereadThen,
    ) {
        let OlympiadRereadThen { player, npc, action } = then;
        if !self.players.contains_key(&player) {
            return;
        }
        let Ok(rows) = result else {
            return;
        };
        match (action, rows) {
            (OlympiadRereadAction::Ranking, OlympiadRereadRows::Names(names)) => {
                let mut replace = Vec::with_capacity(21);
                for index in 1..=10usize {
                    let (place, rank) = match names.get(index - 1) {
                        Some(name) => (index.to_string(), name.clone()),
                        None => (String::new(), String::new()),
                    };
                    replace.push((format!("%place{index}%"), place));
                    replace.push((format!("%rank{index}%"), rank));
                }
                replace.push(("%objectId%".to_owned(), npc.0.to_string()));
                let replace: Vec<(&str, String)> =
                    replace.iter().map(|(k, v)| (k.as_str(), v.clone())).collect();
                self.olympiad_page(player, npc, "olympiad_ranking.htm", &replace);
            }
            (OlympiadRereadAction::LastPoints, OlympiadRereadRows::Points(points)) => {
                self.olympiad_page(
                    player,
                    npc,
                    "noble_points2.htm",
                    &[
                        ("%points%", points.to_string()),
                        ("%objectId%", npc.0.to_string()),
                    ],
                );
            }
            (action, OlympiadRereadRows::Passes(value)) => {
                let passes = value.and_then(|v| v.trim().parse::<i32>().ok()).unwrap_or(0);
                self.olympiad_passes_with(player, npc, action, passes, true);
            }
            (action, rows) => warn!("Olympiad: unexpected read answer {rows:?} for {action:?}"),
        }
    }

    /// `olybuff <index>`.
    fn olympiad_buff(&mut self, player: ObjectId, npc: ObjectId, command: &str) -> bool {
        let mut buff_count = self.olympiad_buff_count(player);
        if buff_count <= 0 {
            return false;
        }
        let param = command.split(' ').nth(1).unwrap_or("");
        let npc_id = self.npcs.get(&npc).map_or(0, |n| n.npc_id);
        if param.is_empty() || !param.chars().all(|c| c.is_ascii_digit()) {
            warn!(
                "Olympiad Buffer Warning: npcId = {npc_id} has invalid buffGroup set in the bypass for the buff selected: {param}"
            );
            return false;
        }
        let Some(&(skill_id, level)) = param.parse::<usize>().ok().and_then(|i| ALLOWED_BUFFS.get(i)) else {
            warn!("Olympiad Buffer Warning: npcId = {npc_id} has invalid index sent in the bypass: {param}");
            return false;
        };
        let exists = self
            .data
            .stat_data()
            .is_some_and(|d| d.skills.get(skill_id, level).is_some());
        if exists {
            self.set_npc_target(npc, Some(player));
            buff_count -= 1;
            if let Some(state) = self.player_olympiad_mut(player) {
                state.buff_count = buff_count;
            }
            self.olympiad_buff_cast(npc, player, skill_id, level);
            // NOTE(java-quirk): `skill.applyEffects(player, player)`: the player is the effector.
            self.apply_effects(player, player, SkillKey::new(skill_id, level));
            if let Some(summon) = self.summon_of(player) {
                self.olympiad_buff_cast(npc, summon, skill_id, level);
                self.apply_effects(summon, summon, SkillKey::new(skill_id, level));
            }
        }
        let object_id = ("%objectId%", npc.0.to_string());
        if buff_count > 0 {
            let file = if buff_count == self.olympiad.settings.max_buffs {
                "olympiad_buffs.htm"
            } else {
                "olympiad_5buffs.htm"
            };
            self.olympiad_page(player, npc, file, &[object_id]);
        } else {
            self.olympiad_page(player, npc, "olympiad_nobuffs.htm", &[object_id]);
            self.decay_keep_entry(npc);
        }
        true
    }

    /// `target.broadcastSkillPacket(new MagicSkillUse(target, to, id, level, 0, 0), to)`.
    fn olympiad_buff_cast(&mut self, npc: ObjectId, to: ObjectId, skill_id: i32, level: i32) {
        let (Some((x, y, z)), Some((tx, ty, tz))) = (self.position_of(npc), self.position_of(to)) else {
            return;
        };
        self.broadcast_creature_packet(
            npc,
            &MagicSkillUse {
                creature_id: npc.0,
                target_id: to.0,
                skill_id,
                skill_level: level,
                hit_time: 0,
                reuse_delay: 0,
                creature: SkillPosition { x, y, z },
                ground_location: None,
                target: SkillPosition { x: tx, y: ty, z: tz },
            },
        );
    }

    /// `olympiad <n>`.
    fn olympiad_command(&mut self, player: ObjectId, npc: ObjectId, command: &str) -> Option<()> {
        let value: i32 = command.get(9..10)?.parse().ok()?;
        match value {
            // "show rank for a specific class"
            2 => {
                let class_id: i32 = command.get(11..)?.parse().ok()?;
                if RANKING_CLASSES.contains(&class_id) {
                    let monthly = self.olympiad.settings.show_monthly_winners;
                    let min_matches = self.olympiad.settings.min_matches;
                    self.olympiad_reread(
                        OlympiadRereadRequest::ClassLeaderBoard {
                            class_id,
                            monthly,
                            min_matches,
                        },
                        OlympiadRereadThen {
                            player,
                            npc,
                            action: OlympiadRereadAction::Ranking,
                        },
                    );
                }
            }
            // hero list
            4 => {
                // FOLLOW-UP(M8-HERO-01): `new ExHeroList()` lists `Hero.getHeroes()`.
                self.send(player, &ExHeroList { heroes: Vec::new() });
            }
            // Hero Certification
            5 => {
                // FOLLOW-UP(M8-HERO-01): `Hero.isUnclaimedHero` / `claimHero` (`hero_receive.htm`).
                self.olympiad_page(player, npc, "hero_notreceive.htm", &[]);
            }
            _ => warn!("Olympiad System: Could not send packet for request {value}"),
        }
        Some(())
    }

    /// `Npc.showChatWindow`'s `case 36402`: the buffer page by the buffs left.
    pub(crate) fn olympiad_host_page(&self, player: ObjectId) -> String {
        let count = self.olympiad_buff_count(player);
        if count > 0 {
            if count == self.olympiad.settings.max_buffs {
                format!("{OLYMPIAD_HTML_PATH}olympiad_buffs.htm")
            } else {
                format!("{OLYMPIAD_HTML_PATH}olympiad_5buffs.htm")
            }
        } else {
            format!("{OLYMPIAD_HTML_PATH}olympiad_nobuffs.htm")
        }
    }

    /// Java `OlympiadStat.onCommand` (`/olympiadstat`, user command 109).
    pub(crate) fn user_command_olympiad_stat(&mut self, player: ObjectId) {
        if !self.olympiad.settings.enabled {
            self.send(
                player,
                &SystemMessage::new(SystemMessageId::TheGrandOlympiadGamesAreNotCurrentlyInProgress),
            );
            return;
        }
        let mut noble = player;
        let only_noble = SystemMessage::new(SystemMessageId::ThisCommandCanOnlyBeUsedByANoblesse);
        if let Some(target) = self.target_of(player) {
            if self.players.contains_key(&target) && self.is_noble(target) {
                noble = target;
            } else {
                self.send(player, &only_noble);
                return;
            }
        } else if !self.is_noble(player) {
            self.send(player, &only_noble);
            return;
        }
        let id = noble.0;
        let first = SystemMessage::new(
            SystemMessageId::ForTheCurrentGrandOlympiadYouHaveParticipatedInS1MatchEsS2WinSAndS3DefeatSYouCurrentlyHaveS4OlympiadPointS,
        )
        .with(SystemMessageParam::IntNumber(self.competition_done(id)))
        .with(SystemMessageParam::IntNumber(self.competition_won(id)))
        .with(SystemMessageParam::IntNumber(self.competition_lost(id)))
        .with(SystemMessageParam::IntNumber(self.noble_points(id)));
        self.send(player, &first);
        let second = SystemMessage::new(
            SystemMessageId::YouHaveS1MatchEsRemainingThatYouCanParticipateInThisWeekS21Vs1ClassMatchesS31Vs1MatchesS43Vs3TeamMatches,
        )
        .with(SystemMessageParam::IntNumber(self.remaining_weekly_matches(id)))
        .with(SystemMessageParam::IntNumber(self.remaining_weekly_matches_classed(id)))
        .with(SystemMessageParam::IntNumber(self.remaining_weekly_matches_non_classed(id)))
        .with(SystemMessageParam::IntNumber(self.remaining_weekly_matches_team(id)));
        self.send(player, &second);
    }
}
