//! The Last Imperial Tomb (instance 136): the guide that takes a command channel in, the four
//! rooms, the wait, Frintezza's movie, the two forms of Scarlet van Halisha, the portraits and
//! demons, the songs and the end.
// Java: dist/game/data/scripts/ai/bosses/Frintezza/LastImperialTomb.java
//
// Java keeps the NPCs and lists in `InstanceWorld` parameters; here they are `TombWorld`, by
// instance id, in the script's state. A timer whose player or NPC is not in a world any more
// ends there (Java throws a NullPointerException in that task).
//
// Not ported (and why):
// - `overheadDummy.setCollisionHeight(600)`: the world has no collision override for an NPC; the
//   dummy only serves the camera.
// - `Quest.teleportPlayer` of `onEnterInstance` is `TeleportTarget` with the instance id.

use l2_protocol_game::NpcStringId;
use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{
    Earthquake, ExShowScreenMessage, MagicSkillCanceled, MagicSkillUse, SkillPosition, SocialAction,
    SpecialCamera, SystemMessage, SystemMessageParam,
};
use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup, SkillKey, TeleportTarget, World};

// NPCs
const GUIDE: i32 = 32011;
const CUBE: i32 = 29061;
const HALL_ALARM: i32 = 18328;
const HALL_KEEPER_CAPTAIN: i32 = 18329;
const HALL_KEEPER_WIZARD: i32 = 18330;
const HALL_KEEPER_GUARD: i32 = 18331;
const HALL_KEEPER_PATROL: i32 = 18332;
const HALL_KEEPER_SUICIDAL_SOLDIER: i32 = 18333;
const DARK_CHOIR_CAPTAIN: i32 = 18334;
const DARK_CHOIR_PRIMA_DONNA: i32 = 18335;
const DARK_CHOIR_LANCER: i32 = 18336;
const DARK_CHOIR_ARCHER: i32 = 18337;
const DARK_CHOIR_WITCH_DOCTOR: i32 = 18338;
const DARK_CHOIR_PLAYER: i32 = 18339;
const DUMMY: i32 = 29052;
const DUMMY2: i32 = 29053;
const FRINTEZZA: i32 = 29045;
const SCARLET1: i32 = 29046;
const SCARLET2: i32 = 29047;
const MONSTERS_FIRST_ROOM: [i32; 5] = [
    HALL_KEEPER_CAPTAIN,
    HALL_KEEPER_WIZARD,
    HALL_KEEPER_GUARD,
    HALL_KEEPER_PATROL,
    HALL_KEEPER_SUICIDAL_SOLDIER,
];
const MONSTERS_SECOND_ROOM: [i32; 5] = [
    DARK_CHOIR_CAPTAIN,
    DARK_CHOIR_PRIMA_DONNA,
    DARK_CHOIR_LANCER,
    DARK_CHOIR_ARCHER,
    DARK_CHOIR_WITCH_DOCTOR,
];
const PORTRAITS: [i32; 2] = [29048, 29049];
const DEMONS: [i32; 2] = [29050, 29051];

// Items
const DEWDROP_OF_DESTRUCTION_ITEM_ID: i32 = 8556;
const FIRST_SCARLET_WEAPON: i32 = 8204;
const SECOND_SCARLET_WEAPON: i32 = 7903;
const FRINTEZZAS_SCROLL: i32 = 8073;

// Doors
const FIRST_ROOM_DOORS: [i32; 8] = [
    17130051, 17130052, 17130053, 17130054, 17130055, 17130056, 17130057, 17130058,
];
const SECOND_ROOM_DOORS: [i32; 10] = [
    17130061, 17130062, 17130063, 17130064, 17130065, 17130066, 17130067, 17130068, 17130069, 17130070,
];
const FIRST_ROUTE_DOORS: [i32; 2] = [17130042, 17130043];
const SECOND_ROUTE_DOORS: [i32; 2] = [17130045, 17130046];

// Skills
const DEWDROP_OF_DESTRUCTION_SKILL_ID: i32 = 2276;
const INTRO_SKILL: SkillKey = SkillKey { id: 5004, level: 1 };
const FIRST_MORPH_SKILL: SkillKey = SkillKey { id: 5017, level: 1 };

/// `SKILL_MSG`.
fn skill_msg(index: i32) -> Option<NpcStringId> {
    match index {
        1 => Some(NpcStringId::RequiemOfHatred),
        2 => Some(NpcStringId::RondoOfSolitude),
        3 => Some(NpcStringId::FreneticToccata),
        4 => Some(NpcStringId::FugueOfJubilation),
        5 => Some(NpcStringId::HypnoticMazurka),
        _ => None,
    }
}

// Locations
const ENTER_TELEPORT: (i32, i32, i32) = (-88015, -141153, -9168);
const ENTER_TELEPORT_FRINTEZZA_ROOM: (i32, i32, i32) = (-87773, -151624, -9168);
const FIRST_ROOM_CENTER: (i32, i32, i32) = (-87904, -141296, -9168);
const SECOND_ROOM_CENTER: (i32, i32, i32) = (-87919, -147013, -9214);

// Spawns: the portrait id, its place and heading, then the place and heading of its demons.
const PORTRAIT_SPAWNS: [[i32; 9]; 4] = [
    [29048, -89381, -153981, -9168, 3368, -89378, -153968, -9168, 3368],
    [
        29048, -86234, -152467, -9168, 37656, -86261, -152492, -9168, 37656,
    ],
    [
        29049, -89342, -152479, -9168, -5152, -89311, -152491, -9168, -5152,
    ],
    [
        29049, -86189, -153968, -9168, 29456, -86217, -153956, -9168, 29456,
    ],
];

// Misc
const TEMPLATE_ID: i32 = 136;
const MIN_PLAYERS: usize = 9;
const MAX_PLAYERS: usize = 27;
const FRINTEZZA_WAIT_TIME: i64 = 10; // minutes
const RANDOM_SONG_INTERVAL: i64 = 90; // seconds
const TIME_BETWEEN_DEMON_SPAWNS: i64 = 20; // seconds
const MAX_DEMONS: usize = 24;

/// The parameters of Java's `InstanceWorld` this script keeps.
#[derive(Debug, Default)]
pub(crate) struct TombWorld {
    pub(crate) frintezza_dummy: Option<ObjectId>,
    pub(crate) overhead_dummy: Option<ObjectId>,
    pub(crate) portrait_dummy1: Option<ObjectId>,
    pub(crate) portrait_dummy3: Option<ObjectId>,
    pub(crate) scarlet_dummy: Option<ObjectId>,
    pub(crate) frintezza: Option<ObjectId>,
    pub(crate) active_scarlet: Option<ObjectId>,
    /// `demons`.
    pub(crate) demons: Vec<ObjectId>,
    /// `portraits`: the portrait and the index of its spawn.
    pub(crate) portraits: Vec<(ObjectId, usize)>,
    /// `isPlayingSong`.
    pub(crate) is_playing_song: bool,
    /// `monstersCount`.
    pub(crate) monsters_count: i32,
    /// `scarletLocation`: x, y, z, heading.
    pub(crate) scarlet_location: (i32, i32, i32, i32),
    /// `newHeading`.
    pub(crate) new_heading: i32,
}

/// The state of the script: one [`TombWorld`] per instance.
#[derive(Debug, Default)]
pub(crate) struct TombState {
    pub(crate) worlds: std::collections::HashMap<i32, TombWorld>,
}

/// Java `ai.bosses.Frintezza.LastImperialTomb`.
pub struct LastImperialTomb;

impl Script for LastImperialTomb {
    fn name(&self) -> &'static str {
        "LastImperialTomb"
    }

    fn path(&self) -> &'static str {
        "ai/bosses/Frintezza"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[GUIDE]);
        s.add_talk_id(&[GUIDE, CUBE]);
        s.add_attack_id(&[SCARLET1]);
        s.add_skill_see_id(&PORTRAITS);
        s.add_kill_id(&[HALL_ALARM]);
        s.add_kill_id(&[DARK_CHOIR_PLAYER]);
        s.add_kill_id(&MONSTERS_FIRST_ROOM);
        s.add_kill_id(&MONSTERS_SECOND_ROOM);
        s.add_kill_id(&[SCARLET2]);
        s.add_kill_id(&PORTRAITS);
        s.add_kill_id(&DEMONS);
        s.add_spawn_id(&[HALL_ALARM, DUMMY, DUMMY2]);
        s.add_spell_finished_id(&[HALL_KEEPER_SUICIDAL_SOLDIER]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        // `InstanceManager.getWorld(player)` or `getWorld(npc)`: the instance of the event.
        let from_npc = matches!(
            event,
            "PLAY_RANDOM_SONG"
                | "SCARLET_FIRST_MORPH"
                | "SCARLET_SECOND_MORPH_CAMERA_8"
                | "SCARLET_SECOND_MORPH_CAMERA_9"
                | "FINISH_CAMERA_1"
                | "FINISH_CAMERA_2"
        );
        let iid = if from_npc {
            npc.and_then(|n| instance_of_object(ctx, n.object_id))
        } else {
            player.and_then(|p| instance_of_player(ctx, p))
        }?;

        match event {
            "FRINTEZZA_INTRO_START" => {
                ctx.start_quest_timer("FRINTEZZA_INTRO_1", 17000, None, player, false);
                ctx.start_quest_timer("FRINTEZZA_INTRO_2", 20000, None, player, false);
                broadcast(
                    ctx,
                    iid,
                    &Earthquake {
                        x: -87784,
                        y: -155083,
                        z: -9087,
                        intensity: 45,
                        duration: 27,
                    },
                );
            }
            "FRINTEZZA_INTRO_1" => {
                for door_id in FIRST_ROOM_DOORS {
                    ctx.world_mut().instance_close_door(iid, door_id);
                }

                for door_id in FIRST_ROUTE_DOORS {
                    ctx.world_mut().instance_close_door(iid, door_id);
                }

                for door_id in SECOND_ROOM_DOORS {
                    ctx.world_mut().instance_close_door(iid, door_id);
                }

                for door_id in SECOND_ROUTE_DOORS {
                    ctx.world_mut().instance_close_door(iid, door_id);
                }

                ctx.add_spawn(CUBE, -87904, -141296, -9168, 0, false, 0, iid);
            }
            "FRINTEZZA_INTRO_2" => {
                let frintezza_dummy = spawn_id(ctx, DUMMY, (-87784, -155083, -9087), 16048, iid);
                let overhead_dummy = spawn_id(ctx, DUMMY, (-87784, -153298, -9175), 16384, iid);
                let portrait_dummy1 = spawn_id(ctx, DUMMY, (-89566, -153168, -9165), 16048, iid);
                let portrait_dummy3 = spawn_id(ctx, DUMMY, (-86004, -153168, -9165), 16048, iid);
                let scarlet_dummy = spawn_id(ctx, DUMMY2, (-87784, -153298, -9175), 16384, iid);
                disable_players(ctx, iid);

                // broadcastPacket(world, new SpecialCamera(overheadDummy, 0, 75, -89, 0, 100, 0, 0, 1, 0, 0));
                camera(ctx, iid, overhead_dummy, [0, 75, 89, 0, 100, 0, 0, 1, 0, 0]);
                camera(ctx, iid, overhead_dummy, [300, 90, 10, 6500, 7000, 0, 0, 1, 0, 0]);

                let frintezza = spawn_id(ctx, FRINTEZZA, (-87780, -155086, -9080), 16384, iid);
                if let Some(frintezza) = frintezza {
                    ctx.world_mut().set_immobilized(frintezza, true);
                    ctx.world_mut().set_invul(frintezza, true);
                    ctx.world_mut().disable_all_skills(frintezza);
                }

                let mut demons = Vec::new();
                for element in PORTRAIT_SPAWNS {
                    let demon = spawn_id(
                        ctx,
                        element[0] + 2,
                        (element[5], element[6], element[7]),
                        element[8],
                        iid,
                    );
                    if let Some(demon) = demon {
                        ctx.world_mut().set_immobilized(demon, true);
                        ctx.world_mut().disable_all_skills(demon);
                        demons.push(demon);
                    }
                }

                let world = tomb(ctx, iid);
                world.frintezza_dummy = frintezza_dummy;
                world.overhead_dummy = overhead_dummy;
                world.portrait_dummy1 = portrait_dummy1;
                world.portrait_dummy3 = portrait_dummy3;
                world.scarlet_dummy = scarlet_dummy;
                world.frintezza = frintezza;
                world.demons = demons;
                ctx.start_quest_timer("FRINTEZZA_INTRO_3", 6500, None, player, false);
            }
            "FRINTEZZA_INTRO_3" => {
                let frintezza_dummy = tomb(ctx, iid).frintezza_dummy;
                camera(
                    ctx,
                    iid,
                    frintezza_dummy,
                    [1800, 90, 8, 6500, 7000, 0, 0, 1, 0, 0],
                );
                ctx.start_quest_timer("FRINTEZZA_INTRO_4", 900, None, player, false);
            }
            "FRINTEZZA_INTRO_4" => {
                let frintezza_dummy = tomb(ctx, iid).frintezza_dummy;
                camera(
                    ctx,
                    iid,
                    frintezza_dummy,
                    [140, 90, 10, 2500, 4500, 0, 0, 1, 0, 0],
                );
                ctx.start_quest_timer("FRINTEZZA_INTRO_5", 4000, None, player, false);
            }
            "FRINTEZZA_INTRO_5" => {
                let frintezza = tomb(ctx, iid).frintezza;
                camera(ctx, iid, frintezza, [40, 75, 10, 0, 1000, 0, 0, 1, 0, 0]);
                camera(ctx, iid, frintezza, [40, 75, 10, 0, 12000, 0, 0, 1, 0, 0]);
                ctx.start_quest_timer("FRINTEZZA_INTRO_6", 1350, None, player, false);
            }
            "FRINTEZZA_INTRO_6" => {
                let (frintezza, frintezza_dummy) = {
                    let world = tomb(ctx, iid);
                    (world.frintezza, world.frintezza_dummy)
                };
                social(ctx, iid, frintezza, 2);
                if let Some(frintezza_dummy) = frintezza_dummy {
                    ctx.world_mut().delete_npc(frintezza_dummy);
                }
                ctx.start_quest_timer("FRINTEZZA_INTRO_7", 8000, None, player, false);
            }
            "FRINTEZZA_INTRO_7" => {
                let demons = tomb(ctx, iid).demons.clone();
                social(ctx, iid, demons.get(1).copied(), 1);
                social(ctx, iid, demons.get(2).copied(), 1);
                ctx.start_quest_timer("FRINTEZZA_INTRO_8", 400, None, player, false);
            }
            "FRINTEZZA_INTRO_8" => {
                let (demons, portrait_dummy1, portrait_dummy3) = {
                    let world = tomb(ctx, iid);
                    (world.demons.clone(), world.portrait_dummy1, world.portrait_dummy3)
                };
                social(ctx, iid, demons.first().copied(), 1);
                social(ctx, iid, demons.get(3).copied(), 1);
                send_packet_x(
                    ctx,
                    iid,
                    camera_packet(portrait_dummy1, [1000, 118, 0, 0, 1000, 0, 0, 1, 0, 0]),
                    camera_packet(portrait_dummy3, [1000, 62, 0, 0, 1000, 0, 0, 1, 0, 0]),
                    -87784,
                );
                send_packet_x(
                    ctx,
                    iid,
                    camera_packet(portrait_dummy1, [1000, 118, 0, 0, 10000, 0, 0, 1, 0, 0]),
                    camera_packet(portrait_dummy3, [1000, 62, 0, 0, 10000, 0, 0, 1, 0, 0]),
                    -87784,
                );
                ctx.start_quest_timer("FRINTEZZA_INTRO_9", 2000, None, player, false);
            }
            "FRINTEZZA_INTRO_9" => {
                let (frintezza, portrait_dummy1, portrait_dummy3) = {
                    let world = tomb(ctx, iid);
                    (world.frintezza, world.portrait_dummy1, world.portrait_dummy3)
                };
                camera(ctx, iid, frintezza, [240, 90, 0, 0, 1000, 0, 0, 1, 0, 0]);
                camera(ctx, iid, frintezza, [240, 90, 25, 5500, 10000, 0, 0, 1, 0, 0]);
                social(ctx, iid, frintezza, 3);
                for dummy in [portrait_dummy1, portrait_dummy3].into_iter().flatten() {
                    ctx.world_mut().delete_npc(dummy);
                }
                ctx.start_quest_timer("FRINTEZZA_INTRO_10", 4500, None, player, false);
            }
            "FRINTEZZA_INTRO_10" => {
                let frintezza = tomb(ctx, iid).frintezza;
                camera(ctx, iid, frintezza, [100, 195, 35, 0, 10000, 0, 0, 1, 0, 0]);
                ctx.start_quest_timer("FRINTEZZA_INTRO_11", 700, None, player, false);
            }
            "FRINTEZZA_INTRO_11" => {
                let frintezza = tomb(ctx, iid).frintezza;
                camera(ctx, iid, frintezza, [100, 195, 35, 0, 10000, 0, 0, 1, 0, 0]);
                ctx.start_quest_timer("FRINTEZZA_INTRO_12", 1300, None, player, false);
            }
            "FRINTEZZA_INTRO_12" => {
                let frintezza = tomb(ctx, iid).frintezza;
                broadcast(
                    ctx,
                    iid,
                    &ExShowScreenMessage::npc_string(NpcStringId::MournfulChoralePrelude, 2, 5000, &[]),
                );
                camera(ctx, iid, frintezza, [120, 180, 45, 1500, 10000, 0, 0, 1, 0, 0]);
                skill_use(ctx, iid, frintezza, frintezza, 5006, 1, 34000);
                ctx.start_quest_timer("FRINTEZZA_INTRO_13", 1500, None, player, false);
            }
            "FRINTEZZA_INTRO_13" => {
                let frintezza = tomb(ctx, iid).frintezza;
                camera(ctx, iid, frintezza, [520, 135, 45, 8000, 10000, 0, 0, 1, 0, 0]);
                ctx.start_quest_timer("FRINTEZZA_INTRO_14", 7500, None, player, false);
            }
            "FRINTEZZA_INTRO_14" => {
                let frintezza = tomb(ctx, iid).frintezza;
                camera(ctx, iid, frintezza, [1500, 110, 25, 10000, 13000, 0, 0, 1, 0, 0]);
                ctx.start_quest_timer("FRINTEZZA_INTRO_15", 9500, None, player, false);
            }
            "FRINTEZZA_INTRO_15" => {
                let (overhead_dummy, scarlet_dummy) = {
                    let world = tomb(ctx, iid);
                    (world.overhead_dummy, world.scarlet_dummy)
                };
                camera(ctx, iid, overhead_dummy, [930, 160, 10, 0, 1000, 0, 0, 1, 0, 0]);
                camera(ctx, iid, overhead_dummy, [1000, 180, 0, 0, 10000, 0, 0, 1, 0, 0]);
                skill_use(ctx, iid, scarlet_dummy, overhead_dummy, 5004, 1, 5800);
                ctx.start_quest_timer("FRINTEZZA_INTRO_16", 5000, None, player, false);
            }
            "FRINTEZZA_INTRO_16" => {
                let scarlet_dummy = tomb(ctx, iid).scarlet_dummy;
                let active_scarlet = spawn_id(ctx, SCARLET1, (-87789, -153295, -9176), 16384, iid);
                tomb(ctx, iid).active_scarlet = active_scarlet;
                if let Some(active_scarlet) = active_scarlet {
                    ctx.world_mut()
                        .set_npc_right_hand(active_scarlet, FIRST_SCARLET_WEAPON);
                    ctx.world_mut().set_invul(active_scarlet, true);
                    ctx.world_mut().set_immobilized(active_scarlet, true);
                    ctx.world_mut().disable_all_skills(active_scarlet);
                }
                social(ctx, iid, active_scarlet, 3);
                camera(
                    ctx,
                    iid,
                    scarlet_dummy,
                    [800, 180, 10, 1000, 10000, 0, 0, 1, 0, 0],
                );
                ctx.start_quest_timer("FRINTEZZA_INTRO_17", 2100, None, player, false);
            }
            "FRINTEZZA_INTRO_17" => {
                let active_scarlet = tomb(ctx, iid).active_scarlet;
                camera(ctx, iid, active_scarlet, [300, 60, 8, 0, 10000, 0, 0, 1, 0, 0]);
                ctx.start_quest_timer("FRINTEZZA_INTRO_18", 2000, None, player, false);
            }
            "FRINTEZZA_INTRO_18" => {
                let active_scarlet = tomb(ctx, iid).active_scarlet;
                camera(ctx, iid, active_scarlet, [500, 90, 10, 3000, 5000, 0, 0, 1, 0, 0]);
                tomb(ctx, iid).is_playing_song = false;
                play_random_song(ctx, iid);
                ctx.start_quest_timer("FRINTEZZA_INTRO_19", 3000, None, player, false);
            }
            "FRINTEZZA_INTRO_19" => {
                let mut portraits = Vec::new();
                for (i, element) in PORTRAIT_SPAWNS.iter().enumerate() {
                    let portrait = spawn_id(
                        ctx,
                        element[0],
                        (element[1], element[2], element[3]),
                        element[4],
                        iid,
                    );
                    if let Some(portrait) = portrait {
                        portraits.push((portrait, i));
                    }
                }

                let world = tomb(ctx, iid);
                world.portraits = portraits;
                let (overhead_dummy, scarlet_dummy) = (world.overhead_dummy, world.scarlet_dummy);
                for dummy in [overhead_dummy, scarlet_dummy].into_iter().flatten() {
                    ctx.world_mut().delete_npc(dummy);
                }
                ctx.start_quest_timer("FRINTEZZA_INTRO_20", 2000, None, player, false);
            }
            "FRINTEZZA_INTRO_20" => {
                let (frintezza, active_scarlet, demons) = {
                    let world = tomb(ctx, iid);
                    (world.frintezza, world.active_scarlet, world.demons.clone())
                };
                for demon in demons {
                    ctx.world_mut().set_immobilized(demon, false);
                    ctx.world_mut().enable_all_skills(demon);
                }

                if let Some(active_scarlet) = active_scarlet {
                    ctx.world_mut().set_invul(active_scarlet, false);
                    ctx.world_mut().set_immobilized(active_scarlet, false);
                    ctx.world_mut().enable_all_skills(active_scarlet);
                    ctx.world_mut().set_running(active_scarlet, true);
                    ctx.world_mut().do_cast(active_scarlet, INTRO_SKILL);
                }
                if let Some(frintezza) = frintezza {
                    ctx.world_mut().enable_all_skills(frintezza);
                    ctx.world_mut().set_core_ai_disabled(frintezza, true);
                    ctx.world_mut().set_invul(frintezza, true);
                }
                enable_players(ctx, iid);
                let frintezza = frintezza.and_then(|f| ctx.script_npc(f));
                ctx.start_quest_timer(
                    "PLAY_RANDOM_SONG",
                    RANDOM_SONG_INTERVAL * 1000,
                    frintezza,
                    None,
                    false,
                );
                ctx.start_quest_timer(
                    "SPAWN_DEMONS",
                    TIME_BETWEEN_DEMON_SPAWNS * 1000,
                    None,
                    player,
                    false,
                );
            }
            "SPAWN_DEMONS" => {
                let (portraits, mut demons) = {
                    let world = tomb(ctx, iid);
                    (world.portraits.clone(), world.demons.clone())
                };
                if !portraits.is_empty() {
                    for (_, i) in portraits {
                        if demons.len() > MAX_DEMONS {
                            break;
                        }

                        let element = PORTRAIT_SPAWNS[i];
                        let demon = spawn_id(
                            ctx,
                            element[0] + 2,
                            (element[5], element[6], element[7]),
                            element[8],
                            iid,
                        );
                        if let Some(demon) = demon {
                            demons.push(demon);
                        }
                    }

                    tomb(ctx, iid).demons = demons;
                    ctx.start_quest_timer(
                        "SPAWN_DEMONS",
                        TIME_BETWEEN_DEMON_SPAWNS * 1000,
                        None,
                        player,
                        false,
                    );
                }
            }
            "PLAY_RANDOM_SONG" => {
                let frintezza = tomb(ctx, iid).frintezza;
                if let Some(frintezza) = frintezza {
                    play_random_song(ctx, iid);
                    let frintezza = ctx.script_npc(frintezza);
                    ctx.start_quest_timer(
                        "PLAY_RANDOM_SONG",
                        RANDOM_SONG_INTERVAL * 1000,
                        frintezza,
                        None,
                        false,
                    );
                }
            }
            "SCARLET_FIRST_MORPH" => {
                let active_scarlet = tomb(ctx, iid).active_scarlet;
                if let Some(active_scarlet) = active_scarlet {
                    ctx.world_mut().do_cast(active_scarlet, FIRST_MORPH_SKILL);
                }
                play_random_song(ctx, iid);
            }
            "SCARLET_SECOND_MORPH" => {
                disable_players(ctx, iid);
                deactivate_demons(ctx, iid);
                let (active_scarlet, frintezza) = {
                    let world = tomb(ctx, iid);
                    (world.active_scarlet, world.frintezza)
                };
                if let Some(active_scarlet) = active_scarlet {
                    ctx.world_mut().abort_attack(active_scarlet);
                    ctx.world_mut().abort_cast(active_scarlet);
                    ctx.world_mut().set_invul(active_scarlet, true);
                    ctx.world_mut().set_immobilized(active_scarlet, true);
                    ctx.world_mut().disable_all_skills(active_scarlet);
                }
                let frintezza_npc = frintezza.and_then(|f| ctx.script_npc(f));
                ctx.cancel_quest_timer("PLAY_RANDOM_SONG", frintezza_npc, None);
                if let Some(frintezza) = frintezza {
                    broadcast(
                        ctx,
                        iid,
                        &MagicSkillCanceled {
                            object_id: frintezza.0,
                        },
                    );
                }
                ctx.start_quest_timer(
                    "PLAY_RANDOM_SONG",
                    RANDOM_SONG_INTERVAL * 1000,
                    frintezza_npc,
                    None,
                    false,
                );
                ctx.start_quest_timer("SCARLET_SECOND_MORPH_CAMERA_1", 2000, None, player, false);
            }
            "SCARLET_SECOND_MORPH_CAMERA_1" => {
                let frintezza = tomb(ctx, iid).frintezza;
                social(ctx, iid, frintezza, 4);
                camera(ctx, iid, frintezza, [250, 120, 15, 0, 1000, 0, 0, 1, 0, 0]);
                camera(ctx, iid, frintezza, [250, 120, 15, 0, 10000, 0, 0, 1, 0, 0]);
                ctx.start_quest_timer("SCARLET_SECOND_MORPH_CAMERA_2", 7000, None, player, false);
            }
            "SCARLET_SECOND_MORPH_CAMERA_2" => {
                let frintezza = tomb(ctx, iid).frintezza;
                broadcast(
                    ctx,
                    iid,
                    &ExShowScreenMessage::npc_string(NpcStringId::MournfulChoralePrelude, 2, 5000, &[]),
                );
                camera(ctx, iid, frintezza, [500, 70, 15, 3000, 10000, 0, 0, 1, 0, 0]);
                skill_use(ctx, iid, frintezza, frintezza, 5006, 1, 34000);
                ctx.start_quest_timer("SCARLET_SECOND_MORPH_CAMERA_3", 3000, None, player, false);
            }
            "SCARLET_SECOND_MORPH_CAMERA_3" => {
                let frintezza = tomb(ctx, iid).frintezza;
                camera(ctx, iid, frintezza, [2500, 90, 12, 6000, 10000, 0, 0, 1, 0, 0]);
                ctx.start_quest_timer("SCARLET_SECOND_MORPH_CAMERA_4", 3000, None, player, false);
            }
            "SCARLET_SECOND_MORPH_CAMERA_4" => {
                let active_scarlet = tomb(ctx, iid).active_scarlet;
                let scarlet_location = active_scarlet
                    .and_then(|s| {
                        let (x, y, z) = ctx.world().position_of(s)?;
                        Some((x, y, z, ctx.heading_of(s)))
                    })
                    .unwrap_or((0, 0, 0, 0));
                let heading = scarlet_location.3;
                let new_heading = if heading < 32768 {
                    (180 - (f64::from(heading) / 182.044444444) as i32).abs()
                } else {
                    (540 - (f64::from(heading) / 182.044444444) as i32).abs()
                };

                let world = tomb(ctx, iid);
                world.scarlet_location = scarlet_location;
                world.new_heading = new_heading;
                camera(
                    ctx,
                    iid,
                    active_scarlet,
                    [250, new_heading, 12, 0, 1000, 0, 0, 1, 0, 0],
                );
                camera(
                    ctx,
                    iid,
                    active_scarlet,
                    [250, new_heading, 12, 0, 10000, 0, 0, 1, 0, 0],
                );
                ctx.start_quest_timer("SCARLET_SECOND_MORPH_CAMERA_5", 500, None, player, false);
            }
            "SCARLET_SECOND_MORPH_CAMERA_5" => {
                let (active_scarlet, new_heading) = {
                    let world = tomb(ctx, iid);
                    (world.active_scarlet, world.new_heading)
                };
                if let Some(active_scarlet) = active_scarlet {
                    ctx.world_mut().do_die(active_scarlet, Some(active_scarlet));
                }
                camera(
                    ctx,
                    iid,
                    active_scarlet,
                    [450, new_heading, 14, 8000, 8000, 0, 0, 1, 0, 0],
                );
                ctx.start_quest_timer("SCARLET_SECOND_MORPH_CAMERA_6", 6250, None, player, false);
                ctx.start_quest_timer("SCARLET_SECOND_MORPH_CAMERA_7", 7200, None, player, false);
            }
            "SCARLET_SECOND_MORPH_CAMERA_6" => {
                if let Some(active_scarlet) = tomb(ctx, iid).active_scarlet {
                    ctx.world_mut().delete_npc(active_scarlet);
                }
            }
            "SCARLET_SECOND_MORPH_CAMERA_7" => {
                let (new_heading, (x, y, z, heading)) = {
                    let world = tomb(ctx, iid);
                    (world.new_heading, world.scarlet_location)
                };
                let active_scarlet = spawn_id(ctx, SCARLET2, (x, y, z), heading, iid);
                tomb(ctx, iid).active_scarlet = active_scarlet;
                if let Some(active_scarlet) = active_scarlet {
                    ctx.world_mut()
                        .set_npc_left_hand(active_scarlet, SECOND_SCARLET_WEAPON);
                    ctx.world_mut().set_invul(active_scarlet, true);
                    ctx.world_mut().set_immobilized(active_scarlet, true);
                    ctx.world_mut().disable_all_skills(active_scarlet);
                }
                camera(
                    ctx,
                    iid,
                    active_scarlet,
                    [450, new_heading, 12, 500, 14000, 0, 0, 1, 0, 0],
                );
                let scarlet = active_scarlet.and_then(|s| ctx.script_npc(s));
                ctx.start_quest_timer("SCARLET_SECOND_MORPH_CAMERA_8", 8100, scarlet, None, false);
            }
            "SCARLET_SECOND_MORPH_CAMERA_8" => {
                let npc = npc?;
                social(ctx, iid, Some(npc.object_id), 2);
                ctx.start_quest_timer("SCARLET_SECOND_MORPH_CAMERA_9", 9000, Some(npc), None, false);
            }
            "SCARLET_SECOND_MORPH_CAMERA_9" => {
                let npc = npc?;
                ctx.world_mut().set_invul(npc.object_id, false);
                ctx.world_mut().set_immobilized(npc.object_id, false);
                ctx.world_mut().enable_all_skills(npc.object_id);
                enable_players(ctx, iid);
                activate_demons(ctx, iid);
                play_random_song(ctx, iid);
            }
            "FINISH_CAMERA_1" => {
                let (active_scarlet, frintezza, new_heading) = {
                    let world = tomb(ctx, iid);
                    (world.active_scarlet, world.frintezza, world.new_heading)
                };
                camera(
                    ctx,
                    iid,
                    active_scarlet,
                    [300, new_heading - 180, 5, 0, 7000, 0, 0, 1, 0, 0],
                );
                camera(
                    ctx,
                    iid,
                    active_scarlet,
                    [200, new_heading, 85, 4000, 10000, 0, 0, 1, 0, 0],
                );
                let frintezza = frintezza.and_then(|f| ctx.script_npc(f));
                ctx.start_quest_timer("FINISH_CAMERA_2", 7400, frintezza, player, false);
                ctx.start_quest_timer("FINISH_CAMERA_3", 7500, None, player, false);
            }
            "FINISH_CAMERA_2" => {
                let frintezza = tomb(ctx, iid).frintezza;
                if let Some(frintezza) = frintezza {
                    ctx.world_mut().do_die(frintezza, player);
                }
            }
            "FINISH_CAMERA_3" => {
                let frintezza = tomb(ctx, iid).frintezza;
                camera(ctx, iid, frintezza, [100, 120, 5, 0, 7000, 0, 0, 1, 0, 0]);
                camera(ctx, iid, frintezza, [100, 90, 5, 5000, 15000, 0, 0, 1, 0, 0]);
                ctx.start_quest_timer("FINISH_CAMERA_4", 7000, None, player, false);
            }
            "FINISH_CAMERA_4" => {
                let frintezza = tomb(ctx, iid).frintezza;
                camera(ctx, iid, frintezza, [900, 90, 25, 7000, 10000, 0, 0, 1, 0, 0]);
                ctx.start_quest_timer("FINISH_CAMERA_5", 9000, None, player, false);
            }
            "FINISH_CAMERA_5" => {
                for door_id in FIRST_ROOM_DOORS {
                    ctx.world_mut().instance_open_door(iid, door_id);
                }

                for door_id in FIRST_ROUTE_DOORS {
                    ctx.world_mut().instance_open_door(iid, door_id);
                }

                for door_id in SECOND_ROOM_DOORS {
                    ctx.world_mut().instance_open_door(iid, door_id);
                }

                for door_id in SECOND_ROUTE_DOORS {
                    ctx.world_mut().instance_open_door(iid, door_id);
                }

                enable_players(ctx, iid);
            }
            _ => {}
        }

        None
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        if npc.id == GUIDE {
            enter_instance(ctx, player);
        } else {
            // Teleport Cube.
            let x = 181380 + ctx.get_random(50);
            let y = -80903 + ctx.get_random(50);
            ctx.world_mut()
                .tele_to_location(player, TeleportTarget::at(x, y, -2731));
        }

        None
    }

    fn on_spawn(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc) {
        ctx.set_random_walking(npc, false);
        ctx.set_immobilized(npc, true);
        if npc.id == HALL_ALARM {
            ctx.disable_core_ai(npc, true);
        } else {
            // dummy
            ctx.world_mut().set_invul(npc.object_id, true);
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
        if npc.id == SCARLET1 {
            let (current_hp, max_hp) = ctx.world().hp_and_max(npc.object_id).unwrap_or((0.0, 0.0));
            if ctx.is_script_value(npc, 0) && (current_hp < (max_hp * 0.80)) {
                ctx.set_script_value(npc, 1);
                ctx.start_quest_timer("SCARLET_FIRST_MORPH", 1000, Some(npc), None, false);
            }

            if ctx.is_script_value(npc, 1) && (current_hp < (max_hp * 0.20)) {
                ctx.set_script_value(npc, 2);
                ctx.start_quest_timer("SCARLET_SECOND_MORPH", 1000, None, Some(attacker), false);
            }
        }
    }

    fn on_skill_see(
        &self,
        ctx: &mut ScriptCtx<'_>,
        npc: ScriptNpc,
        caster: ObjectId,
        skill: SkillKey,
        _targets: &[ObjectId],
        _is_summon: bool,
    ) {
        // When Dewdrop of Destruction is used on Portraits they suicide.
        if PORTRAITS.contains(&npc.id) && (skill.id == DEWDROP_OF_DESTRUCTION_SKILL_ID) {
            ctx.world_mut().do_die(npc.object_id, Some(caster));
        }
    }

    fn on_spell_finished(
        &self,
        ctx: &mut ScriptCtx<'_>,
        npc: ScriptNpc,
        _player: Option<ObjectId>,
        skill: SkillKey,
    ) {
        if ctx.world().skill_is_suicide_attack(skill) {
            kill(ctx, npc, None);
        }
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        kill(ctx, npc, Some(killer));
    }
}

/// Java `onKill(npc, killer, isSummon)`; `onSpellFinished` calls it without a killer.
fn kill(ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: Option<ObjectId>) {
    let Some(iid) = instance_of_object(ctx, npc.object_id) else {
        return;
    };
    let status = ctx.world().instance_world(iid).map_or(0, |w| w.status());
    if npc.id == HALL_ALARM && status == 0 {
        set_status(ctx, iid, 1);
        ctx.world_mut().spawn_instance_group(iid, "room1");
        let monsters = alive_npcs(ctx, iid, &MONSTERS_FIRST_ROOM);
        tomb(ctx, iid).monsters_count = monsters.len() as i32;
        for door_id in FIRST_ROOM_DOORS {
            ctx.world_mut().instance_open_door(iid, door_id);
        }

        for monster in monsters {
            attack_player(ctx, iid, monster, FIRST_ROOM_CENTER);
        }
    } else if npc.id == SCARLET2 {
        let frintezza = tomb(ctx, iid).frintezza;
        let frintezza_npc = frintezza.and_then(|f| ctx.script_npc(f));
        ctx.cancel_quest_timer("PLAY_RANDOM_SONG", frintezza_npc, None);
        if let Some(frintezza) = frintezza {
            broadcast(
                ctx,
                iid,
                &MagicSkillCanceled {
                    object_id: frintezza.0,
                },
            );
        }
        ctx.start_quest_timer("FINISH_CAMERA_1", 500, Some(npc), killer, false);
        kill_portraits_and_demons(ctx, iid);
        ctx.world_mut().set_instance_duration(iid, 300_000);
    } else if DEMONS.contains(&npc.id) {
        tomb(ctx, iid).demons.retain(|d| *d != npc.object_id);
    } else if PORTRAITS.contains(&npc.id) {
        tomb(ctx, iid).portraits.retain(|(p, _)| *p != npc.object_id);
    } else {
        let kill_count = {
            let world = tomb(ctx, iid);
            world.monsters_count -= 1;
            world.monsters_count
        };

        if kill_count <= 0 {
            match status {
                1 => {
                    set_status(ctx, iid, 2);
                    ctx.world_mut().spawn_instance_group(iid, "room2_part1");
                    let monsters = alive_npcs(ctx, iid, &[DARK_CHOIR_PLAYER]);
                    tomb(ctx, iid).monsters_count = monsters.len() as i32;
                    for door_id in FIRST_ROUTE_DOORS {
                        ctx.world_mut().instance_open_door(iid, door_id);
                    }
                }
                2 => {
                    set_status(ctx, iid, 3);
                    ctx.world_mut().spawn_instance_group(iid, "room2_part2");
                    let monsters = alive_npcs(ctx, iid, &MONSTERS_SECOND_ROOM);
                    tomb(ctx, iid).monsters_count = monsters.len() as i32;
                    for door_id in SECOND_ROOM_DOORS {
                        ctx.world_mut().instance_open_door(iid, door_id);
                    }

                    for monster in monsters {
                        attack_player(ctx, iid, monster, SECOND_ROOM_CENTER);
                    }
                }
                3 => {
                    set_status(ctx, iid, 4);
                    for door_id in SECOND_ROUTE_DOORS {
                        ctx.world_mut().instance_open_door(iid, door_id);
                    }

                    ctx.start_quest_timer(
                        "FRINTEZZA_INTRO_START",
                        FRINTEZZA_WAIT_TIME * 60 * 1000,
                        None,
                        killer,
                        false,
                    );
                }
                _ => {}
            }
        }

        if ctx.get_random(100) < 5 {
            ctx.drop_item(npc, killer, DEWDROP_OF_DESTRUCTION_ITEM_ID, 1);
        }
    }
}

fn set_status(ctx: &mut ScriptCtx<'_>, iid: i32, status: i32) {
    if let Some(world) = ctx.world_mut().instance_world_mut(iid) {
        world.set_status(status);
    }
}

fn tomb<'a>(ctx: &'a mut ScriptCtx<'_>, iid: i32) -> &'a mut TombWorld {
    ctx.state::<TombState>().worlds.entry(iid).or_default()
}

/// `InstanceManager.getInstance().getWorld(npc)`.
fn instance_of_object(ctx: &ScriptCtx<'_>, object: ObjectId) -> Option<i32> {
    ctx.world().instance_world_of(object).map(|w| w.instance_id)
}

/// `InstanceManager.getInstance().getWorld(player)`.
fn instance_of_player(ctx: &ScriptCtx<'_>, player: ObjectId) -> Option<i32> {
    ctx.world().player_instance_world(player).map(|w| w.instance_id)
}

fn spawn_id(
    ctx: &mut ScriptCtx<'_>,
    npc_id: i32,
    (x, y, z): (i32, i32, i32),
    heading: i32,
    iid: i32,
) -> Option<ObjectId> {
    ctx.add_spawn(npc_id, x, y, z, heading, false, 0, iid)
        .map(|n| n.object_id)
}

/// `world.getAliveNpcs(ids)`.
fn alive_npcs(ctx: &ScriptCtx<'_>, iid: i32, ids: &[i32]) -> Vec<ObjectId> {
    let Some(instance) = ctx.world().instance(iid) else {
        return Vec::new();
    };
    instance
        .npcs
        .keys()
        .copied()
        .filter(|n| {
            ctx.world().npc(*n).is_some_and(|npc| ids.contains(&npc.npc_id)) && !ctx.world().is_dead(*n)
        })
        .collect()
}

/// The online players of `world.getAllowed()`.
fn online_allowed(ctx: &ScriptCtx<'_>, iid: i32) -> Vec<ObjectId> {
    ctx.world()
        .instance_world(iid)
        .map(|w| w.allowed.clone())
        .unwrap_or_default()
        .into_iter()
        .filter(|p| ctx.world().player(*p).is_some())
        .collect()
}

fn broadcast(ctx: &ScriptCtx<'_>, iid: i32, packet: &impl l2_net::Encode) {
    for player in online_allowed(ctx, iid) {
        ctx.world().send(player, packet);
    }
}

/// `new SpecialCamera(creature, force, angle1, angle2, time, range, duration, relYaw, relPitch,
/// isWide, relAngle)` (the 11-argument constructor; `range` is the packet's duration).
fn camera_packet(creature: Option<ObjectId>, p: [i32; 10]) -> Option<SpecialCamera> {
    creature.map(|c| {
        SpecialCamera::from_range_duration(c.0, p[0], p[1], p[2], p[3], p[4], p[5], p[6], p[7], p[8], p[9])
    })
}

fn camera(ctx: &ScriptCtx<'_>, iid: i32, creature: Option<ObjectId>, p: [i32; 10]) {
    if let Some(packet) = camera_packet(creature, p) {
        broadcast(ctx, iid, &packet);
    }
}

fn social(ctx: &ScriptCtx<'_>, iid: i32, creature: Option<ObjectId>, action_id: i32) {
    if let Some(creature) = creature {
        broadcast(
            ctx,
            iid,
            &SocialAction {
                object_id: creature.0,
                action_id,
            },
        );
    }
}

/// `new MagicSkillUse(caster, target, skillId, level, hitTime, 0)` to the allowed players.
fn skill_use(
    ctx: &ScriptCtx<'_>,
    iid: i32,
    caster: Option<ObjectId>,
    target: Option<ObjectId>,
    skill_id: i32,
    skill_level: i32,
    hit_time: i32,
) {
    let (Some(caster), Some(target)) = (caster, target) else {
        return;
    };
    let (Some(c), Some(t)) = (ctx.world().position_of(caster), ctx.world().position_of(target)) else {
        return;
    };
    broadcast(
        ctx,
        iid,
        &MagicSkillUse {
            creature_id: caster.0,
            target_id: target.0,
            skill_id,
            skill_level,
            hit_time,
            reuse_delay: 0,
            creature: SkillPosition {
                x: c.0,
                y: c.1,
                z: c.2,
            },
            ground_location: None,
            target: SkillPosition {
                x: t.0,
                y: t.1,
                z: t.2,
            },
        },
    );
}

/// `sendPacketX`: the first packet to the players west of `x`, the second to the others.
fn send_packet_x(
    ctx: &ScriptCtx<'_>,
    iid: i32,
    packet1: Option<SpecialCamera>,
    packet2: Option<SpecialCamera>,
    x: i32,
) {
    for player in online_allowed(ctx, iid) {
        let west = ctx.world().position_of(player).is_some_and(|(px, _, _)| px < x);
        let packet = if west { &packet1 } else { &packet2 };
        if let Some(packet) = packet {
            ctx.world().send(player, packet);
        }
    }
}

fn play_random_song(ctx: &mut ScriptCtx<'_>, iid: i32) {
    if tomb(ctx, iid).is_playing_song {
        return;
    }

    let Some(frintezza) = tomb(ctx, iid).frintezza else {
        return;
    };
    tomb(ctx, iid).is_playing_song = true;
    let random = ctx.get_random_range(1, 5);
    let skill = SkillKey::new(5007, random);
    let skill_effect = SkillKey::new(5008, random);
    if let Some(message) = skill_msg(random) {
        // `new ExShowScreenMessage(2, -1, 2, 0, 0, 0, 0, true, 4000, false, null, npcString, null)`.
        let packet = ExShowScreenMessage {
            show_effect: true,
            ..ExShowScreenMessage::npc_string(message, 2, 4000, &[])
        };
        broadcast(ctx, iid, &packet);
    }
    let hit_time = ctx.world().skill_hit_time(skill.id, skill.level);
    skill_use(
        ctx,
        iid,
        Some(frintezza),
        Some(frintezza),
        skill.id,
        skill.level,
        hit_time,
    );
    for player in online_allowed(ctx, iid) {
        ctx.world_mut().set_npc_target(frintezza, Some(player));
        ctx.world_mut().do_cast(frintezza, skill_effect);
    }

    tomb(ctx, iid).is_playing_song = false;
}

fn disable_players(ctx: &mut ScriptCtx<'_>, iid: i32) {
    for player in online_allowed(ctx, iid) {
        let world = ctx.world_mut();
        world.abort_attack(player);
        world.abort_cast(player);
        world.disable_all_skills(player);
        world.set_target(player, None);
        world.stop_move(player, None);
        world.set_immobilized(player, true);
        world.script_set_intention_idle(player);
    }
}

fn enable_players(ctx: &mut ScriptCtx<'_>, iid: i32) {
    for player in online_allowed(ctx, iid) {
        ctx.world_mut().enable_all_skills(player);
        ctx.world_mut().set_immobilized(player, false);
    }
}

fn attack_player(ctx: &mut ScriptCtx<'_>, iid: i32, npc: ObjectId, location: (i32, i32, i32)) {
    let players: Vec<ObjectId> = online_allowed(ctx, iid)
        .into_iter()
        .filter(|p| !ctx.world().is_dead(*p) && !ctx.world().is_object_invisible(*p))
        .collect();
    let target = if players.is_empty() {
        None
    } else {
        let last = players.len() as i32 - 1;
        let index = ctx.get_random_range(0, last);
        players.get(index as usize).copied()
    };

    if let Some(target) = target {
        if let Some(npc) = ctx.script_npc(npc) {
            ctx.add_attack_desire(npc, target, 500);
        }
    } else {
        let (x, y, z) = location;
        ctx.world_mut().npc_set_intention_move_to(npc, x, y, z);
    }
}

fn deactivate_demons(ctx: &mut ScriptCtx<'_>, iid: i32) {
    for demon in alive_npcs(ctx, iid, &DEMONS) {
        ctx.world_mut().set_immobilized(demon, true);
        ctx.world_mut().disable_all_skills(demon);
    }
}

fn activate_demons(ctx: &mut ScriptCtx<'_>, iid: i32) {
    for demon in alive_npcs(ctx, iid, &DEMONS) {
        ctx.world_mut().set_immobilized(demon, false);
        ctx.world_mut().enable_all_skills(demon);
    }
}

fn kill_portraits_and_demons(ctx: &mut ScriptCtx<'_>, iid: i32) {
    for demon in alive_npcs(ctx, iid, &DEMONS) {
        ctx.world_mut().do_die(demon, None);
    }
    tomb(ctx, iid).demons.clear();

    for portrait in alive_npcs(ctx, iid, &PORTRAITS) {
        ctx.world_mut().do_die(portrait, None);
    }
    tomb(ctx, iid).portraits.clear();
}

/// `enterInstance(player, TEMPLATE_ID)` with the script's `checkConditions` and
/// `onEnterInstance`. What `checkConditions` reads from the script (`isGM`, the scroll) is read
/// here.
fn enter_instance(ctx: &mut ScriptCtx<'_>, player: ObjectId) {
    let gm = ctx.is_gm(player);
    let has_scroll = ctx.world().inventory_item_count(player, FRINTEZZAS_SCROLL) > 0;
    ctx.world_mut().enter_instance(
        player,
        TEMPLATE_ID,
        |world, player| check_conditions(world, player, gm, has_scroll),
        on_enter_instance,
    );
}

fn message(world: &World, to: ObjectId, id: SystemMessageId) {
    world.send(to, &SystemMessage::new(id));
}

fn pc_message(world: &World, to: ObjectId, id: SystemMessageId, name: &str) -> SystemMessage {
    let _ = (world, to);
    SystemMessage::new(id).with(SystemMessageParam::PlayerName(name.to_owned()))
}

fn player_name(world: &World, player: ObjectId) -> String {
    world
        .player(player)
        .map(|p| p.data.name.clone())
        .unwrap_or_default()
}

fn check_conditions(world: &mut World, player: ObjectId, gm: bool, has_scroll: bool) -> bool {
    if gm {
        return true;
    }

    if world.party_of(player).is_none() {
        message(
            world,
            player,
            SystemMessageId::YouAreNotCurrentlyInAPartySoYouCannotEnter,
        );
        return false;
    }

    let party_members = world.party_members_of(player);
    let Some(channel) = world.command_channel_of(player) else {
        message(
            world,
            player,
            SystemMessageId::YouCannotEnterBecauseYouAreNotAssociatedWithTheCurrentCommandChannel,
        );
        return false;
    };
    if world.command_channel_leader(channel) != Some(player) {
        message(
            world,
            player,
            SystemMessageId::OnlyAPartyLeaderCanMakeTheRequestToEnter,
        );
        return false;
    } else if !has_scroll {
        let name = player_name(world, player);
        world.send(
            player,
            &pc_message(
                world,
                player,
                SystemMessageId::C1SItemRequirementIsNotSufficientAndCannotBeEntered,
                &name,
            ),
        );
        return false;
    } else if !(MIN_PLAYERS..=MAX_PLAYERS).contains(&world.command_channel_member_count(channel)) {
        message(
            world,
            player,
            SystemMessageId::YouCannotEnterDueToThePartyHavingExceededTheLimit,
        );
        return false;
    }

    for channel_member in world.command_channel_members(channel) {
        let name = player_name(world, channel_member);
        let level = world.player(channel_member).map_or(0, |p| p.core.level);
        if level < 80 {
            let packet = pc_message(
                world,
                player,
                SystemMessageId::C1SLevelDoesNotCorrespondToTheRequirementsForEntry,
                &name,
            );
            for member in &party_members {
                world.send(*member, &packet);
            }
            return false;
        }

        if !world.check_if_in_range(1000, player, channel_member, true) {
            let packet = pc_message(
                world,
                player,
                SystemMessageId::C1IsInALocationWhichCannotBeEnteredThereforeItCannotBeProcessed,
                &name,
            );
            for member in &party_members {
                world.send(*member, &packet);
            }
            return false;
        }

        let reenter_time = world.instance_time(channel_member, TEMPLATE_ID);
        if now_ms() < reenter_time {
            let packet = pc_message(world, player, SystemMessageId::C1MayNotReEnterYet, &name);
            for member in &party_members {
                world.send(*member, &packet);
            }
            return false;
        }
    }

    true
}

fn on_enter_instance(world: &mut World, player: ObjectId, instance_id: i32, first_entrance: bool) {
    let teleport = |world: &mut World, who: ObjectId, (x, y, z): (i32, i32, i32)| {
        world.tele_to_location(who, TeleportTarget::at(x, y, z).with_instance(instance_id));
    };
    if first_entrance {
        let channel = world
            .party_of(player)
            .and_then(|_| world.command_channel_of(player));
        match channel {
            None => {
                if world.inventory_item_count(player, DEWDROP_OF_DESTRUCTION_ITEM_ID) > 0 {
                    world.destroy_item_by_item_id(player, DEWDROP_OF_DESTRUCTION_ITEM_ID, -1, true);
                }

                if let Some(w) = world.instance_world_mut(instance_id) {
                    w.add_allowed(player);
                }
                teleport(world, player, ENTER_TELEPORT);
            }
            Some(channel) => {
                for channel_member in world.command_channel_members(channel) {
                    // NOTE(java-quirk): the condition asks the leader's inventory, the removal
                    // takes what the member has.
                    if world.inventory_item_count(player, DEWDROP_OF_DESTRUCTION_ITEM_ID) > 0
                        && world.inventory_item_count(channel_member, DEWDROP_OF_DESTRUCTION_ITEM_ID) > 0
                    {
                        world.destroy_item_by_item_id(
                            channel_member,
                            DEWDROP_OF_DESTRUCTION_ITEM_ID,
                            -1,
                            true,
                        );
                    }

                    if let Some(w) = world.instance_world_mut(instance_id) {
                        w.add_allowed(channel_member);
                    }
                    teleport(world, channel_member, ENTER_TELEPORT);
                }
            }
        }

        if let Some(w) = world.instance_world_mut(instance_id) {
            w.set_status(0);
        }
        world.spawn_instance_group(instance_id, "alarm");
    } else {
        let status = world.instance_world(instance_id).map_or(0, |w| w.status());
        if status > 3 {
            teleport(world, player, ENTER_TELEPORT_FRINTEZZA_ROOM);
        } else {
            teleport(world, player, ENTER_TELEPORT);
        }
    }
}

/// `System.currentTimeMillis()` for the checks that run inside `enterInstance`'s closure (no
/// `ScriptCtx` there).
fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_millis()).unwrap_or(0))
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::{EnterOutcome, ScriptRegistry};

    use super::*;
    use crate::instances::testing::instance_data;
    use crate::testing::{ScriptTest, names_to, player_data};

    const BODY: &str = r#"<stats str="40" int="21" dex="30" wit="20" con="43" men="20"><vitals hp="1000" hpRegen="2" mp="500" mpRegen="0.9" /><attack physical="8" magical="5" random="30" critical="4" accuracy="4.75" attackSpeed="253" type="SWORD" range="40" distance="80" width="120" /><defence physical="44.44444" magical="29.5" /><speed><walk ground="20" /><run ground="50" /></speed></stats><collision><radius normal="10"/><height normal="20"/></collision>"#;

    fn npcs() -> String {
        let mut out = String::new();
        for id in [
            18328, 18329, 18330, 18334, 18339, 29045, 29046, 29047, 29048, 29049, 29050, 29051,
        ] {
            out.push_str(&format!(
                r#"<npc id="{id}" type="Monster" level="80" name="Tomb {id}">{BODY}</npc>"#
            ));
        }
        for id in [29052, 29053, 29061, 32011] {
            out.push_str(&format!(
                r#"<npc id="{id}" type="Folk" level="80" name="Tomb {id}"/>"#
            ));
        }
        out
    }

    const TEMPLATE: &str = r#"<spawnlist>
<group name="alarm"><npc id="18328" x="-87904" y="-141296" z="-9168" heading="0" /></group>
<group name="room1"><npc id="18329" x="-87361" y="-142312" z="-9176" heading="0" /><npc id="18330" x="-87188" y="-142282" z="-9176" heading="0" /></group>
<group name="room2_part1"><npc id="18339" x="-87919" y="-147013" z="-9214" heading="0" /></group>
<group name="room2_part2"><npc id="18334" x="-87900" y="-148000" z="-9214" heading="0" /><npc id="18334" x="-87800" y="-148000" z="-9214" heading="0" /></group>
</spawnlist>"#;

    fn setup() -> (ScriptTest, i32) {
        let mut registry = ScriptRegistry::new();
        registry.add(LastImperialTomb);
        let mut t = ScriptTest::new(registry, &npcs(), "", &[]);
        t.world
            .set_instance_data(instance_data(&[(136, "Last Imperial Tomb")], &[(136, TEMPLATE)]));
        t.enter_with(player_data(1, 80, vec![]));
        t.enter_with(player_data(2, 80, vec![]));
        // `enterInstance` for a GM-less leader needs a command channel of 9 or more; the tests
        // make the instance the way the entry does once the conditions pass.
        let outcome = t.world.enter_instance(
            Oid(1),
            136,
            |_, _| true,
            |world, player, iid, first| {
                on_enter_instance(world, player, iid, first);
                if let Some(w) = world.instance_world_mut(iid) {
                    w.add_allowed(Oid(2));
                }
            },
        );
        let EnterOutcome::New(iid) = outcome else {
            panic!("{outcome:?}");
        };
        // The queued `onSpawn` of the alarm.
        t.ticks(2);
        (t, iid)
    }

    fn status(t: &ScriptTest, iid: i32) -> i32 {
        t.world.instance_world(iid).unwrap().status()
    }

    fn alive(t: &ScriptTest, iid: i32, id: i32) -> Vec<Oid> {
        t.world
            .instance(iid)
            .unwrap()
            .npcs
            .keys()
            .copied()
            .filter(|n| t.world.npc(*n).is_some_and(|npc| npc.npc_id == id) && !t.world.is_dead(*n))
            .collect()
    }

    fn has_timer(t: &mut ScriptTest, name: &str, npc: Option<Oid>) -> bool {
        let mut found = false;
        t.world.run_as_script("LastImperialTomb", |ctx| {
            let npc = npc.and_then(|n| ctx.script_npc(n));
            found = ctx.has_quest_timer(name, npc, None);
        });
        found
    }

    fn timer_with_player(t: &mut ScriptTest, name: &str) -> bool {
        let mut found = false;
        t.world.run_as_script("LastImperialTomb", |ctx| {
            found = ctx.has_quest_timer(name, None, Some(Oid(1)));
        });
        found
    }

    fn event(t: &mut ScriptTest, name: &str, npc: Option<Oid>, player: Option<Oid>) {
        t.world.run_as_script("LastImperialTomb", |ctx| {
            let npc = npc.and_then(|n| ctx.script_npc(n));
            LastImperialTomb.on_event(ctx, name, npc, player);
        });
    }

    fn kill_npc(t: &mut ScriptTest, npc: Oid) {
        assert!(t.world.do_die(npc, Some(Oid(1))));
        t.ticks(60);
    }

    #[test]
    fn the_first_entrance_makes_the_world_and_spawns_the_alarm() {
        let (t, iid) = setup();
        let world = t.world.instance_world(iid).unwrap();
        assert!(world.is_allowed(Oid(1)) && world.is_allowed(Oid(2)));
        assert_eq!(status(&t, iid), 0);
        assert_eq!(alive(&t, iid, HALL_ALARM).len(), 1);
        // The alarm does not move and has no AI of its own (`onSpawn`).
        let alarm = alive(&t, iid, HALL_ALARM)[0];
        assert!(t.world.is_immobilized(alarm));
        // Back in: the instance of the room before Frintezza sends the player to the tomb.
        let (x, _, _) = t.world.position_of(Oid(1)).unwrap();
        assert_eq!(x, ENTER_TELEPORT.0);
    }

    #[test]
    fn a_player_who_is_not_in_a_command_channel_cannot_enter() {
        let mut registry = ScriptRegistry::new();
        registry.add(LastImperialTomb);
        let mut t = ScriptTest::new(registry, &npcs(), "", &[]);
        t.world
            .set_instance_data(instance_data(&[(136, "Last Imperial Tomb")], &[(136, TEMPLATE)]));
        t.enter_with(player_data(1, 80, vec![]));
        let guide = t.spawn(GUIDE, 20);
        t.take();
        t.talk(1, guide);
        t.world.run_as_script("LastImperialTomb", |ctx| {
            let npc = ctx.script_npc(guide).unwrap();
            LastImperialTomb.on_talk(ctx, npc, Oid(1));
        });
        assert!(names_to(&t.take(), 1).contains(&"SYSTEM_MESSAGE".to_owned()));
        assert!(t.world.player_instance_world(Oid(1)).is_none());
    }

    #[test]
    fn the_rooms_open_one_after_the_other_and_the_intro_waits_ten_minutes() {
        let (mut t, iid) = setup();
        // The alarm.
        let alarm = alive(&t, iid, HALL_ALARM)[0];
        kill_npc(&mut t, alarm);
        assert_eq!(status(&t, iid), 1);
        let room1 = [alive(&t, iid, 18329), alive(&t, iid, 18330)].concat();
        assert_eq!(room1.len(), 2);
        // Room 1: the count is the monsters of the group.
        for monster in room1 {
            kill_npc(&mut t, monster);
        }
        assert_eq!(status(&t, iid), 2);
        // The doors' tests need none; the second part comes with the choir.
        let choir = alive(&t, iid, DARK_CHOIR_PLAYER);
        assert_eq!(choir.len(), 1);
        kill_npc(&mut t, choir[0]);
        assert_eq!(status(&t, iid), 3);
        let room2 = alive(&t, iid, 18334);
        assert_eq!(room2.len(), 2);
        for monster in room2 {
            kill_npc(&mut t, monster);
        }
        assert_eq!(status(&t, iid), 4);
        assert!(timer_with_player(&mut t, "FRINTEZZA_INTRO_START"));
    }

    #[test]
    fn the_intro_ends_with_the_boss_room_ready_and_the_players_free() {
        let (mut t, iid) = setup();
        t.take();
        event(&mut t, "FRINTEZZA_INTRO_START", None, Some(Oid(1)));
        let names = names_to(&t.take(), 1);
        assert!(names.contains(&"EARTHQUAKE".to_owned()));
        // 17 s: the cube. 20 s: the dummies, Frintezza, the four demons, the players held.
        t.ticks(20 * 20 + 20);
        assert_eq!(alive(&t, iid, CUBE).len(), 1);
        assert_eq!(alive(&t, iid, FRINTEZZA).len(), 1);
        assert_eq!(alive(&t, iid, 29050).len() + alive(&t, iid, 29051).len(), 4);
        assert!(t.world.is_immobilized(Oid(1)));
        // The movie runs about 70 seconds after that.
        t.ticks(75 * 20);
        assert_eq!(alive(&t, iid, SCARLET1).len(), 1);
        assert_eq!(alive(&t, iid, 29048).len() + alive(&t, iid, 29049).len(), 4);
        assert!(!t.world.is_immobilized(Oid(1)));
        let scarlet = alive(&t, iid, SCARLET1)[0];
        assert!(!t.world.is_invul(scarlet));
        let frintezza = alive(&t, iid, FRINTEZZA)[0];
        assert!(t.world.is_invul(frintezza));
        assert!(has_timer(&mut t, "PLAY_RANDOM_SONG", Some(frintezza)));
        assert!(timer_with_player(&mut t, "SPAWN_DEMONS"));
        // The dummies are gone.
        assert!(alive(&t, iid, DUMMY).is_empty() && alive(&t, iid, DUMMY2).is_empty());
        // Scarlet holds her first weapon.
        assert_eq!(t.world.npc(scarlet).unwrap().right_hand, FIRST_SCARLET_WEAPON);
    }

    #[test]
    fn scarlet_morphs_at_eighty_and_twenty_percent_and_the_second_form_ends_the_fight() {
        let (mut t, iid) = setup();
        event(&mut t, "FRINTEZZA_INTRO_START", None, Some(Oid(1)));
        t.ticks(100 * 20);
        let scarlet = alive(&t, iid, SCARLET1)[0];
        let attack = |t: &mut ScriptTest, scarlet: Oid| {
            t.world.run_as_script("LastImperialTomb", |ctx| {
                let npc = ctx.script_npc(scarlet).unwrap();
                LastImperialTomb.on_attack(ctx, npc, Oid(1), 10, false, None);
            });
        };
        // Full health: nothing.
        attack(&mut t, scarlet);
        assert!(!has_timer(&mut t, "SCARLET_FIRST_MORPH", Some(scarlet)));
        t.world.set_current_hp_mp(scarlet, 700.0, 500.0);
        attack(&mut t, scarlet);
        assert!(has_timer(&mut t, "SCARLET_FIRST_MORPH", Some(scarlet)));
        // Once: the value is 1 now.
        t.world.set_current_hp_mp(scarlet, 100.0, 500.0);
        attack(&mut t, scarlet);
        assert!(timer_with_player(&mut t, "SCARLET_SECOND_MORPH"));

        t.ticks(25 * 20);
        // The first form died in the movie; the second one holds its weapon and is not hit yet.
        let second = alive(&t, iid, SCARLET2);
        assert_eq!(second.len(), 1);
        assert_eq!(t.world.npc(second[0]).unwrap().left_hand, SECOND_SCARLET_WEAPON);
        t.ticks(20 * 20);
        assert!(!t.world.is_invul(second[0]));
        assert!(!t.world.is_immobilized(Oid(1)));

        // Its death: the cameras, Frintezza dies, the doors open, five minutes are left.
        let frintezza = alive(&t, iid, FRINTEZZA)[0];
        t.take();
        assert!(t.world.do_die(second[0], Some(Oid(1))));
        // `onKillDelay` (2.5 s), then the timer of 500 ms.
        t.ticks(52);
        let mut found = false;
        t.world.run_as_script("LastImperialTomb", |ctx| {
            let npc = ctx.script_npc(second[0]);
            found = ctx.has_quest_timer("FINISH_CAMERA_1", npc, Some(Oid(1)));
        });
        assert!(found);
        // `FINISH_CAMERA_2` (10.4 s after the death) kills Frintezza.
        t.ticks(8 * 20);
        assert!(t.world.is_dead(frintezza));
        t.ticks(20 * 20);
        assert!(alive(&t, iid, 29050).is_empty() && alive(&t, iid, 29051).is_empty());
        assert!(alive(&t, iid, 29048).is_empty() && alive(&t, iid, 29049).is_empty());
        let names = names_to(&t.take(), 1);
        assert!(names.iter().filter(|n| *n == "SPECIAL_CAMERA").count() >= 5);
    }

    #[test]
    fn the_dewdrop_kills_a_portrait_and_the_cube_takes_the_party_out() {
        let (mut t, iid) = setup();
        event(&mut t, "FRINTEZZA_INTRO_START", None, Some(Oid(1)));
        t.ticks(100 * 20);
        let portrait = [alive(&t, iid, 29048), alive(&t, iid, 29049)].concat()[0];
        let id = t.world.npc(portrait).unwrap().npc_id;
        t.world.run_as_script("LastImperialTomb", |ctx| {
            let npc = ctx.script_npc(portrait).unwrap();
            LastImperialTomb.on_skill_see(ctx, npc, Oid(1), SkillKey::new(2276, 1), &[], false);
            // Another skill does nothing.
        });
        assert!(t.world.is_dead(portrait));
        t.ticks(60);
        let mut left = 0;
        t.world.run_as_script("LastImperialTomb", |ctx| {
            left = ctx.state::<TombState>().worlds[&iid].portraits.len();
        });
        assert_eq!(left, 3, "{id}");

        let cube = alive(&t, iid, CUBE)[0];
        t.world.run_as_script("LastImperialTomb", |ctx| {
            let npc = ctx.script_npc(cube).unwrap();
            LastImperialTomb.on_talk(ctx, npc, Oid(2));
        });
        let (x, y, _) = t.world.position_of(Oid(2)).unwrap();
        assert!((181380..181430).contains(&x) && (-80903..-80853).contains(&y));
    }

    #[test]
    fn the_demons_come_back_every_twenty_seconds_up_to_the_limit() {
        let (mut t, iid) = setup();
        event(&mut t, "FRINTEZZA_INTRO_START", None, Some(Oid(1)));
        t.ticks(100 * 20);
        let before = alive(&t, iid, 29050).len() + alive(&t, iid, 29051).len();
        t.ticks(21 * 20);
        let after = alive(&t, iid, 29050).len() + alive(&t, iid, 29051).len();
        assert_eq!(after, before + 4);
    }
}
