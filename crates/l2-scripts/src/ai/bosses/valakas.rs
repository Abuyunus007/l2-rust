//! Valakas, the Fire Dragon: the dormant boss hidden near its lair, the encounter start after the
//! waiting time, the spawn and death movies, the regeneration and skill rotation, the inactivity
//! reset and the respawn cycle on the `GrandBossManager` state.
// Java: dist/game/data/scripts/ai/bosses/Valakas/Valakas.java
//
// The Java fields are `ValakasState` (`ctx.state`). The constructor is `on_load`. Statuses: 0
// dormant (hidden, entry open), 1 waiting (a player entered, the movie is pending), 2 fighting,
// 3 dead. The entry itself (status 1 and the `VALAKAS_START_ENCOUNTER` timer) is
// `ai/others/ValakasTeleporters` (another script card).
//
// Side by side (D-54, `super` module): with `CycleDatabaseWrites` off the constructor restores the
// boss only from an alive row (a dead row still waits for its respawn time), and
// `VALAKAS_UNLOCK` reads the row again and brings Valakas back only when the row says alive.
//
// Not modelled: `unload` (D-16: no reload).

use l2_protocol_game::server::{ExShowScreenMessage, PlaySound, SocialAction, SpecialCamera};
use l2_util::ObjectId;
use l2_world::mount::MountType;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup, SkillKey, TeleportTarget};
use tracing::info;

use super::{EVENT_ROW_READ, date_time_string, row_lets_boss_run};

// NPCs.
const VALAKAS: i32 = 29028;
const TELEPORT_CUBE: i32 = 31759;

// Skills.
const SKILL_REGEN_ID: i32 = 4691;
const SKILL_LAVA_SKIN: SkillKey = SkillKey { id: 4680, level: 1 };
const SKILL_ANTI_STRIDER: SkillKey = SkillKey { id: 4258, level: 1 };
const SKILL_TABLE_REGULAR: [SkillKey; 4] = [
    SkillKey { id: 4681, level: 1 }, // Valakas Trample.
    SkillKey { id: 4682, level: 1 }, // Valakas Trample.
    SkillKey { id: 4683, level: 1 }, // Valakas Dragon Breath.
    SkillKey { id: 4689, level: 1 }, // Valakas Fear (level 1 used only).
];
const SKILL_TABLE_LOW_HP: [SkillKey; 5] = [
    SkillKey { id: 4681, level: 1 }, // Valakas Trample.
    SkillKey { id: 4682, level: 1 }, // Valakas Trample.
    SkillKey { id: 4683, level: 1 }, // Valakas Dragon Breath.
    SkillKey { id: 4689, level: 1 }, // Valakas Fear (level 1 used only).
    SkillKey { id: 4690, level: 1 }, // Valakas Meteor Storm.
];
const SKILL_TABLE_AOE: [SkillKey; 7] = [
    SkillKey { id: 4683, level: 1 }, // Valakas Dragon Breath.
    SkillKey { id: 4684, level: 1 }, // Valakas Dragon Breath.
    SkillKey { id: 4685, level: 1 }, // Valakas Tail Stomp.
    SkillKey { id: 4686, level: 1 }, // Valakas Tail Stomp.
    SkillKey { id: 4688, level: 1 }, // Valakas Stun.
    SkillKey { id: 4689, level: 1 }, // Valakas Fear (level 1 used only).
    SkillKey { id: 4690, level: 1 }, // Valakas Meteor Storm.
];

// Status.
const STATUS_DORMANT: i32 = 0; // Spawned, no one inside, entry open.
const STATUS_WAITING: i32 = 1; // Player entered, entry open, cinematic pending.
const STATUS_FIGHTING: i32 = 2; // In combat, entry locked.
const STATUS_DEAD: i32 = 3; // Dead, entry locked.

// Zones: `getZoneById(12010, BossZone.class)` and `getZoneById(13010, NoRestartZone.class)`.
const ZONE_LAIR: (i32, &str) = (12010, "BossZone");
const ZONE_GROUND: (i32, &str) = (13010, "NoRestartZone");

// Locations.
const TELEPORT_CUBE_LOCATIONS: [(i32, i32, i32); 15] = [
    (214880, -116144, -1644),
    (213696, -116592, -1644),
    (212112, -116688, -1644),
    (211184, -115472, -1664),
    (210336, -114592, -1644),
    (211360, -113904, -1644),
    (213152, -112352, -1644),
    (214032, -113232, -1644),
    (214752, -114592, -1644),
    (209824, -115568, -1421),
    (210528, -112192, -1403),
    (213120, -111136, -1408),
    (215184, -111504, -1392),
    (215456, -117328, -1392),
    (213200, -118160, -1424),
];
const LOC_VALAKAS_HIDE: (i32, i32, i32) = (220963, -104895, -1620);
const LOC_EJECT_ATTACKER: (i32, i32, i32) = (150037, -57255, -2976);
const LOC_VALAKAS_LAIR: (i32, i32, i32) = (212852, -114842, -1632);
const LOC_VALAKAS_REGEN: (i32, i32, i32) = (-105200, -253104, -15264);

// Events.
const EVT_START: &str = "VALAKAS_START_ENCOUNTER";
const EVT_REGEN_TICK: &str = "VALAKAS_REGEN_TICK";
const EVT_SKILL_TICK: &str = "VALAKAS_SKILL_TICK";
const EVT_BROADCAST_SPAWN: &str = "VALAKAS_BROADCAST_SPAWN";
const EVT_UNLOCK: &str = "VALAKAS_UNLOCK";
const EVT_REMOVE_PLAYERS: &str = "VALAKAS_REMOVE_PLAYERS";
const EVT_SPAWN_STEP_1: &str = "VALAKAS_SPAWN_1";
const EVT_SPAWN_STEP_2: &str = "VALAKAS_SPAWN_2";
const EVT_SPAWN_STEP_3: &str = "VALAKAS_SPAWN_3";
const EVT_SPAWN_STEP_4: &str = "VALAKAS_SPAWN_4";
const EVT_SPAWN_STEP_5: &str = "VALAKAS_SPAWN_5";
const EVT_SPAWN_STEP_6: &str = "VALAKAS_SPAWN_6";
const EVT_SPAWN_STEP_7: &str = "VALAKAS_SPAWN_7";
const EVT_SPAWN_STEP_8: &str = "VALAKAS_SPAWN_8";
const EVT_SPAWN_STEP_9: &str = "VALAKAS_SPAWN_9";
const EVT_SPAWN_STEP_10: &str = "VALAKAS_SPAWN_10";
const EVT_DIE_STEP_1: &str = "VALAKAS_DIE_1";
const EVT_DIE_STEP_2: &str = "VALAKAS_DIE_2";
const EVT_DIE_STEP_3: &str = "VALAKAS_DIE_3";
const EVT_DIE_STEP_4: &str = "VALAKAS_DIE_4";
const EVT_DIE_STEP_5: &str = "VALAKAS_DIE_5";
const EVT_DIE_STEP_6: &str = "VALAKAS_DIE_6";
const EVT_DIE_STEP_7: &str = "VALAKAS_DIE_7";
const EVT_DIE_STEP_8: &str = "VALAKAS_DIE_8";

// Timings.
const MILLIS_PER_HOUR: i64 = 3600000;
const INACTIVITY_TIMEOUT: i64 = 900000; // 15 minutes.
const REGEN_TICK_DELAY: i64 = 60000;
const SKILL_TICK_DELAY: i64 = 2000;
const TELEPORT_CUBE_LIFETIME: i64 = 900000;
const BROADCAST_SPAWN_DELAY: i64 = 100;
const SPAWN_STEP_1_DELAY: i64 = 1700;
const SPAWN_STEP_2_DELAY: i64 = 3200;
const SPAWN_STEP_3_DELAY: i64 = 6500;
const SPAWN_STEP_4_DELAY: i64 = 9400;
const SPAWN_STEP_5_DELAY: i64 = 12100;
const SPAWN_STEP_6_DELAY: i64 = 12430;
const SPAWN_STEP_7_DELAY: i64 = 15430;
const SPAWN_STEP_8_DELAY: i64 = 16830;
const SPAWN_STEP_9_DELAY: i64 = 23530;
const SPAWN_STEP_10_DELAY: i64 = 26000;
const DIE_STEP_1_DELAY: i64 = 300;
const DIE_STEP_2_DELAY: i64 = 600;
const DIE_STEP_3_DELAY: i64 = 3800;
const DIE_STEP_4_DELAY: i64 = 8200;
const DIE_STEP_5_DELAY: i64 = 8700;
const DIE_STEP_6_DELAY: i64 = 13300;
const DIE_STEP_7_DELAY: i64 = 14000;
const DIE_STEP_8_DELAY: i64 = 16500;

/// The fields of the Java class.
#[derive(Debug, Default)]
pub(crate) struct ValakasState {
    /// `_valakas`.
    pub(crate) valakas: Option<ObjectId>,
    /// `_currentTarget` (a player or a summon).
    pub(crate) current_target: Option<ObjectId>,
    pub(crate) last_attack_time: i64,
}

/// Java `ai.bosses.Valakas.Valakas`.
pub struct Valakas;

impl Script for Valakas {
    fn name(&self) -> &'static str {
        "Valakas"
    }

    fn path(&self) -> &'static str {
        "ai/bosses/Valakas"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_attack_id(&[VALAKAS]);
        s.add_kill_id(&[VALAKAS]);
        s.add_spawn_id(&[VALAKAS]);
        s.add_spell_finished_id(&[VALAKAS]);
    }

    /// The constructor's second half.
    fn on_load(&self, ctx: &mut ScriptCtx<'_>) {
        let Some(info) = ctx.world().grand_boss_info(VALAKAS).cloned() else {
            // Java: `getStatSet` answers null and the constructor throws.
            tracing::warn!("Valakas: no grandboss_data row for {VALAKAS}, the script does not start.");
            return;
        };
        let status = ctx.world().grand_boss_status(VALAKAS).unwrap_or(-1);
        if status == STATUS_DEAD {
            let respawn_time = info.respawn_time;
            let remaining = respawn_time - ctx.now_ms();
            if remaining > 0 {
                ctx.start_quest_timer(EVT_UNLOCK, remaining, None, None, false);
            } else if !ctx.world().boss_writes_enabled() {
                // D-54: the respawn of a dead row waits for the other server's alive row.
                ctx.notify_event(EVT_UNLOCK, None, None);
            } else {
                spawn_dormant_valakas(ctx);
            }
        } else {
            // D-54: a waiting or fighting row belongs to the other server's fight.
            if !row_lets_boss_run(ctx, VALAKAS) {
                info!("Valakas: the shared state row says {status}; the AI waits for an alive row (D-54).");
                return;
            }
            let loc_x = info.loc_x;
            let loc_y = info.loc_y;
            let loc_z = info.loc_z;
            let heading = info.heading;
            let current_hp = info.current_hp;
            let current_mp = info.current_mp;
            let Some(valakas) = ctx.add_spawn(VALAKAS, loc_x, loc_y, loc_z, heading, false, 0, 0) else {
                return;
            };
            ctx.state::<ValakasState>().valakas = Some(valakas.object_id);
            ctx.world_mut().add_grand_boss(VALAKAS, valakas.object_id);
            ctx.world_mut()
                .set_current_hp_mp(valakas.object_id, current_hp, current_mp);
            // NOTE(java-quirk): the teleport below spawns Valakas again (`Attackable.onSpawn`:
            // `setWalking()`), so a dormant or waiting Valakas walks after all.
            ctx.world_mut().set_running(valakas.object_id, true);
            if status == STATUS_FIGHTING {
                let now = ctx.now_ms();
                ctx.state::<ValakasState>().last_attack_time = now;
                ctx.start_quest_timer(EVT_REGEN_TICK, REGEN_TICK_DELAY, Some(valakas), None, true);
                ctx.start_quest_timer(EVT_SKILL_TICK, SKILL_TICK_DELAY, Some(valakas), None, true);
            } else {
                tele_npc(ctx, valakas.object_id, LOC_VALAKAS_HIDE);
                ctx.world_mut().set_invul(valakas.object_id, true);
                ctx.world_mut().npc_set_intention_idle(valakas.object_id);
                if status == STATUS_WAITING {
                    let wait_millis =
                        i64::from(ctx.world().boss_settings().grand_boss.valakas_wait_time) * 60000;
                    ctx.start_quest_timer(EVT_START, wait_millis, Some(valakas), None, false);
                }
            }
        }
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        _player: Option<ObjectId>,
    ) -> Option<String> {
        if let Some(npc) = npc {
            match event {
                EVT_START => handle_encounter_start(ctx, npc),
                EVT_REGEN_TICK => handle_regen_tick(ctx, npc),
                EVT_SKILL_TICK => handle_skill_tick(ctx, npc),
                EVT_BROADCAST_SPAWN => broadcast_spawn_intro(ctx, npc),
                EVT_SPAWN_STEP_1 => broadcast_camera(ctx, npc, 1800, 180, -1, 1500, 10000, 0, 0, 1, 0, 0),
                EVT_SPAWN_STEP_2 => broadcast_camera(ctx, npc, 1300, 180, -5, 3000, 10000, 0, -5, 1, 0, 0),
                EVT_SPAWN_STEP_3 => broadcast_camera(ctx, npc, 500, 180, -8, 600, 10000, 0, 60, 1, 0, 0),
                EVT_SPAWN_STEP_4 => broadcast_camera(ctx, npc, 800, 180, -8, 2700, 10000, 0, 30, 1, 0, 0),
                EVT_SPAWN_STEP_5 => broadcast_camera(ctx, npc, 200, 250, 70, 0, 10000, 30, 80, 1, 0, 0),
                EVT_SPAWN_STEP_6 => broadcast_camera(ctx, npc, 1100, 250, 70, 2500, 10000, 30, 80, 1, 0, 0),
                EVT_SPAWN_STEP_7 => broadcast_camera(ctx, npc, 700, 150, 30, 0, 10000, -10, 60, 1, 0, 0),
                EVT_SPAWN_STEP_8 => broadcast_camera(ctx, npc, 1200, 150, 20, 2900, 10000, -10, 30, 1, 0, 0),
                EVT_SPAWN_STEP_9 => broadcast_camera(ctx, npc, 750, 170, -10, 3400, 4000, 10, -15, 1, 0, 0),
                EVT_SPAWN_STEP_10 => handle_spawn_step_10(ctx, npc),
                EVT_DIE_STEP_1 => broadcast_camera(ctx, npc, 2000, 130, -1, 0, 10000, 0, 0, 1, 1, 0),
                EVT_DIE_STEP_2 => broadcast_camera(ctx, npc, 1100, 210, -5, 3000, 10000, -13, 0, 1, 1, 0),
                EVT_DIE_STEP_3 => broadcast_camera(ctx, npc, 1300, 200, -8, 3000, 10000, 0, 15, 1, 1, 0),
                EVT_DIE_STEP_4 => broadcast_camera(ctx, npc, 1000, 190, 0, 500, 10000, 0, 10, 1, 1, 0),
                EVT_DIE_STEP_5 => broadcast_camera(ctx, npc, 1700, 120, 0, 2500, 10000, 12, 40, 1, 1, 0),
                EVT_DIE_STEP_6 => broadcast_camera(ctx, npc, 1700, 20, 0, 700, 10000, 10, 10, 1, 1, 0),
                EVT_DIE_STEP_7 => broadcast_camera(ctx, npc, 1700, 10, 0, 1000, 10000, 20, 70, 1, 1, 0),
                EVT_DIE_STEP_8 => handle_die_step_8(ctx, npc),
                _ => {}
            }
        } else {
            match event {
                EVT_UNLOCK => {
                    // D-54: on the shared database the row decides; read it again first.
                    if !ctx.world().boss_writes_enabled() {
                        ctx.world_mut()
                            .reread_grand_boss_for_script(VALAKAS, self.name(), EVENT_ROW_READ);
                        return None;
                    }
                    spawn_dormant_valakas(ctx);
                }
                EVENT_ROW_READ => {
                    // D-54: the answer of the read `VALAKAS_UNLOCK` asked for.
                    match ctx.world().grand_boss_status(VALAKAS) {
                        Some(STATUS_DORMANT) => spawn_dormant_valakas(ctx),
                        Some(STATUS_DEAD) => {
                            let respawn = ctx.world().grand_boss_info(VALAKAS).map_or(0, |i| i.respawn_time);
                            let remaining = respawn - ctx.now_ms();
                            if remaining > 0 {
                                ctx.start_quest_timer(EVT_UNLOCK, remaining, None, None, false);
                            } else {
                                info!("Valakas: the shared state row says dead; Valakas stays away (D-54).");
                            }
                        }
                        status => {
                            info!(
                                "Valakas: the shared state row says {status:?}; Valakas stays away (D-54)."
                            );
                        }
                    }
                }
                EVT_REMOVE_PLAYERS => oust_players_from_lair(ctx),
                _ => {}
            }
        }

        None
    }

    fn on_spawn(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc) {
        ctx.set_can_return_to_spawn_point(npc, false);
        ctx.set_random_walking(npc, false);
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
        let (zone_id, zone_type) = ZONE_LAIR;
        if ctx.world().zone_by_id(zone_id, zone_type).is_some()
            && !ctx
                .world()
                .is_inside_zone_by_position(zone_id, zone_type, attacker)
        {
            ctx.world_mut().do_die(attacker, Some(attacker));
            return;
        }

        if ctx.world().is_invul(npc.object_id) {
            return;
        }

        if ctx.world().grand_boss_status(VALAKAS) != Some(STATUS_FIGHTING) {
            let (x, y, z) = LOC_EJECT_ATTACKER;
            ctx.world_mut()
                .tele_to_location(attacker, TeleportTarget::at(x, y, z).with_instance(0));
            return;
        }

        if (ctx.world().mount_type_of(attacker) == MountType::Strider)
            && !ctx.world().is_affected_by_skill(attacker, SKILL_ANTI_STRIDER.id)
        {
            ctx.world_mut().set_npc_target(npc.object_id, Some(attacker));
            ctx.world_mut().do_cast(npc.object_id, SKILL_ANTI_STRIDER);
        }

        let now = ctx.now_ms();
        ctx.state::<ValakasState>().last_attack_time = now;
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, _killer: ObjectId, _is_summon: bool) {
        ctx.cancel_quest_timer(EVT_REGEN_TICK, Some(npc), None);
        ctx.cancel_quest_timer(EVT_SKILL_TICK, Some(npc), None);

        let (zone_id, zone_type) = ZONE_LAIR;
        if ctx.world().zone_by_id(zone_id, zone_type).is_some() {
            broadcast_lair(ctx, &PlaySound::with_unknown1(1, "B03_D"));
            broadcast_lair(ctx, &camera(npc, 1200, 20, -10, 0, 13000, 0, 0, 0, 0, 0));
        }

        for (event, delay) in [
            (EVT_DIE_STEP_1, DIE_STEP_1_DELAY),
            (EVT_DIE_STEP_2, DIE_STEP_2_DELAY),
            (EVT_DIE_STEP_3, DIE_STEP_3_DELAY),
            (EVT_DIE_STEP_4, DIE_STEP_4_DELAY),
            (EVT_DIE_STEP_5, DIE_STEP_5_DELAY),
            (EVT_DIE_STEP_6, DIE_STEP_6_DELAY),
            (EVT_DIE_STEP_7, DIE_STEP_7_DELAY),
            (EVT_DIE_STEP_8, DIE_STEP_8_DELAY),
        ] {
            ctx.start_quest_timer(event, delay, Some(npc), None, false);
        }

        ctx.world_mut().set_grand_boss_status(VALAKAS, STATUS_DEAD);

        let grand_boss = &ctx.world().boss_settings().grand_boss;
        let base_interval_millis = i64::from(grand_boss.valakas_spawn_interval) * MILLIS_PER_HOUR;
        let random_range_millis = i64::from(grand_boss.valakas_spawn_random) * MILLIS_PER_HOUR;
        let respawn_delay = base_interval_millis
            + ctx
                .world_mut()
                .rng()
                .get_i64(-random_range_millis, random_range_millis);
        let next_respawn_time = ctx.now_ms() + respawn_delay;
        info!("Valakas will respawn at: {}", date_time_string(next_respawn_time));
        ctx.start_quest_timer(EVT_UNLOCK, respawn_delay, None, None, false);

        if let Some(mut info) = ctx.world().grand_boss_info(VALAKAS).cloned() {
            info.respawn_time = next_respawn_time;
            ctx.world_mut().set_grand_boss_info(VALAKAS, info);
        }
    }

    fn on_spell_finished(
        &self,
        ctx: &mut ScriptCtx<'_>,
        npc: ScriptNpc,
        _player: Option<ObjectId>,
        _skill: SkillKey,
    ) {
        ctx.start_quest_timer(EVT_SKILL_TICK, 1000, Some(npc), None, false);

        let (zone_id, zone_type) = ZONE_GROUND;
        let valakas = ctx.state::<ValakasState>().valakas;
        if ctx.world().zone_by_id(zone_id, zone_type).is_some()
            && !ctx
                .world()
                .is_character_in_zone(zone_id, zone_type, npc.object_id)
            && let Some(valakas) = valakas
        {
            tele_npc(ctx, valakas, LOC_VALAKAS_LAIR);
        }
    }
}

/// Spawns dormant Valakas and initializes boss status to DORMANT.
fn spawn_dormant_valakas(ctx: &mut ScriptCtx<'_>) {
    let (x, y, z) = LOC_VALAKAS_REGEN;
    let valakas = ctx.add_spawn(VALAKAS, x, y, z, 0, false, 0, 0);
    ctx.state::<ValakasState>().valakas = valakas.map(|v| v.object_id);
    if let Some(valakas) = valakas {
        tele_npc(ctx, valakas.object_id, LOC_VALAKAS_HIDE);
        ctx.world_mut().set_invul(valakas.object_id, true);
        ctx.world_mut().set_running(valakas.object_id, true);
        ctx.world_mut().npc_set_intention_idle(valakas.object_id);
        ctx.world_mut().add_grand_boss(VALAKAS, valakas.object_id);
    }
    ctx.world_mut().set_grand_boss_status(VALAKAS, STATUS_DORMANT);
}

/// Starts encounter: moves Valakas into lair, runs intro cinematic and schedules AI tasks.
fn handle_encounter_start(ctx: &mut ScriptCtx<'_>, npc: ScriptNpc) {
    let now = ctx.now_ms();
    ctx.state::<ValakasState>().last_attack_time = now;
    tele_npc(ctx, npc.object_id, LOC_VALAKAS_LAIR);
    for (event, delay) in [
        (EVT_BROADCAST_SPAWN, BROADCAST_SPAWN_DELAY),
        (EVT_SPAWN_STEP_1, SPAWN_STEP_1_DELAY),
        (EVT_SPAWN_STEP_2, SPAWN_STEP_2_DELAY),
        (EVT_SPAWN_STEP_3, SPAWN_STEP_3_DELAY),
        (EVT_SPAWN_STEP_4, SPAWN_STEP_4_DELAY),
        (EVT_SPAWN_STEP_5, SPAWN_STEP_5_DELAY),
        (EVT_SPAWN_STEP_6, SPAWN_STEP_6_DELAY),
        (EVT_SPAWN_STEP_7, SPAWN_STEP_7_DELAY),
        (EVT_SPAWN_STEP_8, SPAWN_STEP_8_DELAY),
        (EVT_SPAWN_STEP_9, SPAWN_STEP_9_DELAY),
        (EVT_SPAWN_STEP_10, SPAWN_STEP_10_DELAY),
    ] {
        ctx.start_quest_timer(event, delay, Some(npc), None, false);
    }
}

/// Regeneration task and inactivity handling.
fn handle_regen_tick(ctx: &mut ScriptCtx<'_>, npc: ScriptNpc) {
    let id = npc.object_id;
    let last_attack_time = ctx.state::<ValakasState>().last_attack_time;
    if (ctx.world().grand_boss_status(VALAKAS) == Some(STATUS_FIGHTING))
        && ((last_attack_time + INACTIVITY_TIMEOUT) < ctx.now_ms())
    {
        ctx.world_mut().npc_set_intention_idle(id);
        tele_npc(ctx, id, LOC_VALAKAS_REGEN);
        ctx.world_mut().set_grand_boss_status(VALAKAS, STATUS_DORMANT);
        let (_, max_hp) = ctx.world().hp_and_max(id).unwrap_or((0.0, 0.0));
        let (_, max_mp) = ctx.world().mp_and_max(id).unwrap_or((0.0, 0.0));
        ctx.world_mut().set_current_hp_mp(id, max_hp, max_mp);
        let (zone_id, zone_type) = ZONE_LAIR;
        if ctx.world().zone_by_id(zone_id, zone_type).is_some() {
            ctx.world_mut().oust_all_boss_zone_players(zone_id);
        }
        ctx.cancel_quest_timer(EVT_REGEN_TICK, Some(npc), None);
        ctx.cancel_quest_timer(EVT_SKILL_TICK, Some(npc), None);
        return;
    }

    let current_regen_level = ctx
        .world()
        .effect_list(id)
        .and_then(|list| list.info_by_skill_id(SKILL_REGEN_ID))
        .map_or(0, |info| info.skill().level);
    let (current_hp, max_hp) = ctx.world().hp_and_max(id).unwrap_or((0.0, 0.0));
    let level = if (current_hp < (max_hp / 4.0)) && (current_regen_level != 4) {
        4
    } else if (current_hp < ((max_hp * 2.0) / 4.0)) && (current_regen_level != 3) {
        3
    } else if (current_hp < ((max_hp * 3.0) / 4.0)) && (current_regen_level != 2) {
        2
    } else if current_regen_level != 1 {
        1
    } else {
        return;
    };
    ctx.world_mut().set_npc_target(id, Some(id));
    ctx.world_mut().do_cast(id, SkillKey::new(SKILL_REGEN_ID, level));
}

/// Schedules skill AI evaluation.
fn handle_skill_tick(ctx: &mut ScriptCtx<'_>, npc: ScriptNpc) {
    call_skill_ai(ctx, npc);
}

/// Plays spawn sound and social action for all players inside the lair.
fn broadcast_spawn_intro(ctx: &mut ScriptCtx<'_>, npc: ScriptNpc) {
    let (zone_id, zone_type) = ZONE_LAIR;
    if ctx.world().zone_by_id(zone_id, zone_type).is_none() {
        return;
    }

    for player in ctx.world().zone_players_inside(zone_id, zone_type) {
        ctx.world().send(player, &PlaySound::with_unknown1(1, "BS03_A"));
        ctx.world().send(
            player,
            &SocialAction {
                object_id: npc.object_id.0,
                action_id: 3,
            },
        );
    }
}

fn handle_spawn_step_10(ctx: &mut ScriptCtx<'_>, npc: ScriptNpc) {
    ctx.world_mut().set_grand_boss_status(VALAKAS, STATUS_FIGHTING);
    ctx.world_mut().set_invul(npc.object_id, false);
    ctx.start_quest_timer(EVT_REGEN_TICK, REGEN_TICK_DELAY, Some(npc), None, true);
    ctx.start_quest_timer(EVT_SKILL_TICK, SKILL_TICK_DELAY, Some(npc), None, true);

    let (zone_id, zone_type) = ZONE_LAIR;
    if ctx.world().zone_by_id(zone_id, zone_type).is_some()
        && ctx.world().boss_settings().grand_boss.valakas_recognize_hero
    {
        for player in ctx.world().zone_players_inside(zone_id, zone_type) {
            if ctx.world().is_hero(player) {
                let text = format!(
                    "{}!!!! You cannot hope to defeat me with your meager strength.",
                    ctx.player_name(player)
                );
                // `new ExShowScreenMessage(text, 2, 4000)`.
                let message = ExShowScreenMessage {
                    position: 2,
                    ..ExShowScreenMessage::text(&text, 4000)
                };
                broadcast_lair(ctx, &message);
                break;
            }
        }
    }
}

fn handle_die_step_8(ctx: &mut ScriptCtx<'_>, npc: ScriptNpc) {
    broadcast_camera(ctx, npc, 1700, 10, 0, 300, 250, 20, -20, 1, 1, 0);
    for (x, y, z) in TELEPORT_CUBE_LOCATIONS {
        ctx.add_spawn(TELEPORT_CUBE, x, y, z, 0, false, TELEPORT_CUBE_LIFETIME, 0);
    }

    ctx.start_quest_timer(EVT_REMOVE_PLAYERS, TELEPORT_CUBE_LIFETIME, None, None, false);
}

/// Java `broadcastCamera(npc, distance, yaw, pitch, time, duration, unk, relYaw, relPitch, isWide,
/// unk2, unk3)` passes `duration` as the camera's unused `range` and `unk` as its duration; the
/// arguments here are the packet's.
#[allow(clippy::too_many_arguments)]
fn broadcast_camera(
    ctx: &mut ScriptCtx<'_>,
    npc: ScriptNpc,
    distance: i32,
    yaw: i32,
    pitch: i32,
    time: i32,
    duration: i32,
    rel_yaw: i32,
    rel_pitch: i32,
    is_wide: i32,
    rel_angle: i32,
    unk: i32,
) {
    let (zone_id, zone_type) = ZONE_LAIR;
    if ctx.world().zone_by_id(zone_id, zone_type).is_none() {
        return;
    }

    broadcast_lair(
        ctx,
        &camera(
            npc, distance, yaw, pitch, time, duration, rel_yaw, rel_pitch, is_wide, rel_angle, unk,
        ),
    );
}

/// Ejects all players from lair and returns them to town.
fn oust_players_from_lair(ctx: &mut ScriptCtx<'_>) {
    let (zone_id, zone_type) = ZONE_LAIR;
    if ctx.world().zone_by_id(zone_id, zone_type).is_some() {
        ctx.world_mut().oust_all_boss_zone_players(zone_id);
    }
}

/// Core AI: selects targets, chooses skill and handles movement / casting decisions.
fn call_skill_ai(ctx: &mut ScriptCtx<'_>, npc: ScriptNpc) {
    let id = npc.object_id;
    if ctx.world().is_invul(id) || ctx.world().is_casting_now(id) {
        return;
    }

    let current_target = ctx.state::<ValakasState>().current_target;
    let lost = match current_target {
        None => true,
        Some(target) => ctx.world().is_dead(target) || !in_surrounding_region(ctx, id, target),
    };
    if lost || (ctx.get_random(10) == 0) {
        let target = get_random_target(ctx, id);
        ctx.state::<ValakasState>().current_target = target;
    }

    let Some(current_target) = ctx.state::<ValakasState>().current_target else {
        if ctx.get_random(10) == 0 {
            let Some((current_x, current_y, current_z)) = ctx.world().position_of(id) else {
                return;
            };
            let next_x = current_x + ctx.get_random_range(-1400, 1400);
            let next_y = current_y + ctx.get_random_range(-1400, 1400);
            let instance = ctx.world().object(id).map_or(0, |e| e.instance_id);
            if ctx.world().can_move_to_target(
                (current_x, current_y, current_z),
                (next_x, next_y, current_z),
                instance,
            ) {
                ctx.world_mut()
                    .npc_set_intention_move_to(id, next_x, next_y, current_z);
            }
        }
        return;
    };

    let Some(chosen_skill) = get_random_skill(ctx, id) else {
        return;
    };
    // `chosenHolder.getSkill()` is null for a skill the table does not have.
    let Some(base_range) = ctx.world().skill_cast_range(chosen_skill) else {
        return;
    };
    let effective_range = if base_range < 600 { 600 } else { base_range };
    if ctx
        .world()
        .check_if_in_range(effective_range, id, current_target, true)
    {
        ctx.world_mut().npc_set_intention_idle(id);
        ctx.world_mut().set_casting_now(id, true);
        ctx.world_mut().set_npc_target(id, Some(current_target));
        ctx.world_mut().do_cast(id, chosen_skill);
    } else {
        ctx.world_mut().npc_set_intention_follow(id, current_target);
        ctx.world_mut().set_casting_now(id, false);
    }
}

/// Selects a random skill based on HP and nearby players.
fn get_random_skill(ctx: &mut ScriptCtx<'_>, id: ObjectId) -> Option<SkillKey> {
    let (current_hp, max_hp) = ctx.world().hp_and_max(id)?;
    let hp_ratio = current_hp / max_hp;
    let hp_percent = (hp_ratio * 100.0) as i32;
    if (hp_percent < 75)
        && (ctx.get_random(150) == 0)
        && !ctx.world().is_affected_by_skill(id, SKILL_LAVA_SKIN.id)
    {
        return Some(SKILL_LAVA_SKIN);
    }

    let nearby_players = ctx
        .world()
        .visible_objects_in_range(id, 1200)
        .into_iter()
        .filter(|o| ctx.world().player(*o).is_some())
        .count();
    if nearby_players >= 20 {
        return ctx.get_random_entry(&SKILL_TABLE_AOE);
    }

    if hp_percent > 50 {
        return ctx.get_random_entry(&SKILL_TABLE_REGULAR);
    }

    ctx.get_random_entry(&SKILL_TABLE_LOW_HP)
}

/// Selects a random valid playable target in Valakas' surroundings: players and servitors, not
/// pets.
fn get_random_target(ctx: &mut ScriptCtx<'_>, id: ObjectId) -> Option<ObjectId> {
    let candidates: Vec<ObjectId> = ctx
        .world()
        .visible_objects(id)
        .into_iter()
        .filter(|o| {
            let world = ctx.world();
            let playable = world.player(*o).is_some() || world.is_summon(*o);
            playable
                && !world.is_dead(*o)
                && !world.object(*o).is_some_and(|e| e.invisible)
                && !world.is_pet(*o)
        })
        .collect();
    ctx.get_random_entry(&candidates)
}

/// Java `isInSurroundingRegion(other)`: the other's region is one of the 3x3 around this one.
fn in_surrounding_region(ctx: &ScriptCtx<'_>, id: ObjectId, other: ObjectId) -> bool {
    let world = ctx.world();
    match (
        world.object(id).and_then(|e| e.region),
        world.object(other).and_then(|e| e.region),
    ) {
        (Some(mine), Some(theirs)) => theirs.surrounding().contains(&mine),
        _ => false,
    }
}

// ------------------------------------------------------------------ world helpers

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

fn broadcast_lair(ctx: &ScriptCtx<'_>, packet: &impl l2_net::Encode) {
    let (zone_id, zone_type) = ZONE_LAIR;
    ctx.world().broadcast_to_zone(zone_id, zone_type, packet);
}

/// `npc.teleToLocation(new Location(x, y, z))`.
fn tele_npc(ctx: &mut ScriptCtx<'_>, id: ObjectId, (x, y, z): (i32, i32, i32)) {
    ctx.world_mut()
        .tele_to_location(id, TeleportTarget::at(x, y, z).with_instance(0));
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::{BossReadKind, BossReadThen, BossWrite, GrandBossData, ScriptRegistry};

    use super::*;
    use crate::ai::bosses::test_support::*;
    use crate::testing::{ScriptTest, names_to};

    const NPCS: &str = r#"<npc id="29028" type="GrandBoss" level="85" name="Valakas" title="Fire Dragon">
    <stats str="40" int="21" dex="30" wit="20" con="43" men="20">
      <vitals hp="1000" hpRegen="2" mp="500" mpRegen="0.9" />
      <attack physical="8" magical="5" random="30" critical="4" accuracy="4.75" attackSpeed="253" type="SWORD" range="40" distance="80" width="120" />
      <defence physical="44.44444" magical="29.5" />
      <speed><walk ground="20" /><run ground="50" /></speed>
    </stats>
    <collision><radius normal="10"/><height normal="20"/></collision>
    <corpseTime>21</corpseTime>
  </npc>
  <npc id="31759" type="Folk" level="70" name="Teleportation Cubic"/>"#;

    /// A player standing in the lair, allowed in by the boss zone.
    const IN_LAIR: (i32, i32, i32) = (212000, -114000, -1630);

    fn setup_row(status: i32, respawn_time: i64, writes: bool) -> ScriptTest {
        let mut registry = ScriptRegistry::new();
        registry.add(Valakas);
        boss_test(
            registry,
            NPCS,
            "",
            &[],
            vec![row(VALAKAS, status, respawn_time, (213000, -115000, -1630))],
            writes,
        )
    }

    fn has_timer(t: &mut ScriptTest, name: &str, npc: Option<Oid>) -> bool {
        let mut found = false;
        t.world.run_as_script("Valakas", |ctx| {
            let npc = npc.and_then(|n| ctx.script_npc(n));
            found = ctx.has_quest_timer(name, npc, None);
        });
        found
    }

    fn notify(t: &mut ScriptTest, event: &str, npc: Option<Oid>) {
        t.world.run_as_script("Valakas", |ctx| {
            let npc = npc.and_then(|n| ctx.script_npc(n));
            ctx.notify_event(event, npc, None);
        });
    }

    fn status(t: &ScriptTest) -> Option<i32> {
        t.world.grand_boss_status(VALAKAS)
    }

    fn enter_lair(t: &mut ScriptTest, player: i32) {
        t.world.allow_boss_zone_entry(ZONE_LAIR.0, Oid(player), 3600);
        t.enter_with(player_at(player, IN_LAIR, vec![]));
    }

    /// `ValakasTeleporters` let a player in; the waiting time is over; the movie ran.
    fn awake(t: &mut ScriptTest, player: i32) -> Oid {
        enter_lair(t, player);
        let valakas = t.world.grand_boss_npc(VALAKAS).unwrap();
        t.world.set_grand_boss_status(VALAKAS, STATUS_WAITING);
        notify(t, EVT_START, Some(valakas));
        run_ms(t, 26_000);
        valakas
    }

    #[test]
    fn the_dormant_valakas_hides_and_wakes_up_with_its_movie() {
        let mut t = setup_row(STATUS_DORMANT, 0, true);
        let valakas = t.world.grand_boss_npc(VALAKAS).unwrap();
        let (x, y, _) = t.world.position_of(valakas).unwrap();
        assert_eq!((x, y), (LOC_VALAKAS_HIDE.0, LOC_VALAKAS_HIDE.1));
        assert!(t.world.is_invul(valakas));

        enter_lair(&mut t, 1);
        assert!(t.world.zone_players_inside(12010, "BossZone").contains(&Oid(1)));
        t.take();
        t.take_boss();
        t.world.set_grand_boss_status(VALAKAS, STATUS_WAITING);
        notify(&mut t, EVT_START, Some(valakas));
        let (x, y, _) = t.world.position_of(valakas).unwrap();
        assert_eq!((x, y), (LOC_VALAKAS_LAIR.0, LOC_VALAKAS_LAIR.1));
        run_ms(&mut t, 200);
        let names = names_to(&t.take(), 1);
        assert!(names.contains(&"PLAY_SOUND".to_owned()));
        assert!(names.contains(&"SOCIAL_ACTION".to_owned()));
        assert_eq!(status(&t), Some(STATUS_WAITING));

        run_ms(&mut t, 26_000);
        let names = names_to(&t.take(), 1);
        assert_eq!(names.iter().filter(|n| *n == "SPECIAL_CAMERA").count(), 9);
        assert_eq!(status(&t), Some(STATUS_FIGHTING));
        assert!(!t.world.is_invul(valakas));
        assert!(has_timer(&mut t, EVT_REGEN_TICK, Some(valakas)));
        assert!(has_timer(&mut t, EVT_SKILL_TICK, Some(valakas)));
        let status_writes: Vec<i32> = grand_writes(&t.take_boss())
            .into_iter()
            .filter_map(|w| match w {
                BossWrite::GrandBossStatus { status, .. } => Some(status),
                _ => None,
            })
            .collect();
        assert_eq!(status_writes, [STATUS_WAITING, STATUS_FIGHTING]);
    }

    #[test]
    fn the_death_movie_writes_the_row_and_the_cubes_come() {
        let mut t = setup_row(STATUS_DORMANT, 0, true);
        let valakas = awake(&mut t, 1);
        t.take();
        t.take_boss();
        let before = now_ms();
        assert!(t.world.do_die(valakas, Some(Oid(1))));
        // `onKillDelay`: 2.5 seconds.
        run_ms(&mut t, 2_600);
        assert_eq!(status(&t), Some(STATUS_DEAD));
        assert!(!has_timer(&mut t, EVT_REGEN_TICK, Some(valakas)));
        assert!(has_timer(&mut t, EVT_UNLOCK, None));
        let writes = grand_writes(&t.take_boss());
        assert_eq!(
            writes[0],
            BossWrite::GrandBossStatus {
                boss_id: VALAKAS,
                status: STATUS_DEAD
            }
        );
        // `setStatSet` writes the whole row with the new respawn time.
        let BossWrite::GrandBoss(GrandBossData {
            respawn_time, status, ..
        }) = writes[1].clone()
        else {
            panic!("{writes:?}");
        };
        assert_eq!(status, STATUS_DEAD);
        let hour = 3_600_000;
        assert!(respawn_time >= before + 192 * hour && respawn_time <= now_ms() + 336 * hour);
        assert_eq!(
            t.world.grand_boss_info(VALAKAS).unwrap().respawn_time,
            respawn_time
        );

        run_ms(&mut t, 17_000);
        let names = names_to(&t.take(), 1);
        assert_eq!(names.iter().filter(|n| *n == "SPECIAL_CAMERA").count(), 1 + 8);
        assert_eq!(npcs_of(&t, TELEPORT_CUBE).len(), 15);
        assert!(has_timer(&mut t, EVT_REMOVE_PLAYERS, None));
    }

    #[test]
    fn with_the_switch_off_nothing_is_written_and_the_row_decides_the_unlock() {
        let mut t = setup_row(STATUS_DORMANT, 0, false);
        let valakas = awake(&mut t, 1);
        assert_eq!(status(&t), Some(STATUS_FIGHTING));
        assert!(t.world.do_die(valakas, Some(Oid(1))));
        run_ms(&mut t, 2_600);
        assert_eq!(status(&t), Some(STATUS_DEAD));
        assert!(grand_writes(&t.take_boss()).is_empty());

        notify(&mut t, EVT_UNLOCK, None);
        let then = BossReadThen::ScriptEvent {
            script: "Valakas",
            event: EVENT_ROW_READ,
        };
        assert_eq!(
            rereads(&t.take_boss()),
            [(BossReadKind::GrandBoss(VALAKAS), then)]
        );
        assert!(npcs_of(&t, VALAKAS).is_empty());
        answer(&mut t, then, VALAKAS, row(VALAKAS, STATUS_DORMANT, 0, (1, 2, 3)));
        assert_eq!(status(&t), Some(STATUS_DORMANT));
        assert_eq!(npcs_of(&t, VALAKAS).len(), 1);
        assert!(grand_writes(&t.take_boss()).is_empty());
    }

    #[test]
    fn fifteen_minutes_without_a_hit_send_valakas_back() {
        let mut t = setup_row(STATUS_DORMANT, 0, true);
        let valakas = awake(&mut t, 1);
        t.world.run_as_script("Valakas", |ctx| {
            ctx.state::<ValakasState>().last_attack_time = 0;
        });
        t.take_boss();
        notify(&mut t, EVT_REGEN_TICK, Some(valakas));
        assert_eq!(status(&t), Some(STATUS_DORMANT));
        let (x, y, _) = t.world.position_of(valakas).unwrap();
        assert_eq!((x, y), (LOC_VALAKAS_REGEN.0, LOC_VALAKAS_REGEN.1));
        assert!(!has_timer(&mut t, EVT_REGEN_TICK, Some(valakas)));
        assert!(!has_timer(&mut t, EVT_SKILL_TICK, Some(valakas)));
        // `oustAllPlayers` forgets the players the zone let in.
        assert_eq!(t.world.boss_zone_allowed(12010), Some(&[][..]));
    }

    #[test]
    fn an_attack_from_outside_the_lair_kills_the_attacker() {
        let mut t = setup_row(STATUS_DORMANT, 0, true);
        let valakas = t.world.grand_boss_npc(VALAKAS).unwrap();
        // Outside the lair.
        t.enter(2, 80, vec![]);
        // Inside, while Valakas still sleeps (invulnerable): nothing.
        enter_lair(&mut t, 3);
        let attack = |t: &mut ScriptTest, attacker: i32| {
            t.world.run_as_script("Valakas", |ctx| {
                let npc = ctx.script_npc(valakas).unwrap();
                Valakas.on_attack(ctx, npc, Oid(attacker), 10, false, None);
            });
        };
        attack(&mut t, 2);
        assert!(t.world.is_dead(Oid(2)));
        attack(&mut t, 3);
        assert!(!t.world.is_dead(Oid(3)));
        assert_eq!(
            t.world.position_of(Oid(3)).map(|(x, y, _)| (x, y)),
            Some((IN_LAIR.0, IN_LAIR.1))
        );
        // Not invulnerable but not fighting: the attacker is sent away.
        t.world.set_invul(valakas, false);
        attack(&mut t, 3);
        let (x, y, _) = t.world.position_of(Oid(3)).unwrap();
        assert_eq!((x, y), (LOC_EJECT_ATTACKER.0, LOC_EJECT_ATTACKER.1));
    }

    #[test]
    fn the_constructor_restores_the_row() {
        // Fighting, this server writes: Valakas fights on where it stood.
        let mut t = setup_row(STATUS_FIGHTING, 0, true);
        let valakas = t.world.grand_boss_npc(VALAKAS).unwrap();
        let (x, y, _) = t.world.position_of(valakas).unwrap();
        assert_eq!((x, y), (213000, -115000));
        assert!(!t.world.is_invul(valakas));
        assert!(has_timer(&mut t, EVT_REGEN_TICK, Some(valakas)));
        // Waiting: hidden, and the encounter timer runs.
        let mut t = setup_row(STATUS_WAITING, 0, true);
        let valakas = t.world.grand_boss_npc(VALAKAS).unwrap();
        assert!(t.world.is_invul(valakas));
        assert!(has_timer(&mut t, EVT_START, Some(valakas)));
        // D-54: fighting or waiting on the shared database is the other server's fight.
        for status in [STATUS_FIGHTING, STATUS_WAITING] {
            let t = setup_row(status, 0, false);
            assert!(npcs_of(&t, VALAKAS).is_empty());
        }
        // Dead with the respawn time over.
        let t = setup_row(STATUS_DEAD, now_ms() - 1000, true);
        assert_eq!(npcs_of(&t, VALAKAS).len(), 1);
        assert_eq!(status(&t), Some(STATUS_DORMANT));
        // `spawnDormantValakas` runs after the teleport, so it stays.
        assert!(t.world.is_running(npcs_of(&t, VALAKAS)[0]));
        let t = setup_row(STATUS_DEAD, now_ms() - 1000, false);
        assert!(npcs_of(&t, VALAKAS).is_empty());
        assert_eq!(rereads(&t.take_boss()).len(), 1);
        let mut t = setup_row(STATUS_DEAD, now_ms() + 3_600_000, true);
        assert!(npcs_of(&t, VALAKAS).is_empty());
        assert!(has_timer(&mut t, EVT_UNLOCK, None));
    }
}
