//! Antharas, the Earth Dragon: the nest entry through the Heart of Warding, the waiting time,
//! the wake-up movie, the minion waves, the fear phase with its bombers, the skill rotation and the
//! respawn cycle on the `GrandBossManager` state.
// Java: dist/game/data/scripts/ai/bosses/Antharas/Antharas.java
//
// The Java fields (static and not) are `AntharasState` (`ctx.state`). The constructor is
// `on_load`. Statuses: 0 alive (idle in the nest), 1 waiting (players entered), 2 in fight, 3 dead.
//
// Side by side (D-54, `super` module): with `CycleDatabaseWrites` off the constructor restores the
// boss only from an alive row (a dead row still waits for its respawn time), and `CLEAR_STATUS`
// reads the row again and brings Antharas back only when the row says alive.
//
// Not modelled by the engine (the Java lines are ported, they do nothing yet): the skill of a hit
// in `on_attack` is always `None` (so the hate is always `damage * 1000`), `unload` (D-16: no
// reload).

use l2_protocol_game::NpcStringId;
use l2_protocol_game::server::{Earthquake, ExShowScreenMessage, PlaySound, SocialAction, SpecialCamera};
use l2_util::ObjectId;
use l2_world::mount::MountType;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup, SkillKey, TeleportTarget};
use tracing::{info, warn};

use super::{EVENT_ROW_READ, angle_from, date_time_string, distance_3d, row_lets_boss_run};

// NPC IDs.
const ANTHARAS_NPC_ID: i32 = 29068; // Antharas.
const ANTHARAS_BEHEMOTH_ID: i32 = 29069; // Behemoth Dragon.
const ANTHARAS_TERASQUE_ID: i32 = 29190; // Tarask Dragon
const ANTHARAS_BOMBER_ID: i32 = 29070; // Dragon Bomber.
const ANTHARAS_HEART_ID: i32 = 13001; // Heart of Warding.
const ANTHARAS_TELEPORT_CUBE_ID: i32 = 31859; // Teleportation Cubic.

/// Invisible NPC spawn locations (fear phase), in the order Java's `HashMap<Integer, Location>`
/// walks them.
// NOTE(java-quirk): `INVISIBLE_NPC_SPAWN_LOCATIONS.entrySet().forEach(..)` runs in the hash order
// of the map (32 buckets, `key & 31`): 29088..29094 first, then 29077..29087. The order decides
// which bombers take which random numbers.
const INVISIBLE_NPC_SPAWN_LOCATIONS: [(i32, (i32, i32, i32)); 18] = [
    (29088, (179208, 115452, -7735)), // antaras_clear_npc_12.
    (29089, (179191, 115079, -7735)), // antaras_clear_npc_13.
    (29090, (179221, 114546, -7735)), // antaras_clear_npc_14.
    (29091, (178916, 113925, -7735)), // antaras_clear_npc_15.
    (29092, (178782, 113814, -7735)), // antaras_clear_npc_16.
    (29093, (178419, 113417, -7735)), // antaras_clear_npc_17.
    (29094, (177855, 113282, -7735)), // antaras_clear_npc_18.
    (29077, (177229, 113298, -7735)), // antaras_clear_npc_1.
    (29078, (176707, 113585, -7735)), // antaras_clear_npc_2.
    (29079, (176385, 113889, -7735)), // antaras_clear_npc_3.
    (29080, (176082, 114241, -7735)), // antaras_clear_npc_4.
    (29081, (176066, 114802, -7735)), // antaras_clear_npc_5.
    (29082, (176095, 115313, -7735)), // antaras_clear_npc_6.
    (29083, (176425, 115829, -7735)), // antaras_clear_npc_7.
    (29084, (176949, 116378, -7735)), // antaras_clear_npc_8.
    (29085, (177655, 116402, -7735)), // antaras_clear_npc_9.
    (29086, (178248, 116395, -7735)), // antaras_clear_npc_10.
    (29087, (178706, 115998, -7735)), // antaras_clear_npc_11.
];

// Item IDs.
const ANTHARAS_STONE_ITEM_ID: i32 = 3865; // Portal Stone.

// Antharas skills.
const ANTHARAS_JUMP: SkillKey = SkillKey { id: 4106, level: 1 }; // Antharas Stun.
const ANTHARAS_TAIL_ATTACK: SkillKey = SkillKey { id: 4107, level: 1 }; // Antharas Stun (tail).
const ANTHARAS_FEAR: SkillKey = SkillKey { id: 4108, level: 1 }; // Antharas Terror.
const ANTHARAS_DEBUFF: SkillKey = SkillKey { id: 4109, level: 1 }; // Curse of Antharas.
const ANTHARAS_MOUTH_ATTACK: SkillKey = SkillKey { id: 4110, level: 2 }; // Breath Attack.
const ANTHARAS_BREATH_ATTACK: SkillKey = SkillKey { id: 4111, level: 1 }; // Antharas Fossilization.
const ANTHARAS_NORMAL_ATTACK: SkillKey = SkillKey { id: 4112, level: 1 }; // Ordinary Attack.
const ANTHARAS_NORMAL_ATTACK_EX: SkillKey = SkillKey { id: 4113, level: 1 }; // Ordinary Attack (extra).
const ANTHARAS_REGEN_PHASE_1: SkillKey = SkillKey { id: 4125, level: 1 }; // Antharas Regeneration.
const ANTHARAS_REGEN_PHASE_2: SkillKey = SkillKey { id: 4239, level: 1 }; // Antharas Regeneration.
const ANTHARAS_REGEN_PHASE_3: SkillKey = SkillKey { id: 4240, level: 1 }; // Antharas Regeneration.
const ANTHARAS_REGEN_PHASE_4: SkillKey = SkillKey { id: 4241, level: 1 }; // Antharas Regeneration.
const ANTHARAS_BOMBER_DISPEL: SkillKey = SkillKey { id: 5042, level: 1 }; // NPC Dispel Bomb.
const ANTHARAS_ANTI_STRIDER: SkillKey = SkillKey { id: 4258, level: 1 }; // Hinder Strider.
const ANTHARAS_FEAR_SHORT: SkillKey = SkillKey { id: 5092, level: 1 }; // Antharas Terror (short).
const ANTHARAS_METEOR: SkillKey = SkillKey { id: 5093, level: 1 }; // Antharas Meteor.

// Zone: `ZoneManager.getZoneById(70050, NoRestartZone.class)`, Antharas Nest zone.
const ANTHARAS_NEST_ZONE: (i32, &str) = (70050, "NoRestartZone");

// Grand boss status.
const STATUS_ALIVE: i32 = 0;
const STATUS_WAITING: i32 = 1;
const STATUS_IN_FIGHT: i32 = 2;
const STATUS_DEAD: i32 = 3;

// Event names.
const EVENT_ENTER: &str = "ENTER";
const EVENT_EXIT: &str = "EXIT";
const EVENT_SPAWN_ANTHARAS: &str = "SPAWN_ANTHARAS";
const EVENT_CAMERA_1: &str = "CAMERA_1";
const EVENT_CAMERA_2: &str = "CAMERA_2";
const EVENT_CAMERA_3: &str = "CAMERA_3";
const EVENT_CAMERA_4: &str = "CAMERA_4";
const EVENT_CAMERA_5: &str = "CAMERA_5";
const EVENT_SOCIAL: &str = "SOCIAL";
const EVENT_START_MOVE: &str = "START_MOVE";
const EVENT_SET_REGEN: &str = "SET_REGEN";
const EVENT_CHECK_ATTACK: &str = "CHECK_ATTACK";
const EVENT_SPAWN_MINION: &str = "SPAWN_MINION";
const EVENT_CLEAR_ZONE: &str = "CLEAR_ZONE";
const EVENT_TID_USED_FEAR: &str = "TID_USED_FEAR";
const EVENT_TID_FEAR_MOVE_TIMEOVER: &str = "TID_FEAR_MOVE_TIMEOVER";
const EVENT_TID_FEAR_COOLTIME: &str = "TID_FEAR_COOLTIME";
const EVENT_CLEAR_STATUS: &str = "CLEAR_STATUS";
const EVENT_SKIP_WAITING: &str = "SKIP_WAITING";
const EVENT_RESPAWN_ANTHARAS: &str = "RESPAWN_ANTHARAS";
const EVENT_DESPAWN_MINIONS: &str = "DESPAWN_MINIONS";
const EVENT_ABORT_FIGHT: &str = "ABORT_FIGHT";
const EVENT_MANAGE_SKILL: &str = "MANAGE_SKILL";

// Teleport and position constants.
const ANTHARAS_IDLE_LOCATION: (i32, i32, i32) = (185708, 114298, -8221);
const ANTHARAS_SPAWN_LOCATION: (i32, i32, i32, i32) = (181323, 114850, -7623, 32542);
const ANTHARAS_MOVE_TARGET_LOCATION: (i32, i32, i32) = (179011, 114871, -7704);
const ANTHARAS_FEAR_MOVE_X: i32 = 177648;
const ANTHARAS_FEAR_MOVE_Y: i32 = 114816;
const ANTHARAS_FEAR_MOVE_Z: i32 = -7735;
const TELEPORT_EXIT_BASE_X: i32 = 79800;
const TELEPORT_EXIT_BASE_Y: i32 = 151200;
const TELEPORT_EXIT_Z: i32 = -3534;

// Timing constants (milliseconds).
const CHECK_ATTACK_INTERVAL: i64 = 60000;
const MINION_SPAWN_INTERVAL: i64 = 300000;
const REGEN_INTERVAL: i64 = 60000;
const INACTIVITY_LIMIT: i64 = 900000;
const TELEPORT_CUBE_DURATION: i64 = 900000;
const FEAR_COOLDOWN: i64 = 300000;
const FEAR_MOVE_FIRST_CHECK: i64 = 2000;
const FEAR_MOVE_RETRY_INTERVAL: i64 = 5000;
const FEAR_MOVE_FORCE_DELAY: i64 = 1000;
const FEAR_USED_DELAY: i64 = 7000;
const MAX_MOVE_RETRY_COUNT: i32 = 3;

// Distance / range constants.
const MAX_THREAT_DISTANCE: f64 = 9000.0;
const BOMBER_EXPLOSION_RANGE: f64 = 230.0;

// Misc.
const MAX_PEOPLE: usize = 200; // Max allowed players.

/// The fields of the Java class.
#[derive(Debug, Default)]
pub(crate) struct AntharasState {
    /// `_antharasBoss`.
    pub(crate) antharas_boss: Option<ObjectId>,
    pub(crate) last_attack_time: i64,
    pub(crate) minion_count: i32,
    pub(crate) minion_multiplier: i32,
    pub(crate) move_retry_count: i32,
    pub(crate) sand_storm_state: i32,
    pub(crate) primary_attacker: Option<ObjectId>,
    pub(crate) secondary_attacker: Option<ObjectId>,
    pub(crate) tertiary_attacker: Option<ObjectId>,
    pub(crate) primary_attacker_hate: i32,
    pub(crate) secondary_attacker_hate: i32,
    pub(crate) tertiary_attacker_hate: i32,
}

/// Java `ai.bosses.Antharas.Antharas`.
pub struct Antharas;

impl Script for Antharas {
    fn name(&self) -> &'static str {
        "Antharas"
    }

    fn path(&self) -> &'static str {
        "ai/bosses/Antharas"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[ANTHARAS_HEART_ID, ANTHARAS_TELEPORT_CUBE_ID]);
        s.add_talk_id(&[ANTHARAS_HEART_ID, ANTHARAS_TELEPORT_CUBE_ID]);
        s.add_first_talk_id(&[ANTHARAS_HEART_ID]);
        let invisible: Vec<i32> = INVISIBLE_NPC_SPAWN_LOCATIONS.iter().map(|(id, _)| *id).collect();
        s.add_spawn_id(&invisible);
        s.add_spawn_id(&[ANTHARAS_NPC_ID]);
        s.add_move_finished_id(&[ANTHARAS_BOMBER_ID]);
        s.add_aggro_range_enter_id(&[ANTHARAS_BOMBER_ID]);
        s.add_spell_finished_id(&[ANTHARAS_NPC_ID]);
        s.add_attack_id(&[
            ANTHARAS_NPC_ID,
            ANTHARAS_BOMBER_ID,
            ANTHARAS_BEHEMOTH_ID,
            ANTHARAS_TERASQUE_ID,
        ]);
        s.add_kill_id(&[ANTHARAS_NPC_ID, ANTHARAS_BEHEMOTH_ID, ANTHARAS_TERASQUE_ID]);
    }

    /// The constructor's second half: restores the persisted state from `GrandBossManager`.
    fn on_load(&self, ctx: &mut ScriptCtx<'_>) {
        let Some(antharas_info) = ctx.world().grand_boss_info(ANTHARAS_NPC_ID).cloned() else {
            // Java: `getStatSet` answers null and the constructor throws.
            warn!("Antharas: no grandboss_data row for {ANTHARAS_NPC_ID}, the script does not start.");
            return;
        };
        let current_hp = antharas_info.current_hp;
        let current_mp = antharas_info.current_mp;
        let saved_x = antharas_info.loc_x;
        let saved_y = antharas_info.loc_y;
        let saved_z = antharas_info.loc_z;
        let saved_heading = antharas_info.heading;
        let respawn_time = antharas_info.respawn_time;

        let status = get_status(ctx);
        // D-54: a waiting or fighting row belongs to the other server's fight.
        if status != STATUS_DEAD && !row_lets_boss_run(ctx, ANTHARAS_NPC_ID) {
            info!("Antharas: the shared state row says {status}; the AI waits for an alive row (D-54).");
            return;
        }
        match status {
            STATUS_ALIVE => {
                reset_fight_state(ctx);
                let boss = spawn_idle(ctx);
                if let Some(boss) = boss {
                    ctx.world_mut()
                        .set_current_hp_mp(boss.object_id, current_hp, current_mp);
                    add_boss(ctx, boss);
                }
            }
            STATUS_WAITING => {
                reset_fight_state(ctx);
                let boss = spawn_idle(ctx);
                if let Some(boss) = boss {
                    ctx.world_mut()
                        .set_current_hp_mp(boss.object_id, current_hp, current_mp);
                    add_boss(ctx, boss);
                }
                let wait = wait_time_ms(ctx);
                ctx.start_quest_timer(EVENT_SPAWN_ANTHARAS, wait, None, None, false);
            }
            STATUS_IN_FIGHT => {
                let boss = ctx.add_spawn(
                    ANTHARAS_NPC_ID,
                    saved_x,
                    saved_y,
                    saved_z,
                    saved_heading,
                    false,
                    0,
                    0,
                );
                ctx.state::<AntharasState>().antharas_boss = boss.map(|b| b.object_id);
                if let Some(boss) = boss {
                    ctx.world_mut()
                        .set_current_hp_mp(boss.object_id, current_hp, current_mp);
                    add_boss(ctx, boss);
                }
                let now = ctx.now_ms();
                ctx.state::<AntharasState>().last_attack_time = now;
                ctx.start_quest_timer(EVENT_CHECK_ATTACK, CHECK_ATTACK_INTERVAL, boss, None, false);
                ctx.start_quest_timer(EVENT_SPAWN_MINION, MINION_SPAWN_INTERVAL, boss, None, false);
            }
            STATUS_DEAD => {
                let remaining_time = respawn_time - ctx.now_ms();
                if remaining_time > 0 {
                    ctx.start_quest_timer(EVENT_CLEAR_STATUS, remaining_time, None, None, false);
                } else if !ctx.world().boss_writes_enabled() {
                    // D-54: the respawn of a dead row waits for the other server's alive row.
                    ctx.notify_event(EVENT_CLEAR_STATUS, None, None);
                } else {
                    set_status(ctx, STATUS_ALIVE);
                    reset_fight_state(ctx);
                    if let Some(boss) = spawn_idle(ctx) {
                        add_boss(ctx, boss);
                    }
                }
            }
            _ => {}
        }
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        match event {
            EVENT_ENTER => {
                let mut html_text = None;
                let status = get_status(ctx);
                let Some(player) = player else {
                    return None;
                };
                if status == STATUS_DEAD {
                    html_text = Some("13001-01.html");
                } else if status == STATUS_IN_FIGHT {
                    html_text = Some("13001-02.html");
                } else if nest_players_inside(ctx) >= MAX_PEOPLE {
                    html_text = Some("13001-04.html");
                } else if let Some(party) = ctx.world().party_of(player) {
                    let channel = ctx.world().command_channel_of(player);
                    let is_in_cc = channel.is_some();
                    let members = match channel {
                        Some(channel) => ctx.world().command_channel_members(channel),
                        None => ctx.world().party_members_of(player),
                    };
                    let is_party_leader = match channel {
                        Some(channel) => ctx.world().is_command_channel_leader(channel, player),
                        None => ctx.world().party_leader(party) == Some(player),
                    };
                    let _ = is_in_cc;
                    if !is_party_leader {
                        html_text = Some("13001-05.html");
                    } else if !ctx.has_quest_items(player, ANTHARAS_STONE_ITEM_ID) {
                        html_text = Some("13001-03.html");
                    } else if members.len() > MAX_PEOPLE.saturating_sub(nest_players_inside(ctx)) {
                        html_text = Some("13001-04.html");
                    } else {
                        for member in members {
                            let near = npc.is_some_and(|npc| inside_radius_3d(ctx, member, npc.object_id, 1000.0));
                            if near {
                                let x = 179700 + ctx.get_random(700);
                                let y = 113800 + ctx.get_random(2100);
                                tele_player(ctx, member, x, y, -7709);
                            }
                        }

                        if get_status(ctx) != STATUS_WAITING {
                            set_status(ctx, STATUS_WAITING);
                            let wait = wait_time_ms(ctx);
                            ctx.start_quest_timer(EVENT_SPAWN_ANTHARAS, wait, None, None, false);
                        }
                    }
                } else if !ctx.has_quest_items(player, ANTHARAS_STONE_ITEM_ID) {
                    html_text = Some("13001-03.html");
                } else {
                    let x = 179700 + ctx.get_random(700);
                    let y = 113800 + ctx.get_random(2100);
                    tele_player(ctx, player, x, y, -7709);

                    if status != STATUS_WAITING {
                        set_status(ctx, STATUS_WAITING);
                        let wait = wait_time_ms(ctx);
                        ctx.start_quest_timer(EVENT_SPAWN_ANTHARAS, wait, None, None, false);
                    }
                }

                return html_text.map(str::to_owned);
            }
            EVENT_EXIT => {
                if let Some(player) = player {
                    tele_exit(ctx, player);
                }
            }
            EVENT_SPAWN_ANTHARAS => {
                let Some(boss) = ctx.state::<AntharasState>().antharas_boss else {
                    return None;
                };
                let Some(boss) = ctx.script_npc(boss) else {
                    return None;
                };
                ctx.world_mut().disable_core_ai(boss.object_id, true);
                ctx.world_mut().set_random_walking(boss.object_id, false);
                let (x, y, z, heading) = ANTHARAS_SPAWN_LOCATION;
                tele_npc(ctx, boss.object_id, (x, y, z), heading);
                set_status(ctx, STATUS_IN_FIGHT);
                let now = ctx.now_ms();
                ctx.state::<AntharasState>().last_attack_time = now;
                broadcast_nest(ctx, &PlaySound::file("BS02_A"));
                ctx.start_quest_timer(EVENT_CAMERA_1, 23, Some(boss), None, false);
            }
            EVENT_CAMERA_1 => {
                let Some(npc) = npc else {
                    return None;
                };
                broadcast_nest(ctx, &camera(npc, 700, 13, -19, 0, 20000, 0, 0, 0, 0, 0));
                ctx.start_quest_timer(EVENT_CAMERA_2, 3000, Some(npc), None, false);
            }
            EVENT_CAMERA_2 => {
                let Some(npc) = npc else {
                    return None;
                };
                broadcast_nest(ctx, &camera(npc, 700, 13, 0, 6000, 20000, 0, 0, 0, 0, 0));
                ctx.start_quest_timer(EVENT_CAMERA_3, 10000, Some(npc), None, false);
            }
            EVENT_CAMERA_3 => {
                let Some(npc) = npc else {
                    return None;
                };
                broadcast_nest(ctx, &camera(npc, 3700, 0, -3, 0, 10000, 0, 0, 0, 0, 0));
                broadcast_nest(ctx, &social(npc, 1));
                ctx.start_quest_timer(EVENT_CAMERA_4, 200, Some(npc), None, false);
                ctx.start_quest_timer(EVENT_SOCIAL, 5200, Some(npc), None, false);
            }
            EVENT_CAMERA_4 => {
                let Some(npc) = npc else {
                    return None;
                };
                broadcast_nest(ctx, &camera(npc, 1100, 0, -3, 22000, 30000, 0, 0, 0, 0, 0));
                ctx.start_quest_timer(EVENT_CAMERA_5, 10800, Some(npc), None, false);
            }
            EVENT_CAMERA_5 => {
                let Some(npc) = npc else {
                    return None;
                };
                broadcast_nest(ctx, &camera(npc, 1100, 0, -3, 300, 7000, 0, 0, 0, 0, 0));
                ctx.start_quest_timer(EVENT_START_MOVE, 1900, Some(npc), None, false);
            }
            EVENT_SOCIAL => {
                let Some(npc) = npc else {
                    return None;
                };
                broadcast_nest(ctx, &social(npc, 2));
            }
            EVENT_START_MOVE => {
                let Some(npc) = npc else {
                    return None;
                };
                ctx.world_mut().disable_core_ai(npc.object_id, false);
                ctx.world_mut().set_random_walking(npc.object_id, true);

                // `World.forFirstVisibleObjectInRange(npc, Player.class, 4000, isHero && config, ..)`.
                if ctx.world().boss_settings().grand_boss.antharas_recognize_hero {
                    let hero = ctx
                        .world()
                        .visible_objects_in_range(npc.object_id, 4000)
                        .into_iter()
                        .find(|id| ctx.world().player(*id).is_some() && ctx.world().is_hero(*id));
                    if let Some(hero) = hero {
                        let name = ctx.player_name(hero);
                        broadcast_nest(
                            ctx,
                            &ExShowScreenMessage::npc_string(
                                NpcStringId::S1YouCannotHopeToDefeatMeWithYourMeagerStrength,
                                2,
                                4000,
                                &[&name],
                            ),
                        );
                    }
                }

                let (x, y, z) = ANTHARAS_MOVE_TARGET_LOCATION;
                ctx.world_mut().npc_set_intention_move_to(npc.object_id, x, y, z);
                ctx.start_quest_timer(EVENT_CHECK_ATTACK, CHECK_ATTACK_INTERVAL, Some(npc), None, false);
                ctx.start_quest_timer(EVENT_SPAWN_MINION, MINION_SPAWN_INTERVAL, Some(npc), None, false);
            }
            EVENT_SET_REGEN => {
                let Some(npc) = npc else {
                    return None;
                };
                let (hp, max_hp) = hp_of(ctx, npc.object_id);
                let regen = if hp < max_hp * 0.25 {
                    ANTHARAS_REGEN_PHASE_4
                } else if hp < max_hp * 0.5 {
                    ANTHARAS_REGEN_PHASE_3
                } else if hp < max_hp * 0.75 {
                    ANTHARAS_REGEN_PHASE_2
                } else {
                    ANTHARAS_REGEN_PHASE_1
                };
                if !ctx.world().is_affected_by_skill(npc.object_id, regen.id) {
                    ctx.world_mut()
                        .set_intention_cast(npc.object_id, regen, Some(npc.object_id));
                }

                ctx.start_quest_timer(EVENT_SET_REGEN, REGEN_INTERVAL, Some(npc), None, false);
            }
            EVENT_CHECK_ATTACK => {
                let Some(npc) = npc else {
                    return None;
                };
                let last_attack_time = ctx.state::<AntharasState>().last_attack_time;
                if (last_attack_time + INACTIVITY_LIMIT) < ctx.now_ms() {
                    set_status(ctx, STATUS_ALIVE);
                    for creature in nest_characters_inside(ctx) {
                        if ctx.world().is_npc(creature) {
                            if ctx.script_npc(creature).is_some_and(|n| n.id == ANTHARAS_NPC_ID) {
                                tele_npc(ctx, creature, ANTHARAS_IDLE_LOCATION, 0);
                            } else {
                                ctx.world_mut().delete_npc(creature);
                            }
                        } else if ctx.world().player(creature).is_some() {
                            tele_exit(ctx, creature);
                        }
                    }

                    reset_fight_state(ctx);
                    ctx.cancel_quest_timer(EVENT_CHECK_ATTACK, Some(npc), None);
                    ctx.cancel_quest_timer(EVENT_SPAWN_MINION, Some(npc), None);
                    ctx.cancel_quest_timer(EVENT_SET_REGEN, Some(npc), None);
                    ctx.cancel_quest_timer(EVENT_TID_USED_FEAR, Some(npc), None);
                    ctx.cancel_quest_timer(EVENT_TID_FEAR_MOVE_TIMEOVER, Some(npc), None);
                    ctx.cancel_quest_timer(EVENT_TID_FEAR_COOLTIME, Some(npc), None);
                } else {
                    if ctx.state::<AntharasState>().primary_attacker_hate > 10 {
                        let r = ctx.get_random(10);
                        ctx.state::<AntharasState>().primary_attacker_hate -= r;
                    }

                    if ctx.state::<AntharasState>().secondary_attacker_hate > 10 {
                        let r = ctx.get_random(10);
                        ctx.state::<AntharasState>().secondary_attacker_hate -= r;
                    }

                    if ctx.state::<AntharasState>().tertiary_attacker_hate > 10 {
                        let r = ctx.get_random(10);
                        ctx.state::<AntharasState>().tertiary_attacker_hate -= r;
                    }

                    manage_skills(ctx, npc);
                    ctx.start_quest_timer(EVENT_CHECK_ATTACK, CHECK_ATTACK_INTERVAL, Some(npc), None, false);
                }
            }
            EVENT_SPAWN_MINION => {
                let Some(npc) = npc else {
                    return None;
                };
                let (minion_count, minion_multiplier) = {
                    let st = ctx.state::<AntharasState>();
                    (st.minion_count, st.minion_multiplier)
                };
                if (minion_multiplier > 1) && (minion_count < (100 - (minion_multiplier * 2))) {
                    for _ in 0..minion_multiplier {
                        ctx.add_spawn_at(ANTHARAS_BEHEMOTH_ID, npc.object_id, true, 0);
                        ctx.add_spawn_at(ANTHARAS_TERASQUE_ID, npc.object_id, true, 0);
                    }

                    ctx.state::<AntharasState>().minion_count += minion_multiplier * 2;
                } else if minion_count < 98 {
                    ctx.add_spawn_at(ANTHARAS_BEHEMOTH_ID, npc.object_id, true, 0);
                    ctx.add_spawn_at(ANTHARAS_TERASQUE_ID, npc.object_id, true, 0);
                    ctx.state::<AntharasState>().minion_count += 2;
                } else if minion_count < 99 {
                    let id = if ctx.get_random_boolean() {
                        ANTHARAS_BEHEMOTH_ID
                    } else {
                        ANTHARAS_TERASQUE_ID
                    };
                    ctx.add_spawn_at(id, npc.object_id, true, 0);
                    ctx.state::<AntharasState>().minion_count += 1;
                }

                if (ctx.get_random(100) > 10) && (ctx.state::<AntharasState>().minion_multiplier < 4) {
                    ctx.state::<AntharasState>().minion_multiplier += 1;
                }

                ctx.start_quest_timer(EVENT_SPAWN_MINION, MINION_SPAWN_INTERVAL, Some(npc), None, false);
            }
            EVENT_CLEAR_ZONE => {
                for creature in nest_characters_inside(ctx) {
                    if ctx.world().is_npc(creature) {
                        ctx.world_mut().delete_npc(creature);
                    } else if ctx.world().player(creature).is_some() {
                        tele_exit(ctx, creature);
                    }
                }
            }
            EVENT_TID_USED_FEAR => {
                if let Some(npc) = npc
                    && ctx.state::<AntharasState>().sand_storm_state == 0
                {
                    ctx.state::<AntharasState>().sand_storm_state = 1;
                    ctx.world_mut().disable_core_ai(npc.object_id, true);
                    ctx.world_mut().npc_set_intention_move_to(
                        npc.object_id,
                        ANTHARAS_FEAR_MOVE_X,
                        ANTHARAS_FEAR_MOVE_Y,
                        ANTHARAS_FEAR_MOVE_Z,
                    );
                    ctx.start_quest_timer(EVENT_TID_FEAR_MOVE_TIMEOVER, FEAR_MOVE_FIRST_CHECK, Some(npc), None, false);
                    ctx.start_quest_timer(EVENT_TID_FEAR_COOLTIME, FEAR_COOLDOWN, Some(npc), None, false);
                }
            }
            EVENT_TID_FEAR_COOLTIME => {
                ctx.state::<AntharasState>().sand_storm_state = 0;
            }
            EVENT_TID_FEAR_MOVE_TIMEOVER => {
                let Some(npc) = npc else {
                    return None;
                };
                let (x, y) = ctx
                    .world()
                    .position_of(npc.object_id)
                    .map_or((0, 0), |(x, y, _)| (x, y));
                let sand_storm_state = ctx.state::<AntharasState>().sand_storm_state;
                if (sand_storm_state == 1) && (x == ANTHARAS_FEAR_MOVE_X) && (y == ANTHARAS_FEAR_MOVE_Y) {
                    ctx.state::<AntharasState>().sand_storm_state = 2;
                    ctx.state::<AntharasState>().move_retry_count = 0;
                    ctx.world_mut().disable_core_ai(npc.object_id, false);
                    for (id, (x, y, z)) in INVISIBLE_NPC_SPAWN_LOCATIONS {
                        ctx.add_spawn(id, x, y, z, 0, false, 0, 0);
                    }
                } else if sand_storm_state == 1 {
                    if ctx.state::<AntharasState>().move_retry_count <= MAX_MOVE_RETRY_COUNT {
                        ctx.state::<AntharasState>().move_retry_count += 1;
                        ctx.world_mut().npc_set_intention_move_to(
                            npc.object_id,
                            ANTHARAS_FEAR_MOVE_X,
                            ANTHARAS_FEAR_MOVE_Y,
                            ANTHARAS_FEAR_MOVE_Z,
                        );
                        ctx.start_quest_timer(
                            EVENT_TID_FEAR_MOVE_TIMEOVER,
                            FEAR_MOVE_RETRY_INTERVAL,
                            Some(npc),
                            None,
                            false,
                        );
                    } else {
                        let heading = ctx.world().heading_of(npc.object_id);
                        ctx.world_mut().tele_to_location(
                            npc.object_id,
                            TeleportTarget::at(ANTHARAS_FEAR_MOVE_X, ANTHARAS_FEAR_MOVE_Y, ANTHARAS_FEAR_MOVE_Z)
                                .with_heading(heading),
                        );
                        ctx.start_quest_timer(
                            EVENT_TID_FEAR_MOVE_TIMEOVER,
                            FEAR_MOVE_FORCE_DELAY,
                            Some(npc),
                            None,
                            false,
                        );
                    }
                }
            }
            EVENT_CLEAR_STATUS => {
                // D-54: on the shared database the row decides; read it again first.
                if !ctx.world().boss_writes_enabled() {
                    let name = self.name();
                    ctx.world_mut()
                        .reread_grand_boss_for_script(ANTHARAS_NPC_ID, name, EVENT_ROW_READ);
                    return None;
                }
                clear_status(ctx);
            }
            EVENT_ROW_READ => {
                // D-54: the answer of the read `CLEAR_STATUS` asked for.
                match get_status(ctx) {
                    STATUS_ALIVE => clear_status(ctx),
                    STATUS_DEAD => {
                        let respawn = ctx
                            .world()
                            .grand_boss_info(ANTHARAS_NPC_ID)
                            .map_or(0, |i| i.respawn_time);
                        let remaining_time = respawn - ctx.now_ms();
                        if remaining_time > 0 {
                            ctx.start_quest_timer(EVENT_CLEAR_STATUS, remaining_time, None, None, false);
                        } else {
                            info!("Antharas: the shared state row says dead; Antharas stays away (D-54).");
                        }
                    }
                    status => {
                        info!("Antharas: the shared state row says {status}; Antharas stays away (D-54).");
                    }
                }
            }
            EVENT_SKIP_WAITING => {
                if get_status(ctx) == STATUS_WAITING {
                    ctx.cancel_quest_timer(EVENT_SPAWN_ANTHARAS, None, None);
                    ctx.notify_event(EVENT_SPAWN_ANTHARAS, None, None);
                    if let Some(player) = player {
                        ctx.world()
                            .send_message(player, &format!("{}: Skipping waiting time ...", self.name()));
                    }
                } else if let Some(player) = player {
                    ctx.world().send_message(
                        player,
                        &format!("{}: You can't skip waiting time right now!", self.name()),
                    );
                }
            }
            EVENT_RESPAWN_ANTHARAS => {
                if get_status(ctx) == STATUS_DEAD {
                    set_respawn(ctx, 0);
                    ctx.cancel_quest_timer(EVENT_CLEAR_STATUS, None, None);
                    ctx.notify_event(EVENT_CLEAR_STATUS, None, None);
                    if let Some(player) = player {
                        ctx.world()
                            .send_message(player, &format!("{}: Antharas has been respawned.", self.name()));
                    }
                } else if let Some(player) = player {
                    ctx.world().send_message(
                        player,
                        &format!(
                            "{}: You can't respawn antharas while antharas is alive!",
                            self.name()
                        ),
                    );
                }
            }
            EVENT_DESPAWN_MINIONS => {
                if get_status(ctx) == STATUS_IN_FIGHT {
                    ctx.state::<AntharasState>().minion_count = 0;
                    for creature in nest_characters_inside(ctx) {
                        let minion = ctx.script_npc(creature).is_some_and(|n| {
                            n.id == ANTHARAS_BEHEMOTH_ID || n.id == ANTHARAS_TERASQUE_ID || n.id == ANTHARAS_BOMBER_ID
                        });
                        if minion {
                            ctx.world_mut().delete_npc(creature);
                        }
                    }

                    if let Some(player) = player {
                        ctx.world()
                            .send_message(player, &format!("{}: All minions have been deleted!", self.name()));
                    }
                } else if let Some(player) = player {
                    ctx.world().send_message(
                        player,
                        &format!("{}: You can't despawn minions right now!", self.name()),
                    );
                }
            }
            EVENT_ABORT_FIGHT => {
                if get_status(ctx) == STATUS_IN_FIGHT {
                    set_status(ctx, STATUS_ALIVE);
                    let boss = ctx
                        .state::<AntharasState>()
                        .antharas_boss
                        .and_then(|b| ctx.script_npc(b));
                    for timer in [
                        EVENT_CHECK_ATTACK,
                        EVENT_SPAWN_MINION,
                        EVENT_SET_REGEN,
                        EVENT_TID_USED_FEAR,
                        EVENT_TID_FEAR_MOVE_TIMEOVER,
                        EVENT_TID_FEAR_COOLTIME,
                    ] {
                        // NOTE(java-quirk): with `_antharasBoss` null Java cancels the timers
                        // started without an NPC, so the boss's timers would keep running.
                        ctx.cancel_quest_timer(timer, boss, None);
                    }

                    for creature in nest_characters_inside(ctx) {
                        if ctx.world().is_npc(creature) {
                            if ctx.script_npc(creature).is_some_and(|n| n.id == ANTHARAS_NPC_ID) {
                                tele_npc(ctx, creature, ANTHARAS_IDLE_LOCATION, 0);
                            } else {
                                ctx.world_mut().delete_npc(creature);
                            }
                        } else if ctx.world().player(creature).is_some() && !ctx.is_gm(creature) {
                            tele_exit(ctx, creature);
                        }
                    }

                    reset_fight_state(ctx);
                    if let Some(player) = player {
                        ctx.world()
                            .send_message(player, &format!("{}: Fight has been aborted!", self.name()));
                    }
                } else if let Some(player) = player {
                    ctx.world().send_message(
                        player,
                        &format!("{}: You can't abort fight right now!", self.name()),
                    );
                }
            }
            EVENT_MANAGE_SKILL => {
                if let Some(npc) = npc {
                    manage_skills(ctx, npc);
                }
            }
            _ => {}
        }

        None
    }

    /// Handles bomber aggro: the bomber blows up when a player comes near.
    fn on_aggro_range_enter(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId, _is_summon: bool) {
        if npc.id != ANTHARAS_BOMBER_ID {
            return;
        }

        ctx.world_mut().do_cast(npc.object_id, ANTHARAS_BOMBER_DISPEL);
        ctx.world_mut().do_die(npc.object_id, Some(player));
    }

    fn on_attack(
        &self,
        ctx: &mut ScriptCtx<'_>,
        npc: ScriptNpc,
        attacker: ObjectId,
        damage: i32,
        _is_summon: bool,
        skill: Option<SkillKey>,
    ) {
        let npc_id = npc.id;
        if npc_id == ANTHARAS_BOMBER_ID {
            if distance_between(ctx, npc.object_id, attacker) < BOMBER_EXPLOSION_RANGE {
                ctx.world_mut().do_cast(npc.object_id, ANTHARAS_BOMBER_DISPEL);
                ctx.world_mut().do_die(npc.object_id, Some(attacker));
            }
        } else if npc_id == ANTHARAS_NPC_ID {
            let now = ctx.now_ms();
            ctx.state::<AntharasState>().last_attack_time = now;

            let (zone_id, zone_type) = ANTHARAS_NEST_ZONE;
            if !ctx.world().is_character_in_zone(zone_id, zone_type, attacker) || (get_status(ctx) != STATUS_IN_FIGHT) {
                warn!(
                    "{}: Player {} attacked Antharas in invalid conditions!",
                    self.name(),
                    ctx.player_name(attacker)
                );
                tele_player(ctx, attacker, 80464, 152294, -3534);
            }

            if (ctx.world().mount_type_of(attacker) == MountType::Strider)
                && !ctx
                    .world()
                    .is_affected_by_skill(attacker, ANTHARAS_ANTI_STRIDER.id)
                && ctx
                    .world_mut()
                    .check_do_cast_conditions_of(npc.object_id, ANTHARAS_ANTI_STRIDER)
            {
                ctx.add_skill_cast_desire(npc, attacker, ANTHARAS_ANTI_STRIDER, 100);
            }

            let (hp, max_hp) = hp_of(ctx, npc.object_id);
            if skill.is_none() {
                refresh_ai_params(ctx, attacker, damage.wrapping_mul(1000));
            } else if hp < (max_hp * 0.25) {
                refresh_ai_params(ctx, attacker, (damage / 3).wrapping_mul(100));
            } else if hp < (max_hp * 0.5) {
                refresh_ai_params(ctx, attacker, damage.wrapping_mul(20));
            } else if hp < (max_hp * 0.75) {
                refresh_ai_params(ctx, attacker, damage.wrapping_mul(10));
            } else {
                refresh_ai_params(ctx, attacker, (damage / 3).wrapping_mul(20));
            }

            manage_skills(ctx, npc);
        }
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        let npc_id = npc.id;
        let (zone_id, zone_type) = ANTHARAS_NEST_ZONE;
        if npc_id == ANTHARAS_NPC_ID {
            if !ctx.world().is_character_in_zone(zone_id, zone_type, killer) {
                warn!(
                    "{}: Antharas was killed by {} outside of nest zone. Forcing death handling.",
                    self.name(),
                    ctx.player_name(killer)
                );
            }

            ctx.state::<AntharasState>().antharas_boss = None;
            ctx.notify_event(EVENT_DESPAWN_MINIONS, None, None);
            broadcast_nest(ctx, &camera(npc, 1200, 20, -10, 0, 13000, 0, 0, 0, 0, 0));
            broadcast_nest(ctx, &PlaySound::file("BS01_D"));
            ctx.add_spawn(
                ANTHARAS_TELEPORT_CUBE_ID,
                177615,
                114941,
                -7709,
                0,
                false,
                TELEPORT_CUBE_DURATION,
                0,
            );

            let grand_boss = &ctx.world().boss_settings().grand_boss;
            let base_interval_millis = i64::from(grand_boss.antharas_spawn_interval) * 3600000;
            let random_range_millis = i64::from(grand_boss.antharas_spawn_random) * 3600000;
            let respawn_delay = base_interval_millis
                + ctx
                    .world_mut()
                    .rng()
                    .get_i64(-random_range_millis, random_range_millis);

            set_respawn(ctx, respawn_delay);
            ctx.start_quest_timer(EVENT_CLEAR_STATUS, respawn_delay, None, None, false);
            ctx.cancel_quest_timer(EVENT_SET_REGEN, Some(npc), None);
            ctx.cancel_quest_timer(EVENT_CHECK_ATTACK, Some(npc), None);
            ctx.cancel_quest_timer(EVENT_SPAWN_MINION, Some(npc), None);
            ctx.start_quest_timer(EVENT_CLEAR_ZONE, TELEPORT_CUBE_DURATION, None, None, false);
            set_status(ctx, STATUS_DEAD);

            let next_respawn_time = ctx.now_ms() + respawn_delay;
            info!("Antharas will respawn at: {}", date_time_string(next_respawn_time));

            reset_fight_state(ctx);
        } else if (npc_id == ANTHARAS_BEHEMOTH_ID) && ctx.world().is_character_in_zone(zone_id, zone_type, killer) {
            ctx.state::<AntharasState>().minion_count -= 1;
        }
    }

    /// Handles bomber movement completion (self-destruction).
    fn on_move_finished(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc) {
        ctx.world_mut().do_cast(npc.object_id, ANTHARAS_BOMBER_DISPEL);
        ctx.world_mut().do_die(npc.object_id, None);
    }

    /// Handles spawn initialization for Antharas and the invisible fear-phase triggers.
    fn on_spawn(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc) {
        if npc.id == ANTHARAS_NPC_ID {
            ctx.world_mut().set_can_return_to_spawn_point(npc.object_id, false);
            ctx.world_mut().set_random_walking(npc.object_id, false);

            ctx.cancel_quest_timer(EVENT_SET_REGEN, Some(npc), None);
            ctx.start_quest_timer(EVENT_SET_REGEN, REGEN_INTERVAL, Some(npc), None, false);
            ctx.world_mut().set_on_kill_delay(npc.object_id, 0);
        } else {
            let Some((x, y, z)) = ctx.world().position_of(npc.object_id) else {
                return;
            };
            for i in 1..=6 {
                // Java's `getInt` throws for a missing parameter; 0 here.
                let suicide_x = ctx
                    .world()
                    .npc_template_param_int(npc.object_id, &format!("suicide{i}_x"))
                    .unwrap_or(0);
                let suicide_y = ctx
                    .world()
                    .npc_template_param_int(npc.object_id, &format!("suicide{i}_y"))
                    .unwrap_or(0);
                if let Some(bomber) = ctx.add_spawn(ANTHARAS_BOMBER_ID, x, y, z, 0, true, 15000, 0) {
                    ctx.world_mut()
                        .npc_set_intention_move_to(bomber.object_id, suicide_x, suicide_y, z);
                }
            }

            ctx.world_mut().delete_npc(npc.object_id);
        }
    }

    /// Chains the follow-up AI events of Antharas' spells.
    fn on_spell_finished(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, _player: Option<ObjectId>, skill: SkillKey) {
        let skill_id = skill.id;
        if (skill_id == ANTHARAS_FEAR.id) || (skill_id == ANTHARAS_FEAR_SHORT.id) {
            ctx.start_quest_timer(EVENT_TID_USED_FEAR, FEAR_USED_DELAY, Some(npc), None, false);
        }

        ctx.start_quest_timer(EVENT_MANAGE_SKILL, 1000, Some(npc), None, false);
    }

    fn on_first_talk(&self, _ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, _player: ObjectId) -> Option<String> {
        Some(format!("{}.html", npc.id))
    }
}

/// The `CLEAR_STATUS` case: Antharas is back in its nest.
fn clear_status(ctx: &mut ScriptCtx<'_>) {
    if let Some(boss) = spawn_idle(ctx) {
        add_boss(ctx, boss);
    }
    let (x, y, z) = ANTHARAS_IDLE_LOCATION;
    ctx.world().broadcast_to_all_online_players(&Earthquake {
        x,
        y,
        z,
        intensity: 20,
        duration: 10,
    });
    set_status(ctx, STATUS_ALIVE);
    reset_fight_state(ctx);
}

/// `_antharasBoss = (GrandBoss) addSpawn(ANTHARAS_NPC_ID, ANTHARAS_IDLE_LOCATION, false, 0)`.
fn spawn_idle(ctx: &mut ScriptCtx<'_>) -> Option<ScriptNpc> {
    let (x, y, z) = ANTHARAS_IDLE_LOCATION;
    let boss = ctx.add_spawn(ANTHARAS_NPC_ID, x, y, z, 0, false, 0, 0);
    ctx.state::<AntharasState>().antharas_boss = boss.map(|b| b.object_id);
    boss
}

fn wait_time_ms(ctx: &ScriptCtx<'_>) -> i64 {
    i64::from(ctx.world().boss_settings().grand_boss.antharas_wait_time) * 60000
}

/// Java `getStatus()`; a boss without a row (Java throws) matches no status.
fn get_status(ctx: &ScriptCtx<'_>) -> i32 {
    ctx.world().grand_boss_status(ANTHARAS_NPC_ID).unwrap_or(-1)
}

fn add_boss(ctx: &mut ScriptCtx<'_>, grand_boss: ScriptNpc) {
    ctx.world_mut().add_grand_boss(ANTHARAS_NPC_ID, grand_boss.object_id);
}

fn set_status(ctx: &mut ScriptCtx<'_>, status: i32) {
    ctx.world_mut().set_grand_boss_status(ANTHARAS_NPC_ID, status);
}

fn set_respawn(ctx: &mut ScriptCtx<'_>, respawn_time: i64) {
    let at = ctx.now_ms() + respawn_time;
    ctx.world_mut().set_grand_boss_respawn_time(ANTHARAS_NPC_ID, at);
}

fn reset_fight_state(ctx: &mut ScriptCtx<'_>) {
    let boss = ctx.state::<AntharasState>().antharas_boss;
    *ctx.state::<AntharasState>() = AntharasState {
        antharas_boss: boss,
        ..AntharasState::default()
    };
}

fn refresh_ai_params(ctx: &mut ScriptCtx<'_>, attacker: ObjectId, damage: i32) {
    let st = ctx.state::<AntharasState>();
    let (primary, secondary, tertiary) = (st.primary_attacker, st.secondary_attacker, st.tertiary_attacker);
    let (primary_hate, secondary_hate, tertiary_hate) =
        (st.primary_attacker_hate, st.secondary_attacker_hate, st.tertiary_attacker_hate);
    if primary == Some(attacker) {
        if primary_hate < damage.wrapping_add(1000) {
            let hate = damage.wrapping_add(ctx.get_random(3000));
            ctx.state::<AntharasState>().primary_attacker_hate = hate;
        }
    } else if secondary == Some(attacker) {
        if secondary_hate < damage.wrapping_add(1000) {
            let hate = damage.wrapping_add(ctx.get_random(3000));
            ctx.state::<AntharasState>().secondary_attacker_hate = hate;
        }
    } else if tertiary == Some(attacker) {
        if tertiary_hate < damage.wrapping_add(1000) {
            let hate = damage.wrapping_add(ctx.get_random(3000));
            ctx.state::<AntharasState>().tertiary_attacker_hate = hate;
        }
    } else {
        let minimum_hate = primary_hate.min(secondary_hate).min(tertiary_hate);
        if primary_hate == minimum_hate {
            let hate = damage.wrapping_add(ctx.get_random(3000));
            let st = ctx.state::<AntharasState>();
            st.primary_attacker_hate = hate;
            st.primary_attacker = Some(attacker);
        } else if secondary_hate == minimum_hate {
            let hate = damage.wrapping_add(ctx.get_random(3000));
            let st = ctx.state::<AntharasState>();
            st.secondary_attacker_hate = hate;
            st.secondary_attacker = Some(attacker);
        } else if tertiary_hate == minimum_hate {
            let hate = damage.wrapping_add(ctx.get_random(3000));
            let st = ctx.state::<AntharasState>();
            st.tertiary_attacker_hate = hate;
            st.tertiary_attacker = Some(attacker);
        }
    }
}

/// `(attacker == null) || (npc.calculateDistance3D(attacker) > 9000) || attacker.isDead()`.
fn attacker_lost(ctx: &ScriptCtx<'_>, npc: ObjectId, attacker: Option<ObjectId>) -> bool {
    match attacker {
        None => true,
        Some(attacker) => distance_between(ctx, npc, attacker) > MAX_THREAT_DISTANCE || ctx.world().is_dead(attacker),
    }
}

/// The tail and debuff ranges of `manageSkills`: distance and direction to the target.
fn tail_range(distance: f64, direction: f64) -> bool {
    ((distance < 1423.0) && (direction < 188.0) && (direction > 172.0))
        || ((distance < 802.0) && (direction < 194.0) && (direction > 166.0))
}

fn debuff_range(distance: f64, direction: f64) -> bool {
    ((distance < 850.0) && (direction < 210.0) && (direction > 150.0))
        || ((distance < 425.0) && (direction < 270.0) && (direction > 90.0))
}

fn manage_skills(ctx: &mut ScriptCtx<'_>, npc: ScriptNpc) {
    let id = npc.object_id;
    if ctx.world().is_casting_now(id) || ctx.world().is_core_ai_disabled(id) || !ctx.world().is_in_combat(id) {
        return;
    }

    let mut selected_target = None;
    let mut selected_slot = 0;
    let mut selected_hate = 0;

    let (primary, secondary, tertiary) = {
        let st = ctx.state::<AntharasState>();
        (st.primary_attacker, st.secondary_attacker, st.tertiary_attacker)
    };
    if attacker_lost(ctx, id, primary) {
        ctx.state::<AntharasState>().primary_attacker_hate = 0;
    }

    if attacker_lost(ctx, id, secondary) {
        ctx.state::<AntharasState>().secondary_attacker_hate = 0;
    }

    if attacker_lost(ctx, id, tertiary) {
        ctx.state::<AntharasState>().tertiary_attacker_hate = 0;
    }

    let (primary_hate, secondary_hate, tertiary_hate) = {
        let st = ctx.state::<AntharasState>();
        (st.primary_attacker_hate, st.secondary_attacker_hate, st.tertiary_attacker_hate)
    };
    if primary_hate > secondary_hate {
        selected_slot = 2;
        selected_hate = primary_hate;
        selected_target = primary;
    } else if secondary_hate > 0 {
        selected_slot = 3;
        selected_hate = secondary_hate;
        selected_target = secondary;
    }

    if tertiary_hate > selected_hate {
        selected_slot = 4;
        selected_hate = tertiary_hate;
        selected_target = tertiary;
    }

    if selected_hate > 0 {
        if ctx.get_random(100) < 70 {
            let st = ctx.state::<AntharasState>();
            match selected_slot {
                2 => st.primary_attacker_hate = 500,
                3 => st.secondary_attacker_hate = 500,
                4 => st.tertiary_attacker_hate = 500,
                _ => {}
            }
        }

        // A hate above 0 always comes with its attacker.
        let Some(selected_target) = selected_target else {
            return;
        };
        let distance_to_target = distance_between(ctx, id, selected_target);
        let direction_to_target = match (ctx.world().position_of(id), ctx.world().position_of(selected_target)) {
            (Some(me), Some(target)) => angle_from(me, target),
            _ => 0.0,
        };
        let (hp, max_hp) = hp_of(ctx, id);
        let skill_to_cast;
        let mut cast_on_target = false;

        if hp < (max_hp * 0.25) {
            if ctx.get_random(100) < 30 {
                cast_on_target = true;
                skill_to_cast = ANTHARAS_MOUTH_ATTACK;
            } else if (ctx.get_random(100) < 80) && tail_range(distance_to_target, direction_to_target) {
                skill_to_cast = ANTHARAS_TAIL_ATTACK;
            } else if (ctx.get_random(100) < 40) && debuff_range(distance_to_target, direction_to_target) {
                skill_to_cast = ANTHARAS_DEBUFF;
            } else if (ctx.get_random(100) < 10) && (distance_to_target < 1100.0) {
                skill_to_cast = ANTHARAS_JUMP;
            } else if ctx.get_random(100) < 10 {
                cast_on_target = true;
                skill_to_cast = ANTHARAS_METEOR;
            } else if ctx.get_random(100) < 6 {
                cast_on_target = true;
                skill_to_cast = ANTHARAS_BREATH_ATTACK;
            } else if ctx.get_random_boolean() {
                cast_on_target = true;
                skill_to_cast = ANTHARAS_NORMAL_ATTACK_EX;
            } else if ctx.get_random(100) < 5 {
                cast_on_target = true;
                skill_to_cast = if ctx.get_random_boolean() {
                    ANTHARAS_FEAR
                } else {
                    ANTHARAS_FEAR_SHORT
                };
            } else {
                cast_on_target = true;
                skill_to_cast = ANTHARAS_NORMAL_ATTACK;
            }
        } else if hp < (max_hp * 0.5) {
            if (ctx.get_random(100) < 80) && tail_range(distance_to_target, direction_to_target) {
                skill_to_cast = ANTHARAS_TAIL_ATTACK;
            } else if (ctx.get_random(100) < 40) && debuff_range(distance_to_target, direction_to_target) {
                skill_to_cast = ANTHARAS_DEBUFF;
            } else if (ctx.get_random(100) < 10) && (distance_to_target < 1100.0) {
                skill_to_cast = ANTHARAS_JUMP;
            } else if ctx.get_random(100) < 7 {
                cast_on_target = true;
                skill_to_cast = ANTHARAS_METEOR;
            } else if ctx.get_random(100) < 6 {
                cast_on_target = true;
                skill_to_cast = ANTHARAS_BREATH_ATTACK;
            } else if ctx.get_random_boolean() {
                cast_on_target = true;
                skill_to_cast = ANTHARAS_NORMAL_ATTACK_EX;
            } else if ctx.get_random(100) < 5 {
                cast_on_target = true;
                skill_to_cast = if ctx.get_random_boolean() {
                    ANTHARAS_FEAR
                } else {
                    ANTHARAS_FEAR_SHORT
                };
            } else {
                cast_on_target = true;
                skill_to_cast = ANTHARAS_NORMAL_ATTACK;
            }
        } else if hp < (max_hp * 0.75) {
            if (ctx.get_random(100) < 80) && tail_range(distance_to_target, direction_to_target) {
                skill_to_cast = ANTHARAS_TAIL_ATTACK;
            } else if (ctx.get_random(100) < 10) && (distance_to_target < 1100.0) {
                skill_to_cast = ANTHARAS_JUMP;
            } else if ctx.get_random(100) < 5 {
                cast_on_target = true;
                skill_to_cast = ANTHARAS_METEOR;
            } else if ctx.get_random(100) < 6 {
                cast_on_target = true;
                skill_to_cast = ANTHARAS_BREATH_ATTACK;
            } else if ctx.get_random_boolean() {
                cast_on_target = true;
                skill_to_cast = ANTHARAS_NORMAL_ATTACK_EX;
            } else if ctx.get_random(100) < 5 {
                cast_on_target = true;
                skill_to_cast = if ctx.get_random_boolean() {
                    ANTHARAS_FEAR
                } else {
                    ANTHARAS_FEAR_SHORT
                };
            } else {
                cast_on_target = true;
                skill_to_cast = ANTHARAS_NORMAL_ATTACK;
            }
        } else if (ctx.get_random(100) < 80) && tail_range(distance_to_target, direction_to_target) {
            skill_to_cast = ANTHARAS_TAIL_ATTACK;
        } else if ctx.get_random(100) < 3 {
            cast_on_target = true;
            skill_to_cast = ANTHARAS_METEOR;
        } else if ctx.get_random(100) < 6 {
            cast_on_target = true;
            skill_to_cast = ANTHARAS_BREATH_ATTACK;
        } else if ctx.get_random_boolean() {
            cast_on_target = true;
            skill_to_cast = ANTHARAS_NORMAL_ATTACK_EX;
        } else if ctx.get_random(100) < 5 {
            cast_on_target = true;
            skill_to_cast = if ctx.get_random_boolean() {
                ANTHARAS_FEAR
            } else {
                ANTHARAS_FEAR_SHORT
            };
        } else {
            cast_on_target = true;
            skill_to_cast = ANTHARAS_NORMAL_ATTACK;
        }

        if ctx.world_mut().check_do_cast_conditions_of(id, skill_to_cast) {
            if cast_on_target {
                ctx.add_skill_cast_desire(npc, selected_target, skill_to_cast, 100);
            } else {
                ctx.world_mut().set_intention_cast(id, skill_to_cast, Some(id));
            }
        }
    }
}

// ------------------------------------------------------------------ world helpers

/// Java `new SpecialCamera(npc, force, angle1, angle2, time, range, duration, relYaw, relPitch,
/// isWide, relAngle, unk)` without the unused `range`.
#[allow(clippy::too_many_arguments)]
fn camera(
    npc: ScriptNpc,
    force: i32,
    angle1: i32,
    angle2: i32,
    time: i32,
    duration: i32,
    rel_yaw: i32,
    rel_pitch: i32,
    is_wide: i32,
    rel_angle: i32,
    unk: i32,
) -> SpecialCamera {
    SpecialCamera::new(
        npc.object_id.0,
        force,
        angle1,
        angle2,
        time,
        duration,
        rel_yaw,
        rel_pitch,
        is_wide,
        rel_angle,
        unk,
    )
}

fn social(npc: ScriptNpc, action_id: i32) -> SocialAction {
    SocialAction {
        object_id: npc.object_id.0,
        action_id,
    }
}

fn broadcast_nest(ctx: &ScriptCtx<'_>, packet: &impl l2_net::Encode) {
    let (zone_id, zone_type) = ANTHARAS_NEST_ZONE;
    ctx.world().broadcast_to_zone(zone_id, zone_type, packet);
}

fn nest_players_inside(ctx: &ScriptCtx<'_>) -> usize {
    let (zone_id, zone_type) = ANTHARAS_NEST_ZONE;
    ctx.world().zone_players_inside(zone_id, zone_type).len()
}

fn nest_characters_inside(ctx: &ScriptCtx<'_>) -> Vec<ObjectId> {
    let (zone_id, zone_type) = ANTHARAS_NEST_ZONE;
    ctx.world().zone_characters_inside(zone_id, zone_type)
}

/// `creature.teleToLocation(new Location(x, y, z, heading))` of an NPC.
fn tele_npc(ctx: &mut ScriptCtx<'_>, id: ObjectId, (x, y, z): (i32, i32, i32), heading: i32) {
    ctx.world_mut().tele_to_location(
        id,
        TeleportTarget::at(x, y, z).with_heading(heading).with_instance(0),
    );
}

/// `player.teleToLocation(x, y, z)`.
fn tele_player(ctx: &mut ScriptCtx<'_>, id: ObjectId, x: i32, y: i32, z: i32) {
    ctx.world_mut().tele_to_location(id, TeleportTarget::at(x, y, z));
}

/// `teleToLocation(TELEPORT_EXIT_BASE_X + getRandom(600), TELEPORT_EXIT_BASE_Y + getRandom(1100), TELEPORT_EXIT_Z)`.
fn tele_exit(ctx: &mut ScriptCtx<'_>, id: ObjectId) {
    let x = TELEPORT_EXIT_BASE_X + ctx.get_random(600);
    let y = TELEPORT_EXIT_BASE_Y + ctx.get_random(1100);
    tele_player(ctx, id, x, y, TELEPORT_EXIT_Z);
}

fn hp_of(ctx: &ScriptCtx<'_>, id: ObjectId) -> (f64, f64) {
    ctx.world().hp_and_max(id).unwrap_or((0.0, 0.0))
}

fn distance_between(ctx: &ScriptCtx<'_>, a: ObjectId, b: ObjectId) -> f64 {
    match (ctx.world().position_of(a), ctx.world().position_of(b)) {
        (Some(a), Some(b)) => distance_3d(a, b),
        _ => f64::MAX,
    }
}

/// Java `isInsideRadius3D(other, radius)`: `calculateDistance3D(other) < radius`.
fn inside_radius_3d(ctx: &ScriptCtx<'_>, a: ObjectId, b: ObjectId, radius: f64) -> bool {
    distance_between(ctx, a, b) < radius
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::{BossReadKind, BossReadThen, BossWrite, ScriptRegistry};

    use super::*;
    use crate::ai::bosses::test_support::*;
    use crate::testing::{ScriptTest, item, names_to, pages_to};

    const DIR: &str = "data/scripts/ai/bosses/Antharas";
    const ITEMS: &str = r#"<item id="3865" type="EtcItem" name="Portal Stone"><set name="is_stackable" val="true"/></item>"#;

    fn creature_xml(id: i32, kind: &str, name: &str, params: &str) -> String {
        format!(
            r#"<npc id="{id}" type="{kind}" level="85" name="{name}">
    <parameters>{params}</parameters>
    <stats str="40" int="21" dex="30" wit="20" con="43" men="20">
      <vitals hp="1000" hpRegen="2" mp="500" mpRegen="0.9" />
      <attack physical="8" magical="5" random="30" critical="4" accuracy="4.75" attackSpeed="253" type="SWORD" range="40" distance="80" width="120" />
      <defence physical="44.44444" magical="29.5" />
      <speed><walk ground="20" /><run ground="50" /></speed>
    </stats>
    <collision><radius normal="10"/><height normal="20"/></collision>
  </npc>"#
        )
    }

    fn npcs() -> String {
        let mut xml = creature_xml(ANTHARAS_NPC_ID, "GrandBoss", "Antharas", "");
        xml += &creature_xml(ANTHARAS_BEHEMOTH_ID, "Monster", "Behemoth Dragon", "");
        xml += &creature_xml(ANTHARAS_TERASQUE_ID, "Monster", "Tarask Dragon", "");
        xml += &creature_xml(ANTHARAS_BOMBER_ID, "Monster", "Dragon Bomber", "");
        for (id, _) in INVISIBLE_NPC_SPAWN_LOCATIONS {
            let params: String = (1..=6)
                .map(|i| {
                    format!(
                        r#"<param name="suicide{i}_x" value="{}" /><param name="suicide{i}_y" value="111400" />"#,
                        177000 + i * 100
                    )
                })
                .collect();
            xml += &creature_xml(id, "Monster", "antaras_clear_npc", &params);
        }
        xml += r#"<npc id="13001" type="Folk" level="70" name="Heart of Warding"/>"#;
        xml += r#"<npc id="31859" type="Folk" level="70" name="Teleportation Cubic"/>"#;
        xml
    }

    fn setup_rows(status: i32, respawn_time: i64, at: (i32, i32, i32), writes: bool) -> ScriptTest {
        let mut registry = ScriptRegistry::new();
        registry.add(Antharas);
        let enter = r#"<html><body>13001<a action="bypass -h Script Antharas ENTER">Move to Antharas' Nest</a></body></html>"#;
        let pages: Vec<(String, String)> = [
            ("13001.html", enter.to_owned()),
            ("13001-01.html", "<html><body>13001-01</body></html>".to_owned()),
            ("13001-02.html", "<html><body>13001-02</body></html>".to_owned()),
            ("13001-03.html", "<html><body>13001-03</body></html>".to_owned()),
            ("13001-04.html", "<html><body>13001-04</body></html>".to_owned()),
            ("13001-05.html", "<html><body>13001-05</body></html>".to_owned()),
        ]
        .into_iter()
        .map(|(file, text)| (format!("{DIR}/{file}"), text))
        .collect();
        let pages: Vec<(&str, &str)> = pages.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
        boss_test(
            registry,
            &npcs(),
            ITEMS,
            &pages,
            vec![row(ANTHARAS_NPC_ID, status, respawn_time, at)],
            writes,
        )
    }

    fn setup(status: i32, writes: bool) -> ScriptTest {
        setup_rows(status, 0, (1, 2, 3), writes)
    }

    fn has_timer(t: &mut ScriptTest, name: &str, npc: Option<Oid>) -> bool {
        let mut found = false;
        t.world.run_as_script("Antharas", |ctx| {
            let npc = npc.and_then(|n| ctx.script_npc(n));
            found = ctx.has_quest_timer(name, npc, None);
        });
        found
    }

    fn notify(t: &mut ScriptTest, event: &str, npc: Option<Oid>, player: Option<i32>) {
        t.world.run_as_script("Antharas", |ctx| {
            let npc = npc.and_then(|n| ctx.script_npc(n));
            ctx.notify_event(event, npc, player.map(Oid));
        });
    }

    fn status(t: &ScriptTest) -> Option<i32> {
        t.world.grand_boss_status(ANTHARAS_NPC_ID)
    }

    /// A player with the Portal Stone goes into the nest through the heart and lands there.
    fn enter_nest(t: &mut ScriptTest, player: i32) {
        let heart = t.spawn(ANTHARAS_HEART_ID, 20);
        t.enter(player, 80, vec![item(player * 100, ANTHARAS_STONE_ITEM_ID, 1)]);
        t.talk(player, heart);
        t.bypass(player, "Script Antharas ENTER");
        appear(t, player);
    }

    /// Up to the end of the wake-up movie.
    fn awake(t: &mut ScriptTest, player: i32) -> Oid {
        enter_nest(t, player);
        notify(t, EVENT_SKIP_WAITING, None, Some(player));
        run_ms(t, 26_000);
        t.world.grand_boss_npc(ANTHARAS_NPC_ID).unwrap()
    }

    #[test]
    fn the_heart_lets_a_player_with_the_portal_stone_in_and_the_wait_starts() {
        let mut t = setup(STATUS_ALIVE, true);
        // The constructor: Antharas sleeps in its nest.
        let boss = t.world.grand_boss_npc(ANTHARAS_NPC_ID).unwrap();
        let (x, y, _) = t.world.position_of(boss).unwrap();
        assert_eq!((x, y), (ANTHARAS_IDLE_LOCATION.0, ANTHARAS_IDLE_LOCATION.1));
        assert!(has_timer(&mut t, EVENT_SET_REGEN, Some(boss)));

        let heart = t.spawn(ANTHARAS_HEART_ID, 20);
        t.enter(1, 80, vec![]);
        t.take();
        t.talk(1, heart);
        assert!(pages_to(&t.take(), 1).pop().unwrap().contains("13001"));
        t.bypass(1, "Script Antharas ENTER");
        assert!(pages_to(&t.take(), 1).pop().unwrap().contains("13001-03"));
        assert_eq!(status(&t), Some(STATUS_ALIVE));
        t.take_boss();

        enter_nest(&mut t, 2);
        assert_eq!(status(&t), Some(STATUS_WAITING));
        assert_eq!(
            grand_writes(&t.take_boss()),
            [BossWrite::GrandBossStatus {
                boss_id: ANTHARAS_NPC_ID,
                status: STATUS_WAITING
            }]
        );
        assert!(has_timer(&mut t, EVENT_SPAWN_ANTHARAS, None));
        let (x, y, z) = t.world.position_of(Oid(2)).unwrap();
        assert!((179700..179700 + 700).contains(&x) && (113800..113800 + 2100).contains(&y));
        // `teleToLocation` lifts the player a few units (no geodata here).
        assert!((-7709..-7699).contains(&z));
        assert!(t.world.zone_players_inside(70050, "NoRestartZone").contains(&Oid(2)));
        // The stone stays with the player (Java only checks it).
        assert_eq!(t.count(2, ANTHARAS_STONE_ITEM_ID), 1);
    }

    #[test]
    fn the_wake_up_movie_runs_and_the_fight_starts() {
        let mut t = setup(STATUS_ALIVE, true);
        enter_nest(&mut t, 2);
        t.take();
        t.take_boss();
        let boss = t.world.grand_boss_npc(ANTHARAS_NPC_ID).unwrap();

        notify(&mut t, EVENT_SKIP_WAITING, None, Some(2));
        assert_eq!(status(&t), Some(STATUS_IN_FIGHT));
        assert!(!has_timer(&mut t, EVENT_SPAWN_ANTHARAS, None));
        assert!(t.world.is_core_ai_disabled(boss));
        let (x, y, _) = t.world.position_of(boss).unwrap();
        assert_eq!((x, y), (ANTHARAS_SPAWN_LOCATION.0, ANTHARAS_SPAWN_LOCATION.1));
        let out = t.take();
        let names = names_to(&out, 2);
        assert!(names.contains(&"PLAY_SOUND".to_owned()));
        assert_eq!(
            grand_writes(&t.take_boss()),
            [BossWrite::GrandBossStatus {
                boss_id: ANTHARAS_NPC_ID,
                status: STATUS_IN_FIGHT
            }]
        );

        run_ms(&mut t, 26_000);
        let names = names_to(&t.take(), 2);
        assert_eq!(names.iter().filter(|n| *n == "SPECIAL_CAMERA").count(), 5);
        assert!(names.iter().filter(|n| *n == "SOCIAL_ACTION").count() >= 2);
        // START_MOVE: the core AI is back and the fight timers run.
        assert!(!t.world.is_core_ai_disabled(boss));
        assert!(has_timer(&mut t, EVENT_CHECK_ATTACK, Some(boss)));
        assert!(has_timer(&mut t, EVENT_SPAWN_MINION, Some(boss)));

        // A second skip is refused.
        notify(&mut t, EVENT_SKIP_WAITING, None, Some(2));
        assert!(names_to(&t.take(), 2).contains(&"SYSTEM_MESSAGE".to_owned()));
    }

    #[test]
    fn minion_waves_come_and_the_gm_removes_them() {
        let mut t = setup(STATUS_ALIVE, true);
        let boss = awake(&mut t, 2);
        notify(&mut t, EVENT_SPAWN_MINION, Some(boss), None);
        assert_eq!(npcs_of(&t, ANTHARAS_BEHEMOTH_ID).len(), 1);
        assert_eq!(npcs_of(&t, ANTHARAS_TERASQUE_ID).len(), 1);
        let mut count = 0;
        t.world.run_as_script("Antharas", |ctx| {
            count = ctx.state::<AntharasState>().minion_count;
        });
        assert_eq!(count, 2);

        t.take();
        notify(&mut t, EVENT_DESPAWN_MINIONS, None, Some(2));
        assert!(npcs_of(&t, ANTHARAS_BEHEMOTH_ID).is_empty());
        assert!(npcs_of(&t, ANTHARAS_TERASQUE_ID).is_empty());
        assert!(names_to(&t.take(), 2).contains(&"SYSTEM_MESSAGE".to_owned()));
    }

    #[test]
    fn fifteen_minutes_without_a_hit_send_everybody_out() {
        let mut t = setup(STATUS_ALIVE, true);
        let boss = awake(&mut t, 2);
        let minion = t
            .world
            .spawn_npc_at(ANTHARAS_BEHEMOTH_ID, 181000, 114850, -7623, 0)
            .unwrap();
        t.world.run_as_script("Antharas", |ctx| {
            ctx.state::<AntharasState>().last_attack_time = 0;
        });
        t.take_boss();
        notify(&mut t, EVENT_CHECK_ATTACK, Some(boss), None);
        assert_eq!(status(&t), Some(STATUS_ALIVE));
        let (x, y, _) = t.world.position_of(boss).unwrap();
        assert_eq!((x, y), (ANTHARAS_IDLE_LOCATION.0, ANTHARAS_IDLE_LOCATION.1));
        assert!(t.world.npc(minion).is_none());
        let (x, y, z) = t.world.position_of(Oid(2)).unwrap();
        assert!((TELEPORT_EXIT_BASE_X..TELEPORT_EXIT_BASE_X + 600).contains(&x));
        assert!((TELEPORT_EXIT_BASE_Y..TELEPORT_EXIT_BASE_Y + 1100).contains(&y));
        assert!((TELEPORT_EXIT_Z..TELEPORT_EXIT_Z + 10).contains(&z));
        assert!(!has_timer(&mut t, EVENT_CHECK_ATTACK, Some(boss)));
        assert_eq!(
            grand_writes(&t.take_boss()),
            [BossWrite::GrandBossStatus {
                boss_id: ANTHARAS_NPC_ID,
                status: STATUS_ALIVE
            }]
        );
    }

    #[test]
    fn the_death_writes_the_state_and_the_gm_respawn_brings_antharas_back() {
        let mut t = setup(STATUS_ALIVE, true);
        let boss = awake(&mut t, 2);
        t.take_boss();
        let before = now_ms();
        assert!(t.world.do_die(boss, Some(Oid(2))));
        t.ticks(2);
        assert_eq!(status(&t), Some(STATUS_DEAD));
        assert_eq!(npcs_of(&t, ANTHARAS_TELEPORT_CUBE_ID).len(), 1);
        assert!(has_timer(&mut t, EVENT_CLEAR_STATUS, None));
        assert!(has_timer(&mut t, EVENT_CLEAR_ZONE, None));
        // 264 hours +- 72 (the Java defaults).
        let respawn = t.world.grand_boss_info(ANTHARAS_NPC_ID).unwrap().respawn_time;
        let hour = 3_600_000;
        assert!(respawn >= before + 192 * hour && respawn <= now_ms() + 336 * hour);
        // The status row only; the respawn time goes out with the five-minute store.
        assert_eq!(
            grand_writes(&t.take_boss()),
            [BossWrite::GrandBossStatus {
                boss_id: ANTHARAS_NPC_ID,
                status: STATUS_DEAD
            }]
        );
        // The heart says it is dead.
        let heart = t.spawn(ANTHARAS_HEART_ID, 20);
        t.enter(3, 80, vec![item(300, ANTHARAS_STONE_ITEM_ID, 1)]);
        t.talk(3, heart);
        t.take();
        t.bypass(3, "Script Antharas ENTER");
        assert!(pages_to(&t.take(), 3).pop().unwrap().contains("13001-01"));

        notify(&mut t, EVENT_RESPAWN_ANTHARAS, None, Some(2));
        assert_eq!(status(&t), Some(STATUS_ALIVE));
        let reborn = t.world.grand_boss_npc(ANTHARAS_NPC_ID).unwrap();
        assert_ne!(reborn, boss);
        assert!(!has_timer(&mut t, EVENT_CLEAR_STATUS, None));
        assert!(names_to(&t.take(), 3).contains(&"EARTHQUAKE".to_owned()));
        assert_eq!(
            grand_writes(&t.take_boss()),
            [BossWrite::GrandBossStatus {
                boss_id: ANTHARAS_NPC_ID,
                status: STATUS_ALIVE
            }]
        );
    }

    #[test]
    fn with_the_switch_off_nothing_is_written_and_the_row_decides_the_respawn() {
        let mut t = setup(STATUS_ALIVE, false);
        let boss = awake(&mut t, 2);
        assert_eq!(status(&t), Some(STATUS_IN_FIGHT));
        assert!(t.world.do_die(boss, Some(Oid(2))));
        t.ticks(2);
        assert_eq!(status(&t), Some(STATUS_DEAD));
        assert!(grand_writes(&t.take_boss()).is_empty());

        // The GM respawn (as the respawn timer) asks for the row first.
        notify(&mut t, EVENT_RESPAWN_ANTHARAS, None, Some(2));
        assert!(npcs_of(&t, ANTHARAS_NPC_ID).is_empty());
        let then = BossReadThen::ScriptEvent {
            script: "Antharas",
            event: EVENT_ROW_READ,
        };
        assert_eq!(
            rereads(&t.take_boss()),
            [(BossReadKind::GrandBoss(ANTHARAS_NPC_ID), then)]
        );

        // The other server killed it too: wait for its respawn time.
        let later = now_ms() + 3_600_000;
        answer(
            &mut t,
            then,
            ANTHARAS_NPC_ID,
            row(ANTHARAS_NPC_ID, STATUS_DEAD, later, (1, 2, 3)),
        );
        assert!(npcs_of(&t, ANTHARAS_NPC_ID).is_empty());
        assert!(has_timer(&mut t, EVENT_CLEAR_STATUS, None));

        // The row says alive: Antharas is back, still without a write.
        answer(
            &mut t,
            then,
            ANTHARAS_NPC_ID,
            row(ANTHARAS_NPC_ID, STATUS_ALIVE, 0, (1, 2, 3)),
        );
        assert_eq!(status(&t), Some(STATUS_ALIVE));
        assert_eq!(npcs_of(&t, ANTHARAS_NPC_ID).len(), 1);
        assert!(grand_writes(&t.take_boss()).is_empty());
    }

    #[test]
    fn the_constructor_restores_the_row() {
        let saved = (181000, 114000, -7600);
        // In fight, this server writes: Antharas stands where it was.
        let mut t = setup_rows(STATUS_IN_FIGHT, 0, saved, true);
        let boss = t.world.grand_boss_npc(ANTHARAS_NPC_ID).unwrap();
        let (x, y, _) = t.world.position_of(boss).unwrap();
        assert_eq!((x, y), (saved.0, saved.1));
        assert!(has_timer(&mut t, EVENT_CHECK_ATTACK, Some(boss)));
        assert!(has_timer(&mut t, EVENT_SPAWN_MINION, Some(boss)));

        // D-54: the same row on the shared database is the other server's fight.
        let t = setup_rows(STATUS_IN_FIGHT, 0, saved, false);
        assert!(npcs_of(&t, ANTHARAS_NPC_ID).is_empty());
        assert!(t.world.grand_boss_npc(ANTHARAS_NPC_ID).is_none());

        // Dead with the respawn time over: alive at once, or after a read of the row.
        let t = setup_rows(STATUS_DEAD, now_ms() - 1000, saved, true);
        assert_eq!(status(&t), Some(STATUS_ALIVE));
        assert_eq!(npcs_of(&t, ANTHARAS_NPC_ID).len(), 1);
        assert_eq!(
            grand_writes(&t.take_boss()),
            [BossWrite::GrandBossStatus {
                boss_id: ANTHARAS_NPC_ID,
                status: STATUS_ALIVE
            }]
        );
        let t = setup_rows(STATUS_DEAD, now_ms() - 1000, saved, false);
        assert!(npcs_of(&t, ANTHARAS_NPC_ID).is_empty());
        assert_eq!(rereads(&t.take_boss()).len(), 1);

        // Dead with time left: the timer.
        let mut t = setup_rows(STATUS_DEAD, now_ms() + 3_600_000, saved, false);
        assert!(npcs_of(&t, ANTHARAS_NPC_ID).is_empty());
        assert!(has_timer(&mut t, EVENT_CLEAR_STATUS, None));
    }

    #[test]
    fn the_fear_phase_sends_the_bombers() {
        let mut t = setup(STATUS_ALIVE, true);
        let boss = awake(&mut t, 2);
        notify(&mut t, EVENT_TID_USED_FEAR, Some(boss), None);
        assert!(t.world.is_core_ai_disabled(boss));
        // Antharas does not reach the spot: after the retries it is put there.
        for _ in 0..=MAX_MOVE_RETRY_COUNT {
            t.world.stop_move(boss, None);
            notify(&mut t, EVENT_TID_FEAR_MOVE_TIMEOVER, Some(boss), None);
        }
        t.world.stop_move(boss, None);
        notify(&mut t, EVENT_TID_FEAR_MOVE_TIMEOVER, Some(boss), None);
        let (x, y, _) = t.world.position_of(boss).unwrap();
        assert_eq!((x, y), (ANTHARAS_FEAR_MOVE_X, ANTHARAS_FEAR_MOVE_Y));
        notify(&mut t, EVENT_TID_FEAR_MOVE_TIMEOVER, Some(boss), None);
        assert!(!t.world.is_core_ai_disabled(boss));
        // Each of the 18 invisible NPCs leaves six bombers and goes.
        for (id, _) in INVISIBLE_NPC_SPAWN_LOCATIONS {
            assert!(npcs_of(&t, id).is_empty());
        }
        assert_eq!(npcs_of(&t, ANTHARAS_BOMBER_ID).len(), 18 * 6);
    }

    #[test]
    fn the_gm_events_answer_when_they_cannot_run() {
        let mut t = setup(STATUS_ALIVE, true);
        t.enter(5, 80, vec![]);
        t.take();
        for event in [
            EVENT_SKIP_WAITING,
            EVENT_RESPAWN_ANTHARAS,
            EVENT_DESPAWN_MINIONS,
            EVENT_ABORT_FIGHT,
        ] {
            notify(&mut t, event, None, Some(5));
            assert_eq!(names_to(&t.take(), 5), ["SYSTEM_MESSAGE"], "{event}");
        }
        assert_eq!(status(&t), Some(STATUS_ALIVE));
    }
}
