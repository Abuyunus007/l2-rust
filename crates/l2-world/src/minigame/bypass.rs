//! The NPCs of the minigames: the Kratei's Cube entrance and match managers, the coliseum
//! curators and the coliseum helper.
// Java: entity/actor/instance/KrateisCubeManager.java (`onBypassFeedback`)
// Java: entity/actor/instance/KrateisMatchManager.java (`onBypassFeedback`)
// Java: entity/actor/instance/UCManager.java (`onBypassFeedback`)
// Java: entity/actor/instance/UCHelper.java (`onBypassFeedback`)
//
// A command the NPC does not know goes on to `super.onBypassFeedback` in Java; here the function
// answers `false` and the caller goes on.

use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{ActionFailed, ExPvpMatchCcRetire, NpcHtmlMessage};
use l2_util::ObjectId;
use tracing::warn;

use super::UC_STATE_NONE;
use crate::world::World;

/// `Location[][] LOCATIONS` of `UCHelper`: two places per coliseum.
const HELPER_LOCATIONS: [[(i32, i32, i32); 2]; 5] = [
    [(-84451, -45452, -10728), (-84580, -45587, -10728)],
    [(-86154, -50429, -10728), (-86118, -50624, -10728)],
    [(-82009, -53652, -10728), (-81802, -53665, -10728)],
    [(-77603, -50673, -10728), (-77586, -50503, -10728)],
    [(-79186, -45644, -10728), (-79309, -45561, -10728)],
];
/// Where the Kratei's Cube managers send a player back to.
const KRATEIS_EXIT: (i32, i32, i32) = (-70381, -70937, -1428);
/// `TeleportToFI`.
const FANTASY_ISLE: (i32, i32, i32) = (-59193, -56893, -2034);
const KRATEIS_HTML: &str = "data/html/krateisCube/";
const COLISEUM_HTML: &str = "data/html/undergroundColiseum/";

impl World {
    /// The `onBypassFeedback` of the four minigame NPC types; `false` for any other NPC or an
    /// unknown command.
    pub(crate) fn minigame_npc_bypass_feedback(&mut self, npc: ObjectId, player: ObjectId, command: &str) -> bool {
        let Some(npc_type) = self
            .npcs
            .get(&npc)
            .and_then(|n| self.npc_world.template(n.npc_id))
            .map(|t| t.npc_type.clone())
        else {
            return false;
        };
        match npc_type.as_str() {
            "KrateisCubeManager" => self.krateis_cube_manager_bypass(npc, player, command),
            "KrateisMatchManager" => self.krateis_match_manager_bypass(npc, player, command),
            "UCManager" => self.uc_manager_bypass(npc, player, command),
            "UCHelper" => self.uc_helper_bypass(player, command),
            _ => false,
        }
    }

    /// Java `Npc.showChatWindow(player, filename)`: the page with `%objectId%`, then `ActionFailed`.
    fn minigame_show_chat_window(&mut self, npc: ObjectId, player: ObjectId, path: &str) {
        let Some(mut html) = self.html_from_file(player, npc.0, path) else {
            return;
        };
        html.html = html.html.replace("%objectId%", &npc.0.to_string());
        self.send_html(player, &html);
        self.send(player, &ActionFailed);
    }

    /// `html.setFile(player, path)` and `player.sendPacket(html)` with some `replace` calls.
    fn minigame_send_page(&mut self, npc: ObjectId, player: ObjectId, path: &str, replace: &[(&str, &str)]) {
        let Some(mut html): Option<NpcHtmlMessage> = self.html_from_file(player, npc.0, path) else {
            return;
        };
        for (pattern, value) in replace {
            html.html = html.html.replace(pattern, value);
        }
        self.send_html(player, &html);
    }

    // ------------------------------------------------------------------ Kratei's Cube

    /// `KrateisCubeManager.onBypassFeedback`.
    fn krateis_cube_manager_bypass(&mut self, npc: ObjectId, player: ObjectId, command: &str) -> bool {
        if command.starts_with("Register") {
            self.krateis_register_command(npc, player, command);
        } else if command.starts_with("SeeList") {
            let level = self.players.get(&player).map_or(0, |p| p.core.level);
            let page = if level < 70 { "32503-09.htm" } else { "32503-02.htm" };
            self.minigame_show_chat_window(npc, player, &format!("{KRATEIS_HTML}{page}"));
        } else if command.starts_with("Cancel") {
            let ids: Vec<i32> = self.minigames.krateis.arenas.keys().copied().collect();
            for arena in ids {
                if self.krateis_remove_player(arena, player) {
                    self.minigame_show_chat_window(npc, player, &format!("{KRATEIS_HTML}32503-05.htm"));
                    break;
                }
            }
        } else if command.starts_with("TeleportToFI") {
            self.minigame_tele(player, FANTASY_ISLE);
            if let Some(pet) = self.summon_of(player) {
                self.minigame_tele(pet, FANTASY_ISLE);
            }
        } else {
            return false;
        }
        true
    }

    /// The "Register" branch.
    fn krateis_register_command(&mut self, npc: ObjectId, player: ObjectId, command: &str) {
        // `(player.getInventoryLimit() * 0.8) <= player.getInventory().getSize()`.
        let limit = self.stats(player).map_or(0, |s| s.inventory_limit());
        let size = self.players.get(&player).map_or(0, |p| {
            p.data
                .items
                .iter()
                .filter(|i| i.loc == crate::inventory::LOC_INVENTORY || i.loc == crate::inventory::LOC_PAPERDOLL)
                .count()
        });
        if f64::from(limit) * 0.8 <= size as f64 {
            self.minigame_system_message(
                player,
                SystemMessageId::YouCanProceedOnlyWhenTheInventoryWeightIsBelow80PercentAndTheQuantityIsBelow90Percent,
            );
            self.minigame_show_chat_window(npc, player, &format!("{KRATEIS_HTML}32503-08.htm"));
            return;
        }
        let simultaneous = SystemMessageId::YouCannotBeSimultaneouslyRegisteredForPvpMatchesSuchAsTheOlympiadUndergroundColiseumAerialCleftKrateiSCubeAndHandySBlockCheckers;
        if self.krateis_arena_of(player).is_none()
            && (self.minigame_olympiad_registered(player)
                || self.minigame_in_olympiad_mode(player)
                || self.is_registered_on_event(player))
        {
            self.minigame_system_message(player, simultaneous);
            return;
        }
        let party_in_coliseum = self
            .party_of(player)
            .is_some_and(|p| self.uc_party_state(p).is_some());
        if party_in_coliseum || self.uc_state_of(player) > UC_STATE_NONE {
            self.minigame_system_message(player, simultaneous);
            return;
        }
        if self.is_cursed_weapon_equipped(player) {
            self.minigame_system_message(
                player,
                SystemMessageId::YouCannotRegisterWhileInPossessionOfACursedWeapon,
            );
            return;
        }
        // `Integer.parseInt(command.substring(9, 10).trim())`.
        let Some(arena_id) = command
            .get(9..10)
            .and_then(|digit| digit.trim().parse::<i32>().ok())
        else {
            warn!("KrateisCubeManager: bad Register command {command:?}");
            return;
        };
        let Some((min_level, max_level)) = self
            .minigames
            .krateis
            .arenas
            .get(&arena_id)
            .map(|a| (a.def.min_level, a.def.max_level))
        else {
            self.minigame_show_chat_window(npc, player, &format!("{KRATEIS_HTML}32503-09.htm"));
            return;
        };
        let level = self.players.get(&player).map_or(0, |p| p.core.level);
        if level < min_level || level > max_level {
            self.minigame_show_chat_window(npc, player, &format!("{KRATEIS_HTML}32503-06.htm"));
            return;
        }
        let page = if self.krateis_is_register_time() {
            if self.krateis_add_register_player(arena_id, player) {
                "32503-03.htm"
            } else {
                "32503-04.htm"
            }
        } else {
            "32503-07.htm"
        };
        self.minigame_show_chat_window(npc, player, &format!("{KRATEIS_HTML}{page}"));
    }

    /// `KrateisMatchManager.onBypassFeedback`.
    fn krateis_match_manager_bypass(&mut self, npc: ObjectId, player: ObjectId, command: &str) -> bool {
        let arena = self
            .minigames
            .krateis
            .arenas
            .iter()
            .find(|(_, a)| a.manager == Some(npc))
            .map(|(id, _)| *id);
        let registered = arena.is_some_and(|a| {
            self.minigames
                .krateis
                .arenas
                .get(&a)
                .is_some_and(|arena| arena.players.contains_key(&player))
        });
        if command.starts_with("TeleToArena") {
            match arena.filter(|_| registered) {
                Some(arena_id) => {
                    let active = self
                        .minigames
                        .krateis
                        .arenas
                        .get(&arena_id)
                        .is_some_and(|a| a.active_now);
                    if active {
                        if let Some(place) = self.krateis_random_place(arena_id, true) {
                            self.minigame_tele(player, place);
                        }
                        self.krateis_add_effects(arena_id, player);
                    } else {
                        let npc_id = self.npcs.get(&npc).map_or(0, |n| n.npc_id);
                        self.minigame_show_chat_window(npc, player, &format!("{KRATEIS_HTML}{npc_id}-01.htm"));
                    }
                }
                None => {
                    self.minigame_system_message(
                        player,
                        SystemMessageId::YouCannotEnterBecauseYouDoNotMeetTheRequirements,
                    );
                    self.minigame_tele(player, KRATEIS_EXIT);
                }
            }
        } else if command.starts_with("TeleFromArena") {
            if let Some(arena_id) = arena.filter(|_| registered) {
                self.krateis_remove_player(arena_id, player);
                self.stop_all_effects(player);
                self.send(player, &ExPvpMatchCcRetire);
                self.broadcast_status_update(player);
                self.broadcast_user_info(player);
                if let Some(pet) = self.summon_of(player) {
                    self.stop_all_effects(pet);
                }
            }
            self.minigame_tele(player, KRATEIS_EXIT);
        } else {
            return false;
        }
        true
    }

    // ------------------------------------------------------------------ Underground Coliseum

    /// `UCManager.onBypassFeedback`.
    fn uc_manager_bypass(&mut self, npc: ObjectId, player: ObjectId, command: &str) -> bool {
        let mut tokens = command.split(' ').filter(|t| !t.is_empty());
        let Some(action) = tokens.next() else {
            return false;
        };
        let arena_arg = tokens.next();
        if action.eq_ignore_ascii_case("register") {
            self.uc_register(npc, player, arena_arg);
        } else if action.eq_ignore_ascii_case("cancel") {
            let Some(party) = self.party_of(player).filter(|_| self.is_party_leader(player)) else {
                return true;
            };
            if self.coliseum_cancel_waiting(party) {
                self.minigame_send_page(npc, player, &format!("{COLISEUM_HTML}registrantionCanceled.htm"), &[]);
            }
        } else if action.eq_ignore_ascii_case("bestTeam") {
            let Some(arena) = arena_arg.and_then(|a| a.parse::<i32>().ok()) else {
                warn!("UCManager: bad bestTeam command {command:?}");
                return true;
            };
            if self.coliseum_arena_def(arena).is_none() {
                // Java: `arena.getId()` throws.
                return true;
            }
            match self.coliseum_best_team(arena) {
                Some((leader, wins)) => self.minigame_send_page(
                    npc,
                    player,
                    &format!("{COLISEUM_HTML}bestTeam.htm"),
                    &[("%name%", &leader), ("%best%", &wins.to_string())],
                ),
                None => {
                    self.minigame_send_page(npc, player, &format!("{COLISEUM_HTML}view-most-wins.htm"), &[]);
                }
            }
        } else if action.eq_ignore_ascii_case("listTeams") {
            let Some(arena) = arena_arg.and_then(|a| a.parse::<i32>().ok()) else {
                warn!("UCManager: bad listTeams command {command:?}");
                return true;
            };
            if self.coliseum_arena_def(arena).is_none() {
                self.send_message(player, "This arena is temporarly unavailable.");
                return true;
            }
            let waiting = self.coliseum_waiting(arena);
            let mut list = String::new();
            for i in 1..=5usize {
                match waiting.get(i - 1) {
                    None => list.push_str(&format!("{i}. (Participating Team: Team)<br>")),
                    Some(party) => {
                        let mut names = String::new();
                        for member in self.party_members(*party) {
                            names.push_str(&self.minigame_player_name(member));
                            names.push(';');
                        }
                        list.push_str(&format!(
                            "{i}. (Participating Team: <font color=00ffff>{names}</font>)<br>"
                        ));
                    }
                }
            }
            self.minigame_send_page(
                npc,
                player,
                &format!("{COLISEUM_HTML}view-participating-teams.htm"),
                &[("%list%", &list)],
            );
        } else {
            return false;
        }
        true
    }

    /// The "register" branch of `UCManager`.
    fn uc_register(&mut self, npc: ObjectId, player: ObjectId, arena_arg: Option<&str>) {
        let page = |name: &str| format!("{COLISEUM_HTML}{name}");
        let Some(party) = self.party_of(player) else {
            self.minigame_send_page(npc, player, &page("noTeam.htm"), &[]);
            return;
        };
        if self.minigame_olympiad_registered(player)
            || self.minigame_in_olympiad_mode(player)
            || self.is_registered_on_event(player)
        {
            self.minigame_system_message(
                player,
                SystemMessageId::YouCannotBeSimultaneouslyRegisteredForPvpMatchesSuchAsTheOlympiadUndergroundColiseumAerialCleftKrateiSCubeAndHandySBlockCheckers,
            );
            return;
        }
        if self.is_cursed_weapon_equipped(player) {
            self.minigame_system_message(
                player,
                SystemMessageId::YouCannotRegisterWhileInPossessionOfACursedWeapon,
            );
            return;
        }
        if !self.coliseum_is_started() {
            self.minigame_send_page(npc, player, &page("notStarted.htm"), &[]);
            return;
        }
        if !self.is_party_leader(player) {
            self.minigame_send_page(npc, player, &page("notPartyLeader.htm"), &[]);
            return;
        }
        if matches!(
            self.uc_party_state(party),
            Some(super::UcPartyState::Waiting { .. })
        ) {
            self.minigame_send_page(npc, player, &page("alreadyRegistered.htm"), &[]);
            return;
        }
        let Some(arena_id) = arena_arg.and_then(|a| a.parse::<i32>().ok()) else {
            warn!("UCManager: bad register command");
            return;
        };
        let Some(def) = self.coliseum_arena_def(arena_id) else {
            self.send_message(player, "This arena is temporarly unavailable.");
            return;
        };
        if self.coliseum_arena_teams_taken(arena_id) && self.coliseum_arena_has_party(arena_id, party) {
            self.minigame_send_page(npc, player, &page("alreadyRegistered.htm"), &[]);
            return;
        }
        let mut real_count = 0;
        for member in self.party_members(party) {
            let class_level = self
                .players
                .get(&member)
                .and_then(|p| u16::try_from(p.data.class_id).ok())
                .map_or(0, |c| l2_data::ClassId(c).class_level());
            let level = self.players.get(&member).map_or(0, |p| p.core.level);
            if class_level < 2 || !(level >= def.min_level && level <= def.max_level) {
                let name = self.minigame_player_name(member);
                self.minigame_send_page(npc, player, &page("wrongLevel.htm"), &[("%name%", &name)]);
                return;
            }
            real_count += 1;
        }
        if real_count < self.minigames.settings.uc_party_size {
            self.minigame_send_page(npc, player, &page("notEnoughMembers.htm"), &[]);
            return;
        }
        if self.coliseum_waiting_len(arena_id) >= 5 {
            self.minigame_send_page(npc, player, &page("arenaFull.htm"), &[]);
            return;
        }
        self.coliseum_register_party(arena_id, party);
        self.minigame_send_page(npc, player, &page("registered.htm"), &[]);
    }

    /// `UCHelper.onBypassFeedback`: "coliseum N" takes the player to one of two places.
    fn uc_helper_bypass(&mut self, player: ObjectId, command: &str) -> bool {
        let mut tokens = command.split(' ').filter(|t| !t.is_empty());
        let Some(action) = tokens.next() else {
            return false;
        };
        if !action.starts_with("coliseum") {
            return false;
        }
        let index = tokens.next().and_then(|t| t.parse::<usize>().ok());
        let Some(places) = index.and_then(|i| HELPER_LOCATIONS.get(i)) else {
            warn!("UCHelper: bad coliseum command {command:?}");
            return true;
        };
        let pick = self.rng().below(places.len() as i32) as usize;
        self.minigame_tele(player, places[pick]);
        true
    }
}
