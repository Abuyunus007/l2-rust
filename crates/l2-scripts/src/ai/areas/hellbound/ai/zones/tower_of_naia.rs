//! The Tower of Naia: the Lock and its controller, the twelve rooms of the climb, and the spore
//! challenge before Mutated Elpy's chamber.
// Java: dist/game/data/scripts/ai/areas/Hellbound/AI/Zones/TowerOfNaia/TowerOfNaia.java
//
// # Left out
// - `ZoneManager.getZoneById(200100 / 200101).setEnabled(..)`: the two effect zones of the core
//   (`18_25_core_naia`, `18_25_naia_pillar_green`) cannot be switched; the world has no
//   enable switch for an `EffectZone` (only the castle trap zones have one).

use std::collections::HashMap;

use l2_protocol_game::NpcStringId;
use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::ChatType;
use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup, SkillKey};

// Challenge states
const STATE_SPORE_CHALLENGE_IN_PROGRESS: i32 = 1;
const STATE_SPORE_CHALLENGE_SUCCESSFULL: i32 = 2;
const STATE_SPORE_IDLE_TOO_LONG: i32 = 3;

// Some constants
const SELF_DESPAWN_LIMIT: i32 = 600; // Challenge discontinues after 600 self-despawns by timer
const ELEMENT_INDEX_LIMIT: i32 = 120; // Epidos spawns when index reaches 120 points

const LOCK: i32 = 18491;
const CONTROLLER: i32 = 18492;
const ROOM_MANAGER_FIRST: i32 = 18494;
const ROOM_MANAGER_LAST: i32 = 18505;
const MUTATED_ELPY: i32 = 25604;
const SPORE_BASIC: i32 = 25613;
const SPORE_FIRE: i32 = 25605;
const SPORE_WATER: i32 = 25606;
const SPORE_WIND: i32 = 25607;
const SPORE_EARTH: i32 = 25608;
const DWARVEN_GHOST: i32 = 32370;
const EPIDOSES: [i32; 4] = [25610, 25609, 25612, 25611]; // Order is important!
const TOWER_MONSTERS: [i32; 11] = [
    18490, 22393, 22394, 22395, 22411, 22412, 22413, 22439, 22440, 22441, 22442,
];
const ELEMENTS: [i32; 4] = [25605, 25606, 25607, 25608];
const OPPOSITE_ELEMENTS: [i32; 4] = [25606, 25605, 25608, 25607];
const ELEMENTS_NAME: [&str; 4] = ["Fire", "Water", "Wind", "Earth"];
const SPORES_MOVE_POINTS: [(i32, i32, i32); 6] = [
    (-46080, 246368, -14183),
    (-44816, 246368, -14183),
    (-44224, 247440, -14184),
    (-44896, 248464, -14183),
    (-46064, 248544, -14183),
    (-46720, 247424, -14183),
];
const SPORES_MERGE_POSITION: [(i32, i32, i32); 4] = [
    (-45488, 246768, -14183),
    (-44767, 247419, -14183),
    (-46207, 247417, -14183),
    (-45462, 248174, -14183),
];
const SPORES_NPCSTRING_ID: [NpcStringId; 4] = [
    NpcStringId::ItSS1,
    NpcStringId::S1IsStrong,
    NpcStringId::ItSAlwaysS1,
    NpcStringId::S1WonTDo,
];

/// Java `DOORS`: manager id -> `(entrance door, exit door)`.
const DOORS: [(i32, i32, i32); 12] = [
    (18494, 18250001, 18250002),
    (18495, 18250003, 18250004),
    (18496, 18250005, 18250006),
    (18497, 18250007, 18250008),
    (18498, 18250009, 18250010),
    (18499, 18250011, 18250101),
    (18500, 18250013, 18250014),
    (18501, 18250015, 18250102),
    (18502, 18250017, 18250018),
    (18503, 18250019, 18250103),
    (18504, 18250021, 18250022),
    (18505, 18250023, 18250024),
];

/// Java `ZONES`: manager id -> the script zone of its room.
const ZONES: [(i32, i32); 12] = [
    (18494, 200020),
    (18495, 200021),
    (18496, 200022),
    (18497, 200023),
    (18498, 200024),
    (18499, 200025),
    (18500, 200026),
    (18501, 200027),
    (18502, 200028),
    (18503, 200029),
    (18504, 200030),
    (18505, 200031),
];

/// `(npc id, x, y, z, heading)`.
type RoomSpawn = (i32, i32, i32, i32, i32);

/// Java `SPAWNS`: the monsters of a room.
fn room_spawns(manager_id: i32) -> &'static [RoomSpawn] {
    match manager_id {
        18494 => &[
            (22393, -46371, 246400, -9120, 0),
            (22394, -46435, 245830, -9120, 0),
            (22394, -46536, 246275, -9120, 0),
            (22393, -46239, 245996, -9120, 0),
            (22394, -46229, 246347, -9120, 0),
            (22394, -46019, 246198, -9120, 0),
        ],
        18495 => &[
            (22439, -48146, 249597, -9124, -16280),
            (22439, -48144, 248711, -9124, 16368),
            (22439, -48704, 249597, -9104, -16380),
            (22439, -49219, 249596, -9104, -16400),
            (22439, -49715, 249601, -9104, -16360),
            (22439, -49714, 248696, -9104, 15932),
            (22439, -49225, 248710, -9104, 16512),
            (22439, -48705, 248708, -9104, 16576),
        ],
        18496 => &[
            (22441, -51176, 246055, -9984, 0),
            (22441, -51699, 246190, -9984, 0),
            (22442, -52060, 245956, -9984, 0),
            (22442, -51565, 246433, -9984, 0),
        ],
        18497 => &[
            (22440, -49754, 243866, -9968, -16328),
            (22440, -49754, 242940, -9968, 16336),
            (22440, -48733, 243858, -9968, -16208),
            (22440, -48745, 242936, -9968, 16320),
            (22440, -49264, 242946, -9968, 16312),
            (22440, -49268, 243869, -9968, -16448),
            (22440, -48186, 242934, -9968, 16576),
            (22440, -48185, 243855, -9968, -16448),
        ],
        18498 => &[
            (22411, -46355, 246375, -9984, 0),
            (22411, -46167, 246160, -9984, 0),
            (22393, -45952, 245748, -9984, 0),
            (22394, -46428, 246254, -9984, 0),
            (22393, -46490, 245871, -9984, 0),
            (22394, -45877, 246309, -9984, 0),
        ],
        18499 => &[
            (22395, -48730, 248067, -9984, 0),
            (22395, -49112, 248250, -9984, 0),
        ],
        18500 => &[
            (22393, -51954, 246475, -10848, 0),
            (22394, -51421, 246512, -10848, 0),
            (22394, -51404, 245951, -10848, 0),
            (22393, -51913, 246206, -10848, 0),
            (22394, -51663, 245979, -10848, 0),
            (22394, -51969, 245809, -10848, 0),
            (22412, -51259, 246357, -10848, 0),
        ],
        18501 => &[
            (22395, -48856, 243949, -10848, 0),
            (22395, -49144, 244190, -10848, 0),
        ],
        18502 => &[
            (22441, -46471, 246135, -11704, 0),
            (22441, -46449, 245997, -11704, 0),
            (22441, -46235, 246187, -11704, 0),
            (22441, -46513, 246326, -11704, 0),
            (22441, -45889, 246313, -11704, 0),
        ],
        18503 => &[
            (22395, -49067, 248050, -11712, 0),
            (22395, -48957, 248223, -11712, 0),
        ],
        18504 => &[
            (22413, -51748, 246138, -12568, 0),
            (22413, -51279, 246200, -12568, 0),
            (22413, -51787, 246594, -12568, 0),
            (22413, -51892, 246544, -12568, 0),
            (22413, -51500, 245781, -12568, 0),
            (22413, -51941, 246045, -12568, 0),
        ],
        18505 => &[
            (18490, -48238, 243347, -13376, 0),
            (18490, -48462, 244022, -13376, 0),
            (18490, -48050, 244045, -13376, 0),
            (18490, -48229, 243823, -13376, 0),
            (18490, -47871, 243208, -13376, 0),
            (18490, -48255, 243528, -13376, 0),
            (18490, -48461, 243780, -13376, 0),
            (18490, -47983, 243197, -13376, 0),
            (18490, -47841, 243819, -13376, 0),
            (18490, -48646, 243764, -13376, 0),
            (18490, -47806, 243850, -13376, 0),
            (18490, -48456, 243447, -13376, 0),
        ],
        _ => &[],
    }
}

/// The fields of Java's `TowerOfNaia`.
struct NaiaState {
    /// `_lock`.
    lock: Option<ScriptNpc>,
    /// `_controller`.
    controller: Option<ScriptNpc>,
    /// `_counter`.
    counter: i32,
    /// `_despawnedSporesCount`.
    despawned_spores_count: i32,
    /// `_indexCount`.
    index_count: [i32; 2],
    /// `_challengeState`.
    challenge_state: i32,
    /// `_winIndex`.
    win_index: i32,
    /// `_activeRooms`.
    active_rooms: HashMap<i32, bool>,
    /// `_spawns`.
    spawns: HashMap<i32, Vec<ObjectId>>,
    /// `_sporeSpawn`.
    spore_spawn: Vec<ObjectId>,
}

impl Default for NaiaState {
    fn default() -> Self {
        Self {
            lock: None,
            controller: None,
            counter: 0,
            despawned_spores_count: 0,
            index_count: [0, 0],
            challenge_state: 0,
            win_index: -1,
            active_rooms: HashMap::new(),
            spawns: HashMap::new(),
            spore_spawn: Vec::new(),
        }
    }
}

/// Java `ai.areas.Hellbound.AI.Zones.TowerOfNaia.TowerOfNaia`.
pub struct TowerOfNaia;

fn state<'a>(ctx: &'a mut ScriptCtx<'_>) -> &'a mut NaiaState {
    ctx.state::<NaiaState>()
}

fn door_pair(manager_id: i32) -> Option<(i32, i32)> {
    DOORS.iter().find(|d| d.0 == manager_id).map(|d| (d.1, d.2))
}

fn zone_of(manager_id: i32) -> Option<i32> {
    ZONES.iter().find(|z| z.0 == manager_id).map(|z| z.1)
}

fn live(ctx: &ScriptCtx<'_>, npc: ObjectId) -> bool {
    ctx.world().npc(npc).is_some_and(|n| !n.decayed)
}

/// Java `getSporeGroup(sporeId)`.
fn get_spore_group(spore_id: i32) -> i32 {
    match spore_id {
        SPORE_FIRE | SPORE_WATER => 0,
        SPORE_WIND | SPORE_EARTH => 1,
        _ => -1,
    }
}

/// Java `initRoom(managerId)`.
fn init_room(ctx: &mut ScriptCtx<'_>, manager_id: i32) {
    remove_all_players(ctx, manager_id);
    state(ctx).active_rooms.insert(manager_id, false);

    if let Some((entrance, exit)) = door_pair(manager_id) {
        ctx.open_door(entrance, 0);
        ctx.close_door(exit, 0);
    }

    if let Some(spawned) = state(ctx).spawns.remove(&manager_id) {
        for npc in spawned {
            if live(ctx, npc) && !ctx.is_dead(npc) {
                ctx.world_mut().delete_npc(npc);
            }
        }
    }
}

/// Java `initSporeChallenge()`.
fn init_spore_challenge(ctx: &mut ScriptCtx<'_>) {
    let s = state(ctx);
    s.despawned_spores_count = 0;
    s.challenge_state = 0;
    s.win_index = -1;
    s.index_count = [0, 0];
    // `ZoneManager.getZoneById(200100).setEnabled(false)`, `(200101).setEnabled(false)` and then
    // `(200101).setEnabled(true)`: see "Left out".
}

/// Java `markElpyRespawn()`.
fn mark_elpy_respawn(ctx: &mut ScriptCtx<'_>) {
    let respawn_time = i64::from(ctx.get_random_range(43200, 216000).wrapping_mul(1000)) + ctx.now_ms();
    ctx.set_global_variable("elpy_respawn_time", &respawn_time.to_string());
}

/// Java `moveTo(npc, coords)`.
fn move_to(ctx: &mut ScriptCtx<'_>, npc: ObjectId, (x, y, z): (i32, i32, i32)) {
    let Some(script_npc) = ctx.script_npc(npc) else {
        return;
    };
    ctx.world_mut().set_running(npc, false);
    ctx.disable_core_ai(script_npc, true);
    ctx.set_random_walking(script_npc, false);
    ctx.npc_move_to(script_npc, x, y, z);
    ctx.world_mut().set_npc_spawn_point(npc, x, y, z);
}

/// Java `spawnRandomSpore()`.
fn spawn_random_spore(ctx: &mut ScriptCtx<'_>) -> Option<ScriptNpc> {
    let id = ctx.get_random_range(SPORE_FIRE, SPORE_EARTH);
    ctx.add_spawn(id, -45474, 247450, -13994, 49152, false, 0, 0)
}

/// Java `spawnOppositeSpore(srcSporeId)`.
fn spawn_opposite_spore(ctx: &mut ScriptCtx<'_>, src_spore_id: i32) -> Option<ScriptNpc> {
    let idx = ELEMENTS.iter().position(|e| *e == src_spore_id)?;
    ctx.add_spawn(OPPOSITE_ELEMENTS[idx], -45474, 247450, -13994, 49152, false, 0, 0)
}

/// Java `startRoom(managerId)`.
fn start_room(ctx: &mut ScriptCtx<'_>, manager_id: i32) {
    state(ctx).active_rooms.insert(manager_id, true);

    if let Some((entrance, _)) = door_pair(manager_id) {
        ctx.close_door(entrance, 0);
    }

    let spawn_list = room_spawns(manager_id);
    if !spawn_list.is_empty() {
        let mut spawned = Vec::new();
        for &(npc_id, x, y, z, heading) in spawn_list {
            if let Some(npc) = ctx.add_spawn(npc_id, x, y, z, heading, false, 0, 0) {
                spawned.push(npc.object_id);
            }
        }

        if !spawned.is_empty() {
            state(ctx).spawns.insert(manager_id, spawned);
        }
    }
}

/// Java `removeForeigners(managerId, party)`: the players of the room who are not in the party of
/// `leader` leave it.
fn remove_foreigners(ctx: &mut ScriptCtx<'_>, manager_id: i32, leader: ObjectId) {
    if let Some(zone) = zone_of(manager_id) {
        for player in ctx.world().zone_players_inside(zone, "ScriptZone") {
            let same = ctx
                .world()
                .party_of(player)
                .and_then(|p| ctx.world().party_leader(p))
                == Some(leader);
            if !same {
                ctx.teleport_player(player, 16110, 243841, 11616, 0, false);
            }
        }
    }
}

/// Java `removeAllPlayers(managerId)`.
fn remove_all_players(ctx: &mut ScriptCtx<'_>, manager_id: i32) {
    if let Some(zone) = zone_of(manager_id) {
        for player in ctx.world().zone_players_inside(zone, "ScriptZone") {
            ctx.teleport_player(player, 16110, 243841, 11616, 0, false);
        }
    }
}

/// Java `removeSpores()`.
fn remove_spores(ctx: &mut ScriptCtx<'_>) {
    let spores = std::mem::take(&mut state(ctx).spore_spawn);
    for spore in spores {
        if live(ctx, spore) && !ctx.is_dead(spore) {
            ctx.world_mut().delete_npc(spore);
        }
    }

    ctx.cancel_quest_timers("despawn_spore");
}

/// Java `spawnElpy()`'s spawn.
fn spawn_elpy_now(ctx: &mut ScriptCtx<'_>) {
    ctx.add_spawn(MUTATED_ELPY, -45474, 247450, -13994, 49152, false, 0, 0);
}

impl Script for TowerOfNaia {
    fn name(&self) -> &'static str {
        "TowerOfNaia"
    }

    fn path(&self) -> &'static str {
        "ai/areas/Hellbound/AI/Zones/TowerOfNaia"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_first_talk_id(&[CONTROLLER]);
        s.add_start_npc(&[CONTROLLER, DWARVEN_GHOST]);
        s.add_talk_id(&[CONTROLLER, DWARVEN_GHOST]);
        s.add_attack_id(&[LOCK]);
        s.add_kill_id(&[LOCK, MUTATED_ELPY, SPORE_BASIC]);
        s.add_spawn_id(&[MUTATED_ELPY, SPORE_BASIC]);

        for npc_id in SPORE_FIRE..=SPORE_EARTH {
            s.add_kill_id(&[npc_id]);
            s.add_spawn_id(&[npc_id]);
        }

        for npc_id in ROOM_MANAGER_FIRST..=ROOM_MANAGER_LAST {
            s.add_first_talk_id(&[npc_id]);
            s.add_talk_id(&[npc_id]);
            s.add_start_npc(&[npc_id]);
        }

        s.add_kill_id(&TOWER_MONSTERS);
    }

    // The Java constructor.
    fn on_load(&self, ctx: &mut ScriptCtx<'_>) {
        for npc_id in ROOM_MANAGER_FIRST..=ROOM_MANAGER_LAST {
            init_room(ctx, npc_id);
        }

        let lock = ctx.add_spawn(LOCK, 16409, 244438, 11620, -1048, false, 0, 0);
        let controller = ctx.add_spawn(CONTROLLER, 16608, 244420, 11620, 31264, false, 0, 0);
        let s = state(ctx);
        s.lock = lock;
        s.controller = controller;
        s.counter = 90;
        init_spore_challenge(ctx);
        // `spawnElpy()` decides on the `elpy_respawn_time` row, which the Java GS may have
        // changed (D-50): read it first.
        ctx.reread_global_variables(&["elpy_respawn_time"], "spawn_elpy", None, None);
    }

    fn on_first_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let npc_id = npc.id;
        if npc_id == CONTROLLER {
            if state(ctx).lock.is_none() {
                return Some("18492-02.htm".to_owned());
            }

            return Some("18492-01.htm".to_owned());
        } else if (ROOM_MANAGER_FIRST..=ROOM_MANAGER_LAST).contains(&npc_id)
            && state(ctx).active_rooms.get(&npc_id) == Some(&false)
        {
            if ctx.world().party_of(player).is_none() {
                ctx.send_system_message(
                    player,
                    SystemMessageId::YouMustBeInAPartyInOrderToOperateTheMachine,
                );
                return None;
            }

            return Some("manager.htm".to_owned());
        }

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

        // The room stop timer of `ThreadPool.schedule(new StopRoomTask(npcId), 300000)`.
        if let Some(manager) = event.strip_prefix("stop_room ") {
            if let Ok(manager_id) = manager.parse::<i32>() {
                init_room(ctx, manager_id);
            }
            return None;
        }

        // The continuation of the re-read of `spawnElpy()`.
        if event == "spawn_elpy" {
            let respawn_time = ctx.global_variables().get_long("elpy_respawn_time", 0);
            if respawn_time <= ctx.now_ms() {
                spawn_elpy_now(ctx);
            } else {
                ctx.start_quest_timer("spawn_elpy_later", respawn_time - ctx.now_ms(), None, None, false);
            }
            return None;
        }
        if event == "spawn_elpy_later" {
            spawn_elpy_now(ctx);
            return None;
        }

        // Timer. Spawns Naia Lock
        if event.eq_ignore_ascii_case("spawn_lock") {
            htmltext = None;
            let lock = ctx.add_spawn(LOCK, 16409, 244438, 11620, -1048, false, 0, 0);
            let s = state(ctx);
            s.lock = lock;
            s.counter = 90;
        }
        // Timer. Depending of _challengeState despans all spawned spores, or spores, reached
        // assembly point
        else if event.eq_ignore_ascii_case("despawn_total") {
            let (challenge_state, win_index) = {
                let s = state(ctx);
                (s.challenge_state, s.win_index)
            };
            // Spores is not attacked too long - despawn them all, reinit values
            if challenge_state == STATE_SPORE_IDLE_TOO_LONG {
                remove_spores(ctx);
                init_spore_challenge(ctx);
            }
            // Spores are moving to assembly point. Despawn all reached, check for reached spores
            // count.
            else if challenge_state == STATE_SPORE_CHALLENGE_SUCCESSFULL && win_index >= 0 {
                // Requirements are met, despawn all spores, spawn Epidos
                if state(ctx).despawned_spores_count >= 10 || state(ctx).spore_spawn.is_empty() {
                    remove_spores(ctx);
                    state(ctx).despawned_spores_count = 0;
                    let coords = SPORES_MERGE_POSITION[win_index as usize];
                    ctx.add_spawn(
                        EPIDOSES[win_index as usize],
                        coords.0,
                        coords.1,
                        coords.2,
                        0,
                        false,
                        0,
                        0,
                    );
                    init_spore_challenge(ctx);
                }
                // Requirements aren't met, despawn reached spores
                else {
                    let spores = state(ctx).spore_spawn.clone();
                    for spore in spores {
                        let reached = live(ctx, spore)
                            && !ctx.is_dead(spore)
                            && ctx
                                .world()
                                .position_of(spore)
                                .zip(ctx.world().npc(spore).map(|n| n.spawn_point))
                                .is_some_and(|(here, home)| here.0 == home.0 && here.1 == home.1);
                        if reached {
                            ctx.world_mut().delete_npc(spore);
                            let s = state(ctx);
                            s.spore_spawn.retain(|id| *id != spore);
                            s.despawned_spores_count += 1;
                        }
                    }

                    ctx.start_quest_timer("despawn_total", 3000, None, None, false);
                }
            }
        }

        let npc = npc?;
        let npc_id = npc.id;
        if event.eq_ignore_ascii_case("despawn_spore")
            && !ctx.is_dead(npc.object_id)
            && state(ctx).challenge_state == STATE_SPORE_CHALLENGE_IN_PROGRESS
        {
            htmltext = None;
            state(ctx).spore_spawn.retain(|id| *id != npc.object_id);

            if npc_id == SPORE_BASIC {
                spawn_random_spore(ctx);
                spawn_random_spore(ctx);
            } else if (SPORE_FIRE..=SPORE_EARTH).contains(&npc_id) {
                state(ctx).despawned_spores_count += 1;

                if state(ctx).despawned_spores_count < SELF_DESPAWN_LIMIT {
                    spawn_opposite_spore(ctx, npc_id);
                } else {
                    state(ctx).challenge_state = STATE_SPORE_IDLE_TOO_LONG;
                    ctx.start_quest_timer("despawn_total", 60000, None, None, false);
                }
            }

            ctx.world_mut().delete_npc(npc.object_id);
        } else if event.eq_ignore_ascii_case("18492-05.htm") {
            let lock = state(ctx).lock;
            let weak = lock.is_none_or(|l| ctx.npc_current_hp(l) > f64::from(ctx.npc_max_hp(l) / 10));
            if weak {
                htmltext = None;
                ctx.cast_skill_at(npc, player, SkillKey::new(5527, 1));
            }
        } else if event.eq_ignore_ascii_case("teleport") && state(ctx).lock.is_some() {
            htmltext = None;
            let player = player?;
            if let Some(party) = ctx.world().party_of(player) {
                let leader = ctx.world().party_leader(party)?;
                if ctx.check_if_in_range(3000, leader, npc.object_id, true) {
                    for member in ctx.world().party_members(party) {
                        if ctx.check_if_in_range(2000, member, npc.object_id, true) {
                            ctx.teleport_player(member, -47271, 246098, -9120, 0, true);
                        }
                    }

                    if let Some(lock) = state(ctx).lock {
                        ctx.world_mut().delete_npc(lock.object_id);
                    }
                    state(ctx).lock = None;
                    ctx.cancel_quest_timers("spawn_lock");
                    ctx.start_quest_timer("spawn_lock", 1_200_000, None, None, false);
                } else {
                    ctx.cast_skill_at(npc, Some(player), SkillKey::new(5527, 1));
                }
            } else {
                ctx.teleport_player(player, -47271, 246098, -9120, 0, false);
                if let Some(lock) = state(ctx).lock {
                    ctx.world_mut().delete_npc(lock.object_id);
                }
                state(ctx).lock = None;
                ctx.cancel_quest_timers("spawn_lock");
                ctx.start_quest_timer("spawn_lock", 1_200_000, None, None, false);
            }
        } else if event.eq_ignore_ascii_case("go") && state(ctx).active_rooms.get(&npc_id) == Some(&false) {
            htmltext = None;
            let player = player?;
            if let Some(party) = ctx.world().party_of(player) {
                if let Some(leader) = ctx.world().party_leader(party) {
                    remove_foreigners(ctx, npc_id, leader);
                }
                start_room(ctx, npc_id);
                ctx.start_quest_timer(&format!("stop_room {npc_id}"), 300_000, None, None, false);
            } else {
                ctx.send_system_message(
                    player,
                    SystemMessageId::YouMustBeInAPartyInOrderToOperateTheMachine,
                );
            }
        }

        htmltext
    }

    fn on_attack(
        &self,
        ctx: &mut ScriptCtx<'_>,
        npc: ScriptNpc,
        _attacker: ObjectId,
        _damage: i32,
        _is_summon: bool,
        _skill: Option<SkillKey>,
    ) {
        let (lock, controller, counter) = {
            let s = state(ctx);
            (s.lock, s.controller, s.counter)
        };
        if let Some(lock) = lock
            && npc.object_id == lock.object_id
        {
            let remainded_hp_percent =
                ((ctx.npc_current_hp(npc) * 100.0) / f64::from(ctx.npc_max_hp(npc))) as i32;
            if remainded_hp_percent <= counter
                && let Some(controller) = controller
            {
                if counter == 50 {
                    ctx.spawn_minion(lock, 18493);
                } else if counter == 10 {
                    ctx.spawn_minion(lock, 18493);
                    ctx.spawn_minion(lock, 18493);
                }

                ctx.broadcast_npc_say(
                    controller,
                    ChatType::NpcGeneral,
                    NpcStringId::EmergencyEmergencyTheOuterWallIsWeakeningRapidly,
                    &[],
                );
                state(ctx).counter -= 10;
            }
        }
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, _killer: ObjectId, _is_summon: bool) {
        let npc_id = npc.id;
        if npc_id == LOCK {
            state(ctx).lock = None;
            ctx.cancel_quest_timers("spawn_lock");
            ctx.start_quest_timer("spawn_lock", 300_000, None, None, false);
        } else if TOWER_MONSTERS.contains(&npc_id) {
            let mut manager_id = 0;
            for (manager, zone) in ZONES {
                if ctx.world().is_inside_zone_id(npc.object_id, zone) {
                    manager_id = manager;
                }
            }

            if manager_id > 0 && state(ctx).spawns.contains_key(&manager_id) {
                let spawned = state(ctx).spawns.get_mut(&manager_id);
                let empty = spawned.is_some_and(|list| {
                    list.retain(|id| *id != npc.object_id);
                    list.is_empty()
                });
                if empty && let Some((_, exit)) = door_pair(manager_id) {
                    ctx.open_door(exit, 0);
                    state(ctx).spawns.remove(&manager_id);
                }
            }
        } else if npc_id == MUTATED_ELPY {
            state(ctx).challenge_state = STATE_SPORE_CHALLENGE_IN_PROGRESS;
            mark_elpy_respawn(ctx);
            ctx.close_door(18250025, 0);
            // `ZoneManager.getZoneById(200100).setEnabled(true)`: see "Left out".

            for _ in 0..10 {
                ctx.add_spawn(SPORE_BASIC, -45464, 246768, -14210, 49152, false, 0, 0);
            }
        } else if npc_id == SPORE_BASIC && state(ctx).challenge_state == STATE_SPORE_CHALLENGE_IN_PROGRESS {
            state(ctx).spore_spawn.retain(|id| *id != npc.object_id);
            spawn_random_spore(ctx);
            spawn_random_spore(ctx);
        } else if (SPORE_FIRE..=SPORE_EARTH).contains(&npc_id)
            && (state(ctx).challenge_state == STATE_SPORE_CHALLENGE_IN_PROGRESS
                || state(ctx).challenge_state == STATE_SPORE_CHALLENGE_SUCCESSFULL)
        {
            state(ctx).spore_spawn.retain(|id| *id != npc.object_id);

            if state(ctx).challenge_state == STATE_SPORE_CHALLENGE_IN_PROGRESS {
                state(ctx).despawned_spores_count -= 1;
                let spore_group = get_spore_group(npc_id);
                if spore_group >= 0 {
                    let g = spore_group as usize;
                    let other = (spore_group - 1).unsigned_abs() as usize;
                    {
                        let s = state(ctx);
                        if npc_id == SPORE_FIRE || npc_id == SPORE_WIND {
                            s.index_count[g] += 2;
                        } else {
                            s.index_count[g] -= 2;
                        }

                        if s.index_count[other] > 0 {
                            s.index_count[other] -= 1;
                        } else if s.index_count[other] < 0 {
                            s.index_count[other] += 1;
                        }
                    }

                    let index = state(ctx).index_count[g];
                    if index.abs() < ELEMENT_INDEX_LIMIT
                        && index.abs() > 0
                        && index % 20 == 0
                        && ctx.get_random(100) < 50
                    {
                        let el = ELEMENTS
                            .iter()
                            .position(|e| *e == npc_id)
                            .map_or("", |i| ELEMENTS_NAME[i]);
                        let spores = state(ctx).spore_spawn.clone();
                        for spore in spores {
                            let Some(spore_npc) = ctx.script_npc(spore) else {
                                continue;
                            };
                            if !ctx.is_dead(spore) && spore_npc.id == npc_id {
                                let line = SPORES_NPCSTRING_ID[ctx.get_random(4) as usize];
                                ctx.broadcast_npc_say(spore_npc, ChatType::NpcGeneral, line, &[el]);
                            }
                        }
                    }

                    if index.abs() < ELEMENT_INDEX_LIMIT {
                        if ((index > 0 && (npc_id == SPORE_FIRE || npc_id == SPORE_WIND))
                            || (index <= 0 && (npc_id == SPORE_WATER || npc_id == SPORE_EARTH)))
                            && ctx.get_random(1000) > 200
                        {
                            spawn_opposite_spore(ctx, npc_id);
                        } else {
                            spawn_random_spore(ctx);
                        }
                    } else {
                        // index value was reached
                        let win_index = ELEMENTS.iter().position(|e| *e == npc_id).unwrap_or(0);
                        {
                            let s = state(ctx);
                            s.challenge_state = STATE_SPORE_CHALLENGE_SUCCESSFULL;
                            s.despawned_spores_count = 0;
                            s.win_index = win_index as i32;
                        }
                        let coord = SPORES_MERGE_POSITION[win_index];
                        let spores = state(ctx).spore_spawn.clone();
                        for spore in spores {
                            if live(ctx, spore) && !ctx.is_dead(spore) {
                                move_to(ctx, spore, coord);
                            }
                        }

                        ctx.start_quest_timer("despawn_total", 3000, None, None, false);
                    }
                }
            }
        }
    }

    fn on_spawn(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc) {
        let npc_id = npc.id;
        if npc_id == MUTATED_ELPY {
            ctx.open_door(18250025, 0);
            // `ZoneManager.getZoneById(200100 / 200101).setEnabled(..)`: see "Left out".
        } else if (npc_id == SPORE_BASIC || (SPORE_FIRE..=SPORE_EARTH).contains(&npc_id))
            && state(ctx).challenge_state == STATE_SPORE_CHALLENGE_IN_PROGRESS
        {
            state(ctx).spore_spawn.push(npc.object_id);
            ctx.world_mut().set_running(npc.object_id, false);
            if let Some((x, y, z)) = ctx.get_random_entry(&SPORES_MOVE_POINTS) {
                ctx.world_mut().set_npc_spawn_point(npc.object_id, x, y, z);
                ctx.npc_move_to(npc, x, y, z);
            }
            ctx.start_quest_timer("despawn_spore", 60000, Some(npc), None, false);
        }
    }
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::ai::areas::hellbound::testing::{last_page, spawn};
    use crate::testing::{Pages, ScriptTest};

    const NPCS: &str = r#"<npc id="18491" type="Monster" level="80" name="Naia Lock"/>
        <npc id="18492" type="Folk" level="80" name="Controller"/>
        <npc id="18493" type="Monster" level="80" name="Lock Minion"/>
        <npc id="18494" type="Folk" level="80" name="Room Manager 1"/>
        <npc id="22393" type="Monster" level="80" name="Room 1 Monster"/>
        <npc id="22394" type="Monster" level="80" name="Room 1 Monster 2"/>
        <npc id="25604" type="Monster" level="80" name="Mutated Elpy"/>
        <npc id="25613" type="Monster" level="80" name="Spore"/>
        <npc id="25605" type="Monster" level="80" name="Spore of Fire"/>
        <npc id="25606" type="Monster" level="80" name="Spore of Water"/>
        <npc id="25607" type="Monster" level="80" name="Spore of Wind"/>
        <npc id="25608" type="Monster" level="80" name="Spore of Earth"/>"#;

    fn setup() -> ScriptTest {
        let mut registry = ScriptRegistry::new();
        registry.add(TowerOfNaia);
        let pages = Pages::of(&TowerOfNaia).plain(&["18492-01.htm", "18492-02.htm", "manager.htm"]);
        let mut t = ScriptTest::with_pages(registry, NPCS, "", pages);
        t.enter(1, 80, vec![]);
        t.ticks(5);
        t
    }

    #[test]
    fn the_load_spawns_the_lock_the_controller_and_a_waiting_elpy() {
        let mut t = setup();
        assert_eq!(t.npcs_of(LOCK).len(), 1);
        assert_eq!(t.npcs_of(CONTROLLER).len(), 1);
        t.world.run_as_script("TowerOfNaia", |ctx| {
            assert_eq!(state(ctx).counter, 90);
            assert_eq!(state(ctx).win_index, -1);
            assert_eq!(state(ctx).active_rooms.len(), 12);
        });
    }

    #[test]
    fn the_controller_s_page_follows_the_lock() {
        let mut t = setup();
        let controller = t.npcs_of(CONTROLLER)[0];
        let page = |t: &mut ScriptTest| {
            let mut page = None;
            t.world.run_as_script("TowerOfNaia", |ctx| {
                let npc = ctx.script_npc(controller).unwrap();
                page = TowerOfNaia.on_first_talk(ctx, npc, ObjectId(1));
            });
            page
        };
        assert_eq!(page(&mut t).as_deref(), Some("18492-01.htm"));
        t.kill(1, t.npcs_of(LOCK)[0]);
        t.world.run_as_script("TowerOfNaia", |ctx| {
            assert!(state(ctx).lock.is_none());
            // The Lock comes back after five minutes.
            assert!(ctx.has_quest_timer("spawn_lock", None, None));
        });
        assert_eq!(page(&mut t).as_deref(), Some("18492-02.htm"));
        t.wait_ms(300_100);
        assert_eq!(t.npcs_of(LOCK).len(), 1);
    }

    #[test]
    fn a_room_needs_a_party_and_opens_when_its_monsters_are_dead() {
        let mut t = setup();
        let manager = spawn(&mut t, 18494, 20);
        // Without a party the manager only complains.
        t.talk(1, manager);
        assert!(last_page(&t.take(), 1).is_empty());
        t.enter(2, 80, vec![]);
        t.party(1, 2);
        t.talk(1, manager);
        assert!(last_page(&t.take(), 1).contains("manager.htm"));
        t.world.run_as_script("TowerOfNaia", |ctx| {
            let m = ctx.script_npc(manager).unwrap();
            assert_eq!(TowerOfNaia.on_event(ctx, "go", Some(m), Some(ObjectId(1))), None);
            assert_eq!(state(ctx).active_rooms.get(&18494), Some(&true));
        });
        let monsters: Vec<ObjectId> = [22393, 22394].iter().flat_map(|id| t.npcs_of(*id)).collect();
        assert_eq!(monsters.len(), 6);
        // The room closes by itself after five minutes.
        t.wait_ms(300_100);
        t.world.run_as_script("TowerOfNaia", |ctx| {
            assert_eq!(state(ctx).active_rooms.get(&18494), Some(&false));
        });
        assert!(t.npcs_of(22393).is_empty());
    }

    #[test]
    fn the_table_of_rooms_is_complete() {
        for manager in ROOM_MANAGER_FIRST..=ROOM_MANAGER_LAST {
            assert!(door_pair(manager).is_some() && zone_of(manager).is_some());
            assert!(!room_spawns(manager).is_empty());
        }
        assert!(TOWER_MONSTERS.windows(2).all(|w| w[0] < w[1]));
    }

    #[test]
    fn killing_the_elpy_starts_the_spore_challenge() {
        let mut t = setup();
        let elpy = spawn(&mut t, MUTATED_ELPY, 20);
        t.kill(1, elpy);
        t.ticks(4);
        t.world.run_as_script("TowerOfNaia", |ctx| {
            assert_eq!(state(ctx).challenge_state, STATE_SPORE_CHALLENGE_IN_PROGRESS);
            assert!(ctx.global_variables().get_long("elpy_respawn_time", 0) > 0);
        });
        assert_eq!(t.npcs_of(SPORE_BASIC).len(), 10);
        // A basic spore killed gives two elemental ones.
        let basic = t.npcs_of(SPORE_BASIC)[0];
        t.kill(1, basic);
        t.ticks(4);
        let elemental: usize = (SPORE_FIRE..=SPORE_EARTH).map(|id| t.npcs_of(id).len()).sum();
        assert_eq!(elemental, 2);
    }
}
