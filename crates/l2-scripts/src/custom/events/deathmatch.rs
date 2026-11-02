//! Deathmatch event: everybody fights everybody in the Coliseum for three minutes, the player
//! with the most kills wins a million adena.
// Java: dist/game/data/scripts/custom/events/Deathmatch/Deathmatch.java
//
// Off by default: the shipped `config.xml` schedules nothing, so the event only runs when a GM
// starts it (`//event_start Deathmatch`).

use std::collections::BTreeSet;

use l2_util::ObjectId;
use l2_world::script::EventKind;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup, TimerParams};

use super::arena::{self, GHOST_WALKING, SCOREBOARD_FINISH, SCOREBOARD_INITIALIZE, SCOREBOARD_UPDATE};
use super::schedule;

// NPC
const MANAGER: i32 = 70_011;

// Others
const INSTANCE_ID: i32 = 3049;
const BLUE_DOOR_ID: i32 = 24_190_002;
const RED_DOOR_ID: i32 = 24_190_003;
const MANAGER_SPAWN_LOC: (i32, i32, i32, i32) = (83_425, 148_585, -3406, 32_938);
const SPAWN_ZONES: [&str; 3] = ["colosseum_battle1", "colosseum_battle2", "colosseum_battle3"];

// Settings
const REGISTRATION_TIME: i64 = 1; // Minutes
const WAIT_TIME: i64 = 20; // Seconds
const FIGHT_TIME: i64 = 3; // Minutes
const INACTIVITY_TIME: i64 = 2; // Minutes
const MINIMUM_PARTICIPANT_LEVEL: i32 = 85;
const MAXIMUM_PARTICIPANT_LEVEL: i32 = 200;
const MINIMUM_PARTICIPANT_COUNT: usize = 4;
const MAXIMUM_PARTICIPANT_COUNT: usize = 24; // Scoreboard has 25 slots
const REWARD: (i32, i64) = (57, 1_000_000); // Adena

/// The tag of the death listeners (`this`).
const LISTENER_TAG: i64 = 0;

/// The static fields of `Deathmatch`.
#[derive(Default)]
struct DeathmatchState {
    player_scores: Vec<(ObjectId, i32)>,
    player_list: Vec<ObjectId>,
    pvp_world: Option<i32>,
    manager_npc: Option<ScriptNpc>,
    event_active: bool,
    /// The players with an `ON_PLAYER_LOGOUT` listener of this script.
    logout_listeners: BTreeSet<ObjectId>,
}

/// Java `custom.events.Deathmatch.Deathmatch`.
pub struct Deathmatch;

impl Script for Deathmatch {
    fn name(&self) -> &'static str {
        "Deathmatch"
    }

    fn path(&self) -> &'static str {
        "custom/events/Deathmatch"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_talk_id(&[MANAGER]);
        s.add_first_talk_id(&[MANAGER]);
        s.set_on_player_logout();
        // NOTE(java-quirk): the Java constructor registers no zone: its `onExitZone` is never
        // called.
    }

    fn on_load(&self, ctx: &mut ScriptCtx<'_>) {
        schedule::load_config(ctx, "Deathmatch", "Deathmatch");
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
        if !ctx.state::<DeathmatchState>().event_active {
            return None;
        }

        let mut htmltext = None;
        match event {
            "Participate" => {
                let player = player?;
                if can_register(ctx, player) {
                    // `DUALBOX_CHECK_MAX_L2EVENT_PARTICIPANTS_PER_IP` is 0 (the dualbox check is
                    // not ported): no IP count.
                    let state = ctx.state::<DeathmatchState>();
                    state.player_list.push(player);
                    arena::put_score(&mut state.player_scores, player, 0);
                    ctx.world_mut().set_registered_on_event(player, true);
                    ctx.state::<DeathmatchState>().logout_listeners.insert(player);
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

                let state = ctx.state::<DeathmatchState>();
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
                // Remove offline players.
                for participant in ctx.state::<DeathmatchState>().player_list.clone() {
                    if ctx.world().player(participant).is_none() {
                        let state = ctx.state::<DeathmatchState>();
                        state.player_list.retain(|p| *p != participant);
                        arena::remove_score(&mut state.player_scores, participant);
                    }
                }

                // Check if there are enough players to start the event.
                if ctx.state::<DeathmatchState>().player_list.len() < MINIMUM_PARTICIPANT_COUNT {
                    ctx.broadcast_to_all_online_players(
                        "Deathmatch Event: Event was canceled, not enough participants.",
                    );
                    for participant in ctx.state::<DeathmatchState>().player_list.clone() {
                        remove_listeners(ctx, participant);
                        ctx.world_mut().set_registered_on_event(participant, false);
                    }

                    ctx.state::<DeathmatchState>().event_active = false;
                    return None;
                }

                // Create the instance.
                let world = ctx.world_mut().create_dynamic_instance(INSTANCE_ID);
                ctx.state::<DeathmatchState>().pvp_world = Some(world);

                // Make sure doors are closed.
                ctx.world_mut().instance_close_door(world, BLUE_DOOR_ID);
                ctx.world_mut().instance_close_door(world, RED_DOOR_ID);

                // Randomize player list.
                let mut list = ctx.state::<DeathmatchState>().player_list.clone();
                arena::shuffle(ctx, &mut list);
                ctx.state::<DeathmatchState>().player_list = list.clone();

                for participant in list {
                    ctx.world_mut().set_on_event(participant, true);
                    ctx.world_mut().set_on_solo_event(participant, true);
                    ctx.world_mut().set_registered_on_event(participant, false);
                    ctx.world_mut().player_leave_party(participant);
                    teleport_to_random_spawn(ctx, participant, world);

                    ctx.world_mut().event_set_player_disabled(participant, true);

                    ctx.add_listener(participant, EventKind::CreatureDeath, LISTENER_TAG);
                }

                // Initialize scoreboard.
                let scores = ctx.state::<DeathmatchState>().player_scores.clone();
                arena::broadcast_scoreboard(ctx, world, SCOREBOARD_INITIALIZE, &scores);

                // Schedule start.
                for second in (1..=5).rev() {
                    ctx.start_quest_timer(
                        &second.to_string(),
                        (WAIT_TIME * 1000) - i64::from(second) * 1000,
                        None,
                        None,
                        false,
                    );
                }
                ctx.start_quest_timer("StartFight", WAIT_TIME * 1000, None, None, false);
            }
            "StartFight" => {
                // Send message.
                let world = ctx.state::<DeathmatchState>().pvp_world?;
                arena::broadcast_screen_message(ctx, world, "The fight has began!", 5, true);
                for participant in ctx.state::<DeathmatchState>().player_list.clone() {
                    ctx.world_mut().event_set_player_disabled(participant, false);
                }

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
                let world = ctx.state::<DeathmatchState>().pvp_world?;
                // Close doors.
                ctx.world_mut().instance_close_door(world, BLUE_DOOR_ID);
                ctx.world_mut().instance_close_door(world, RED_DOOR_ID);

                // Disable players.
                let list = ctx.state::<DeathmatchState>().player_list.clone();
                for participant in &list {
                    ctx.world_mut().event_set_player_disabled(*participant, true);
                }

                // Make sure noone is dead.
                for participant in &list {
                    if ctx.is_dead(*participant) {
                        ctx.world_mut().do_revive(*participant);
                    }
                }

                // `Collections.max(PLAYER_SCORES.entrySet(), comparingByValue())`: the first of
                // the highest scores; NOTE(java-quirk): an empty map throws and ends the task,
                // so the scoreboard and the teleport out never follow.
                let scores = ctx.state::<DeathmatchState>().player_scores.clone();
                let mut winner: Option<(ObjectId, i32)> = None;
                for (player, score) in scores {
                    if winner.is_none_or(|(_, best)| score > best) {
                        winner = Some((player, score));
                    }
                }
                let (winner, _) = winner?;
                ctx.give_items(winner, REWARD.0, REWARD.1);

                ctx.start_quest_timer("ScoreBoard", 3500, None, None, false);
                ctx.start_quest_timer("TeleportOut", 7000, None, None, false);
            }
            "ScoreBoard" => {
                let world = ctx.state::<DeathmatchState>().pvp_world?;
                let scores = ctx.state::<DeathmatchState>().player_scores.clone();
                arena::broadcast_scoreboard(ctx, world, SCOREBOARD_FINISH, &scores);
            }
            "TeleportOut" => {
                // Remove event listeners.
                let list = ctx.state::<DeathmatchState>().player_list.clone();
                let world = ctx.state::<DeathmatchState>().pvp_world;
                for participant in &list {
                    remove_listeners(ctx, *participant);
                    ctx.world_mut().set_on_event(*participant, false);
                    ctx.world_mut().set_on_solo_event(*participant, false);
                    ctx.world_mut().player_leave_party(*participant);
                    // `PVP_WORLD.ejectPlayer(participant)`: a NullPointerException without a world.
                    let world = world?;
                    ctx.world_mut().instance_eject_player(world, *participant);
                }

                // Destroy world.
                if let Some(world) = ctx.state::<DeathmatchState>().pvp_world.take() {
                    ctx.world_mut().set_instance_duration(world, 60_000);
                    ctx.world_mut().set_instance_empty_destroy_time(world, 0);
                }

                // Enable players.
                for participant in &list {
                    ctx.world_mut().event_set_player_disabled(*participant, false);
                }

                ctx.state::<DeathmatchState>().event_active = false;
            }
            "ResurrectPlayer" => {
                let player = player?;
                if ctx.is_dead(player) && ctx.world().is_on_event(player) {
                    ctx.world_mut().player_set_pending_revive(player, true);
                    let world = ctx.state::<DeathmatchState>().pvp_world?;
                    teleport_to_random_spawn(ctx, player, world);

                    reset_activity_timers(ctx, player);
                }
            }
            "10" | "9" | "8" | "7" | "6" | "5" | "4" | "3" | "2" | "1" => {
                let world = ctx.state::<DeathmatchState>().pvp_world?;
                arena::broadcast_screen_message(ctx, world, event, 4, false);
            }
            "manager-cancel" => {
                let count = ctx.state::<DeathmatchState>().player_list.len();
                arena::show_manager_page(ctx, npc?, player?, "manager-cancel.html", count);
            }
            "manager-register" => {
                let count = ctx.state::<DeathmatchState>().player_list.len();
                arena::show_manager_page(ctx, npc?, player?, "manager-register.html", count);
            }
            _ => {}
        }

        // Activity timer.
        if event.starts_with("KickPlayer")
            && let Some(player) = player
        {
            // NOTE(java-quirk): without a world Java throws a NullPointerException here.
            let world = ctx.state::<DeathmatchState>().pvp_world?;
            if ctx.world().instance_id_of(player) == world {
                if event.contains("Warning") {
                    arena::send_screen_message(ctx, player, "You have been marked as inactive!", 10);
                } else {
                    ctx.world_mut().instance_eject_player(world, player);
                    {
                        let s = ctx.state::<DeathmatchState>();
                        s.player_list.retain(|p| *p != player);
                        arena::remove_score(&mut s.player_scores, player);
                    }
                    ctx.world_mut().set_on_event(player, false);
                    ctx.world_mut().set_on_solo_event(player, false);
                    remove_listeners(ctx, player);
                    ctx.world().send_message(player, "You have been kicked for been inactive.");
                    let name = ctx.player_name(player);
                    arena::broadcast_screen_message(
                        ctx,
                        world,
                        &format!("Player {name} was kicked for been inactive!"),
                        7,
                        true,
                    );
                }
            }
        }

        if let Some(minutes) = event.strip_prefix("RegistrationWarn:") {
            if let Ok(minutes_left) = minutes.split(':').next().unwrap_or("").parse::<i32>() {
                ctx.broadcast_to_all_online_players(&format!(
                    "Deathmatch Event: Registration opened for {minutes_left} minutes."
                ));
            }
        }

        htmltext
    }

    fn on_first_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        // Event not active.
        if !ctx.state::<DeathmatchState>().event_active {
            return None;
        }

        // Player has already registered.
        if ctx.state::<DeathmatchState>().player_list.contains(&player) {
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

    fn on_exit_zone(&self, ctx: &mut ScriptCtx<'_>, creature: ObjectId, _zone_id: i32) {
        // Not registered for any zone (see `setup`).
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
        if !ctx.state::<DeathmatchState>().logout_listeners.contains(&player) {
            return;
        }

        // Remove player from lists.
        let state = ctx.state::<DeathmatchState>();
        state.player_list.retain(|p| *p != player);
        arena::remove_score(&mut state.player_scores, player);
    }

    /// Java `onPlayerDeath` (the `ON_CREATURE_DEATH` listener of a participant).
    fn on_creature_death(&self, ctx: &mut ScriptCtx<'_>, killer: Option<ObjectId>, victim: ObjectId) {
        if ctx.world().player(victim).is_none() {
            return;
        }
        let killed_player = victim;
        // `event.getAttacker().asPlayer()` is null for a monster, and `PLAYER_SCORES.get(killer)`
        // for a killer who is not in the map: both are a NullPointerException in Java, before the
        // score and the resurrection timer.
        let Some(killer) = killer.and_then(|k| ctx.world().acting_player(k)) else {
            return;
        };
        let Some(score) = arena::get_score(&ctx.state::<DeathmatchState>().player_scores, killer) else {
            return;
        };

        // Confirm player kill.
        arena::put_score(&mut ctx.state::<DeathmatchState>().player_scores, killer, score + 1);
        let Some(world) = ctx.state::<DeathmatchState>().pvp_world else {
            return;
        };
        let scores = ctx.state::<DeathmatchState>().player_scores.clone();
        arena::broadcast_scoreboard(ctx, world, SCOREBOARD_UPDATE, &scores);

        // Auto release after 10 seconds.
        ctx.start_quest_timer("ResurrectPlayer", 10_000, None, Some(killed_player), false);
    }

    fn event_start(&self, ctx: &mut ScriptCtx<'_>, _event_maker: Option<ObjectId>) -> bool {
        if ctx.state::<DeathmatchState>().event_active {
            return false;
        }

        ctx.state::<DeathmatchState>().event_active = true;

        // Cancel timers. (In case event started immediately after another event was canceled.).
        ctx.cancel_all_quest_timers();

        // Clear player lists.
        {
            let s = ctx.state::<DeathmatchState>();
            s.player_list.clear();
            s.player_scores.clear();
        }

        // Spawn event manager. (Its title is the template's: "Deathmatch Event".)
        let (x, y, z, h) = MANAGER_SPAWN_LOC;
        let manager = ctx.add_spawn(MANAGER, x, y, z, h, false, REGISTRATION_TIME * 60_000, 0);
        ctx.state::<DeathmatchState>().manager_npc = manager;
        ctx.start_quest_timer("TeleportToArena", REGISTRATION_TIME * 60_000, None, None, false);

        // Send message to players.
        ctx.broadcast_to_all_online_players(&format!(
            "Deathmatch Event: Registration opened for {REGISTRATION_TIME} minutes."
        ));
        ctx.broadcast_to_all_online_players("Deathmatch Event: You can register at Giran Event Manager.");

        for warn in [10, 5, 4, 3, 2, 1] {
            if REGISTRATION_TIME > warn {
                let delay = (REGISTRATION_TIME - warn) * 60_000;
                ctx.start_quest_timer(&format!("RegistrationWarn:{warn}"), delay, None, None, false);
            }
        }

        true
    }

    fn event_stop(&self, ctx: &mut ScriptCtx<'_>) -> bool {
        if !ctx.state::<DeathmatchState>().event_active {
            return false;
        }

        ctx.state::<DeathmatchState>().event_active = false;

        // Despawn event manager.
        if let Some(manager) = ctx.state::<DeathmatchState>().manager_npc {
            ctx.delete_npc(manager);
        }

        // Cancel timers.
        ctx.cancel_all_quest_timers();

        // Remove participants.
        for participant in ctx.state::<DeathmatchState>().player_list.clone() {
            remove_listeners(ctx, participant);
            ctx.world_mut().set_registered_on_event(participant, false);
            ctx.world_mut().set_on_event(participant, false);
            ctx.world_mut().set_on_solo_event(participant, false);
            ctx.world_mut().event_set_player_disabled(participant, false);
        }

        if let Some(world) = ctx.state::<DeathmatchState>().pvp_world.take() {
            ctx.world_mut().set_instance_duration(world, 60_000);
            ctx.world_mut().set_instance_empty_destroy_time(world, 0);
        }

        // Send message to players.
        ctx.broadcast_to_all_online_players("Deathmatch Event: Event was canceled.");
        true
    }
}

/// Java `canRegister(player)`.
fn can_register(ctx: &mut ScriptCtx<'_>, player: ObjectId) -> bool {
    let (registered, count) = {
        let s = ctx.state::<DeathmatchState>();
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

/// `RANDOM: switch (getRandom(1, 3))`: a random point of one of the three battle zones.
fn teleport_to_random_spawn(ctx: &mut ScriptCtx<'_>, player: ObjectId, world: i32) {
    let which = ctx.get_random_range(1, 3);
    let zone = SPAWN_ZONES[(which - 1) as usize];
    if let Some(point) = ctx.world_mut().zone_random_point(zone) {
        ctx.world_mut().event_teleport(player, point, world, 50);
    }
}

fn remove_listeners(ctx: &mut ScriptCtx<'_>, player: ObjectId) {
    ctx.state::<DeathmatchState>().logout_listeners.remove(&player);
    ctx.remove_listeners(player, EventKind::CreatureDeath);
}

/// Java `resetActivityTimers(player)`.
fn reset_activity_timers(ctx: &mut ScriptCtx<'_>, player: ObjectId) {
    let kick = format!("KickPlayer{}", player.0);
    let warning = format!("KickPlayerWarning{}", player.0);
    ctx.cancel_quest_timer(&kick, None, Some(player));
    ctx.cancel_quest_timer(&warning, None, Some(player));
    let Some(world) = ctx.state::<DeathmatchState>().pvp_world else {
        return;
    };
    // NOTE(java-quirk): `PVP_WORLD.getDoor(BLUE_DOOR_ID)` is null without the template's doors
    // (a NullPointerException); no door counts as closed here.
    let open = ctx.world().is_instance_door_open(world, BLUE_DOOR_ID);
    ctx.start_quest_timer(
        &kick,
        if open {
            INACTIVITY_TIME * 60_000
        } else {
            (INACTIVITY_TIME * 60_000) + (WAIT_TIME * 1000)
        },
        None,
        Some(player),
        false,
    );
    ctx.start_quest_timer(
        &warning,
        if open {
            (INACTIVITY_TIME / 2) * 60_000
        } else {
            ((INACTIVITY_TIME / 2) * 60_000) + (WAIT_TIME * 1000)
        },
        None,
        Some(player),
        false,
    );
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::{ScriptTest, names_to, pages_to};

    const NPCS: &str = r#"<npc id="70011" type="Folk" level="1" name="Manager"/>"#;
    const ITEMS: &str = r#"<item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>"#;
    const DIR: &str = "data/scripts/custom/events/Deathmatch";

    fn scene(players: i32, level: i32) -> (ScriptTest, ObjectId) {
        let mut registry = ScriptRegistry::new();
        registry.add(Deathmatch);
        let pages: Vec<(String, String)> = [
            (
                "manager-register.html",
                r#"<html><body>register %player_numbers% <a action="bypass -h Script Deathmatch Participate">in</a></body></html>"#,
            ),
            ("manager-cancel.html", "<html><body>cancel %player_numbers%</body></html>"),
            ("registration-success.html", "<html><body>success</body></html>"),
            ("registration-failed.html", "<html><body>failed</body></html>"),
        ]
        .iter()
        .map(|(n, t)| (format!("{DIR}/{n}"), (*t).to_owned()))
        .collect();
        let refs: Vec<(&str, &str)> = pages.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
        let mut t = ScriptTest::new_with(registry, NPCS, ITEMS, &refs, |world, root| {
            // The three battle zones of the Coliseum (`pvp.xml`), squares of the real places.
            let zones = std::fs::create_dir_all(root.join("data/zones")).and_then(|()| {
                std::fs::write(
                    root.join("data/zones/pvp.xml"),
                    r#"<?xml version="1.0"?><list>
                    <zone name="colosseum_battle1" type="ArenaZone" shape="NPoly" minZ="-3498" maxZ="-3298"><node X="147800" Y="45400" /><node X="150700" Y="45400" /><node X="150700" Y="46700" /><node X="147800" Y="46700" /></zone>
                    <zone name="colosseum_battle2" type="ArenaZone" shape="NPoly" minZ="-3498" maxZ="-3298"><node X="147800" Y="46800" /><node X="150700" Y="46800" /><node X="150700" Y="48100" /><node X="147800" Y="48100" /></zone>
                    <zone name="colosseum_battle3" type="ArenaZone" shape="NPoly" minZ="-3498" maxZ="-3298"><node X="147800" Y="48200" /><node X="150700" Y="48200" /><node X="150700" Y="49500" /><node X="147800" Y="49500" /></zone>
                    </list>"#,
                )
            });
            zones.unwrap();
            world.set_zone_data(
                std::sync::Arc::new(l2_data::Zones::load(root)),
                std::sync::Arc::new(l2_data::MapRegions::load(root)),
            );
        });
        for id in 1..=players {
            t.enter(id, level, vec![]);
        }
        let manager = t.spawn(MANAGER, 20);
        t.take();
        (t, manager)
    }

    fn start(t: &mut ScriptTest) -> bool {
        let mut started = false;
        t.world.run_as_script("Deathmatch", |ctx| {
            started = Deathmatch.event_start(ctx, Some(ObjectId(1)));
        });
        started
    }

    fn register(t: &mut ScriptTest, player: i32, manager: ObjectId) -> String {
        t.talk(player, manager);
        t.take();
        t.bypass(player, "Script Deathmatch Participate");
        pages_to(&t.take(), player).pop().unwrap_or_default()
    }

    #[test]
    fn switched_off_nothing_runs() {
        let (mut t, manager) = scene(1, 90);
        t.ticks(10);
        t.talk(1, manager);
        assert!(pages_to(&t.take(), 1).iter().all(|p| !p.contains("register")));
        t.bypass(1, "Script Deathmatch Participate");
        assert!(pages_to(&t.take(), 1).is_empty());
        assert!(!t.world.is_registered_on_event(ObjectId(1)));
    }

    #[test]
    fn a_level_below_85_is_refused() {
        let (mut t, manager) = scene(1, 80);
        assert!(start(&mut t));
        assert!(register(&mut t, 1, manager).contains("failed"));
        assert!(!t.world.is_registered_on_event(ObjectId(1)));
    }

    #[test]
    fn everybody_fights_everybody_and_the_best_wins_a_million() {
        let (mut t, manager) = scene(4, 90);
        assert!(start(&mut t));
        assert!(!start(&mut t));
        for id in 1..=4 {
            assert!(register(&mut t, id, manager).contains("success"), "player {id}");
        }

        // After a minute of registration: all four in the instance, solo, frozen for 20 seconds.
        t.wait_ms(60_000 + 2_000);
        let players: Vec<ObjectId> = (1..=4).map(ObjectId).collect();
        let instance = t.world.instance_id_of(players[0]);
        assert!(instance > 0);
        assert!(
            players
                .iter()
                .all(|p| t.world.is_on_event(*p) && t.world.is_on_solo_event(*p) && t.world.instance_id_of(*p) == instance)
        );
        assert!(players.iter().all(|p| t.world.invul_flag(*p) && t.world.is_immobilized(*p) && t.world.is_all_skills_disabled(*p)));
        // Solo: every other player is a target.
        assert!(t.world.is_auto_attackable(players[1], players[0]));

        // The fight begins after the 20 seconds.
        t.wait_ms(20_000);
        assert!(players.iter().all(|p| !t.world.invul_flag(*p) && !t.world.is_immobilized(*p) && !t.world.is_all_skills_disabled(*p)));

        // Player 2 kills player 3 twice (the dead are brought back after ten seconds), player 1
        // kills once: player 2 leads.
        t.take();
        t.world.do_die(players[2], Some(players[1]));
        t.ticks(2);
        assert!(names_to(&t.take(), 4).iter().any(|n| n == "EX_PVP_MATCH_CC_RECORD"));
        t.wait_ms(10_500);
        t.world.do_die(players[2], Some(players[1]));
        t.world.do_die(players[3], Some(players[0]));
        t.ticks(2);

        // Three minutes of fight later the winner is paid, then everybody leaves.
        t.wait_ms(3 * 60_000 + 1_000);
        assert_eq!(t.count(2, 57), 1_000_000);
        assert_eq!(t.count(1, 57), 0);
        t.wait_ms(8_000);
        assert!(players.iter().all(|p| !t.world.is_on_event(*p) && !t.world.is_on_solo_event(*p)));
        assert!(players.iter().all(|p| t.world.instance_id_of(*p) == 0));
        // Inactive again: it can start anew.
        assert!(start(&mut t));
    }

    #[test]
    fn too_few_players_cancel_the_event() {
        let (mut t, manager) = scene(2, 90);
        assert!(start(&mut t));
        assert!(register(&mut t, 1, manager).contains("success"));
        t.wait_ms(60_000 + 2_000);
        assert!(!t.world.is_registered_on_event(ObjectId(1)));
        assert!(start(&mut t), "the event is over, a new one can start");
    }

    #[test]
    fn a_stop_frees_the_players() {
        let (mut t, manager) = scene(1, 90);
        assert!(start(&mut t));
        assert!(register(&mut t, 1, manager).contains("success"));
        let mut stopped = false;
        t.world.run_as_script("Deathmatch", |ctx| stopped = Deathmatch.event_stop(ctx));
        assert!(stopped);
        assert!(!t.world.is_registered_on_event(ObjectId(1)));
    }
}
