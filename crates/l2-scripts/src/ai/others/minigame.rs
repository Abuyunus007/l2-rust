//! Monastery Minigame AI: Sumiel lights the nine burners of a secret room in an order the player
//! has to repeat with the Torch Light skill.
// Java: dist/game/data/scripts/ai/others/Minigame/Minigame.java
//
// The Java `MinigameRoom` objects and the `ConsumerEventListener`s on the player are the script's
// state (`ctx.state`): the rooms, and a list of listeners (id, player, room) that the global
// `on_creature_skill_use` hook walks, since a listener of Java only reaches its own player.

use l2_protocol_game::NpcStringId;
use l2_protocol_game::server::ChatType;
use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup, SkillKey, TerminateReturn};

use crate::ai::areas::gracia::util::set_display_effect;

const SUMIEL: i32 = 32758;
const BURNER: i32 = 18913;
const TREASURE_BOX: i32 = 18911;

const UNLIT_TORCHLIGHT: i32 = 15540;
const TORCHLIGHT: i32 = 15485;

const SKILL_TORCH_LIGHT: i32 = 9059;
const TRIGGER_MIRAGE: SkillKey = SkillKey { id: 5144, level: 1 };

const TELEPORT1: (i32, i32, i32) = (113187, -85388, -3424);
const TELEPORT2: (i32, i32, i32) = (118833, -80589, -2688);

const TIMER_INTERVAL: i64 = 3;
const MAX_ATTEMPTS: i32 = 3;

/// Java `MinigameRoom`.
struct MinigameRoom {
    burners: Vec<ObjectId>,
    manager: ObjectId,
    participant: Option<ObjectId>,
    started: bool,
    attempt_number: i32,
    current_pot: i32,
    order: [i32; 9],
    /// Java `_listener`: the id of the listener the room added last.
    listener: Option<u32>,
}

/// A `ConsumerEventListener(participant, ON_CREATURE_SKILL_USE, .., room)`.
struct Listener {
    id: u32,
    player: ObjectId,
    /// Index of the owner room.
    room: usize,
}

/// Java `Minigame._rooms` and the listeners the rooms registered.
#[derive(Default)]
pub struct MinigameState {
    rooms: Vec<MinigameRoom>,
    listeners: Vec<Listener>,
    next_listener: u32,
}

fn room_by_manager(ctx: &mut ScriptCtx<'_>, manager: ObjectId) -> Option<usize> {
    ctx.state::<MinigameState>()
        .rooms
        .iter()
        .position(|r| r.manager == manager)
}

fn room_by_participant(ctx: &mut ScriptCtx<'_>, participant: ObjectId) -> Option<usize> {
    ctx.state::<MinigameState>()
        .rooms
        .iter()
        .position(|r| r.participant == Some(participant))
}

fn burn(ctx: &mut ScriptCtx<'_>, burner: ObjectId) {
    // `setDisplayEffect(1); setWalking();`
    set_display_effect(ctx, burner, 1);
    if let Some(npc) = ctx.script_npc(burner) {
        ctx.set_walking(npc);
    }
}

/// Java `room.burnThemAll()`.
fn burn_them_all(ctx: &mut ScriptCtx<'_>, room: usize) {
    let burners = ctx.state::<MinigameState>().rooms[room].burners.clone();
    for burner in burners {
        burn(ctx, burner);
    }
}

/// Java `room.clean()`.
fn clean(ctx: &mut ScriptCtx<'_>, room: usize) {
    let state = ctx.state::<MinigameState>();
    if let Some(id) = state.rooms[room].listener.take() {
        state.listeners.retain(|l| l.id != id);
    }
    let manager = state.rooms[room].manager;
    state.rooms[room].participant = None;
    state.rooms[room].started = false;
    state.rooms[room].attempt_number = 1;
    state.rooms[room].current_pot = 0;
    ctx.world_mut().set_npc_target(manager, None);
}

/// Java `room.setListener(listener)`.
fn set_listener(ctx: &mut ScriptCtx<'_>, room: usize) {
    let state = ctx.state::<MinigameState>();
    let Some(player) = state.rooms[room].participant else {
        return;
    };
    let id = state.next_listener;
    state.next_listener += 1;
    state.listeners.push(Listener { id, player, room });
    state.rooms[room].listener = Some(id);
}

fn say(ctx: &mut ScriptCtx<'_>, npc: ObjectId, text: NpcStringId) {
    if let Some(npc) = ctx.script_npc(npc) {
        ctx.broadcast_npc_say(npc, ChatType::NpcGeneral, text, &[]);
    }
}

/// Java `Minigame.initRoom(manager)`.
fn init_room(ctx: &mut ScriptCtx<'_>, manager: ObjectId) -> MinigameRoom {
    let mut burners = Vec::new();
    // `SpawnTable.getInstance().getSpawns(BURNER)` and the NPC each spawn made last. A spawn
    // without one (`getLastSpawn()` is null) is never in range.
    for spawn in ctx.spawns_of(BURNER) {
        let Some(last_spawn) = spawn.last_spawn else {
            continue;
        };
        if burners.len() <= 8 && ctx.check_if_in_range(1000, manager, last_spawn, false) {
            // `lastSpawn.setAutoAttackable(true)`.
            ctx.world_mut().set_npc_auto_attackable(last_spawn, true);
            burners.push(last_spawn);
        }
    }
    MinigameRoom {
        burners,
        manager,
        participant: None,
        started: false,
        attempt_number: 1,
        current_pot: 0,
        order: [0; 9],
        listener: None,
    }
}

/// Java `Minigame.onSkillUse(event)` for one registered listener.
fn on_skill_use(ctx: &mut ScriptCtx<'_>, caster: ObjectId, skill: SkillKey, target: ObjectId) {
    let Some(room) = room_by_participant(ctx, caster) else {
        return;
    };
    if !(ctx.state::<MinigameState>().rooms[room].started && skill.id == SKILL_TORCH_LIGHT) {
        return;
    }
    let Some(burner) = ctx.script_npc(target).filter(|n| n.id == BURNER) else {
        return;
    };
    ctx.world_mut().do_cast(burner.object_id, TRIGGER_MIRAGE);
    let (pos, expected, current_pot, attempt, manager, participant) = {
        let r = &ctx.state::<MinigameState>().rooms[room];
        let pos = r.burners.iter().position(|b| *b == burner.object_id).unwrap_or(0);
        let expected = usize::try_from(r.current_pot)
            .ok()
            .and_then(|p| r.order.get(p).copied())
            .unwrap_or(0);
        (
            i32::try_from(pos).unwrap_or(0),
            expected,
            r.current_pot,
            r.attempt_number,
            r.manager,
            r.participant,
        )
    };
    let manager_npc = ctx.script_npc(manager);
    if pos == expected {
        if current_pot < 8 {
            burn(ctx, burner.object_id);
            ctx.start_quest_timer("off", 2000, Some(burner), None, false);
            ctx.state::<MinigameState>().rooms[room].current_pot = current_pot + 1;
        } else {
            if let Some((x, y, z)) = participant.and_then(|p| ctx.world().position_of(p)) {
                let heading = participant.map_or(0, |p| ctx.heading_of(p));
                ctx.add_spawn(TREASURE_BOX, x, y, z, heading, true, 0, 0);
            }
            say(ctx, manager, NpcStringId::OhYouVeSucceeded);
            ctx.state::<MinigameState>().rooms[room].current_pot = 0;
            burn_them_all(ctx, room);
            ctx.start_quest_timer("off", 2000, manager_npc, None, false);
            ctx.start_quest_timer("end", 4000, manager_npc, None, false);
        }
    } else if attempt == MAX_ATTEMPTS {
        say(ctx, manager, NpcStringId::AhIVeFailedGoingFurtherWillBeDifficult);
        burn_them_all(ctx, room);
        ctx.start_quest_timer("off", 2000, manager_npc, None, false);
        // `removeListenerIf(.. listener.getOwner() == room)`: every listener of the room.
        ctx.state::<MinigameState>().listeners.retain(|l| l.room != room);
        ctx.start_quest_timer("end", 4000, manager_npc, None, false);
    } else if attempt < MAX_ATTEMPTS {
        say(
            ctx,
            manager,
            NpcStringId::AhIsThisFailureButItLooksLikeICanKeepGoing,
        );
        burn_them_all(ctx, room);
        ctx.start_quest_timer("off", 2000, manager_npc, None, false);
        ctx.state::<MinigameState>().rooms[room].attempt_number = attempt + 1;
    }
}

/// Java `ai.others.Minigame.Minigame`.
pub struct Minigame;

impl Script for Minigame {
    fn name(&self) -> &'static str {
        "Minigame"
    }

    fn path(&self) -> &'static str {
        "ai/others/Minigame"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[SUMIEL]);
        s.add_first_talk_id(&[SUMIEL]);
        s.add_talk_id(&[SUMIEL]);
        s.add_spawn_id(&[SUMIEL, TREASURE_BOX]);
        s.set_on_creature_skill_use();
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        // NOTE(java-quirk): Java reads `npc.getId()`/`getRoomByManager(npc)` first; a null NPC
        // throws, nothing happens.
        let npc = npc?;
        // NOTE(java-quirk): `room` is null for a burner or a treasure box; only the cases that
        // use it would throw.
        let room = room_by_manager(ctx, npc.object_id);
        match event {
            "restart" => {
                let (room, player) = (room?, player?);
                let started = ctx.state::<MinigameState>().rooms[room].started;
                if !started && !ctx.has_quest_items(player, UNLIT_TORCHLIGHT) {
                    return Some("32758-05.html".to_owned());
                }

                if ctx
                    .world()
                    .creature_target(npc.object_id)
                    .is_some_and(|t| t != player)
                {
                    return Some("32758-04.html".to_owned());
                }

                ctx.take_items(player, UNLIT_TORCHLIGHT, 1);
                ctx.give_items(player, TORCHLIGHT, 1);
                ctx.broadcast_npc_say(
                    npc,
                    ChatType::NpcGeneral,
                    NpcStringId::TheFurnaceWillGoOutWatchAndSee,
                    &[],
                );
                let manager = ctx.state::<MinigameState>().rooms[room].manager;
                ctx.world_mut().set_npc_target(manager, Some(player));
                for i in 0..9 {
                    let n = ctx.get_random(8);
                    ctx.state::<MinigameState>().rooms[room].order[i] = n;
                }
                let r = &mut ctx.state::<MinigameState>().rooms[room];
                r.participant = Some(player);
                r.started = true;

                ctx.cancel_quest_timer("hurry_up", Some(npc), None);
                ctx.cancel_quest_timer("hurry_up2", Some(npc), None);
                ctx.cancel_quest_timer("expire", Some(npc), None);
                ctx.start_quest_timer("hurry_up", 120_000, Some(npc), None, false);
                ctx.start_quest_timer("expire", 190_000, Some(npc), None, false);
                ctx.start_quest_timer("start", 1000, Some(npc), None, false);
                return None;
            }
            "off" => {
                if npc.id == BURNER {
                    set_display_effect(ctx, npc.object_id, 2);
                } else {
                    let burners = ctx.state::<MinigameState>().rooms[room?].burners.clone();
                    for burner in burners {
                        set_display_effect(ctx, burner, 2);
                    }
                }
            }
            "teleport1" => {
                let (x, y, z) = TELEPORT1;
                ctx.teleport_player(player?, x, y, z, 0, false);
            }
            "teleport2" => {
                let (x, y, z) = TELEPORT2;
                ctx.teleport_player(player?, x, y, z, 0, false);
            }
            "start" => {
                let room = room?;
                burn_them_all(ctx, room);
                // It should be null to stop burnthemAll 2s after.
                ctx.start_quest_timer("off", 2000, Some(npc), None, false);
                ctx.start_quest_timer("timer", 4000, Some(npc), None, false);
            }
            "timer" => {
                let room = room?;
                let current_pot = ctx.state::<MinigameState>().rooms[room].current_pot;
                if current_pot < 9 {
                    let r = &ctx.state::<MinigameState>().rooms[room];
                    let at = usize::try_from(current_pot)
                        .ok()
                        .and_then(|p| r.order.get(p).copied())
                        .and_then(|o| usize::try_from(o).ok());
                    let b = at.and_then(|at| r.burners.get(at).copied());
                    if let Some(b) = b {
                        burn(ctx, b);
                        // Stopping burning each pot 2s after.
                        let b = ctx.script_npc(b);
                        ctx.start_quest_timer("off", 2000, b, None, false);
                    }
                    ctx.start_quest_timer("timer", TIMER_INTERVAL * 1000, Some(npc), None, false);
                    ctx.state::<MinigameState>().rooms[room].current_pot = current_pot + 1;
                } else {
                    let manager = ctx.state::<MinigameState>().rooms[room].manager;
                    say(ctx, manager, NpcStringId::NowLightTheFurnaceSFire);
                    burn_them_all(ctx, room);
                    ctx.start_quest_timer("off", 2000, Some(npc), None, false);
                    set_listener(ctx, room);
                    ctx.state::<MinigameState>().rooms[room].current_pot = 0;
                }
            }
            "hurry_up" => {
                ctx.broadcast_npc_say(
                    npc,
                    ChatType::NpcGeneral,
                    NpcStringId::ThereSAbout1MinuteLeft,
                    &[],
                );
                ctx.start_quest_timer("hurry_up2", 60_000, Some(npc), None, false);
            }
            "hurry_up2" => {
                ctx.broadcast_npc_say(
                    npc,
                    ChatType::NpcGeneral,
                    NpcStringId::ThereSJust10SecondsLeft,
                    &[],
                );
                ctx.start_quest_timer("expire", 10_000, Some(npc), None, false);
            }
            "expire" => {
                ctx.broadcast_npc_say(
                    npc,
                    ChatType::NpcGeneral,
                    NpcStringId::TimeIsUpAndYouHaveFailedAnyMoreWillBeDifficult,
                    &[],
                );
            }
            "end" => {
                ctx.cancel_quest_timer("expire", Some(npc), None);
                ctx.cancel_quest_timer("hurry_up", Some(npc), None);
                ctx.cancel_quest_timer("hurry_up2", Some(npc), None);
                clean(ctx, room?);
            }
            "afterthat" => ctx.delete_npc(npc),
            _ => {}
        }

        // NOTE(java-quirk): every event but "restart" returns itself, so a player's
        // "teleport1" is also sent as a chat message.
        Some(event.to_owned())
    }

    fn on_first_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, talker: ObjectId) -> Option<String> {
        let room = room_by_manager(ctx, npc.object_id)?;
        let (started, attempt) = {
            let r = &ctx.state::<MinigameState>().rooms[room];
            (r.started, r.attempt_number)
        };
        let target = ctx.world().creature_target(npc.object_id);
        let htmltext = if target.is_none() {
            Some(if started { "32758-08.html" } else { "32758.html" })
        } else if target == Some(talker) {
            if started {
                Some("32758-07.html")
            } else if attempt == 2 {
                Some("32758-02.html")
            } else if attempt == 3 {
                Some("32758-03.html")
            } else {
                None
            }
        } else {
            Some("32758-04.html")
        };
        htmltext.map(str::to_owned)
    }

    fn on_spawn(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc) {
        match npc.id {
            SUMIEL => {
                let room = init_room(ctx, npc.object_id);
                ctx.state::<MinigameState>().rooms.push(room);
            }
            TREASURE_BOX => {
                ctx.disable_core_ai(npc, true);
                ctx.cancel_quest_timer("afterthat", Some(npc), None);
                ctx.start_quest_timer("afterthat", 180_000, Some(npc), None, false);
            }
            _ => {}
        }
    }

    fn on_creature_skill_use(
        &self,
        ctx: &mut ScriptCtx<'_>,
        caster: ObjectId,
        skill: SkillKey,
        _simultaneously: bool,
        target: ObjectId,
    ) -> Option<TerminateReturn> {
        // One call per listener the caster has (Java adds a new listener at each round).
        let count = ctx
            .state::<MinigameState>()
            .listeners
            .iter()
            .filter(|l| l.player == caster)
            .count();
        for _ in 0..count {
            on_skill_use(ctx, caster, skill, target);
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::{Pages, ScriptTest, item};

    const NPCS: &str = r#"
        <npc id="32758" type="Folk" level="70" name="Sumiel"/>
        <npc id="18913" type="Monster" level="70" name="Burner"/>
        <npc id="18911" type="Monster" level="70" name="Treasure Box"/>"#;
    const ITEMS: &str = r#"
        <item id="15540" type="EtcItem" name="Unlit"><set name="is_questitem" val="true"/></item>
        <item id="15485" type="EtcItem" name="Torch"><set name="is_questitem" val="true"/></item>"#;

    fn setup() -> (ScriptTest, ScriptNpc, Vec<ObjectId>) {
        let mut registry = ScriptRegistry::new();
        registry.add(Minigame);
        let pages = Pages::of(&Minigame).plain(&[
            "32758.html",
            "32758-02.html",
            "32758-03.html",
            "32758-04.html",
            "32758-05.html",
            "32758-07.html",
            "32758-08.html",
        ]);
        // Nine burners of the spawn table, east of the manager.
        let spawns: String = (0..9)
            .map(|i| {
                format!(
                    r#"<npc id="18913" x="{}" y="{}" z="{}" heading="0" respawnDelay="60" />"#,
                    crate::testing::X + 100 + i * 10,
                    crate::testing::Y,
                    crate::testing::Z
                )
            })
            .collect();
        let pages = pages.file(
            "data/spawns/burners.xml",
            &format!(
                r#"<?xml version="1.0"?><list enabled="true"><spawn name="burners">{spawns}</spawn></list>"#
            ),
        );
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        // The burners exist before Sumiel, as `initRoom` reads them at his spawn.
        t.world.spawn_all_npcs(false);
        let burners: Vec<ObjectId> = t
            .world
            .spawns_of(BURNER)
            .into_iter()
            .filter_map(|s| s.last_spawn)
            .collect();
        assert_eq!(burners.len(), 9);
        let sumiel = t.spawn(SUMIEL, 20);
        t.enter(1, 80, vec![item(1001, 15540, 1)]);
        t.enter(2, 80, vec![]);
        let npc = ScriptNpc {
            object_id: sumiel,
            id: SUMIEL,
        };
        (t, npc, burners)
    }

    fn event(t: &mut ScriptTest, npc: ScriptNpc, player: i32, event: &str) -> Option<String> {
        let mut result = None;
        t.world.run_as_script("Minigame", |ctx| {
            result = Minigame.on_event(ctx, event, Some(npc), Some(ObjectId(player)));
        });
        result
    }

    fn first_talk(t: &mut ScriptTest, npc: ScriptNpc, player: i32) -> Option<String> {
        let mut result = None;
        t.world.run_as_script("Minigame", |ctx| {
            result = Minigame.on_first_talk(ctx, npc, ObjectId(player));
        });
        result
    }

    fn cast(t: &mut ScriptTest, target: ObjectId) {
        let skill = SkillKey {
            id: SKILL_TORCH_LIGHT,
            level: 1,
        };
        t.world.run_as_script("Minigame", |ctx| {
            Minigame.on_creature_skill_use(ctx, ObjectId(1), skill, false, target);
        });
    }

    fn order(t: &mut ScriptTest) -> [i32; 9] {
        let mut order = [0; 9];
        t.world.run_as_script("Minigame", |ctx| {
            order = ctx.state::<MinigameState>().rooms[0].order;
        });
        order
    }

    /// Starts a round for player 1 and lets the burners show their order.
    fn start_round(t: &mut ScriptTest, npc: ScriptNpc) {
        assert_eq!(event(t, npc, 1, "restart"), None);
        t.wait_ms(33_000);
    }

    #[test]
    fn the_room_finds_its_burners_and_the_talk_pages_follow_the_state() {
        let (mut t, npc, _) = setup();
        assert_eq!(first_talk(&mut t, npc, 1).as_deref(), Some("32758.html"));
        assert_eq!(event(&mut t, npc, 2, "restart").as_deref(), Some("32758-05.html"));
        assert_eq!(event(&mut t, npc, 1, "restart"), None);
        assert_eq!(t.count(1, 15540), 0);
        assert_eq!(t.count(1, 15485), 1);
        assert_eq!(first_talk(&mut t, npc, 1).as_deref(), Some("32758-07.html"));
        assert_eq!(first_talk(&mut t, npc, 2).as_deref(), Some("32758-04.html"));
        assert_eq!(event(&mut t, npc, 2, "restart").as_deref(), Some("32758-04.html"));
    }

    #[test]
    fn repeating_the_order_pays_a_treasure_box_and_cleans_the_room() {
        let (mut t, npc, burners) = setup();
        start_round(&mut t, npc);
        let order = order(&mut t);
        for pot in order {
            cast(&mut t, burners[usize::try_from(pot).unwrap()]);
        }
        assert_eq!(t.npcs_of(TREASURE_BOX).len(), 1);
        t.wait_ms(4_100);
        assert_eq!(first_talk(&mut t, npc, 1).as_deref(), Some("32758.html"));
        // The box leaves after 3 minutes.
        t.wait_ms(180_000);
        assert!(t.npcs_of(TREASURE_BOX).is_empty());
    }

    #[test]
    fn three_wrong_burners_end_the_game() {
        let (mut t, npc, burners) = setup();
        start_round(&mut t, npc);
        let first = usize::try_from(order(&mut t)[0]).unwrap();
        let wrong = burners[(first + 1) % 9];
        cast(&mut t, wrong);
        cast(&mut t, wrong);
        cast(&mut t, wrong);
        t.wait_ms(4_100);
        assert_eq!(first_talk(&mut t, npc, 1).as_deref(), Some("32758.html"));
        // No listener is left: a good cast does nothing.
        cast(&mut t, burners[first]);
        assert!(t.npcs_of(TREASURE_BOX).is_empty());
    }

    #[test]
    fn the_teleports_send_the_player_to_the_rooms() {
        let (mut t, npc, _) = setup();
        assert_eq!(event(&mut t, npc, 2, "teleport1").as_deref(), Some("teleport1"));
        t.ticks(60);
        assert_eq!(t.world.position_of(ObjectId(2)).map(|p| p.0), Some(TELEPORT1.0));
    }
}
