//! Kamaloka (the Hall of the Abyss and the Labyrinth of the Abyss): the party instances of the
//! Kamaloka captains, 19 of them, each with a first room, a second room, a miniboss and a boss.
// Java: dist/game/data/scripts/instances/Kamaloka/Kamaloka.java
//
// Java keeps the progress of an instance in a `KamaWorld extends InstanceWorld`; here the
// `InstanceWorld` is the engine's and the progress is the script's state, by instance id
// ([`KamaWorld`]).

use std::collections::HashMap;

use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{SystemMessage, SystemMessageParam};
use l2_util::ObjectId;
use l2_world::effects::SkillKey;
use l2_world::{InstanceWorld, Script, ScriptCtx, ScriptNpc, ScriptSetup};

use super::kamaloka_reset_time;

// Reset time for all kamaloka (06:30 on the server's clock) is `kamaloka_reset_time`.

/// Time after which instance without players will be destroyed Default: 5 minutes
const EMPTY_DESTROY_TIME: i64 = 5;

/// Time to destroy instance (and eject players away) after boss defeat Default: 5 minutes
const EXIT_TIME: i32 = 5;

/// Maximum level difference between players level and kamaloka level Default: 5
const MAX_LEVEL_DIFFERENCE: i32 = 5;

/// If true shaman in the first room will have same npcId as other mobs, making radar useless
/// Default: true (but not retail like)
const STEALTH_SHAMAN: bool = true;

// Template IDs for Kamaloka
const TEMPLATE_IDS: [i32; 19] = [
    57, 58, 73, 60, 61, 74, 63, 64, 75, 66, 67, 76, 69, 70, 77, 72, 78, 79, 134,
];

// Level of the Kamaloka
const LEVEL: [i32; 19] = [
    23, 26, 29, 33, 36, 39, 43, 46, 49, 53, 56, 59, 63, 66, 69, 73, 78, 81, 83,
];

// Duration of the instance, minutes
const DURATION: [i32; 19] = [
    30, 30, 45, 30, 30, 45, 30, 30, 45, 30, 30, 45, 30, 30, 45, 30, 45, 45, 45,
];

// Maximum party size for the instance
const MAX_PARTY_SIZE: [usize; 19] = [6, 6, 9, 6, 6, 9, 6, 6, 9, 6, 6, 9, 6, 6, 9, 6, 9, 9, 9];

/// List of buffs NOT removed on enter from player and pet.
/// On retail only newbie guide buffs not removed.
const BUFFS_WHITELIST: [i32; 13] = [
    4322, 4323, 4324, 4325, 4326, 4327, 4328, 4329, 4330, 4331, 5632, 5637, 5950,
];

// Teleport points into instances x, y, z
const TELEPORTS: [(i32, i32, i32); 19] = [
    (-88429, -220629, -7903),
    (-82464, -219532, -7899),
    (-10700, -174882, -10936), // -76280, -185540, -10936
    (-89683, -213573, -8106),
    (-81413, -213568, -8104),
    (-10700, -174882, -10936), // -76280, -174905, -10936
    (-89759, -206143, -8120),
    (-81415, -206078, -8107),
    (-10700, -174882, -10936),
    (-56999, -219856, -8117),
    (-48794, -220261, -8075),
    (-10700, -174882, -10936),
    (-56940, -212939, -8072),
    (-55566, -206139, -8120),
    (-10700, -174882, -10936),
    (-49805, -206139, -8117),
    (-10700, -174882, -10936),
    (-10700, -174882, -10936),
    (22003, -174886, -10900),
];

// Respawn delay for the mobs in the first room, seconds Default: 25
const FIRST_ROOM_RESPAWN_DELAY: i32 = 25;

/// First room information, `None` if room not spawned. Skill is casted on the boss when shaman is
/// defeated and mobs respawn stopped. Default: 5699 (decrease pdef).
/// shaman npcId, minions npcId, skillId, skillLevel
const FIRST_ROOM: [Option<[i32; 4]>; 19] = [
    None,
    None,
    Some([22485, 22486, 5699, 1]),
    None,
    None,
    Some([22488, 22489, 5699, 2]),
    None,
    None,
    Some([22491, 22492, 5699, 3]),
    None,
    None,
    Some([22494, 22495, 5699, 4]),
    None,
    None,
    Some([22497, 22498, 5699, 5]),
    None,
    Some([22500, 22501, 5699, 6]),
    Some([22503, 22504, 5699, 7]),
    Some([25706, 25707, 5699, 7]),
];

/// First room spawns of the Labyrinths and the Hall of the Abyss that have one.
const FIRST_ROOM_SPAWNS_A: [(i32, i32, i32); 9] = [
    (-12381, -174973, -10955),
    (-12413, -174905, -10955),
    (-12377, -174838, -10953),
    (-12316, -174903, -10953),
    (-12326, -174786, -10953),
    (-12330, -175024, -10953),
    (-12211, -174900, -10955),
    (-12238, -174849, -10953),
    (-12233, -174954, -10953),
];

/// First room spawns of the last Kamaloka (134).
const FIRST_ROOM_SPAWNS_B: [(i32, i32, i32); 9] = [
    (20409, -174827, -10912),
    (20409, -174947, -10912),
    (20494, -174887, -10912),
    (20494, -174767, -10912),
    (20614, -174887, -10912),
    (20579, -174827, -10912),
    (20579, -174947, -10912),
    (20494, -175007, -10912),
    (20374, -174887, -10912),
];

/// First room spawns, `None` if room not spawned x, y, z
fn first_room_spawns(index: usize) -> Option<&'static [(i32, i32, i32)]> {
    match index {
        2 | 5 | 8 | 11 | 14 | 16 | 17 => Some(&FIRST_ROOM_SPAWNS_A),
        18 => Some(&FIRST_ROOM_SPAWNS_B),
        _ => None,
    }
}

/// Second room information, `None` if room not spawned. Skill is casted on the boss when all mobs
/// are defeated. Default: 5700 (decrease mdef). npcId, skillId, skillLevel
const SECOND_ROOM: [Option<[i32; 3]>; 19] = [
    None,
    None,
    Some([22487, 5700, 1]),
    None,
    None,
    Some([22490, 5700, 2]),
    None,
    None,
    Some([22493, 5700, 3]),
    None,
    None,
    Some([22496, 5700, 4]),
    None,
    None,
    Some([22499, 5700, 5]),
    None,
    Some([22502, 5700, 6]),
    Some([22505, 5700, 7]),
    Some([25708, 5700, 7]),
];

const SECOND_ROOM_SPAWNS_A: [(i32, i32, i32); 5] = [
    (-14547, -174901, -10690),
    (-14543, -175030, -10690),
    (-14668, -174900, -10690),
    (-14538, -174774, -10690),
    (-14410, -174904, -10690),
];

const SECOND_ROOM_SPAWNS_B: [(i32, i32, i32); 5] = [
    (18175, -174991, -10653),
    (18070, -174890, -10655),
    (18157, -174886, -10655),
    (18249, -174885, -10653),
    (18144, -174821, -10648),
];

/// Spawns for second room, `None` if room not spawned x, y, z
fn second_room_spawns(index: usize) -> Option<&'static [(i32, i32, i32)]> {
    match index {
        2 | 5 | 8 | 11 | 14 | 16 | 17 => Some(&SECOND_ROOM_SPAWNS_A),
        18 => Some(&SECOND_ROOM_SPAWNS_B),
        _ => None,
    }
}

/// Miniboss information, `None` if miniboss not spawned. Skill is casted on the boss when
/// miniboss is defeated. Default: 5701 (decrease patk). npcId, x, y, z, skillId, skillLevel
const MINIBOSS: [Option<[i32; 6]>; 19] = [
    None,
    None,
    Some([25616, -16874, -174900, -10427, 5701, 1]),
    None,
    None,
    Some([25617, -16874, -174900, -10427, 5701, 2]),
    None,
    None,
    Some([25618, -16874, -174900, -10427, 5701, 3]),
    None,
    None,
    Some([25619, -16874, -174900, -10427, 5701, 4]),
    None,
    None,
    Some([25620, -16874, -174900, -10427, 5701, 5]),
    None,
    Some([25621, -16874, -174900, -10427, 5701, 6]),
    Some([25622, -16874, -174900, -10427, 5701, 7]),
    Some([25709, 15828, -174885, -10384, 5701, 7]),
];

/// Bosses of the kamaloka Instance ends when boss is defeated npcId, x, y, z
const BOSS: [[i32; 4]; 19] = [
    [18554, -88998, -220077, -7892],
    [18555, -81891, -220078, -7893],
    [29129, -20659, -174903, -9983],
    [18558, -89183, -213564, -8100],
    [18559, -81937, -213566, -8100],
    [29132, -20659, -174903, -9983],
    [18562, -89054, -206144, -8115],
    [18564, -81937, -206077, -8100],
    [29135, -20659, -174903, -9983],
    [18566, -56281, -219859, -8115],
    [18568, -49336, -220260, -8068],
    [29138, -20659, -174903, -9983],
    [18571, -56415, -212939, -8068],
    [18573, -56281, -206140, -8115],
    [29141, -20659, -174903, -9983],
    [18577, -49084, -206140, -8115],
    [29144, -20659, -174903, -9983],
    [29147, -20659, -174903, -9983],
    [25710, 12047, -174887, -9944],
];

/// Escape telepoters spawns, `None` if not spawned x, y, z
const TELEPORTERS: [Option<(i32, i32, i32)>; 19] = [
    None,
    None,
    Some((-10865, -174905, -10944)),
    None,
    None,
    Some((-10865, -174905, -10944)),
    None,
    None,
    Some((-10865, -174905, -10944)),
    None,
    None,
    Some((-10865, -174905, -10944)),
    None,
    None,
    Some((-10865, -174905, -10944)),
    None,
    Some((-10865, -174905, -10944)),
    Some((-10865, -174905, -10944)),
    Some((21837, -174885, -10904)),
];

/// Escape teleporter npcId
const TELEPORTER: i32 = 32496;

/// Kamaloka captains (start NPCs) npcIds.
const CAPTAINS: [i32; 6] = [30332, 30071, 30916, 30196, 31981, 31340];

/// Java `KamaWorld extends InstanceWorld`: what the script knows of one instance.
#[derive(Debug, Default)]
struct KamaWorld {
    /// 0-18 index of the kama type in arrays
    index: usize,
    /// objectId of the shaman
    shaman: i32,
    /// the spawns in the first room (excluding shaman), `None` once the shaman is dead
    first_room: Option<Vec<usize>>,
    /// objectIds of the mobs in the second room (0: killed)
    second_room: Option<Vec<i32>>,
    /// objectId of the miniboss
    mini_boss: i32,
    /// boss
    boss: Option<ObjectId>,
}

/// The `KamaWorld`s by instance id.
#[derive(Default)]
struct State {
    worlds: HashMap<i32, KamaWorld>,
}

/// Java `instances.Kamaloka.Kamaloka`.
pub struct Kamaloka;

impl Script for Kamaloka {
    fn name(&self) -> &'static str {
        "Kamaloka"
    }

    fn path(&self) -> &'static str {
        "instances/Kamaloka"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_first_talk_id(&[TELEPORTER]);
        s.add_talk_id(&[TELEPORTER]);
        for cap in CAPTAINS {
            s.add_start_npc(&[cap]);
            s.add_talk_id(&[cap]);
        }

        for mob in FIRST_ROOM.iter().flatten() {
            if STEALTH_SHAMAN {
                s.add_kill_id(&[mob[1]]);
            } else {
                s.add_kill_id(&[mob[0]]);
            }
        }

        for mob in SECOND_ROOM.iter().flatten() {
            s.add_kill_id(&[mob[0]]);
        }

        for mob in MINIBOSS.iter().flatten() {
            s.add_kill_id(&[mob[0]]);
        }

        for mob in BOSS {
            s.add_kill_id(&[mob[0]]);
        }
    }

    /// Handles only player's enter, single parameter - integer kamaloka index
    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        if npc.is_some() {
            let Some(player) = player else {
                return None;
            };
            // A bad event is a `NumberFormatException` Java logs and goes on; scripts do not log.
            if let Ok(index) = event.parse::<i32>() {
                enter_kamaloka_instance(ctx, player, index);
            }
        }

        None
    }

    /// Talk with captains and using of the escape teleporter
    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        if npc.id == TELEPORTER {
            // only party leader can talk with escape teleporter
            if ctx.world().is_party_leader(player) {
                let instance_id = ctx.world().instance_id_of(npc.object_id);
                let allowed = ctx
                    .state::<State>()
                    .worlds
                    .contains_key(&instance_id)
                    && ctx
                        .world()
                        .instance_world(instance_id)
                        .is_some_and(|w| w.is_allowed(player));
                if allowed {
                    let exit = ctx.world().instance(instance_id).and_then(|i| i.exit_location);

                    // teleports entire party away
                    for member in ctx.world().party_members_of(player) {
                        if ctx.world().instance_id_of(member) == instance_id {
                            if let Some((x, y, z)) = exit {
                                ctx.teleport_player(member, x, y, z, 0, true);
                            }
                        }
                    }
                }
            }
        } else {
            return Some(format!("{}.htm", npc.id));
        }

        None
    }

    /// Only escape teleporters first talk handled
    fn on_first_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        if npc.id == TELEPORTER {
            if ctx.world().is_in_party(player) && ctx.world().is_party_leader(player) {
                return Some("32496.htm".to_owned());
            }

            return Some("32496-no.htm".to_owned());
        }

        None
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, _player: ObjectId, _is_summon: bool) {
        let instance_id = ctx.world().instance_id_of(npc.object_id);
        if !ctx.state::<State>().worlds.contains_key(&instance_id) {
            return;
        }
        let object_id = npc.object_id.0;
        let index = ctx.state::<State>().worlds[&instance_id].index;

        // first room was spawned ? is shaman killed ?
        let shaman_killed = {
            let world = &ctx.state::<State>().worlds[&instance_id];
            world.first_room.is_some() && world.shaman != 0 && world.shaman == object_id
        };
        if shaman_killed {
            let spawns = {
                let world = ctx.state::<State>().worlds.get_mut(&instance_id);
                world.map(|w| {
                    w.shaman = 0;
                    w.first_room.take().unwrap_or_default()
                })
            }
            .unwrap_or_default();

            // stop respawn of the minions
            for spawn in spawns {
                ctx.world_mut().stop_spawn_respawn(spawn);
            }

            let boss = ctx.state::<State>().worlds[&instance_id].boss;
            if let (Some(boss), Some(room)) = (boss, FIRST_ROOM[index]) {
                apply_to_boss(ctx, boss, room[2], room[3]);
            }
            return;
        }

        // second room was spawned ?
        if ctx.state::<State>().worlds[&instance_id].second_room.is_some() {
            let mut all = true;

            // check for all mobs in the second room
            if let Some(room) = ctx
                .state::<State>()
                .worlds
                .get_mut(&instance_id)
                .and_then(|w| w.second_room.as_mut())
            {
                for slot in room.iter_mut() {
                    // found killed now mob
                    if *slot == object_id {
                        *slot = 0;
                    } else if *slot != 0 {
                        all = false;
                    }
                }
            }

            // all mobs killed ?
            if all {
                let boss = {
                    let world = ctx.state::<State>().worlds.get_mut(&instance_id);
                    world.and_then(|w| {
                        w.second_room = None;
                        w.boss
                    })
                };
                if let (Some(boss), Some(room)) = (boss, SECOND_ROOM[index]) {
                    apply_to_boss(ctx, boss, room[1], room[2]);
                }
                return;
            }
        }

        // miniboss spawned ?
        let mini_boss = ctx.state::<State>().worlds[&instance_id].mini_boss;
        if mini_boss != 0 && mini_boss == object_id {
            let boss = {
                let world = ctx.state::<State>().worlds.get_mut(&instance_id);
                world.and_then(|w| {
                    w.mini_boss = 0;
                    w.boss
                })
            };
            if let (Some(boss), Some(mini)) = (boss, MINIBOSS[index]) {
                apply_to_boss(ctx, boss, mini[4], mini[5]);
            }
            return;
        }

        // boss was killed, finish instance
        let boss_killed = ctx.state::<State>().worlds[&instance_id].boss == Some(npc.object_id);
        if boss_killed {
            if let Some(world) = ctx.state::<State>().worlds.get_mut(&instance_id) {
                world.boss = None;
            }
            finish_instance(ctx, instance_id);
        }
    }
}

/// `skill.applyEffects(world.boss, world.boss)` for `SkillData.getSkill(skillId, skillLevel)`.
fn apply_to_boss(ctx: &mut ScriptCtx<'_>, boss: ObjectId, skill_id: i32, skill_level: i32) {
    if skill_id != 0 && skill_level != 0 {
        ctx.world_mut()
            .apply_effects(boss, boss, SkillKey::new(skill_id, skill_level));
    }
}

/// Check if party with player as leader allowed to enter
fn check_party_conditions(ctx: &mut ScriptCtx<'_>, player: ObjectId, index: usize) -> bool {
    let Some(party) = ctx.world().party_of(player) else {
        // player must be in party
        ctx.world()
            .send(player, &SystemMessage::new(SystemMessageId::YouAreNotCurrentlyInAPartySoYouCannotEnter));
        return false;
    };

    // ...and be party leader
    if ctx.world().party_leader(party) != Some(player) {
        ctx.world().send(
            player,
            &SystemMessage::new(SystemMessageId::OnlyAPartyLeaderCanMakeTheRequestToEnter),
        );
        return false;
    }

    // party must not exceed max size for selected instance
    let members = ctx.world().party_members(party);
    if members.len() > MAX_PARTY_SIZE[index] {
        ctx.world().send(
            player,
            &SystemMessage::new(SystemMessageId::YouCannotEnterDueToThePartyHavingExceededTheLimit),
        );
        return false;
    }

    // get level of the instance
    let level = LEVEL[index];

    // and client name
    let instance_name = ctx.world().instance_name(TEMPLATE_IDS[index]);

    // for each party member
    for member in members {
        // player level must be in range
        if (ctx.player_level(member) - level).abs() > MAX_LEVEL_DIFFERENCE {
            let name = ctx.player_name(member);
            ctx.world().send(
                player,
                &SystemMessage::new(SystemMessageId::C1SLevelDoesNotCorrespondToTheRequirementsForEntry)
                    .with(SystemMessageParam::PlayerName(name)),
            );
            return false;
        }

        // player must be near party leader
        if !inside_radius_3d(ctx, member, player, 1000.0) {
            let name = ctx.player_name(member);
            ctx.world().send(
                player,
                &SystemMessage::new(
                    SystemMessageId::C1IsInALocationWhichCannotBeEnteredThereforeItCannotBeProcessed,
                )
                .with(SystemMessageParam::PlayerName(name)),
            );
            return false;
        }

        // get instances reenter times for player
        let now = ctx.now_ms();
        for (id, time) in ctx.world().instance_times(member) {
            // find instance with same name (kamaloka or labyrinth)
            // TODO: Zoey76: Don't use instance name, use other system.
            if instance_name != ctx.world().instance_name(id) {
                continue;
            }

            // if found instance still can't be reentered - exit
            if now < time {
                let name = ctx.player_name(member);
                ctx.world().send(
                    player,
                    &SystemMessage::new(SystemMessageId::C1MayNotReEnterYet)
                        .with(SystemMessageParam::PlayerName(name)),
                );
                return false;
            }
        }
    }

    true
}

/// `a.isInsideRadius3D(b, radius)`.
fn inside_radius_3d(ctx: &ScriptCtx<'_>, a: ObjectId, b: ObjectId, radius: f64) -> bool {
    match (ctx.world().position_of(a), ctx.world().position_of(b)) {
        (Some(a), Some(b)) => {
            let (dx, dy, dz) = (
                f64::from(a.0) - f64::from(b.0),
                f64::from(a.1) - f64::from(b.1),
                f64::from(a.2) - f64::from(b.2),
            );
            dx * dx + dy * dy + dz * dz <= radius * radius
        }
        _ => false,
    }
}

/// Removing all buffs from player and pet except BUFFS_WHITELIST
fn remove_buffs(ctx: &mut ScriptCtx<'_>, player: ObjectId) {
    ctx.world_mut().remove_buffs_except(player, &BUFFS_WHITELIST);
    if let Some(summon) = ctx.world().summon_of(player) {
        ctx.world_mut().remove_buffs_except(summon, &BUFFS_WHITELIST);
    }
}

/// Handling enter of the players into kamaloka. `index` is (0-18) kamaloka index in arrays.
fn enter_kamaloka_instance(ctx: &mut ScriptCtx<'_>, player: ObjectId, index: i32) {
    let Some((index, template_id)) = usize::try_from(index)
        .ok()
        .and_then(|i| TEMPLATE_IDS.get(i).map(|t| (i, *t)))
    else {
        // Java: "Problem with Kamaloka" (an `ArrayIndexOutOfBoundsException`).
        return;
    };

    // check for existing instances for this player
    let existing = ctx
        .world()
        .player_instance_world(player)
        .map(|w| (w.instance_id, w.template_id));

    // player already in the instance
    if let Some((instance_id, world_template)) = existing {
        // but not in kamaloka
        let kama_index = ctx.state::<State>().worlds.get(&instance_id).map(|w| w.index);
        let Some(kama_index) = kama_index.filter(|_| world_template == template_id) else {
            ctx.world().send(
                player,
                &SystemMessage::new(
                    SystemMessageId::YouHaveEnteredAnotherInstanceZoneThereforeYouCannotEnterCorrespondingDungeon,
                ),
            );
            return;
        };

        // check for level difference again on reenter
        if (ctx.player_level(player) - LEVEL[kama_index]).abs() > MAX_LEVEL_DIFFERENCE {
            let name = ctx.player_name(player);
            ctx.world().send(
                player,
                &SystemMessage::new(SystemMessageId::C1SLevelDoesNotCorrespondToTheRequirementsForEntry)
                    .with(SystemMessageParam::PlayerName(name)),
            );
            return;
        }

        // check what instance still exist
        if ctx.world().instance(instance_id).is_some() {
            remove_buffs(ctx, player);
            let to = TELEPORTS[index];
            ctx.teleport_player(player, to.0, to.1, to.2, instance_id, true);
        }
        return;
    }

    // Creating new kamaloka instance
    if !check_party_conditions(ctx, player, index) {
        return;
    }

    // Creating new instanceWorld, using our instanceId and templateId
    let instance_id = ctx.world_mut().create_dynamic_instance(template_id);
    let created_template = ctx
        .world()
        .instance(instance_id)
        .map_or(-1, |i| i.template_id);
    ctx.world_mut()
        .add_instance_world(InstanceWorld::new(instance_id, created_template));

    // set return location
    let exit = ctx.world().position_of(player);
    if let Some(instance) = ctx.world_mut().instance_mut(instance_id) {
        instance.exit_location = exit;

        // disable summon friend into instance
        instance.allow_summon = false;
    }

    // set duration and empty destroy time
    ctx.world_mut()
        .set_instance_duration(instance_id, DURATION[index] * 60000);
    ctx.world_mut()
        .set_instance_empty_destroy_time(instance_id, EMPTY_DESTROY_TIME * 60000);

    // set index for easy access to the arrays
    ctx.state::<State>().worlds.insert(
        instance_id,
        KamaWorld {
            index,
            ..KamaWorld::default()
        },
    );

    // spawn npcs
    spawn_kama(ctx, instance_id, index);

    // and finally teleport party into instance
    let to = TELEPORTS[index];
    for member in ctx.world().party_members_of(player) {
        if let Some(world) = ctx.world_mut().instance_world_mut(instance_id) {
            world.add_allowed(member);
        }
        remove_buffs(ctx, member);
        ctx.teleport_player(member, to.0, to.1, to.2, instance_id, true);
    }
}

/// Called on instance finish and handles reenter time for instance
// NOTE: Kamaloka overrides `finishInstance(world)` of `InstanceScript`: it sets 06:30 as the
// re-entry time of every allowed player, whatever the template says, and `EXIT_TIME` minutes to go.
fn finish_instance(ctx: &mut ScriptCtx<'_>, instance_id: i32) {
    let Some((template_id, allowed)) = ctx
        .world()
        .instance_world(instance_id)
        .map(|w| (w.template_id, w.allowed.clone()))
    else {
        return;
    };
    let now = ctx.now_ms();
    let offset = l2_util::time::local_offset_millis(now);
    let reenter = kamaloka_reset_time(now, offset, false);

    // set instance reenter time for all allowed players
    for plr in allowed {
        if ctx.world().player(plr).is_some() {
            ctx.world_mut().set_instance_time(plr, template_id, reenter);
            ctx.world().send(
                plr,
                &SystemMessage::new(
                    SystemMessageId::InstantZoneS1SEntryHasBeenRestrictedYouCanCheckTheNextPossibleEntryTimeByUsingTheCommandInstancezone,
                )
                .with(SystemMessageParam::InstanceName(template_id)),
            );
        }
    }

    // destroy instance after EXIT_TIME
    ctx.world_mut()
        .set_instance_duration(instance_id, EXIT_TIME * 60000);
    ctx.world_mut().set_instance_empty_destroy_time(instance_id, 0);
}

/// Spawn all NPCs in kamaloka
fn spawn_kama(ctx: &mut ScriptCtx<'_>, instance_id: i32, index: usize) {
    // first room
    if let (Some(npcs), Some(spawns)) = (FIRST_ROOM[index], first_room_spawns(index)) {
        let mut first_room = Vec::with_capacity(spawns.len().saturating_sub(1));
        // random position for shaman
        let shaman = ctx.get_random(i32::try_from(spawns.len()).unwrap_or(1));
        for (i, spawn) in spawns.iter().enumerate() {
            if i32::try_from(i).ok() == Some(shaman) {
                // stealth shaman use same npcId as other mobs
                let id = if STEALTH_SHAMAN { npcs[1] } else { npcs[0] };
                if let Some(npc) = ctx.add_spawn(id, spawn.0, spawn.1, spawn.2, 0, false, 0, instance_id) {
                    if let Some(world) = ctx.state::<State>().worlds.get_mut(&instance_id) {
                        world.shaman = npc.object_id.0;
                    }
                    ctx.world_mut().set_npc_random_walking(npc.object_id, false);
                }
            } else if let Some(npc) = ctx.add_spawn(npcs[1], spawn.0, spawn.1, spawn.2, 0, false, 0, instance_id)
            {
                // `spawn.setRespawnDelay(25); spawn.setAmount(1); spawn.startRespawn()`.
                if let Some(spawn_index) = ctx.world().npc_spawn_index(npc.object_id) {
                    ctx.world_mut()
                        .set_spawn_respawn_delay(spawn_index, FIRST_ROOM_RESPAWN_DELAY);
                    ctx.world_mut().start_spawn_respawn(spawn_index);
                    // store mobs spawns
                    first_room.push(spawn_index);
                }
                ctx.world_mut().set_npc_random_walking(npc.object_id, false);
            }
        }
        if let Some(world) = ctx.state::<State>().worlds.get_mut(&instance_id) {
            world.first_room = Some(first_room);
        }
    }

    // second room
    if let (Some(npcs), Some(spawns)) = (SECOND_ROOM[index], second_room_spawns(index)) {
        let mut second_room = Vec::with_capacity(spawns.len());
        for spawn in spawns {
            if let Some(npc) = ctx.add_spawn(npcs[0], spawn.0, spawn.1, spawn.2, 0, false, 0, instance_id) {
                ctx.world_mut().set_npc_random_walking(npc.object_id, false);
                second_room.push(npc.object_id.0);
            }
        }
        if let Some(world) = ctx.state::<State>().worlds.get_mut(&instance_id) {
            world.second_room = Some(second_room);
        }
    }

    // miniboss
    if let Some(mini) = MINIBOSS[index] {
        if let Some(npc) = ctx.add_spawn(mini[0], mini[1], mini[2], mini[3], 0, false, 0, instance_id) {
            ctx.world_mut().set_npc_random_walking(npc.object_id, false);
            if let Some(world) = ctx.state::<State>().worlds.get_mut(&instance_id) {
                world.mini_boss = npc.object_id.0;
            }
        }
    }

    // escape teleporter
    if let Some(at) = TELEPORTERS[index] {
        ctx.add_spawn(TELEPORTER, at.0, at.1, at.2, 0, false, 0, instance_id);
    }

    // boss
    let boss = BOSS[index];
    if let Some(npc) = ctx.add_spawn(boss[0], boss[1], boss[2], boss[3], 0, false, 0, instance_id) {
        ctx.world_mut().set_on_kill_delay(npc.object_id, 100);
        if let Some(world) = ctx.state::<State>().worlds.get_mut(&instance_id) {
            world.boss = Some(npc.object_id);
        }
    }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::{ClientInput, ScriptRegistry};

    use super::*;
    use crate::instances::testing::instance_data;
    use crate::testing::{ScriptTest, X, Y, Z, names_to, pages_to, player_data};

    const CAPTAIN: i32 = 30332;
    const NPCS: &str = r#"
        <npc id="30332" type="Folk" level="70" name="Captain"/>
        <npc id="32496" type="Folk" level="70" name="Escape Teleporter"/>
        <npc id="22486" type="Monster" level="29" name="Minion"/>
        <npc id="22487" type="Monster" level="29" name="Second Room Mob"/>
        <npc id="25616" type="Monster" level="29" name="Miniboss"/>
        <npc id="29129" type="Monster" level="29" name="Boss"/>
        <npc id="18554" type="Monster" level="23" name="Boss Zero"/>"#;
    const DIR: &str = "data/scripts/instances/Kamaloka";

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Kamaloka);
        let pages: Vec<(String, String)> = ["30332.htm", "32496.htm", "32496-no.htm"]
            .iter()
            .map(|p| (format!("{DIR}/{p}"), format!("<html><body>{p}</body></html>")))
            .collect();
        let mut pages: Vec<(&str, &str)> = pages.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
        pages.push((
            "data/html/default/30332.htm",
            r#"<html><body>Captain <a action="bypass Script Kamaloka 0">0</a><a action="bypass Script Kamaloka 2">2</a></body></html>"#,
        ));
        let mut t = ScriptTest::new(registry, NPCS, "", &pages);
        t.world.set_instance_data(instance_data(
            &[
                (57, "Kamaloka (Hall of the Abyss)"),
                (58, "Kamaloka (Hall of the Abyss)"),
                (73, "Kamaloka (Labyrinth of the Abyss)"),
            ],
            &[(57, ""), (58, ""), (73, "")],
        ));
        let captain = t.spawn(CAPTAIN, 20);
        (t, captain)
    }

    /// Players 1 and 2 of `level`, 2 in the party of 1.
    fn party(t: &mut ScriptTest, level: i32) {
        t.enter(1, level, vec![]);
        t.enter(2, level, vec![]);
        t.input(
            1,
            ClientInput::JoinParty {
                name: "P2".to_owned(),
                distribution_type_id: 0,
            },
        );
        t.input(2, ClientInput::AnswerJoinParty { response: 1 });
        assert!(t.world.is_party_leader(Oid(1)) && t.world.is_in_party(Oid(2)));
        t.take();
    }

    fn go(t: &mut ScriptTest, captain: Oid, index: i32) {
        t.talk(1, captain);
        t.bypass(1, &format!("Script Kamaloka {index}"));
        t.ticks(60);
    }

    fn state<R>(t: &mut ScriptTest, f: impl FnOnce(&mut State) -> R) -> R {
        let mut result = None;
        t.world.run_as_script("Kamaloka", |ctx| result = Some(f(ctx.state::<State>())));
        result.unwrap()
    }

    fn npcs_of(t: &ScriptTest, instance_id: i32, template: i32) -> Vec<Oid> {
        t.world
            .instance(instance_id)
            .unwrap()
            .npcs
            .keys()
            .copied()
            .filter(|n| t.world.npc(*n).is_some_and(|n| n.npc_id == template))
            .collect()
    }

    /// The wall clock the instance clock follows in these tests (no override is set).
    fn now() -> i64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| i64::try_from(d.as_millis()).unwrap_or(0))
    }

    /// How many system messages went to `player` in `out`.
    fn messages_to(out: &crate::testing::Sent, player: i32) -> usize {
        names_to(out, player).iter().filter(|n| *n == "SYSTEM_MESSAGE").count()
    }

    fn kill(t: &mut ScriptTest, npc: Oid, id: i32) {
        t.world.run_as_script("Kamaloka", |ctx| {
            Kamaloka.on_kill(ctx, ScriptNpc { object_id: npc, id }, Oid(1), false);
        });
    }

    #[test]
    fn the_leader_takes_the_party_into_a_new_instance_with_its_rooms() {
        let (mut t, captain) = setup();
        party(&mut t, LEVEL[2]);
        go(&mut t, captain, 2);

        // The instance of template 73: both are allowed and inside, no summons, 45 minutes.
        let id = t.world.instance_id_of(Oid(1));
        assert_eq!(id, 300_000);
        assert_eq!(t.world.instance_id_of(Oid(2)), id);
        let world = t.world.instance_world(id).unwrap();
        assert_eq!((world.template_id, world.allowed.clone()), (73, vec![Oid(1), Oid(2)]));
        let instance = t.world.instance(id).unwrap();
        assert!(!instance.allow_summon);
        assert_eq!(instance.exit_location, Some((X, Y, Z)));
        assert_eq!(instance.empty_destroy_time_ms, 5 * 60_000);
        assert!(instance.end_time_ms - t.world.instance(id).unwrap().start_time_ms >= 45 * 60_000);

        // Nine mobs in the first room (one of them the shaman), five in the second, the miniboss,
        // the escape teleporter and the boss.
        assert_eq!(npcs_of(&t, id, 22486).len(), 9);
        assert_eq!(npcs_of(&t, id, 22487).len(), 5);
        assert_eq!(npcs_of(&t, id, 25616).len(), 1);
        assert_eq!(npcs_of(&t, id, TELEPORTER).len(), 1);
        assert_eq!(npcs_of(&t, id, 29129).len(), 1);
        let shaman = state(&mut t, |s| s.worlds[&id].shaman);
        assert!(npcs_of(&t, id, 22486).contains(&Oid(shaman)));
        // The other eight respawn after 25 seconds.
        let respawning = state(&mut t, |s| s.worlds[&id].first_room.clone().unwrap());
        assert_eq!(respawning.len(), 8);
        assert!(respawning.iter().all(|s| t.world.is_respawn_enabled(*s)));
    }

    #[test]
    fn a_party_that_does_not_fit_is_turned_away_with_the_message() {
        let (mut t, captain) = setup();
        t.enter(1, LEVEL[2], vec![]);
        t.take();
        go(&mut t, captain, 2);
        // Not in a party: "You are not currently in a party, so you cannot enter."
        assert_eq!(messages_to(&t.take(), 1), 1);
        assert!(t.world.instances().all(|i| i.id <= 0));

        // The member is too low (more than 5 levels below 29).
        let mut data = player_data(2, LEVEL[2] - 6, vec![]);
        data.name = "P2".to_owned();
        t.enter_with(data);
        t.input(
            1,
            ClientInput::JoinParty {
                name: "P2".to_owned(),
                distribution_type_id: 0,
            },
        );
        t.input(2, ClientInput::AnswerJoinParty { response: 1 });
        t.take();
        go(&mut t, captain, 2);
        assert_eq!(messages_to(&t.take(), 1), 1);
        assert!(t.world.instances().all(|i| i.id <= 0));

        // Only the leader may ask.
        t.talk(2, captain);
        t.bypass(2, "Script Kamaloka 2");
        assert_eq!(messages_to(&t.take(), 2), 1);
        assert!(t.world.instances().all(|i| i.id <= 0));
    }

    #[test]
    fn a_member_who_may_not_re_enter_yet_stops_the_party() {
        let (mut t, captain) = setup();
        party(&mut t, LEVEL[0]);
        // Template 58 has the name of 57: the time of the other Hall of the Abyss counts.
        let later = now() + 60_000;
        t.world.set_instance_time(Oid(2), 58, later);
        go(&mut t, captain, 0);
        assert_eq!(messages_to(&t.take(), 1), 1);
        assert!(t.world.instances().all(|i| i.id <= 0));

        // The Labyrinth has another name: not affected.
        t.world.set_instance_time(Oid(2), 58, 0);
        go(&mut t, captain, 0);
        assert_eq!(t.world.instance_id_of(Oid(1)), 300_000);
    }

    #[test]
    fn the_rooms_weaken_the_boss_and_the_boss_ends_the_instance_with_a_re_entry_time() {
        let (mut t, captain) = setup();
        party(&mut t, LEVEL[2]);
        go(&mut t, captain, 2);
        let id = 300_000;

        // The shaman falls: the minions stop respawning, the room is over.
        let shaman = state(&mut t, |s| s.worlds[&id].shaman);
        let spawns = state(&mut t, |s| s.worlds[&id].first_room.clone().unwrap());
        kill(&mut t, Oid(shaman), 22486);
        assert!(spawns.iter().all(|s| !t.world.is_respawn_enabled(*s)));
        assert!(state(&mut t, |s| s.worlds[&id].first_room.is_none()));
        assert_eq!(state(&mut t, |s| s.worlds[&id].shaman), 0);

        // A minion of the first room is no shaman: nothing happens.
        let minion = npcs_of(&t, id, 22486).into_iter().find(|n| n.0 != shaman).unwrap();
        kill(&mut t, minion, 22486);

        // The second room: only the fifth kill ends it.
        let room = npcs_of(&t, id, 22487);
        for (i, mob) in room.iter().enumerate() {
            assert!(state(&mut t, |s| s.worlds[&id].second_room.is_some()), "{i}");
            kill(&mut t, *mob, 22487);
        }
        assert!(state(&mut t, |s| s.worlds[&id].second_room.is_none()));

        // The miniboss.
        let mini = npcs_of(&t, id, 25616)[0];
        kill(&mut t, mini, 25616);
        assert_eq!(state(&mut t, |s| s.worlds[&id].mini_boss), 0);

        // The boss: the re-entry time (next 06:30) for both, the message, five minutes left.
        let boss = npcs_of(&t, id, 29129)[0];
        t.take();
        kill(&mut t, boss, 29129);
        assert_eq!(state(&mut t, |s| s.worlds[&id].boss), None);
        let now = now();
        let out = t.take();
        for player in [1, 2] {
            let time = t.world.instance_time(Oid(player), 73);
            assert!(time > now && time <= now + 25 * 3_600_000, "{time} {now}");
            assert_eq!(messages_to(&out, player), 1);
        }
        let instance = t.world.instance(id).unwrap();
        assert_eq!(instance.empty_destroy_time_ms, 0);
        assert!(instance.end_time_ms >= now + 5 * 60_000);
        assert!(instance.end_time_ms <= now + 5 * 60_000 + 5_000);
    }

    #[test]
    fn the_escape_teleporter_takes_the_party_out_for_its_leader_only() {
        let (mut t, captain) = setup();
        party(&mut t, LEVEL[2]);
        go(&mut t, captain, 2);
        t.input(1, ClientInput::Appearing);
        t.input(2, ClientInput::Appearing);
        let id = 300_000;
        let exit = npcs_of(&t, id, TELEPORTER)[0];
        // Put the party next to the teleporter, as if they had walked there.
        let (ex, ey, ez) = t.world.position_of(exit).unwrap();
        for p in [1, 2] {
            t.world.tele_to_location(
                Oid(p),
                l2_world::TeleportTarget::at(ex + 20, ey, ez).with_instance(id),
            );
            t.ticks(60);
            t.input(p, ClientInput::Appearing);
        }
        t.take();

        // The leader gets the leader's page, the member the other one.
        t.talk(1, exit);
        assert!(pages_to(&t.take(), 1).last().is_some_and(|p| p.contains("32496.htm")));
        t.talk(2, exit);
        assert!(pages_to(&t.take(), 2).last().is_some_and(|p| p.contains("32496-no.htm")));

        // The member's bypass does nothing; the leader's takes both out to the exit point.
        t.bypass(2, "Script Kamaloka 2");
        t.ticks(60);
        assert_eq!(t.world.instance_id_of(Oid(2)), id);
        t.world.run_as_script("Kamaloka", |ctx| {
            let npc = ScriptNpc {
                object_id: exit,
                id: TELEPORTER,
            };
            let page = Kamaloka.on_talk(ctx, npc, Oid(2));
            assert_eq!(page, None);
            Kamaloka.on_talk(ctx, npc, Oid(1));
        });
        t.ticks(60);
        assert_eq!(t.world.instance_id_of(Oid(1)), 0);
        assert_eq!(t.world.instance_id_of(Oid(2)), 0);
    }

    #[test]
    fn a_captain_answers_with_its_own_page_and_a_bad_event_is_ignored() {
        let (mut t, captain) = setup();
        t.enter(1, LEVEL[0], vec![]);
        t.world.run_as_script("Kamaloka", |ctx| {
            let npc = ScriptNpc {
                object_id: captain,
                id: CAPTAIN,
            };
            assert_eq!(Kamaloka.on_talk(ctx, npc, Oid(1)).as_deref(), Some("30332.htm"));
            assert_eq!(Kamaloka.on_event(ctx, "x", Some(npc), Some(Oid(1))), None);
            // Index out of range: Java's `ArrayIndexOutOfBoundsException` is caught.
            assert_eq!(Kamaloka.on_event(ctx, "19", Some(npc), Some(Oid(1))), None);
            assert_eq!(Kamaloka.on_event(ctx, "-1", Some(npc), Some(Oid(1))), None);
        });
        assert!(t.world.instances().all(|i| i.id <= 0));
    }
}
