//! Dark Cloud Mansion (instance 9): six rooms between Beleth's minions, the monoliths, the shadow
//! columns and Beleth's samples, and the Symbol of Truth that gives the Contaminated Crystal.
// Java: dist/game/data/scripts/instances/DarkCloudMansion/DarkCloudMansion.java
//
// Java keeps the rooms as `DMCRoom` objects in the `InstanceWorld` parameters; here they are the
// script's state by instance id ([`Mansion`]) and the NPCs in them are object ids. The progress
// (`world.getStatus()`) is the `InstanceWorld`'s.

use std::collections::HashMap;

use l2_protocol_game::NpcStringId;
use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{ChatType, SystemMessage, SystemMessageParam};
use l2_util::ObjectId;
use l2_world::effects::SkillKey;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const YIYEN: i32 = 32282;
const SO_FAITH: i32 = 32288; // Symbol of Faith
const SO_ADVERSITY: i32 = 32289; // Symbol of Adversity
const SO_ADVENTURE: i32 = 32290; // Symbol of Adventure
const SO_TRUTH: i32 = 32291; // Symbol of Truth
const BSM: i32 = 32324; // Black Stone Monolith
const SC: i32 = 22402; // Shadow Column

// Mobs
const CCG: [i32; 2] = [18369, 18370]; // Chromatic Crystal Golem
const BM: [i32; 3] = [22272, 22273, 22274]; // Beleth's Minions
const HG: [i32; 2] = [22264, 22264]; // [22318,22319] #Hall Guards
const BS: [i32; 7] = [18371, 18372, 18373, 18374, 18375, 18376, 18377]; // Beleth's Samples
const TOKILL: [i32; 14] = [
    18371, 18372, 18373, 18374, 18375, 18376, 18377, // 22318,
    // 22319,
    22272, 22273, 22274, 18369, 18370, 22402, 22264,
];

// Items
const CC: i32 = 9690; // Contaminated Crystal

// Misc
const TEMPLATE_ID: i32 = 9;
const D1: i32 = 24230001; // Starting Room
const D2: i32 = 24230002; // First Room
const D3: i32 = 24230005; // Second Room
const D4: i32 = 24230003; // Third Room
const D5: i32 = 24230004; // Forth Room
const D6: i32 = 24230006; // Fifth Room
const W1: i32 = 24230007; // Wall 1
const NO_RND_WALK: bool = true;
const ENTER: (i32, i32, i32) = (146534, 180464, -6117);
const EXIT: (i32, i32, i32) = (139968, 150367, -3111);
const SPAWN_CHAT: [NpcStringId; 5] = [
    NpcStringId::IMTheRealOne,
    NpcStringId::PickMe,
    NpcStringId::TrustMe,
    NpcStringId::NotThatDudeIMTheRealOne,
    NpcStringId::DonTBeFooledDonTBeFooledIMTheRealOne,
];
const DECAY_CHAT: [NpcStringId; 3] = [
    NpcStringId::IMTheRealOnePhew,
    NpcStringId::CanTYouEvenFindOut,
    NpcStringId::FindMe,
];
const SUCCESS_CHAT: [NpcStringId; 4] = [
    NpcStringId::HuhHowDidYouKnowItWasMe,
    NpcStringId::ExcellentChoiceTeehee,
    NpcStringId::YouVeDoneWell,
    NpcStringId::OhVerySensible,
];
const FAILD_CHAT: [NpcStringId; 2] = [NpcStringId::YouVeBeenFooled, NpcStringId::SorryButIMTheFakeOne];

// Second room - random monolith order
const MONOLITH_ORDER: [[i32; 6]; 15] = [
    [1, 2, 3, 4, 5, 6],
    [6, 5, 4, 3, 2, 1],
    [4, 5, 6, 3, 2, 1],
    [2, 6, 3, 5, 1, 4],
    [4, 1, 5, 6, 2, 3],
    [3, 5, 1, 6, 2, 4],
    [6, 1, 3, 4, 5, 2],
    [5, 6, 1, 2, 4, 3],
    [5, 2, 6, 3, 4, 1],
    [1, 5, 2, 6, 3, 4],
    [1, 2, 3, 6, 5, 4],
    [6, 4, 3, 1, 5, 2],
    [3, 5, 2, 4, 1, 6],
    [3, 2, 4, 5, 1, 6],
    [5, 4, 3, 1, 6, 2],
];

// Second room - golem spawn locatons - random
const GOLEM_SPAWN: [(i32, i32, i32); 7] = [
    (CCG[0], 148060, 181389),
    (CCG[1], 147910, 181173),
    (CCG[0], 147810, 181334),
    (CCG[1], 147713, 181179),
    (CCG[0], 147569, 181410),
    (CCG[1], 147810, 181517),
    (CCG[0], 147805, 181281),
];

// forth room - random shadow column
const COLUMN_ROWS: [[i32; 5]; 5] = [
    [1, 1, 0, 1, 0],
    [0, 1, 1, 0, 1],
    [1, 0, 1, 1, 0],
    [0, 1, 0, 1, 1],
    [1, 0, 1, 0, 1],
];

// Fifth room - beleth order
const BELETHS: [[i32; 7]; 9] = [
    [1, 0, 1, 0, 1, 0, 0],
    [0, 0, 1, 0, 1, 1, 0],
    [0, 0, 0, 1, 0, 1, 1],
    [1, 0, 1, 1, 0, 0, 0],
    [1, 1, 0, 0, 0, 1, 0],
    [0, 1, 0, 1, 0, 1, 0],
    [0, 0, 0, 1, 1, 1, 0],
    [1, 0, 1, 0, 0, 1, 0],
    [0, 1, 1, 0, 0, 0, 1],
];

/// Java `DMCNpc`.
#[derive(Debug)]
struct DmcNpc {
    npc: ObjectId,
    is_dead: bool,
    golem: Option<ObjectId>,
    status: i32,
    order: i32,
    count: i32,
}

impl DmcNpc {
    fn new(npc: ObjectId) -> Self {
        Self {
            npc,
            is_dead: false,
            golem: None,
            status: 0,
            order: 0,
            count: 0,
        }
    }
}

/// Java `DMCRoom`.
#[derive(Debug, Default)]
struct DmcRoom {
    npc_list: Vec<DmcNpc>,
    counter: i32,
    reset: i32,
    founded: i32,
    order: [i32; 7],
}

/// The names of the `InstanceWorld` parameters that hold a room.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Slot {
    StartRoom,
    Hall,
    FirstRoom,
    SecondRoom,
    ThirdRoom,
    ThirdRoom2,
    ForthRoom,
    FifthRoom,
}

/// What Java keeps in the parameters of the `InstanceWorld`.
#[derive(Debug, Default)]
struct State {
    worlds: HashMap<i32, HashMap<Slot, DmcRoom>>,
}

impl State {
    fn room(&mut self, instance_id: i32, slot: Slot) -> Option<&mut DmcRoom> {
        self.worlds.get_mut(&instance_id)?.get_mut(&slot)
    }

    fn set(&mut self, instance_id: i32, slot: Slot, room: DmcRoom) {
        self.worlds.entry(instance_id).or_default().insert(slot, room);
    }

    fn remove(&mut self, instance_id: i32, slot: Slot) {
        if let Some(rooms) = self.worlds.get_mut(&instance_id) {
            rooms.remove(&slot);
        }
    }
}

/// Java `instances.DarkCloudMansion.DarkCloudMansion`.
pub struct DarkCloudMansion;

impl Script for DarkCloudMansion {
    fn name(&self) -> &'static str {
        "DarkCloudMansion"
    }

    fn path(&self) -> &'static str {
        "instances/DarkCloudMansion"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_first_talk_id(&[BSM, SO_TRUTH]);
        s.add_start_npc(&[YIYEN]);
        s.add_talk_id(&[YIYEN, SO_TRUTH]);
        s.add_attack_id(&[SC]);
        s.add_attack_id(&BS);
        s.add_attack_id(&CCG);
        s.add_kill_id(&TOKILL);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        _player: Option<ObjectId>,
    ) -> Option<String> {
        let Some(npc) = npc else {
            return Some(String::new());
        };
        let Some(instance_id) = world_of_event(ctx, npc) else {
            return Some(String::new());
        };

        let has_fifth = ctx.state::<State>().room(instance_id, Slot::FifthRoom).is_some();
        if has_fifth {
            if event.eq_ignore_ascii_case("decayMe") {
                let mut decayed = Vec::new();
                if let Some(fifth) = ctx.state::<State>().room(instance_id, Slot::FifthRoom) {
                    let done = fifth.reset == 0 && fifth.founded == 3;
                    for mob in &mut fifth.npc_list {
                        if mob.npc == npc.object_id || done {
                            decayed.push(mob.npc);
                            mob.count = 2;
                        }
                    }
                }
                for mob in decayed {
                    ctx.world_mut().decay(mob);
                }

                let done = ctx
                    .state::<State>()
                    .room(instance_id, Slot::FifthRoom)
                    .is_some_and(|f| f.reset == 0 && f.founded == 3);
                if done {
                    end_instance(ctx, instance_id);
                }
            } else if event.eq_ignore_ascii_case("decayBelethSamples") {
                let mut decayed = Vec::new();
                if let Some(fifth) = ctx.state::<State>().room(instance_id, Slot::FifthRoom) {
                    for mob in &mut fifth.npc_list {
                        if mob.count == 0 {
                            decayed.push(mob.npc);
                            mob.count = 2;
                        }
                    }
                }
                for mob in decayed {
                    ctx.world_mut().decay(mob);
                }
            } else if event.eq_ignore_ascii_case("decayChatBelethSamples") {
                let talkers: Vec<ObjectId> = ctx
                    .state::<State>()
                    .room(instance_id, Slot::FifthRoom)
                    .map(|f| {
                        f.npc_list
                            .iter()
                            .filter(|m| m.status == 1)
                            .map(|m| m.npc)
                            .collect()
                    })
                    .unwrap_or_default();
                for mob in talkers {
                    say(ctx, mob, ChatType::NpcGeneral, &DECAY_CHAT);
                }
            } else if event.eq_ignore_ascii_case("respawnFifth") {
                spawn_fifth_room(ctx, instance_id);
            }
        }

        Some(String::new())
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, _killer: ObjectId, _is_summon: bool) {
        let Some(instance_id) = world_of(ctx, npc) else {
            return;
        };

        if status_of(ctx, instance_id) == 0 && check_kill_progress(ctx, instance_id, Slot::StartRoom, npc) {
            run_hall(ctx, instance_id);
        }

        if status_of(ctx, instance_id) == 1 && check_kill_progress(ctx, instance_id, Slot::Hall, npc) {
            run_first_room(ctx, instance_id);
        }

        if status_of(ctx, instance_id) == 2 && check_kill_progress(ctx, instance_id, Slot::FirstRoom, npc) {
            run_hall2(ctx, instance_id);
        }

        if status_of(ctx, instance_id) == 3 && check_kill_progress(ctx, instance_id, Slot::Hall, npc) {
            run_second_room(ctx, instance_id);
        }

        if status_of(ctx, instance_id) == 4 {
            if let Some(second) = ctx.state::<State>().room(instance_id, Slot::SecondRoom) {
                for mob in &mut second.npc_list {
                    if mob.golem == Some(npc.object_id) {
                        mob.golem = None;
                    }
                }
            }
        }

        if status_of(ctx, instance_id) == 5 && check_kill_progress(ctx, instance_id, Slot::Hall, npc) {
            run_third_room(ctx, instance_id);
        }

        if status_of(ctx, instance_id) == 6 && check_kill_progress(ctx, instance_id, Slot::ThirdRoom, npc) {
            run_forth_room(ctx, instance_id);
        }

        if status_of(ctx, instance_id) == 7 {
            chk_shadow_column(ctx, instance_id, npc);
        }

        if status_of(ctx, instance_id) == 8 && check_kill_progress(ctx, instance_id, Slot::ThirdRoom2, npc) {
            run_fifth_room(ctx, instance_id);
        }

        if status_of(ctx, instance_id) == 9 {
            killed_beleth_sample(ctx, instance_id, npc);
        }
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
        let Some(instance_id) = world_of(ctx, npc) else {
            return;
        };

        if status_of(ctx, instance_id) == 7 {
            let matching = ctx
                .state::<State>()
                .room(instance_id, Slot::ForthRoom)
                .map_or(0, |r| {
                    r.npc_list.iter().filter(|m| m.npc == npc.object_id).count()
                });
            for _ in 0..matching {
                if ctx.world().npc_core_ai_disabled(npc.object_id) {
                    ctx.disable_core_ai(npc, false);
                    ctx.set_intention_attack(npc, attacker);
                }

                if ctx.world().is_invul(npc.object_id) && ctx.get_random(100) < 12 {
                    if let Some(minion) = ctx.get_random_entry(&BM) {
                        let (x, y, z) = ctx.world().position_of(attacker).unwrap_or_default();
                        ctx.add_spawn(minion, x, y, z, 0, false, 0, instance_id);
                    }
                }
            }
        }

        if status_of(ctx, instance_id) == 9 {
            check_beleth_sample(ctx, instance_id, npc, attacker);
        }
    }

    fn on_first_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        if let Some(instance_id) = world_of(ctx, npc) {
            if status_of(ctx, instance_id) == 4 {
                let mob_index = ctx
                    .state::<State>()
                    .room(instance_id, Slot::SecondRoom)
                    .and_then(|r| r.npc_list.iter().position(|m| m.npc == npc.object_id));
                if let Some(index) = mob_index {
                    check_stone(ctx, instance_id, npc, index);
                }

                if all_stones_done(ctx, instance_id) {
                    remove_monoliths(ctx, instance_id);
                    run_hall3(ctx, instance_id);
                }
            }

            if npc.id == SO_TRUTH && status_of(ctx, instance_id) == 10 {
                ctx.show_chat_window(npc, player);

                if !ctx.has_quest_items(player, CC) {
                    ctx.give_items(player, CC, 1);
                }
            }
        }

        Some(String::new())
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let npc_id = npc.id;
        if npc_id == YIYEN {
            enter_instance(ctx, player);
        } else {
            let Some(instance_id) = world_of(ctx, npc) else {
                return Some(String::new());
            };

            if npc_id == SO_TRUTH {
                if let Some(world) = ctx.world_mut().instance_world_mut(instance_id) {
                    if world.is_allowed(player) {
                        world.remove_allowed(player);
                    }
                }

                ctx.teleport_player(player, EXIT.0, EXIT.1, EXIT.2, 0, true);
                let instance_id = ctx.world().instance_id_of(npc.object_id);
                if ctx
                    .world()
                    .instance(instance_id)
                    .is_some_and(|i| i.players.is_empty())
                {
                    ctx.world_mut().destroy_instance(instance_id);
                }

                return Some(String::new());
            }
        }

        Some(String::new())
    }
}

/// `InstanceManager.getWorld(npc)`: the instance id of the script world of the NPC.
fn world_of(ctx: &ScriptCtx<'_>, npc: ScriptNpc) -> Option<i32> {
    ctx.world()
        .instance_world_of(npc.object_id)
        .map(|w| w.instance_id)
}

/// [`world_of`] for a timer: the NPC may be gone from the world table by then (a sample that
/// died or was decayed), but Java's object still knows its instance, so the rooms are searched.
fn world_of_event(ctx: &mut ScriptCtx<'_>, npc: ScriptNpc) -> Option<i32> {
    let found = world_of(ctx, npc).or_else(|| {
        ctx.state::<State>()
            .worlds
            .iter()
            .find(|(_, rooms)| {
                rooms
                    .values()
                    .any(|r| r.npc_list.iter().any(|m| m.npc == npc.object_id))
            })
            .map(|(id, _)| *id)
    })?;
    ctx.world().instance_world(found).map(|w| w.instance_id)
}

/// `world.getStatus()`.
fn status_of(ctx: &ScriptCtx<'_>, instance_id: i32) -> i32 {
    ctx.world().instance_world(instance_id).map_or(0, |w| w.status())
}

/// `world.setStatus(status)`.
fn set_status(ctx: &mut ScriptCtx<'_>, instance_id: i32, status: i32) {
    if let Some(world) = ctx.world_mut().instance_world_mut(instance_id) {
        world.set_status(status);
    }
}

/// `addSpawn(npcId, x, y, z, heading, false, 0, false, instanceId)` for a mob that keeps its place
/// (`noRndWalk`).
fn spawn(
    ctx: &mut ScriptCtx<'_>,
    npc_id: i32,
    x: i32,
    y: i32,
    z: i32,
    heading: i32,
    instance_id: i32,
) -> Option<ObjectId> {
    ctx.add_spawn(npc_id, x, y, z, heading, false, 0, instance_id)
        .map(|n| n.object_id)
}

/// `thisNpc.npc = addSpawn(..); if (noRndWalk) thisNpc.npc.setRandomWalking(false);`
fn spawn_walker(
    ctx: &mut ScriptCtx<'_>,
    npc_id: i32,
    x: i32,
    y: i32,
    z: i32,
    instance_id: i32,
) -> Option<DmcNpc> {
    let npc = spawn(ctx, npc_id, x, y, z, 0, instance_id)?;
    if NO_RND_WALK {
        ctx.world_mut().set_npc_random_walking(npc, false);
    }

    Some(DmcNpc::new(npc))
}

/// `npc.broadcastPacket(new NpcSay(npc.getObjectId(), chat, npc.getId(), getRandomEntry(texts)))`.
fn say(ctx: &mut ScriptCtx<'_>, npc: ObjectId, chat: ChatType, texts: &[NpcStringId]) {
    let Some(npc) = ctx.script_npc(npc) else {
        return;
    };
    if let Some(text) = ctx.get_random_entry(texts) {
        ctx.broadcast_npc_say(npc, chat, text, &[]);
    }
}

/// `checkConditions(player)`.
fn check_conditions(world: &l2_world::World, player: ObjectId) -> bool {
    let Some(party) = world.party_of(player) else {
        world.send(
            player,
            &SystemMessage::new(SystemMessageId::YouAreNotCurrentlyInAPartySoYouCannotEnter),
        );
        return false;
    };

    if world.party_leader(party) != Some(player) {
        world.send(
            player,
            &SystemMessage::new(SystemMessageId::OnlyAPartyLeaderCanMakeTheRequestToEnter),
        );
        return false;
    }

    let members = world.party_members(party);
    if members.len() > 2 {
        world.send(
            player,
            &SystemMessage::new(SystemMessageId::YouCannotEnterDueToThePartyHavingExceededTheLimit),
        );
        return false;
    }

    for member in members {
        let (name, level) = world
            .player(member)
            .map(|p| (p.data.name.clone(), p.core.level))
            .unwrap_or_default();
        if level < 78 {
            world.send(
                player,
                &SystemMessage::new(SystemMessageId::C1SLevelDoesNotCorrespondToTheRequirementsForEntry)
                    .with(SystemMessageParam::PlayerName(name)),
            );
            return false;
        }

        if !within_3d(world, member, player, 1000.0) {
            world.send(
                player,
                &SystemMessage::new(
                    SystemMessageId::C1IsInALocationWhichCannotBeEnteredThereforeItCannotBeProcessed,
                )
                .with(SystemMessageParam::PlayerName(name)),
            );
            return false;
        }
    }

    true
}

/// `a.isInsideRadius3D(b, radius)`.
fn within_3d(world: &l2_world::World, a: ObjectId, b: ObjectId, radius: f64) -> bool {
    match (world.position_of(a), world.position_of(b)) {
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

/// Java `enterInstance(player, TEMPLATE_ID)` with this script's `onEnterInstance`.
fn enter_instance(ctx: &mut ScriptCtx<'_>, player: ObjectId) {
    let mut entered: Option<(i32, bool)> = None;
    ctx.world_mut().enter_instance(
        player,
        TEMPLATE_ID,
        |world, player| check_conditions(world, player),
        |_, _, instance_id, first| entered = Some((instance_id, first)),
    );
    // The template has no enter location and no buff removal, so the rest of `enterInstance`
    // has nothing to do between the world's making and `onEnterInstance`.
    let Some((instance_id, first)) = entered else {
        return;
    };

    if first {
        run_start_room(ctx, instance_id);
        for member in ctx.world().party_members_of(player) {
            // `if (partyMember.getQuestState(getName()) == null) newQuestState(partyMember)`
            let _ = ctx.get_quest_state(member, true);

            if let Some(world) = ctx.world_mut().instance_world_mut(instance_id) {
                world.add_allowed(member);
            }
            ctx.teleport_player(member, ENTER.0, ENTER.1, ENTER.2, instance_id, true);
        }
    } else {
        ctx.teleport_player(player, ENTER.0, ENTER.1, ENTER.2, instance_id, true);
    }
}

fn run_start_room(ctx: &mut ScriptCtx<'_>, instance_id: i32) {
    set_status(ctx, instance_id, 0);
    let mut start_room = DmcRoom::default();
    start_room
        .npc_list
        .extend(spawn_walker(ctx, BM[0], 146817, 180335, -6117, instance_id));
    start_room
        .npc_list
        .extend(spawn_walker(ctx, BM[0], 146741, 180589, -6117, instance_id));
    ctx.state::<State>().set(instance_id, Slot::StartRoom, start_room);
}

fn spawn_hall(ctx: &mut ScriptCtx<'_>, instance_id: i32) {
    let mut hall = DmcRoom::default();
    // remove room instance to avoid adding mob every time
    ctx.state::<State>().remove(instance_id, Slot::Hall);
    for (npc_id, x, y) in [
        (BM[1], 147217, 180112),
        (BM[2], 147217, 180209),
        (BM[1], 148521, 180112),
        (BM[0], 148521, 180209),
        (BM[1], 148525, 180910),
        (BM[2], 148435, 180910),
        (BM[1], 147242, 180910),
        (BM[2], 147242, 180819),
    ] {
        hall.npc_list
            .extend(spawn_walker(ctx, npc_id, x, y, -6117, instance_id));
    }

    ctx.state::<State>().set(instance_id, Slot::Hall, hall);
}

fn run_hall(ctx: &mut ScriptCtx<'_>, instance_id: i32) {
    spawn_hall(ctx, instance_id);
    set_status(ctx, instance_id, 1);
    ctx.world_mut().instance_open_door(instance_id, D1);
}

fn run_first_room(ctx: &mut ScriptCtx<'_>, instance_id: i32) {
    let mut first_room = DmcRoom::default();
    for (npc_id, x, y) in [
        (HG[1], 147842, 179837),
        (HG[0], 147711, 179708),
        (HG[1], 147842, 179552),
        (HG[0], 147964, 179708),
    ] {
        first_room
            .npc_list
            .extend(spawn_walker(ctx, npc_id, x, y, -6117, instance_id));
    }

    ctx.state::<State>().set(instance_id, Slot::FirstRoom, first_room);
    set_status(ctx, instance_id, 2);
    ctx.world_mut().instance_open_door(instance_id, D2);
}

fn run_hall2(ctx: &mut ScriptCtx<'_>, instance_id: i32) {
    spawn(ctx, SO_FAITH, 147818, 179643, -6117, 0, instance_id);
    spawn_hall(ctx, instance_id);
    set_status(ctx, instance_id, 3);
}

fn run_second_room(ctx: &mut ScriptCtx<'_>, instance_id: i32) {
    let mut second_room = DmcRoom::default();

    // TODO: find a better way to initialize to [1,0,0,0,0,0,0]
    second_room.order = [1, 0, 0, 0, 0, 0, 0];

    let i = usize::try_from(ctx.get_random(MONOLITH_ORDER.len() as i32)).unwrap_or(0);
    for (index, (x, y)) in [
        (147800, 181150),
        (147900, 181215),
        (147900, 181345),
        (147800, 181410),
        (147700, 181345),
        (147700, 181215),
    ]
    .into_iter()
    .enumerate()
    {
        if let Some(npc) = spawn(ctx, BSM, x, y, -6117, 0, instance_id) {
            let mut this_npc = DmcNpc::new(npc);
            this_npc.order = MONOLITH_ORDER[i][index];
            second_room.npc_list.push(this_npc);
        }
    }

    ctx.state::<State>()
        .set(instance_id, Slot::SecondRoom, second_room);
    set_status(ctx, instance_id, 4);
    ctx.world_mut().instance_open_door(instance_id, D3);
}

fn run_hall3(ctx: &mut ScriptCtx<'_>, instance_id: i32) {
    spawn(ctx, SO_ADVERSITY, 147808, 181281, -6117, 16383, instance_id);
    spawn_hall(ctx, instance_id);
    set_status(ctx, instance_id, 5);
}

/// The six minions of the third room.
///
/// NOTE(java-quirk): Java makes one `DMCNpc` and overwrites its `npc` for each of the six spawns,
/// then adds the same object six times, so the room tracks only the last minion (BM[2] at
/// 148865,180705): the room is done when that one dies and the other five do not count.
fn spawn_third_room_minions(ctx: &mut ScriptCtx<'_>, instance_id: i32) -> DmcRoom {
    let mut third_room = DmcRoom::default();
    let mut last = None;
    for (npc_id, x, y) in [
        (BM[1], 148765, 180450),
        (BM[2], 148865, 180190),
        (BM[1], 148995, 180190),
        (BM[0], 149090, 180450),
        (BM[1], 148995, 180705),
        (BM[2], 148865, 180705),
    ] {
        if let Some(npc) = spawn_walker(ctx, npc_id, x, y, -6117, instance_id) {
            last = Some(npc);
        }
    }
    // Six list entries of one object are one entry for every loop over the list.
    third_room.npc_list.extend(last);
    third_room
}

fn run_third_room(ctx: &mut ScriptCtx<'_>, instance_id: i32) {
    let third_room = spawn_third_room_minions(ctx, instance_id);
    ctx.state::<State>().set(instance_id, Slot::ThirdRoom, third_room);
    set_status(ctx, instance_id, 6);
    ctx.world_mut().instance_open_door(instance_id, D4);
}

fn run_third_room2(ctx: &mut ScriptCtx<'_>, instance_id: i32) {
    spawn(ctx, SO_ADVENTURE, 148910, 178397, -6117, 16383, instance_id);
    let third_room = spawn_third_room_minions(ctx, instance_id);
    ctx.state::<State>()
        .set(instance_id, Slot::ThirdRoom2, third_room);
    set_status(ctx, instance_id, 8);
}

fn run_forth_room(ctx: &mut ScriptCtx<'_>, instance_id: i32) {
    let mut forth_room = DmcRoom::default();
    let mut temp = [0usize; 7];
    for t in &mut temp {
        *t = usize::try_from(ctx.get_random(COLUMN_ROWS.len() as i32)).unwrap_or(0);
    }

    let mut templist = [[0i32; 5]; 7];
    for (i, row) in templist.iter_mut().enumerate() {
        *row = COLUMN_ROWS[temp[i]];
    }

    let mut xx = 0;
    let mut x = 148660;
    while x < 149285 {
        let mut yy = 0;
        let mut y = 179280;
        while y > 178405 {
            if let Some(npc) = spawn(ctx, SC, x, y, -6115, 16215, instance_id) {
                if let Some(script_npc) = ctx.script_npc(npc) {
                    ctx.disable_core_ai(script_npc, true);
                }
                let mut this_npc = DmcNpc::new(npc);
                this_npc.status = templist[yy][xx];
                this_npc.order = yy as i32;
                forth_room.npc_list.push(this_npc);
            }
            yy += 1;
            y -= 125;
        }

        xx += 1;
        x += 125;
    }

    // TODO: unify this into previous loop
    for npc in &forth_room.npc_list {
        if npc.status == 0 {
            ctx.world_mut().set_invul(npc.npc, true);
        }
    }

    ctx.state::<State>().set(instance_id, Slot::ForthRoom, forth_room);
    set_status(ctx, instance_id, 7);
    ctx.world_mut().instance_open_door(instance_id, D5);
}

fn run_fifth_room(ctx: &mut ScriptCtx<'_>, instance_id: i32) {
    spawn_fifth_room(ctx, instance_id);
    set_status(ctx, instance_id, 9);
    ctx.world_mut().instance_open_door(instance_id, D6);
}

fn spawn_fifth_room(ctx: &mut ScriptCtx<'_>, instance_id: i32) {
    let mut fifth_room = DmcRoom::default();
    let temp_index = usize::try_from(ctx.get_random(BELETHS.len() as i32)).unwrap_or(0);
    let temp = BELETHS[temp_index];
    fifth_room.reset = 0;
    fifth_room.founded = 0;
    let mut idx = 0;
    let mut x = 148720;
    while x < 149175 {
        if let Some(npc) = spawn(ctx, BS[idx], x, 182145, -6117, 48810, instance_id) {
            ctx.world_mut().set_npc_random_walking(npc, false);
            if let Some(script_npc) = ctx.script_npc(npc) {
                ctx.disable_core_ai(script_npc, true);
            }
            let mut this_npc = DmcNpc::new(npc);
            this_npc.order = idx as i32;
            this_npc.status = temp[idx];
            this_npc.count = 0;
            fifth_room.npc_list.push(this_npc);
            if (temp[idx] == 1 && ctx.get_random(100) < 95) || (temp[idx] != 1 && ctx.get_random(100) < 67) {
                say(ctx, npc, ChatType::General, &SPAWN_CHAT);
            }
        }

        idx += 1;
        x += 65;
    }

    ctx.state::<State>().set(instance_id, Slot::FifthRoom, fifth_room);
}

/// `checkKillProgress(npc, room)`: marks the NPC dead; true when the whole room is.
fn check_kill_progress(ctx: &mut ScriptCtx<'_>, instance_id: i32, slot: Slot, npc: ScriptNpc) -> bool {
    // A room that is not there is a `NullPointerException` in Java; nothing happens here.
    let Some(room) = ctx.state::<State>().room(instance_id, slot) else {
        return false;
    };
    let mut cont = true;
    for npcobj in &mut room.npc_list {
        if npcobj.npc == npc.object_id {
            npcobj.is_dead = true;
        }

        if !npcobj.is_dead {
            cont = false;
        }
    }

    cont
}

fn spawn_rnd_golem(ctx: &mut ScriptCtx<'_>, instance_id: i32, index: usize) {
    let has_golem = ctx
        .state::<State>()
        .room(instance_id, Slot::SecondRoom)
        .and_then(|r| r.npc_list.get(index))
        .is_none_or(|m| m.golem.is_some());
    if has_golem {
        return;
    }

    let i = usize::try_from(ctx.get_random(GOLEM_SPAWN.len() as i32)).unwrap_or(0);
    let (mob_id, x, y) = GOLEM_SPAWN[i];
    let golem = spawn(ctx, mob_id, x, y, -6117, 0, instance_id);
    if NO_RND_WALK {
        if let Some(golem) = golem {
            ctx.world_mut().set_npc_random_walking(golem, false);
        }
    }
    if let Some(mob) = ctx
        .state::<State>()
        .room(instance_id, Slot::SecondRoom)
        .and_then(|r| r.npc_list.get_mut(index))
    {
        mob.golem = golem;
    }
}

fn check_stone(ctx: &mut ScriptCtx<'_>, instance_id: i32, npc: ScriptNpc, index: usize) {
    for i in 1..7 {
        // if there is a non zero value in the precedent step, the sequence is ok
        let hit = {
            let Some(room) = ctx.state::<State>().room(instance_id, Slot::SecondRoom) else {
                return;
            };
            let mob = &mut room.npc_list[index];
            if room.order[i] == 0 && room.order[i - 1] != 0 && mob.order == i as i32 && mob.status == 0 {
                room.order[i] = 1;
                mob.status = 1;
                mob.is_dead = true;
                true
            } else {
                false
            }
        };
        if hit {
            ctx.broadcast_magic_skill_use(npc.object_id, npc.object_id, npc.object_id, 5441, 1, 1, 0);
            return;
        }
    }

    spawn_rnd_golem(ctx, instance_id, index);
}

fn end_instance(ctx: &mut ScriptCtx<'_>, instance_id: i32) {
    set_status(ctx, instance_id, 10);
    spawn(ctx, SO_TRUTH, 148911, 181940, -6117, 16383, instance_id);
    for slot in [
        Slot::StartRoom,
        Slot::Hall,
        Slot::FirstRoom,
        Slot::SecondRoom,
        Slot::ThirdRoom,
        Slot::ForthRoom,
        Slot::FifthRoom,
    ] {
        ctx.state::<State>().remove(instance_id, slot);
    }
}

fn check_beleth_sample(ctx: &mut ScriptCtx<'_>, instance_id: i32, npc: ScriptNpc, player: ObjectId) {
    let count = ctx
        .state::<State>()
        .room(instance_id, Slot::FifthRoom)
        .map_or(0, |f| f.npc_list.len());
    for index in 0..count {
        let Some((mob_npc, mob_count, mob_status)) = ctx
            .state::<State>()
            .room(instance_id, Slot::FifthRoom)
            .and_then(|f| f.npc_list.get(index))
            .map(|m| (m.npc, m.count, m.status))
        else {
            return;
        };
        if mob_npc == npc.object_id {
            if mob_count == 0 {
                if let Some(mob) = ctx
                    .state::<State>()
                    .room(instance_id, Slot::FifthRoom)
                    .and_then(|f| f.npc_list.get_mut(index))
                {
                    mob.count = 1;
                }
                if mob_status == 1 {
                    say(ctx, mob_npc, ChatType::NpcGeneral, &SUCCESS_CHAT);
                    if let Some(fifth) = ctx.state::<State>().room(instance_id, Slot::FifthRoom) {
                        fifth.founded += 1;
                    }
                    ctx.start_quest_timer("decayMe", 1500, Some(npc), Some(player), false);
                } else {
                    if let Some(fifth) = ctx.state::<State>().room(instance_id, Slot::FifthRoom) {
                        fifth.reset = 1;
                    }
                    if let Some(mob) = ctx.script_npc(mob_npc) {
                        ctx.disable_core_ai(mob, false);
                        ctx.set_intention_attack(mob, player);
                    }
                    say(ctx, mob_npc, ChatType::NpcGeneral, &FAILD_CHAT);
                    ctx.start_quest_timer("decayChatBelethSamples", 4000, Some(npc), Some(player), false);
                    ctx.start_quest_timer("decayBelethSamples", 4500, Some(npc), Some(player), false);
                }
            } else {
                return;
            }
        }
    }
}

fn killed_beleth_sample(ctx: &mut ScriptCtx<'_>, instance_id: i32, npc: ScriptNpc) {
    let mut decayed_samples = 0;
    let Some(fifth) = ctx.state::<State>().room(instance_id, Slot::FifthRoom) else {
        return;
    };
    for mob in &mut fifth.npc_list {
        if mob.npc == npc.object_id {
            decayed_samples += 1;
            mob.count = 2;
        } else if mob.count == 2 {
            decayed_samples += 1;
        }
    }

    let (reset, founded) = (fifth.reset, fifth.founded);
    if reset == 1 {
        let mut gone = Vec::new();
        if let Some(fifth) = ctx.state::<State>().room(instance_id, Slot::FifthRoom) {
            for mob in &mut fifth.npc_list {
                if mob.count == 0 || (mob.status == 1 && mob.count != 2) {
                    decayed_samples += 1;
                    gone.push(mob.npc);
                    mob.count = 2;
                }
            }
        }
        for mob in gone {
            ctx.world_mut().decay(mob);
        }

        if decayed_samples == 7 {
            ctx.start_quest_timer("respawnFifth", 6000, Some(npc), None, false);
        }
    } else if reset == 0 && founded == 3 {
        let all: Vec<ObjectId> = ctx
            .state::<State>()
            .room(instance_id, Slot::FifthRoom)
            .map(|f| f.npc_list.iter().map(|m| m.npc).collect())
            .unwrap_or_default();
        for mob in all {
            ctx.world_mut().decay(mob);
        }

        end_instance(ctx, instance_id);
    }
}

fn all_stones_done(ctx: &mut ScriptCtx<'_>, instance_id: i32) -> bool {
    // A missing room is a `NullPointerException` in Java.
    ctx.state::<State>()
        .room(instance_id, Slot::SecondRoom)
        .is_some_and(|r| r.npc_list.iter().all(|m| m.is_dead))
}

fn remove_monoliths(ctx: &mut ScriptCtx<'_>, instance_id: i32) {
    let all: Vec<ObjectId> = ctx
        .state::<State>()
        .room(instance_id, Slot::SecondRoom)
        .map(|r| r.npc_list.iter().map(|m| m.npc).collect())
        .unwrap_or_default();
    for mob in all {
        ctx.world_mut().decay(mob);
    }
}

fn chk_shadow_column(ctx: &mut ScriptCtx<'_>, instance_id: i32, npc: ScriptNpc) {
    let orders: Vec<i32> = ctx
        .state::<State>()
        .room(instance_id, Slot::ForthRoom)
        .map(|r| {
            r.npc_list
                .iter()
                .filter(|m| m.npc == npc.object_id)
                .map(|m| m.order)
                .collect()
        })
        .unwrap_or_default();
    for order in orders {
        for i in 0..7 {
            let hit = {
                let Some(room) = ctx.state::<State>().room(instance_id, Slot::ForthRoom) else {
                    return;
                };
                if order == i && room.counter == i {
                    room.counter += 1;
                    Some(room.counter)
                } else {
                    None
                }
            };
            if let Some(counter) = hit {
                ctx.world_mut().instance_open_door(instance_id, W1 + i);
                if counter == 7 {
                    run_third_room2(ctx, instance_id);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::instances::testing::instance_data;
    use crate::testing::{ScriptTest, X, Y, Z};

    const NAME: &str = "DarkCloudMansion";
    const ITEMS: &str = r#"
        <item id="9690" type="EtcItem" name="Contaminated Crystal"><set name="is_stackable" val="true"/></item>"#;

    fn npcs() -> String {
        let mut text = String::new();
        let monsters = [
            22272, 22273, 22274, 22264, 22402, 18369, 18370, 18371, 18372, 18373, 18374, 18375, 18376, 18377,
        ];
        for id in monsters {
            text += &format!(r#"<npc id="{id}" type="Monster" level="78" name="M{id}"/>"#);
        }
        for id in [YIYEN, SO_FAITH, SO_ADVERSITY, SO_ADVENTURE, SO_TRUTH, BSM] {
            text += &format!(r#"<npc id="{id}" type="Folk" level="70" name="F{id}"/>"#);
        }
        text
    }

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(DarkCloudMansion);
        let pages = [("data/html/default/32291.htm", "<html><body>Truth</body></html>")];
        let mut t = ScriptTest::new(registry, &npcs(), ITEMS, &pages);
        t.world.set_instance_data(instance_data(
            &[(9, "Dark Cloud Mansion")],
            &[(
                9,
                r#"<activityTime val="30" /><exitPoint x="139968" y="150367" z="-3111" />"#,
            )],
        ));
        let yiyen = t.spawn(YIYEN, 20);
        t.enter(1, 78, vec![]);
        t.enter(2, 78, vec![]);
        t.party(1, 2);
        (t, yiyen)
    }

    fn script_npc(t: &ScriptTest, npc: Oid) -> ScriptNpc {
        ScriptNpc {
            object_id: npc,
            id: t.world.npc(npc).unwrap().npc_id,
        }
    }

    fn enter(t: &mut ScriptTest, yiyen: Oid) -> i32 {
        t.world.run_as_script(NAME, |ctx| {
            DarkCloudMansion.on_talk(
                ctx,
                ScriptNpc {
                    object_id: yiyen,
                    id: YIYEN,
                },
                Oid(1),
            );
        });
        t.ticks(60);
        t.world.instance_id_of(Oid(1))
    }

    fn status(t: &ScriptTest, id: i32) -> i32 {
        t.world.instance_world(id).unwrap().status()
    }

    /// The NPCs of a room, with the template ids.
    fn room_npcs(t: &mut ScriptTest, id: i32, slot: Slot) -> Vec<(Oid, i32, i32, i32)> {
        let mut result = Vec::new();
        t.world.run_as_script(NAME, |ctx| {
            if let Some(room) = ctx.state::<State>().room(id, slot) {
                result = room
                    .npc_list
                    .iter()
                    .map(|m| (m.npc, 0, m.order, m.status))
                    .collect();
            }
        });
        for entry in &mut result {
            entry.1 = t.world.npc(entry.0).unwrap().npc_id;
        }
        result
    }

    fn kill(t: &mut ScriptTest, npc: Oid) {
        let npc = script_npc(t, npc);
        t.world.run_as_script(NAME, |ctx| {
            DarkCloudMansion.on_kill(ctx, npc, Oid(1), false);
        });
    }

    fn kill_room(t: &mut ScriptTest, id: i32, slot: Slot) {
        for (npc, ..) in room_npcs(t, id, slot) {
            kill(t, npc);
        }
    }

    fn count_of(t: &ScriptTest, id: i32, template: i32) -> usize {
        t.world
            .instance(id)
            .unwrap()
            .npcs
            .keys()
            .filter(|n| t.world.npc(**n).is_some_and(|n| n.npc_id == template) && t.in_world(**n))
            .count()
    }

    fn first_talk(t: &mut ScriptTest, npc: Oid) {
        let npc = script_npc(t, npc);
        t.world.run_as_script(NAME, |ctx| {
            assert_eq!(
                DarkCloudMansion.on_first_talk(ctx, npc, Oid(1)),
                Some(String::new())
            );
        });
    }

    #[test]
    fn only_a_party_of_two_at_level_78_enters() {
        let (mut t, yiyen) = setup();
        // A third member exceeds the limit.
        t.enter(3, 78, vec![]);
        t.input(
            1,
            l2_world::ClientInput::JoinParty {
                name: t.world.player(Oid(3)).unwrap().data.name.clone(),
                distribution_type_id: 0,
            },
        );
        t.input(3, l2_world::ClientInput::AnswerJoinParty { response: 1 });
        assert!(t.world.is_in_party(Oid(3)));
        assert_eq!(enter(&mut t, yiyen), 0);
        assert_eq!(t.world.instances().filter(|i| i.id > 0).count(), 0);
    }

    #[test]
    fn the_party_enters_and_the_two_minions_of_the_start_room_wait() {
        let (mut t, yiyen) = setup();
        let id = enter(&mut t, yiyen);
        assert_eq!(id, 300_000);
        assert_eq!(t.world.instance_id_of(Oid(2)), id);
        let world = t.world.instance_world(id).unwrap();
        assert_eq!(
            (world.template_id, world.allowed.clone()),
            (9, vec![Oid(1), Oid(2)])
        );
        let offset = t.world.max_offset_on_teleport();
        let (x, y, _) = t.world.position_of(Oid(1)).unwrap();
        assert!((x - ENTER.0).abs() <= offset && (y - ENTER.1).abs() <= offset);
        assert_eq!(status(&t, id), 0);
        assert_eq!(room_npcs(&mut t, id, Slot::StartRoom).len(), 2);
        assert_eq!(count_of(&t, id, BM[0]), 2);
        // Leaving and talking again brings the leader back to the same instance.
        t.world
            .tele_to_location(Oid(1), l2_world::TeleportTarget::at(X, Y, Z).with_instance(0));
        t.ticks(60);
        assert_eq!(enter(&mut t, yiyen), id);
        assert_eq!(t.world.instances().filter(|i| i.id > 0).count(), 1);
    }

    #[test]
    fn the_rooms_open_one_after_the_other_to_the_symbol_of_truth() {
        let (mut t, yiyen) = setup();
        let id = enter(&mut t, yiyen);

        // Start room -> hall -> first room -> hall -> second room.
        kill_room(&mut t, id, Slot::StartRoom);
        assert_eq!(status(&t, id), 1);
        assert_eq!(room_npcs(&mut t, id, Slot::Hall).len(), 8);
        kill_room(&mut t, id, Slot::Hall);
        assert_eq!(status(&t, id), 2);
        assert_eq!(room_npcs(&mut t, id, Slot::FirstRoom).len(), 4);
        kill_room(&mut t, id, Slot::FirstRoom);
        assert_eq!(status(&t, id), 3);
        assert_eq!(count_of(&t, id, SO_FAITH), 1);
        kill_room(&mut t, id, Slot::Hall);
        assert_eq!(status(&t, id), 4);

        // The monoliths: the wrong one calls a golem (only one at a time), the right ones in
        // the order of the sequence 1..6 are lit.
        let monoliths = room_npcs(&mut t, id, Slot::SecondRoom);
        assert_eq!(monoliths.len(), 6);
        let by_order = |order: i32| monoliths.iter().find(|m| m.2 == order).unwrap().0;
        first_talk(&mut t, by_order(3));
        assert_eq!(count_of(&t, id, CCG[0]) + count_of(&t, id, CCG[1]), 1);
        // The same monolith again: it has its golem already.
        first_talk(&mut t, by_order(3));
        assert_eq!(count_of(&t, id, CCG[0]) + count_of(&t, id, CCG[1]), 1);
        // Another monolith out of turn gets a golem of its own.
        first_talk(&mut t, by_order(4));
        assert_eq!(count_of(&t, id, CCG[0]) + count_of(&t, id, CCG[1]), 2);
        for order in 1..=6 {
            first_talk(&mut t, by_order(order));
        }
        // Everything lit: the monoliths go, the Symbol of Adversity and a hall come.
        assert_eq!(status(&t, id), 5);
        assert!(monoliths.iter().all(|m| !t.world.is_object_spawned(m.0)));
        assert_eq!(count_of(&t, id, SO_ADVERSITY), 1);
        kill_room(&mut t, id, Slot::Hall);
        assert_eq!(status(&t, id), 6);

        // Third room: NOTE(java-quirk) only the last minion counts.
        let third = room_npcs(&mut t, id, Slot::ThirdRoom);
        assert_eq!(third.len(), 1);
        assert_eq!(third[0].1, BM[2]);
        kill(&mut t, third[0].0);
        assert_eq!(status(&t, id), 7);

        // Forth room: 35 columns; one of every row of seven in turn opens a wall.
        let columns = room_npcs(&mut t, id, Slot::ForthRoom);
        assert_eq!(columns.len(), 35);
        assert_eq!(columns.iter().filter(|c| c.3 == 0).count() > 0, true);
        for order in 0..7 {
            let column = columns.iter().find(|c| c.2 == order).unwrap();
            kill(&mut t, column.0);
        }
        assert_eq!(status(&t, id), 8);
        assert_eq!(count_of(&t, id, SO_ADVENTURE), 1);
        let third2 = room_npcs(&mut t, id, Slot::ThirdRoom2);
        kill(&mut t, third2[0].0);
        assert_eq!(status(&t, id), 9);

        // Fifth room: seven samples, three are the real ones. A real one found is thanked and
        // goes after 1.5 seconds; the third ends the instance.
        let samples = room_npcs(&mut t, id, Slot::FifthRoom);
        assert_eq!(samples.len(), 7);
        let real: Vec<_> = samples.iter().filter(|s| s.3 == 1).collect();
        assert_eq!(real.len(), 3);
        for sample in &real {
            let npc = script_npc(&t, sample.0);
            t.world.run_as_script(NAME, |ctx| {
                DarkCloudMansion.on_attack(ctx, npc, Oid(1), 1, false, None);
            });
            t.wait_ms(1_600);
        }
        assert_eq!(status(&t, id), 10);
        let truth = t.world.instance_npc_by_template(id, SO_TRUTH).unwrap();

        // The Symbol of Truth gives the crystal once and takes the party out.
        first_talk(&mut t, truth);
        first_talk(&mut t, truth);
        assert_eq!(t.count(1, CC), 1);
        t.world.run_as_script(NAME, |ctx| {
            DarkCloudMansion.on_talk(
                ctx,
                ScriptNpc {
                    object_id: truth,
                    id: SO_TRUTH,
                },
                Oid(1),
            );
        });
        t.ticks(60);
        assert_eq!(t.world.instance_id_of(Oid(1)), 0);
        assert!(!t.world.instance_world(id).unwrap().is_allowed(Oid(1)));
        assert!(t.world.instance_world(id).unwrap().is_allowed(Oid(2)));
    }

    #[test]
    fn a_wrong_sample_wakes_up_and_the_fifth_room_starts_again() {
        let (mut t, yiyen) = setup();
        let id = enter(&mut t, yiyen);
        // The fifth room as the stages leave it.
        t.world.run_as_script(NAME, |ctx| spawn_fifth_room(ctx, id));
        set_status_for_test(&mut t, id, 9);
        let samples = room_npcs(&mut t, id, Slot::FifthRoom);
        let fake = samples.iter().find(|s| s.3 == 0).unwrap().0;
        let npc = script_npc(&t, fake);
        t.world.run_as_script(NAME, |ctx| {
            DarkCloudMansion.on_attack(ctx, npc, Oid(1), 1, false, None);
        });
        // It is awake (core AI on) and `reset` is set; the unopened samples go after 4.5 seconds.
        assert!(!t.world.npc_core_ai_disabled(fake));
        t.wait_ms(4_600);
        let left: Vec<_> = samples
            .iter()
            .filter(|s| t.world.is_object_spawned(s.0))
            .collect();
        assert_eq!(left.len(), 1, "only the awake fake is left");
        // Killing it decays the real ones that were not found and a new room comes in 6 seconds.
        t.world.run_as_script(NAME, |ctx| {
            DarkCloudMansion.on_kill(ctx, npc, Oid(1), false);
        });
        t.wait_ms(6_100);
        let now = room_npcs(&mut t, id, Slot::FifthRoom);
        assert!(now.iter().all(|s| t.world.is_object_spawned(s.0)));
        assert_ne!(
            now.iter().map(|s| s.0).collect::<Vec<_>>(),
            samples.iter().map(|s| s.0).collect::<Vec<_>>()
        );
    }

    fn set_status_for_test(t: &mut ScriptTest, id: i32, status: i32) {
        t.world.instance_world_mut(id).unwrap().set_status(status);
    }
}
