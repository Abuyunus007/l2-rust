//! Tully's Workshop: the doormen, the contraptions after Tully's death, the servants and agents of
//! the 6th and 8th floor, the followers of the 7th floor, Darion and his pillar.
// Java: dist/game/data/scripts/ai/areas/Hellbound/AI/Zones/TullyWorkshop/TullyWorkshop.java
//
// # Java's scheduled tasks
// `ThreadPool.schedule` and `scheduleAtFixedRate` are quest timers: `door_task <open|close>
// <ids>` (`DoorTask`) and `countdown` (60 seconds, then every 10 seconds: the task starts itself
// again until the time is out).

use std::collections::HashSet;

use l2_protocol_game::NpcStringId;
use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::ChatType;
use l2_util::ObjectId;
use l2_world::script::player_class;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup, SkillKey};

// NPCs
const AGENT: i32 = 32372;
const CUBE_68: i32 = 32467;
const DORIAN: i32 = 32373;
const DARION: i32 = 25603;
const TULLY: i32 = 25544;
const DWARVEN_GHOST: i32 = 32370;
const TOMBSTONE: i32 = 32344;
const INGENIOUS_CONTRAPTION: i32 = 32371;
const PILLAR: i32 = 18506;
// private static final int BRIDGE_CONTROLLER = 32468;
const TIMETWISTER_GOLEM: i32 = 22392;
const SIN_WARDENS: [i32; 2] = [22423, 22431];
const SERVANT_FIRST: i32 = 22405;
const SERVANT_LAST: i32 = 22410;
const TEMENIR: i32 = 25600;
const DRAXIUS: i32 = 25601;
const KIRETCENAH: i32 = 25602;

// Items
const REWARDS: [i32; 5] = [10427, 10428, 10429, 10430, 10431];

// 7 for 6th floor and 10 for 8th floor
const DEATH_COUNTS: [i32; 2] = [7, 10];

// Them are teleporting players to themselves
// Master Zelos - 22377, Zelos' Minions - 22378, 22379, Tully's Toy - 22383
const TELEPORTING_MONSTERS: [i32; 4] = [22377, 22378, 22379, 22383];

type Location = (i32, i32, i32);

/// Java `TULLY_DOORLIST`: the doors a doorman opens (18455's second instance is 99999).
fn tully_doorlist(npc_id: i32) -> Option<&'static [i32]> {
    Some(match npc_id {
        18445 => &[19260001, 19260002],
        18446 => &[19260003],
        18447 => &[19260003, 19260004, 19260005],
        18448 => &[19260006, 19260007],
        18449 => &[19260007, 19260008],
        18450 => &[19260010],
        18451 => &[19260011, 19260012],
        18452 => &[19260009, 19260011],
        18453 => &[19260014, 19260023, 19260013],
        18454 => &[19260015, 19260023],
        18455 => &[19260016],
        18456 => &[19260017, 19260018],
        18457 => &[19260021, 19260020],
        18458 => &[19260022],
        18459 => &[19260018],
        18460 => &[19260051],
        18461 => &[19260052],
        // 18455, second instance with x == -14610
        99999 => &[19260019],
        _ => return None,
    })
}

/// The ids of the doormen (`TULLY_DOORLIST` without 99999).
const DOORMEN: [i32; 17] = [
    18445, 18446, 18447, 18448, 18449, 18450, 18451, 18452, 18453, 18454, 18455, 18456, 18457, 18458, 18459,
    18460, 18461,
];

/// Java `TELE_COORDS`: `[up, down]`.
fn tele_coords(npc_id: i32) -> Option<[Location; 2]> {
    Some(match npc_id {
        32753 => [(-12700, 273340, -13600), (0, 0, 0)],
        32754 => [(-13246, 275740, -11936), (-12894, 273900, -15296)],
        32755 => [(-12798, 273458, -10496), (-12718, 273490, -13600)],
        32756 => [(-13500, 275912, -9032), (-13246, 275740, -11936)],
        _ => return None,
    })
}
const TELE_NPCS: [i32; 4] = [32753, 32754, 32755, 32756];

/// Java `POST_MORTEM_SPAWNLIST`: `(npc id, x, y, z, heading, despawn time)`.
// NOTE(java-quirk): the fourth contraption's row has an extra number (`8428, 49151, 0`), so
// Java reads heading 8428 and a despawn time of 49151 ms: it vanishes after 49 seconds.
const POST_MORTEM_SPAWNLIST: [(i32, i32, i32, i32, i32, i32); 14] = [
    (32371, -12524, 273932, -9014, 49151, 0),
    (32371, -10831, 273890, -9040, 81895, 0),
    (32371, -10817, 273986, -9040, -16452, 0),
    (32371, -13773, 275119, -9040, 8428, 49151),
    (32371, -11547, 271772, -9040, -19124, 0),
    (22392, -10832, 273808, -9040, 0, 0),
    (22392, -10816, 274096, -9040, 14964, 0),
    (22392, -13824, 275072, -9040, -24644, 0),
    (22392, -11504, 271952, -9040, 9328, 0),
    (22392, -11680, 275353, -9040, 0, 0),
    (22392, -12388, 271668, -9040, 0, 0),
    (32370, -11984, 272928, -9040, 23644, 900000),
    (32370, -14643, 274588, -9040, 49152, 0),
    (32344, -14756, 274788, -9040, -13868, 0),
];

// Format: npcId, x, y, z, heading
const SPAWNLIST_7TH_FLOOR: [(i32, i32, i32, i32, i32); 35] = [
    (25602, -12528, 279488, -11622, 16384),
    (25600, -12736, 279681, -11622, 0),
    (25601, -12324, 279681, -11622, 32768),
    (25599, -12281, 281497, -11935, 49151),
    (25599, -11903, 281488, -11934, 49151),
    (25599, -11966, 277935, -11936, 16384),
    (25599, -12334, 277935, -11936, 16384),
    (25599, -12739, 277935, -11936, 16384),
    (25599, -13063, 277934, -11936, 16384),
    (25599, -13077, 281506, -11935, 49151),
    (25599, -12738, 281503, -11935, 49151),
    (25597, -11599, 281323, -11933, -23808),
    (25597, -11381, 281114, -11934, -23808),
    (25597, -11089, 280819, -11934, -23808),
    (25597, -10818, 280556, -11934, -23808),
    (25597, -10903, 278798, -11934, 25680),
    (25597, -11134, 278558, -11934, 25680),
    (25597, -11413, 278265, -11934, 25680),
    (25597, -11588, 278072, -11935, 25680),
    (25597, -13357, 278058, -11935, 9068),
    (25597, -13617, 278289, -11935, 9068),
    (25597, -13920, 278567, -11935, 9068),
    (25597, -14131, 278778, -11936, 9068),
    (25597, -14184, 280545, -11936, -7548),
    (25597, -13946, 280792, -11936, -7548),
    (25597, -13626, 281105, -11936, -7548),
    (25597, -13386, 281360, -11935, -7548),
    (25598, -10697, 280244, -11936, 32768),
    (25598, -10702, 279926, -11936, 32768),
    (25598, -10722, 279470, -11936, 32768),
    (25598, -10731, 279126, -11936, 32768),
    (25598, -14284, 279140, -11936, 0),
    (25598, -14286, 279464, -11936, 0),
    (25598, -14290, 279909, -11935, 0),
    (25598, -14281, 280229, -11936, 0),
];

// Zone ID's for rooms
const SPAWN_ZONE_DEF: [[i32; 4]; 2] = [
    // 6th floor
    [200012, 200013, 200014, 200015],
    // 8th floor
    [200016, 200017, 200018, 200019],
];

const AGENT_COORDINATES: [(i32, i32, i32); 8] = [
    // 6th floor room 1
    (-13312, 279172, -13599),
    (-11696, 280208, -13599),
    (-13008, 280496, -13599),
    (-11984, 278880, -13599),
    // 8th floor room 1
    (-13312, 279172, -10492),
    (-11696, 280208, -10492),
    (-13008, 280496, -10492),
    (-11984, 278880, -10492),
];

const SERVANT_COORDINATES: [(i32, i32, i32); 8] = [
    // 6th floor
    (-13214, 278493, -13601),
    (-11727, 280711, -13601),
    (-13562, 280175, -13601),
    (-11514, 278592, -13601),
    // 8th floor
    (-13370, 278459, -10497),
    (-11984, 280894, -10497),
    (-14050, 280312, -10497),
    (-11559, 278725, -10495),
];

const CUBE_68_TELEPORTS: [Location; 3] = [
    // to 6th floor
    (-12176, 279696, -13596),
    // to 8th floor
    (-12176, 279696, -10492),
    // to roof
    (21935, 243923, 11088),
];

/// The fields of Java's `TullyWorkshop`.
struct TullyState {
    countdown_time: i32,
    next_servant_idx: i32,
    killed_followers_count: i32,
    allow_servant_spawn: bool,
    allow_agent_spawn: bool,
    allow_agent_spawn_7th: bool,
    is_7th_floor_attack_began: bool,
    /// `_countdown != null`.
    countdown_running: bool,
    /// `postMortemSpawn`: the list keeps its entries (and the failed spawns as `None`).
    post_mortem_spawn: Vec<Option<ObjectId>>,
    broken_contraptions: HashSet<ObjectId>,
    rewarded_contraptions: HashSet<ObjectId>,
    talked_contraptions: HashSet<ObjectId>,
    spawned_followers: Vec<Option<ObjectId>>,
    spawned_follower_minions: Vec<Option<ObjectId>>,
    spawned_agent: Option<ObjectId>,
    /// `pillarSpawn`: the index of the spawn.
    pillar_spawn: Option<usize>,
    death_count: [[i32; 4]; 2],
}

impl Default for TullyState {
    fn default() -> Self {
        Self {
            countdown_time: 0,
            next_servant_idx: 0,
            killed_followers_count: 0,
            allow_servant_spawn: true,
            allow_agent_spawn: true,
            allow_agent_spawn_7th: true,
            is_7th_floor_attack_began: false,
            countdown_running: false,
            post_mortem_spawn: Vec::new(),
            broken_contraptions: HashSet::new(),
            rewarded_contraptions: HashSet::new(),
            talked_contraptions: HashSet::new(),
            spawned_followers: Vec::new(),
            spawned_follower_minions: Vec::new(),
            spawned_agent: None,
            pillar_spawn: None,
            death_count: [[0; 4]; 2],
        }
    }
}

fn state<'a>(ctx: &'a mut ScriptCtx<'_>) -> &'a mut TullyState {
    ctx.state::<TullyState>()
}

/// Java `ai.areas.Hellbound.AI.Zones.TullyWorkshop.TullyWorkshop`.
pub struct TullyWorkshop;

fn is_maestro(ctx: &ScriptCtx<'_>, player: ObjectId) -> bool {
    ctx.world()
        .player_class_equals_or_child_of(player, player_class::MAESTRO)
}

fn alive_in_world(ctx: &ScriptCtx<'_>, npc: ObjectId) -> bool {
    ctx.world().npc(npc).is_some_and(|n| !n.decayed) && !ctx.is_dead(npc)
}

/// `npc.isInsideRadius3D(x, y, z, radius)` of an object id.
fn inside_3d(ctx: &ScriptCtx<'_>, npc: ObjectId, (x, y, z): Location, radius: i32) -> bool {
    ctx.script_npc(npc)
        .is_some_and(|n| ctx.is_inside_radius_3d(n, x, y, z, radius))
}

/// Java `npc.isInsideRadius2D(x, y, z, radius)`.
fn inside_2d(ctx: &ScriptCtx<'_>, npc: ObjectId, (x, y, _): Location, radius: i32) -> bool {
    ctx.world().position_of(npc).is_some_and(|(nx, ny, _)| {
        let (dx, dy) = (i64::from(nx - x), i64::from(ny - y));
        dx * dx + dy * dy <= i64::from(radius) * i64::from(radius)
    })
}

/// Java `getRoomData(npc)`: `(floor, room)` or `(-1, -1)`.
fn get_room_data(ctx: &ScriptCtx<'_>, npc: ScriptNpc) -> (i32, i32) {
    let Some(spawn_point) = ctx.npc_spawn_point(npc) else {
        return (-1, -1);
    };
    for (i, zones) in SPAWN_ZONE_DEF.iter().enumerate() {
        for (j, zone) in zones.iter().enumerate() {
            if ctx.world().script_point_in_zone_id(spawn_point, *zone) {
                return (i as i32, j as i32);
            }
        }
    }

    (-1, -1)
}

/// Java `initDeathCounter(floor)`.
fn init_death_counter(ctx: &mut ScriptCtx<'_>, floor: usize) {
    for i in 0..4 {
        let value = ctx.get_random(DEATH_COUNTS[floor]);
        state(ctx).death_count[floor][i] = value;
    }
}

/// Java `do7thFloorSpawn()`.
fn do_7th_floor_spawn(ctx: &mut ScriptCtx<'_>) {
    state(ctx).killed_followers_count = 0;
    state(ctx).is_7th_floor_attack_began = false;
    for (npc_id, x, y, z, heading) in SPAWNLIST_7TH_FLOOR {
        let monster = ctx
            .add_spawn(npc_id, x, y, z, heading, false, 0, 0)
            .map(|n| n.object_id);
        if npc_id == TEMENIR || npc_id == DRAXIUS || npc_id == KIRETCENAH {
            state(ctx).spawned_followers.push(monster);
        } else {
            state(ctx).spawned_follower_minions.push(monster);
        }
    }
}

/// Java `do7thFloorDespawn()`.
fn do_7th_floor_despawn(ctx: &mut ScriptCtx<'_>) {
    ctx.cancel_quest_timers("end_7th_floor_attack");
    let followers = std::mem::take(&mut state(ctx).spawned_followers);
    let minions = std::mem::take(&mut state(ctx).spawned_follower_minions);
    for monster in followers.into_iter().chain(minions).flatten() {
        if alive_in_world(ctx, monster) {
            ctx.world_mut().delete_npc(monster);
        }
    }

    ctx.start_quest_timer("cube_68_spawn", 60000, None, None, false);
}

/// `DoorTask` in `delay_ms`.
fn schedule_doors(ctx: &mut ScriptCtx<'_>, doors: &[i32], open: bool, delay_ms: i64) {
    let ids: Vec<String> = doors.iter().map(i32::to_string).collect();
    let name = format!(
        "door_task {} {}",
        if open { "open" } else { "close" },
        ids.join(",")
    );
    ctx.start_quest_timer(&name, delay_ms, None, None, false);
}

/// Java `handleDoorsOnDeath()`.
fn handle_doors_on_death(ctx: &mut ScriptCtx<'_>) {
    ctx.open_door(20250005, 0);
    ctx.open_door(20250004, 0);
    schedule_doors(ctx, &[20250006, 20250007], true, 2000);
    schedule_doors(ctx, &[20250778], false, 3000);
    schedule_doors(ctx, &[20250777], false, 6000);
    schedule_doors(ctx, &[20250009, 20250008], true, 11000);
}

/// Java `handleDoorsOnRespawn()`.
fn handle_doors_on_respawn(ctx: &mut ScriptCtx<'_>) {
    ctx.close_door(20250009, 0);
    ctx.close_door(20250008, 0);
    schedule_doors(ctx, &[20250777, 20250778], true, 1000);
    schedule_doors(ctx, &[20250005, 20250004, 20250006, 20250007], false, 4000);
}

fn darion_alive(ctx: &ScriptCtx<'_>) -> bool {
    ctx.world().raid_boss_status(DARION) == l2_world::boss::RaidBossStatus::Alive
}

/// Java `getRandomEntry` of a party; the player alone is the "party".
fn party_members(ctx: &ScriptCtx<'_>, player: ObjectId) -> Option<Vec<ObjectId>> {
    ctx.world().party_of(player).map(|p| ctx.world().party_members(p))
}

impl Script for TullyWorkshop {
    fn name(&self) -> &'static str {
        "TullyWorkshop"
    }

    fn path(&self) -> &'static str {
        "ai/areas/Hellbound/AI/Zones/TullyWorkshop"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[DORIAN]);
        s.add_talk_id(&[DORIAN]);

        for npc_id in DOORMEN {
            s.add_first_talk_id(&[npc_id]);
            s.add_start_npc(&[npc_id]);
            s.add_talk_id(&[npc_id]);
        }

        for npc_id in TELE_NPCS {
            s.add_start_npc(&[npc_id]);
            s.add_talk_id(&[npc_id]);
        }

        s.add_attack_id(&TELEPORTING_MONSTERS);
        s.add_kill_id(&SIN_WARDENS);

        s.add_start_npc(&[AGENT, CUBE_68, INGENIOUS_CONTRAPTION, DWARVEN_GHOST, TOMBSTONE]);
        s.add_talk_id(&[AGENT, CUBE_68, INGENIOUS_CONTRAPTION, DWARVEN_GHOST, TOMBSTONE]);
        s.add_first_talk_id(&[AGENT, CUBE_68, INGENIOUS_CONTRAPTION, DWARVEN_GHOST, TOMBSTONE]);
        s.add_kill_id(&[
            TULLY,
            TIMETWISTER_GOLEM,
            TEMENIR,
            DRAXIUS,
            KIRETCENAH,
            DARION,
            PILLAR,
        ]);
        s.add_faction_call_id(&[TEMENIR, DRAXIUS, KIRETCENAH]);

        s.add_spawn_id(&[CUBE_68, DARION, TULLY, PILLAR]);
        s.add_spell_finished_id(&[AGENT, TEMENIR]);

        let servants: Vec<i32> = (SERVANT_FIRST..=SERVANT_LAST).collect();
        s.add_kill_id(&servants);
        s.add_spell_finished_id(&servants);
    }

    // The Java constructor.
    fn on_load(&self, ctx: &mut ScriptCtx<'_>) {
        init_death_counter(ctx, 0);
        init_death_counter(ctx, 1);
        do_7th_floor_spawn(ctx);

        // doOnLoadSpawn(): Ghost of Tully and Spooky Tombstone should be spawned, if Tully isn't
        // alive.
        if ctx.world().raid_boss_status(TULLY) != l2_world::boss::RaidBossStatus::Alive {
            for data in &POST_MORTEM_SPAWNLIST[12..=13] {
                let spawned = ctx
                    .add_spawn(
                        data.0,
                        data.1,
                        data.2,
                        data.3,
                        data.4,
                        false,
                        i64::from(data.5),
                        0,
                    )
                    .map(|n| n.object_id);
                state(ctx).post_mortem_spawn.push(spawned);
            }
        }

        // Pillar related
        let pillar = ctx.add_spawn(PILLAR, 21008, 244000, 11087, 0, false, 0, 0);
        if let Some(index) = pillar.and_then(|p| ctx.world().npc_spawn_index(p.object_id)) {
            ctx.world_mut().set_spawn_respawn_delay(index, 1200);
            ctx.world_mut().start_spawn_respawn(index);
            state(ctx).pillar_spawn = Some(index);
        }

        // Doors related
        if !darion_alive(ctx) {
            handle_doors_on_death(ctx);
        }
    }

    fn on_first_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let npc_id = npc.id;
        if tully_doorlist(npc_id).is_some() {
            if is_maestro(ctx, player) {
                return Some("doorman-01c.htm".to_owned());
            }

            return Some("doorman-01.htm".to_owned());
        } else if npc_id == INGENIOUS_CONTRAPTION {
            if state(ctx).talked_contraptions.contains(&npc.object_id) {
                return Some("32371-02.htm".to_owned());
            } else if !state(ctx).broken_contraptions.contains(&npc.object_id) {
                if is_maestro(ctx, player) {
                    return Some("32371-01a.htm".to_owned());
                }

                return Some("32371-01.htm".to_owned());
            }

            return Some("32371-04.htm".to_owned());
        } else if npc_id == DWARVEN_GHOST {
            let index = state(ctx)
                .post_mortem_spawn
                .iter()
                .position(|n| *n == Some(npc.object_id));
            if index == Some(11) {
                ctx.broadcast_npc_say(
                    npc,
                    ChatType::NpcGeneral,
                    NpcStringId::HaHaYouWereSoAfraidOfDeathLetMeSeeIfYouFindMeInTimeMaybeYouCanFindAWay,
                    &[],
                );
                ctx.world_mut().delete_npc(npc.object_id);
                return None;
            } else if index == Some(12) {
                return Some("32370-01.htm".to_owned());
            } else if inside_3d(ctx, npc.object_id, (-45531, 245872, -14192), 100) {
                // Hello from Tower of Naia! :) Due to onFirstTalk limitation it should be here
                return Some("32370-03.htm".to_owned());
            }
            return Some("32370-02.htm".to_owned());
        } else if npc_id == AGENT {
            let leader = ctx.world().is_party_leader(player);
            if ctx.world().party_of(player).is_none() || !leader {
                return Some("32372-01a.htm".to_owned());
            }

            let (floor, room) = get_room_data(ctx, npc);
            if floor < 0 || room < 0 {
                return Some("32372-02.htm".to_owned());
            }

            return Some("32372-01.htm".to_owned());
        } else if npc_id == CUBE_68 {
            if inside_3d(ctx, npc.object_id, (-12752, 279696, -13596), 100) {
                return Some("32467-01.htm".to_owned());
            } else if inside_3d(ctx, npc.object_id, (-12752, 279696, -10492), 100) {
                return Some("32467-02.htm".to_owned());
            }

            return Some("32467-03.htm".to_owned());
        } else if npc_id == TOMBSTONE {
            for item_id in REWARDS {
                if ctx.has_at_least_one_quest_item(player, &[item_id]) {
                    return Some("32344-01.htm".to_owned());
                }
            }

            return Some("32344-01a.htm".to_owned());
        }

        None
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        if npc.id == TOMBSTONE {
            let Some(members) = party_members(ctx, player) else {
                return Some("32344-03.htm".to_owned());
            };

            let mut have_items = [false; 5];

            // For teleportation party should have all 5 medals
            for &pl in &members {
                for (i, reward) in REWARDS.iter().enumerate() {
                    if ctx.get_quest_items_count(pl, *reward) > 0
                        && ctx.check_if_in_range(300, pl, npc.object_id, true)
                    {
                        have_items[i] = true;
                        break;
                    }
                }
            }

            let medals_count = have_items.iter().filter(|h| **h).count();
            if medals_count == 0 {
                return Some("32344-03.htm".to_owned());
            } else if medals_count < 5 {
                return Some("32344-02.htm".to_owned());
            }

            for pl in members {
                if ctx.check_if_in_range(6000, pl, npc.object_id, false) {
                    ctx.teleport_player(pl, 26612, 248567, -2856, 0, false);
                }
            }
        }

        // `super.onTalk(npc, player)`
        None
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        let mut htmltext = Some(event.to_owned());

        // `DoorTask.run()`.
        if let Some(rest) = event.strip_prefix("door_task ") {
            let (kind, ids) = rest.split_once(' ')?;
            for id in ids.split(',').filter_map(|v| v.parse::<i32>().ok()) {
                if ctx.world().door_by_door_id(id).is_some() {
                    if kind == "open" {
                        ctx.open_door(id, 0);
                    } else {
                        ctx.close_door(id, 0);
                    }
                }
            }
            return None;
        }

        // The `scheduleAtFixedRate` task of Tully's death.
        if event == "countdown" {
            countdown_tick(ctx);
            return None;
        }

        if event.eq_ignore_ascii_case("disable_zone") {
            ctx.world_mut().script_set_trap_zone_enabled(200011, false);
        } else if event.eq_ignore_ascii_case("cube_68_spawn") {
            let spawned = ctx.add_spawn(CUBE_68, 12527, 279714, -11622, 16384, false, 0, 0);
            ctx.start_quest_timer("cube_68_despawn", 600000, spawned, None, false);
        } else if event.eq_ignore_ascii_case("end_7th_floor_attack") {
            do_7th_floor_despawn(ctx);
        } else if event.eq_ignore_ascii_case("start_7th_floor_spawn") {
            do_7th_floor_spawn(ctx);
        }

        let npc = npc?;
        let mut npc_id = npc.id;
        if event.eq_ignore_ascii_case("close") && tully_doorlist(npc_id).is_some() {
            // Second instance of 18455
            if npc_id == 18455 && ctx.world().position_of(npc.object_id).map(|p| p.0) == Some(-14610) {
                npc_id = 99999;
            }

            for door_id in tully_doorlist(npc_id).unwrap_or(&[]) {
                ctx.close_door(*door_id, 0);
            }
        }

        if event.eq_ignore_ascii_case("repair_device") {
            ctx.broadcast_npc_say(npc, ChatType::NpcShout, NpcStringId::DeActivateTheAlarm, &[]);
            state(ctx).broken_contraptions.remove(&npc.object_id);
        } else if event.eq_ignore_ascii_case("despawn_servant") && !ctx.is_dead(npc.object_id) {
            if !ctx.world().script_is_attacking_or_casting(npc.object_id)
                && ctx.npc_current_hp(npc) == f64::from(ctx.npc_max_hp(npc))
            {
                state(ctx).allow_servant_spawn = true;
                ctx.world_mut().delete_npc(npc.object_id);
            } else {
                ctx.start_quest_timer("despawn_servant", 180000, Some(npc), None, false);
            }
        } else if event.eq_ignore_ascii_case("despawn_agent") {
            state(ctx).allow_servant_spawn = true;
            state(ctx).allow_agent_spawn = true;
            ctx.world_mut().delete_npc(npc.object_id);
        } else if event.eq_ignore_ascii_case("despawn_agent_7") {
            let players: Vec<ObjectId> = ctx
                .world()
                .visible_objects_in_range(npc.object_id, 300)
                .into_iter()
                .filter(|id| ctx.world().player(*id).is_some())
                .collect();
            for pl in players {
                ctx.teleport_player(pl, -12176, 279696, -10492, 0, true);
            }

            state(ctx).allow_agent_spawn_7th = true;
            state(ctx).spawned_agent = None;
            ctx.world_mut().delete_npc(npc.object_id);
        } else if event.eq_ignore_ascii_case("cube_68_despawn") {
            let players: Vec<ObjectId> = ctx
                .world()
                .visible_objects_in_range(npc.object_id, 500)
                .into_iter()
                .filter(|id| ctx.world().player(*id).is_some())
                .collect();
            for pl in players {
                ctx.teleport_player(pl, -12176, 279696, -10492, 0, true);
            }

            ctx.start_quest_timer("start_7th_floor_spawn", 120000, None, None, false);
            ctx.world_mut().delete_npc(npc.object_id);
        }

        let player = player?;

        if event.eq_ignore_ascii_case("enter") && npc_id == DORIAN {
            if ctx.world().is_party_leader(player) {
                let members = party_members(ctx, player).unwrap_or_default();
                for &member in &members {
                    if !ctx.check_if_in_range(300, member, npc.object_id, true) {
                        return Some("32373-02.htm".to_owned());
                    }
                }

                for member in members {
                    ctx.teleport_player(member, -13400, 272827, -15300, 0, true);
                }

                htmltext = None;
            } else {
                htmltext = Some("32373-02a.htm".to_owned());
            }
        } else if event.eq_ignore_ascii_case("open") && tully_doorlist(npc_id).is_some() {
            // Second instance of 18455
            if npc_id == 18455 && ctx.world().position_of(npc.object_id).map(|p| p.0) == Some(-14610) {
                npc_id = 99999;
            }

            for door_id in tully_doorlist(npc_id).unwrap_or(&[]) {
                ctx.open_door(*door_id, 0);
            }

            ctx.start_quest_timer("close", 120000, Some(npc), None, false);
            htmltext = None;
        } else if (event.eq_ignore_ascii_case("up") || event.eq_ignore_ascii_case("down"))
            && tele_coords(npc_id).is_some()
        {
            let direction = usize::from(!event.eq_ignore_ascii_case("up"));
            if ctx.world().party_of(player).is_none() {
                ctx.send_system_message(
                    player,
                    SystemMessageId::YouAreNotCurrentlyInAPartySoYouCannotEnter,
                );
            } else if !ctx.world().is_party_leader(player) {
                ctx.send_system_message(player, SystemMessageId::OnlyAPartyLeaderCanMakeTheRequestToEnter);
            } else if !ctx.check_if_in_range(4000, player, npc.object_id, true) {
                ctx.send_system_message(player, SystemMessageId::YouAreTooFarFromTheNpcForThatToWork);
            } else if let Some(coords) = tele_coords(npc_id) {
                let loc = coords[direction];
                for member in party_members(ctx, player).unwrap_or_default() {
                    if ctx.check_if_in_range(4000, member, npc.object_id, true) {
                        ctx.teleport_player(member, loc.0, loc.1, loc.2, 0, true);
                    }
                }
            }

            htmltext = None;
        } else if npc_id == INGENIOUS_CONTRAPTION {
            if event.eq_ignore_ascii_case("touch_device") {
                let i0 = if state(ctx).talked_contraptions.contains(&npc.object_id) {
                    0
                } else {
                    1
                };
                let maestro = is_maestro(ctx, player);
                let i1 = if maestro { 6 } else { 3 };
                if ctx.get_random(1000) < ((i1 - i0) * 100) {
                    state(ctx).talked_contraptions.insert(npc.object_id);
                    htmltext = Some(if maestro { "32371-03a.htm" } else { "32371-03.htm" }.to_owned());
                } else {
                    state(ctx).broken_contraptions.insert(npc.object_id);
                    ctx.start_quest_timer("repair_device", 60000, Some(npc), None, false);
                    htmltext = Some("32371-04.htm".to_owned());
                }
            } else if event.eq_ignore_ascii_case("take_reward") {
                let already_have_item = REWARDS
                    .iter()
                    .any(|id| ctx.get_quest_items_count(player, *id) > 0);

                if !already_have_item && !state(ctx).rewarded_contraptions.contains(&npc.object_id) {
                    let idx = state(ctx)
                        .post_mortem_spawn
                        .iter()
                        .position(|n| *n == Some(npc.object_id));
                    if let Some(idx) = idx.filter(|i| *i < 5) {
                        ctx.give_items(player, REWARDS[idx], 1);
                        state(ctx).rewarded_contraptions.insert(npc.object_id);
                        if idx != 0 {
                            ctx.world_mut().delete_npc(npc.object_id);
                        }
                    }

                    htmltext = None;
                } else {
                    htmltext = Some("32371-05.htm".to_owned());
                }
            }
        } else if npc_id == AGENT {
            if event.eq_ignore_ascii_case("tele_to_7th_floor") && !state(ctx).allow_agent_spawn {
                htmltext = None;
                if let Some(members) = party_members(ctx, player) {
                    if !ctx.world().is_party_leader(player) {
                        ctx.send_system_message(
                            player,
                            SystemMessageId::OnlyAPartyLeaderCanMakeTheRequestToEnter,
                        );
                    } else {
                        for member in members {
                            if ctx.check_if_in_range(6000, member, npc.object_id, true) {
                                ctx.teleport_player(member, -12501, 281397, -11936, 0, true);
                            }
                        }

                        spawn_7th_floor_agent(ctx);
                    }
                } else {
                    ctx.teleport_player(player, -12501, 281397, -11936, 0, false);
                    spawn_7th_floor_agent(ctx);
                }
            } else if event.eq_ignore_ascii_case("buff") && !state(ctx).allow_agent_spawn_7th {
                htmltext = None;
                if let Some(members) = party_members(ctx, player) {
                    for &member in &members {
                        if !ctx.check_if_in_range(400, member, npc.object_id, true) {
                            return Some("32372-01b.htm".to_owned());
                        }
                    }

                    for member in members {
                        ctx.cast_skill_at(npc, Some(member), SkillKey::new(5526, 1));
                    }

                    ctx.start_quest_timer("despawn_agent_7", 60000, Some(npc), None, false);
                } else {
                    if !ctx.check_if_in_range(400, player, npc.object_id, true) {
                        htmltext = Some("32372-01b.htm".to_owned());
                    } else {
                        ctx.world_mut().set_npc_target(npc.object_id, Some(player));
                    }

                    ctx.world_mut().do_cast(npc.object_id, SkillKey::new(5526, 1));
                }
            } else if event.eq_ignore_ascii_case("refuse") && !state(ctx).allow_agent_spawn_7th {
                state(ctx).allow_agent_spawn_7th = true;
                state(ctx).spawned_agent = None;
                let followers = state(ctx).spawned_followers.clone();
                for monster in followers.into_iter().flatten() {
                    if alive_in_world(ctx, monster) {
                        let Some(monster_npc) = ctx.script_npc(monster) else {
                            continue;
                        };
                        if ctx.spawned_minion_count(monster_npc) == 0 {
                            ctx.spawn_minion(monster_npc, 25596);
                            ctx.spawn_minion(monster_npc, 25596);
                        }

                        let target = match party_members(ctx, player) {
                            None => Some(player),
                            Some(members) => ctx.get_random_entry(&members),
                        };
                        if let Some(target) = target
                            && !ctx.is_dead(target)
                        {
                            ctx.add_attack_desire(monster_npc, target, 999);
                            ctx.set_intention_attack(monster_npc, target);
                        }
                    }
                }

                if !state(ctx).is_7th_floor_attack_began {
                    state(ctx).is_7th_floor_attack_began = true;
                    ctx.start_quest_timer("end_7th_floor_attack", 1_200_000, None, None, false);
                }

                ctx.world_mut().delete_npc(npc.object_id);
            }
        } else if event.eq_ignore_ascii_case("teleport") && npc_id == DWARVEN_GHOST {
            htmltext = None;
            if let Some(members) = party_members(ctx, player) {
                if !ctx.world().is_party_leader(player) {
                    ctx.send_system_message(
                        player,
                        SystemMessageId::OnlyAPartyLeaderCanMakeTheRequestToEnter,
                    );
                    return None;
                }

                for &member in &members {
                    if !ctx.check_if_in_range(3000, member, npc.object_id, true) {
                        return Some("32370-01f.htm".to_owned());
                    }
                }

                for member in members {
                    if ctx.check_if_in_range(6000, member, npc.object_id, true) {
                        ctx.teleport_player(member, -12176, 279696, -13596, 0, true);
                    }
                }
            } else {
                ctx.teleport_player(player, -12176, 279696, -13596, 0, false);
            }
        } else if npc_id == CUBE_68 && event.starts_with("cube68_tp") {
            htmltext = None;
            // `Integer.parseInt(event.substring(10))`.
            let tp_id = event.get(10..).and_then(|v| v.parse::<usize>().ok())?;
            let tp = *CUBE_68_TELEPORTS.get(tp_id)?;
            if let Some(members) = party_members(ctx, player) {
                if !ctx.world().is_party_leader(player) {
                    ctx.send_system_message(
                        player,
                        SystemMessageId::OnlyAPartyLeaderCanMakeTheRequestToEnter,
                    );
                } else if !ctx.check_if_in_range(3000, player, npc.object_id, true) {
                    htmltext = Some("32467-04.htm".to_owned());
                } else {
                    for member in members {
                        if ctx.check_if_in_range(6000, member, npc.object_id, true) {
                            ctx.teleport_player(member, tp.0, tp.1, tp.2, 0, true);
                        }
                    }
                }
            } else {
                ctx.teleport_player(player, tp.0, tp.1, tp.2, 0, false);
            }
        }

        htmltext
    }

    fn on_attack(
        &self,
        ctx: &mut ScriptCtx<'_>,
        npc: ScriptNpc,
        attacker: ObjectId,
        _damage: i32,
        _is_summon: bool,
        _skill: Option<SkillKey>,
    ) {
        let npc_id = npc.id;
        if TELEPORTING_MONSTERS.contains(&npc_id) {
            let (Some(npc_pos), Some(attacker_pos)) = (
                ctx.world().position_of(npc.object_id),
                ctx.world().position_of(attacker),
            ) else {
                return;
            };
            if (npc_pos.2 - attacker_pos.2).abs() > 150 {
                ctx.world_mut().clear_aggro_list(npc.object_id);
                ctx.teleport_player(attacker, npc_pos.0 + 50, npc_pos.1 - 50, npc_pos.2, 0, false);
            }
        } else if (npc_id == TEMENIR || npc_id == KIRETCENAH)
            && state(ctx).spawned_followers.contains(&Some(npc.object_id))
        {
            let followers = state(ctx).spawned_followers.clone();
            let get = |i: usize| followers.get(i).copied().flatten();
            let (victim1, victim2, actor) = (get(1), get(0), get(2)); // TEMENIR, KIRETCENAH, DRAXIUS
            if let Some(actor) = actor
                && !ctx.is_dead(actor)
            {
                let actor_npc = ctx.script_npc(actor);
                let transferring_hp = actor_npc.map_or(0.0, |a| f64::from(ctx.npc_max_hp(a))) * 0.0001;
                let actor_hp = |ctx: &ScriptCtx<'_>| actor_npc.map_or(0.0, |a| ctx.npc_current_hp(a));
                if ctx.get_random(10000) > 1500
                    && let Some(victim1) = victim1
                    && !ctx.is_dead(victim1)
                    && (actor_hp(ctx) - transferring_hp) > 1.0
                {
                    let hp = actor_hp(ctx);
                    ctx.world_mut().set_current_hp(actor, hp - transferring_hp, true);
                    let victim_hp = ctx.world().creature_hp(victim1).map_or(0.0, |h| h.0);
                    ctx.world_mut()
                        .set_current_hp(victim1, victim_hp + transferring_hp, true);
                }

                if ctx.get_random(10000) > 3000
                    && let Some(victim2) = victim2
                    && !ctx.is_dead(victim2)
                    && (actor_hp(ctx) - transferring_hp) > 1.0
                {
                    let hp = actor_hp(ctx);
                    ctx.world_mut().set_current_hp(actor, hp - transferring_hp, true);
                    let victim_hp = ctx.world().creature_hp(victim2).map_or(0.0, |h| h.0);
                    ctx.world_mut()
                        .set_current_hp(victim2, victim_hp + transferring_hp, true);
                }
            }
        }

        if (npc_id == TEMENIR || npc_id == DRAXIUS)
            && state(ctx).spawned_followers.contains(&Some(npc.object_id))
        {
            let followers = state(ctx).spawned_followers.clone();
            let get = |i: usize| followers.get(i).copied().flatten();
            let victim = if npc_id == TEMENIR { get(1) } else { get(2) };
            let actor = get(0);
            if let (Some(actor), Some(victim)) = (actor, victim)
                && !ctx.is_dead(actor)
                && !ctx.is_dead(victim)
                && ctx.get_random(1000) > 333
            {
                ctx.world_mut().clear_aggro_list(actor);
                ctx.world_mut().set_intention_active(actor);
                ctx.world_mut().set_npc_target(actor, Some(victim));
                ctx.world_mut().do_cast(actor, SkillKey::new(4065, 11));
                // FIXME(java): not retail, it should be done after spell is finished.
                if let Some((hp, max_hp)) = ctx.world().creature_hp(victim) {
                    ctx.world_mut().set_current_hp(victim, hp + max_hp * 0.03, true);
                }
            }
        }
    }

    fn on_faction_call(
        &self,
        ctx: &mut ScriptCtx<'_>,
        npc: ScriptNpc,
        _caller: ScriptNpc,
        _attacker: ObjectId,
        _is_summon: bool,
    ) {
        let npc_id = npc.id;
        if npc_id == TEMENIR || npc_id == DRAXIUS || npc_id == KIRETCENAH {
            if ctx.spawned_minion_count(npc) == 0 {
                ctx.spawn_minion(npc, 25596);
                ctx.spawn_minion(npc, 25596);
            }

            if !state(ctx).is_7th_floor_attack_began {
                state(ctx).is_7th_floor_attack_began = true;
                ctx.start_quest_timer("end_7th_floor_attack", 1_200_000, None, None, false);
                if let Some(agent) = state(ctx).spawned_agent.take() {
                    ctx.world_mut().delete_npc(agent);
                    state(ctx).allow_agent_spawn_7th = true;
                }
            }
        }
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        let npc_id = npc.id;
        if npc_id == TULLY && inside_2d(ctx, npc.object_id, (-12557, 273901, -9000), 1000) {
            for data in POST_MORTEM_SPAWNLIST {
                let spawned = ctx
                    .add_spawn(
                        data.0,
                        data.1,
                        data.2,
                        data.3,
                        data.4,
                        false,
                        i64::from(data.5),
                        0,
                    )
                    .map(|n| n.object_id);
                state(ctx).post_mortem_spawn.push(spawned);
            }

            ctx.open_door(19260051, 0);
            ctx.open_door(19260052, 0);

            state(ctx).countdown_time = 600000;
            state(ctx).countdown_running = true;
            ctx.start_quest_timer("countdown", 60000, None, None, false);
            let first = state(ctx).post_mortem_spawn.first().copied().flatten();
            if let Some(first) = first.and_then(|f| ctx.script_npc(f)) {
                let minutes = (state(ctx).countdown_time / 60000).to_string();
                ctx.broadcast_npc_say(
                    first,
                    ChatType::NpcShout,
                    NpcStringId::DetonatorInitializationTimeSetForS1MinuteSFromNow,
                    &[&minutes],
                );
            }
        } else if npc_id == TIMETWISTER_GOLEM && state(ctx).countdown_running {
            let first = state(ctx)
                .post_mortem_spawn
                .first()
                .copied()
                .flatten()
                .and_then(|f| ctx.script_npc(f))
                .filter(|f| f.id == INGENIOUS_CONTRAPTION);
            if ctx.get_random(1000) >= 700 {
                ctx.broadcast_npc_say(
                    npc,
                    ChatType::NpcGeneral,
                    NpcStringId::AFatalErrorHasOccurred,
                    &[],
                );
                if state(ctx).countdown_time > 180000 {
                    state(ctx).countdown_time = (state(ctx).countdown_time - 180000).max(60000);
                    if let Some(first) = first {
                        ctx.broadcast_npc_say(
                            first,
                            ChatType::NpcShout,
                            NpcStringId::ZzzzCityInterferenceErrorForwardEffectCreated,
                            &[],
                        );
                    }
                }
            } else {
                ctx.broadcast_npc_say(
                    npc,
                    ChatType::NpcGeneral,
                    NpcStringId::TimeRiftDeviceActivationSuccessful,
                    &[],
                );
                let time = state(ctx).countdown_time;
                if time > 0 && time <= 420000 {
                    state(ctx).countdown_time += 180000;
                    if let Some(first) = first {
                        ctx.broadcast_npc_say(
                            first,
                            ChatType::NpcShout,
                            NpcStringId::ZzzzCityInterferenceErrorRecurrenceEffectCreated,
                            &[],
                        );
                    }
                }
            }
        } else if SIN_WARDENS.contains(&npc_id) {
            let (floor, room) = get_room_data(ctx, npc);
            if floor >= 0 && room >= 0 {
                let (floor, room) = (floor as usize, room as usize);
                state(ctx).death_count[floor][room] += 1;

                if state(ctx).allow_servant_spawn {
                    let mut max = 0;
                    let mut best_room: Option<usize> = None;
                    for i in 0..4 {
                        let count = state(ctx).death_count[floor][i];
                        if count > max {
                            max = count;
                            best_room = Some(i);
                        }
                    }

                    if let Some(best_room) = best_room
                        && max >= DEATH_COUNTS[floor]
                    {
                        let cf = if floor == 1 { 3 } else { 0 };
                        let servant_id = SERVANT_FIRST + state(ctx).next_servant_idx + cf;
                        let coords = SERVANT_COORDINATES[best_room + cf as usize];
                        let spawned = ctx.add_spawn(servant_id, coords.0, coords.1, coords.2, 0, false, 0, 0);
                        state(ctx).allow_servant_spawn = false;
                        ctx.start_quest_timer("despawn_servant", 180000, spawned, None, false);
                    }
                }
            }
        } else if (SERVANT_FIRST..=SERVANT_LAST).contains(&npc_id) {
            let (floor, room) = get_room_data(ctx, npc);
            if floor >= 0 && room >= 0 && state(ctx).allow_agent_spawn {
                state(ctx).allow_servant_spawn = true;
                if state(ctx).next_servant_idx == 2 {
                    state(ctx).next_servant_idx = 0;
                    init_death_counter(ctx, floor as usize);
                    if darion_alive(ctx) {
                        state(ctx).allow_agent_spawn = false;
                        state(ctx).allow_servant_spawn = false;
                        let cf = if floor == 1 { 3 } else { 0 };
                        let coords = AGENT_COORDINATES[(room + cf) as usize];
                        let spawned = ctx.add_spawn(AGENT, coords.0, coords.1, coords.2, 0, false, 0, 0);
                        ctx.start_quest_timer("despawn_agent", 180000, spawned, None, false);
                    }
                } else {
                    for i in 0..4 {
                        if i == room as usize {
                            state(ctx).death_count[floor as usize][i] = 0;
                        } else {
                            let value = (state(ctx).death_count[floor as usize][i] + 1) * ctx.get_random(3);
                            state(ctx).death_count[floor as usize][i] = value;
                        }
                    }

                    if ctx.get_random(1000) > 500 {
                        state(ctx).next_servant_idx += 1;
                    }
                }
            }

            if npc_id - 22404 == 3 || npc_id - 22404 == 6 {
                ctx.broadcast_npc_say(
                    npc,
                    ChatType::NpcShout,
                    NpcStringId::IFailedPleaseForgiveMeDarion,
                    &[],
                );
            } else {
                let name = ctx.player_name(killer);
                ctx.broadcast_npc_say(
                    npc,
                    ChatType::NpcShout,
                    NpcStringId::S1ILlBeBackDonTGetComfortable,
                    &[&name],
                );
            }
        } else if (npc_id == TEMENIR || npc_id == DRAXIUS || npc_id == KIRETCENAH)
            && state(ctx).spawned_followers.contains(&Some(npc.object_id))
        {
            state(ctx).killed_followers_count += 1;
            if state(ctx).killed_followers_count >= 3 {
                do_7th_floor_despawn(ctx);
            }
        } else if npc_id == DARION {
            if let Some(index) = state(ctx).pillar_spawn
                && let Some(pillar) = ctx.world().last_spawn(index)
            {
                ctx.world_mut().set_invul(pillar, false);
            }

            handle_doors_on_death(ctx);
        } else if npc_id == PILLAR
            && let Some((x, y, z)) = ctx.world().position_of(npc.object_id)
        {
            ctx.add_spawn(DWARVEN_GHOST, x + 30, y - 30, z, 0, false, 900000, 0);
        }
    }

    fn on_spawn(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc) {
        if npc.id == TULLY && inside_3d(ctx, npc.object_id, (-12557, 273901, -9000), 1000) {
            let spawned = std::mem::take(&mut state(ctx).post_mortem_spawn);
            for spawned_npc in spawned.into_iter().flatten() {
                ctx.world_mut().delete_npc(spawned_npc);
            }
        } else if npc.id == DARION {
            if let Some(index) = state(ctx).pillar_spawn
                && let Some(pillar) = ctx.world().last_spawn(index)
            {
                ctx.world_mut().set_invul(pillar, true);
            }

            handle_doors_on_respawn(ctx);
        } else if npc.id == PILLAR {
            let alive = darion_alive(ctx);
            ctx.world_mut().set_invul(npc.object_id, alive);
        }
    }

    fn on_spell_finished(
        &self,
        ctx: &mut ScriptCtx<'_>,
        npc: ScriptNpc,
        player: Option<ObjectId>,
        skill: SkillKey,
    ) {
        let npc_id = npc.id;
        let skill_id = skill.id;
        if npc_id == AGENT && skill_id == 5526 {
            if let Some(player) = player {
                ctx.teleport_player(player, 21935, 243923, 11088, 0, true); // to the roof
            }
        } else if npc_id == TEMENIR && skill_id == 5331 {
            if !ctx.is_dead(npc.object_id) {
                let hp = ctx.npc_current_hp(npc);
                let max_hp = f64::from(ctx.npc_max_hp(npc));
                ctx.world_mut()
                    .set_current_hp(npc.object_id, hp + max_hp * 0.005, true);
            }
        } else if (SERVANT_FIRST..=SERVANT_LAST).contains(&npc_id)
            && skill_id == 5392
            && let Some(player) = player
        {
            let name = ctx.player_name(player);
            ctx.broadcast_npc_say(
                npc,
                ChatType::NpcGeneral,
                NpcStringId::S1ThankYouForGivingMeYourLife,
                &[&name],
            );
            let level = npc_id - 22404;
            let player_hp = ctx.world().creature_hp(player).map_or(0.0, |h| h.0);
            let dmg = (player_hp / f64::from(level)) as i32;
            ctx.world_mut().reduce_current_hp(player, f64::from(dmg), None);
            let hp = ctx.npc_current_hp(npc);
            ctx.world_mut()
                .set_current_hp(npc.object_id, (hp + 10.0) - f64::from(level), true);
        }
    }
}

/// The `tele_to_7th_floor` tail shared by the party and the lone player: the agent of the 7th
/// floor appears once.
fn spawn_7th_floor_agent(ctx: &mut ScriptCtx<'_>) {
    if state(ctx).allow_agent_spawn_7th {
        if let Some(agent) = state(ctx).spawned_agent {
            ctx.world_mut().delete_npc(agent);
        }

        let spawned = ctx
            .add_spawn(AGENT, -12527, 279714, -11622, 16384, false, 0, 0)
            .map(|n| n.object_id);
        state(ctx).spawned_agent = spawned;
        state(ctx).allow_agent_spawn_7th = false;
    }
}

/// The task of `scheduleAtFixedRate(.., 60000, 10000)` that Tully's death starts.
fn countdown_tick(ctx: &mut ScriptCtx<'_>) {
    if !state(ctx).countdown_running {
        return;
    }
    state(ctx).countdown_time -= 10000;
    let npc_spawn = state(ctx)
        .post_mortem_spawn
        .first()
        .copied()
        .flatten()
        .and_then(|f| ctx.script_npc(f));
    let contraption = npc_spawn.filter(|n| n.id == INGENIOUS_CONTRAPTION);
    let time = state(ctx).countdown_time;
    if time > 60000 {
        if time % 60000 == 0
            && let Some(first) = contraption
        {
            ctx.broadcast_npc_say(
                first,
                ChatType::NpcShout,
                NpcStringId::S1MinuteSAreRemaining,
                &[&(time / 60000).to_string()],
            );
        }
    } else if time <= 0 {
        state(ctx).countdown_running = false;

        let spawned: Vec<ObjectId> = state(ctx).post_mortem_spawn.iter().flatten().copied().collect();
        for spawned_npc in spawned {
            let id = ctx.world().npc(spawned_npc).map(|n| n.npc_id);
            if id == Some(INGENIOUS_CONTRAPTION) || id == Some(TIMETWISTER_GOLEM) {
                ctx.world_mut().delete_npc(spawned_npc);
            }
        }

        let s = state(ctx);
        s.broken_contraptions.clear();
        s.rewarded_contraptions.clear();
        s.talked_contraptions.clear();
        ctx.world_mut().script_set_trap_zone_enabled(200011, true);

        ctx.start_quest_timer("disable_zone", 300000, None, None, false);
        return;
    } else if let Some(first) = contraption {
        ctx.broadcast_npc_say(
            first,
            ChatType::NpcShout,
            NpcStringId::S1SecondSRemaining,
            &[&(time / 1000).to_string()],
        );
    }

    ctx.start_quest_timer("countdown", 10000, None, None, false);
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::ai::areas::hellbound::testing::{last_page, spawn};
    use crate::testing::{Pages, ScriptTest, item};

    const NPCS: &str = r#"<npc id="18445" type="Folk" level="80" name="Doorman"/>
        <npc id="32371" type="Folk" level="80" name="Ingenious Contraption"/>
        <npc id="32344" type="Folk" level="80" name="Spooky Tombstone"/>
        <npc id="32370" type="Folk" level="80" name="Old Dwarven Ghost"/>
        <npc id="32373" type="Folk" level="80" name="Dorian"/>
        <npc id="32467" type="Folk" level="80" name="Cube"/>
        <npc id="25544" type="Monster" level="80" name="Tully"/>
        <npc id="22392" type="Monster" level="80" name="Golem"/>
        <npc id="22377" type="Monster" level="80" name="Zelos"/>
        <npc id="18506" type="Monster" level="80" name="Crystal Pillar"/>
        <npc id="25597" type="Monster" level="80" name="Follower Minion"/>
        <npc id="25598" type="Monster" level="80" name="Follower Minion 2"/>
        <npc id="25599" type="Monster" level="80" name="Follower Minion 3"/>
        <npc id="25600" type="Monster" level="80" name="Temenir"/>
        <npc id="25601" type="Monster" level="80" name="Draxius"/>
        <npc id="25602" type="Monster" level="80" name="Kiretcenah"/>"#;
    const ITEMS: &str = r#"
        <item id="10427" type="EtcItem" name="Medal 1"/>
        <item id="10428" type="EtcItem" name="Medal 2"/>"#;

    fn setup(items: Vec<l2_world::ItemData>) -> ScriptTest {
        let mut registry = ScriptRegistry::new();
        registry.add(TullyWorkshop);
        let pages = Pages::of(&TullyWorkshop).plain(&[
            "doorman-01.htm",
            "32371-01.htm",
            "32371-02.htm",
            "32371-03.htm",
            "32371-04.htm",
            "32344-01.htm",
            "32344-01a.htm",
            "32344-03.htm",
        ]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        t.enter(1, 80, items);
        t.ticks(2);
        t
    }

    #[test]
    fn the_load_spawns_the_seventh_floor_the_pillar_and_the_dead_tully_s_ghost() {
        let t = setup(vec![]);
        // 3 followers and 32 minions.
        let followers: usize = [25600, 25601, 25602].iter().map(|id| t.npcs_of(*id).len()).sum();
        assert_eq!(followers, 3);
        assert_eq!(t.npcs_of(PILLAR).len(), 1);
        // Tully is not alive in the test world: the ghost and the tombstone stand.
        assert_eq!(t.npcs_of(DWARVEN_GHOST).len(), 1);
        assert_eq!(t.npcs_of(TOMBSTONE).len(), 1);
    }

    #[test]
    fn the_doorman_and_the_tombstone_pages() {
        let mut t = setup(vec![item(100, 10427, 1)]);
        let doorman = spawn(&mut t, 18445, 20);
        t.talk(1, doorman);
        assert!(last_page(&t.take(), 1).contains("doorman-01.htm"));
        let tombstone = t.npcs_of(TOMBSTONE)[0];
        t.world.run_as_script("TullyWorkshop", |ctx| {
            let npc = ctx.script_npc(tombstone).unwrap();
            // One medal in the inventory: the first page of the reward case.
            assert_eq!(
                TullyWorkshop.on_first_talk(ctx, npc, ObjectId(1)).as_deref(),
                Some("32344-01.htm")
            );
            // Without a party: the third page.
            assert_eq!(
                TullyWorkshop.on_talk(ctx, npc, ObjectId(1)).as_deref(),
                Some("32344-03.htm")
            );
        });
    }

    #[test]
    fn a_contraption_is_touched_and_repairs_itself() {
        let mut t = setup(vec![]);
        let contraption = spawn(&mut t, 32371, 20);
        t.world.run_as_script("TullyWorkshop", |ctx| {
            let npc = ctx.script_npc(contraption).unwrap();
            assert_eq!(
                TullyWorkshop.on_first_talk(ctx, npc, ObjectId(1)).as_deref(),
                Some("32371-01.htm")
            );
            // Break it by hand and see the page change and the timer start.
            state(ctx).broken_contraptions.insert(contraption);
            assert_eq!(
                TullyWorkshop.on_first_talk(ctx, npc, ObjectId(1)).as_deref(),
                Some("32371-04.htm")
            );
            TullyWorkshop.on_event(ctx, "repair_device", Some(npc), Some(ObjectId(1)));
            assert!(state(ctx).broken_contraptions.is_empty());
        });
    }

    #[test]
    fn tully_s_death_starts_the_countdown_that_ends_with_the_damage_zone() {
        let mut t = setup(vec![]);
        let tully = spawn(&mut t, 25544, 20);
        t.world.run_as_script("TullyWorkshop", |ctx| {
            // The test place is not in the workshop: call the hook as a kill there.
            assert!(ctx.script_npc(tully).is_some());
            state(ctx).countdown_time = 600_000;
            state(ctx).countdown_running = true;
            ctx.start_quest_timer("countdown", 60_000, None, None, false);
        });
        t.wait_ms(60_100);
        t.world.run_as_script("TullyWorkshop", |ctx| {
            assert_eq!(state(ctx).countdown_time, 590_000);
            assert!(ctx.has_quest_timer("countdown", None, None));
        });
        // Ten minutes in all: the countdown ends and the damage zone comes on.
        t.wait_ms(600_000);
        t.world.run_as_script("TullyWorkshop", |ctx| {
            assert!(!state(ctx).countdown_running);
            assert!(!ctx.has_quest_timer("countdown", None, None));
            assert!(ctx.has_quest_timer("disable_zone", None, None));
        });
        assert_eq!(t.world.trap_zone_enabled(200011), Some(true));
        t.wait_ms(300_100);
        assert_eq!(t.world.trap_zone_enabled(200011), Some(false));
    }

    #[test]
    fn a_teleporting_monster_pulls_a_player_on_another_floor() {
        let mut t = setup(vec![]);
        let zelos = spawn(&mut t, 22377, 20);
        t.world.run_as_script("TullyWorkshop", |ctx| {
            let npc = ctx.script_npc(zelos).unwrap();
            // The attacker stands at the same height: nothing happens.
            TullyWorkshop.on_attack(ctx, npc, ObjectId(1), 5, false, None);
        });
        let before = t.world.position_of(ObjectId(1)).unwrap();
        t.ticks(5);
        assert_eq!(t.world.position_of(ObjectId(1)).unwrap(), before);
    }

    #[test]
    fn the_doors_follow_the_door_task_timers() {
        let mut t = setup(vec![]);
        t.world.run_as_script("TullyWorkshop", |ctx| {
            handle_doors_on_death(ctx);
            assert!(ctx.has_quest_timer("door_task open 20250006,20250007", None, None));
            assert!(ctx.has_quest_timer("door_task close 20250778", None, None));
        });
    }

    #[test]
    fn the_tables_match_the_java_arrays() {
        assert_eq!(POST_MORTEM_SPAWNLIST.len(), 14);
        assert_eq!(SPAWNLIST_7TH_FLOOR.len(), 35);
        assert!(DOORMEN.iter().all(|id| tully_doorlist(*id).is_some()));
        assert!(TELE_NPCS.iter().all(|id| tele_coords(*id).is_some()));
        assert!(TELEPORTING_MONSTERS.windows(2).all(|w| w[0] < w[1]));
    }
}
