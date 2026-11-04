//! Race event: sign up at the start NPC in Dion, transform, and be the first to reach the finish
//! NPC at a random place.
// Off by default: the shipped `config.xml` schedules nothing, so the event only runs when a GM
// starts it (`//event_start Race`).

use l2_protocol_game::server::ChatType;
use l2_util::ObjectId;
use l2_world::{JavaException, Script, ScriptCtx, ScriptNpc, ScriptSetup, SkillKey, TimerParams};

use super::schedule;

// 5 min for register
const REGISTER_TIME: i64 = 5;

// 5 min for race (the Java comment; the value is ten)
const RACE_TIME: i64 = 10;

// NPCs
const START_NPC: i32 = 900_103;
const STOP_NPC: i32 = 900_104;

// Skills (Frog by default)
const DEFAULT_SKILL: i32 = 6201;

// Locations
const LOCATIONS: [&str; 4] = [
    "Heretic catacomb enterance",
    "Dion castle bridge",
    "Floran village enterance",
    "Floran fort gate",
];

// x, y, z, heading
const COORDS: [[i32; 4]; 4] = [
    [39_177, 144_345, -3650, 0],
    [22_294, 155_892, -2950, 0],
    [16_537, 169_937, -3500, 0],
    [7_644, 150_898, -2890, 0],
];

const REWARDS: [[i32; 2]; 14] = [
    [6622, 2], // Giant's Codex
    [9625, 2], // Giant's Codex -
    [9626, 2], // Giant's Codex -
    [9627, 2], // Giant's Codex -
    [9546, 5], // Attr stones
    [9547, 5],
    [9548, 5],
    [9549, 5],
    [9550, 5],
    [9551, 5],
    [9574, 3],  // Mid-Grade Life Stone: level 80
    [9575, 2],  // High-Grade Life Stone: level 80
    [9576, 1],  // Top-Grade Life Stone: level 80
    [20034, 1], // Revita pop
];

/// The timers of Java's `ThreadPool.schedule` calls (`_eventTask`).
const START_RACE: &str = "RACE_START_RACE";
const TIME_UP: &str = "RACE_TIME_UP";

/// The Java fields: `_npcs`, `_npc`, `_players`, `_isactive`, `_isRaceStarted`, `_skill`,
/// `_randspawn`.
struct RaceState {
    npcs: Vec<ObjectId>,
    npc: Option<ScriptNpc>,
    players: Vec<ObjectId>,
    is_active: bool,
    is_race_started: bool,
    skill: i32,
    randspawn: Option<[i32; 4]>,
}

impl Default for RaceState {
    fn default() -> Self {
        Self {
            npcs: Vec::new(),
            npc: None,
            players: Vec::new(),
            is_active: false,
            is_race_started: false,
            skill: DEFAULT_SKILL,
            randspawn: None,
        }
    }
}

/// Java `custom.events.Race.Race`.
pub struct Race;

impl Script for Race {
    fn name(&self) -> &'static str {
        "Race"
    }

    fn path(&self) -> &'static str {
        "custom/events/Race"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[START_NPC]);
        s.add_first_talk_id(&[START_NPC]);
        s.add_talk_id(&[START_NPC]);
        s.add_start_npc(&[STOP_NPC]);
        s.add_first_talk_id(&[STOP_NPC]);
        s.add_talk_id(&[STOP_NPC]);
    }

    fn on_load(&self, ctx: &mut ScriptCtx<'_>) {
        schedule::load_config(ctx, "Race", "Race");
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

    fn event_start(&self, ctx: &mut ScriptCtx<'_>, event_maker: Option<ObjectId>) -> bool {
        // Don't start event if it is active.
        if ctx.state::<RaceState>().is_active {
            return false;
        }

        // Check Custom Table - we use custom NPCs.
        if !ctx.world().custom_script_settings().custom_npc_data {
            tracing::info!("Race: Event can't be started, because custom npc table is disabled!");
            if let Some(maker) = event_maker {
                ctx.world().send_message(
                    maker,
                    "Event Race can't be started because custom NPC table is disabled!",
                );
            }
            return false;
        }

        // Set Event active
        ctx.state::<RaceState>().is_active = true;

        // Spawn Manager
        let npc = record_spawn(ctx, START_NPC, 18_429, 145_861, -3090, 35_000, false, 0);
        ctx.state::<RaceState>().npc = npc;

        // Announce event start
        ctx.broadcast_to_all_online_players("* Race Event started! *");
        ctx.broadcast_to_all_online_players(&format!(
            "Visit Event Manager in Dion village and signup, you have {REGISTER_TIME} min before Race Start..."
        ));

        // Schedule Event end
        ctx.start_quest_timer(START_RACE, REGISTER_TIME * 60 * 1000, None, None, false);
        true
    }

    fn event_stop(&self, ctx: &mut ScriptCtx<'_>) -> bool {
        // Don't stop inactive event.
        if !ctx.state::<RaceState>().is_active {
            return false;
        }

        // Set inactive
        ctx.state::<RaceState>().is_active = false;
        ctx.state::<RaceState>().is_race_started = false;

        // Cancel task if any.
        ctx.cancel_quest_timers(START_RACE);
        ctx.cancel_quest_timers(TIME_UP);

        // Untransform players
        // Teleport to event start point.
        let players = std::mem::take(&mut ctx.state::<RaceState>().players);
        let at = ctx
            .state::<RaceState>()
            .npc
            .and_then(|npc| ctx.world().position_of(npc.object_id));
        for player in players {
            if ctx.world().player(player).is_some() {
                ctx.world_mut().untransform(player);
                if let Some((x, y, z)) = at {
                    ctx.teleport_player(player, x, y, z, -1, true);
                }
            }
        }

        // Despawn NPCs
        let npcs = std::mem::take(&mut ctx.state::<RaceState>().npcs);
        for npc in npcs {
            if let Some(npc) = ctx.script_npc(npc) {
                ctx.delete_npc(npc);
            }
        }

        // Announce event end
        ctx.broadcast_to_all_online_players("* Race Event finished *");
        true
    }

    fn event_bypass(
        &self,
        ctx: &mut ScriptCtx<'_>,
        player: ObjectId,
        bypass: &str,
    ) -> Result<bool, JavaException> {
        if let Some(number) = bypass.strip_prefix("skill") {
            if ctx.state::<RaceState>().is_race_started {
                ctx.world().send_message(
                    player,
                    "Race already started, you cannot change transform skill now",
                );
            } else {
                let number: i32 = number.parse().map_err(|_| JavaException::number_format(number))?;
                // `SkillData.getSkill(number, 1)`.
                if let Some(name) = ctx.world().skill_name_at(number, 1) {
                    ctx.state::<RaceState>().skill = number;
                    ctx.world().send_message(player, "Transform skill set to:");
                    ctx.world().send_message(player, &name);
                } else {
                    ctx.world()
                        .send_message(player, "Error while changing transform skill");
                }
            }
        } else if let Some(number) = bypass.strip_prefix("tele") {
            let number: i32 = number.parse().map_err(|_| JavaException::number_format(number))?;
            let randspawn = ctx.state::<RaceState>().randspawn;
            if number > 0 && randspawn.is_some() {
                if let Some(r) = randspawn {
                    ctx.teleport_player(player, r[0], r[1], r[2], -1, false);
                }
            } else {
                ctx.teleport_player(player, 18_429, 145_861, -3090, -1, false);
            }
        }

        show_menu(ctx, player);
        Ok(true)
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        _npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        // The two timers Java's `ThreadPool.schedule` is.
        if event == START_RACE {
            start_race(self, ctx);
            return None;
        }
        if event == TIME_UP {
            time_up(self, ctx);
            return None;
        }

        let player = player?;
        let htmltext = event.to_owned();
        ctx.get_quest_state(player, false)?;

        if event.eq_ignore_ascii_case("transform") {
            transform_player(ctx, player);
            return None;
        } else if event.eq_ignore_ascii_case("untransform") {
            ctx.world_mut().untransform(player);
            return None;
        } else if event.eq_ignore_ascii_case("showfinish") {
            if let Some(r) = ctx.state::<RaceState>().randspawn {
                ctx.add_radar(player, r[0], r[1], r[2]);
            }
            return None;
        } else if event.eq_ignore_ascii_case("signup") {
            if ctx.state::<RaceState>().players.contains(&player) {
                return Some("900103-onlist.htm".to_owned());
            }

            ctx.state::<RaceState>().players.push(player);
            return Some("900103-signup.htm".to_owned());
        } else if event.eq_ignore_ascii_case("quit") {
            ctx.world_mut().untransform(player);
            ctx.state::<RaceState>().players.retain(|p| *p != player);
            return Some("900103-quit.htm".to_owned());
        } else if event.eq_ignore_ascii_case("finish") {
            let skill = ctx.state::<RaceState>().skill;
            if ctx.is_affected_by_skill(player, skill) {
                win_race(self, ctx, player);
                return Some("900104-winner.htm".to_owned());
            }

            return Some("900104-notrans.htm".to_owned());
        }

        Some(htmltext)
    }

    fn on_first_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        ctx.get_quest_state(player, true);
        let is_race_started = ctx.state::<RaceState>().is_race_started;
        if npc.id == START_NPC {
            if is_race_started {
                return Some(format!("{START_NPC}-started-{}.htm", is_racing(ctx, player)));
            }

            return Some(format!("{START_NPC}-{}.htm", is_racing(ctx, player)));
        } else if npc.id == STOP_NPC && is_race_started {
            return Some(format!("{STOP_NPC}-{}.htm", is_racing(ctx, player)));
        }

        Some(format!("{}.htm", npc.id))
    }
}

/// Java `StartRace()`.
fn start_race(script: &Race, ctx: &mut ScriptCtx<'_>) {
    // Abort race if no players signup.
    if ctx.state::<RaceState>().players.is_empty() {
        ctx.broadcast_to_all_online_players("Race aborted, nobody signup.");
        script.event_stop(ctx);
        return;
    }

    // Set state
    ctx.state::<RaceState>().is_race_started = true;

    // Announce
    ctx.broadcast_to_all_online_players("Race started!");

    // Get random Finish
    let location = ctx.get_random_range(0, LOCATIONS.len() as i32 - 1) as usize;
    let randspawn = COORDS[location];
    ctx.state::<RaceState>().randspawn = Some(randspawn);

    // And spawn NPC
    record_spawn(
        ctx,
        STOP_NPC,
        randspawn[0],
        randspawn[1],
        randspawn[2],
        randspawn[3],
        false,
        0,
    );

    // Transform players and send message.
    let players = ctx.state::<RaceState>().players.clone();
    let manager = ctx.state::<RaceState>().npc;
    for player in players {
        if ctx.world().player(player).is_some() {
            let near = manager.is_some_and(|m| ctx.is_inside_radius_2d(player, m.object_id, 500));
            if near {
                send_message(
                    ctx,
                    player,
                    &format!(
                        "Race started! Go find Finish NPC as fast as you can... He is located near {}",
                        LOCATIONS[location]
                    ),
                );
                transform_player(ctx, player);
                ctx.add_radar(player, randspawn[0], randspawn[1], randspawn[2]);
            } else {
                send_message(
                    ctx,
                    player,
                    "I told you stay near me right? Distance was too high, you are excluded from race",
                );
                ctx.state::<RaceState>().players.retain(|p| *p != player);
            }
        }
    }

    // Schedule timeup for Race.
    ctx.start_quest_timer(TIME_UP, RACE_TIME * 60 * 1000, None, None, false);
}

/// Java `timeUp()`.
fn time_up(script: &Race, ctx: &mut ScriptCtx<'_>) {
    ctx.broadcast_to_all_online_players("Time up, nobody wins!");
    script.event_stop(ctx);
}

/// Java `winRace(player)`.
fn win_race(script: &Race, ctx: &mut ScriptCtx<'_>, player: ObjectId) {
    let reward = REWARDS[ctx.get_random(REWARDS.len() as i32 - 1) as usize];
    ctx.world_mut()
        .add_item(player, reward[0], i64::from(reward[1]), true);
    let name = ctx.player_name(player);
    ctx.broadcast_to_all_online_players(&format!("{name} is a winner!"));
    script.event_stop(ctx);
}

/// Java `isRacing(player)`.
fn is_racing(ctx: &mut ScriptCtx<'_>, player: ObjectId) -> i32 {
    i32::from(ctx.state::<RaceState>().players.contains(&player))
}

/// Java `recordSpawn(npcId, x, y, z, heading, randomOffSet, despawnDelay)`.
#[allow(clippy::too_many_arguments)]
fn record_spawn(
    ctx: &mut ScriptCtx<'_>,
    npc_id: i32,
    x: i32,
    y: i32,
    z: i32,
    heading: i32,
    random_offset: bool,
    despawn_delay: i64,
) -> Option<ScriptNpc> {
    let npc = ctx.add_spawn(npc_id, x, y, z, heading, random_offset, despawn_delay, 0)?;
    ctx.state::<RaceState>().npcs.push(npc.object_id);
    Some(npc)
}

/// Java `transformPlayer(player)`.
fn transform_player(ctx: &mut ScriptCtx<'_>, player: ObjectId) {
    if ctx.world().is_transformed(player) || ctx.world().is_in_stance(player) {
        ctx.world_mut().untransform(player);
    }

    if ctx.world().is_sitting(player) {
        ctx.world_mut().stand_up(player);
    }

    ctx.world_mut().stop_speed_up_effects(player);
    ctx.world_mut().stop_skill_effects_removed(player, 268);
    ctx.world_mut().stop_skill_effects_removed(player, 298); // Rabbit Spirit Totem
    let skill = ctx.state::<RaceState>().skill;
    ctx.world_mut()
        .apply_effects(player, player, SkillKey { id: skill, level: 1 });
}

/// Java `sendMessage(player, text)`: the manager says it on the battlefield channel.
fn send_message(ctx: &mut ScriptCtx<'_>, player: ObjectId, text: &str) {
    if let Some(npc) = ctx.state::<RaceState>().npc {
        let name = ctx.world().npc_template_name(npc.object_id);
        ctx.world()
            .send_creature_say_text(player, Some(npc.object_id), ChatType::Battlefield, &name, text);
    }
}

/// Java `showMenu(player)`.
fn show_menu(ctx: &mut ScriptCtx<'_>, player: ObjectId) {
    let content = ctx.get_htm(player, "admin_menu.htm").unwrap_or_default();
    ctx.world_mut().send_page_text(player, &content);
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;
    use l2_world::custom::CustomScriptSettings;

    use super::*;
    use crate::testing::{ScriptTest, pages_to};

    const NPCS: &str = r#"<npc id="900103" type="Folk" level="1" name="Start"/>
        <npc id="900104" type="Folk" level="1" name="Finish"/>"#;
    const ITEMS: &str = r#"<item id="6622" type="EtcItem" name="Codex"/>"#;
    const DIR: &str = "data/scripts/custom/events/Race";

    fn scene(custom_npc_data: bool) -> ScriptTest {
        let mut registry = ScriptRegistry::new();
        registry.add(Race);
        let pages: Vec<(String, String)> = [
            (
                "900103-0.htm",
                r#"<html><body>start0 <a action="bypass -h Script Race signup">s</a></body></html>"#,
            ),
            ("900103-1.htm", "<html><body>start1</body></html>"),
            ("900103-signup.htm", "<html><body>signed</body></html>"),
            ("900103-onlist.htm", "<html><body>onlist</body></html>"),
            ("900103-quit.htm", "<html><body>quit</body></html>"),
            ("admin_menu.htm", "<html><body>menu</body></html>"),
        ]
        .iter()
        .map(|(n, t)| (format!("{DIR}/{n}"), (*t).to_owned()))
        .collect();
        let refs: Vec<(&str, &str)> = pages.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
        let mut t = ScriptTest::new_with(registry, NPCS, ITEMS, &refs, |world, _| {
            world.set_custom_script_settings(CustomScriptSettings {
                custom_npc_data,
                ..CustomScriptSettings::default()
            });
        });
        t.enter(1, 40, vec![]);
        t.take();
        t
    }

    fn start(t: &mut ScriptTest) -> bool {
        let mut started = false;
        t.world.run_as_script("Race", |ctx| {
            started = Race.event_start(ctx, Some(ObjectId(1)));
        });
        started
    }

    #[test]
    fn switched_off_nothing_spawns() {
        let mut t = scene(false);
        t.ticks(5);
        assert!(t.npcs_of(START_NPC).is_empty());
        assert!(!start(&mut t));
        assert!(t.npcs_of(START_NPC).is_empty());
    }

    #[test]
    fn the_registration_and_an_empty_race() {
        let mut t = scene(true);
        assert!(start(&mut t));
        assert!(!start(&mut t));
        // The manager of the event stands in Dion; a second one next to the player does the talking.
        let manager = t.spawn(START_NPC, 20);
        t.talk(1, manager);
        assert!(pages_to(&t.take(), 1).iter().any(|p| p.contains("start0")));
        t.bypass(1, "Script Race signup");
        assert!(pages_to(&t.take(), 1).iter().any(|p| p.contains("signed")));
        // The race page changes: the player is on the list.
        t.talk(1, manager);
        assert!(pages_to(&t.take(), 1).iter().any(|p| p.contains("start1")));

        // Nobody near the manager (the test players stand far from Dion): the race starts but
        // the player is excluded, the finish NPC stands.
        t.wait_ms(5 * 60_000 + 2_000);
        assert_eq!(t.npcs_of(STOP_NPC).len(), 1);
        t.wait_ms(10 * 60_000 + 2_000);
        // Time up: everything is gone and a new race can start.
        assert!(t.npcs_of(STOP_NPC).iter().all(|n| !t.in_world(*n)));
        assert!(start(&mut t));
    }

    #[test]
    fn nobody_signed_up_aborts_the_race() {
        let mut t = scene(true);
        assert!(start(&mut t));
        t.wait_ms(5 * 60_000 + 2_000);
        assert!(t.npcs_of(START_NPC).iter().all(|n| !t.in_world(*n)));
        assert!(t.npcs_of(STOP_NPC).is_empty());
        assert!(start(&mut t));
    }

    #[test]
    fn the_admin_bypass_changes_the_skill_and_shows_the_menu() {
        let mut t = scene(true);
        let mut result = None;
        t.world.run_as_script("Race", |ctx| {
            result = Some(Race.event_bypass(ctx, ObjectId(1), "tele0"));
        });
        assert_eq!(result, Some(Ok(true)));
        assert!(pages_to(&t.take(), 1).iter().any(|p| p.contains("menu")));
        t.world.run_as_script("Race", |ctx| {
            result = Some(Race.event_bypass(ctx, ObjectId(1), "skillx"));
        });
        assert!(matches!(result, Some(Err(_))));
    }
}
