//! Andreas Van Halter raid AI: balcony cleanup -> door swap -> ritual cinematic -> boss combat.
// Java: dist/game/data/scripts/ai/areas/PaganTemple/AndreasVanHalter.java

use l2_protocol_game::server::{PlaySound, SocialAction, SpecialCamera};
use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup, SkillKey};

// NPCs
const BOSS: i32 = 29062;
const RITUAL_OFFERING: i32 = 32038;
const ALTAR_GATEKEEPER: i32 = 32051;
const CAMERA_DUMMY: i32 = 13014;
const CAPTAIN_ROYAL_GUARD_A: i32 = 22175;
const CAPTAIN_ROYAL_GUARD_B: i32 = 22188;
const ROYAL_GUARD_BALCONY: i32 = 22176;
const ROYAL_GUARD_AID: i32 = 22189;

const BALCONY_MOBS: [i32; 4] = [
    CAPTAIN_ROYAL_GUARD_A,
    CAPTAIN_ROYAL_GUARD_B,
    ROYAL_GUARD_BALCONY,
    ROYAL_GUARD_AID,
];

// Doors (retail HighFive Pagan Temple altar 3rd floor).
const BALCONY_DOOR_1: i32 = 19160014;
const BALCONY_DOOR_2: i32 = 19160015;
const ALTAR_DOOR_1: i32 = 19160016;
const ALTAR_DOOR_2: i32 = 19160017;

// Locations (retail PTS confirmed): `(x, y, z, heading)`.
const RITUAL_VICTIM_SPAWN: (i32, i32, i32, i32) = (-16384, -53197, -10439, 15992);
const CAMERA_SPAWN: (i32, i32, i32, i32) = (-16362, -53754, -10439, 0);
const BOSS_FALLBACK: (i32, i32, i32) = (-16393, -53433, -10439);
const BALCONY_DOOR_1_FALLBACK: (i32, i32, i32) = (-15690, -54030, -10439);
const BALCONY_DOOR_2_FALLBACK: (i32, i32, i32) = (-17150, -54064, -10439);
const ALTAR_GATEKEEPER_SPAWNS: [(i32, i32, i32, i32); 4] = [
    (-17248, -54832, -10424, 16384),
    (-15547, -54835, -10424, 16384),
    (-18116, -54831, -10579, 16384),
    (-14645, -54836, -10577, 16384),
];

// Skill (retail PTS Curse Poison lvl 7 cast on the ritual offering NPC).
const RITUAL_SKILL: SkillKey = SkillKey { id: 1168, level: 7 };

// 16 kills triggers the ritual without waiting for respawns (~80% of the 20 mobs reachable near the altar).
const BALCONY_KILLS_THRESHOLD: i32 = 16;
const RAID_RESET_DELAY: i64 = 5 * 60 * 60 * 1000;
// Z below altar floor (-10439) means the boss was dragged toward the balcony floor (-10594).
const ANTI_GLITCH_Z: i32 = -10500;
const ANTI_GLITCH_DOOR_RANGE: f64 = 200.0;

// State
const STATUS_DORMANT: i32 = 0;
const STATUS_CINEMATIC: i32 = 1;
const STATUS_FIGHT: i32 = 2;

/// The fields of the Java class.
#[derive(Default)]
struct RaidState {
    status: i32,
    balcony_kills: i32,
    last_attack: i64,
    boss: Option<ScriptNpc>,
    camera: Option<ScriptNpc>,
    ritual_victim: Option<ScriptNpc>,
    spawned: Vec<ScriptNpc>,
}

fn open_door(ctx: &mut ScriptCtx<'_>, door_id: i32) {
    ctx.open_door(door_id, 0);
}

fn close_door(ctx: &mut ScriptCtx<'_>, door_id: i32) {
    ctx.close_door(door_id, 0);
}

/// Java `broadcastCinematic(packet)`: from the boss, nothing without one.
fn broadcast_cinematic(ctx: &ScriptCtx<'_>, boss: Option<ScriptNpc>, packet: &impl l2_net::Encode) {
    if let Some(boss) = boss {
        ctx.world().broadcast_packet(boss.object_id, packet);
    }
}

/// Java `new SpecialCamera(_camera, force, angle1, angle2, time, duration, 0, 0, 1, 0, 0)`.
fn camera_shot(camera: ScriptNpc, force: i32, angle1: i32, angle2: i32, time: i32) -> SpecialCamera {
    SpecialCamera::new(
        camera.object_id.0,
        force,
        angle1,
        angle2,
        time,
        5000,
        0,
        0,
        1,
        0,
        0,
    )
}

fn despawn_ephemeral(ctx: &mut ScriptCtx<'_>) {
    let spawned = std::mem::take(&mut ctx.state::<RaidState>().spawned);
    for npc in spawned {
        if !ctx.is_dead(npc.object_id) {
            ctx.delete_npc(npc);
        }
    }
    let state = ctx.state::<RaidState>();
    state.camera = None;
    state.ritual_victim = None;
}

fn reset_doors(ctx: &mut ScriptCtx<'_>) {
    open_door(ctx, BALCONY_DOOR_1);
    open_door(ctx, BALCONY_DOOR_2);
    close_door(ctx, ALTAR_DOOR_1);
    close_door(ctx, ALTAR_DOOR_2);
}

fn reset_raid(ctx: &mut ScriptCtx<'_>) {
    let now = ctx.now_ms();
    let state = ctx.state::<RaidState>();
    state.status = STATUS_DORMANT;
    state.balcony_kills = 0;
    state.last_attack = now;
    despawn_ephemeral(ctx);
    reset_doors(ctx);
}

fn cleanup_raid(ctx: &mut ScriptCtx<'_>) {
    let state = ctx.state::<RaidState>();
    state.status = STATUS_DORMANT;
    state.balcony_kills = 0;
    despawn_ephemeral(ctx);
    reset_doors(ctx);
    let boss = ctx.state::<RaidState>().boss.take();
    ctx.cancel_quest_timer("RESET_CHECK", boss, None);
}

fn trigger_ritual(ctx: &mut ScriptCtx<'_>) {
    let Some(boss) = ctx.state::<RaidState>().boss else {
        return;
    };
    if ctx.is_dead(boss.object_id) {
        return;
    }
    ctx.state::<RaidState>().status = STATUS_CINEMATIC;

    close_door(ctx, BALCONY_DOOR_1);
    close_door(ctx, BALCONY_DOOR_2);
    open_door(ctx, ALTAR_DOOR_1);
    open_door(ctx, ALTAR_DOOR_2);

    for (x, y, z, heading) in ALTAR_GATEKEEPER_SPAWNS {
        if let Some(gatekeeper) = ctx.add_spawn(ALTAR_GATEKEEPER, x, y, z, heading, false, 0, 0) {
            ctx.state::<RaidState>().spawned.push(gatekeeper);
        }
    }

    let (x, y, z, heading) = RITUAL_VICTIM_SPAWN;
    let victim = ctx.add_spawn(RITUAL_OFFERING, x, y, z, heading, false, 0, 0);
    ctx.state::<RaidState>().ritual_victim = victim;
    if let Some(victim) = victim {
        ctx.state::<RaidState>().spawned.push(victim);
    }

    ctx.start_quest_timer("CINEMATIC_1", 3000, Some(boss), None, false);
}

/// Java `ai.areas.PaganTemple.AndreasVanHalter`.
pub struct AndreasVanHalter;

impl Script for AndreasVanHalter {
    fn name(&self) -> &'static str {
        "AndreasVanHalter"
    }

    fn path(&self) -> &'static str {
        "ai/areas/PaganTemple"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_spawn_id(&[BOSS]);
        s.add_attack_id(&[BOSS]);
        s.add_kill_id(&[BOSS]);
        s.add_kill_id(&BALCONY_MOBS);
    }

    fn on_spawn(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc) {
        if npc.id != BOSS {
            return;
        }
        let now = ctx.now_ms();
        let state = ctx.state::<RaidState>();
        state.boss = Some(npc);
        state.status = STATUS_DORMANT;
        state.balcony_kills = 0;
        state.last_attack = now;

        open_door(ctx, BALCONY_DOOR_1);
        open_door(ctx, BALCONY_DOOR_2);
        close_door(ctx, ALTAR_DOOR_1);
        close_door(ctx, ALTAR_DOOR_2);

        ctx.start_quest_timer("RESET_CHECK", 60_000, Some(npc), None, true);
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
        if npc.id != BOSS {
            return;
        }
        let now = ctx.now_ms();
        ctx.state::<RaidState>().last_attack = now;

        let Some((x, y, z)) = ctx.world().position_of(npc.object_id) else {
            return;
        };
        if z <= ANTI_GLITCH_Z {
            let (fx, fy, fz) = BOSS_FALLBACK;
            ctx.tele_npc_to(npc, fx, fy, fz);
            return;
        }

        let distance_2d = |ctx: &ScriptCtx<'_>, door_id: i32| {
            ctx.world().door_by_door_id(door_id).map(|d| {
                let (dx, dy) = (f64::from(x - d.x), f64::from(y - d.y));
                (dx * dx + dy * dy).sqrt()
            })
        };
        if distance_2d(ctx, ALTAR_DOOR_1).is_some_and(|d| d <= ANTI_GLITCH_DOOR_RANGE) {
            let (fx, fy, fz) = BALCONY_DOOR_1_FALLBACK;
            ctx.tele_npc_to(npc, fx, fy, fz);
            return;
        }

        if distance_2d(ctx, ALTAR_DOOR_2).is_some_and(|d| d <= ANTI_GLITCH_DOOR_RANGE) {
            let (fx, fy, fz) = BALCONY_DOOR_2_FALLBACK;
            ctx.tele_npc_to(npc, fx, fy, fz);
        }
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, _killer: ObjectId, _is_summon: bool) {
        if npc.id == BOSS {
            cleanup_raid(ctx);
            return;
        }

        if ctx.state::<RaidState>().status != STATUS_DORMANT {
            return;
        }

        // Atomic in Java to survive AOE wipes that kill several balcony mobs in the same tick.
        let state = ctx.state::<RaidState>();
        state.balcony_kills += 1;
        if state.balcony_kills == BALCONY_KILLS_THRESHOLD {
            trigger_ritual(ctx);
        }
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        _npc: Option<ScriptNpc>,
        _player: Option<ObjectId>,
    ) -> Option<String> {
        let (boss, camera, victim) = {
            let state = ctx.state::<RaidState>();
            (state.boss, state.camera, state.ritual_victim)
        };
        match event {
            "CINEMATIC_1" => {
                let boss = boss?;
                let victim = victim?;
                if ctx.is_dead(boss.object_id) {
                    return None;
                }

                ctx.set_heading_and_validate(boss, 16384);
                let (cx, cy, cz, _) = CAMERA_SPAWN;
                let camera = ctx.add_spawn(CAMERA_DUMMY, cx, cy, cz, 0, false, 30_000, 0);
                ctx.state::<RaidState>().camera = camera;
                if let Some(camera) = camera {
                    ctx.state::<RaidState>().spawned.push(camera);
                }

                ctx.world_mut()
                    .set_npc_target(boss.object_id, Some(victim.object_id));
                ctx.world_mut().do_cast(boss.object_id, RITUAL_SKILL);
                ctx.set_immobilized(boss, true);
                if let Some((x, y, z)) = ctx.world().position_of(boss.object_id) {
                    broadcast_cinematic(
                        ctx,
                        Some(boss),
                        &PlaySound {
                            unknown1: 0,
                            sound_file: "BS04_A".to_owned(),
                            unknown3: 1,
                            unknown4: boss.object_id.0,
                            unknown5: x,
                            unknown6: y,
                            unknown7: z,
                            unknown8: 0,
                        },
                    );
                }

                if let Some(camera) = camera {
                    broadcast_cinematic(ctx, Some(boss), &camera_shot(camera, 1500, 88, 89, 0));
                }

                ctx.start_quest_timer("CINEMATIC_2", 300, Some(boss), None, false);
            }
            "CINEMATIC_2" => {
                if let Some(victim) = victim
                    && !ctx.is_dead(victim.object_id)
                {
                    ctx.world().broadcast_packet(
                        victim.object_id,
                        &SocialAction {
                            object_id: victim.object_id.0,
                            action_id: 1,
                        },
                    );
                    ctx.world_mut().set_current_hp(victim.object_id, 0.0, true);
                }

                if let Some(camera) = camera {
                    broadcast_cinematic(ctx, boss, &camera_shot(camera, 1500, 88, 89, 0));
                }

                ctx.start_quest_timer("CINEMATIC_3", 300, boss, None, false);
            }
            "CINEMATIC_3" => {
                if let Some(victim) = victim
                    && !ctx.is_dead(victim.object_id)
                {
                    ctx.delete_npc(victim);
                }
                ctx.state::<RaidState>().ritual_victim = None;

                if let Some(camera) = camera {
                    broadcast_cinematic(ctx, boss, &camera_shot(camera, 450, 88, 3, 5500));
                }

                ctx.start_quest_timer("CINEMATIC_4", 9400, boss, None, false);
            }
            "CINEMATIC_4" => {
                if let Some(camera) = camera {
                    broadcast_cinematic(ctx, boss, &camera_shot(camera, 500, 88, 4, 5000));
                }

                ctx.start_quest_timer("CINEMATIC_5", 5000, boss, None, false);
            }
            "CINEMATIC_5" => {
                if let Some(camera) = camera {
                    broadcast_cinematic(ctx, boss, &camera_shot(camera, 3000, 88, 4, 6000));
                }

                ctx.start_quest_timer("CINEMATIC_END", 6000, boss, None, false);
            }
            "CINEMATIC_END" => {
                // Acolyte minions (29063/29064) are spawned automatically from the boss <minions> stats.
                if let Some(boss) = boss {
                    ctx.set_immobilized(boss, false);
                }

                if let Some(camera) = camera {
                    ctx.delete_npc(camera);
                    ctx.state::<RaidState>().camera = None;
                }

                ctx.state::<RaidState>().status = STATUS_FIGHT;
            }
            "RESET_CHECK" => {
                let last_attack = ctx.state::<RaidState>().last_attack;
                if let Some(boss) = boss
                    && !ctx.is_dead(boss.object_id)
                    && !ctx.world().is_attacking_now(boss.object_id)
                    && ((ctx.now_ms() - last_attack) >= RAID_RESET_DELAY)
                {
                    reset_raid(ctx);
                }
            }
            _ => {}
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::ScriptTest;

    const NPCS: &str = r#"
        <npc id="29062" type="Monster" level="80" name="Andreas Van Halter"/>
        <npc id="22175" type="Monster" level="80" name="Captain of the Royal Guard"/>
        <npc id="32051" type="Folk" level="70" name="Altar Gatekeeper"/>
        <npc id="32038" type="Folk" level="70" name="Ritual Offering"/>
        <npc id="13014" type="Folk" level="70" name="Camera"/>"#;

    #[test]
    fn the_sixteenth_balcony_kill_starts_the_ritual_and_the_cinematic_ends_in_the_fight() {
        let mut registry = ScriptRegistry::new();
        registry.add(AndreasVanHalter);
        let mut t = ScriptTest::new(registry, NPCS, "", &[]);
        let boss = t.spawn(BOSS, 20);
        t.ticks(2);
        t.world.run_as_script("AndreasVanHalter", |ctx| {
            let npc = ctx.script_npc(boss).unwrap();
            assert!(ctx.has_quest_timer("RESET_CHECK", Some(npc), None));
            for _ in 0..15 {
                AndreasVanHalter.on_kill(
                    ctx,
                    ScriptNpc {
                        object_id: ObjectId(0),
                        id: CAPTAIN_ROYAL_GUARD_A,
                    },
                    ObjectId(1),
                    false,
                );
            }
            assert_eq!(ctx.state::<RaidState>().status, STATUS_DORMANT);
            AndreasVanHalter.on_kill(
                ctx,
                ScriptNpc {
                    object_id: ObjectId(0),
                    id: CAPTAIN_ROYAL_GUARD_A,
                },
                ObjectId(1),
                false,
            );
            assert_eq!(ctx.state::<RaidState>().status, STATUS_CINEMATIC);
            assert!(ctx.has_quest_timer("CINEMATIC_1", Some(npc), None));
        });
        assert_eq!(t.npcs_of(ALTAR_GATEKEEPER).len(), 4);
        assert_eq!(t.npcs_of(RITUAL_OFFERING).len(), 1);
        t.wait_ms(3_100);
        assert_eq!(t.npcs_of(CAMERA_DUMMY).len(), 1);
        // 0.3 + 0.3 + 9.4 + 5 + 6 seconds later the fight begins and the victim is gone.
        t.wait_ms(21_500);
        t.world.run_as_script("AndreasVanHalter", |ctx| {
            assert_eq!(ctx.state::<RaidState>().status, STATUS_FIGHT);
        });
        assert!(t.npcs_of(RITUAL_OFFERING).is_empty());
    }
}
