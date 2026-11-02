//! Capture the Flag event: two teams fight in the Coliseum instance for 20 minutes and carry
//! the flag of the enemy to their own headquarters.
// Java: dist/game/data/scripts/custom/events/CaptureTheFlag/CtF.java
//
// Off by default: the shipped `config.xml` schedules nothing, so the event only runs when a GM
// starts it (`//event_start CtF`).

use std::collections::BTreeSet;

use l2_protocol_game::server::{ChatType, ExShowScreenMessage, NpcHtmlMessage};
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
const MANAGER: i32 = 70_012;

// Others
const INSTANCE_ID: i32 = 3050;
const BLUE_DOOR_ID: i32 = 24_190_002;
const RED_DOOR_ID: i32 = 24_190_003;
/// Blue Flag NPC ID
const BLUE_TEAM_HEADQUARTERS: i32 = 82_016;
/// Blue Flag x,y,z[,heading] Location.
const BLUE_TEAM_HEADQUARTERS_LOC: (i32, i32, i32, i32) = (148_487, 46_703, -3414, 297);
/// Blue Flag Item id
const BLUE_TEAM_FLAG_ITEM_ID: i32 = 13_531;
/// Red Flag NPC ID
const RED_TEAM_HEADQUARTERS: i32 = 82_017;
/// Red Flag x,y,z[,heading] location.
const RED_TEAM_HEADQUARTERS_LOC: (i32, i32, i32, i32) = (150_452, 46_714, -3414, 34_490);
/// red Flag Item ID
const RED_TEAM_FLAG_ITEM_ID: i32 = 13_534;
const MANAGER_SPAWN_LOC: (i32, i32, i32, i32) = (83_425, 148_585, -3406, 32_938);
const BLUE_BUFFER_SPAWN_LOC: (i32, i32, i32, i32) = (147_450, 46_913, -3400, 49_000);
const RED_BUFFER_SPAWN_LOC: (i32, i32, i32, i32) = (151_545, 46_528, -3400, 16_000);
const BLUE_SPAWN_ZONE: &str = "blue_team_spawn";
const RED_SPAWN_ZONE: &str = "red_team_spawn";
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

const BLUE_TEAM_NAME: &str = "Blue";
const RED_TEAM_NAME: &str = "Red";

/// `Team.NONE`, `BLUE`, `RED`.
const TEAM_NONE: u8 = 0;
const TEAM_BLUE: u8 = 1;
const TEAM_RED: u8 = 2;

/// `Inventory.PAPERDOLL_RHAND` and `PAPERDOLL_LHAND`.
const PAPERDOLL_RHAND: usize = 5;
const PAPERDOLL_LHAND: usize = 7;

/// `CommonSkill.FIREWORK` (skill 5965).
const FIREWORK: (i32, i32) = (5965, 1);

/// The tag of the death listeners (`this`).
const LISTENER_TAG: i64 = 0;

/// What a flag carrier took off: the object and the item id (Java keeps the `Item`).
type Worn = Option<(ObjectId, i32)>;

/// The static fields of `CtF`.
#[derive(Default)]
struct CtfState {
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
    flag_blue_spawn: Option<ScriptNpc>,
    flag_red_spawn: Option<ScriptNpc>,
    blue_team_carrier: Option<ObjectId>,
    red_team_carrier: Option<ObjectId>,
    blue_carrier_r_hand: Worn,
    blue_carrier_l_hand: Worn,
    red_carrier_r_hand: Worn,
    red_carrier_l_hand: Worn,
    /// The players with an `ON_PLAYER_LOGOUT` listener of this script.
    logout_listeners: BTreeSet<ObjectId>,
}

impl CtfState {
    fn in_blue(&self, player: ObjectId) -> bool {
        self.blue_team.contains(&player)
    }

    fn in_red(&self, player: ObjectId) -> bool {
        self.red_team.contains(&player)
    }
}

/// Java `custom.events.CaptureTheFlag.CtF`.
pub struct CtF;

impl Script for CtF {
    fn name(&self) -> &'static str {
        "CtF"
    }

    fn path(&self) -> &'static str {
        "custom/events/CaptureTheFlag"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_talk_id(&[MANAGER, BLUE_TEAM_HEADQUARTERS, RED_TEAM_HEADQUARTERS]);
        s.add_first_talk_id(&[MANAGER, BLUE_TEAM_HEADQUARTERS, RED_TEAM_HEADQUARTERS]);
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
        schedule::load_config(ctx, "CtF", "CtF");
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
        // `ThreadPool.schedule(() -> sendScreenMessage(activeChar, text, seconds), delay)` of
        // `onEnterZone`: they run whatever the state of the event is.
        if let Some(rest) = event.strip_prefix("ScreenMessage:") {
            if let (Some(player), Some((seconds, text))) = (player, rest.split_once(':'))
                && let Ok(seconds) = seconds.parse::<i32>()
            {
                arena::send_screen_message(ctx, player, text, seconds);
            }
            return None;
        }

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
                    let state = ctx.state::<CtfState>();
                    state.player_list.push(player);
                    arena::put_score(&mut state.player_scores, player, 0);
                    ctx.world_mut().set_registered_on_event(player, true);
                    ctx.state::<CtfState>().logout_listeners.insert(player);
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

                let state = ctx.state::<CtfState>();
                state.player_list.retain(|p| *p != player);
                arena::remove_score(&mut state.player_scores, player);
                remove_listeners(ctx, player);
                ctx.world_mut().set_registered_on_event(player, false);
                htmltext = Some("registration-canceled.html".to_owned());
            }
            "BuffHeal" => {
                return arena::buff_heal(ctx, npc?, player?);
            }
            "TeleportToArena" => {
                // Set state to STARTING.
                set_state(ctx, EventState::Starting);

                // Remove offline players.
                for participant in ctx.state::<CtfState>().player_list.clone() {
                    if ctx.world().player(participant).is_none() {
                        let state = ctx.state::<CtfState>();
                        state.player_list.retain(|p| *p != participant);
                        arena::remove_score(&mut state.player_scores, participant);
                    }
                }

                // Check if there are enough players to start the event.
                if ctx.state::<CtfState>().player_list.len() < MINIMUM_PARTICIPANT_COUNT {
                    ctx.broadcast_to_all_online_players("CtF Event: Event was canceled, not enough participants.");
                    for participant in ctx.state::<CtfState>().player_list.clone() {
                        remove_listeners(ctx, participant);
                        ctx.world_mut().set_registered_on_event(participant, false);
                    }

                    // Set state INACTIVE
                    set_state(ctx, EventState::Inactive);
                    return None;
                }

                // Create the instance.
                let world = ctx.world_mut().create_dynamic_instance(INSTANCE_ID);
                ctx.state::<CtfState>().pvp_world = Some(world);

                // Make sure doors are closed.
                ctx.world_mut().instance_close_door(world, BLUE_DOOR_ID);
                ctx.world_mut().instance_close_door(world, RED_DOOR_ID);

                // Randomize player list and separate teams.
                let mut list = ctx.state::<CtfState>().player_list.clone();
                arena::shuffle(ctx, &mut list);
                ctx.state::<CtfState>().player_list = list.clone();
                let mut team = ctx.get_random_boolean(); // If teams are not even, randomize where extra player goes.
                for participant in list {
                    ctx.world_mut().set_on_event(participant, true);
                    ctx.world_mut().set_registered_on_event(participant, false);
                    if team {
                        ctx.state::<CtfState>().blue_team.push(participant);
                        ctx.world_mut().player_leave_party(participant);
                        teleport_to_zone(ctx, participant, BLUE_SPAWN_ZONE, world);
                        ctx.world_mut().set_player_team(participant, TEAM_BLUE);
                        team = false;
                    } else {
                        ctx.state::<CtfState>().red_team.push(participant);
                        ctx.world_mut().player_leave_party(participant);
                        teleport_to_zone(ctx, participant, RED_SPAWN_ZONE, world);
                        ctx.world_mut().set_player_team(participant, TEAM_RED);
                        team = true;
                    }

                    ctx.add_listener(participant, EventKind::CreatureDeath, LISTENER_TAG);
                }

                // Make Blue CC.
                let blue = ctx.state::<CtfState>().blue_team.clone();
                ctx.world_mut().event_make_team_parties(&blue, PARTY_MEMBER_COUNT);

                // Make Red CC.
                let red = ctx.state::<CtfState>().red_team.clone();
                ctx.world_mut().event_make_team_parties(&red, PARTY_MEMBER_COUNT);

                // Spawn managers.
                let delay = (WAIT_TIME + FIGHT_TIME) * 60_000;
                let (x, y, z, h) = BLUE_BUFFER_SPAWN_LOC;
                ctx.add_spawn(MANAGER, x, y, z, h, false, delay, world);
                let (x, y, z, h) = RED_BUFFER_SPAWN_LOC;
                ctx.add_spawn(MANAGER, x, y, z, h, false, delay, world);

                // Initialize scores.
                ctx.state::<CtfState>().blue_score = 0;
                ctx.state::<CtfState>().red_score = 0;

                // Initialize scoreboard.
                let scores = ctx.state::<CtfState>().player_scores.clone();
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
                let world = ctx.state::<CtfState>().pvp_world?;
                ctx.world_mut().instance_open_door(world, BLUE_DOOR_ID);
                ctx.world_mut().instance_open_door(world, RED_DOOR_ID);

                // Spawn the flags
                let delay = (WAIT_TIME + FIGHT_TIME) * 60_000;
                let (x, y, z, h) = BLUE_TEAM_HEADQUARTERS_LOC;
                let blue_flag = ctx.add_spawn(BLUE_TEAM_HEADQUARTERS, x, y, z, h, false, delay, world);
                ctx.state::<CtfState>().flag_blue_spawn = blue_flag;
                let (x, y, z, h) = RED_TEAM_HEADQUARTERS_LOC;
                let red_flag = ctx.add_spawn(RED_TEAM_HEADQUARTERS, x, y, z, h, false, delay, world);
                ctx.state::<CtfState>().flag_red_spawn = red_flag;

                // add event FIGHT_TIME
                for participant in ctx.state::<CtfState>().player_list.clone() {
                    arena::send_time_remaining(ctx, participant, (FIGHT_TIME * 60) as i32);
                }

                // Send message.
                arena::broadcast_screen_message(ctx, world, "Capture The Flag Event Started - Go to flags!", 5, true);

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
                let world = ctx.state::<CtfState>().pvp_world?;
                // Close doors.
                ctx.world_mut().instance_close_door(world, BLUE_DOOR_ID);
                ctx.world_mut().instance_close_door(world, RED_DOOR_ID);

                // Reset flag carriers
                reset_carriers_and_flags(ctx);

                // Disable players.
                let list = ctx.state::<CtfState>().player_list.clone();
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
                    let s = ctx.state::<CtfState>();
                    (s.team_forfeit, s.blue_score, s.red_score)
                };
                let blue = ctx.state::<CtfState>().blue_team.clone();
                let red = ctx.state::<CtfState>().red_team.clone();
                if forfeit {
                    // Team wins by Forfeit.
                    let blue_wins = !(blue.is_empty() && !red.is_empty());
                    let winners = if blue_wins { &blue } else { &red };
                    arena::broadcast_screen_message(
                        ctx,
                        world,
                        &format!("Team {} won the event by forfeit!", if blue_wins { "Blue" } else { "Red" }),
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
                let world = ctx.state::<CtfState>().pvp_world?;
                let scores = ctx.state::<CtfState>().player_scores.clone();
                arena::broadcast_scoreboard(ctx, world, SCOREBOARD_FINISH, &scores);

                // remove event FIGHT_TIME
                for participant in ctx.state::<CtfState>().player_list.clone() {
                    arena::send_time_remaining(ctx, participant, 0);
                }
            }
            "TeleportOut" => {
                ctx.state::<CtfState>().team_forfeit = false;

                // Remove event listeners.
                let list = ctx.state::<CtfState>().player_list.clone();
                for participant in &list {
                    remove_listeners(ctx, *participant);
                    ctx.world_mut().set_player_team(*participant, TEAM_NONE);
                    ctx.world_mut().set_on_event(*participant, false);
                    ctx.world_mut().player_leave_party(*participant);
                }

                // Destroy world.
                if let Some(world) = ctx.state::<CtfState>().pvp_world.take() {
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
                    let world = ctx.state::<CtfState>().pvp_world?;
                    let (in_blue, in_red) = {
                        let s = ctx.state::<CtfState>();
                        (s.in_blue(player), s.in_red(player))
                    };
                    if in_blue {
                        ctx.world_mut().player_set_pending_revive(player, true);
                        teleport_to_zone(ctx, player, BLUE_SPAWN_ZONE, world);

                        // Make player invulnerable for 30 seconds.
                        ctx.world_mut().apply_effects(player, player, GHOST_WALKING);

                        // Reset existing activity timers.
                        reset_activity_timers(ctx, player); // In case player died in peace zone.
                    } else if in_red {
                        ctx.world_mut().player_set_pending_revive(player, true);
                        teleport_to_zone(ctx, player, RED_SPAWN_ZONE, world);

                        // Make player invulnerable for 30 seconds.
                        ctx.world_mut().apply_effects(player, player, GHOST_WALKING);

                        // Reset existing activity timers.
                        reset_activity_timers(ctx, player); // In case player died in peace zone.
                    }
                }
            }
            "10" | "9" | "8" | "7" | "6" | "5" | "4" | "3" | "2" | "1" => {
                let world = ctx.state::<CtfState>().pvp_world?;
                arena::broadcast_screen_message(ctx, world, event, 4, false);
            }
            "manager-cancel" => {
                let count = ctx.state::<CtfState>().player_list.len();
                arena::show_manager_page(ctx, npc?, player?, "manager-cancel.html", count);
            }
            "manager-register" => {
                let count = ctx.state::<CtfState>().player_list.len();
                arena::show_manager_page(ctx, npc?, player?, "manager-register.html", count);
            }
            _ => {}
        }

        // Activity timer.
        if event.starts_with("KickPlayer")
            && let Some(player) = player
        {
            // NOTE(java-quirk): without a world Java throws a NullPointerException here.
            let world = ctx.state::<CtfState>().pvp_world?;
            if ctx.world().instance_id_of(player) == world {
                if event.contains("Warning") {
                    arena::send_screen_message(ctx, player, "You have been marked as inactive!", 10);
                } else {
                    ctx.world_mut().set_player_team(player, TEAM_NONE);
                    ctx.world_mut().instance_eject_player(world, player);
                    {
                        let s = ctx.state::<CtfState>();
                        s.player_list.retain(|p| *p != player);
                        arena::remove_score(&mut s.player_scores, player);
                        s.blue_team.retain(|p| *p != player);
                        s.red_team.retain(|p| *p != player);
                    }
                    ctx.world_mut().set_on_event(player, false);
                    remove_listeners(ctx, player);
                    ctx.world().send_message(player, "You have been kicked for been inactive.");
                    // Manage forfeit.
                    let one_side_left = one_side_left(ctx);
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
                    let scores = ctx.state::<CtfState>().player_scores.clone();
                    ctx.world()
                        .send(player, &arena::scoreboard_packet(ctx, SCOREBOARD_FINISH, &scores));
                }
            }
        }

        if let Some(minutes) = event.strip_prefix("RegistrationWarn:") {
            if let Ok(minutes_left) = minutes.split(':').next().unwrap_or("").parse::<i32>() {
                ctx.broadcast_to_all_online_players(&format!(
                    "CtF Event: Registration opened for {minutes_left} minutes."
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
        if ctx.state::<CtfState>().player_list.contains(&player) {
            let in_instance = ctx.world().instance_id_of(npc.object_id) > 0;
            // Npc is in instance.
            if in_instance && npc.id == MANAGER {
                return Some("manager-buffheal.html".to_owned());
            }

            // NOTE(java-quirk): the precedence of Java's `&&` and `||` makes any red
            // headquarters count, in an instance or not.
            if (in_instance && npc.id == BLUE_TEAM_HEADQUARTERS) || npc.id == RED_TEAM_HEADQUARTERS {
                flag_talk(ctx, npc, player);
                return None;
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
        let Some(world) = ctx.state::<CtfState>().pvp_world else {
            return;
        };

        // Kick enemy players.
        if blue_zone && team == TEAM_RED {
            teleport_to_zone(ctx, creature, RED_SPAWN_ZONE, world);
            arena::send_screen_message(ctx, player, "Entering the enemy headquarters is prohibited!", 7);
        }

        if red_zone && team == TEAM_BLUE {
            teleport_to_zone(ctx, creature, BLUE_SPAWN_ZONE, world);
            arena::send_screen_message(ctx, player, "Entering the enemy headquarters is prohibited!", 7);
        }

        // Start inactivity check.
        if ctx.world().player(creature).is_some()
            && ((blue_zone && team == TEAM_BLUE) || (red_zone && team == TEAM_RED))
        {
            reset_activity_timers(ctx, player);
        }

        if player_is_carrier(ctx, player) && ((blue_zone && team == TEAM_RED) || (red_zone && team == TEAM_BLUE)) {
            remove_flag_carrier(ctx, player);
            enemy_team_flag(ctx, player);
            for active_char in participant_enemy_team(ctx, player) {
                let name = ctx.player_name(active_char);
                // `ThreadPool.schedule(() -> sendScreenMessage(..), 1000)` and 4000: the text
                // goes through a timer of the event.
                ctx.start_quest_timer(
                    &format!("ScreenMessage:4:{name} The enemy has entered your zone with the flag"),
                    1000,
                    None,
                    Some(active_char),
                    false,
                );
                ctx.start_quest_timer(
                    &format!("ScreenMessage:3:{name} Your flag returned to your base!"),
                    4000,
                    None,
                    Some(active_char),
                    false,
                );
            }
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
                ctx.world_mut().stop_skill_effects_removed(player, GHOST_WALKING.id);
            }
        }
    }

    /// Java `onPlayerLogout` (the `ON_PLAYER_LOGOUT` listener of a registered player).
    fn on_player_logout(&self, ctx: &mut ScriptCtx<'_>, player: ObjectId) {
        if !ctx.state::<CtfState>().logout_listeners.contains(&player) {
            return;
        }

        // Remove player from lists.
        {
            let s = ctx.state::<CtfState>();
            s.player_list.retain(|p| *p != player);
            arena::remove_score(&mut s.player_scores, player);
            s.blue_team.retain(|p| *p != player);
            s.red_team.retain(|p| *p != player);
        }

        if player_is_carrier(ctx, player) {
            remove_flag_carrier(ctx, player);
            enemy_team_flag(ctx, player);
            let name = ctx.player_name(player);
            if let Some(world) = ctx.state::<CtfState>().pvp_world {
                arena::broadcast_screen_message(
                    ctx,
                    world,
                    &format!("{name} has logout, Flag returned to base"),
                    7,
                    false,
                );
            }
        }

        if is_started(ctx) {
            arena::send_time_remaining(ctx, player, 0);
        }

        // Manage forfeit.
        if one_side_left(ctx) {
            if let Some(world) = ctx.state::<CtfState>().pvp_world {
                manage_forfeit(ctx, world);
            } else {
                ctx.state::<CtfState>().team_forfeit = true;
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
        let Some(world) = ctx.state::<CtfState>().pvp_world else {
            return;
        };

        // Confirm Blue team kill.
        if killer_team == TEAM_BLUE && killed_team == TEAM_RED {
            let Some(score) = arena::get_score(&ctx.state::<CtfState>().player_scores, killer) else {
                return;
            };
            arena::put_score(&mut ctx.state::<CtfState>().player_scores, killer, score + 1);
            let scores = ctx.state::<CtfState>().player_scores.clone();
            arena::broadcast_scoreboard(ctx, world, SCOREBOARD_UPDATE, &scores);
        }

        // Confirm Red team kill.
        if killer_team == TEAM_RED && killed_team == TEAM_BLUE {
            let Some(score) = arena::get_score(&ctx.state::<CtfState>().player_scores, killer) else {
                return;
            };
            arena::put_score(&mut ctx.state::<CtfState>().player_scores, killer, score + 1);
            let scores = ctx.state::<CtfState>().player_scores.clone();
            arena::broadcast_scoreboard(ctx, world, SCOREBOARD_UPDATE, &scores);
            // NOTE(java-quirk): only the red kills are announced to the team.
            let killer_name = ctx.player_name(killer);
            let killed_name = ctx.player_name(killed_player);
            for active_char in participant_team(ctx, killer) {
                ctx.world().send_creature_say_text(
                    active_char,
                    Some(killer),
                    ChatType::Whisper,
                    &killer_name,
                    &format!("I have killed {killed_name}!"),
                );
            }
        }

        if player_is_carrier(ctx, killed_player) {
            remove_flag_carrier(ctx, killed_player);
            enemy_team_flag(ctx, killed_player);
            let killer_name = ctx.player_name(killer);
            let killed_name = ctx.player_name(killed_player);
            arena::broadcast_screen_message(
                ctx,
                world,
                &format!("{killer_name} has killed {killed_name} Flag returned to base!"),
                7,
                false,
            );
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
            let s = ctx.state::<CtfState>();
            s.player_list.clear();
            s.player_scores.clear();
            s.blue_team.clear();
            s.red_team.clear();
        }

        // Spawn event manager.
        let (x, y, z, h) = MANAGER_SPAWN_LOC;
        let manager = ctx.add_spawn(MANAGER, x, y, z, h, false, REGISTRATION_TIME * 60_000, 0);
        ctx.state::<CtfState>().manager_npc = manager;
        ctx.start_quest_timer("TeleportToArena", REGISTRATION_TIME * 60_000, None, None, false);

        // Send message to players.
        ctx.broadcast_to_all_online_players(&format!("CtF Event: Registration opened for {REGISTRATION_TIME} minutes."));
        ctx.broadcast_to_all_online_players("CtF Event: You can register at Giran CtF Event Manager.");

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
        // NOTE(java-quirk): `MANAGER_NPC_INSTANCE.deleteMe()` throws a NullPointerException when the
        // event never started; the rest does not run.
        let Some(manager) = ctx.state::<CtfState>().manager_npc else {
            return false;
        };
        ctx.delete_npc(manager);

        // Cancel timers.
        ctx.cancel_all_quest_timers();

        // Reset flag carriers, remove the headquarters.
        reset_carriers_and_flags(ctx);

        // Remove participants.
        let list = ctx.state::<CtfState>().player_list.clone();
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

        if started && let Some(world) = ctx.state::<CtfState>().pvp_world {
            let scores = ctx.state::<CtfState>().player_scores.clone();
            arena::broadcast_scoreboard(ctx, world, SCOREBOARD_FINISH, &scores);
        }

        if let Some(world) = ctx.state::<CtfState>().pvp_world.take() {
            ctx.world_mut().destroy_instance_world(world);
        }

        // Send message to players.
        ctx.broadcast_to_all_online_players("CtF Event: Event was canceled.");

        // Set state PARTICIPATING
        set_state(ctx, EventState::Inactive);
        true
    }
}

fn set_state(ctx: &mut ScriptCtx<'_>, state: EventState) {
    ctx.state::<CtfState>().state = state;
}

fn is_participating(ctx: &mut ScriptCtx<'_>) -> bool {
    ctx.state::<CtfState>().state == EventState::Participating
}

fn is_starting(ctx: &mut ScriptCtx<'_>) -> bool {
    ctx.state::<CtfState>().state == EventState::Starting
}

fn is_started(ctx: &mut ScriptCtx<'_>) -> bool {
    ctx.state::<CtfState>().state == EventState::Started
}

/// Java `canRegister(player)`.
fn can_register(ctx: &mut ScriptCtx<'_>, player: ObjectId) -> bool {
    let (registered, count) = {
        let s = ctx.state::<CtfState>();
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

/// `participant.teleToLocation(ZONE.getRandomPoint(), PVP_WORLD.getInstanceId(), 50)`.
fn teleport_to_zone(ctx: &mut ScriptCtx<'_>, player: ObjectId, zone: &str, world: i32) {
    if let Some(point) = ctx.world_mut().zone_random_point(zone) {
        ctx.world_mut().event_teleport(player, point, world, 50);
    }
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

/// `(BLUE_TEAM.isEmpty() && !RED_TEAM.isEmpty()) || (RED_TEAM.isEmpty() && !BLUE_TEAM.isEmpty())`.
fn one_side_left(ctx: &mut ScriptCtx<'_>) -> bool {
    let s = ctx.state::<CtfState>();
    (s.blue_team.is_empty() && !s.red_team.is_empty()) || (s.red_team.is_empty() && !s.blue_team.is_empty())
}

fn remove_listeners(ctx: &mut ScriptCtx<'_>, player: ObjectId) {
    ctx.state::<CtfState>().logout_listeners.remove(&player);
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
    ctx.state::<CtfState>().team_forfeit = true;
    for second in (1..=10).rev() {
        ctx.cancel_quest_timer(&second.to_string(), None, None);
    }
    ctx.cancel_quest_timer("EndFight", None, None);
    ctx.start_quest_timer("EndFight", 10_000, None, None, false);
    arena::broadcast_screen_message(ctx, world, "Enemy team forfeit!", 7, true);
}

fn broadcast_score_message(ctx: &mut ScriptCtx<'_>, world: i32) {
    let (blue, red) = {
        let s = ctx.state::<CtfState>();
        (s.blue_score, s.red_score)
    };
    ctx.world().instance_broadcast_packet(
        world,
        &ExShowScreenMessage::styled_text(&format!("Blue: {blue} - Red: {red}"), BOTTOM_RIGHT, 15_000, 0, true, false),
    );
}

/// Java `increasePoints(player)`.
fn increase_points(ctx: &mut ScriptCtx<'_>, player: ObjectId) {
    let (in_blue, in_red) = {
        let s = ctx.state::<CtfState>();
        (s.in_blue(player), s.in_red(player))
    };
    if in_blue {
        ctx.state::<CtfState>().blue_score += 1;
    } else if in_red {
        ctx.state::<CtfState>().red_score += 1;
    } else {
        return;
    }
    if let Some(world) = ctx.state::<CtfState>().pvp_world {
        broadcast_score_message(ctx, world);
    }
}

/// Java `getParticipantTeamName(player)`.
fn participant_team_name(ctx: &mut ScriptCtx<'_>, player: ObjectId) -> &'static str {
    let s = ctx.state::<CtfState>();
    if s.in_blue(player) {
        BLUE_TEAM_NAME
    } else if s.in_red(player) {
        RED_TEAM_NAME
    } else {
        ""
    }
}

/// Java `getParticipantEnemyTeamName(player)`.
fn participant_enemy_team_name(ctx: &mut ScriptCtx<'_>, player: ObjectId) -> &'static str {
    let s = ctx.state::<CtfState>();
    if s.in_blue(player) {
        RED_TEAM_NAME
    } else if s.in_red(player) {
        BLUE_TEAM_NAME
    } else {
        ""
    }
}

/// Java `getParticipantTeam(player)` (by the player's `Team`).
fn participant_team(ctx: &mut ScriptCtx<'_>, player: ObjectId) -> Vec<ObjectId> {
    let blue = ctx.world().team_of(player) == TEAM_BLUE;
    let s = ctx.state::<CtfState>();
    if blue { s.blue_team.clone() } else { s.red_team.clone() }
}

/// Java `getParticipantEnemyTeam(player)`.
fn participant_enemy_team(ctx: &mut ScriptCtx<'_>, player: ObjectId) -> Vec<ObjectId> {
    let blue = ctx.world().team_of(player) == TEAM_BLUE;
    let s = ctx.state::<CtfState>();
    if blue { s.red_team.clone() } else { s.blue_team.clone() }
}

/// Java `getEnemyTeamFlagId(player)`.
fn enemy_team_flag_id(ctx: &mut ScriptCtx<'_>, player: ObjectId) -> i32 {
    if ctx.state::<CtfState>().in_blue(player) {
        RED_TEAM_FLAG_ITEM_ID
    } else {
        BLUE_TEAM_FLAG_ITEM_ID
    }
}

/// Java `playerIsCarrier(player)`.
fn player_is_carrier(ctx: &mut ScriptCtx<'_>, player: ObjectId) -> bool {
    let s = ctx.state::<CtfState>();
    s.blue_team_carrier == Some(player) || s.red_team_carrier == Some(player)
}

/// A carrier who disconnected or left the instance (the check at the top of `getTeamCarrier` and
/// `getEnemyCarrier`).
fn carrier_is_gone(ctx: &mut ScriptCtx<'_>, carrier: Option<ObjectId>) -> bool {
    let Some(carrier) = carrier else {
        return false;
    };
    // NOTE(java-quirk): `PVP_WORLD.getInstanceId()` throws without a world.
    let Some(world) = ctx.state::<CtfState>().pvp_world else {
        return false;
    };
    ctx.world().player(carrier).is_none() || ctx.world().instance_id_of(carrier) != world
}

/// Java `getTeamCarrier(player)`.
fn team_carrier(ctx: &mut ScriptCtx<'_>, player: ObjectId) -> Option<ObjectId> {
    let (in_blue, in_red, blue_carrier, red_carrier) = {
        let s = ctx.state::<CtfState>();
        (s.in_blue(player), s.in_red(player), s.blue_team_carrier, s.red_team_carrier)
    };
    // Check if team carrier has disconnected.
    if (in_blue && carrier_is_gone(ctx, blue_carrier)) || (in_red && carrier_is_gone(ctx, red_carrier)) {
        let flag = enemy_team_flag_id(ctx, player);
        ctx.world_mut().destroy_item_by_item_id(player, flag, 1, false);
        return None;
    }

    // Return team carrier.
    if in_blue { blue_carrier } else { red_carrier }
}

/// Java `getEnemyCarrier(player)`.
fn enemy_carrier(ctx: &mut ScriptCtx<'_>, player: ObjectId) -> Option<ObjectId> {
    let (in_blue, in_red, blue_carrier, red_carrier) = {
        let s = ctx.state::<CtfState>();
        (s.in_blue(player), s.in_red(player), s.blue_team_carrier, s.red_team_carrier)
    };
    // Check if enemy carrier has disconnected.
    if (in_blue && carrier_is_gone(ctx, red_carrier)) || (in_red && carrier_is_gone(ctx, blue_carrier)) {
        let flag = enemy_team_flag_id(ctx, player);
        ctx.world_mut().destroy_item_by_item_id(player, flag, 1, false);
        return None;
    }

    // Return enemy carrier.
    if in_blue { red_carrier } else { blue_carrier }
}

/// Java `setCarrierUnequippedWeapons(player, itemRight, itemLeft)`.
fn set_carrier_unequipped_weapons(ctx: &mut ScriptCtx<'_>, player: ObjectId, right: Worn, left: Worn) {
    let s = ctx.state::<CtfState>();
    if s.in_blue(player) {
        s.blue_carrier_r_hand = right;
        s.blue_carrier_l_hand = left;
    } else {
        s.red_carrier_r_hand = right;
        s.red_carrier_l_hand = left;
    }
}

/// Java `removeFlagCarrier(player)`.
fn remove_flag_carrier(ctx: &mut ScriptCtx<'_>, player: ObjectId) {
    // Un-equip - destroy flag.
    ctx.world_mut().event_unequip_slot(player, PAPERDOLL_RHAND);
    let flag = enemy_team_flag_id(ctx, player);
    ctx.world_mut().destroy_item_by_item_id(player, flag, 1, false);

    // Unblock inventory.
    ctx.world_mut().unblock_inventory(player);

    // Re-equip player items.
    let (right, left) = {
        let s = ctx.state::<CtfState>();
        if s.in_blue(player) {
            (s.blue_carrier_r_hand, s.blue_carrier_l_hand)
        } else {
            (s.red_carrier_r_hand, s.red_carrier_l_hand)
        }
    };
    if let Some((object_id, item_id)) = right
        && ctx.has_quest_items(player, item_id)
    {
        ctx.world_mut().event_equip_object(player, object_id);
    }
    if let Some((object_id, item_id)) = left
        && ctx.has_quest_items(player, item_id)
    {
        ctx.world_mut().event_equip_object(player, object_id);
    }

    set_carrier_unequipped_weapons(ctx, player, None, None);

    // Flag carrier removal.
    // NOTE(java-quirk): a player who is in no team list (a logout removes it first) counts as
    // red here.
    let s = ctx.state::<CtfState>();
    if s.in_blue(player) {
        s.blue_team_carrier = None;
    } else {
        s.red_team_carrier = None;
    }

    // Show re-equipped weapons.
    ctx.world_mut().broadcast_user_info(player);
}

/// Java `setTeamCarrier(player)`.
fn set_team_carrier(ctx: &mut ScriptCtx<'_>, player: ObjectId) {
    let (in_blue, in_red, blue, red) = {
        let s = ctx.state::<CtfState>();
        (s.in_blue(player), s.in_red(player), s.blue_team_carrier, s.red_team_carrier)
    };
    if in_blue {
        if blue.is_none() {
            ctx.state::<CtfState>().blue_team_carrier = Some(player);
        } else {
            ctx.world().send_message(player, "The Blue flag has already been taken");
        }
    }

    if in_red {
        if red.is_none() {
            ctx.state::<CtfState>().red_team_carrier = Some(player);
        } else {
            ctx.world().send_message(player, "The Red flag has already been taken");
        }
    }
}

/// Java `enemyTeamFlag(player)`: the flag of the enemy comes back to its headquarters.
fn enemy_team_flag(ctx: &mut ScriptCtx<'_>, player: ObjectId) {
    let Some(world) = ctx.state::<CtfState>().pvp_world else {
        return;
    };
    let delay = (WAIT_TIME + FIGHT_TIME) * 60_000;
    let (in_blue, in_red) = {
        let s = ctx.state::<CtfState>();
        (s.in_blue(player), s.in_red(player))
    };
    if in_blue {
        let (x, y, z, h) = RED_TEAM_HEADQUARTERS_LOC;
        let flag = ctx.add_spawn(RED_TEAM_HEADQUARTERS, x, y, z, h, false, delay, world);
        ctx.state::<CtfState>().flag_red_spawn = flag;
    }

    if in_red {
        let (x, y, z, h) = BLUE_TEAM_HEADQUARTERS_LOC;
        let flag = ctx.add_spawn(BLUE_TEAM_HEADQUARTERS, x, y, z, h, false, delay, world);
        ctx.state::<CtfState>().flag_blue_spawn = flag;
    }
}

/// Java `deleteTeamFlag(player)`.
fn delete_team_flag(ctx: &mut ScriptCtx<'_>, player: ObjectId) {
    let (in_blue, in_red) = {
        let s = ctx.state::<CtfState>();
        (s.in_blue(player), s.in_red(player))
    };
    if in_blue && let Some(flag) = ctx.state::<CtfState>().flag_red_spawn.take() {
        ctx.delete_npc(flag);
    }

    if in_red && let Some(flag) = ctx.state::<CtfState>().flag_blue_spawn.take() {
        ctx.delete_npc(flag);
    }
}

/// The carriers lose the flag and the headquarters go (`EndFight` and `eventStop`).
fn reset_carriers_and_flags(ctx: &mut ScriptCtx<'_>) {
    // Reset flag carriers
    if let Some(carrier) = ctx.state::<CtfState>().blue_team_carrier {
        remove_flag_carrier(ctx, carrier);
    }

    if let Some(carrier) = ctx.state::<CtfState>().red_team_carrier {
        remove_flag_carrier(ctx, carrier);
    }

    // Remove Headquarters team Blue.
    if let Some(flag) = ctx.state::<CtfState>().flag_blue_spawn.take() {
        ctx.delete_npc(flag);
    }

    // Remove Headquarters team Red.
    if let Some(flag) = ctx.state::<CtfState>().flag_red_spawn.take() {
        ctx.delete_npc(flag);
    }
}

/// The part of `onFirstTalk` for a registered player at a flag.
fn flag_talk(ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) {
    let flag = ctx.world().npc_template_name(npc.object_id);
    let team = participant_team_name(ctx, player);
    let enemyteam = participant_enemy_team_name(ctx, player);
    let distance = ctx.world().event_distance_2d(player, npc.object_id) as i32;
    let npc_name = flag.clone();
    let player_name = ctx.player_name(player);
    let Some(world) = ctx.state::<CtfState>().pvp_world else {
        return;
    };
    if flag == team {
        if team_carrier(ctx, player) == Some(player) {
            if distance > 70 {
                ctx.world().send_message(player, &format!("{npc_name} : Your request cannot be completed."));
                ctx.world().send_message(player, &format!("{npc_name} : you are not at the correct distance."));
            } else {
                // Player has returned with enemy flag.
                ctx.broadcast_magic_skill_use(player, player, npc.object_id, 1034, 1, 1, 1);
                remove_flag_carrier(ctx, player);
                enemy_team_flag(ctx, player);
                ctx.world().instance_broadcast_packet(
                    world,
                    &ExShowScreenMessage::text(&format!("{player_name} has captured the {enemyteam} flag!"), 5 * 1000),
                );
                increase_points(ctx, player);
            }
        } else if enemy_carrier(ctx, player).is_none() {
            // Team flag is missing.
            send_flag_page(ctx, npc, player, "flag_friendly.html", enemyteam, Some(team), &player_name);
        } else {
            // Go get the flag.
            send_flag_page(ctx, npc, player, "flag_friendly_missing.html", enemyteam, Some(team), &player_name);
        }
    } else if let Some(carrier) = team_carrier(ctx, player) {
        // enemy flag enemy is missing
        let carrier_name = ctx.player_name(carrier);
        send_flag_page(ctx, npc, player, "flag_enemy_missing.html", enemyteam, None, &carrier_name);
    } else if distance > 70 {
        ctx.world().send_message(player, &format!("{npc_name} : Your request cannot be completed."));
        ctx.world().send_message(player, &format!("{npc_name} : you are not at the correct distance."));
    } else {
        // player has flag
        send_flag_page(ctx, npc, player, "flag_enemy.html", enemyteam, Some(team), &player_name);

        // take flag
        let right = ctx.world().event_paperdoll_item(player, PAPERDOLL_RHAND);
        let left = ctx.world().event_paperdoll_item(player, PAPERDOLL_LHAND);
        set_carrier_unequipped_weapons(ctx, player, right, left);
        let flag_id = enemy_team_flag_id(ctx, player);
        ctx.world_mut().event_equip_new_item(player, flag_id);
        ctx.world_mut().event_block_all_items(player);
        ctx.world_mut().broadcast_user_info(player);
        set_team_carrier(ctx, player);
        delete_team_flag(ctx, player);
        ctx.world().instance_broadcast_packet(
            world,
            &ExShowScreenMessage::text(&format!("{player_name} has taken the {enemyteam} flag!"), 7000),
        );
    }
}

/// The flag pages: `%enemyteam%`, `%team%` and `%player%` replaced.
fn send_flag_page(
    ctx: &mut ScriptCtx<'_>,
    npc: ScriptNpc,
    player: ObjectId,
    file: &str,
    enemyteam: &str,
    team: Option<&str>,
    player_name: &str,
) {
    let mut html = ctx.get_htm(player, file).unwrap_or_default().replace("%enemyteam%", enemyteam);
    // NOTE(java-quirk): the page of the missing enemy flag is sent without a `%team%` replaced.
    if let Some(team) = team {
        html = html.replace("%team%", team);
    }
    let html = html.replace("%player%", player_name);
    if let Some(message) = NpcHtmlMessage::new(npc.object_id.0, &html) {
        ctx.world_mut().send_html(player, &message);
    }
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::{ScriptTest, item, pages_to};

    const NPCS: &str = r#"<npc id="70012" type="Folk" level="1" name="Manager"/>
        <npc id="82016" type="Folk" level="1" name="Blue"/>
        <npc id="82017" type="Folk" level="1" name="Red"/>"#;
    const ITEMS: &str = r#"<item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="13531" type="Weapon" name="Flag of Dion"><set name="bodypart" val="rhand"/><set name="weapon_type" val="SWORD"/></item>
        <item id="13534" type="Weapon" name="Flag of Aden"><set name="bodypart" val="rhand"/><set name="weapon_type" val="SWORD"/></item>"#;
    const DIR: &str = "data/scripts/custom/events/CaptureTheFlag";

    fn scene(players: i32) -> (ScriptTest, ObjectId) {
        let mut registry = ScriptRegistry::new();
        registry.add(CtF);
        let mut pages: Vec<(String, String)> = [
            (
                "manager-register.html",
                r#"<html><body>register %player_numbers% <a action="bypass -h Script CtF Participate">in</a></body></html>"#,
            ),
            ("manager-cancel.html", "<html><body>cancel %player_numbers%</body></html>"),
            ("registration-success.html", "<html><body>success</body></html>"),
            ("flag_enemy.html", "<html><body>took %enemyteam% for %team% %player%</body></html>"),
            ("flag_enemy_missing.html", "<html><body>missing %enemyteam% %team% %player%</body></html>"),
            ("flag_friendly.html", "<html><body>friendly %team%</body></html>"),
        ]
        .iter()
        .map(|(n, t)| (format!("{DIR}/{n}"), (*t).to_owned()))
        .collect();
        pages.push((
            "data/zones/ctf.xml".to_owned(),
            String::new(),
        ));
        let pages: Vec<(String, String)> = pages.into_iter().filter(|(p, _)| !p.starts_with("data/zones")).collect();
        let refs: Vec<(&str, &str)> = pages.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
        let mut t = ScriptTest::new_with(registry, NPCS, ITEMS, &refs, |world, root| {
            std::fs::create_dir_all(root.join("data/zones")).unwrap();
            std::fs::write(
                root.join("data/zones/ctf_event_zone.xml"),
                r#"<?xml version="1.0"?><list>
                <zone name="blue_team_spawn" type="ArenaZone" shape="Cuboid" minZ="-3468" maxZ="-3268"><node X="147000" Y="46000" /><node X="147900" Y="46900" /></zone>
                <zone name="red_team_spawn" type="ArenaZone" shape="Cuboid" minZ="-3472" maxZ="-3272"><node X="151000" Y="46000" /><node X="151900" Y="46900" /></zone>
                </list>"#,
            )
            .unwrap();
            world.set_zone_data(
                std::sync::Arc::new(l2_data::Zones::load(root)),
                std::sync::Arc::new(l2_data::MapRegions::load(root)),
            );
        });
        for id in 1..=players {
            t.enter(id, 80, vec![item(1000 + id, 57, 1)]);
        }
        let manager = t.spawn(MANAGER, 20);
        t.take();
        (t, manager)
    }

    fn start(t: &mut ScriptTest) -> bool {
        let mut started = false;
        t.world.run_as_script("CtF", |ctx| {
            started = CtF.event_start(ctx, Some(ObjectId(1)));
        });
        started
    }

    fn register(t: &mut ScriptTest, player: i32, manager: ObjectId) -> String {
        t.talk(player, manager);
        t.take();
        t.bypass(player, "Script CtF Participate");
        pages_to(&t.take(), player).pop().unwrap_or_default()
    }

    fn first_talk(t: &mut ScriptTest, npc: ObjectId, player: ObjectId) -> Option<String> {
        let mut page = None;
        t.world.run_as_script("CtF", |ctx| {
            let n = ctx.script_npc(npc).unwrap();
            page = CtF.on_first_talk(ctx, n, player);
        });
        page
    }

    #[test]
    fn switched_off_nothing_runs() {
        let (mut t, manager) = scene(1);
        t.ticks(10);
        t.talk(1, manager);
        assert!(pages_to(&t.take(), 1).iter().all(|p| !p.contains("register")));
        t.bypass(1, "Script CtF Participate");
        assert!(pages_to(&t.take(), 1).is_empty());
        assert!(t.npcs_of(BLUE_TEAM_HEADQUARTERS).is_empty());
    }

    #[test]
    fn a_flag_is_taken_carried_home_and_scores() {
        let (mut t, manager) = scene(4);
        assert!(start(&mut t));
        for id in 1..=4 {
            assert!(register(&mut t, id, manager).contains("success"), "player {id}");
        }
        t.wait_ms(10 * 60_000 + 2_000);
        let players: Vec<ObjectId> = (1..=4).map(ObjectId).collect();
        let blue: Vec<ObjectId> = players.iter().copied().filter(|p| t.world.team_of(*p) == TEAM_BLUE).collect();
        let red: Vec<ObjectId> = players.iter().copied().filter(|p| t.world.team_of(*p) == TEAM_RED).collect();
        assert_eq!((blue.len(), red.len()), (2, 2));
        let instance = t.world.instance_id_of(blue[0]);
        assert!(instance > 0);
        // Nothing stands at the headquarters before the fight.
        assert!(t.npcs_of(BLUE_TEAM_HEADQUARTERS).is_empty());

        t.wait_ms(60_000);
        let blue_hq = t.npcs_of(BLUE_TEAM_HEADQUARTERS)[0];
        let red_hq = t.npcs_of(RED_TEAM_HEADQUARTERS)[0];
        let carrier = blue[0];
        // The blue player goes to the red flag and takes it.
        t.world.event_teleport(carrier, (150_452 + 10, 46_714, -3414), instance, 0);
        assert_eq!(first_talk(&mut t, red_hq, carrier), None);
        assert_eq!(t.world.inventory_item_count(carrier, RED_TEAM_FLAG_ITEM_ID), 1);
        assert_eq!(t.world.paperdoll_object(carrier, PAPERDOLL_RHAND).is_some(), true);
        assert!(t.world.can_manipulate_with_item_id(carrier, 57) == false, "the inventory is blocked");
        // The red headquarters flag is gone from the world until it comes back.
        assert!(!t.in_world(red_hq));

        // A second flag cannot be taken; the other blue player sees the "missing" page.
        t.world.event_teleport(blue[1], (148_487 + 10, 46_703, -3414), instance, 0);
        assert!(first_talk(&mut t, blue_hq, blue[1]).is_none());

        // The carrier goes home and captures: the score is 1 for blue.
        t.world.event_teleport(carrier, (148_487 + 10, 46_703, -3414), instance, 0);
        assert_eq!(first_talk(&mut t, blue_hq, carrier), None);
        assert_eq!(t.world.inventory_item_count(carrier, RED_TEAM_FLAG_ITEM_ID), 0);
        assert!(t.world.can_manipulate_with_item_id(carrier, 57), "the inventory is free again");
        // The enemy flag is back at its headquarters.
        assert_eq!(t.npcs_of(RED_TEAM_HEADQUARTERS).len(), 1);

        // At the end blue wins 1:0 and is paid.
        t.wait_ms(20 * 60_000);
        t.wait_ms(1_000);
        for p in &blue {
            assert_eq!(t.count(p.0, 57), 100_001, "winner {p:?}");
        }
        for p in &red {
            assert_eq!(t.count(p.0, 57), 1);
        }
        t.wait_ms(20_000);
        for p in &players {
            assert!(!t.world.is_on_event(*p), "on event {p:?}");
            assert_eq!(t.world.instance_id_of(*p), 0, "instance of {p:?}");
        }
    }

    #[test]
    fn a_dead_carrier_loses_the_flag_and_it_goes_home() {
        let (mut t, manager) = scene(4);
        assert!(start(&mut t));
        for id in 1..=4 {
            register(&mut t, id, manager);
        }
        t.wait_ms(10 * 60_000 + 2_000 + 60_000);
        let players: Vec<ObjectId> = (1..=4).map(ObjectId).collect();
        let blue: Vec<ObjectId> = players.iter().copied().filter(|p| t.world.team_of(*p) == TEAM_BLUE).collect();
        let red: Vec<ObjectId> = players.iter().copied().filter(|p| t.world.team_of(*p) == TEAM_RED).collect();
        let instance = t.world.instance_id_of(blue[0]);
        let red_hq = t.npcs_of(RED_TEAM_HEADQUARTERS)[0];
        t.world.event_teleport(blue[0], (150_452 + 10, 46_714, -3414), instance, 0);
        first_talk(&mut t, red_hq, blue[0]);
        assert_eq!(t.world.inventory_item_count(blue[0], RED_TEAM_FLAG_ITEM_ID), 1);

        // A red player kills the carrier.
        t.world.do_die(blue[0], Some(red[0]));
        t.ticks(2);
        assert_eq!(t.world.inventory_item_count(blue[0], RED_TEAM_FLAG_ITEM_ID), 0);
        assert_eq!(t.npcs_of(RED_TEAM_HEADQUARTERS).len(), 1, "the flag is back at its base");
        assert!(t.world.can_manipulate_with_item_id(blue[0], 57));
    }

    #[test]
    fn a_stop_frees_everybody() {
        let (mut t, manager) = scene(1);
        assert!(start(&mut t));
        assert!(register(&mut t, 1, manager).contains("success"));
        let mut stopped = false;
        t.world.run_as_script("CtF", |ctx| stopped = CtF.event_stop(ctx));
        assert!(stopped);
        assert!(!t.world.is_registered_on_event(ObjectId(1)));
    }
}
