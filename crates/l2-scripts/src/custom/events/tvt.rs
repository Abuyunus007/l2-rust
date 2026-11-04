//! Team vs Team event: two teams fight in the Coliseum instance for 20 minutes.
// Off by default: the shipped `config.xml` schedules nothing, so the event only runs when a GM
// starts it (`//event_start TvT`).

use std::collections::BTreeSet;

use l2_util::ObjectId;
use l2_world::script::EventKind;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup, TimerParams};

use super::arena::{
    self, BOTTOM_RIGHT, GHOST_WALKING, SCOREBOARD_FINISH, SCOREBOARD_INITIALIZE, SCOREBOARD_UPDATE,
};
use super::schedule;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum EventState {
    #[default]
    Inactive,
    Participating,
    Starting,
    Started,
}

// NPC
const MANAGER: i32 = 70_010;

// Others
const INSTANCE_ID: i32 = 3049;
const BLUE_DOOR_ID: i32 = 24_190_002;
const RED_DOOR_ID: i32 = 24_190_003;
const MANAGER_SPAWN_LOC: (i32, i32, i32, i32) = (83_425, 148_585, -3406, 32_938);
const BLUE_BUFFER_SPAWN_LOC: (i32, i32, i32, i32) = (147_450, 46_913, -3400, 49_000);
const RED_BUFFER_SPAWN_LOC: (i32, i32, i32, i32) = (151_545, 46_528, -3400, 16_000);
const BLUE_SPAWN_LOC: (i32, i32, i32) = (147_447, 46_722, -3416);
const RED_SPAWN_LOC: (i32, i32, i32) = (151_536, 46_722, -3416);
const BLUE_PEACE_ZONE: &str = "colosseum_peace1";
const RED_PEACE_ZONE: &str = "colosseum_peace2";

// Settings
const REGISTRATION_TIME: i64 = 10; // Minutes
const WAIT_TIME: i64 = 1; // Minutes
const FIGHT_TIME: i64 = 20; // Minutes
const INACTIVITY_TIME: i64 = 2; // Minutes
const MINIMUM_PARTICIPANT_LEVEL: i32 = 76;
const MAXIMUM_PARTICIPANT_LEVEL: i32 = 200;
const MINIMUM_PARTICIPANT_COUNT: usize = 4;
const MAXIMUM_PARTICIPANT_COUNT: usize = 24; // Scoreboard has 25 slots
const PARTY_MEMBER_COUNT: usize = 7;
const REWARD: (i32, i64) = (57, 100_000); // Adena

/// `Team.NONE`, `BLUE`, `RED`.
const TEAM_NONE: u8 = 0;
const TEAM_BLUE: u8 = 1;
const TEAM_RED: u8 = 2;

/// `CommonSkill.FIREWORK` (skill 5965).
const FIREWORK: (i32, i32) = (5965, 1);

/// The tag of the death listeners (`this`).
const LISTENER_TAG: i64 = 0;

/// The static fields of `TvT`: the state, the lists, the scores, the instance and the manager.
#[derive(Default)]
struct TvtState {
    state: EventState,
    player_scores: Vec<(ObjectId, i32)>,
    player_list: Vec<ObjectId>,
    blue_team: Vec<ObjectId>,
    red_team: Vec<ObjectId>,
    blue_score: i32,
    red_score: i32,
    pvp_world: Option<i32>,
    manager_npc: Option<ScriptNpc>,
    team_forfeit: bool,
    /// The players with an `ON_PLAYER_LOGOUT` listener of this script.
    logout_listeners: BTreeSet<ObjectId>,
}

/// Java `custom.events.TeamVsTeam.TvT`.
pub struct TvT;

impl Script for TvT {
    fn name(&self) -> &'static str {
        "TvT"
    }

    fn path(&self) -> &'static str {
        "custom/events/TeamVsTeam"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_talk_id(&[MANAGER]);
        s.add_start_npc(&[MANAGER]);
        s.add_first_talk_id(&[MANAGER]);
        s.set_on_player_logout();
    }

    fn setup_with(&self, s: &mut ScriptSetup, world: &l2_world::World) {
        self.setup(s);
        let zones: Vec<i32> = [BLUE_PEACE_ZONE, RED_PEACE_ZONE]
            .iter()
            .filter_map(|name| world.zone_id_by_name(name))
            .collect();
        s.add_exit_zone_id(&zones);
        s.add_enter_zone_id(&zones);
    }

    fn on_load(&self, ctx: &mut ScriptCtx<'_>) {
        schedule::load_config(ctx, "TvT", "TvT");
    }

    fn is_event(&self) -> bool {
        true
    }

    fn on_timer_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        params: Option<&TimerParams>,
        _npc: Option<ScriptNpc>,
        _player: Option<ObjectId>,
    ) {
        if schedule::is_schedule(event) {
            self.event_start(ctx, None);
            schedule::reschedule(ctx, event, params);
        }
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        // Event not participating, no starting or started return null.
        if !(is_participating(ctx) || is_starting(ctx) || is_started(ctx)) {
            return None;
        }

        let mut htmltext = None;
        match event {
            "Participate" => {
                let player = player?;
                if can_register(ctx, player) {
                    // `DUALBOX_CHECK_MAX_L2EVENT_PARTICIPANTS_PER_IP` is 0 (the dualbox check is
                    // not ported): no IP count.
                    let state = ctx.state::<TvtState>();
                    state.player_list.push(player);
                    arena::put_score(&mut state.player_scores, player, 0);
                    ctx.world_mut().set_registered_on_event(player, true);
                    add_logout_listener(ctx, player);
                    htmltext = Some("registration-success.html".to_owned());
                } else {
                    htmltext = Some("registration-failed.html".to_owned());
                }
            }
            "CancelParticipation" => {
                let player = player?;
                if ctx.world().is_on_event(player) {
                    return None;
                }

                let state = ctx.state::<TvtState>();
                state.player_list.retain(|p| *p != player);
                arena::remove_score(&mut state.player_scores, player);
                remove_listeners(ctx, player);
                ctx.world_mut().set_registered_on_event(player, false);
                htmltext = Some("registration-canceled.html".to_owned());
            }
            "BuffHeal" => {
                return arena::buff_heal(ctx, npc?, player?).or(htmltext);
            }
            "TeleportToArena" => {
                // Set state to STARTING.
                set_state(ctx, EventState::Starting);

                ctx.state::<TvtState>().team_forfeit = false;

                // Remove offline players.
                let list = ctx.state::<TvtState>().player_list.clone();
                for participant in list {
                    if ctx.world().player(participant).is_none() {
                        let state = ctx.state::<TvtState>();
                        state.player_list.retain(|p| *p != participant);
                        arena::remove_score(&mut state.player_scores, participant);
                    }
                }

                // Check if there are enough players to start the event.
                if ctx.state::<TvtState>().player_list.len() < MINIMUM_PARTICIPANT_COUNT {
                    ctx.broadcast_to_all_online_players(
                        "TvT Event: Event was canceled, not enough participants.",
                    );
                    for participant in ctx.state::<TvtState>().player_list.clone() {
                        remove_listeners(ctx, participant);
                        ctx.world_mut().set_registered_on_event(participant, false);
                    }

                    // Set state INACTIVE
                    set_state(ctx, EventState::Inactive);
                    return None;
                }

                // Create the instance.
                let world = ctx.world_mut().create_dynamic_instance(INSTANCE_ID);
                ctx.state::<TvtState>().pvp_world = Some(world);

                // Make sure doors are closed.
                ctx.world_mut().instance_close_door(world, BLUE_DOOR_ID);
                ctx.world_mut().instance_close_door(world, RED_DOOR_ID);

                // Randomize player list and separate teams.
                let mut list = ctx.state::<TvtState>().player_list.clone();
                arena::shuffle(ctx, &mut list);
                ctx.state::<TvtState>().player_list = list.clone();
                let mut team = ctx.get_random_boolean(); // If teams are not even, randomize where extra player goes.
                for participant in list {
                    ctx.world_mut().set_on_event(participant, true);
                    ctx.world_mut().set_registered_on_event(participant, false);
                    if team {
                        ctx.state::<TvtState>().blue_team.push(participant);
                        ctx.world_mut().player_leave_party(participant);
                        ctx.world_mut()
                            .event_teleport(participant, BLUE_SPAWN_LOC, world, 50);
                        ctx.world_mut().set_player_team(participant, TEAM_BLUE);
                        team = false;
                    } else {
                        ctx.state::<TvtState>().red_team.push(participant);
                        ctx.world_mut().player_leave_party(participant);
                        ctx.world_mut()
                            .event_teleport(participant, RED_SPAWN_LOC, world, 50);
                        ctx.world_mut().set_player_team(participant, TEAM_RED);
                        team = true;
                    }

                    add_death_listener(ctx, participant);
                }

                // Make Blue CC.
                let blue = ctx.state::<TvtState>().blue_team.clone();
                ctx.world_mut().event_make_team_parties(&blue, PARTY_MEMBER_COUNT);

                // Make Red CC.
                let red = ctx.state::<TvtState>().red_team.clone();
                ctx.world_mut().event_make_team_parties(&red, PARTY_MEMBER_COUNT);

                // Spawn managers.
                let delay = (WAIT_TIME + FIGHT_TIME) * 60_000;
                let (x, y, z, h) = BLUE_BUFFER_SPAWN_LOC;
                ctx.add_spawn(MANAGER, x, y, z, h, false, delay, world);
                let (x, y, z, h) = RED_BUFFER_SPAWN_LOC;
                ctx.add_spawn(MANAGER, x, y, z, h, false, delay, world);

                // Initialize scores.
                ctx.state::<TvtState>().blue_score = 0;
                ctx.state::<TvtState>().red_score = 0;

                // Initialize scoreboard.
                let scores = ctx.state::<TvtState>().player_scores.clone();
                arena::broadcast_scoreboard(ctx, world, SCOREBOARD_INITIALIZE, &scores);

                // Schedule start.
                for second in (1..=5).rev() {
                    ctx.start_quest_timer(
                        &second.to_string(),
                        (WAIT_TIME * 60_000) - i64::from(second) * 1000,
                        None,
                        None,
                        false,
                    );
                }
                ctx.start_quest_timer("StartFight", WAIT_TIME * 60_000, None, None, false);
            }
            "StartFight" => {
                // Set state STARTED
                set_state(ctx, EventState::Started);

                // Open doors.
                let world = ctx.state::<TvtState>().pvp_world?;
                ctx.world_mut().instance_open_door(world, BLUE_DOOR_ID);
                ctx.world_mut().instance_open_door(world, RED_DOOR_ID);

                // add event FIGHT_TIME
                for participant in ctx.state::<TvtState>().player_list.clone() {
                    arena::send_time_remaining(ctx, participant, (FIGHT_TIME * 60) as i32);
                }

                // Send message.
                arena::broadcast_screen_message(ctx, world, "The fight has began!", 5, true);

                // Schedule finish.
                for second in (1..=10).rev() {
                    ctx.start_quest_timer(
                        &second.to_string(),
                        (FIGHT_TIME * 60_000) - i64::from(second) * 1000,
                        None,
                        None,
                        false,
                    );
                }
                ctx.start_quest_timer("EndFight", FIGHT_TIME * 60_000, None, None, false);
            }
            "EndFight" => {
                let world = ctx.state::<TvtState>().pvp_world?;
                // Close doors.
                ctx.world_mut().instance_close_door(world, BLUE_DOOR_ID);
                ctx.world_mut().instance_close_door(world, RED_DOOR_ID);

                // Disable players.
                let list = ctx.state::<TvtState>().player_list.clone();
                for participant in &list {
                    ctx.world_mut().event_set_player_disabled(*participant, true);
                }

                // Make sure noone is dead.
                for participant in &list {
                    if ctx.is_dead(*participant) {
                        ctx.world_mut().do_revive(*participant);
                    }
                }

                let (forfeit, blue_score, red_score) = {
                    let s = ctx.state::<TvtState>();
                    (s.team_forfeit, s.blue_score, s.red_score)
                };
                let blue = ctx.state::<TvtState>().blue_team.clone();
                let red = ctx.state::<TvtState>().red_team.clone();
                if forfeit {
                    // Team wins by Forfeit.
                    let blue_wins = !(blue.is_empty() && !red.is_empty());
                    let winners = if blue_wins { &blue } else { &red };
                    arena::broadcast_screen_message(
                        ctx,
                        world,
                        &format!(
                            "Team {} won the event by forfeit!",
                            if blue_wins { "Blue" } else { "Red" }
                        ),
                        7,
                        true,
                    );
                    reward_team(ctx, world, winners);
                } else if blue_score > red_score {
                    // Team Blue wins.
                    arena::broadcast_screen_message(ctx, world, "Team Blue won the event!", 7, true);
                    reward_team(ctx, world, &blue);
                } else if red_score > blue_score {
                    // Team Red wins.
                    arena::broadcast_screen_message(ctx, world, "Team Red won the event!", 7, true);
                    reward_team(ctx, world, &red);
                } else {
                    // Tie.
                    arena::broadcast_screen_message(ctx, world, "The event ended with a tie!", 7, true);
                    for participant in &list {
                        ctx.world_mut().broadcast_social_action(*participant, 13);
                    }
                }

                ctx.start_quest_timer("ScoreBoard", 3500, None, None, false);
                ctx.start_quest_timer("TeleportOut", 7000, None, None, false);
            }
            "ScoreBoard" => {
                let world = ctx.state::<TvtState>().pvp_world?;
                let scores = ctx.state::<TvtState>().player_scores.clone();
                arena::broadcast_scoreboard(ctx, world, SCOREBOARD_FINISH, &scores);

                // remove event FIGHT_TIME
                for participant in ctx.state::<TvtState>().player_list.clone() {
                    arena::send_time_remaining(ctx, participant, 0);
                }
            }
            "TeleportOut" => {
                ctx.state::<TvtState>().team_forfeit = false;

                // Remove event listeners.
                let list = ctx.state::<TvtState>().player_list.clone();
                for participant in &list {
                    remove_listeners(ctx, *participant);
                    ctx.world_mut().set_player_team(*participant, TEAM_NONE);
                    ctx.world_mut().set_on_event(*participant, false);
                    ctx.world_mut().player_leave_party(*participant);
                }

                // Destroy world.
                if let Some(world) = ctx.state::<TvtState>().pvp_world.take() {
                    ctx.world_mut().destroy_instance_world(world);
                }

                // Enable players.
                for participant in &list {
                    ctx.world_mut().event_set_player_disabled(*participant, false);
                }

                // Set state INACTIVE
                set_state(ctx, EventState::Inactive);
            }
            "ResurrectPlayer" => {
                let player = player?;
                if ctx.is_dead(player) && ctx.world().is_on_event(player) {
                    let world = ctx.state::<TvtState>().pvp_world?;
                    let (in_blue, in_red) = {
                        let s = ctx.state::<TvtState>();
                        (s.blue_team.contains(&player), s.red_team.contains(&player))
                    };
                    if in_blue {
                        ctx.world_mut().player_set_pending_revive(player, true);
                        ctx.world_mut().event_teleport(player, BLUE_SPAWN_LOC, world, 50);

                        // Make player invulnerable for 30 seconds.
                        ctx.world_mut().apply_effects(player, player, GHOST_WALKING);

                        // Reset existing activity timers.
                        reset_activity_timers(ctx, player); // In case player died in peace zone.
                    } else if in_red {
                        ctx.world_mut().player_set_pending_revive(player, true);
                        ctx.world_mut().event_teleport(player, RED_SPAWN_LOC, world, 50);

                        // Make player invulnerable for 30 seconds.
                        ctx.world_mut().apply_effects(player, player, GHOST_WALKING);

                        // Reset existing activity timers.
                        reset_activity_timers(ctx, player); // In case player died in peace zone.
                    }
                }
            }
            "10" | "9" | "8" | "7" | "6" | "5" | "4" | "3" | "2" | "1" => {
                let world = ctx.state::<TvtState>().pvp_world?;
                arena::broadcast_screen_message(ctx, world, event, 4, false);
            }
            "manager-cancel" => {
                let count = ctx.state::<TvtState>().player_list.len();
                arena::show_manager_page(ctx, npc?, player?, "manager-cancel.html", count);
            }
            "manager-register" => {
                let count = ctx.state::<TvtState>().player_list.len();
                arena::show_manager_page(ctx, npc?, player?, "manager-register.html", count);
            }
            _ => {}
        }

        // Activity timer.
        if event.starts_with("KickPlayer")
            && let Some(player) = player
        {
            let world = ctx.state::<TvtState>().pvp_world?;
            if ctx.world().instance_id_of(player) == world {
                if event.contains("Warning") {
                    arena::send_screen_message(ctx, player, "You have been marked as inactive!", 10);
                } else {
                    ctx.world_mut().set_player_team(player, TEAM_NONE);
                    ctx.world_mut().instance_eject_player(world, player);
                    {
                        let s = ctx.state::<TvtState>();
                        s.player_list.retain(|p| *p != player);
                        arena::remove_score(&mut s.player_scores, player);
                        s.blue_team.retain(|p| *p != player);
                        s.red_team.retain(|p| *p != player);
                    }
                    ctx.world_mut().set_on_event(player, false);
                    remove_listeners(ctx, player);
                    ctx.world()
                        .send_message(player, "You have been kicked for been inactive.");
                    // Manage forfeit.
                    let one_side_left = {
                        let s = ctx.state::<TvtState>();
                        (s.blue_team.is_empty() && !s.red_team.is_empty())
                            || (s.red_team.is_empty() && !s.blue_team.is_empty())
                    };
                    if one_side_left {
                        manage_forfeit(ctx, world);
                    } else {
                        let name = ctx.player_name(player);
                        arena::broadcast_screen_message(
                            ctx,
                            world,
                            &format!("Player {name} was kicked for been inactive!"),
                            7,
                            true,
                        );
                    }

                    arena::send_time_remaining(ctx, player, 0);
                    let scores = ctx.state::<TvtState>().player_scores.clone();
                    ctx.world()
                        .send(player, &arena::scoreboard_packet(ctx, SCOREBOARD_FINISH, &scores));
                }
            }
        }

        if let Some(minutes) = event.strip_prefix("RegistrationWarn:") {
            // `Integer.parseInt(event.split(":")[1])`.
            if let Ok(minutes_left) = minutes.split(':').next().unwrap_or("").parse::<i32>() {
                ctx.broadcast_to_all_online_players(&format!(
                    "TvT Event: Registration opened for {minutes_left} minutes."
                ));
            }
        }

        htmltext
    }

    fn on_first_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        // Event not participating, no starting or started return null.
        if !(is_participating(ctx) || is_starting(ctx) || is_started(ctx)) {
            return None;
        }

        // Player has already registered.
        if ctx.state::<TvtState>().player_list.contains(&player) {
            // Npc is in instance.
            if ctx.world().instance_id_of(npc.object_id) > 0 {
                return Some("manager-buffheal.html".to_owned());
            }

            ctx.start_quest_timer("manager-cancel", 5, Some(npc), Some(player), false);
            return Some("manager-cancel.html".to_owned());
        }

        // Player is not registered.
        ctx.start_quest_timer("manager-register", 5, Some(npc), Some(player), false);
        Some("manager-register.html".to_owned())
    }

    fn on_enter_zone(&self, ctx: &mut ScriptCtx<'_>, creature: ObjectId, zone_id: i32) {
        let Some(player) = ctx.world().acting_player(creature) else {
            return;
        };
        if !ctx.world().is_on_event(player) {
            return;
        }
        let blue_zone = ctx.world().zone_id_by_name(BLUE_PEACE_ZONE) == Some(zone_id);
        let red_zone = ctx.world().zone_id_by_name(RED_PEACE_ZONE) == Some(zone_id);
        let team = ctx.world().team_of(creature);
        let Some(world) = ctx.state::<TvtState>().pvp_world else {
            return;
        };

        // Kick enemy players.
        if blue_zone && team == TEAM_RED {
            ctx.world_mut().event_teleport(creature, RED_SPAWN_LOC, world, 50);
            arena::send_screen_message(ctx, player, "Entering the enemy headquarters is prohibited!", 10);
        }

        if red_zone && team == TEAM_BLUE {
            ctx.world_mut()
                .event_teleport(creature, BLUE_SPAWN_LOC, world, 50);
            arena::send_screen_message(ctx, player, "Entering the enemy headquarters is prohibited!", 10);
        }

        // Start inactivity check.
        if ctx.world().player(creature).is_some()
            && ((blue_zone && team == TEAM_BLUE) || (red_zone && team == TEAM_RED))
        {
            reset_activity_timers(ctx, player);
        }
    }

    fn on_exit_zone(&self, ctx: &mut ScriptCtx<'_>, creature: ObjectId, _zone_id: i32) {
        if ctx.world().player(creature).is_none() {
            return;
        }
        let player = creature;
        if ctx.world().is_on_event(player) {
            ctx.cancel_quest_timer(&format!("KickPlayer{}", player.0), None, Some(player));
            ctx.cancel_quest_timer(&format!("KickPlayerWarning{}", player.0), None, Some(player));

            // Removed invulnerability shield.
            if ctx.is_affected_by_skill(player, GHOST_WALKING.id) {
                ctx.world_mut()
                    .stop_skill_effects_removed(player, GHOST_WALKING.id);
            }
        }
    }

    /// Java `onPlayerLogout` (the `ON_PLAYER_LOGOUT` listener of a registered player).
    fn on_player_logout(&self, ctx: &mut ScriptCtx<'_>, player: ObjectId) {
        if !ctx.state::<TvtState>().logout_listeners.contains(&player) {
            return;
        }

        // Remove player from lists.
        {
            let s = ctx.state::<TvtState>();
            s.player_list.retain(|p| *p != player);
            arena::remove_score(&mut s.player_scores, player);
            s.blue_team.retain(|p| *p != player);
            s.red_team.retain(|p| *p != player);
        }

        if is_started(ctx) {
            arena::send_time_remaining(ctx, player, 0);
        }

        // Manage forfeit.
        let one_side_left = {
            let s = ctx.state::<TvtState>();
            (s.blue_team.is_empty() && !s.red_team.is_empty())
                || (s.red_team.is_empty() && !s.blue_team.is_empty())
        };
        if one_side_left {
            if let Some(world) = ctx.state::<TvtState>().pvp_world {
                manage_forfeit(ctx, world);
            } else {
                ctx.state::<TvtState>().team_forfeit = true;
            }
        }
    }

    /// Java `onPlayerDeath` (the `ON_CREATURE_DEATH` listener of a participant).
    fn on_creature_death(&self, ctx: &mut ScriptCtx<'_>, killer: Option<ObjectId>, victim: ObjectId) {
        if ctx.world().player(victim).is_none() {
            return;
        }
        let killed_player = victim;
        // `event.getAttacker().asPlayer()`: a monster's kill is a NullPointerException.
        let Some(killer) = killer.and_then(|k| ctx.world().acting_player(k)) else {
            return;
        };
        let killer_team = ctx.world().team_of(killer);
        let killed_team = ctx.world().team_of(killed_player);
        let Some(world) = ctx.state::<TvtState>().pvp_world else {
            return;
        };

        // Confirm Blue team kill.
        if killer_team == TEAM_BLUE && killed_team == TEAM_RED {
            let Some(score) = arena::get_score(&ctx.state::<TvtState>().player_scores, killer) else {
                return;
            };
            let s = ctx.state::<TvtState>();
            arena::put_score(&mut s.player_scores, killer, score + 1);
            s.blue_score += 1;
            broadcast_score_message(ctx, world);
            let scores = ctx.state::<TvtState>().player_scores.clone();
            arena::broadcast_scoreboard(ctx, world, SCOREBOARD_UPDATE, &scores);
        }

        // Confirm Red team kill.
        if killer_team == TEAM_RED && killed_team == TEAM_BLUE {
            let Some(score) = arena::get_score(&ctx.state::<TvtState>().player_scores, killer) else {
                return;
            };
            let s = ctx.state::<TvtState>();
            arena::put_score(&mut s.player_scores, killer, score + 1);
            s.red_score += 1;
            broadcast_score_message(ctx, world);
            let scores = ctx.state::<TvtState>().player_scores.clone();
            arena::broadcast_scoreboard(ctx, world, SCOREBOARD_UPDATE, &scores);
        }

        // Auto release after 10 seconds.
        ctx.start_quest_timer("ResurrectPlayer", 10_000, None, Some(killed_player), false);
    }

    fn event_start(&self, ctx: &mut ScriptCtx<'_>, _event_maker: Option<ObjectId>) -> bool {
        // Set state PARTICIPATING
        set_state(ctx, EventState::Participating);

        // Cancel timers. (In case event started immediately after another event was canceled.).
        ctx.cancel_all_quest_timers();

        // Clear player lists.
        {
            let s = ctx.state::<TvtState>();
            s.player_list.clear();
            s.player_scores.clear();
            s.blue_team.clear();
            s.red_team.clear();
        }

        // Spawn event manager.
        let (x, y, z, h) = MANAGER_SPAWN_LOC;
        let manager = ctx.add_spawn(MANAGER, x, y, z, h, false, REGISTRATION_TIME * 60_000, 0);
        ctx.state::<TvtState>().manager_npc = manager;
        ctx.start_quest_timer("TeleportToArena", REGISTRATION_TIME * 60_000, None, None, false);

        // Send message to players.
        ctx.broadcast_to_all_online_players(&format!(
            "TvT Event: Registration opened for {REGISTRATION_TIME} minutes."
        ));
        ctx.broadcast_to_all_online_players("TvT Event: You can register at Giran TvT Event Manager.");

        for warn in [10, 5, 4, 3, 2, 1] {
            if REGISTRATION_TIME > warn {
                let delay = (REGISTRATION_TIME - warn) * 60_000;
                ctx.start_quest_timer(&format!("RegistrationWarn:{warn}"), delay, None, None, false);
            }
        }

        true
    }

    fn event_stop(&self, ctx: &mut ScriptCtx<'_>) -> bool {
        // Despawn event manager.
        let Some(manager) = ctx.state::<TvtState>().manager_npc else {
            return false;
        };
        ctx.delete_npc(manager);

        // Cancel timers.
        ctx.cancel_all_quest_timers();

        // Remove participants.
        let list = ctx.state::<TvtState>().player_list.clone();
        let started = is_started(ctx);
        for participant in &list {
            remove_listeners(ctx, *participant);
            ctx.world_mut().set_player_team(*participant, TEAM_NONE);
            ctx.world_mut().set_registered_on_event(*participant, false);
            ctx.world_mut().set_on_event(*participant, false);
            ctx.world_mut().event_set_player_disabled(*participant, false);
            if started {
                arena::send_time_remaining(ctx, *participant, 0);
            }
        }

        if started && let Some(world) = ctx.state::<TvtState>().pvp_world {
            let scores = ctx.state::<TvtState>().player_scores.clone();
            arena::broadcast_scoreboard(ctx, world, SCOREBOARD_FINISH, &scores);
        }

        if let Some(world) = ctx.state::<TvtState>().pvp_world.take() {
            ctx.world_mut().destroy_instance_world(world);
        }

        // Send message to players.
        ctx.broadcast_to_all_online_players("TvT Event: Event was canceled.");

        // Set state PARTICIPATING
        set_state(ctx, EventState::Inactive);
        true
    }
}

fn set_state(ctx: &mut ScriptCtx<'_>, state: EventState) {
    ctx.state::<TvtState>().state = state;
}

fn is_participating(ctx: &mut ScriptCtx<'_>) -> bool {
    ctx.state::<TvtState>().state == EventState::Participating
}

fn is_starting(ctx: &mut ScriptCtx<'_>) -> bool {
    ctx.state::<TvtState>().state == EventState::Starting
}

fn is_started(ctx: &mut ScriptCtx<'_>) -> bool {
    ctx.state::<TvtState>().state == EventState::Started
}

/// Java `canRegister(player)`.
fn can_register(ctx: &mut ScriptCtx<'_>, player: ObjectId) -> bool {
    let (registered, count) = {
        let s = ctx.state::<TvtState>();
        (s.player_list.contains(&player), s.player_list.len())
    };
    arena::can_register(
        ctx,
        player,
        registered,
        count,
        (MINIMUM_PARTICIPANT_LEVEL, MAXIMUM_PARTICIPANT_LEVEL),
        MAXIMUM_PARTICIPANT_COUNT,
    )
}

/// The winners' fireworks, social action and adena.
fn reward_team(ctx: &mut ScriptCtx<'_>, world: i32, winners: &[ObjectId]) {
    for participant in winners {
        if ctx.world().instance_id_of(*participant) == world {
            let (hit_time, reuse_delay) = ctx.world().skill_hit_and_reuse(FIREWORK.0, FIREWORK.1);
            ctx.broadcast_magic_skill_use(
                *participant,
                *participant,
                *participant,
                FIREWORK.0,
                FIREWORK.1,
                hit_time,
                reuse_delay,
            );
            ctx.world_mut().broadcast_social_action(*participant, 3);
            ctx.give_items(*participant, REWARD.0, REWARD.1);
        }
    }
}

fn broadcast_score_message(ctx: &mut ScriptCtx<'_>, world: i32) {
    let (blue, red) = {
        let s = ctx.state::<TvtState>();
        (s.blue_score, s.red_score)
    };
    ctx.world().instance_broadcast_packet(
        world,
        &l2_protocol_game::server::ExShowScreenMessage::styled_text(
            &format!("Blue: {blue} - Red: {red}"),
            BOTTOM_RIGHT,
            15_000,
            0,
            true,
            false,
        ),
    );
}

fn add_logout_listener(ctx: &mut ScriptCtx<'_>, player: ObjectId) {
    ctx.state::<TvtState>().logout_listeners.insert(player);
}

fn add_death_listener(ctx: &mut ScriptCtx<'_>, player: ObjectId) {
    ctx.add_listener(player, EventKind::CreatureDeath, LISTENER_TAG);
}

fn remove_listeners(ctx: &mut ScriptCtx<'_>, player: ObjectId) {
    ctx.state::<TvtState>().logout_listeners.remove(&player);
    ctx.remove_listeners(player, EventKind::CreatureDeath);
}

/// Java `resetActivityTimers(player)`.
fn reset_activity_timers(ctx: &mut ScriptCtx<'_>, player: ObjectId) {
    let kick = format!("KickPlayer{}", player.0);
    let warning = format!("KickPlayerWarning{}", player.0);
    ctx.cancel_quest_timer(&kick, None, Some(player));
    ctx.cancel_quest_timer(&warning, None, Some(player));
    let started = is_started(ctx);
    ctx.start_quest_timer(
        &kick,
        if started {
            INACTIVITY_TIME * 60_000
        } else {
            (INACTIVITY_TIME * 60_000) + (WAIT_TIME * 60_000)
        },
        None,
        Some(player),
        false,
    );
    ctx.start_quest_timer(
        &warning,
        if started {
            (INACTIVITY_TIME / 2) * 60_000
        } else {
            ((INACTIVITY_TIME / 2) * 60_000) + (WAIT_TIME * 60_000)
        },
        None,
        Some(player),
        false,
    );
}

/// Java `manageForfeit()`.
fn manage_forfeit(ctx: &mut ScriptCtx<'_>, world: i32) {
    ctx.state::<TvtState>().team_forfeit = true;
    for second in (1..=10).rev() {
        ctx.cancel_quest_timer(&second.to_string(), None, None);
    }
    ctx.cancel_quest_timer("EndFight", None, None);
    ctx.start_quest_timer("EndFight", 10_000, None, None, false);
    arena::broadcast_screen_message(ctx, world, "Enemy team forfeit!", 7, true);
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::{ScriptTest, names_to, pages_to};

    const NPCS: &str = r#"<npc id="70010" type="Folk" level="1" name="Manager"/>"#;
    const ITEMS: &str =
        r#"<item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>"#;
    const DIR: &str = "data/scripts/custom/events/TeamVsTeam";

    fn scene(players: i32) -> (ScriptTest, ObjectId) {
        let mut registry = ScriptRegistry::new();
        registry.add(TvT);
        let pages: Vec<(String, String)> = [
            (
                "manager-register.html",
                r#"<html><body>register %player_numbers% <a action="bypass -h Script TvT Participate">in</a></body></html>"#,
            ),
            (
                "manager-cancel.html",
                r#"<html><body>cancel %player_numbers% <a action="bypass -h Script TvT CancelParticipation">out</a></body></html>"#,
            ),
            ("registration-success.html", "<html><body>success</body></html>"),
            ("registration-canceled.html", "<html><body>canceled</body></html>"),
            ("registration-failed.html", "<html><body>failed</body></html>"),
        ]
        .iter()
        .map(|(n, t)| (format!("{DIR}/{n}"), (*t).to_owned()))
        .collect();
        let refs: Vec<(&str, &str)> = pages.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
        let mut t = ScriptTest::new(registry, NPCS, ITEMS, &refs);
        for id in 1..=players {
            t.enter(id, 80, vec![]);
        }
        // The event's manager stands in Giran; a second one next to the players does the talking.
        let manager = t.spawn(MANAGER, 20);
        t.take();
        (t, manager)
    }

    fn start(t: &mut ScriptTest) -> bool {
        let mut started = false;
        t.world.run_as_script("TvT", |ctx| {
            started = TvT.event_start(ctx, Some(ObjectId(1)));
        });
        started
    }

    fn register(t: &mut ScriptTest, player: i32, manager: ObjectId) -> String {
        t.talk(player, manager);
        t.take();
        t.bypass(player, "Script TvT Participate");
        pages_to(&t.take(), player).pop().unwrap_or_default()
    }

    #[test]
    fn switched_off_nothing_runs_and_the_manager_has_no_page() {
        let (mut t, manager) = scene(1);
        t.ticks(10);
        // The event never started: the registration link and the page do nothing.
        t.talk(1, manager);
        assert!(pages_to(&t.take(), 1).iter().all(|p| !p.contains("register")));
        t.bypass(1, "Script TvT Participate");
        assert!(pages_to(&t.take(), 1).is_empty());
        assert!(!t.world.is_registered_on_event(ObjectId(1)));
    }

    #[test]
    fn too_few_players_cancel_the_event() {
        let (mut t, manager) = scene(3);
        assert!(start(&mut t));
        assert!(register(&mut t, 1, manager).contains("success"));
        assert!(t.world.is_registered_on_event(ObjectId(1)));
        // A character below level 76 is refused.
        t.enter(9, 50, vec![]);
        assert!(register(&mut t, 9, manager).contains("failed"));
        assert!(!t.world.is_registered_on_event(ObjectId(9)));
        t.wait_ms(10 * 60_000 + 2_000);
        // One player is fewer than four: the registration ends, the player is free again.
        assert!(!t.world.is_registered_on_event(ObjectId(1)));
        assert!(!t.world.is_on_event(ObjectId(1)));
    }

    #[test]
    fn the_whole_event_blue_wins_and_everybody_comes_out() {
        let (mut t, manager) = scene(4);
        assert!(start(&mut t));
        for id in 1..=4 {
            assert!(register(&mut t, id, manager).contains("success"), "player {id}");
        }
        // The cancel page of a registered player.
        t.talk(1, manager);
        t.ticks(2);
        assert!(pages_to(&t.take(), 1).iter().any(|p| p.contains("cancel 4")));

        t.wait_ms(10 * 60_000 + 2_000);
        let players: Vec<ObjectId> = (1..=4).map(ObjectId).collect();
        let blue: Vec<ObjectId> = players
            .iter()
            .copied()
            .filter(|p| t.world.team_of(*p) == TEAM_BLUE)
            .collect();
        let red: Vec<ObjectId> = players
            .iter()
            .copied()
            .filter(|p| t.world.team_of(*p) == TEAM_RED)
            .collect();
        assert_eq!((blue.len(), red.len()), (2, 2));
        let instance = t.world.instance_id_of(blue[0]);
        assert!(instance > 0);
        assert!(
            players
                .iter()
                .all(|p| t.world.is_on_event(*p) && t.world.instance_id_of(*p) == instance)
        );
        // Teams of two make a party each.
        assert!(t.world.is_in_party_with(blue[0], blue[1]));
        assert!(!t.world.is_in_party_with(blue[0], red[0]));
        // They attack only the other team.
        assert!(t.world.is_auto_attackable(red[0], blue[0]));
        assert!(!t.world.is_auto_attackable(blue[1], blue[0]));

        // The fight starts after a minute; a blue kill scores.
        t.wait_ms(60_000);
        t.take();
        t.world.do_die(red[0], Some(blue[0]));
        t.ticks(2);
        assert!(
            names_to(&t.take(), blue[1].0)
                .iter()
                .any(|n| n == "EX_PVP_MATCH_CC_RECORD")
        );
        // Ten seconds later the dead player is resurrected at its base.
        t.wait_ms(10_500);

        // The fight ends after twenty minutes: blue wins 1:0 and gets the adena.
        t.wait_ms(20 * 60_000);
        t.wait_ms(1_000);
        for p in &blue {
            assert_eq!(t.count(p.0, 57), 100_000, "winner {p:?}");
        }
        for p in &red {
            assert_eq!(t.count(p.0, 57), 0);
        }
        // Seven seconds after the end everybody is out.
        t.wait_ms(8_000);
        assert!(
            players
                .iter()
                .all(|p| !t.world.is_on_event(*p) && t.world.team_of(*p) == TEAM_NONE)
        );
        assert!(players.iter().all(|p| t.world.instance_id_of(*p) == 0));
        // Inactive again: the next registration is not taken.
        assert!(register(&mut t, 1, manager).is_empty());
    }

    #[test]
    fn the_event_can_be_stopped_while_it_runs() {
        let (mut t, manager) = scene(1);
        assert!(start(&mut t));
        assert!(register(&mut t, 1, manager).contains("success"));
        let mut stopped = false;
        t.world.run_as_script("TvT", |ctx| stopped = TvT.event_stop(ctx));
        assert!(stopped);
        assert!(!t.world.is_registered_on_event(ObjectId(1)));
    }
}
