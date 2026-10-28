//! Crystal Caverns (instance 10): the Emerald Square, the Steam Corridor, the Coral Garden, the
//! bosses Tears, Darnel, Kechi and Baylor, and the Oracles that take the party between them.
// Java: dist/game/data/scripts/instances/CrystalCaverns/CrystalCaverns.java
//
// Java keeps the progress in `CCWorld extends InstanceWorld`; here the status is the engine
// `InstanceWorld`'s and the rest is the script's state by instance id ([`CcWorld`]).
//
// Not in the engine (the rest is ported): NPC traps (`addTrap`), so the door-opening trap that
// follows Dolph is never made and `on_trap_action` never fires (the Java TODO says the same of
// the Darnel/Lahm traps); the spawn animation flag `isSummonSpawn` of `addSpawn` (Kechi's
// henchmen appear without it, as the Java TODO admits).

use std::collections::HashMap;

use l2_protocol_game::NpcStringId;
use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{
    ActionFailed, ChatType, CreatureSay, PlaySound, SayBody, SenderField, SocialAction, SpecialCamera,
    SystemMessage, SystemMessageParam,
};
use l2_util::ObjectId;
use l2_world::effects::{SkillFinishType, SkillKey};
use l2_world::script::{COMPLETED, TrapAction};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup, TeleportTarget};

// Items
const WHITE_SEED_OF_EVIL_SHARD: i32 = 9597;
const BLACK_SEED_OF_EVIL_SHARD: i32 = 9598;
const CONTAMINATED_CRYSTAL: i32 = 9690;
const RED_CORAL: i32 = 9692;
const CRYSTAL_FRAGMENT: i32 = 9693;
const SECRET_KEY: i32 = 9694;
const BLUE_CRYSTAL: i32 = 9695;
const RED_CRYSTAL: i32 = 9696;
const CLEAR_CRYSTAL: i32 = 9697;

// NPCs
const ORACLE_GUIDE_1: i32 = 32281;
const ORACLE_GUIDE_2: i32 = 32278;
const ORACLE_GUIDE_3: i32 = 32280;
const ORACLE_GUIDE_4: i32 = 32279;
const CRYSTALLINE_GOLEM: i32 = 32328;
const DOOR_OPENING_TRAP: [i32; 5] = [18378, 143680, 142608, -11886, 0];
const GATEKEEPER_LOHAN: i32 = 22275;
const GATEKEEPER_PROVO: i32 = 22277;
const TOURMALINE: i32 = 22292;
const TEROD: i32 = 22301;
const DOLPH: i32 = 22299;
const WEYLIN: i32 = 22298;
const GUARDIAN_OF_THE_SQUARE: i32 = 22303;
const GUARDIAN_OF_THE_EMERALD: i32 = 22304;
const TEARS: i32 = 25534;
const TEARS_COPY: i32 = 25535;
const KECHI: i32 = 25532;
const KECHIS_HENCHMAN: i32 = 25533;
const BAYLOR: i32 = 29099;
const DARNEL: i32 = 25531;
const ALARM: i32 = 18474;
const CGMOBS: [i32; 7] = [22311, 22312, 22313, 22314, 22315, 22316, 22317];
/// Kechi Hencmans spawn times
const SPAWN: [i64; 6] = [60000, 120000, 90000, 60000, 50000, 40000];
const MOBLIST: [i32; 22] = [
    22279, 22280, 22281, 22282, 22283, 22285, 22286, 22287, 22288, 22289, 22293, 22294, 22295, 22296, 22297,
    22305, 22306, 22307, 22416, 22418, 22419, 22420,
];

// Locations
const START_LOC: (i32, i32, i32) = (143348, 148707, -11972);

// Misc
const TEMPLATE_ID: i32 = 10;
const MIN_LEVEL: i32 = 78;
const DOOR1: i32 = 24220021;
const DOOR2: i32 = 24220024;
const DOOR3: i32 = 24220023;
const DOOR4: i32 = 24220061;
const DOOR5: i32 = 24220025;
const DOOR6: i32 = 24220022;
const ZONES: [i32; 3] = [20105, 20106, 20107];
const DRAGONSCALETIME: i64 = 3000;
const DRAGONCLAWTIME: i64 = 3000;

// Spawn lists: [npcId, x, y, z, heading] (the Oracle orders: [npcId, x, y, z, heading]).
#[rustfmt::skip]
const ALARMSPAWN: [[i32; 3]; 4] = [
    [153572, 141277, -12738],
    [153572, 142852, -12738],
    [154358, 142075, -12738],
    [152788, 142075, -12738],
];

#[rustfmt::skip]
const ORDRE_ORACLE1: [[i32; 5]; 4] = [
    [32274, 147090, 152505, -12169, 31613],
    [32275, 147090, 152575, -12169, 31613],
    [32274, 147090, 152645, -12169, 31613],
    [32274, 147090, 152715, -12169, 31613],
];

#[rustfmt::skip]
const ORDRE_ORACLE2: [[i32; 5]; 3] = [
    [32274, 149783, 152505, -12169, 31613],
    // {32274, 149783, 152575, -12169, 31613},
    [32274, 149783, 152645, -12169, 31613],
    [32276, 149783, 152715, -12169, 31613],
];

#[rustfmt::skip]
const ORDRE_ORACLE3: [[i32; 5]; 2] = [
    [32274, 152461, 152505, -12169, 31613],
    // {32274, 152461, 152575, -12169, 31613},
    [32277, 152461, 152645, -12169, 31613],
    // {32274, 152461, 152715, -12169, 31613}
];

#[rustfmt::skip]
const HALL_SPAWNS: [[i32; 4]; 37] = [
    [141842, 152556, -11814, 50449],
    [141503, 153395, -11814, 40738],
    [141070, 153201, -11814, 39292],
    [141371, 152986, -11814, 35575],
    [141602, 154188, -11814, 24575],
    [141382, 154719, -11814, 37640],
    [141376, 154359, -11814, 12054],
    [140895, 154383, -11814, 37508],
    [140972, 154740, -11814, 52690],
    [141045, 154504, -11814, 50674],
    [140757, 152740, -11814, 39463],
    [140406, 152376, -11814, 16599],
    [140268, 152007, -11817, 45316],
    [139996, 151485, -11814, 47403],
    [140378, 151190, -11814, 58116],
    [140521, 150711, -11815, 55997],
    [140816, 150215, -11814, 53682],
    [141528, 149909, -11814, 22020],
    [141644, 150360, -11817, 13283],
    [142048, 150695, -11815, 5929],
    [141852, 151065, -11817, 27071],
    [142408, 151211, -11815, 2402],
    [142481, 151762, -11815, 12876],
    [141929, 152193, -11815, 27511],
    [142083, 151791, -11814, 47176],
    [141435, 150402, -11814, 41798],
    [140390, 151199, -11814, 50069],
    [140557, 151849, -11814, 45293],
    [140964, 153445, -11814, 56672],
    [142851, 154109, -11814, 24920],
    [142379, 154725, -11814, 30342],
    [142816, 154712, -11814, 33193],
    [142276, 154223, -11814, 33922],
    [142459, 154490, -11814, 33184],
    [142819, 154372, -11814, 21318],
    [141157, 154541, -11814, 27090],
    [141095, 150281, -11814, 55186],
];

#[rustfmt::skip]
const FIRST_SPAWNS: [[i32; 5]; 29] = [
    [22276, 148109, 149601, -12132, 34490],
    [22276, 148017, 149529, -12132, 33689],
    [22278, 148065, 151202, -12132, 35323],
    [22278, 147966, 151117, -12132, 33234],
    [22279, 144063, 150238, -12132, 29654],
    [22279, 144300, 149118, -12135, 5520],
    [22279, 144397, 149337, -12132, 644],
    [22279, 144426, 150639, -12132, 50655],
    [22282, 145841, 151097, -12132, 31810],
    [22282, 144387, 149958, -12132, 61173],
    [22282, 145821, 149498, -12132, 31490],
    [22282, 146619, 149694, -12132, 33374],
    [22282, 146669, 149244, -12132, 31360],
    [22284, 144147, 151375, -12132, 58395],
    [22284, 144485, 151067, -12132, 64786],
    [22284, 144356, 149571, -12132, 63516],
    [22285, 144151, 150962, -12132, 664],
    [22285, 146657, 151365, -12132, 33154],
    [22285, 146623, 150857, -12132, 28034],
    [22285, 147046, 151089, -12132, 32941],
    [22285, 145704, 151255, -12132, 32523],
    [22285, 145359, 151101, -12132, 32767],
    [22285, 147785, 150817, -12132, 27423],
    [22285, 147727, 151375, -12132, 37117],
    [22285, 145428, 149494, -12132, 890],
    [22285, 145601, 149682, -12132, 32442],
    [22285, 147003, 149476, -12132, 31554],
    [22285, 147738, 149210, -12132, 20971],
    [22285, 147769, 149757, -12132, 34980],
];

#[rustfmt::skip]
const EMERALD_SPAWNS: [[i32; 5]; 44] = [
    [22280, 144437, 143395, -11969, 34248],
    [22281, 149241, 143735, -12230, 24575],
    [22281, 147917, 146861, -12289, 60306],
    [22281, 144406, 147782, -12133, 14349],
    [22281, 144960, 146881, -12039, 23881],
    [22281, 144985, 147679, -12135, 27594],
    [22283, 147784, 143540, -12222, 2058],
    [22283, 149091, 143491, -12230, 24836],
    [22287, 144479, 147569, -12133, 20723],
    [22287, 145158, 146986, -12058, 21970],
    [22287, 145142, 147175, -12092, 24420],
    [22287, 145110, 147133, -12088, 22465],
    [22287, 144664, 146604, -12028, 14861],
    [22287, 144596, 146600, -12028, 14461],
    [22288, 143925, 146773, -12037, 10813],
    [22288, 144415, 147070, -12069, 8568],
    [22288, 143794, 145584, -12027, 14849],
    [22288, 143429, 146166, -12030, 4078],
    [22288, 144477, 147009, -12056, 8752],
    [22289, 142577, 145319, -12029, 5403],
    [22289, 143831, 146902, -12051, 9717],
    [22289, 143714, 146705, -12028, 10044],
    [22289, 143937, 147134, -12078, 7517],
    [22293, 143356, 145287, -12027, 8126],
    [22293, 143462, 144352, -12008, 25905],
    [22293, 143745, 142529, -11882, 17102],
    [22293, 144574, 144032, -12005, 34668],
    [22295, 143992, 142419, -11884, 19697],
    [22295, 144671, 143966, -12004, 32088],
    [22295, 144440, 143269, -11957, 34169],
    [22295, 142642, 146362, -12028, 281],
    [22295, 143865, 142707, -11881, 21326],
    [22295, 143573, 142530, -11879, 16141],
    [22295, 143148, 146039, -12031, 65014],
    [22295, 143001, 144853, -12014, 0],
    [22296, 147505, 146580, -12260, 59041],
    [22296, 149366, 146932, -12358, 39407],
    [22296, 149284, 147029, -12352, 41120],
    [22296, 149439, 143940, -12230, 23189],
    [22296, 147698, 143995, -12220, 27028],
    [22296, 141885, 144969, -12007, 2526],
    [22296, 147843, 143763, -12220, 28386],
    [22296, 144753, 143650, -11982, 35429],
    [22296, 147613, 146760, -12271, 56296],
];

#[rustfmt::skip]
const ROOM1_SPAWNS: [[i32; 5]; 6] = [
    [22288, 143114, 140027, -11888, 15025],
    [22288, 142173, 140973, -11888, 55698],
    [22289, 143210, 140577, -11888, 17164],
    [22289, 142638, 140107, -11888, 6571],
    [22297, 142547, 140938, -11888, 48556],
    [22298, 142690, 140479, -11887, 7663],
];

#[rustfmt::skip]
const ROOM2_SPAWNS: [[i32; 5]; 5] = [
    [22303, 146276, 141483, -11880, 34643],
    [22287, 145707, 142161, -11880, 28799],
    [22288, 146857, 142129, -11880, 33647],
    [22288, 146869, 142000, -11880, 31215],
    [22289, 146897, 140880, -11880, 19210],
];

#[rustfmt::skip]
const ROOM3_SPAWNS: [[i32; 5]; 5] = [
    [22302, 145123, 143713, -12808, 65323],
    [22294, 145188, 143331, -12808, 496],
    [22294, 145181, 144104, -12808, 64415],
    [22293, 144994, 143431, -12808, 65431],
    [22293, 144976, 143915, -12808, 61461],
];

#[rustfmt::skip]
const ROOM4_SPAWNS: [[i32; 5]; 5] = [
    [22304, 150563, 142240, -12108, 16454],
    [22294, 150769, 142495, -12108, 16870],
    [22281, 150783, 141995, -12108, 20033],
    [22283, 150273, 141983, -12108, 16043],
    [22294, 150276, 142492, -12108, 13540],
];

#[rustfmt::skip]
const STEAM1_SPAWNS: [[i32; 5]; 6] = [
    [22305, 145260, 152387, -12165, 32767],
    [22305, 144967, 152390, -12165, 30464],
    [22305, 145610, 152586, -12165, 17107],
    [22305, 145620, 152397, -12165, 8191],
    [22418, 146081, 152847, -12165, 31396],
    [22418, 146795, 152641, -12165, 33850],
    // {22308, 145093, 152502, -12165, 31841},
    // {22308, 146158, 152776, -12165, 30810},
    // {22308, 146116, 152976, -12133, 32571}
];

#[rustfmt::skip]
const STEAM2_SPAWNS: [[i32; 5]; 5] = [
    [22306, 147740, 152767, -12165, 65043],
    [22306, 148215, 152828, -12165, 970],
    [22306, 147743, 152846, -12165, 64147],
    // {22308, 147849, 152854, -12165, 60534},
    // {22308, 147754, 152908, -12141, 59827},
    // {22308, 148194, 152681, -12165, 63620},
    // {22308, 147767, 152939, -12133, 63381},
    // {22309, 147737, 152671, -12165, 65320},
    [22418, 148207, 152725, -12165, 61801],
    [22419, 149058, 152828, -12165, 64564],
];

#[rustfmt::skip]
const STEAM3_SPAWNS: [[i32; 5]; 4] = [
    [22307, 150735, 152316, -12145, 31930],
    [22307, 150725, 152467, -12165, 33635],
    [22307, 151058, 152316, -12146, 65342],
    [22307, 151057, 152461, -12165, 2171],
    // {22308, 150794, 152455, -12165, 31613},
    // {22308, 150665, 152383, -12165, 32767},
    // {22308, 151697, 152621, -12167, 31423},
    // {22309, 151061, 152581, -12165, 6228},
    // {22309, 150653, 152253, -12132, 31343},
    // {22309, 150628, 152431, -12165, 33022},
    // {22309, 151620, 152487, -12165, 30114},
    // {22309, 151672, 152544, -12165, 31846},
    // {22309, 150488, 152350, -12165, 29072},
    // {22310, 151139, 152238, -12132, 1069}
];

#[rustfmt::skip]
const STEAM4_SPAWNS: [[i32; 5]; 7] = [
    // {22308, 151707, 150199, -12165, 32859},
    // {22308, 152091, 150140, -12165, 32938},
    // {22308, 149757, 150204, -12138, 65331},
    // {22308, 149950, 150307, -12132, 62437},
    // {22308, 149901, 150322, -12132, 62136},
    // {22309, 150071, 150173, -12165, 64943},
    [22416, 151636, 150280, -12142, 36869],
    [22416, 149893, 150232, -12165, 64258],
    [22416, 149864, 150110, -12165, 65054],
    [22416, 151926, 150218, -12165, 31613],
    [22420, 149986, 150051, -12165, 105],
    [22420, 151970, 149997, -12165, 32170],
    [22420, 150744, 150006, -12165, 63],
    // {22417, 149782, 150188, -12151, 64001}
];

/// Java `CrystalGolem`.
#[derive(Debug, Default)]
struct CrystalGolem {
    food_item: Option<ObjectId>,
    is_at_destination: bool,
    old_loc: Option<(i32, i32, i32, i32)>,
}

/// Java `CCWorld`, without the status (the `InstanceWorld`'s own).
#[derive(Debug, Default)]
struct CcWorld {
    npc_list1: HashMap<ObjectId, bool>,
    tears: Option<ObjectId>,
    is_used_invul_skill: bool,
    dragon_scale_start: i64,
    dragon_scale_need: i32,
    cleaned_rooms: i32,
    end_time: i64,
    copys: Vec<ObjectId>,
    crystal_golems: HashMap<ObjectId, CrystalGolem>,
    correct_golems: i32,
    oracle_triggered: [bool; 3],
    kechis_henchman_spawn: i32,
    /// 0: not spawned, 1: spawned, 2: cleared
    rooms_status: [i32; 4],
    /// `openedDoors`: door id -> the player who opened it.
    opened_doors: HashMap<i32, ObjectId>,
    npc_list2: HashMap<i32, HashMap<ObjectId, bool>>,
    oracles: HashMap<ObjectId, Option<ObjectId>>,
    key_keepers: Vec<ObjectId>,
    guards: Vec<ObjectId>,
    oracle: Vec<ObjectId>,

    // baylor variables
    raiders: Vec<ObjectId>,
    raid_status: i32,
    dragon_claw_start: i64,
    dragon_claw_need: i32,
    animation_mobs: Vec<ObjectId>,
    camera: Option<ObjectId>,
    baylor: Option<ObjectId>,
    alarm: Option<ObjectId>,
}

/// What Java keeps in its `CCWorld`s.
#[derive(Debug, Default)]
struct State {
    worlds: HashMap<i32, CcWorld>,
    /// The instance of every NPC this script spawned: a Java NPC object keeps its `instanceId`
    /// after `decayMe()`/`removeNpcs()`, the world's table does not.
    npc_instance: HashMap<ObjectId, i32>,
}

/// Java `instances.CrystalCaverns.CrystalCaverns`.
pub struct CrystalCaverns;

impl Script for CrystalCaverns {
    fn name(&self) -> &'static str {
        "CrystalCaverns"
    }

    fn path(&self) -> &'static str {
        "instances/CrystalCaverns"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[ORACLE_GUIDE_1, ORACLE_GUIDE_4]);
        s.add_talk_id(&[
            ORACLE_GUIDE_1,
            ORACLE_GUIDE_3,
            ORACLE_GUIDE_4,
            32275,
            32276,
            32277,
        ]);
        s.add_first_talk_id(&[
            ORACLE_GUIDE_1,
            ORACLE_GUIDE_2,
            ORACLE_GUIDE_4,
            CRYSTALLINE_GOLEM,
            32274,
            32275,
            32276,
            32277,
        ]);
        s.add_kill_id(&[
            TEARS,
            GATEKEEPER_LOHAN,
            GATEKEEPER_PROVO,
            TEROD,
            WEYLIN,
            DOLPH,
            DARNEL,
            KECHI,
            GUARDIAN_OF_THE_SQUARE,
            GUARDIAN_OF_THE_EMERALD,
            TOURMALINE,
            BAYLOR,
            ALARM,
        ]);
        s.add_skill_see_id(&[BAYLOR, 25534, 32275, 32276, 32277]);
        // `addTrapActionId(DOOR_OPENING_TRAP[0])`: the engine registers the event for every trap.
        s.set_on_trap_action();
        s.add_spell_finished_id(&[BAYLOR]);
        s.add_attack_id(&[TEARS]);
        s.add_kill_id(&MOBLIST);
        s.add_kill_id(&CGMOBS);
        s.add_enter_zone_id(&ZONES);
        s.add_exit_zone_id(&ZONES);
    }

    fn on_first_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        if npc.id == ORACLE_GUIDE_1 {
            if let Some(id) = cc_world(ctx, npc.object_id) {
                let first = status_of(ctx, id) == 0
                    && ctx.state::<State>().worlds[&id].oracle.contains(&npc.object_id);
                if first {
                    return Some("32281.htm".to_owned()); // TODO: Missing HTML.
                }
            }

            ctx.show_chat_window(npc, player);
            return None;
        } else if (32275..=32277).contains(&npc.id) {
            if let Some(id) = cc_world(ctx, npc.object_id) {
                let triggered = ctx.state::<State>().worlds[&id].oracle_triggered[(npc.id - 32275) as usize];
                if !triggered {
                    return Some("no.htm".to_owned()); // TODO: Missing HTML.
                }

                ctx.show_chat_window(npc, player);
                return None;
            }
        } else if npc.id == 32274 {
            if cc_world(ctx, npc.object_id).is_some() {
                return Some("no.htm".to_owned()); // TODO: Missing HTML.
            }
        } else if npc.id == 32279 {
            let qs = ctx
                .player_quest_state(player, "Q00131_BirdInACage")
                .map(|q| q.state());
            return Some(
                if qs.is_some_and(|s| s != COMPLETED) {
                    "32279-01.htm"
                } else {
                    "32279.htm"
                }
                .to_owned(),
            );
        } else if npc.id == CRYSTALLINE_GOLEM {
            ctx.world().send(player, &ActionFailed);
        }

        None
    }

    fn on_skill_see(
        &self,
        ctx: &mut ScriptCtx<'_>,
        npc: ScriptNpc,
        caster: ObjectId,
        skill: SkillKey,
        targets: &[ObjectId],
        _is_summon: bool,
    ) {
        if !targets.contains(&npc.object_id) {
            return;
        }

        match skill.id {
            1011 | 1015 | 1217 | 1218 | 1401 | 2360 | 2369 | 5146 => {}
            _ => return,
        }

        if (32275..=32277).contains(&npc.id) && skill.id != 2360 && skill.id != 2369 {
            if let Some(id) = cc_world(ctx, npc.object_id) {
                if ctx.get_random(100) < 15 {
                    let others: Vec<ObjectId> = ctx.state::<State>().worlds[&id]
                        .oracles
                        .keys()
                        .copied()
                        .filter(|o| *o != npc.object_id)
                        .collect();
                    for oracle in others {
                        ctx.world_mut().decay(oracle);
                    }

                    if let Some(w) = ctx.state::<State>().worlds.get_mut(&id) {
                        w.oracle_triggered[(npc.id - 32275) as usize] = true;
                    }
                }
            }
        } else if ctx.world().is_invul(npc.object_id) && npc.id == BAYLOR && skill.id == 2360 {
            let Some(party) = ctx.world().party_of(caster) else {
                return;
            };

            if let Some(id) = cc_world(ctx, npc.object_id) {
                let now = ctx.world().instance_clock_ms();
                let members = ctx.world().party_members(party).len() as i32;
                let need = {
                    let Some(w) = ctx.state::<State>().worlds.get_mut(&id) else {
                        return;
                    };
                    if w.dragon_claw_start + DRAGONCLAWTIME <= now || w.dragon_claw_need <= 0 {
                        w.dragon_claw_start = now;
                        w.dragon_claw_need = members - 1;
                    } else {
                        w.dragon_claw_need -= 1;
                    }
                    w.dragon_claw_need
                };

                if need == 0 {
                    ctx.world_mut()
                        .stop_skill_effects(npc.object_id, SkillFinishType::Silent, 5225);
                    ctx.broadcast_magic_skill_use(
                        npc.object_id,
                        npc.object_id,
                        npc.object_id,
                        5480,
                        1,
                        4000,
                        0,
                    );
                    if let Some(w) = ctx.state::<State>().worlds.get_mut(&id) {
                        if w.raid_status == 3 {
                            w.raid_status += 1;
                        }
                    }
                }
            }
        } else if ctx.world().is_invul(npc.object_id) && npc.id == TEARS && skill.id == 2369 {
            if let Some(id) = cc_world(ctx, npc.object_id) {
                let party = ctx.world().party_of(caster);
                if party.is_none() && !ctx.is_gm(caster) {
                    return;
                }

                let now = ctx.world().instance_clock_ms();
                let members = party.map(|p| ctx.world().party_members(p).len() as i32);
                let need = {
                    let Some(w) = ctx.state::<State>().worlds.get_mut(&id) else {
                        return;
                    };
                    if w.dragon_scale_start + DRAGONSCALETIME <= now || w.dragon_scale_need <= 0 {
                        w.dragon_scale_start = now;
                        w.dragon_scale_need = match members {
                            None => 0, // (party == null) && caster.isGM()
                            Some(members) => members - 1,
                        };
                    } else {
                        w.dragon_scale_need -= 1;
                    }
                    w.dragon_scale_need
                };

                if need == 0 && ctx.get_random(100) < 80 {
                    ctx.world_mut().set_invul(npc.object_id, false);
                }
            }
        }
    }

    fn on_attack(
        &self,
        ctx: &mut ScriptCtx<'_>,
        npc: ScriptNpc,
        attacker: ObjectId,
        _damage: i32,
        _is_summon: bool,
        skill: Option<SkillKey>,
    ) {
        let Some(id) = cc_world(ctx, npc.object_id) else {
            return;
        };

        let tears = ctx.state::<State>().worlds[&id].tears;
        if status_of(ctx, id) != 4 {
            // Lucky cheater, the code only kicks his/her ass out of the dungeon
            ctx.teleport_player(attacker, 149361, 172327, -945, 0, true);
            if let Some(world) = ctx.world_mut().instance_world_mut(id) {
                world.remove_allowed(attacker);
            }
        } else if tears != Some(npc.object_id) {
            return;
        } else if !ctx.state::<State>().worlds[&id].copys.is_empty() {
            let mut not_aoe = true;
            if skill.is_some_and(|s| ctx.world().skill_target_is_area(s)) {
                not_aoe = false;
            }

            if not_aoe {
                let copys = ctx
                    .state::<State>()
                    .worlds
                    .get_mut(&id)
                    .map(|w| std::mem::take(&mut w.copys))
                    .unwrap_or_default();
                for copy in copys {
                    ctx.world_mut().delete_npc(copy); // copy.onDecay()
                }
            }
            return;
        }

        let Some((now_hp, max_hp)) = ctx.world().creature_hp(npc.object_id) else {
            return;
        };
        let rand = ctx.get_random(1000);
        let used_invul = ctx.state::<State>().worlds[&id].is_used_invul_skill;
        if now_hp < max_hp * 0.4 && rand < 5 {
            let party = ctx.world().party_of(attacker);
            if let Some(party) = party {
                for member in ctx.world().party_members(party) {
                    stop_attack(ctx, member);
                }
            } else {
                stop_attack(ctx, attacker);
            }

            let target = ctx.world().attack_target_of(npc.object_id);
            for _ in 0..10 {
                let (x, y, z) = ctx.world().position_of(npc.object_id).unwrap_or_default();
                let instance = ctx.world().instance_id_of(attacker);
                if let Some(copy) = spawn(ctx, TEARS_COPY, x, y, z, 0, instance) {
                    ctx.world_mut().set_npc_running(copy);
                    if let Some(target) = target {
                        ctx.world_mut().add_damage_hate(copy, target, 0, 99999);
                        ctx.world_mut().set_intention_attack(copy, target);
                    }
                    ctx.world_mut().set_current_hp(copy, now_hp, true);
                    if let Some(w) = ctx.state::<State>().worlds.get_mut(&id) {
                        w.copys.push(copy);
                    }
                }
            }
        } else if now_hp < max_hp * 0.15 && !used_invul && (rand > 994 || now_hp < max_hp * 0.1) {
            if let Some(w) = ctx.state::<State>().worlds.get_mut(&id) {
                w.is_used_invul_skill = true;
            }
            ctx.world_mut().set_invul(npc.object_id, true);
        }
    }

    fn on_spell_finished(
        &self,
        ctx: &mut ScriptCtx<'_>,
        npc: ScriptNpc,
        _player: Option<ObjectId>,
        skill: SkillKey,
    ) {
        if npc.id == BAYLOR && skill.id == 5225 {
            if let Some(id) = cc_world(ctx, npc.object_id) {
                if let Some(w) = ctx.state::<State>().worlds.get_mut(&id) {
                    w.raid_status += 1;
                }
            }
        }
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        // `InstanceManager.getInstance().getWorld(npc)`: a null NPC is an exception in Java.
        let npc = npc?;
        let id = cc_world(ctx, npc.object_id)?;
        let is = |name: &str| event.eq_ignore_ascii_case(name);
        if is("TeleportOut") {
            ctx.teleport_player(player?, 149413, 173078, -5014, 0, true);
        } else if is("TeleportParme") {
            ctx.teleport_player(player?, 153689, 142226, -9750, id, true);
        } else if is("Timer2") || is("Timer3") || is("Timer4") || is("Timer5") {
            let player = player?;
            if ctx.world().instance_id_of(player) == id {
                ctx.teleport_player(player, 144653, 152606, -12126, id, true);
                ctx.world_mut()
                    .stop_skill_effects(player, SkillFinishType::Removed, 5239);
                ctx.world_mut()
                    .apply_effects(player, player, SkillKey::new(5239, 1));
                ctx.start_quest_timer("Timer2", 300000, Some(npc), Some(player), false);
            }
        } else if is("Timer21") || is("Timer31") || is("Timer41") || is("Timer51") {
            ctx.world_mut().remove_instance_npcs(id);
            if let Some(w) = ctx.state::<State>().worlds.get_mut(&id) {
                w.npc_list2.clear();
            }
            run_steam_rooms(ctx, id, &STEAM1_SPAWNS, 22);
            ctx.start_quest_timer("Timer21", 300000, Some(npc), None, false);
        } else if is("checkKechiAttack") {
            if ctx.is_in_combat(npc.object_id) {
                ctx.start_quest_timer("spawnGuards", SPAWN[0], Some(npc), None, false);
                ctx.cancel_quest_timers("checkKechiAttack");
                ctx.world_mut().instance_close_door(id, DOOR4);
                ctx.world_mut().instance_close_door(id, DOOR3);
            } else {
                ctx.start_quest_timer("checkKechiAttack", 1000, Some(npc), None, false);
            }
        } else if is("spawnGuards") {
            let spawned = {
                let Some(w) = ctx.state::<State>().worlds.get_mut(&id) else {
                    return None;
                };
                w.kechis_henchman_spawn += 1;
                w.kechis_henchman_spawn
            };
            for (x, y, z, heading) in [
                (153622, 149699, -12131, 56890),
                (153609, 149622, -12131, 64023),
                (153606, 149428, -12131, 64541),
                (153601, 149534, -12131, 64901),
                (153620, 149354, -12131, 1164),
                (153637, 149776, -12131, 61733),
                (153638, 149292, -12131, 64071),
                (153647, 149857, -12131, 59402),
                (153661, 149227, -12131, 65275),
            ] {
                if let Some(guard) = spawn(ctx, KECHIS_HENCHMAN, x, y, z, heading, id) {
                    if let Some(w) = ctx.state::<State>().worlds.get_mut(&id) {
                        w.guards.push(guard);
                    }
                }
            }
            if spawned <= 5 {
                ctx.start_quest_timer("spawnGuards", SPAWN[spawned as usize], Some(npc), None, false);
            } else {
                ctx.cancel_quest_timers("spawnGuards");
            }
        } else if is("EmeraldSteam") {
            run_emerald(ctx, id);
            decay_oracles(ctx, id);
        } else if is("CoralGarden") {
            run_coral(ctx, id);
            decay_oracles(ctx, id);
        } else if is("spawn_oracle") {
            spawn(ctx, 32271, 153572, 142075, -9728, 10800, id);
            // Baylor's Chest
            let chest = if ctx.get_random(10) < 5 { 29116 } else { 29117 };
            let (x, y, z) = ctx.world().position_of(npc.object_id).unwrap_or_default();
            let heading = ctx.world().heading_of(npc.object_id);
            spawn(ctx, chest, x, y, z, heading, id);
            spawn(ctx, ORACLE_GUIDE_4, 153572, 142075, -12738, 10800, id);
            ctx.cancel_quest_timer("baylor_despawn", Some(npc), None);
            ctx.cancel_quest_timers("baylor_skill");
        } else if is("baylorEffect0") {
            ctx.world_mut().script_set_intention_idle(npc.object_id);
            social_action(ctx, npc, 1);
            ctx.start_quest_timer("baylorCamera0", 11000, Some(npc), None, false);
            ctx.start_quest_timer("baylorEffect1", 19000, Some(npc), None, false);
        } else if is("baylorCamera0") {
            special_camera(ctx, npc, [500, -45, 170, 5000, 9000, 0, 0, 1, 0, 0]);
        } else if is("baylorEffect1") {
            special_camera(ctx, npc, [300, 0, 120, 2000, 5000, 0, 0, 1, 0, 0]);
            social_action(ctx, npc, 3);
            ctx.start_quest_timer("baylorEffect2", 4000, Some(npc), None, false);
        } else if is("baylorEffect2") {
            special_camera(ctx, npc, [747, 0, 160, 2000, 3000, 0, 0, 1, 0, 0]);
            ctx.broadcast_magic_skill_use(npc.object_id, npc.object_id, npc.object_id, 5402, 1, 2000, 0);
            ctx.start_quest_timer("RaidStart", 2000, Some(npc), None, false);
        } else if is("BaylorMinions") {
            for i in 0..10 {
                let radius = 300.0_f64;
                let x = (radius * (f64::from(i) * 0.618).cos()) as i32;
                let y = (radius * (f64::from(i) * 0.618).sin()) as i32;
                if let Some(mob) = spawn(ctx, 29104, 153571 + x, 142075 + y, -12737, 0, id) {
                    ctx.world_mut().script_set_intention_idle(mob);
                    if let Some(w) = ctx.state::<State>().worlds.get_mut(&id) {
                        w.animation_mobs.push(mob);
                    }
                }
            }

            ctx.start_quest_timer("baylorEffect0", 200, Some(npc), None, false);
        } else if is("RaidStart") {
            let camera = ctx
                .state::<State>()
                .worlds
                .get_mut(&id)
                .and_then(|w| w.camera.take());
            if let Some(camera) = camera {
                ctx.world_mut().decay(camera);
            }
            ctx.world_mut().set_creature_paralyzed(npc.object_id, false);
            let raiders = ctx.state::<State>().worlds[&id].raiders.clone();
            for p in raiders {
                ctx.world_mut().set_creature_paralyzed(p, false);
                ctx.world_mut().throw_creature(npc.object_id, p);
                if let Some(summon) = ctx.world().summon_of(p) {
                    ctx.world_mut().throw_creature(npc.object_id, summon);
                }
            }

            let mobs = {
                let Some(w) = ctx.state::<State>().worlds.get_mut(&id) else {
                    return None;
                };
                w.raid_status = 0;
                std::mem::take(&mut w.animation_mobs)
            };
            for mob in mobs {
                ctx.world_mut().do_die(mob, Some(mob));
            }

            ctx.start_quest_timer("baylor_despawn", 60000, Some(npc), None, true);
            ctx.start_quest_timer("checkBaylorAttack", 1000, Some(npc), None, false);
        } else if is("checkBaylorAttack") {
            if ctx.is_in_combat(npc.object_id) {
                ctx.cancel_quest_timers("checkBaylorAttack");
                ctx.start_quest_timer("baylor_alarm", 40000, Some(npc), None, false);
                ctx.start_quest_timer("baylor_skill", 5000, Some(npc), None, true);
                if let Some(w) = ctx.state::<State>().worlds.get_mut(&id) {
                    w.raid_status += 1;
                }
            } else {
                ctx.start_quest_timer("checkBaylorAttack", 1000, Some(npc), None, false);
            }
        } else if is("baylor_alarm") {
            let no_alarm = ctx.state::<State>().worlds[&id].alarm.is_none();
            if no_alarm {
                let spawn_loc = ctx.get_random_entry(&ALARMSPAWN).unwrap_or(ALARMSPAWN[0]);
                ctx.world_mut().add_skill(npc.object_id, 5244, 1);
                ctx.world_mut().add_skill(npc.object_id, 5245, 1);
                let alarm = spawn(ctx, ALARM, spawn_loc[0], spawn_loc[1], spawn_loc[2], 10800, id);
                if let Some(w) = ctx.state::<State>().worlds.get_mut(&id) {
                    w.alarm = alarm;
                }
                if let Some(alarm) = alarm {
                    if let Some(script_npc) = ctx.script_npc(alarm) {
                        ctx.disable_core_ai(script_npc, true);
                        ctx.set_immobilized(script_npc, true);
                    }
                    creature_say(
                        ctx,
                        alarm,
                        ChatType::Shout,
                        NpcStringId::AnAlarmHasBeenSetOffEverybodyWillBeInDangerIfTheyAreNotTakenCareOfImmediately,
                    );
                }
            }
        } else if is("baylor_skill") {
            let (baylor, raid_status) = {
                let w = &ctx.state::<State>().worlds[&id];
                (w.baylor, w.raid_status)
            };
            if baylor.is_none() {
                ctx.cancel_quest_timers("baylor_skill");
            } else {
                let (now_hp, max_hp) = ctx.world().creature_hp(npc.object_id).unwrap_or_default();
                let rand = ctx.get_random(100);
                let affected_5224 = ctx.is_affected_by_skill(npc.object_id, 5224);
                let affected_5225 = ctx.is_affected_by_skill(npc.object_id, 5225);
                if now_hp < max_hp * 0.2 && raid_status < 3 && !affected_5224 && !affected_5225 {
                    if now_hp < max_hp * 0.15 && raid_status == 2 {
                        ctx.world_mut().do_cast(npc.object_id, SkillKey::new(5225, 1));
                        creature_say(
                            ctx,
                            npc.object_id,
                            ChatType::Shout,
                            NpcStringId::DemonKingBelethGiveMeThePowerAaahh,
                        );
                    } else if rand < 10 || now_hp < max_hp * 0.15 {
                        ctx.world_mut().do_cast(npc.object_id, SkillKey::new(5225, 1));
                        creature_say(
                            ctx,
                            npc.object_id,
                            ChatType::Shout,
                            NpcStringId::DemonKingBelethGiveMeThePowerAaahh,
                        );
                        let baylor = baylor.and_then(|b| ctx.script_npc(b));
                        ctx.start_quest_timer("baylor_remove_invul", 30000, baylor, None, false);
                    }
                } else if now_hp < max_hp * 0.3 && rand > 50 && !affected_5225 && !affected_5224 {
                    ctx.world_mut().do_cast(npc.object_id, SkillKey::new(5224, 1));
                } else if rand < 33 {
                    let raiders = ctx.state::<State>().worlds[&id].raiders.clone();
                    let target = ctx.get_random_entry(&raiders);
                    ctx.world_mut().set_npc_target(npc.object_id, target);
                    ctx.world_mut().do_cast(npc.object_id, SkillKey::new(5229, 1));
                }
            }
        } else if is("baylor_remove_invul") {
            ctx.world_mut()
                .stop_skill_effects(npc.object_id, SkillFinishType::Silent, 5225);
        } else if is("Baylor") {
            let baylor = spawn(ctx, 29099, 153572, 142075, -12738, 10800, id);
            if let Some(baylor) = baylor {
                ctx.world_mut().set_creature_paralyzed(baylor, true);
            }
            let camera = spawn(ctx, 29120, 153273, 141400, -12738, 10800, id);
            if let Some(w) = ctx.state::<State>().worlds.get_mut(&id) {
                w.baylor = baylor;
                w.camera = camera;
            }
            if let Some(camera) = camera.and_then(|c| ctx.script_npc(c)) {
                special_camera(ctx, camera, [700, -45, 160, 500, 15200, 0, 0, 1, 0, 0]);
            }
            let baylor = baylor.and_then(|b| ctx.script_npc(b));
            ctx.start_quest_timer("baylorMinions", 2000, baylor, None, false);
        } else if !event.ends_with("Food") {
            return None;
        } else if is("autoFood") {
            let Some(w) = ctx.state::<State>().worlds.get_mut(&id) else {
                return None;
            };
            w.crystal_golems.entry(npc.object_id).or_default();
            let golem = &w.crystal_golems[&npc.object_id];
            let busy = golem.food_item.is_some() || golem.is_at_destination;
            if status_of(ctx, id) != 3 || busy {
                return None;
            }

            let nearest = ctx
                .world()
                .nearest_ground_item(npc.object_id, CRYSTAL_FRAGMENT, 300);
            if let Some(nearest) = nearest {
                if let Some(g) = golem_of(ctx, id, npc.object_id) {
                    g.food_item = Some(nearest);
                }
                ctx.start_quest_timer("getFood", 2000, Some(npc), None, false);
            } else {
                if ctx.get_random(100) < 5 {
                    creature_say(ctx, npc.object_id, ChatType::Shout, NpcStringId::AhIMHungry);
                }

                ctx.start_quest_timer("autoFood", 2000, Some(npc), None, false);
            }

            return None;
        } else if golem_of(ctx, id, npc.object_id).is_none_or(|g| g.is_at_destination) {
            return None;
        } else if is("backFood") {
            if ctx.is_intention_active(npc) {
                ctx.cancel_quest_timers("backFood");
                ctx.world_mut().script_set_intention_idle(npc.object_id);
                if let Some(g) = golem_of(ctx, id, npc.object_id) {
                    g.food_item = None;
                }
                ctx.start_quest_timer("autoFood", 2000, Some(npc), None, false);
            }
        } else if is("reachFood") {
            let (food, old_loc) = golem_of(ctx, id, npc.object_id).map(|g| (g.food_item, g.old_loc))?;
            if food.is_none_or(|f| ctx.world().ground_item(f).is_none()) {
                if let Some((x, y, z, _)) = old_loc {
                    ctx.npc_move_to(npc, x, y, z);
                }
                ctx.cancel_quest_timers("reachFood");
                ctx.start_quest_timer("backFood", 2000, Some(npc), None, true);
                return None;
            } else if ctx.is_intention_active(npc) {
                if let Some(food) = food {
                    ctx.world_mut().remove_ground_item(food);
                }
                ctx.world_mut().script_set_intention_idle(npc.object_id);
                if let Some(g) = golem_of(ctx, id, npc.object_id) {
                    g.food_item = None;
                }
                let (x, y, _) = ctx.world().position_of(npc.object_id).unwrap_or_default();
                let d1 = (x.wrapping_sub(142999))
                    .wrapping_mul(x.wrapping_sub(142999))
                    .wrapping_add((y.wrapping_sub(151671)).wrapping_mul(y.wrapping_sub(151671)));
                let d2 = (x.wrapping_sub(139494))
                    .wrapping_mul(x.wrapping_sub(139494))
                    .wrapping_add((y.wrapping_sub(151668)).wrapping_mul(y.wrapping_sub(151668)));
                if d1 < 10000 || d2 < 10000 {
                    ctx.broadcast_magic_skill_use(npc.object_id, npc.object_id, npc.object_id, 5441, 1, 1, 0);
                    if let Some(g) = golem_of(ctx, id, npc.object_id) {
                        g.is_at_destination = true;
                    }
                    let correct = {
                        let Some(w) = ctx.state::<State>().worlds.get_mut(&id) else {
                            return None;
                        };
                        w.correct_golems += 1;
                        w.correct_golems
                    };
                    if correct >= 2 {
                        ctx.world_mut().instance_open_door(id, 24220026);
                        set_status(ctx, id, 4);
                    }
                } else {
                    ctx.start_quest_timer("autoFood", 2000, Some(npc), None, false);
                }

                ctx.cancel_quest_timers("reachFood");
            }

            return None;
        } else if is("getFood") {
            let food = golem_of(ctx, id, npc.object_id).and_then(|g| g.food_item)?;
            let (fx, fy, fz) = ctx.world().ground_item(food).map(|i| (i.x, i.y, i.z))?;
            let (x, y, z) = ctx.world().position_of(npc.object_id).unwrap_or_default();
            let heading = ctx.world().heading_of(npc.object_id);
            if let Some(g) = golem_of(ctx, id, npc.object_id) {
                g.old_loc = Some((x, y, z, heading));
            }
            ctx.npc_move_to(npc, fx, fy, fz);
            ctx.start_quest_timer("reachFood", 2000, Some(npc), None, true);
            ctx.cancel_quest_timers("getFood");
        }

        None
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId, _is_summon: bool) {
        let Some(id) = cc_world(ctx, npc.object_id) else {
            return;
        };
        let status = status_of(ctx, id);
        let is_listed_mob = ctx.state::<State>().worlds[&id]
            .npc_list1
            .contains_key(&npc.object_id);
        let is_key_keeper = ctx.state::<State>().worlds[&id]
            .key_keepers
            .contains(&npc.object_id);
        if status == 2 && is_listed_mob {
            let all_dead = {
                let Some(w) = ctx.state::<State>().worlds.get_mut(&id) else {
                    return;
                };
                w.npc_list1.insert(npc.object_id, true);
                w.npc_list1.values().all(|d| *d)
            };
            if !all_dead {
                return;
            }

            set_status(ctx, id, 3);
            let tears = spawn(ctx, TEARS, 144298, 154420, -11854, 32767, id); // Tears
            let mut golems = Vec::new();
            for (x, y, z, heading) in [(140547, 151670, -11813, 32767), (141941, 151684, -11813, 63371)] {
                golems.extend(spawn(ctx, CRYSTALLINE_GOLEM, x, y, z, heading, id));
            }
            if let Some(w) = ctx.state::<State>().worlds.get_mut(&id) {
                w.tears = tears;
                for golem in &golems {
                    w.crystal_golems.insert(*golem, CrystalGolem::default());
                }
            }
            for golem in golems {
                let golem = ctx.script_npc(golem);
                ctx.start_quest_timer("autoFood", 2000, golem, None, false);
            }
        } else if status == 4 && npc.id == TEARS {
            ctx.world_mut().set_instance_duration(id, 300000);
            spawn(ctx, 32280, 144312, 154420, -11855, 0, id);
            let instance = ctx.world().instance_id_of(npc.object_id);
            give_rewards(ctx, player, instance, CLEAR_CRYSTAL, false);
        } else if status == 2 && is_key_keeper {
            if npc.id == GATEKEEPER_LOHAN {
                ctx.world_mut()
                    .npc_drop_item(npc.object_id, Some(player), 9698, 1);
                run_emerald_square(ctx, id);
            } else if npc.id == GATEKEEPER_PROVO {
                ctx.world_mut()
                    .npc_drop_item(npc.object_id, Some(player), 9699, 1);
                run_steam_rooms(ctx, id, &STEAM1_SPAWNS, 22);
                if let Some(party) = ctx.world().party_of(player) {
                    for member in ctx.world().party_members(party) {
                        if ctx.world().instance_id_of(member) == id {
                            ctx.world_mut()
                                .apply_effects(member, member, SkillKey::new(5239, 1));
                            ctx.start_quest_timer("Timer2", 300000, Some(npc), Some(member), false);
                        }
                    }
                } else {
                    ctx.world_mut()
                        .apply_effects(player, player, SkillKey::new(5239, 1));
                    ctx.start_quest_timer("Timer2", 300000, Some(npc), Some(player), false);
                }

                ctx.start_quest_timer("Timer21", 300000, Some(npc), None, false);
            }

            let keepers = ctx.state::<State>().worlds[&id].key_keepers.clone();
            for gk in keepers {
                if gk != npc.object_id {
                    ctx.world_mut().decay(gk);
                }
            }
        } else if status == 3 {
            if check_kill_progress(ctx, id, 0, npc.object_id) {
                set_status(ctx, id, 4);
                spawn(ctx, TOURMALINE, 148202, 144791, -12235, 0, id);
            } else {
                return;
            }
        } else if status == 4 {
            if npc.id == TOURMALINE {
                set_status(ctx, id, 5);
                spawn(ctx, TEROD, 147777, 146780, -12281, 0, id);
            }
        } else if status == 5 {
            if npc.id == TEROD {
                set_status(ctx, id, 6);
                spawn(ctx, TOURMALINE, 143694, 142659, -11882, 0, id);
            }
        } else if status == 6 {
            if npc.id == TOURMALINE {
                set_status(ctx, id, 7);
                spawn(ctx, DOLPH, 142054, 143288, -11825, 0, id);
            }
        } else if status == 7 {
            if npc.id == DOLPH {
                set_status(ctx, id, 8);

                // first door opener trap
                // NOTE: the engine has no NPC traps (`addTrap`): the trap, and the shout it
                // makes, are not there, so the secret room door is not opened by it.
            }
        } else if status == 8 {
            for i in 0..4 {
                let rooms_status = ctx.state::<State>().worlds[&id].rooms_status[i];
                if rooms_status == 1 && check_kill_progress(ctx, id, i as i32 + 1, npc.object_id) {
                    if let Some(w) = ctx.state::<State>().worlds.get_mut(&id) {
                        w.rooms_status[i] = 2;
                    }
                }

                if ctx.state::<State>().worlds[&id].rooms_status[i] == 2 {
                    let cleaned = {
                        let Some(w) = ctx.state::<State>().worlds.get_mut(&id) else {
                            return;
                        };
                        w.cleaned_rooms += 1;
                        w.cleaned_rooms
                    };
                    if cleaned == 21 {
                        run_darnel(ctx, id);
                    }
                }
            }
        } else if (22..=25).contains(&status) {
            if npc.id == 22416 {
                if let Some(w) = ctx.state::<State>().worlds.get_mut(&id) {
                    for oracle in w.oracles.values_mut() {
                        if *oracle == Some(npc.object_id) {
                            *oracle = None;
                        }
                    }
                }
            }

            if check_kill_progress(ctx, id, 0, npc.object_id) {
                if let Some(w) = ctx.state::<State>().worlds.get_mut(&id) {
                    w.npc_list2.clear();
                }
                let oracle_order: &[[i32; 5]] = match status {
                    22 => {
                        ctx.world_mut().instance_close_door(id, DOOR6);
                        &ORDRE_ORACLE1
                    }
                    23 => &ORDRE_ORACLE2,
                    24 => &ORDRE_ORACLE3,
                    25 => {
                        set_status(ctx, id, 26);
                        if let Some(party) = ctx.world().party_of(player) {
                            for member in ctx.world().party_members(party) {
                                ctx.world_mut()
                                    .stop_skill_effects(member, SkillFinishType::Removed, 5239);
                            }
                        }

                        ctx.cancel_quest_timers("Timer5");
                        ctx.cancel_quest_timers("Timer51");
                        ctx.world_mut().instance_open_door(id, DOOR3);
                        ctx.world_mut().instance_open_door(id, DOOR4);
                        let kechi = spawn(ctx, KECHI, 154069, 149525, -12158, 51165, id);
                        let kechi = kechi.and_then(|k| ctx.script_npc(k));
                        ctx.start_quest_timer("checkKechiAttack", 1000, kechi, None, false);
                        return;
                    }
                    _ => {
                        // CrystalCavern-SteamCorridor: status error. OracleOrder not found.
                        tracing::warn!(
                            "CrystalCavern-SteamCorridor: status {status} error. OracleOrder not found in {id}"
                        );
                        return;
                    }
                };

                run_steam_oracles(ctx, id, oracle_order);
            }
        } else if (status == 9 && npc.id == DARNEL) || (status == 26 && npc.id == KECHI) {
            ctx.world_mut().set_instance_duration(id, 300000);
            let boss_cry;
            if npc.id == KECHI {
                boss_cry = RED_CRYSTAL;
                ctx.cancel_quest_timers("spawnGuards");
                spawn(ctx, 32280, 154077, 149527, -12159, 0, id);
            } else if npc.id == DARNEL {
                boss_cry = BLUE_CRYSTAL;
                spawn(ctx, 32280, 152761, 145950, -12588, 0, id);
            } else {
                // something is wrong
                return;
            }

            let instance = ctx.world().instance_id_of(npc.object_id);
            give_rewards(ctx, player, instance, boss_cry, false);
        }

        if npc.id == ALARM {
            let baylor = {
                let Some(w) = ctx.state::<State>().worlds.get_mut(&id) else {
                    return;
                };
                w.alarm = None;
                w.baylor
            };
            if let Some(baylor) = baylor {
                ctx.world_mut().remove_skill(baylor, 5244, true);
                ctx.world_mut().remove_skill(baylor, 5245, true);

                if let Some((now_hp, max_hp)) = ctx.world().creature_hp(baylor) {
                    if max_hp * 0.3 < now_hp {
                        let baylor = ctx.script_npc(baylor);
                        ctx.start_quest_timer("baylor_alarm", 40000, baylor, None, false);
                    }
                }
            }
        } else if npc.id == BAYLOR {
            set_status(ctx, id, 31);
            if let Some(w) = ctx.state::<State>().worlds.get_mut(&id) {
                w.baylor = None;
            }
            let (x, y, z) = ctx.world().position_of(npc.object_id).unwrap_or_default();
            ctx.world().broadcast_packet(
                npc.object_id,
                &PlaySound {
                    unknown1: 1,
                    sound_file: "BS01_D".to_owned(),
                    unknown3: 1,
                    unknown4: npc.object_id.0,
                    unknown5: x,
                    unknown6: y,
                    unknown7: z,
                    unknown8: 0,
                },
            );
            let instance = ctx.world().instance_id_of(npc.object_id);
            ctx.world_mut().set_instance_duration(instance, 300000);
            ctx.start_quest_timer("spawn_oracle", 1000, Some(npc), None, false);
            give_rewards(ctx, player, instance, -1, true);
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let npc_id = npc.id;
        let _ = ctx.get_quest_state(player, true);
        if npc_id == ORACLE_GUIDE_1 {
            let existing = ctx
                .world()
                .player_instance_world(player)
                .map(|w| (w.instance_id, w.template_id));
            if let Some((instance_id, template_id)) = existing {
                if template_id == TEMPLATE_ID {
                    on_enter_instance(ctx, player, instance_id, false);
                    return None;
                }

                ctx.world().send(
                    player,
                    &SystemMessage::new(
                        SystemMessageId::YouHaveEnteredAnotherInstanceZoneThereforeYouCannotEnterCorrespondingDungeon,
                    ),
                );
                return None;
            }

            if check_conditions(ctx, player) {
                let instance_id = ctx.world_mut().create_dynamic_instance(TEMPLATE_ID);
                let template = ctx.world().instance(instance_id).map_or(-1, |i| i.template_id);
                ctx.world_mut()
                    .add_instance_world(l2_world::instance::InstanceWorld::new(instance_id, template));
                let end_time = ctx.world().instance_clock_ms() + 5_400_000;
                ctx.state::<State>().worlds.insert(
                    instance_id,
                    CcWorld {
                        end_time,
                        ..CcWorld::default()
                    },
                );
                on_enter_instance(ctx, player, instance_id, true);

                ctx.world_mut().handle_reenter_time_on_enter(instance_id);

                if ctx
                    .world()
                    .instance(instance_id)
                    .is_some_and(|i| i.is_remove_buff_enabled())
                {
                    ctx.world_mut().instance_remove_buffs(instance_id);
                }
            }

            return None;
        }

        if let Some(id) = cc_world(ctx, npc.object_id) {
            if npc_id == CRYSTALLINE_GOLEM {
                return None;
            } else if (32275..=32277).contains(&npc_id)
                && ctx.state::<State>().worlds[&id].oracle_triggered[(npc_id - 32275) as usize]
            {
                let party = ctx.world().party_of(player);
                let mut do_teleport = true;
                let mut loc: Option<(i32, i32, i32)> = None;
                match npc_id {
                    32275 => {
                        if status_of(ctx, id) == 22 {
                            run_steam_rooms(ctx, id, &STEAM2_SPAWNS, 23);
                        }

                        loc = Some((147529, 152587, -12169));
                        ctx.cancel_quest_timers("Timer2");
                        ctx.cancel_quest_timers("Timer21");
                        steam_timer(ctx, id, npc, player, party, 2, "Timer3", 600000);
                        ctx.start_quest_timer("Timer31", 600000, Some(npc), None, false);
                    }
                    32276 => {
                        if status_of(ctx, id) == 23 {
                            run_steam_rooms(ctx, id, &STEAM3_SPAWNS, 24);
                        }

                        loc = Some((150194, 152610, -12169));
                        ctx.cancel_quest_timers("Timer3");
                        ctx.cancel_quest_timers("Timer31");
                        steam_timer(ctx, id, npc, player, party, 4, "Timer4", 1200000);
                        ctx.start_quest_timer("Timer41", 1200000, Some(npc), None, false);
                    }
                    32277 => {
                        if status_of(ctx, id) == 24 {
                            run_steam_rooms(ctx, id, &STEAM4_SPAWNS, 25);
                        }

                        loc = Some((149743, 149986, -12141));
                        ctx.cancel_quest_timers("Timer4");
                        ctx.cancel_quest_timers("Timer41");
                        steam_timer(ctx, id, npc, player, party, 3, "Timer5", 900000);
                        ctx.start_quest_timer("Timer51", 900000, Some(npc), None, false);
                    }
                    _ => {
                        // something is wrong
                        do_teleport = false;
                    }
                }

                if let (true, Some((x, y, z))) = (do_teleport, loc) {
                    if !check_oracle_conditions(ctx, player) {
                        return None;
                    } else if let Some(party) = party {
                        let instance = ctx.world().instance_id_of(npc.object_id);
                        for member in ctx.world().party_members(party) {
                            ctx.take_items(member, RED_CORAL, 1);
                            ctx.teleport_player(member, x, y, z, instance, true);
                        }
                    } else {
                        let instance = ctx.world().instance_id_of(npc.object_id);
                        ctx.teleport_player(player, x, y, z, instance, true);
                    }
                }
            } else if npc_id == ORACLE_GUIDE_3 {
                if status_of(ctx, id) < 30 && check_baylor_conditions(ctx, player) {
                    let raiders: Vec<ObjectId> = match ctx.world().party_of(player) {
                        None => vec![player],
                        // Crystals are no longer beign cunsumed while entering to Baylor Lair.
                        Some(party) => ctx.world().party_members(party),
                    };
                    if let Some(w) = ctx.state::<State>().worlds.get_mut(&id) {
                        w.raiders = raiders;
                    }
                } else {
                    return None;
                }

                set_status(ctx, id, 30);
                let end_time = ctx.state::<State>().worlds[&id].end_time;
                let time = end_time - ctx.world().instance_clock_ms();
                ctx.world_mut().set_instance_duration(id, time as i32);

                let radius = 150.0_f64;
                let raiders = ctx.state::<State>().worlds[&id].raiders.clone();
                let members = raiders.len() as f64;
                for (i, p) in raiders.into_iter().enumerate() {
                    let angle = (i as f64 * 2.0 * std::f64::consts::PI) / members;
                    let x = (radius * angle.cos()) as i32;
                    let y = (radius * angle.sin()) as i32;
                    ctx.world_mut().tele_to_location(
                        p,
                        TeleportTarget::at(153571 + x, 142075 + y, -12737).with_instance(id),
                    );
                    ctx.world_mut().set_instance_id(p, id);
                    if let Some(pet) = ctx.world().summon_of(p) {
                        ctx.world_mut().tele_to_location(
                            pet,
                            TeleportTarget::at(153571 + x, 142075 + y, -12737).with_instance(id),
                        );
                        ctx.world_mut().set_instance_id(pet, id);
                    }

                    ctx.world_mut().set_creature_paralyzed(p, true);
                }

                ctx.start_quest_timer("Baylor", 30000, Some(npc), None, false);
            } else if npc_id == ORACLE_GUIDE_4 && status_of(ctx, id) == 31 {
                let instance = ctx.world().instance_id_of(npc.object_id);
                ctx.teleport_player(player, 153522, 144212, -9747, instance, true);
            }
        }

        None
    }

    fn on_trap_action(
        &self,
        ctx: &mut ScriptCtx<'_>,
        trap: ScriptNpc,
        _trigger: ObjectId,
        action: TrapAction,
    ) {
        if let Some(id) = cc_world(ctx, trap.object_id) {
            if action == TrapAction::TrapDisarmed && trap.id == DOOR_OPENING_TRAP[0] {
                ctx.world_mut().instance_open_door(id, 24220001);
                run_emerald_rooms(ctx, id, &ROOM1_SPAWNS, 1);
            }
        }
    }

    fn on_enter_zone(&self, ctx: &mut ScriptCtx<'_>, creature: ObjectId, zone_id: i32) {
        if ctx.world().player(creature).is_none() {
            return;
        }
        let Some(id) = cc_world(ctx, creature) else {
            return;
        };
        if status_of(ctx, id) != 8 {
            return;
        }

        let (spawns, room): (&[[i32; 5]], i32) = match zone_id {
            20105 => (&ROOM2_SPAWNS, 2),
            20106 => (&ROOM3_SPAWNS, 3),
            20107 => (&ROOM4_SPAWNS, 4),
            _ => return,
        };

        let door = room + 24220000;
        if ctx.world().instance_door(id, door).is_some() {
            if ctx.world().is_instance_door_open(id, door) {
                return;
            }

            if !ctx.has_quest_items(creature, SECRET_KEY) {
                return;
            }

            let index = (zone_id - 20104) as usize;
            let rooms_status = ctx.state::<State>().worlds[&id].rooms_status.get(index).copied();
            if rooms_status == Some(0) {
                run_emerald_rooms(ctx, id, spawns, room);
            }

            ctx.world_mut().instance_open_door(id, door);
            ctx.take_items(creature, SECRET_KEY, 1);
            if let Some(w) = ctx.state::<State>().worlds.get_mut(&id) {
                w.opened_doors.insert(door, creature);
            }
        }
    }

    fn on_exit_zone(&self, ctx: &mut ScriptCtx<'_>, creature: ObjectId, zone_id: i32) {
        if ctx.world().player(creature).is_none() {
            return;
        }
        let Some(id) = cc_world(ctx, creature) else {
            return;
        };
        if status_of(ctx, id) != 8 {
            return;
        }

        let door_id = match zone_id {
            20105 => 24220002,
            20106 => 24220003,
            20107 => 24220004,
            _ => return,
        };

        if ctx.world().instance_door(id, door_id).is_some() {
            let opened_by = ctx.state::<State>().worlds[&id]
                .opened_doors
                .get(&door_id)
                .copied();
            if ctx.world().is_instance_door_open(id, door_id) && opened_by == Some(creature) {
                ctx.world_mut().instance_close_door(id, door_id);
                if let Some(w) = ctx.state::<State>().worlds.get_mut(&id) {
                    w.opened_doors.remove(&door_id);
                }
            }
        }
    }
}

// ------------------------------------------------------------------ helpers

/// `InstanceManager.getInstance().getWorld(object) instanceof CCWorld`: the instance id.
fn cc_world(ctx: &mut ScriptCtx<'_>, object: ObjectId) -> Option<i32> {
    let mut id = ctx.world().instance_id_of(object);
    if id <= 0 {
        id = ctx
            .state::<State>()
            .npc_instance
            .get(&object)
            .copied()
            .unwrap_or(0);
    }
    (ctx.state::<State>().worlds.contains_key(&id) && ctx.world().instance_world(id).is_some()).then_some(id)
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

/// `addSpawn(npcId, x, y, z, heading, false, 0, false, instanceId)`.
fn spawn(
    ctx: &mut ScriptCtx<'_>,
    npc_id: i32,
    x: i32,
    y: i32,
    z: i32,
    heading: i32,
    instance_id: i32,
) -> Option<ObjectId> {
    let npc = ctx
        .add_spawn(npc_id, x, y, z, heading, false, 0, instance_id)
        .map(|n| n.object_id)?;
    ctx.state::<State>().npc_instance.insert(npc, instance_id);
    Some(npc)
}

/// The golem's record of the world.
fn golem_of<'a>(ctx: &'a mut ScriptCtx<'_>, instance_id: i32, npc: ObjectId) -> Option<&'a mut CrystalGolem> {
    ctx.state::<State>()
        .worlds
        .get_mut(&instance_id)?
        .crystal_golems
        .get_mut(&npc)
}

/// `npc.broadcastPacket(new CreatureSay(npc, chatType, npcString))`.
fn creature_say(ctx: &mut ScriptCtx<'_>, npc: ObjectId, chat: ChatType, text: NpcStringId) {
    let packet = CreatureSay {
        sender_object_id: npc.0,
        chat_type: chat,
        sender: SenderField::Name(ctx.world().npc_template_name(npc)),
        message_id: text.id(),
        body: SayBody::Parameters(Vec::new()),
    };
    ctx.world().broadcast_packet(npc, &packet);
}

/// `npc.broadcastSocialAction(id)`.
fn social_action(ctx: &ScriptCtx<'_>, npc: ScriptNpc, action_id: i32) {
    ctx.world().broadcast_packet(
        npc.object_id,
        &SocialAction {
            object_id: npc.object_id.0,
            action_id,
        },
    );
}

/// `npc.broadcastPacket(new SpecialCamera(npc, force, angle1, angle2, time, range, duration, relYaw,
/// relPitch, isWide, relAngle))`, the numbers in that order.
///
/// NOTE(java-quirk): the 11-argument constructor passes `duration` and `range` on in swapped
/// places, so the packet's duration is the `range` argument and the `duration` argument is lost.
fn special_camera(ctx: &ScriptCtx<'_>, npc: ScriptNpc, a: [i32; 10]) {
    ctx.world().broadcast_packet(
        npc.object_id,
        &SpecialCamera::new(
            npc.object_id.0,
            a[0],
            a[1],
            a[2],
            a[3],
            a[4],
            a[6],
            a[7],
            a[8],
            a[9],
            0,
        ),
    );
}

/// `decayMe()` of the Oracles of the first spawn.
fn decay_oracles(ctx: &mut ScriptCtx<'_>, instance_id: i32) {
    let oracles = ctx
        .state::<State>()
        .worlds
        .get(&instance_id)
        .map(|w| w.oracle.clone())
        .unwrap_or_default();
    for oracle in oracles {
        ctx.world_mut().decay(oracle);
    }
}

/// `stopAttack(player)`.
fn stop_attack(ctx: &mut ScriptCtx<'_>, player: ObjectId) {
    ctx.world_mut().stop_creature_attack(player);
    if let Some(pet) = ctx.world().summon_of(player) {
        ctx.world_mut().stop_creature_attack(pet);
    }
}

/// The skill level and timer of the Oracle teleports: every member of the party in the instance
/// (or the player) loses the skill 5239 and gets it at `level`, and a timer for each.
#[allow(clippy::too_many_arguments)]
fn steam_timer(
    ctx: &mut ScriptCtx<'_>,
    instance_id: i32,
    npc: ScriptNpc,
    player: ObjectId,
    party: Option<l2_world::PartyId>,
    level: i32,
    timer: &str,
    ms: i64,
) {
    let members = match party {
        Some(party) => ctx.world().party_members(party),
        None => vec![player],
    };
    for member in members {
        if party.is_none() || ctx.world().instance_id_of(member) == instance_id {
            ctx.world_mut()
                .stop_skill_effects(member, SkillFinishType::Removed, 5239);
            ctx.world_mut()
                .apply_effects(member, member, SkillKey::new(5239, level));
            ctx.start_quest_timer(timer, ms, Some(npc), Some(member), false);
        }
    }
}

/// `party.broadcastPacket(sm)` of a refusal.
fn refuse(ctx: &ScriptCtx<'_>, party: Option<l2_world::PartyId>, to: ObjectId, sm: &SystemMessage) {
    match party {
        Some(party) => {
            for member in ctx.world().party_members(party) {
                ctx.world().send(member, sm);
            }
        }
        None => {
            ctx.world().send(to, sm);
        }
    }
}

/// The party checks the three `check...Conditions` share: in a party, leader, and each member
/// passes `member_ok` (a refusal message id) and stands within 1000.
fn check_party(
    ctx: &ScriptCtx<'_>,
    player: ObjectId,
    member_ok: impl Fn(&ScriptCtx<'_>, ObjectId) -> Option<SystemMessageId>,
    min_level: Option<i32>,
) -> bool {
    if ctx.is_gm(player) {
        return true;
    }

    let Some(party) = ctx.world().party_of(player) else {
        ctx.world().send(
            player,
            &SystemMessage::new(SystemMessageId::YouAreNotCurrentlyInAPartySoYouCannotEnter),
        );
        return false;
    };

    if ctx.world().party_leader(party) != Some(player) {
        ctx.world().send(
            player,
            &SystemMessage::new(SystemMessageId::OnlyAPartyLeaderCanMakeTheRequestToEnter),
        );
        return false;
    }

    for member in ctx.world().party_members(party) {
        let name = ctx.player_name(member);
        let message =
            |id: SystemMessageId| SystemMessage::new(id).with(SystemMessageParam::PlayerName(name.clone()));
        if let Some(min_level) = min_level {
            if ctx.player_level(member) < min_level {
                refuse(
                    ctx,
                    Some(party),
                    player,
                    &message(SystemMessageId::C1SLevelDoesNotCorrespondToTheRequirementsForEntry),
                );
                return false;
            }
        }

        if let Some(refusal) = member_ok(ctx, member) {
            refuse(ctx, Some(party), player, &message(refusal));
            return false;
        }

        if !ctx.check_if_in_range(1000, player, member, true) {
            refuse(
                ctx,
                Some(party),
                player,
                &message(SystemMessageId::C1IsInALocationWhichCannotBeEnteredThereforeItCannotBeProcessed),
            );
            return false;
        }

        if min_level.is_some() {
            let reenter = ctx.world().instance_time(member, TEMPLATE_ID);
            if ctx.world().instance_clock_ms() < reenter {
                refuse(
                    ctx,
                    Some(party),
                    player,
                    &message(SystemMessageId::C1MayNotReEnterYet),
                );
                return false;
            }
        }
    }

    true
}

/// `checkConditions(player)`.
fn check_conditions(ctx: &ScriptCtx<'_>, player: ObjectId) -> bool {
    check_party(
        ctx,
        player,
        |ctx, member| {
            (!ctx.has_quest_items(member, CONTAMINATED_CRYSTAL))
                .then_some(SystemMessageId::C1SItemRequirementIsNotSufficientAndCannotBeEntered)
        },
        Some(MIN_LEVEL),
    )
}

/// `checkOracleConditions(player)`.
fn check_oracle_conditions(ctx: &ScriptCtx<'_>, player: ObjectId) -> bool {
    check_party(
        ctx,
        player,
        |ctx, member| {
            (!ctx.has_quest_items(member, RED_CORAL))
                .then_some(SystemMessageId::C1SItemRequirementIsNotSufficientAndCannotBeEntered)
        },
        None,
    )
}

/// `checkBaylorConditions(player)`.
fn check_baylor_conditions(ctx: &ScriptCtx<'_>, player: ObjectId) -> bool {
    check_party(
        ctx,
        player,
        |ctx, member| {
            (!ctx.has_quest_items(member, BLUE_CRYSTAL)
                || !ctx.has_quest_items(member, RED_CRYSTAL)
                || !ctx.has_quest_items(member, CLEAR_CRYSTAL))
            .then_some(SystemMessageId::C1SItemRequirementIsNotSufficientAndCannotBeEntered)
        },
        None,
    )
}

/// `onEnterInstance(player, world, firstEntrance)`.
fn on_enter_instance(ctx: &mut ScriptCtx<'_>, player: ObjectId, instance_id: i32, first_entrance: bool) {
    if first_entrance {
        let party = ctx.world().party_of(player);
        let members = match party {
            // This can happen only if GM enters solo.
            None => vec![player],
            Some(party) => ctx.world().party_members(party),
        };
        for member in members {
            ctx.world().send_message(member, "Welcome to Crystal Caverns.");
            ctx.teleport_player(member, START_LOC.0, START_LOC.1, START_LOC.2, instance_id, true);
            if let Some(world) = ctx.world_mut().instance_world_mut(instance_id) {
                world.add_allowed(member);
            }
        }

        run_oracle(ctx, instance_id);
    } else {
        ctx.teleport_player(player, START_LOC.0, START_LOC.1, START_LOC.2, instance_id, true);
    }
}

fn run_oracle(ctx: &mut ScriptCtx<'_>, id: i32) {
    set_status(ctx, id, 0);

    let oracle = spawn(ctx, ORACLE_GUIDE_1, 143172, 148894, -11975, 0, id);
    if let Some(w) = ctx.state::<State>().worlds.get_mut(&id) {
        w.oracle.extend(oracle);
    }
}

fn run_emerald(ctx: &mut ScriptCtx<'_>, id: i32) {
    set_status(ctx, id, 1);
    run_first(ctx, id);
    ctx.world_mut().instance_open_door(id, DOOR1);
}

fn run_coral(ctx: &mut ScriptCtx<'_>, id: i32) {
    set_status(ctx, id, 1);
    run_hall(ctx, id);
    ctx.world_mut().instance_open_door(id, DOOR2);
    ctx.world_mut().instance_open_door(id, DOOR5);
}

fn run_hall(ctx: &mut ScriptCtx<'_>, id: i32) {
    set_status(ctx, id, 2);

    for spawn_point in HALL_SPAWNS {
        let mob_id = ctx.get_random_entry(&CGMOBS).unwrap_or(CGMOBS[0]);
        if let Some(mob) = spawn(
            ctx,
            mob_id,
            spawn_point[0],
            spawn_point[1],
            spawn_point[2],
            spawn_point[3],
            id,
        ) {
            if let Some(w) = ctx.state::<State>().worlds.get_mut(&id) {
                w.npc_list1.insert(mob, false);
            }
        }
    }
}

fn run_first(ctx: &mut ScriptCtx<'_>, id: i32) {
    set_status(ctx, id, 2);

    let lohan = spawn(ctx, GATEKEEPER_LOHAN, 148206, 149486, -12140, 32308, id);
    let provo = spawn(ctx, GATEKEEPER_PROVO, 148203, 151093, -12140, 31100, id);
    if let Some(w) = ctx.state::<State>().worlds.get_mut(&id) {
        w.key_keepers.extend(lohan);
        w.key_keepers.extend(provo);
    }
    for spawn_point in FIRST_SPAWNS {
        spawn(
            ctx,
            spawn_point[0],
            spawn_point[1],
            spawn_point[2],
            spawn_point[3],
            spawn_point[4],
            id,
        );
    }
}

/// Spawns a list `[npcId, x, y, z, heading]` and gives the mobs, all alive.
fn spawn_list(ctx: &mut ScriptCtx<'_>, id: i32, spawn_list: &[[i32; 5]]) -> HashMap<ObjectId, bool> {
    let mut spawned = HashMap::new();
    for point in spawn_list {
        if let Some(mob) = spawn(ctx, point[0], point[1], point[2], point[3], point[4], id) {
            spawned.insert(mob, false);
        }
    }
    spawned
}

fn run_emerald_square(ctx: &mut ScriptCtx<'_>, id: i32) {
    set_status(ctx, id, 3);

    let spawned = spawn_list(ctx, id, &EMERALD_SPAWNS);
    if let Some(w) = ctx.state::<State>().worlds.get_mut(&id) {
        w.npc_list2.insert(0, spawned);
    }
}

fn run_emerald_rooms(ctx: &mut ScriptCtx<'_>, id: i32, spawn_points: &[[i32; 5]], room: i32) {
    let spawned = spawn_list(ctx, id, spawn_points);

    if room == 1 {
        spawn(ctx, 32359, 142110, 139896, -11888, 8033, id);
    }

    if let Some(w) = ctx.state::<State>().worlds.get_mut(&id) {
        w.npc_list2.insert(room, spawned);
        w.rooms_status[(room - 1) as usize] = 1;
    }
}

fn run_darnel(ctx: &mut ScriptCtx<'_>, id: i32) {
    set_status(ctx, id, 9);

    spawn(ctx, DARNEL, 152759, 145949, -12588, 21592, id);

    // TODO: missing traps
    ctx.world_mut().instance_open_door(id, 24220005);
    ctx.world_mut().instance_open_door(id, 24220006);
}

fn run_steam_rooms(ctx: &mut ScriptCtx<'_>, id: i32, spawn_points: &[[i32; 5]], status: i32) {
    set_status(ctx, id, status);

    let spawned = spawn_list(ctx, id, spawn_points);
    if let Some(w) = ctx.state::<State>().worlds.get_mut(&id) {
        w.npc_list2.insert(0, spawned);
    }
}

fn run_steam_oracles(ctx: &mut ScriptCtx<'_>, id: i32, oracle_order: &[[i32; 5]]) {
    if let Some(w) = ctx.state::<State>().worlds.get_mut(&id) {
        w.oracles.clear();
    }
    for oracle in oracle_order {
        if let Some(npc) = spawn(ctx, oracle[0], oracle[1], oracle[2], oracle[3], oracle[4], id) {
            if let Some(w) = ctx.state::<State>().worlds.get_mut(&id) {
                w.oracles.insert(npc, None);
            }
        }
    }
}

/// `checkKillProgress(room, mob, world)`.
fn check_kill_progress(ctx: &mut ScriptCtx<'_>, id: i32, room: i32, mob: ObjectId) -> bool {
    // A room that is not there is a `NullPointerException` in Java.
    let Some(list) = ctx
        .state::<State>()
        .worlds
        .get_mut(&id)
        .and_then(|w| w.npc_list2.get_mut(&room))
    else {
        return false;
    };
    if let Some(dead) = list.get_mut(&mob) {
        *dead = true;
    }

    list.values().all(|d| *d)
}

fn give_rewards(ctx: &mut ScriptCtx<'_>, player: ObjectId, instance_id: i32, boss_cry: i32, is_baylor: bool) {
    let num = i64::from((ctx.world().death_drop_chance_multiplier() as i32).max(1));
    let party = ctx.world().party_of(player);
    if let Some(party) = party {
        for member in ctx.world().party_members(party) {
            if ctx.world().instance_id_of(member) == instance_id {
                reward_member(ctx, member, boss_cry, is_baylor, num);
            }
        }
    } else if ctx.world().instance_id_of(player) == instance_id {
        reward_member(ctx, player, boss_cry, is_baylor, num);
    }
}

fn reward_member(ctx: &mut ScriptCtx<'_>, member: ObjectId, boss_cry: i32, is_baylor: bool, num: i64) {
    if !is_baylor && ctx.has_quest_items(member, CONTAMINATED_CRYSTAL) {
        ctx.take_items(member, CONTAMINATED_CRYSTAL, 1);
        ctx.give_items(member, boss_cry, 1);
    }

    if ctx.get_random(10) < 5 {
        ctx.give_items(member, WHITE_SEED_OF_EVIL_SHARD, num);
    } else {
        ctx.give_items(member, BLACK_SEED_OF_EVIL_SHARD, num);
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use l2_util::ObjectId as Oid;
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::instances::testing::instance_data;
    use crate::testing::{ScriptTest, item};

    const NAME: &str = "CrystalCaverns";

    /// Every template the tables spawn, as monsters, and the Folk the script talks to.
    fn npcs() -> String {
        let mut ids = BTreeSet::new();
        for table in [
            FIRST_SPAWNS.as_slice(),
            EMERALD_SPAWNS.as_slice(),
            ROOM1_SPAWNS.as_slice(),
            ROOM2_SPAWNS.as_slice(),
            STEAM1_SPAWNS.as_slice(),
        ] {
            for row in table {
                ids.insert(row[0]);
            }
        }
        ids.extend(CGMOBS);
        ids.extend([
            GATEKEEPER_LOHAN,
            GATEKEEPER_PROVO,
            TOURMALINE,
            TEROD,
            DOLPH,
            TEARS,
            TEARS_COPY,
            BAYLOR,
            DARNEL,
            KECHI,
            29116,
            29117,
            29104,
            29120,
            ALARM,
        ]);
        let mut text = String::new();
        for id in ids {
            text += &format!(r#"<npc id="{id}" type="Monster" level="78" name="M{id}"/>"#);
        }
        for id in [
            ORACLE_GUIDE_1,
            ORACLE_GUIDE_3,
            ORACLE_GUIDE_4,
            CRYSTALLINE_GOLEM,
            32271,
            32274,
            32275,
            32280,
            32359,
        ] {
            text += &format!(r#"<npc id="{id}" type="Folk" level="70" name="F{id}"/>"#);
        }
        text
    }

    fn items() -> String {
        let mut text = String::new();
        for id in [9597, 9598, 9690, 9692, 9695, 9696, 9697, 9698, 9699] {
            text += &format!(
                r#"<item id="{id}" type="EtcItem" name="I{id}"><set name="is_stackable" val="true"/></item>"#
            );
        }
        text
    }

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(CrystalCaverns);
        let mut t = ScriptTest::new(registry, &npcs(), &items(), &[]);
        t.world.set_instance_data(instance_data(
            &[(10, "Crystal Caverns")],
            &[(10, r#"<exitPoint x="149413" y="173078" z="-5014" />"#)],
        ));
        let guide = t.spawn(ORACLE_GUIDE_1, 20);
        t.enter(1, 78, vec![item(1001, 9690, 1)]);
        t.enter(2, 78, vec![item(1002, 9690, 1)]);
        t.party(1, 2);
        (t, guide)
    }

    fn script_npc(t: &ScriptTest, npc: Oid) -> ScriptNpc {
        ScriptNpc {
            object_id: npc,
            id: t.world.npc(npc).unwrap().npc_id,
        }
    }

    fn enter(t: &mut ScriptTest, guide: Oid) -> i32 {
        let npc = script_npc(t, guide);
        t.world.run_as_script(NAME, |ctx| {
            CrystalCaverns.on_talk(ctx, npc, Oid(1));
        });
        t.ticks(60);
        t.world.instance_id_of(Oid(1))
    }

    fn status(t: &ScriptTest, id: i32) -> i32 {
        t.world.instance_world(id).unwrap().status()
    }

    fn event(t: &mut ScriptTest, name: &str, npc: Oid, player: Option<i32>) {
        let npc = script_npc(t, npc);
        t.world.run_as_script(NAME, |ctx| {
            CrystalCaverns.on_event(ctx, name, Some(npc), player.map(Oid));
        });
    }

    fn kill(t: &mut ScriptTest, npc: Oid) {
        let npc = script_npc(t, npc);
        t.world.run_as_script(NAME, |ctx| {
            CrystalCaverns.on_kill(ctx, npc, Oid(1), false);
        });
    }

    fn state<R>(t: &mut ScriptTest, f: impl FnOnce(&mut State) -> R) -> R {
        let mut result = None;
        t.world
            .run_as_script(NAME, |ctx| result = Some(f(ctx.state::<State>())));
        result.unwrap()
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

    fn first_of(t: &ScriptTest, id: i32, template: i32) -> Oid {
        t.world.instance_npc_by_template(id, template).unwrap()
    }

    #[test]
    fn a_party_with_the_contaminated_crystal_enters() {
        let (mut t, guide) = setup();
        // Player 2 has no crystal: nobody enters.
        t.world.run_as_script(NAME, |ctx| {
            ctx.take_items(Oid(2), CONTAMINATED_CRYSTAL, 1);
        });
        assert_eq!(enter(&mut t, guide), 0);
        assert_eq!(t.world.instances().filter(|i| i.id > 0).count(), 0);

        t.world.run_as_script(NAME, |ctx| {
            ctx.give_items(Oid(2), CONTAMINATED_CRYSTAL, 1);
        });
        let id = enter(&mut t, guide);
        assert_eq!(id, 300_000);
        assert_eq!(t.world.instance_id_of(Oid(2)), id);
        let world = t.world.instance_world(id).unwrap();
        assert_eq!(
            (world.template_id, world.allowed.clone()),
            (10, vec![Oid(1), Oid(2)])
        );
        assert_eq!(status(&t, id), 0);
        // The first Oracle waits at the start.
        assert_eq!(count_of(&t, id, ORACLE_GUIDE_1), 1);
        let oracle = first_of(&t, id, ORACLE_GUIDE_1);
        let npc = script_npc(&t, oracle);
        t.world.run_as_script(NAME, |ctx| {
            assert_eq!(
                CrystalCaverns.on_first_talk(ctx, npc, Oid(1)),
                Some("32281.htm".to_owned())
            );
        });
        // A second talk with the leader brings him back to the start of the same instance.
        assert_eq!(enter(&mut t, guide), id);
        assert_eq!(t.world.instances().filter(|i| i.id > 0).count(), 1);
    }

    #[test]
    fn the_emerald_steam_way_opens_stage_after_stage() {
        let (mut t, guide) = setup();
        let id = enter(&mut t, guide);
        let oracle = first_of(&t, id, ORACLE_GUIDE_1);
        event(&mut t, "EmeraldSteam", oracle, Some(1));
        // run_emerald: the first room, the two gate keepers and the mobs.
        assert_eq!(status(&t, id), 2);
        assert!(!t.world.is_object_spawned(oracle));
        assert_eq!(count_of(&t, id, GATEKEEPER_LOHAN), 1);
        assert_eq!(count_of(&t, id, GATEKEEPER_PROVO), 1);

        // Lohan: a key drops, the other keeper goes, the Emerald Square comes.
        let lohan = first_of(&t, id, GATEKEEPER_LOHAN);
        let provo = first_of(&t, id, GATEKEEPER_PROVO);
        kill(&mut t, lohan);
        assert_eq!(status(&t, id), 3);
        assert!(!t.world.is_object_spawned(provo));
        assert_eq!(t.world.ground_item_ids().len(), 1);
        let square: Vec<Oid> = state(&mut t, |s| {
            s.worlds[&300_000].npc_list2[&0].keys().copied().collect()
        });
        assert_eq!(square.len(), EMERALD_SPAWNS.len());
        for mob in &square[..square.len() - 1] {
            kill(&mut t, *mob);
            assert_eq!(status(&t, id), 3);
        }
        kill(&mut t, square[square.len() - 1]);
        assert_eq!(status(&t, id), 4);

        // The guardians in turn: Tourmaline, Terod, Tourmaline, Dolph.
        for (template, next, spawned) in [
            (TOURMALINE, 5, TEROD),
            (TEROD, 6, TOURMALINE),
            (TOURMALINE, 7, DOLPH),
            (DOLPH, 8, 0),
        ] {
            let boss = t
                .world
                .instance(id)
                .unwrap()
                .npcs
                .keys()
                .copied()
                .filter(|n| t.in_world(*n) && t.world.npc(*n).is_some_and(|n| n.npc_id == template))
                .next_back()
                .unwrap();
            kill(&mut t, boss);
            assert_eq!(status(&t, id), next);
            if spawned != 0 {
                assert!(count_of(&t, id, spawned) >= 1);
            }
        }
    }

    #[test]
    fn the_coral_garden_ends_with_tears_and_the_clear_crystal() {
        let (mut t, guide) = setup();
        let id = enter(&mut t, guide);
        let oracle = first_of(&t, id, ORACLE_GUIDE_1);
        event(&mut t, "CoralGarden", oracle, Some(1));
        assert_eq!(status(&t, id), 2);
        let hall: Vec<Oid> = state(&mut t, |s| s.worlds[&300_000].npc_list1.keys().copied().collect());
        assert_eq!(hall.len(), HALL_SPAWNS.len());
        for mob in &hall[..hall.len() - 1] {
            kill(&mut t, *mob);
        }
        assert_eq!(status(&t, id), 2);
        kill(&mut t, hall[hall.len() - 1]);
        // Tears and the two Crystalline Golems.
        assert_eq!(status(&t, id), 3);
        assert_eq!(count_of(&t, id, TEARS), 1);
        assert_eq!(count_of(&t, id, CRYSTALLINE_GOLEM), 2);
        // They look for crystal fragments every two seconds and find none.
        t.wait_ms(2_100);
        assert!(t.world.quest_timer_count() >= 2);

        // Tears attacked out of turn: the attacker is thrown out of the dungeon.
        let tears = first_of(&t, id, TEARS);
        let tears_npc = script_npc(&t, tears);
        t.world.run_as_script(NAME, |ctx| {
            CrystalCaverns.on_attack(ctx, tears_npc, Oid(2), 10, false, None);
        });
        assert!(!t.world.instance_world(id).unwrap().is_allowed(Oid(2)));
        assert!(t.world.instance_world(id).unwrap().is_allowed(Oid(1)));

        // The golems did their work (status 4): Tears dies, the party gets the clear crystal.
        t.world.instance_world_mut(id).unwrap().set_status(4);
        kill(&mut t, tears);
        assert_eq!(count_of(&t, id, 32280), 1);
        assert_eq!(
            t.world.instance(id).unwrap().end_time_ms - t.world.instance_clock_ms() < 301_000,
            true
        );
        // Player 1 is in the instance: the contaminated crystal turned into the clear one.
        assert_eq!(t.count(1, CONTAMINATED_CRYSTAL), 0);
        assert_eq!(t.count(1, CLEAR_CRYSTAL), 1);
        assert_eq!(
            t.count(1, WHITE_SEED_OF_EVIL_SHARD) + t.count(1, BLACK_SEED_OF_EVIL_SHARD),
            1
        );
    }

    #[test]
    fn baylor_dies_and_the_chest_and_the_oracle_come() {
        let (mut t, guide) = setup();
        let id = enter(&mut t, guide);
        let baylor = t.spawn(BAYLOR, 40);
        t.world.instance_world_mut(id).unwrap().set_status(30);
        t.world.set_instance_id(baylor, id);
        let npc = script_npc(&t, baylor);
        t.world.run_as_script(NAME, |ctx| {
            ctx.state::<State>().npc_instance.insert(baylor, id);
            CrystalCaverns.on_kill(ctx, npc, Oid(1), false);
        });
        assert_eq!(status(&t, id), 31);
        // The shards come without the crystal exchange.
        assert_eq!(t.count(1, CONTAMINATED_CRYSTAL), 1);
        assert_eq!(
            t.count(1, WHITE_SEED_OF_EVIL_SHARD) + t.count(1, BLACK_SEED_OF_EVIL_SHARD),
            1
        );
        // A second later: the Oracle of the lair and a chest of Baylor.
        t.wait_ms(1_100);
        assert_eq!(count_of(&t, id, ORACLE_GUIDE_4), 1);
        assert_eq!(count_of(&t, id, 29116) + count_of(&t, id, 29117), 1);
    }

    #[test]
    fn the_first_talk_of_the_oracles_and_the_bird_in_a_cage() {
        let (mut t, guide) = setup();
        let id = enter(&mut t, guide);
        let oracle = first_of(&t, id, ORACLE_GUIDE_1);
        event(&mut t, "EmeraldSteam", oracle, Some(1));
        // After the first stage the guide shows his page (`showChatWindow`, no text returned).
        let oracle_npc = script_npc(&t, oracle);
        t.world.run_as_script(NAME, |ctx| {
            assert_eq!(CrystalCaverns.on_first_talk(ctx, oracle_npc, Oid(1)), None);
        });
        // The Oracle of the lair: page by the state of Q00131_BirdInACage (none: 32279.htm).
        let lair = ScriptNpc {
            object_id: guide,
            id: ORACLE_GUIDE_4,
        };
        t.world.run_as_script(NAME, |ctx| {
            assert_eq!(
                CrystalCaverns.on_first_talk(ctx, lair, Oid(1)),
                Some("32279.htm".to_owned())
            );
        });
    }

    #[test]
    fn baylor_throws_the_raiders_and_holds_them_before_the_fight() {
        let (mut t, _) = setup();
        let baylor = t.spawn(BAYLOR, 40);
        let before = t.world.position_of(Oid(1)).unwrap();
        t.world.set_creature_paralyzed(Oid(1), true);
        assert!(t.world.is_paralyzed(Oid(1)));
        t.world.set_creature_paralyzed(Oid(1), false);
        assert!(!t.world.is_paralyzed(Oid(1)));
        // `Throw`: along the line through the thrower, at least 5 units, never onto the same spot.
        t.world.throw_creature(baylor, Oid(1));
        let after = t.world.position_of(Oid(1)).unwrap();
        assert_ne!((after.0, after.1), (before.0, before.1));
    }
}
