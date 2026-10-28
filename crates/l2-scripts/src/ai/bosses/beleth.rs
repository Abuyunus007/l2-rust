//! Beleth's chamber: the opening movie, the fake Belethes and the real one, the death movie, the
//! priest and the stone coffin with the ring, and the respawn cycle.
// Java: dist/game/data/scripts/ai/bosses/Beleth/Beleth.java
//
// The Java constructor is `on_load`; the fields are `BelethState` (`ctx.state`). Statuses: 0 alive
// (entry open), 1 init (the entry script let a party in), 2 fight, 3 dead. Setting status 1 is the
// entry script's job (not in this folder).
//
// Side by side (D-54, `super` module): with `CycleDatabaseWrites` off the constructor leaves a
// row that is not alive alone (Java would write ALIVE over it), and `BELETH_UNLOCK` reads the
// row again and opens the chamber only when the row says alive.
//
// Not ported (and why):
// - `setShowSummonAnimation(true)` of the fake Belethes and the priest: a visual flag the NPC
//   packet of this server does not carry.
// - `ZONE.broadcastPacket(new StaticObjectInfo(door, false))` and `DoorStatusUpdate(door)` after
//   `closeMe()`/`openMe()`: `World::open_door`/`close_door` already send both to everybody who
//   sees the door.
// - `decayMe()` + `spawnMe()` of the six fake Belethes and the priest: they are created at the
//   `spawnMe()` step instead (nobody sees them in between, and a decayed object has no table entry
//   to come back to).
// - `setOverloaded(true)`: an NPC's overload only feeds `isMovementDisabled()`, so `setImmobilized`
//   stands in for it.

use l2_protocol_game::server::{PlaySound, SocialAction, SpecialCamera};
use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup, SkillKey, TeleportTarget, TeleportWhereType};
use tracing::{info, warn};

use super::{EVENT_ROW_READ, row_lets_boss_run};

// Status.
const ALIVE: i32 = 0;
const INIT: i32 = 1;
const FIGHT: i32 = 2;
const DEAD: i32 = 3;

// NPCs.
const REAL_BELETH: i32 = 29118;
const FAKE_BELETH: i32 = 29119;
const STONE_COFFIN: i32 = 32470;
const ELF: i32 = 29128;
const WHIRPOOL: i32 = 29125;

// Zones: `ZoneManager.getZoneById(12018)`.
const ZONE: (i32, &str) = (12018, "BossZone");
const BELETH_SPAWN: (i32, i32, i32, i32) = (16323, 213059, -9357, 49152);

// Skills.
const BLEED: SkillKey = SkillKey { id: 5495, level: 1 };
const FIREBALL: SkillKey = SkillKey { id: 5496, level: 1 };
const HORN_OF_RISING: SkillKey = SkillKey { id: 5497, level: 1 };
const LIGHTENING: SkillKey = SkillKey { id: 5499, level: 1 };

// Doors.
const DOOR1: i32 = 20240001;
const DOOR2: i32 = 20240002;
const DOOR3: i32 = 20240003;

// Items: `new ItemHolder(10314, 1)`.
const RING: i32 = 10314;

/// The fields of the Java class.
#[derive(Debug, Default)]
pub(crate) struct BelethState {
    pub(crate) camera1: Option<ObjectId>,
    pub(crate) camera2: Option<ObjectId>,
    pub(crate) camera3: Option<ObjectId>,
    pub(crate) camera4: Option<ObjectId>,
    pub(crate) whirpool: Option<ObjectId>,
    pub(crate) beleth: Option<ObjectId>,
    pub(crate) priest: Option<ObjectId>,
    pub(crate) stone: Option<ObjectId>,
    /// `_killer`.
    pub(crate) killer: Option<ObjectId>,
    /// `_allowedObjId` (0 for none).
    pub(crate) allowed_obj_id: i32,
    pub(crate) killed_count: i32,
    pub(crate) last_attack: i64,
    /// `_minions`.
    pub(crate) minions: Vec<ObjectId>,
}

/// Java `ai.bosses.Beleth.Beleth`.
pub struct Beleth;

impl Script for Beleth {
    fn name(&self) -> &'static str {
        "Beleth"
    }

    fn path(&self) -> &'static str {
        "ai/bosses/Beleth"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_enter_zone_id(&[ZONE.0]);
        s.add_attack_id(&[REAL_BELETH, FAKE_BELETH]);
        s.add_kill_id(&[REAL_BELETH, FAKE_BELETH]);
        s.add_spawn_id(&[REAL_BELETH, FAKE_BELETH]);
        s.add_spell_finished_id(&[REAL_BELETH, FAKE_BELETH]);
        s.add_skill_see_id(&[REAL_BELETH, FAKE_BELETH]);
        s.add_aggro_range_enter_id(&[REAL_BELETH, FAKE_BELETH]);
        s.add_start_npc(&[STONE_COFFIN]);
        s.add_talk_id(&[STONE_COFFIN]);
        s.add_first_talk_id(&[ELF]);
    }

    /// The constructor's second half.
    fn on_load(&self, ctx: &mut ScriptCtx<'_>) {
        match ctx.world().grand_boss_info(REAL_BELETH).cloned() {
            None => {
                // Java: `getStatSet` answers null and the constructor throws.
                warn!("Beleth: no grandboss_data row for {REAL_BELETH}, the script does not start.");
                return;
            }
            Some(info) => {
                let status = ctx.world().grand_boss_status(REAL_BELETH).unwrap_or(-1);
                if !row_lets_boss_run(ctx, REAL_BELETH) && status != DEAD {
                    // D-54: the row is the other server's fight; Java would write ALIVE over it.
                    info!("Beleth: the shared state row says {status}; the AI waits for an alive row (D-54).");
                } else if status == DEAD {
                    let time = info.respawn_time - ctx.now_ms();
                    if time > 0 {
                        ctx.start_quest_timer("BELETH_UNLOCK", time, None, None, false);
                    } else if !ctx.world().boss_writes_enabled() {
                        // D-54: the unlock waits for the other server's alive row.
                        ctx.notify_event("BELETH_UNLOCK", None, None);
                    } else {
                        ctx.world_mut().set_grand_boss_status(REAL_BELETH, ALIVE);
                    }
                } else if status != ALIVE {
                    ctx.world_mut().set_grand_boss_status(REAL_BELETH, ALIVE);
                }
            }
        }

        ctx.open_door(DOOR1, 0);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        _player: Option<ObjectId>,
    ) -> Option<String> {
        match event {
            "BELETH_UNLOCK" => {
                // D-54: on the shared database the row decides; read it again first.
                if !ctx.world().boss_writes_enabled() {
                    ctx.world_mut()
                        .reread_grand_boss_for_script(REAL_BELETH, self.name(), EVENT_ROW_READ);
                    return None;
                }
                ctx.world_mut().set_grand_boss_status(REAL_BELETH, ALIVE);
                ctx.open_door(DOOR1, 0);
            }
            EVENT_ROW_READ => {
                // D-54: the answer of the read `BELETH_UNLOCK` asked for.
                match ctx.world().grand_boss_status(REAL_BELETH) {
                    Some(ALIVE) => {
                        ctx.world_mut().set_grand_boss_status(REAL_BELETH, ALIVE);
                        ctx.open_door(DOOR1, 0);
                    }
                    Some(DEAD) => {
                        let respawn = ctx.world().grand_boss_info(REAL_BELETH).map_or(0, |i| i.respawn_time);
                        let remaining = respawn - ctx.now_ms();
                        if remaining > 0 {
                            ctx.start_quest_timer("BELETH_UNLOCK", remaining, None, None, false);
                        } else {
                            info!("Beleth: the shared state row says dead; the chamber stays shut (D-54).");
                        }
                    }
                    status => info!("Beleth: the shared state row says {status:?}; the chamber stays shut (D-54)."),
                }
            }
            "CAST" => {
                if let Some(npc) = npc
                    && !ctx.world().is_dead(npc.object_id)
                    && !ctx.world().is_casting_now(npc.object_id)
                {
                    ctx.world_mut().set_intention_active(npc.object_id);
                    ctx.world_mut().do_cast(npc.object_id, FIREBALL);
                }
            }
            "SPAWN1" => {
                for c in zone_characters(ctx) {
                    ctx.world_mut().disable_all_skills(c);
                    ctx.world_mut().set_invul(c, true);
                    ctx.world_mut().set_immobilized(c, true);
                }

                let camera1 = ctx.add_spawn(29120, 16323, 213142, -9357, 0, false, 0, 0);
                let camera2 = ctx.add_spawn(29121, 16323, 210741, -9357, 0, false, 0, 0);
                let camera3 = ctx.add_spawn(29122, 16323, 213170, -9357, 0, false, 0, 0);
                let camera4 = ctx.add_spawn(29123, 16323, 214917, -9356, 0, false, 0, 0);
                let state = ctx.state::<BelethState>();
                state.camera1 = camera1.map(|n| n.object_id);
                state.camera2 = camera2.map(|n| n.object_id);
                state.camera3 = camera3.map(|n| n.object_id);
                state.camera4 = camera4.map(|n| n.object_id);
                if let Some(camera1) = camera1 {
                    let (x, y, z) = ctx.world().position_of(camera1.object_id).unwrap_or((0, 0, 0));
                    broadcast_zone(ctx, &sound("BS07_A", camera1.object_id, (x, y, z)));
                    camera(ctx, camera1.object_id, 400, 75, -25, 0, 2500, 0, 0, 1, 0, 0);
                    camera(ctx, camera1.object_id, 400, 75, -25, 0, 2500, 0, 0, 1, 0, 0);
                }
                ctx.start_quest_timer("SPAWN2", 300, None, None, false);
            }
            "SPAWN2" => {
                cam_of(ctx, 1, 1800, -45, -45, 5000, 5000, 0, 0, 1, 0, 0);
                ctx.start_quest_timer("SPAWN3", 4900, None, None, false);
            }
            "SPAWN3" => {
                cam_of(ctx, 1, 2500, -120, -45, 5000, 5000, 0, 0, 1, 0, 0);
                ctx.start_quest_timer("SPAWN4", 4900, None, None, false);
            }
            "SPAWN4" => {
                cam_of(ctx, 2, 2200, 130, 0, 0, 1500, -20, 15, 1, 0, 0);
                ctx.start_quest_timer("SPAWN5", 1400, None, None, false);
            }
            "SPAWN5" => {
                cam_of(ctx, 2, 2300, 100, 0, 2000, 4500, 0, 10, 1, 0, 0);
                ctx.start_quest_timer("SPAWN6", 2500, None, None, false);
            }
            "SPAWN6" => {
                ctx.close_door(DOOR1, 0);
                ctx.start_quest_timer("SPAWN7", 1700, None, None, false);
            }
            "SPAWN7" => {
                cam_of(ctx, 4, 1500, 210, 0, 0, 1500, 0, 0, 1, 0, 0);
                cam_of(ctx, 4, 900, 255, 0, 5000, 6500, 0, 10, 1, 0, 0);
                ctx.start_quest_timer("SPAWN8", 6000, None, None, false);
            }
            "SPAWN8" => {
                let whirpool = ctx.add_spawn(WHIRPOOL, 16323, 214917, -9356, 0, false, 0, 0);
                ctx.state::<BelethState>().whirpool = whirpool.map(|n| n.object_id);
                cam_of(ctx, 4, 900, 255, 0, 0, 1500, 0, 10, 1, 0, 0);
                ctx.start_quest_timer("SPAWN9", 1000, None, None, false);
            }
            "SPAWN9" => {
                cam_of(ctx, 4, 1000, 255, 0, 7000, 17000, 0, 25, 1, 0, 0);
                ctx.start_quest_timer("SPAWN10", 3000, None, None, false);
            }
            "SPAWN10" => {
                let beleth = ctx.add_spawn(REAL_BELETH, 16321, 214211, -9352, 49369, false, 0, 0);
                ctx.state::<BelethState>().beleth = beleth.map(|n| n.object_id);
                if let Some(beleth) = beleth {
                    freeze(ctx, beleth.object_id);
                }

                ctx.start_quest_timer("SPAWN11", 200, None, None, false);
            }
            "SPAWN11" => {
                if let Some(beleth) = ctx.state::<BelethState>().beleth {
                    broadcast_zone(
                        ctx,
                        &SocialAction {
                            object_id: beleth.0,
                            action_id: 1,
                        },
                    );
                }
                // The six fake Belethes appear at SPAWN19 (see the header).
                ctx.start_quest_timer("SPAWN12", 6800, None, None, false);
            }
            "SPAWN12" => {
                cam_of_beleth(ctx, 0, 270, -5, 0, 4000, 0, 0, 1, 0, 0);
                ctx.start_quest_timer("SPAWN13", 3500, None, None, false);
            }
            "SPAWN13" => {
                cam_of_beleth(ctx, 800, 270, 10, 3000, 6000, 0, 0, 1, 0, 0);
                ctx.start_quest_timer("SPAWN14", 5000, None, None, false);
            }
            "SPAWN14" => {
                cam_of(ctx, 3, 100, 270, 15, 0, 5000, 0, 0, 1, 0, 0);
                cam_of(ctx, 3, 100, 270, 15, 0, 5000, 0, 0, 1, 0, 0);
                ctx.start_quest_timer("SPAWN15", 100, None, None, false);
            }
            "SPAWN15" => {
                cam_of(ctx, 3, 100, 270, 15, 3000, 6000, 0, 5, 1, 0, 0);
                ctx.start_quest_timer("SPAWN16", 1400, None, None, false);
            }
            "SPAWN16" => {
                if let Some(beleth) = ctx.state::<BelethState>().beleth {
                    let (x, y, z, heading) = BELETH_SPAWN;
                    ctx.world_mut().tele_to_location(
                        beleth,
                        TeleportTarget::at(x, y, z).with_heading(heading).with_instance(0),
                    );
                }

                ctx.start_quest_timer("SPAWN17", 200, None, None, false);
            }
            "SPAWN17" => {
                if let Some(beleth) = ctx.state::<BelethState>().beleth {
                    ctx.broadcast_magic_skill_use(beleth, beleth, beleth, 5532, 1, 2000, 0);
                }
                ctx.start_quest_timer("SPAWN18", 2000, None, None, false);
            }
            "SPAWN18" => {
                cam_of(ctx, 3, 700, 270, 20, 1500, 8000, 0, 0, 1, 0, 0);
                ctx.start_quest_timer("SPAWN19", 6900, None, None, false);
            }
            "SPAWN19" => {
                cam_of(ctx, 3, 40, 260, 0, 0, 4000, 0, 0, 1, 0, 0);
                // `fakeBeleth.spawnMe()`: the six fake Belethes appear now.
                for i in 0..6 {
                    let angle = f64::from(i) * 1.046666667;
                    let x = ((150.0 * angle.cos()) + 16323.0) as i32;
                    let y = ((150.0 * angle.sin()) + 213059.0) as i32;
                    if let Some(fake) = ctx.add_spawn(FAKE_BELETH, x, y, -9357, 49152, false, 0, 0) {
                        ctx.state::<BelethState>().minions.push(fake.object_id);
                        freeze(ctx, fake.object_id);
                    }
                }

                ctx.start_quest_timer("SPAWN20", 3000, None, None, false);
            }
            "SPAWN20" => {
                cam_of(ctx, 3, 40, 280, 0, 0, 4000, 5, 0, 1, 0, 0);
                ctx.start_quest_timer("SPAWN21", 3000, None, None, false);
            }
            "SPAWN21" => {
                cam_of(ctx, 3, 5, 250, 5, 0, 13000, 20, 15, 1, 0, 0);
                ctx.start_quest_timer("SPAWN22", 1000, None, None, false);
            }
            "SPAWN22" => {
                if let Some(beleth) = ctx.state::<BelethState>().beleth {
                    broadcast_zone(
                        ctx,
                        &SocialAction {
                            object_id: beleth.0,
                            action_id: 3,
                        },
                    );
                }
                ctx.start_quest_timer("SPAWN23", 4000, None, None, false);
            }
            "SPAWN23" => {
                if let Some(beleth) = ctx.state::<BelethState>().beleth {
                    ctx.broadcast_magic_skill_use(beleth, beleth, beleth, 5533, 1, 2000, 0);
                }
                ctx.start_quest_timer("SPAWN24", 6800, None, None, false);
            }
            "SPAWN24" => {
                let state = ctx.state::<BelethState>();
                let beleth = state.beleth.take();
                let minions = std::mem::take(&mut state.minions);
                let cameras = [state.camera1, state.camera2, state.camera3, state.camera4];
                if let Some(beleth) = beleth {
                    ctx.world_mut().delete_npc(beleth);
                }
                for fake_beleth in minions {
                    ctx.world_mut().delete_npc(fake_beleth);
                }

                for camera in cameras.into_iter().flatten() {
                    ctx.world_mut().delete_npc(camera);
                }

                release_zone(ctx);

                let now = ctx.now_ms();
                ctx.state::<BelethState>().last_attack = now;
                ctx.start_quest_timer("CHECK_ATTACK", 60_000, None, None, false);
                ctx.start_quest_timer("SPAWN25", 60_000, None, None, false);
            }
            "SPAWN25" => spawn_fakes(ctx),
            "SPAWN_REAL" => {
                let beleth = ctx.add_spawn(REAL_BELETH, 16323, 213170, -9357, 49152, false, 0, 0);
                ctx.state::<BelethState>().beleth = beleth.map(|n| n.object_id);
                if let Some(beleth) = beleth {
                    ctx.world_mut().add_grand_boss(REAL_BELETH, beleth.object_id);
                }
            }
            "SPAWN26" => {
                if let Some(beleth) = ctx.state::<BelethState>().beleth {
                    ctx.world_mut().do_die(beleth, None);
                }

                let camera1 = ctx.add_spawn(29122, 16323, 213170, -9357, 0, false, 0, 0);
                ctx.state::<BelethState>().camera1 = camera1.map(|n| n.object_id);
                if let Some(camera1) = camera1 {
                    let at = ctx.world().position_of(camera1.object_id).unwrap_or((0, 0, 0));
                    ctx.world()
                        .broadcast_packet(camera1.object_id, &sound("BS07_D", camera1.object_id, at));
                    camera(ctx, camera1.object_id, 400, 290, 25, 0, 10000, 0, 0, 1, 0, 0);
                    camera(ctx, camera1.object_id, 400, 290, 25, 0, 10000, 0, 0, 1, 0, 0);
                    camera(ctx, camera1.object_id, 400, 110, 25, 4000, 10000, 0, 0, 1, 0, 0);
                }
                if let Some(beleth) = ctx.state::<BelethState>().beleth {
                    broadcast_zone(
                        ctx,
                        &SocialAction {
                            object_id: beleth.0,
                            action_id: 5,
                        },
                    );
                }
                ctx.start_quest_timer("SPAWN27", 4000, None, None, false);
            }
            "SPAWN27" => {
                cam_of(ctx, 1, 400, 295, 25, 4000, 5000, 0, 0, 1, 0, 0);
                ctx.start_quest_timer("SPAWN28", 4500, None, None, false);
            }
            "SPAWN28" => {
                cam_of(ctx, 1, 400, 295, 10, 4000, 11000, 0, 25, 1, 0, 0);
                ctx.start_quest_timer("SPAWN29", 9000, None, None, false);
            }
            "SPAWN29" => {
                cam_of(ctx, 1, 250, 90, 25, 0, 1000, 0, 0, 1, 0, 0);
                cam_of(ctx, 1, 250, 90, 25, 0, 10000, 0, 0, 1, 0, 0);
                ctx.start_quest_timer("SPAWN30", 2000, None, None, false);
            }
            "SPAWN30" => {
                // `_priest.spawnMe()`: the priest appears where Beleth died (see the header).
                let priest = ctx.add_spawn(ELF, 16323, 213170, -9357, 49152, false, 0, 0);
                ctx.state::<BelethState>().priest = priest.map(|n| n.object_id);
                if let Some(beleth) = ctx.state::<BelethState>().beleth {
                    ctx.world_mut().delete_npc(beleth);
                }

                let camera2 = ctx.add_spawn(29121, 14056, 213170, -9357, 0, false, 0, 0);
                ctx.state::<BelethState>().camera2 = camera2.map(|n| n.object_id);
                ctx.start_quest_timer("SPAWN31", 3500, None, None, false);
            }
            "SPAWN31" => {
                cam_of(ctx, 2, 800, 180, 0, 0, 4000, 0, 10, 1, 0, 0);
                cam_of(ctx, 2, 800, 180, 0, 0, 4000, 0, 10, 1, 0, 0);

                ctx.open_door(DOOR2, 0);
                ctx.open_door(DOOR3, 0);

                let state = ctx.state::<BelethState>();
                let doomed = [state.camera1, state.camera2, state.whirpool];
                for id in doomed.into_iter().flatten() {
                    ctx.world_mut().delete_npc(id);
                }

                release_zone(ctx);
            }
            "CHECK_ATTACK" => {
                let last_attack = ctx.state::<BelethState>().last_attack;
                // NOTE(java-quirk): `_lastAttack` is set once, at the end of the movie, and
                // `onAttack` never updates it: 15 minutes after the fight starts the chamber is
                // reset whoever is still fighting.
                if (last_attack + 900_000) < ctx.now_ms() {
                    ctx.world_mut().set_grand_boss_status(REAL_BELETH, ALIVE);
                    for creature in zone_characters(ctx) {
                        if ctx.world().is_npc(creature) {
                            ctx.world_mut().delete_npc(creature);
                        } else if ctx.world().player(creature).is_some() {
                            ctx.world_mut()
                                .tele_to_location_where(creature, TeleportWhereType::Town);
                        }
                    }

                    let state = ctx.state::<BelethState>();
                    state.minions.clear();
                    state.killed_count = 0;

                    ctx.cancel_quest_timer("CHECK_ATTACK", None, None);
                } else {
                    ctx.start_quest_timer("CHECK_ATTACK", 60_000, None, None, false);
                }
            }
            _ => {}
        }

        None
    }

    fn on_enter_zone(&self, ctx: &mut ScriptCtx<'_>, creature: ObjectId, _zone_id: i32) {
        if ctx.world().player(creature).is_some() && ctx.world().grand_boss_status(REAL_BELETH) == Some(INIT) {
            let (priest, stone) = {
                let state = ctx.state::<BelethState>();
                (state.priest, state.stone)
            };
            if let Some(priest) = priest {
                ctx.world_mut().delete_npc(priest);
            }

            if let Some(stone) = stone {
                ctx.world_mut().delete_npc(stone);
            }

            ctx.world_mut().set_grand_boss_status(REAL_BELETH, FIGHT);
            ctx.start_quest_timer("SPAWN1", 300_000, None, None, false);
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
        if !ctx.world().is_dead(npc.object_id)
            && (npc.id == REAL_BELETH)
            && !ctx.world().is_casting_now(npc.object_id)
            && ctx.world_mut().skill_has_heal_effect(skill)
            && (ctx.get_random(100) < 80)
        {
            ctx.world_mut().set_npc_target(npc.object_id, Some(caster));
            ctx.world_mut().do_cast(npc.object_id, HORN_OF_RISING);
        }
    }

    fn on_aggro_range_enter(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId, _is_summon: bool) {
        if !ctx.world().is_dead(npc.object_id) && !ctx.world().is_casting_now(npc.object_id) {
            if (ctx.get_random(100) < 40) && player_in_range(ctx, npc.object_id, 200) {
                ctx.world_mut().do_cast(npc.object_id, BLEED);
                return;
            }

            ctx.world_mut().set_npc_target(npc.object_id, Some(player));
            ctx.world_mut().do_cast(npc.object_id, FIREBALL);
        }
    }

    fn on_spell_finished(
        &self,
        ctx: &mut ScriptCtx<'_>,
        npc: ScriptNpc,
        player: Option<ObjectId>,
        _skill: SkillKey,
    ) {
        let Some(player) = player else {
            return;
        };
        let id = npc.object_id;
        if !ctx.world().is_dead(id) && !ctx.world().is_casting_now(id) {
            if !ctx.world().is_dead(player) {
                let distance2 = distance_2d(ctx, id, player);
                if (distance2 > 890.0) && !ctx.world().is_movement_disabled(id) {
                    ctx.world_mut().set_npc_target(id, Some(player));
                    ctx.world_mut().npc_set_intention_follow(id, player);
                    let speed = {
                        let running = ctx.world().is_running(id);
                        ctx.world()
                            .stats(id)
                            .map_or(0.0, |s| if running { s.run_speed() } else { s.walk_speed() })
                    };
                    let delay = (((distance2 - 890.0) / speed) * 1000.0) as i64;
                    ctx.start_quest_timer("CAST", delay, Some(npc), None, false);
                } else if distance2 < 890.0 {
                    ctx.world_mut().set_npc_target(id, Some(player));
                    ctx.world_mut().do_cast(id, FIREBALL);
                }
                return;
            }

            if (ctx.get_random(100) < 40) && player_in_range(ctx, id, 200) {
                ctx.world_mut().do_cast(id, LIGHTENING);
                return;
            }

            if let Some(fireball_target) = first_player_in_range(ctx, id, 950) {
                ctx.world_mut().set_npc_target(id, Some(fireball_target));
                ctx.world_mut().do_cast(id, FIREBALL);
                return;
            }

            ctx.world_mut().clear_aggro_list(id);
        }
    }

    fn on_spawn(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc) {
        ctx.set_running(npc);
        if player_in_range(ctx, npc.object_id, 300) && (ctx.get_random(100) < 60) {
            ctx.world_mut().do_cast(npc.object_id, BLEED);
        }

        if npc.id == REAL_BELETH
            && let Some(index) = ctx.world().npc_spawn_index(npc.object_id)
        {
            ctx.world_mut().set_spawn_respawn_delay(index, 0);
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let html;
        if ctx.state::<BelethState>().killer == Some(player) {
            ctx.state::<BelethState>().killer = None;
            ctx.give_items(player, RING, 1);
            html = "32470a.htm";
        } else {
            html = "32470b.htm";
        }

        ctx.world().get_htm(player, &format!("data/html/default/{html}"))
    }

    fn on_first_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        self.on_talk(ctx, npc, player)
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
        if ctx.get_random(100) < 40 {
            return;
        }

        let distance = distance_2d(ctx, npc.object_id, attacker);
        if (distance > 500.0) || (ctx.get_random(100) < 80) {
            let minions = ctx.state::<BelethState>().minions.clone();
            for beleth in minions {
                if !ctx.world().is_dead(beleth)
                    && ctx.check_if_in_range(900, beleth, attacker, false)
                    && !ctx.world().is_casting_now(beleth)
                {
                    ctx.world_mut().set_npc_target(beleth, Some(attacker));
                    ctx.world_mut().do_cast(beleth, FIREBALL);
                }
            }

            if let Some(beleth) = ctx.state::<BelethState>().beleth
                && !ctx.world().is_dead(beleth)
                && ctx.check_if_in_range(900, beleth, attacker, false)
                && !ctx.world().is_casting_now(beleth)
            {
                ctx.world_mut().set_npc_target(beleth, Some(attacker));
                ctx.world_mut().do_cast(beleth, FIREBALL);
            }
        } else if !ctx.world().is_dead(npc.object_id) && !ctx.world().is_casting_now(npc.object_id) {
            if player_in_range(ctx, npc.object_id, 200) {
                ctx.world_mut().do_cast(npc.object_id, LIGHTENING);
                return;
            }

            ctx.world_mut().clear_aggro_list(npc.object_id);
        }
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        if npc.id == REAL_BELETH {
            ctx.cancel_quest_timer("CHECK_ATTACK", None, None);
            set_beleth_killer(ctx, killer);
            ctx.world_mut().set_grand_boss_status(REAL_BELETH, DEAD);

            let grand_boss = &ctx.world().boss_settings().grand_boss;
            let base_interval_millis = i64::from(grand_boss.beleth_spawn_interval) * 3_600_000;
            let random_range_millis = i64::from(grand_boss.beleth_spawn_random) * 3_600_000;
            let respawn_time = base_interval_millis
                + ctx
                    .world_mut()
                    .rng()
                    .get_i64(-random_range_millis, random_range_millis);
            let respawn_at = ctx.now_ms() + respawn_time;
            if let Some(mut info) = ctx.world().grand_boss_info(REAL_BELETH).cloned() {
                info.respawn_time = respawn_at;
                ctx.world_mut().set_grand_boss_info(REAL_BELETH, info);
            }
            ctx.start_quest_timer("BELETH_UNLOCK", respawn_time, None, None, false);
            delete_all(ctx);
            ctx.world_mut().delete_npc(npc.object_id);

            for c in zone_characters(ctx) {
                ctx.world_mut().disable_all_skills(c);
                ctx.world_mut().set_invul(c, true);
                ctx.world_mut().set_immobilized(c, true);
            }

            let beleth = ctx.add_spawn(REAL_BELETH, 16323, 213170, -9357, 49152, false, 0, 0);
            ctx.state::<BelethState>().beleth = beleth.map(|n| n.object_id);
            if let Some(beleth) = beleth {
                freeze(ctx, beleth.object_id);
            }

            // `_priest` (the elf) appears at SPAWN30 (see the header).
            let stone = ctx.add_spawn(STONE_COFFIN, 12470, 215607, -9381, 49152, false, 0, 0);
            ctx.state::<BelethState>().stone = stone.map(|n| n.object_id);

            ctx.state::<BelethState>().killed_count = 0;

            ctx.start_quest_timer("SPAWN26", 1000, None, None, false);
        } else if npc.object_id.0 == ctx.state::<BelethState>().allowed_obj_id {
            delete_all(ctx);

            let state = ctx.state::<BelethState>();
            state.killed_count += 1;
            if state.killed_count >= 5 {
                ctx.start_quest_timer("SPAWN_REAL", 60_000, None, None, false);
            } else {
                ctx.start_quest_timer("SPAWN25", 60_000, None, None, false);
            }
        }
    }
}

/// The chamber's creatures: the players inside, then the NPCs.
fn zone_characters(ctx: &ScriptCtx<'_>) -> Vec<ObjectId> {
    let (zone_id, zone_type) = ZONE;
    ctx.world().zone_characters_inside(zone_id, zone_type)
}

/// `c.enableAllSkills(); c.setInvul(false); c.setImmobilized(false);` for everybody inside.
fn release_zone(ctx: &mut ScriptCtx<'_>) {
    for c in zone_characters(ctx) {
        ctx.world_mut().enable_all_skills(c);
        ctx.world_mut().set_invul(c, false);
        ctx.world_mut().set_immobilized(c, false);
    }
}

/// `npc.disableAllSkills(); npc.setInvul(true); npc.setImmobilized(true);`.
fn freeze(ctx: &mut ScriptCtx<'_>, id: ObjectId) {
    ctx.world_mut().disable_all_skills(id);
    ctx.world_mut().set_invul(id, true);
    ctx.world_mut().set_immobilized(id, true);
}

fn broadcast_zone(ctx: &ScriptCtx<'_>, packet: &impl l2_net::Encode) {
    let (zone_id, zone_type) = ZONE;
    ctx.world().broadcast_to_zone(zone_id, zone_type, packet);
}

/// `PlaySound(1, file, 1, objectId, x, y, z)`.
fn sound(file: &str, object_id: ObjectId, (x, y, z): (i32, i32, i32)) -> PlaySound {
    PlaySound {
        unknown1: 1,
        sound_file: file.to_owned(),
        unknown3: 1,
        unknown4: object_id.0,
        unknown5: x,
        unknown6: y,
        unknown7: z,
        unknown8: 0,
    }
}

/// `ZONE.broadcastPacket(new SpecialCamera(creature, force, angle1, angle2, time, range, duration,
/// relYaw, relPitch, isWide, relAngle))`: Java's 11-argument constructor, whose packet duration is
/// `range` (`SpecialCamera::from_range_duration` keeps that quirk).
#[allow(clippy::too_many_arguments)]
fn camera(
    ctx: &ScriptCtx<'_>,
    creature: ObjectId,
    force: i32,
    angle1: i32,
    angle2: i32,
    time: i32,
    range: i32,
    duration: i32,
    rel_yaw: i32,
    rel_pitch: i32,
    is_wide: i32,
    rel_angle: i32,
) {
    broadcast_zone(
        ctx,
        &SpecialCamera::from_range_duration(
            creature.0, force, angle1, angle2, time, range, duration, rel_yaw, rel_pitch, is_wide, rel_angle,
        ),
    );
}

/// `camera` for the camera NPC number `n` of the state.
#[allow(clippy::too_many_arguments)]
fn cam_of(
    ctx: &mut ScriptCtx<'_>,
    n: u8,
    force: i32,
    angle1: i32,
    angle2: i32,
    time: i32,
    range: i32,
    duration: i32,
    rel_yaw: i32,
    rel_pitch: i32,
    is_wide: i32,
    rel_angle: i32,
) {
    let state = ctx.state::<BelethState>();
    let id = match n {
        1 => state.camera1,
        2 => state.camera2,
        3 => state.camera3,
        _ => state.camera4,
    };
    if let Some(id) = id {
        camera(
            ctx, id, force, angle1, angle2, time, range, duration, rel_yaw, rel_pitch, is_wide, rel_angle,
        );
    }
}

/// `camera` for Beleth himself.
#[allow(clippy::too_many_arguments)]
fn cam_of_beleth(
    ctx: &mut ScriptCtx<'_>,
    force: i32,
    angle1: i32,
    angle2: i32,
    time: i32,
    range: i32,
    duration: i32,
    rel_yaw: i32,
    rel_pitch: i32,
    is_wide: i32,
    rel_angle: i32,
) {
    if let Some(id) = ctx.state::<BelethState>().beleth {
        camera(
            ctx, id, force, angle1, angle2, time, range, duration, rel_yaw, rel_pitch, is_wide, rel_angle,
        );
    }
}

/// `World.getFirstVisibleObjectInRange(npc, Player.class, range)`.
fn first_player_in_range(ctx: &ScriptCtx<'_>, npc: ObjectId, range: i32) -> Option<ObjectId> {
    ctx.world()
        .visible_objects_in_range(npc, range)
        .into_iter()
        .find(|o| ctx.world().player(*o).is_some())
}

fn player_in_range(ctx: &ScriptCtx<'_>, npc: ObjectId, range: i32) -> bool {
    first_player_in_range(ctx, npc, range).is_some()
}

/// `npc.calculateDistance2D(other)`.
fn distance_2d(ctx: &ScriptCtx<'_>, a: ObjectId, b: ObjectId) -> f64 {
    match (ctx.world().position_of(a), ctx.world().position_of(b)) {
        (Some((ax, ay, _)), Some((bx, by, _))) => {
            let (dx, dy) = (f64::from(ax - bx), f64::from(ay - by));
            (dx * dx + dy * dy).sqrt()
        }
        _ => f64::MAX,
    }
}

/// `setBelethKiller(killer)`: the command channel leader, else the party leader, else the killer.
fn set_beleth_killer(ctx: &mut ScriptCtx<'_>, killer: ObjectId) {
    let world = ctx.world();
    let chosen = match world.party_of(killer) {
        Some(party) => match world.command_channel_of(killer) {
            Some(channel) => world.command_channel_leader(channel),
            None => world.party_leader(party),
        },
        None => Some(killer),
    };
    ctx.state::<BelethState>().killer = chosen;
}

/// `deleteAll()`.
fn delete_all(ctx: &mut ScriptCtx<'_>) {
    let minions = std::mem::take(&mut ctx.state::<BelethState>().minions);
    for minion in minions {
        if !ctx.world().is_dead(minion) {
            ctx.world_mut().abort_cast(minion);
            ctx.world_mut().set_npc_target(minion, None);
            ctx.world_mut().npc_set_intention_idle(minion);
            ctx.world_mut().delete_npc(minion);
        }
    }

    ctx.state::<BelethState>().allowed_obj_id = 0;
}

/// The `SPAWN25` wave: sixteen fake Belethes on a ring, eight on the corners and edges of the
/// square, eight more between; one of them is the one whose death ends the wave.
fn spawn_fakes(ctx: &mut ScriptCtx<'_>) {
    ctx.state::<BelethState>().minions.clear();

    let mut a = 0;
    for i in 0..16 {
        a += 1;

        let angle = f64::from(i) * 0.39;
        let x = ((650.0 * angle.cos()) + 16323.0) as i32;
        let y = ((650.0 * angle.sin()) + 213170.0) as i32;
        if let Some(spawn) = ctx.add_spawn(FAKE_BELETH, x, y, -9357, 49152, false, 0, 0) {
            ctx.state::<BelethState>().minions.push(spawn.object_id);

            if a >= 2 {
                // `setOverloaded(true)`: see the header.
                ctx.world_mut().set_immobilized(spawn.object_id, true);
                a = 0;
            }
        }
    }

    let mut xm = [0_i32; 16];
    let mut ym = [0_i32; 16];
    for i in 0..4 {
        let angle = (f64::from(i) * 1.57) + 0.78;
        xm[i as usize] = ((1700.0 * angle.cos()) + 16323.0) as i32;
        ym[i as usize] = ((1700.0 * angle.sin()) + 213170.0) as i32;
        if let Some(spawn) = ctx.add_spawn(FAKE_BELETH, xm[i as usize], ym[i as usize], -9357, 49152, false, 0, 0) {
            ctx.world_mut().set_immobilized(spawn.object_id, true);
            ctx.state::<BelethState>().minions.push(spawn.object_id);
        }
    }

    // The midpoints: 4..7 between the corners (immobilized), 8..15 between those.
    let mids: [(usize, usize, usize, bool); 12] = [
        (4, 0, 1, true),
        (5, 1, 2, true),
        (6, 2, 3, true),
        (7, 3, 0, true),
        (8, 0, 4, false),
        (9, 4, 1, false),
        (10, 1, 5, false),
        (11, 5, 2, false),
        (12, 2, 6, false),
        (13, 6, 3, false),
        (14, 3, 7, false),
        (15, 7, 0, false),
    ];
    for (n, p, q, immobilized) in mids {
        xm[n] = (xm[p] + xm[q]) / 2;
        ym[n] = (ym[p] + ym[q]) / 2;
        if let Some(spawn) = ctx.add_spawn(FAKE_BELETH, xm[n], ym[n], -9357, 49152, false, 0, 0) {
            if immobilized {
                ctx.world_mut().set_immobilized(spawn.object_id, true);
            }
            ctx.state::<BelethState>().minions.push(spawn.object_id);
        }
    }

    let minions = ctx.state::<BelethState>().minions.clone();
    let allowed = ctx.get_random_entry(&minions).map_or(0, |m| m.0);
    ctx.state::<BelethState>().allowed_obj_id = allowed;
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::{BossReadKind, BossReadThen, BossWrite, GrandBossData, ScriptRegistry};

    use super::*;
    use crate::ai::bosses::test_support::*;
    use crate::testing::{ScriptTest, names_to};

    const MONSTER: &str = r#"<stats str="40" int="21" dex="30" wit="20" con="43" men="20"><vitals hp="1000" hpRegen="2" mp="500" mpRegen="0.9" /><attack physical="8" magical="5" random="30" critical="4" accuracy="4.75" attackSpeed="253" type="SWORD" range="40" distance="80" width="120" /><defence physical="44.44444" magical="29.5" /><speed><walk ground="20" /><run ground="50" /></speed></stats><collision><radius normal="10"/><height normal="20"/></collision>"#;

    fn npcs() -> String {
        let mut out = format!(
            r#"<npc id="29118" type="GrandBoss" level="83" name="Beleth" title="">{MONSTER}<corpseTime>21</corpseTime></npc>
<npc id="29119" type="Monster" level="83" name="Beleth">{MONSTER}</npc>
<npc id="32470" type="Folk" level="70" name="Stone Coffin"/>
<npc id="29128" type="Folk" level="70" name="Elf"/>"#
        );
        for id in [29125, 29120, 29121, 29122, 29123] {
            out.push_str(&format!(
                r#"<npc id="{id}" type="Folk" level="70" name="Beleth helper"/>"#
            ));
        }
        out
    }

    const HOME: (i32, i32, i32) = (16323, 213170, -9357);

    fn pages() -> Vec<(&'static str, &'static str)> {
        vec![
            (
                "data/html/default/32470a.htm",
                "<html><body>ring in the coffin</body></html>",
            ),
            (
                "data/html/default/32470b.htm",
                "<html><body>empty coffin</body></html>",
            ),
        ]
    }

    fn setup_row(status: i32, respawn_time: i64, writes: bool) -> ScriptTest {
        let mut registry = ScriptRegistry::new();
        registry.add(Beleth);
        let zone = r#"<?xml version="1.0" encoding="UTF-8"?>
<list enabled="true">
	<zone name="Beleth's Chamber" id="12018" type="BossZone" shape="NPoly" minZ="-10412" maxZ="-5112">
		<stat name="InvadeTime" val="1800000" />
		<node X="11214" Y="205578" />
		<node X="20607" Y="205578" />
		<node X="20648" Y="220271" />
		<node X="11172" Y="220271" />
	</zone>
</list>
"#;
        let mut pages = pages();
        pages.push(("data/zones/beleth.xml", zone));
        boss_test(
            registry,
            &npcs(),
            "<item id=\"10314\" type=\"EtcItem\" name=\"Beleth's Ring\"/>",
            &pages,
            vec![row(REAL_BELETH, status, respawn_time, HOME)],
            writes,
        )
    }

    fn status(t: &ScriptTest) -> Option<i32> {
        t.world.grand_boss_status(REAL_BELETH)
    }

    fn has_timer(t: &mut ScriptTest, name: &str) -> bool {
        let mut found = false;
        t.world.run_as_script("Beleth", |ctx| {
            found = ctx.has_quest_timer(name, None, None);
        });
        found
    }

    fn alive(t: &ScriptTest, npc_id: i32) -> Vec<Oid> {
        npcs_of(t, npc_id)
            .into_iter()
            .filter(|m| t.world.object(*m).is_some())
            .collect()
    }

    fn fire(t: &mut ScriptTest, event: &str) {
        t.world.run_as_script("Beleth", |ctx| {
            ctx.notify_event(event, None, None);
        });
    }

    #[test]
    fn the_constructor_resets_the_row_to_alive() {
        // A fight that was running is over after a restart (writes on).
        let t = setup_row(FIGHT, 0, true);
        assert_eq!(status(&t), Some(ALIVE));
        // A dead row waits for its time, or wakes up.
        let mut t = setup_row(DEAD, now_ms() + 3_600_000, true);
        assert_eq!(status(&t), Some(DEAD));
        assert!(has_timer(&mut t, "BELETH_UNLOCK"));
        let t = setup_row(DEAD, now_ms() - 10, true);
        assert_eq!(status(&t), Some(ALIVE));
    }

    #[test]
    fn with_the_switch_off_the_row_decides() {
        // The other server's fight stays as it is.
        let mut t = setup_row(FIGHT, 0, false);
        assert_eq!(status(&t), Some(FIGHT));
        assert!(grand_writes(&t.take_boss()).is_empty());
        // A dead row whose time is over asks the database.
        let mut t = setup_row(DEAD, now_ms() - 10, false);
        let then = BossReadThen::ScriptEvent {
            script: "Beleth",
            event: EVENT_ROW_READ,
        };
        assert_eq!(
            rereads(&t.take_boss()),
            [(BossReadKind::GrandBoss(REAL_BELETH), then)]
        );
        answer(&mut t, then, REAL_BELETH, row(REAL_BELETH, ALIVE, 0, HOME));
        assert_eq!(status(&t), Some(ALIVE));
        assert!(grand_writes(&t.take_boss()).is_empty());
    }

    /// The first player walks in with the status at INIT: the fight starts after five minutes and
    /// the movie plays.
    fn start_fight(t: &mut ScriptTest) {
        t.world.set_grand_boss_status(REAL_BELETH, INIT);
        t.world.allow_boss_zone_entry(ZONE.0, Oid(1), 3600);
        t.enter_with(player_at(1, (16323, 212000, -9357), vec![]));
        assert_eq!(status(t), Some(FIGHT));
        assert!(has_timer(t, "SPAWN1"));
    }

    #[test]
    fn the_opening_movie_ends_with_the_fake_belethes_and_a_free_chamber() {
        let mut t = setup_row(ALIVE, 0, true);
        start_fight(&mut t);
        t.take();
        run_ms(&mut t, 300_000 + 400);
        // SPAWN1: the four cameras and the first camera moves.
        assert_eq!(alive(&t, 29120).len(), 1);
        assert!(t.world.is_invul(Oid(1)));
        let names = names_to(&t.take(), 1);
        assert!(names.contains(&"PLAY_SOUND".to_owned()));
        // Two at once, one more at SPAWN2 (300 ms).
        assert_eq!(names.iter().filter(|n| *n == "SPECIAL_CAMERA").count(), 3);
        // The whole movie takes about 70 seconds.
        run_ms(&mut t, 70_000);
        assert!(alive(&t, REAL_BELETH).is_empty());
        assert!(alive(&t, 29120).is_empty());
        assert!(!t.world.is_invul(Oid(1)));
        assert!(has_timer(&mut t, "CHECK_ATTACK"));
        assert!(has_timer(&mut t, "SPAWN25"));
        // SPAWN25: sixteen on the ring, eight on the square, twelve between (36 - 4 + ...).
        run_ms(&mut t, 60_000);
        let fakes = alive(&t, FAKE_BELETH);
        assert_eq!(fakes.len(), 16 + 4 + 12);
        let allowed = {
            let mut id = 0;
            t.world.run_as_script("Beleth", |ctx| id = ctx.state::<BelethState>().allowed_obj_id);
            id
        };
        assert!(fakes.iter().any(|f| f.0 == allowed));
    }

    #[test]
    fn killing_the_allowed_fake_calls_the_next_wave_and_the_fifth_the_real_beleth() {
        let mut t = setup_row(ALIVE, 0, true);
        start_fight(&mut t);
        fire(&mut t, "SPAWN25");
        for wave in 1..=5 {
            let allowed = {
                let mut id = 0;
                t.world.run_as_script("Beleth", |ctx| id = ctx.state::<BelethState>().allowed_obj_id);
                Oid(id)
            };
            assert!(t.world.do_die(allowed, Some(Oid(1))), "wave {wave}");
            run_ms(&mut t, 2_600);
            // `deleteAll()`: the survivors are gone at once.
            assert!(alive(&t, FAKE_BELETH).is_empty());
            if wave < 5 {
                assert!(has_timer(&mut t, "SPAWN25"));
                fire(&mut t, "SPAWN25");
                t.world.run_as_script("Beleth", |ctx| ctx.cancel_quest_timers("SPAWN25"));
            } else {
                assert!(has_timer(&mut t, "SPAWN_REAL"));
            }
        }
        run_ms(&mut t, 60_000);
        let real = alive(&t, REAL_BELETH);
        assert_eq!(real.len(), 1);
        assert_eq!(t.world.grand_boss_npc(REAL_BELETH), Some(real[0]));
    }

    #[test]
    fn the_real_beleth_s_death_writes_the_row_and_the_coffin_gives_the_ring_once() {
        let mut t = setup_row(ALIVE, 0, true);
        start_fight(&mut t);
        fire(&mut t, "SPAWN_REAL");
        let real = t.world.grand_boss_npc(REAL_BELETH).unwrap();
        t.take_boss();
        let before = now_ms();
        assert!(t.world.do_die(real, Some(Oid(1))));
        run_ms(&mut t, 2_600);
        assert_eq!(status(&t), Some(DEAD));
        assert!(has_timer(&mut t, "BELETH_UNLOCK"));
        assert!(t.world.is_invul(Oid(1)));
        let writes = grand_writes(&t.take_boss());
        assert_eq!(
            writes[0],
            BossWrite::GrandBossStatus {
                boss_id: REAL_BELETH,
                status: DEAD
            }
        );
        let BossWrite::GrandBoss(GrandBossData { respawn_time, .. }) = writes[1].clone() else {
            panic!("{writes:?}");
        };
        let hour = 3_600_000;
        assert!(respawn_time >= before + 44 * hour && respawn_time <= now_ms() + 340 * hour);
        // The stone coffin is there; the movie runs and frees everybody.
        assert_eq!(alive(&t, STONE_COFFIN).len(), 1);
        run_ms(&mut t, 30_000);
        assert!(!t.world.is_invul(Oid(1)));
        assert_eq!(alive(&t, ELF).len(), 1);

        // The killer takes the ring once.
        let coffin = alive(&t, STONE_COFFIN)[0];
        let mut pages = Vec::new();
        t.world.run_as_script("Beleth", |ctx| {
            let npc = ctx.script_npc(coffin).unwrap();
            pages.push(Beleth.on_talk(ctx, npc, Oid(1)));
            pages.push(Beleth.on_talk(ctx, npc, Oid(1)));
        });
        assert!(pages[0].as_deref().unwrap().contains("ring in the coffin"));
        assert!(pages[1].as_deref().unwrap().contains("empty coffin"));
        assert_eq!(t.count(1, 10314), 1);
    }

    #[test]
    fn fifteen_minutes_after_the_movie_the_chamber_is_reset() {
        let mut t = setup_row(ALIVE, 0, true);
        start_fight(&mut t);
        fire(&mut t, "SPAWN24");
        t.world.run_as_script("Beleth", |ctx| {
            ctx.state::<BelethState>().last_attack = 0;
        });
        fire(&mut t, "SPAWN25");
        assert!(!alive(&t, FAKE_BELETH).is_empty());
        fire(&mut t, "CHECK_ATTACK");
        assert_eq!(status(&t), Some(ALIVE));
        assert!(alive(&t, FAKE_BELETH).is_empty());
        assert!(!has_timer(&mut t, "CHECK_ATTACK"));
    }
}
