//! Doctor Chaos and the Chaos Golem: the paranoia countdown that the visitors run down, the
//! transformation cinematic, the golem's 30 minute leash and the 36 +- 24 hour respawn.
// Java: dist/game/data/scripts/ai/bosses/DrChaos/DrChaos.java
//
// The Java constructor is `on_load`; the instance fields are `DrChaosState` (`ctx.state`). The
// status of the grand boss row 25512: 0 normal (the doctor), 1 crazy (the golem), 2 dead.
//
// Side by side (D-54, `super` module): with `CycleDatabaseWrites` off the constructor starts the
// doctor only from a row that says 0, and `reset_drchaos` reads the row again and brings him back
// only when it says 0.
//
// Not modelled: nothing.

use l2_protocol_game::server::{ChatType, PlaySound, SocialAction, SpecialCamera};
use l2_util::ObjectId;
use l2_world::{GrandBossInfo, Script, ScriptCtx, ScriptNpc, ScriptSetup, TeleportTarget};
use tracing::{info, warn};

use super::{EVENT_ROW_READ, row_lets_boss_run};

// NPC identifiers.
const DOCTOR_CHAOS_NPC_ID: i32 = 32033;
const CHAOS_GOLEM_NPC_ID: i32 = 25512;

// Boss states.
const STATUS_NORMAL: i32 = 0; // Dr. Chaos is in NPC form.
const STATUS_CRAZY: i32 = 1; // Dr. Chaos is in Chaos Golem form.
const STATUS_DEAD: i32 = 2; // Dr. Chaos is dead and waiting for respawn.

// Locations.
const CHAOS_SPAWN_LOCATION: (i32, i32, i32, i32) = (96320, -110912, -3328, 8191);
const CHAOS_RETURN_LOCATION: (i32, i32, i32) = (96320, -110912, -3328);
const CHAOS_MOVE_LOCATION: (i32, i32, i32) = (95928, -110671, -3340);
const CHAOS_BOX_LOCATION: (i32, i32, i32) = (96323, -110914, -3328);
const CHAOS_GOLEM_SPAWN_LOCATION: (i32, i32, i32) = (96080, -110822, -3343);

// Timers and delays.
const GOLEM_DESPAWN_CHECK_DELAY_MS: i64 = 60_000;
const GOLEM_INACTIVITY_LIMIT_MS: i64 = 1_800_000; // 30 minutes.
const PARANOIA_TICK_DELAY_MS: i64 = 1000;
const DISTANCE_CHECK_DELAY_MS: i64 = 10_000;

// Paranoia configuration.
const PARANOIA_INITIAL_TIME_SECONDS: i32 = 30;
const PARANOIA_FIRST_MESSAGE_THRESHOLD: i32 = 15;
const PARANOIA_SECOND_MESSAGE_UPPER_BOUND: i32 = 20;
const PARANOIA_THIRD_MESSAGE_UPPER_BOUND: i32 = 10;
const PARANOIA_RANGE: i32 = 500;

// Distance limits.
const CHAOS_RETURN_DISTANCE: f64 = 2000.0;
const DISTANCE_CHECK_LIMIT: f64 = 10_000.0;

// Respawn configuration.
const BASE_RESPAWN_HOURS: i32 = 36;
const RANDOM_RESPAWN_VARIATION_HOURS: i32 = 24;
const MILLIS_PER_HOUR: i64 = 3_600_000;

// Events.
const EVENT_RESET_DRCHAOS: &str = "reset_drchaos";
const EVENT_GOLEM_DESPAWN: &str = "golem_despawn";
const EVENT_PARANOIA_ACTIVITY: &str = "paranoia_activity";
const EVENT_DISTANCE_CHECK: &str = "DISTANCE_CHECK";

/// The instance fields of the Java class.
#[derive(Debug, Default)]
pub(crate) struct DrChaosState {
    /// `_lastGolemAttackTime`.
    pub(crate) last_golem_attack_time: i64,
    /// `_paranoiaCountdown`.
    pub(crate) paranoia_countdown: i32,
}

/// Java `ai.bosses.DrChaos.DrChaos`.
pub struct DrChaos;

impl Script for DrChaos {
    fn name(&self) -> &'static str {
        "DrChaos"
    }

    fn path(&self) -> &'static str {
        "ai/bosses/DrChaos"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_first_talk_id(&[DOCTOR_CHAOS_NPC_ID]);
        s.add_spawn_id(&[DOCTOR_CHAOS_NPC_ID]);
        s.add_kill_id(&[CHAOS_GOLEM_NPC_ID]);
        s.add_attack_id(&[CHAOS_GOLEM_NPC_ID]);
    }

    /// The rest of the constructor: the boss row decides what comes back.
    fn on_load(&self, ctx: &mut ScriptCtx<'_>) {
        let info_row = ctx.world().grand_boss_info(CHAOS_GOLEM_NPC_ID).cloned();
        // Java unboxes the status and throws for a boss without a row.
        let Some(status) = ctx.world().grand_boss_status(CHAOS_GOLEM_NPC_ID) else {
            warn!("DrChaos: no grandboss_data row for {CHAOS_GOLEM_NPC_ID}, the script does not start.");
            return;
        };

        if status == STATUS_DEAD {
            if let Some(info_row) = info_row {
                let remaining = info_row.respawn_time - ctx.now_ms();
                if remaining > 0 {
                    ctx.start_quest_timer(EVENT_RESET_DRCHAOS, remaining, None, None, false);
                } else if !ctx.world().boss_writes_enabled() {
                    // D-54: the respawn of a dead row waits for the other server's alive row.
                    ctx.notify_event(EVENT_RESET_DRCHAOS, None, None);
                } else {
                    spawn_doctor_and_reset(ctx);
                }
            } else {
                // Missing StatSet for DEAD status, fallback to NORMAL spawn.
                spawn_doctor_and_reset(ctx);
            }
        } else if status == STATUS_CRAZY {
            // D-54: with the switch off a row that is not alive is the other server's.
            if !row_lets_boss_run(ctx, CHAOS_GOLEM_NPC_ID) {
                info!("DrChaos: the shared state row says {status}; the AI waits for an alive row (D-54).");
                return;
            }
            if let Some(info_row) = info_row {
                let golem = ctx.add_spawn(
                    CHAOS_GOLEM_NPC_ID,
                    info_row.loc_x,
                    info_row.loc_y,
                    info_row.loc_z,
                    info_row.heading,
                    false,
                    0,
                    0,
                );
                if let Some(golem) = golem {
                    ctx.world_mut()
                        .add_grand_boss(CHAOS_GOLEM_NPC_ID, golem.object_id);
                    ctx.world_mut().set_current_hp_mp(
                        golem.object_id,
                        info_row.current_hp,
                        info_row.current_mp,
                    );
                    ctx.set_running(golem);

                    ctx.state::<DrChaosState>().last_golem_attack_time = ctx.now_ms();
                    ctx.start_quest_timer(
                        EVENT_GOLEM_DESPAWN,
                        GOLEM_DESPAWN_CHECK_DELAY_MS,
                        Some(golem),
                        None,
                        true,
                    );
                }
            } else {
                // Missing StatSet for CRAZY status, fallback to NORMAL spawn.
                spawn_doctor_and_reset(ctx);
            }
        } else {
            if !row_lets_boss_run(ctx, CHAOS_GOLEM_NPC_ID) {
                info!("DrChaos: the shared state row says {status}; the AI waits for an alive row (D-54).");
                return;
            }
            spawn_doctor(ctx);
        }
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        _player: Option<ObjectId>,
    ) -> Option<String> {
        match event {
            EVENT_RESET_DRCHAOS => {
                // D-54: on the shared database the row decides; read it again first.
                if !ctx.world().boss_writes_enabled() {
                    ctx.world_mut().reread_grand_boss_for_script(
                        CHAOS_GOLEM_NPC_ID,
                        self.name(),
                        EVENT_ROW_READ,
                    );
                    return None;
                }
                spawn_doctor_and_reset(ctx);
            }
            EVENT_ROW_READ => {
                // D-54: the answer of the read `reset_drchaos` asked for.
                match ctx.world().grand_boss_status(CHAOS_GOLEM_NPC_ID) {
                    Some(STATUS_NORMAL) => {
                        spawn_doctor(ctx);
                    }
                    Some(STATUS_DEAD) => {
                        let respawn = ctx
                            .world()
                            .grand_boss_info(CHAOS_GOLEM_NPC_ID)
                            .map_or(0, |i| i.respawn_time);
                        let remaining = respawn - ctx.now_ms();
                        if remaining > 0 {
                            ctx.start_quest_timer(EVENT_RESET_DRCHAOS, remaining, None, None, false);
                        } else {
                            info!("DrChaos: the shared state row says dead; the doctor stays away (D-54).");
                        }
                    }
                    status => {
                        info!("DrChaos: the shared state row says {status:?}; the doctor stays away (D-54).")
                    }
                }
            }
            EVENT_GOLEM_DESPAWN => {
                // Despawn the Chaos Golem after 30 minutes of inactivity since last hit.
                if let Some(npc) = npc
                    && npc.id == CHAOS_GOLEM_NPC_ID
                    && (ctx.state::<DrChaosState>().last_golem_attack_time + GOLEM_INACTIVITY_LIMIT_MS)
                        < ctx.now_ms()
                {
                    let chaos = spawn_doctor_npc(ctx);
                    ctx.world_mut()
                        .set_grand_boss_status(CHAOS_GOLEM_NPC_ID, STATUS_NORMAL);

                    // NOTE(java-quirk): the timer is cancelled for the new doctor, not for the
                    // golem it was started with; the golem's own timer ends with `deleteMe`.
                    ctx.cancel_quest_timer(EVENT_GOLEM_DESPAWN, chaos, None);
                    ctx.cancel_quest_timers(EVENT_DISTANCE_CHECK);

                    ctx.delete_npc(npc);
                }
            }
            "1" => {
                let npc = npc?;
                broadcast(ctx, npc, &social(npc, 2));
                broadcast(ctx, npc, &camera(npc, 1, -200, 15, 5500, 13500));
            }
            "2" => {
                let npc = npc?;
                broadcast(ctx, npc, &social(npc, 3));
            }
            "3" => {
                let npc = npc?;
                broadcast(ctx, npc, &social(npc, 1));
            }
            "4" => {
                let npc = npc?;
                broadcast(ctx, npc, &camera(npc, 1, -150, 10, 3500, 5000));
                let (x, y, z) = CHAOS_MOVE_LOCATION;
                ctx.npc_move_to(npc, x, y, z);
            }
            "5" => {
                let npc = npc?;
                let (x, y, z) = CHAOS_GOLEM_SPAWN_LOCATION;
                let golem = ctx.add_spawn(CHAOS_GOLEM_NPC_ID, x, y, z, 0, false, 0, 0);
                if let Some(golem) = golem {
                    ctx.world_mut()
                        .add_grand_boss(CHAOS_GOLEM_NPC_ID, golem.object_id);
                    ctx.start_quest_timer(
                        EVENT_DISTANCE_CHECK,
                        DISTANCE_CHECK_DELAY_MS,
                        Some(golem),
                        None,
                        true,
                    );
                    broadcast(ctx, golem, &camera(npc, 30, 200, 20, 6000, 8000));
                    broadcast(ctx, golem, &social(npc, 1));
                    broadcast(
                        ctx,
                        golem,
                        &PlaySound {
                            unknown1: 1,
                            sound_file: "Rm03_A".to_owned(),
                            unknown3: 0,
                            unknown4: 0,
                            unknown5: 0,
                            unknown6: 0,
                            unknown7: 0,
                            unknown8: 0,
                        },
                    );

                    ctx.state::<DrChaosState>().last_golem_attack_time = ctx.now_ms();
                    ctx.start_quest_timer(
                        EVENT_GOLEM_DESPAWN,
                        GOLEM_DESPAWN_CHECK_DELAY_MS,
                        Some(golem),
                        None,
                        true,
                    );
                }

                ctx.delete_npc(npc);
            }
            EVENT_PARANOIA_ACTIVITY => {
                let npc = npc?;
                if ctx.world().grand_boss_status(CHAOS_GOLEM_NPC_ID) == Some(STATUS_NORMAL) {
                    let nearby: Vec<ObjectId> = ctx
                        .world()
                        .visible_objects_in_range(npc.object_id, PARANOIA_RANGE)
                        .into_iter()
                        .filter(|o| ctx.world().player(*o).is_some())
                        .collect();
                    for nearby_player in nearby {
                        if ctx.is_dead(nearby_player) {
                            continue;
                        }

                        ctx.state::<DrChaosState>().paranoia_countdown -= 1;
                        let countdown = ctx.state::<DrChaosState>().paranoia_countdown;

                        if countdown == PARANOIA_FIRST_MESSAGE_THRESHOLD {
                            ctx.broadcast_npc_say_text(
                                npc,
                                ChatType::NpcGeneral,
                                "How dare you trespass into my territory! Have you no fear?",
                            );
                        }

                        if countdown <= 0 {
                            crazy_midget_becomes_angry(ctx, npc);
                        }
                    }
                }

                let (rx, ry, rz) = CHAOS_RETURN_LOCATION;
                if distance_2d_to(ctx, npc.object_id, (rx, ry)) > CHAOS_RETURN_DISTANCE {
                    if ctx.world().is_attackable_npc(npc.object_id) {
                        ctx.world_mut().clear_aggro_list(npc.object_id);
                    }
                    ctx.npc_move_to(npc, rx, ry, rz);
                }
            }
            EVENT_DISTANCE_CHECK => {
                if npc.is_none_or(|n| ctx.is_dead(n.object_id)) {
                    ctx.cancel_quest_timers(EVENT_DISTANCE_CHECK);
                } else if let Some(npc) = npc
                    && let Some((sx, sy, sz)) = ctx.npc_spawn_point(npc)
                    && distance_2d_to(ctx, npc.object_id, (sx, sy)) > DISTANCE_CHECK_LIMIT
                {
                    if ctx.world().is_attackable_npc(npc.object_id) {
                        ctx.world_mut().clear_aggro_list(npc.object_id);
                    }
                    ctx.world_mut()
                        .tele_to_location(npc.object_id, TeleportTarget::at(sx, sy, sz).with_instance(0));
                }
            }
            _ => {}
        }

        None
    }

    /// Advances the doctor's paranoia and his dialog.
    fn on_first_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, _player: ObjectId) -> Option<String> {
        let mut html_text = String::new();
        if ctx.world().grand_boss_status(CHAOS_GOLEM_NPC_ID) == Some(STATUS_NORMAL) {
            let step = 1 + ctx.get_random(5);
            ctx.state::<DrChaosState>().paranoia_countdown -= step;
            let countdown = ctx.state::<DrChaosState>().paranoia_countdown;

            if countdown > PARANOIA_SECOND_MESSAGE_UPPER_BOUND && countdown <= PARANOIA_INITIAL_TIME_SECONDS {
                html_text = "<html><body>Doctor Chaos:<br>What?! Who are you? How did you come here?<br>You really look suspicious... Aren't those filthy members of Black Anvil guild send you? No? Mhhhhh... I don't trust you!</body></html>".to_owned();
            } else if countdown > PARANOIA_THIRD_MESSAGE_UPPER_BOUND
                && countdown <= PARANOIA_SECOND_MESSAGE_UPPER_BOUND
            {
                html_text = "<html><body>Doctor Chaos:<br>Why are you standing here? Don't you see it's a private propertie? Don't look at him with those eyes... Did you smile?! Don't make fun of me! He will ... destroy ... you ... if you continue!</body></html>".to_owned();
            } else if countdown > 0 && countdown <= PARANOIA_THIRD_MESSAGE_UPPER_BOUND {
                html_text = "<html><body>Doctor Chaos:<br>I know why you are here, traitor! He discovered your plans! You are assassin ... sent by the Black Anvil guild! But you won't kill the Emperor of Evil!</body></html>".to_owned();
            } else if countdown <= 0 {
                crazy_midget_becomes_angry(ctx, npc);
            }
        }

        Some(html_text)
    }

    /// Initializes the paranoia countdown and the distance checks when Dr. Chaos spawns.
    fn on_spawn(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc) {
        ctx.state::<DrChaosState>().paranoia_countdown = PARANOIA_INITIAL_TIME_SECONDS;

        ctx.start_quest_timer(
            EVENT_PARANOIA_ACTIVITY,
            PARANOIA_TICK_DELAY_MS,
            Some(npc),
            None,
            true,
        );

        ctx.cancel_quest_timer(EVENT_DISTANCE_CHECK, Some(npc), None);
        ctx.start_quest_timer(
            EVENT_DISTANCE_CHECK,
            DISTANCE_CHECK_DELAY_MS,
            Some(npc),
            None,
            true,
        );
    }

    /// Chaos Golem death: the doctor's respawn is scheduled and stored.
    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, _killer: ObjectId, _is_summon: bool) {
        ctx.cancel_quest_timer(EVENT_GOLEM_DESPAWN, Some(npc), None);
        ctx.broadcast_npc_say_text(
            npc,
            ChatType::NpcGeneral,
            "Urggh! You will pay dearly for this insult.",
        );

        let respawn_time = i64::from(
            BASE_RESPAWN_HOURS
                + ctx.get_random_range(-RANDOM_RESPAWN_VARIATION_HOURS, RANDOM_RESPAWN_VARIATION_HOURS),
        ) * MILLIS_PER_HOUR;

        ctx.world_mut()
            .set_grand_boss_status(CHAOS_GOLEM_NPC_ID, STATUS_DEAD);
        ctx.start_quest_timer(EVENT_RESET_DRCHAOS, respawn_time, None, None, false);

        // `info == null` makes Java a new, empty StatSet.
        let mut info_row = ctx
            .world()
            .grand_boss_info(CHAOS_GOLEM_NPC_ID)
            .cloned()
            .unwrap_or(GrandBossInfo {
                loc_x: 0,
                loc_y: 0,
                loc_z: 0,
                heading: 0,
                respawn_time: 0,
                current_hp: 0.0,
                current_mp: 0.0,
            });
        info_row.respawn_time = ctx.now_ms() + respawn_time;
        ctx.world_mut().set_grand_boss_info(CHAOS_GOLEM_NPC_ID, info_row);

        ctx.cancel_quest_timers(EVENT_DISTANCE_CHECK);
    }

    /// Chaos Golem attacks: random taunts in the fight and the inactivity clock.
    fn on_attack(
        &self,
        ctx: &mut ScriptCtx<'_>,
        npc: ScriptNpc,
        _attacker: ObjectId,
        _damage: i32,
        _is_summon: bool,
        _skill: Option<l2_world::SkillKey>,
    ) {
        ctx.state::<DrChaosState>().last_golem_attack_time = ctx.now_ms();

        let chance = ctx.get_random(300);
        if chance < 3 {
            let message = match chance {
                0 => "Bwah-ha-ha! Your doom is at hand! Behold the Ultra Secret Super Weapon!",
                1 => "Foolish, insignificant creatures! How dare you challenge me!",
                _ => "I see that none will challenge me now!",
            };

            ctx.broadcast_npc_say_text(npc, ChatType::NpcGeneral, message);
        }
    }
}

/// `addSpawn(DOCTOR_CHAOS_NPC_ID, CHAOS_SPAWN_LOCATION, false, 0, false)`.
fn spawn_doctor_npc(ctx: &mut ScriptCtx<'_>) -> Option<ScriptNpc> {
    let (x, y, z, heading) = CHAOS_SPAWN_LOCATION;
    ctx.add_spawn(DOCTOR_CHAOS_NPC_ID, x, y, z, heading, false, 0, 0)
}

fn spawn_doctor(ctx: &mut ScriptCtx<'_>) {
    spawn_doctor_npc(ctx);
}

/// The doctor comes back and the row says so: `setStatus(CHAOS_GOLEM_NPC_ID, STATUS_NORMAL)`.
fn spawn_doctor_and_reset(ctx: &mut ScriptCtx<'_>) {
    spawn_doctor(ctx);
    ctx.world_mut()
        .set_grand_boss_status(CHAOS_GOLEM_NPC_ID, STATUS_NORMAL);
}

/// Launches the cinematic and turns Dr. Chaos into his Chaos Golem form.
fn crazy_midget_becomes_angry(ctx: &mut ScriptCtx<'_>, npc: ScriptNpc) {
    if ctx.world().grand_boss_status(CHAOS_GOLEM_NPC_ID) == Some(STATUS_NORMAL) {
        ctx.world_mut()
            .set_grand_boss_status(CHAOS_GOLEM_NPC_ID, STATUS_CRAZY);

        ctx.cancel_quest_timer(EVENT_PARANOIA_ACTIVITY, Some(npc), None);

        let (x, y, z) = CHAOS_BOX_LOCATION;
        ctx.npc_move_to(npc, x, y, z);
        ctx.broadcast_npc_say_text(
            npc,
            ChatType::NpcGeneral,
            "Fools! Why haven't you fled yet? Prepare to learn a lesson!",
        );

        ctx.start_quest_timer("1", 2000, Some(npc), None, false);
        ctx.start_quest_timer("2", 4000, Some(npc), None, false);
        ctx.start_quest_timer("3", 6500, Some(npc), None, false);
        ctx.start_quest_timer("4", 12_500, Some(npc), None, false);
        ctx.start_quest_timer("5", 17_000, Some(npc), None, false);
    }
}

/// `npc.broadcastPacket(packet)`.
fn broadcast(ctx: &ScriptCtx<'_>, npc: ScriptNpc, packet: &impl l2_net::Encode) {
    ctx.world().broadcast_packet(npc.object_id, packet);
}

/// `new SocialAction(npc.getObjectId(), id)`.
fn social(npc: ScriptNpc, action_id: i32) -> SocialAction {
    SocialAction {
        object_id: npc.object_id.0,
        action_id,
    }
}

/// `new SpecialCamera(npc, force, angle1, angle2, time, range, duration, 0, 0, 0, 0, 0)`; the
/// `range` is not in the packet.
fn camera(npc: ScriptNpc, force: i32, angle1: i32, angle2: i32, time: i32, duration: i32) -> SpecialCamera {
    SpecialCamera::new(
        npc.object_id.0,
        force,
        angle1,
        angle2,
        time,
        duration,
        0,
        0,
        0,
        0,
        0,
    )
}

/// Java `calculateDistance2D(x, y)`: `f64::MAX` when the NPC is not in the world.
fn distance_2d_to(ctx: &ScriptCtx<'_>, a: ObjectId, (x, y): (i32, i32)) -> f64 {
    let Some((ax, ay, _)) = ctx.world().position_of(a) else {
        return f64::MAX;
    };
    let (dx, dy) = (f64::from(ax - x), f64::from(ay - y));
    (dx * dx + dy * dy).sqrt()
}

#[cfg(test)]
mod tests {
    use l2_world::{BossReadKind, BossReadThen, BossWrite, ScriptRegistry};

    use super::*;
    use crate::ai::bosses::test_support::*;
    use crate::testing::ScriptTest;

    const STATS: &str = r#"<stats str="40" int="21" dex="30" wit="20" con="43" men="20"><vitals hp="1000" hpRegen="2" mp="500" mpRegen="0.9" /><attack physical="8" magical="5" random="30" critical="4" accuracy="4.75" attackSpeed="253" type="SWORD" range="40" distance="80" width="120" /><defence physical="44.44444" magical="29.5" /><speed><walk ground="20" /><run ground="50" /></speed></stats><collision><radius normal="10"/><height normal="20"/></collision>"#;

    fn npcs() -> String {
        format!(
            r#"<npc id="32033" type="Folk" level="70" name="Doctor Chaos"/>
<npc id="25512" type="GrandBoss" level="85" name="Chaos Golem" title="Boss">{STATS}</npc>"#
        )
    }

    fn setup_row(status: i32, respawn_time: i64, writes: bool) -> ScriptTest {
        let mut registry = ScriptRegistry::new();
        registry.add(DrChaos);
        boss_test(
            registry,
            &npcs(),
            "",
            &[],
            vec![row(
                CHAOS_GOLEM_NPC_ID,
                status,
                respawn_time,
                (96080, -110822, -3343),
            )],
            writes,
        )
    }

    fn status(t: &ScriptTest) -> Option<i32> {
        t.world.grand_boss_status(CHAOS_GOLEM_NPC_ID)
    }

    fn has_timer(t: &mut ScriptTest, name: &str, npc: Option<ObjectId>) -> bool {
        let mut found = false;
        t.world.run_as_script("DrChaos", |ctx| {
            let npc = npc.and_then(|n| ctx.script_npc(n));
            found = ctx.has_quest_timer(name, npc, None);
        });
        found
    }

    fn fire(t: &mut ScriptTest, event: &str, npc: Option<ObjectId>) {
        t.world.run_as_script("DrChaos", |ctx| {
            let npc = npc.and_then(|n| ctx.script_npc(n));
            ctx.notify_event(event, npc, None);
        });
    }

    fn talk(t: &mut ScriptTest, doctor: ObjectId) -> String {
        let mut out = None;
        t.world.run_as_script("DrChaos", |ctx| {
            let npc = ctx.script_npc(doctor).unwrap();
            out = DrChaos.on_first_talk(ctx, npc, ObjectId(1));
        });
        out.unwrap()
    }

    fn countdown(t: &mut ScriptTest) -> i32 {
        let mut value = 0;
        t.world.run_as_script("DrChaos", |ctx| {
            value = ctx.state::<DrChaosState>().paranoia_countdown
        });
        value
    }

    #[test]
    fn a_normal_row_spawns_the_doctor_who_starts_counting() {
        let mut t = setup_row(STATUS_NORMAL, 0, true);
        t.ticks(2);
        let doctor = t.npcs_of(DOCTOR_CHAOS_NPC_ID);
        assert_eq!(doctor.len(), 1);
        assert_eq!(t.world.position_of(doctor[0]).unwrap(), (96320, -110912, -3328));
        assert_eq!(countdown(&mut t), 30);
        assert!(has_timer(&mut t, EVENT_PARANOIA_ACTIVITY, Some(doctor[0])));
        assert!(has_timer(&mut t, EVENT_DISTANCE_CHECK, Some(doctor[0])));
        assert!(grand_writes(&t.take_boss()).is_empty());
    }

    #[test]
    fn the_dead_row_waits_or_brings_the_doctor_back() {
        let mut t = setup_row(STATUS_DEAD, now_ms() + 3_600_000, true);
        assert!(t.npcs_of(DOCTOR_CHAOS_NPC_ID).is_empty());
        assert!(has_timer(&mut t, EVENT_RESET_DRCHAOS, None));

        let t = setup_row(STATUS_DEAD, now_ms() - 1000, true);
        assert_eq!(t.npcs_of(DOCTOR_CHAOS_NPC_ID).len(), 1);
        assert_eq!(status(&t), Some(STATUS_NORMAL));
    }

    #[test]
    fn a_crazy_row_restores_the_golem_with_its_vitals_and_the_leash() {
        let mut t = setup_row(STATUS_CRAZY, 0, true);
        let golem = t.world.grand_boss_npc(CHAOS_GOLEM_NPC_ID).unwrap();
        assert_eq!(t.world.position_of(golem).unwrap(), (96080, -110822, -3343));
        assert_eq!(t.world.hp_and_max(golem).unwrap().0, 1000.0);
        assert!(has_timer(&mut t, EVENT_GOLEM_DESPAWN, Some(golem)));
        assert!(t.npcs_of(DOCTOR_CHAOS_NPC_ID).is_empty());
    }

    #[test]
    fn with_the_switch_off_the_row_decides() {
        // A row that is not 0 is the other server's.
        let t = setup_row(STATUS_CRAZY, 0, false);
        assert!(t.npcs_of(CHAOS_GOLEM_NPC_ID).is_empty());
        assert!(t.npcs_of(DOCTOR_CHAOS_NPC_ID).is_empty());

        // A dead row whose time has come asks for the row first.
        let mut t = setup_row(STATUS_DEAD, now_ms() - 1000, false);
        let then = BossReadThen::ScriptEvent {
            script: "DrChaos",
            event: EVENT_ROW_READ,
        };
        assert_eq!(
            rereads(&t.take_boss()),
            [(BossReadKind::GrandBoss(CHAOS_GOLEM_NPC_ID), then)]
        );
        assert!(t.npcs_of(DOCTOR_CHAOS_NPC_ID).is_empty());
        answer(
            &mut t,
            then,
            CHAOS_GOLEM_NPC_ID,
            row(CHAOS_GOLEM_NPC_ID, STATUS_NORMAL, 0, (96080, -110822, -3343)),
        );
        assert_eq!(t.npcs_of(DOCTOR_CHAOS_NPC_ID).len(), 1);
        assert!(grand_writes(&t.take_boss()).is_empty());

        // The row stays dead: the doctor stays away.
        let mut t = setup_row(STATUS_DEAD, now_ms() - 1000, false);
        t.take_boss();
        answer(
            &mut t,
            then,
            CHAOS_GOLEM_NPC_ID,
            row(
                CHAOS_GOLEM_NPC_ID,
                STATUS_DEAD,
                now_ms() - 5,
                (96080, -110822, -3343),
            ),
        );
        assert!(t.npcs_of(DOCTOR_CHAOS_NPC_ID).is_empty());
    }

    #[test]
    fn talking_runs_the_countdown_down_through_three_pages_to_the_transformation() {
        let mut t = setup_row(STATUS_NORMAL, 0, true);
        t.ticks(2);
        let doctor = t.npcs_of(DOCTOR_CHAOS_NPC_ID)[0];
        let page = talk(&mut t, doctor);
        assert!(page.contains("What?! Who are you?"), "{page}");
        assert!((24..=29).contains(&countdown(&mut t)));

        t.world.run_as_script("DrChaos", |ctx| {
            ctx.state::<DrChaosState>().paranoia_countdown = 21
        });
        // 21 - (1..=5) is 16..=20.
        assert!(talk(&mut t, doctor).contains("private propertie"));
        t.world.run_as_script("DrChaos", |ctx| {
            ctx.state::<DrChaosState>().paranoia_countdown = 11
        });
        assert!(talk(&mut t, doctor).contains("traitor"));

        t.world.run_as_script("DrChaos", |ctx| {
            ctx.state::<DrChaosState>().paranoia_countdown = 1
        });
        assert_eq!(talk(&mut t, doctor), "");
        assert_eq!(status(&t), Some(STATUS_CRAZY));
        for name in ["1", "2", "3", "4", "5"] {
            assert!(has_timer(&mut t, name, Some(doctor)), "{name}");
        }
        assert!(!has_timer(&mut t, EVENT_PARANOIA_ACTIVITY, Some(doctor)));
    }

    #[test]
    fn a_player_close_to_the_doctor_runs_the_countdown_each_second() {
        let mut t = setup_row(STATUS_NORMAL, 0, true);
        t.enter_with(player_at(1, (96320 + 100, -110912, -3328), vec![]));
        t.ticks(2);
        let doctor = t.npcs_of(DOCTOR_CHAOS_NPC_ID)[0];
        fire(&mut t, EVENT_PARANOIA_ACTIVITY, Some(doctor));
        assert_eq!(countdown(&mut t), 29);
        t.world.run_as_script("DrChaos", |ctx| {
            ctx.state::<DrChaosState>().paranoia_countdown = 1
        });
        fire(&mut t, EVENT_PARANOIA_ACTIVITY, Some(doctor));
        assert_eq!(status(&t), Some(STATUS_CRAZY));
    }

    #[test]
    fn the_cinematic_ends_with_the_golem_and_without_the_doctor() {
        let mut t = setup_row(STATUS_NORMAL, 0, true);
        t.ticks(2);
        let doctor = t.npcs_of(DOCTOR_CHAOS_NPC_ID)[0];
        t.world.run_as_script("DrChaos", |ctx| {
            ctx.state::<DrChaosState>().paranoia_countdown = 0
        });
        fire(&mut t, EVENT_PARANOIA_ACTIVITY, Some(doctor));
        // The doctor has no player near: nothing yet. Start it as the talk does.
        assert_eq!(status(&t), Some(STATUS_NORMAL));
        t.world.run_as_script("DrChaos", |ctx| {
            let npc = ctx.script_npc(doctor).unwrap();
            crazy_midget_becomes_angry(ctx, npc);
        });
        t.wait_ms(17_100);
        let golem = t.npcs_of(CHAOS_GOLEM_NPC_ID);
        assert_eq!(golem.len(), 1);
        assert_eq!(t.world.position_of(golem[0]).unwrap(), (96080, -110822, -3343));
        assert_eq!(t.world.grand_boss_npc(CHAOS_GOLEM_NPC_ID), Some(golem[0]));
        assert!(!t.in_world(doctor));
        assert!(has_timer(&mut t, EVENT_DISTANCE_CHECK, Some(golem[0])));
        assert!(has_timer(&mut t, EVENT_GOLEM_DESPAWN, Some(golem[0])));
    }

    #[test]
    fn the_golem_goes_back_to_the_doctor_after_half_an_hour_without_a_hit() {
        let mut t = setup_row(STATUS_CRAZY, 0, true);
        let golem = t.world.grand_boss_npc(CHAOS_GOLEM_NPC_ID).unwrap();
        // A hit now: not yet.
        fire(&mut t, EVENT_GOLEM_DESPAWN, Some(golem));
        assert!(t.in_world(golem));
        t.world.run_as_script("DrChaos", |ctx| {
            ctx.state::<DrChaosState>().last_golem_attack_time = 0
        });
        fire(&mut t, EVENT_GOLEM_DESPAWN, Some(golem));
        assert!(!t.in_world(golem));
        assert_eq!(t.npcs_of(DOCTOR_CHAOS_NPC_ID).len(), 1);
        assert_eq!(status(&t), Some(STATUS_NORMAL));
    }

    #[test]
    fn the_golem_s_death_stores_the_respawn_between_12_and_60_hours() {
        let mut t = setup_row(STATUS_CRAZY, 0, true);
        t.enter_with(player_at(1, (96080 + 50, -110822, -3343), vec![]));
        let golem = t.world.grand_boss_npc(CHAOS_GOLEM_NPC_ID).unwrap();
        let before = now_ms();
        t.kill(1, golem);
        assert_eq!(status(&t), Some(STATUS_DEAD));
        let stored = t.world.grand_boss_info(CHAOS_GOLEM_NPC_ID).unwrap().respawn_time;
        assert!(stored >= before + 12 * MILLIS_PER_HOUR, "{stored}");
        assert!(stored <= now_ms() + 60 * MILLIS_PER_HOUR, "{stored}");
        assert!(has_timer(&mut t, EVENT_RESET_DRCHAOS, None));
        assert!(!has_timer(&mut t, EVENT_DISTANCE_CHECK, None));
        let writes = grand_writes(&t.take_boss());
        assert!(
            writes.iter().any(|w| matches!(
                w,
                BossWrite::GrandBossStatus {
                    boss_id: CHAOS_GOLEM_NPC_ID,
                    status: STATUS_DEAD
                }
            )),
            "{writes:?}"
        );
    }

    #[test]
    fn a_far_away_golem_is_brought_back_to_its_spawn() {
        let mut t = setup_row(STATUS_CRAZY, 0, true);
        let golem = t.world.grand_boss_npc(CHAOS_GOLEM_NPC_ID).unwrap();
        t.world.tele_to_location(
            golem,
            TeleportTarget::at(96080 + 12_000, -110822, -3343).with_instance(0),
        );
        fire(&mut t, EVENT_DISTANCE_CHECK, Some(golem));
        let (x, y, _) = t.world.position_of(golem).unwrap();
        assert_eq!((x, y), (96080, -110822));
    }
}
