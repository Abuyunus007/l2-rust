//! Core, the Cruma Tower grand boss: the row restored at start, the minions and their one-minute
//! respawns, the voice lines, the teleport cubes on death and the respawn cycle.
// Java: dist/game/data/scripts/ai/bosses/Core/Core.java
//
// The Java constructor is `on_load`; the static fields are `CoreState` (`ctx.state`). Statuses: 0
// alive, 1 dead.
//
// Side by side (D-54, `super` module): with `CycleDatabaseWrites` off the constructor restores
// the boss only from an alive row, and `core_unlock` reads the row again and brings Core back
// only when the row says alive. `Core_Attacked` is a script variable of `global_variables`
// (written only with the switch on, D-54).
//
// Not modelled: `unload` (D-16: no reload).

use std::collections::HashMap;

use l2_protocol_game::NpcStringId;
use l2_protocol_game::server::{ChatType, PlaySound};
use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup, TeleportTarget};
use tracing::{info, warn};

use super::{EVENT_ROW_READ, date_time_string, row_lets_boss_run};

// NPC identifiers.
const CORE_NPC_ID: i32 = 29006;
const MINION_DEATH_KNIGHT_ID: i32 = 29007;
const MINION_DOOM_WRAITH_ID: i32 = 29008;
const MINION_SUSCEPTOR_ID: i32 = 29011;
const TELEPORT_CUBE_NPC_ID: i32 = 899099;

// Boss status flags.
const STATUS_ALIVE: i32 = 0;
const STATUS_DEAD: i32 = 1;

// Event identifiers.
const EVENT_CORE_UNLOCK: &str = "core_unlock";
const EVENT_SPAWN_MINION_LEGACY: &str = "spawn_minion";
const EVENT_DESPAWN_MINIONS: &str = "despawn_minions";
const EVENT_TELEPORT_PREFIX: &str = "TELEPORT";
const EVENT_SPAWN_MINION_PREFIX: &str = "spawn_minion_";

// Timing constants.
const MILLIS_PER_HOUR: i64 = 3_600_000;
const MINION_RESPAWN_DELAY_MS: i64 = 60_000;
const MINION_DESPAWN_DELAY_MS: i64 = 20_000;
const TELEPORT_CUBE_DESPAWN_MS: i64 = 900_000;

// Teleport destinations.
const LOCATION_CRUMA_ENTRANCE: (i32, i32, i32) = (17253, 114232, -3440); // Cruma Tower entrance.
const LOCATION_CRUMA_CORE_FLOOR3: (i32, i32, i32) = (17719, 115590, -6584); // Cruma Core (3rd floor).

// Core spawn position.
const CORE_DEFAULT_SPAWN: (i32, i32, i32) = (17726, 108915, -6480);

/// Java `MINION_SPAWN_POINTS` in the order a `HashMap<Integer, ..>` iterates the three keys
/// (bucket = id mod 16 of a 16-slot table: 29008 -> 0, 29011 -> 3, 29007 -> 15).
// NOTE(java-quirk): the minions spawn in that hash order, not in the order they are added.
const MINION_SPAWN_POINTS: [(i32, &[(i32, i32, i32)]); 3] = [
    (
        MINION_DOOM_WRAITH_ID,
        &[
            (17113, 110970, -6648),
            (17496, 110880, -6648),
            (18061, 110990, -6648),
            (18384, 110698, -6648),
            (17993, 111458, -6584),
        ],
    ),
    (
        MINION_SUSCEPTOR_ID,
        &[
            (17297, 111470, -6584),
            (17893, 110198, -6648),
            (17706, 109423, -6488),
            (17849, 109388, -6480),
        ],
    ),
    (
        MINION_DEATH_KNIGHT_ID,
        &[
            (17191, 109298, -6488),
            (17564, 109548, -6488),
            (17855, 109552, -6488),
            (18280, 109202, -6488),
            (18784, 109253, -6488),
            (18059, 108314, -6488),
            (17300, 108444, -6488),
            (17148, 110071, -6648),
            (18318, 110077, -6648),
            (17726, 110391, -6648),
        ],
    ),
];

/// Java `MinionRespawnData`: the NPC id and the place of a dead minion.
#[derive(Debug, Clone, Copy)]
pub(crate) struct MinionRespawnData {
    npc_id: i32,
    location: (i32, i32, i32),
}

/// The static fields of the Java class.
#[derive(Debug, Default)]
pub(crate) struct CoreState {
    /// `_minions`.
    pub(crate) minions: Vec<ObjectId>,
    /// `_pendingRespawns`.
    pub(crate) pending_respawns: HashMap<i32, MinionRespawnData>,
    /// `_respawnIdGenerator`.
    pub(crate) respawn_id_generator: i32,
    /// `_firstAttacked`.
    pub(crate) first_attacked: bool,
}

/// Java `ai.bosses.Core.Core`.
pub struct Core;

impl Script for Core {
    fn name(&self) -> &'static str {
        "Core"
    }

    fn path(&self) -> &'static str {
        "ai/bosses/Core"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_kill_id(&[
            CORE_NPC_ID,
            MINION_DEATH_KNIGHT_ID,
            MINION_DOOM_WRAITH_ID,
            MINION_SUSCEPTOR_ID,
        ]);
        s.add_attack_id(&[CORE_NPC_ID]);
        s.add_spawn_id(&[CORE_NPC_ID]);
        s.add_start_npc(&[TELEPORT_CUBE_NPC_ID]);
        s.add_first_talk_id(&[TELEPORT_CUBE_NPC_ID]);
        s.add_talk_id(&[TELEPORT_CUBE_NPC_ID]);
    }

    /// The constructor's second half.
    fn on_load(&self, ctx: &mut ScriptCtx<'_>) {
        ctx.state::<CoreState>().first_attacked = false;

        let Some(info) = ctx.world().grand_boss_info(CORE_NPC_ID).cloned() else {
            // Java: `getStatSet` answers null and the constructor throws.
            warn!("Core: no grandboss_data row for {CORE_NPC_ID}, the script does not start.");
            return;
        };
        let status = ctx.world().grand_boss_status(CORE_NPC_ID).unwrap_or(-1);
        if status == STATUS_DEAD {
            let remaining = info.respawn_time - ctx.now_ms();
            if remaining > 0 {
                ctx.start_quest_timer(EVENT_CORE_UNLOCK, remaining, None, None, false);
            } else if !ctx.world().boss_writes_enabled() {
                // D-54: the respawn of a dead row waits for the other server's alive row.
                ctx.notify_event(EVENT_CORE_UNLOCK, None, None);
            } else {
                spawn_core_at_default(ctx);
            }
        } else {
            // D-54: with the switch off a row that is not alive is the other server's.
            if !row_lets_boss_run(ctx, CORE_NPC_ID) {
                info!("Core: the shared state row says {status}; the AI waits for an alive row (D-54).");
                return;
            }
            let attacked = ctx.global_variables().get_bool("Core_Attacked", false);
            if attacked {
                ctx.state::<CoreState>().first_attacked = true;
            }

            let Some(core) =
                ctx.add_spawn(CORE_NPC_ID, info.loc_x, info.loc_y, info.loc_z, info.heading, false, 0, 0)
            else {
                return;
            };
            ctx.world_mut()
                .set_current_hp_mp(core.object_id, info.current_hp, info.current_mp);
            spawn_boss(ctx, core);
        }
    }

    /// Persists Core attack state to global variables.
    fn on_save(&self, ctx: &mut ScriptCtx<'_>) {
        let attacked = ctx.state::<CoreState>().first_attacked;
        ctx.set_global_variable("Core_Attacked", if attacked { "true" } else { "false" });
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        // Robust minion respawn events of the form "spawn_minion_X".
        if let Some(id_token) = event.strip_prefix(EVENT_SPAWN_MINION_PREFIX) {
            match id_token.parse::<i32>() {
                Ok(respawn_id) => {
                    let data = ctx.state::<CoreState>().pending_respawns.remove(&respawn_id);
                    if let Some(data) = data
                        && ctx.world().grand_boss_status(CORE_NPC_ID) == Some(STATUS_ALIVE)
                    {
                        let (x, y, z) = data.location;
                        if let Some(minion) = ctx.add_spawn(data.npc_id, x, y, z, 0, false, 0, 0) {
                            ctx.world_mut().set_is_raid_minion(minion.object_id, true);
                            ctx.state::<CoreState>().minions.push(minion.object_id);
                        }
                    }
                }
                Err(_) => {
                    warn!("Core: Failed to parse minion respawn id from event '{event}'.");
                }
            }
        } else if event == EVENT_CORE_UNLOCK {
            // D-54: on the shared database the row decides; read it again first.
            if !ctx.world().boss_writes_enabled() {
                ctx.world_mut()
                    .reread_grand_boss_for_script(CORE_NPC_ID, self.name(), EVENT_ROW_READ);
                return None;
            }
            spawn_core_at_default(ctx);
        } else if event == EVENT_ROW_READ {
            // D-54: the answer of the read `core_unlock` asked for.
            match ctx.world().grand_boss_status(CORE_NPC_ID) {
                Some(STATUS_ALIVE) => spawn_core_at_default(ctx),
                Some(STATUS_DEAD) => {
                    let respawn = ctx.world().grand_boss_info(CORE_NPC_ID).map_or(0, |i| i.respawn_time);
                    let remaining = respawn - ctx.now_ms();
                    if remaining > 0 {
                        ctx.start_quest_timer(EVENT_CORE_UNLOCK, remaining, None, None, false);
                    } else {
                        info!("Core: the shared state row says dead; Core stays away (D-54).");
                    }
                }
                status => info!("Core: the shared state row says {status:?}; Core stays away (D-54)."),
            }
        } else if event == EVENT_SPAWN_MINION_LEGACY {
            // Legacy simple minion respawn using NPC position from timer context.
            if let Some(npc) = npc
                && let Some((x, y, z, heading)) = ctx.npc_position(npc)
                && let Some(minion) = ctx.add_spawn(npc.id, x, y, z, heading, false, 0, 0)
            {
                ctx.world_mut().set_is_raid_minion(minion.object_id, true);
                ctx.state::<CoreState>().minions.push(minion.object_id);
            }
        } else if event == EVENT_DESPAWN_MINIONS {
            let minions = std::mem::take(&mut ctx.state::<CoreState>().minions);
            for minion in minions {
                ctx.world_mut().decay(minion);
            }
            let state = ctx.state::<CoreState>();
            state.pending_respawns.clear();
            state.respawn_id_generator = 0;
        } else if event.starts_with(EVENT_TELEPORT_PREFIX) {
            if let Some(player) = player {
                let split: Vec<&str> = event.split(' ').collect();
                if split.len() >= 2 {
                    let destination = split[1];
                    match destination {
                        "floor3" => tele_player(ctx, player, LOCATION_CRUMA_CORE_FLOOR3),
                        "ground" => tele_player(ctx, player, LOCATION_CRUMA_ENTRANCE),
                        _ => warn!("Core: Unknown teleport destination '{destination}' in event '{event}'."),
                    }
                } else {
                    warn!("Core: TELEPORT event missing destination token: '{event}'.");
                }
            }
        }

        None
    }

    /// Handles Core attack events and broadcasts combat voice lines.
    fn on_attack(
        &self,
        ctx: &mut ScriptCtx<'_>,
        npc: ScriptNpc,
        _attacker: ObjectId,
        _damage: i32,
        _is_summon: bool,
        _skill: Option<l2_world::SkillKey>,
    ) {
        if ctx.state::<CoreState>().first_attacked {
            if ctx.get_random(100) == 0 {
                ctx.broadcast_npc_say(npc, ChatType::NpcGeneral, NpcStringId::RemovingIntruders, &[]);
            }
        } else {
            ctx.state::<CoreState>().first_attacked = true;
            ctx.broadcast_npc_say(
                npc,
                ChatType::NpcGeneral,
                NpcStringId::ANonPermittedTargetHasBeenDiscovered,
                &[],
            );
            ctx.broadcast_npc_say(
                npc,
                ChatType::NpcGeneral,
                NpcStringId::IntruderRemovalSystemInitiated,
                &[],
            );
        }
    }

    /// Handles Core and minion kill events, including respawn scheduling.
    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, _killer: ObjectId, _is_summon: bool) {
        let npc_id = npc.id;
        if npc_id == CORE_NPC_ID {
            let (x, y, z) = ctx.world().position_of(npc.object_id).unwrap_or((0, 0, 0));
            ctx.world()
                .broadcast_packet(npc.object_id, &npc_sound("BS02_D", npc.object_id, (x, y, z)));
            ctx.broadcast_npc_say(npc, ChatType::NpcGeneral, NpcStringId::AFatalErrorHasOccurred, &[]);
            ctx.broadcast_npc_say(npc, ChatType::NpcGeneral, NpcStringId::SystemIsBeingShutDown, &[]);
            ctx.broadcast_npc_say(npc, ChatType::NpcGeneral, NpcStringId::Empty, &[]);

            ctx.state::<CoreState>().first_attacked = false;

            ctx.add_spawn(TELEPORT_CUBE_NPC_ID, 16502, 110165, -6394, 0, false, TELEPORT_CUBE_DESPAWN_MS, 0);
            ctx.add_spawn(TELEPORT_CUBE_NPC_ID, 18948, 110166, -6397, 0, false, TELEPORT_CUBE_DESPAWN_MS, 0);

            ctx.world_mut().set_grand_boss_status(CORE_NPC_ID, STATUS_DEAD);

            let grand_boss = &ctx.world().boss_settings().grand_boss;
            let base_interval_millis = i64::from(grand_boss.core_spawn_interval) * MILLIS_PER_HOUR;
            let random_range_millis = i64::from(grand_boss.core_spawn_random) * MILLIS_PER_HOUR;
            let respawn_time = base_interval_millis
                + ctx
                    .world_mut()
                    .rng()
                    .get_i64(-random_range_millis, random_range_millis);

            let next_respawn_time = ctx.now_ms() + respawn_time;
            info!("Core will respawn at: {}", date_time_string(next_respawn_time));

            ctx.start_quest_timer(EVENT_CORE_UNLOCK, respawn_time, None, None, false);

            if let Some(mut info) = ctx.world().grand_boss_info(CORE_NPC_ID).cloned() {
                info.respawn_time = next_respawn_time;
                ctx.world_mut().set_grand_boss_info(CORE_NPC_ID, info);
            }

            ctx.start_quest_timer(EVENT_DESPAWN_MINIONS, MINION_DESPAWN_DELAY_MS, None, None, false);

            let generated = ctx.state::<CoreState>().respawn_id_generator;
            for i in 0..generated {
                ctx.cancel_quest_timer(&format!("{EVENT_SPAWN_MINION_PREFIX}{i}"), None, None);
            }

            let state = ctx.state::<CoreState>();
            state.pending_respawns.clear();
            state.respawn_id_generator = 0;

            // Cancel legacy spawn_minion timers as well.
            ctx.cancel_quest_timers(EVENT_SPAWN_MINION_LEGACY);
        } else if ctx.world().grand_boss_status(CORE_NPC_ID) == Some(STATUS_ALIVE) {
            let state = ctx.state::<CoreState>();
            let Some(index) = state.minions.iter().position(|m| *m == npc.object_id) else {
                return;
            };
            state.minions.remove(index);
            let respawn_id = state.respawn_id_generator;
            state.respawn_id_generator += 1;
            let (x, y, z) = ctx.world().position_of(npc.object_id).unwrap_or((0, 0, 0));
            ctx.state::<CoreState>().pending_respawns.insert(
                respawn_id,
                MinionRespawnData {
                    npc_id,
                    location: (x, y, z),
                },
            );
            ctx.start_quest_timer(
                &format!("{EVENT_SPAWN_MINION_PREFIX}{respawn_id}"),
                MINION_RESPAWN_DELAY_MS,
                None,
                None,
                false,
            );
        }
    }

    fn on_spawn(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc) {
        ctx.set_immobilized(npc, true);
    }

    fn on_first_talk(&self, _ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, _player: ObjectId) -> Option<String> {
        Some(format!("{}.html", npc.id))
    }
}

/// `PlaySound(1, file, 1, objectId, x, y, z)`.
fn npc_sound(file: &str, object_id: ObjectId, (x, y, z): (i32, i32, i32)) -> PlaySound {
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

/// `core_unlock` and the constructor of a dead row whose time is over: Core at its default place.
fn spawn_core_at_default(ctx: &mut ScriptCtx<'_>) {
    let (x, y, z) = CORE_DEFAULT_SPAWN;
    let core = ctx.add_spawn(CORE_NPC_ID, x, y, z, 0, false, 0, 0);
    ctx.world_mut().set_grand_boss_status(CORE_NPC_ID, STATUS_ALIVE);
    if let Some(core) = core {
        spawn_boss(ctx, core);
    }
}

/// Registers Core in the boss manager and spawns all configured minions.
fn spawn_boss(ctx: &mut ScriptCtx<'_>, npc: ScriptNpc) {
    ctx.world_mut().add_grand_boss(CORE_NPC_ID, npc.object_id);
    let at = ctx.world().position_of(npc.object_id).unwrap_or((0, 0, 0));
    ctx.world()
        .broadcast_packet(npc.object_id, &npc_sound("BS01_A", npc.object_id, at));

    for (minion_id, locations) in MINION_SPAWN_POINTS {
        for (x, y, z) in locations {
            if let Some(minion) = ctx.add_spawn(minion_id, *x, *y, *z, 0, false, 0, 0) {
                ctx.world_mut().set_is_raid_minion(minion.object_id, true);
                ctx.state::<CoreState>().minions.push(minion.object_id);
            }
        }
    }

    let state = ctx.state::<CoreState>();
    state.pending_respawns.clear();
    state.respawn_id_generator = 0;
}

/// `player.teleToLocation(Location)`.
fn tele_player(ctx: &mut ScriptCtx<'_>, player: ObjectId, (x, y, z): (i32, i32, i32)) {
    ctx.world_mut()
        .tele_to_location(player, TeleportTarget::at(x, y, z).with_instance(0));
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::{BossReadKind, BossReadThen, BossWrite, GrandBossData, ScriptRegistry};

    use super::*;
    use crate::ai::bosses::test_support::*;
    use crate::testing::{ScriptTest, names_to};

    const NPCS: &str = r#"<npc id="29006" type="GrandBoss" level="75" name="Core" title="Ultimate Weapon">
    <stats str="40" int="21" dex="30" wit="20" con="43" men="20">
      <vitals hp="1000" hpRegen="2" mp="500" mpRegen="0.9" />
      <attack physical="8" magical="5" random="30" critical="4" accuracy="4.75" attackSpeed="253" type="SWORD" range="40" distance="80" width="120" />
      <defence physical="44.44444" magical="29.5" />
      <speed><walk ground="20" /><run ground="50" /></speed>
    </stats>
    <collision><radius normal="10"/><height normal="20"/></collision>
    <corpseTime>21</corpseTime>
  </npc>
  <npc id="29007" type="Monster" level="70" name="Death Knight"><stats str="40" int="21" dex="30" wit="20" con="43" men="20"><vitals hp="100" hpRegen="2" mp="50" mpRegen="0.9" /><attack physical="8" magical="5" random="30" critical="4" accuracy="4.75" attackSpeed="253" type="SWORD" range="40" distance="80" width="120" /><defence physical="44.44444" magical="29.5" /><speed><walk ground="20" /><run ground="50" /></speed></stats><collision><radius normal="10"/><height normal="20"/></collision></npc>
  <npc id="29008" type="Monster" level="70" name="Doom Wraith"><stats str="40" int="21" dex="30" wit="20" con="43" men="20"><vitals hp="100" hpRegen="2" mp="50" mpRegen="0.9" /><attack physical="8" magical="5" random="30" critical="4" accuracy="4.75" attackSpeed="253" type="SWORD" range="40" distance="80" width="120" /><defence physical="44.44444" magical="29.5" /><speed><walk ground="20" /><run ground="50" /></speed></stats><collision><radius normal="10"/><height normal="20"/></collision></npc>
  <npc id="29011" type="Monster" level="70" name="Susceptor"><stats str="40" int="21" dex="30" wit="20" con="43" men="20"><vitals hp="100" hpRegen="2" mp="50" mpRegen="0.9" /><attack physical="8" magical="5" random="30" critical="4" accuracy="4.75" attackSpeed="253" type="SWORD" range="40" distance="80" width="120" /><defence physical="44.44444" magical="29.5" /><speed><walk ground="20" /><run ground="50" /></speed></stats><collision><radius normal="10"/><height normal="20"/></collision></npc>
  <npc id="899099" type="Folk" level="70" name="Teleportation Cubic"/>"#;

    const CORE_AT: (i32, i32, i32) = (17726, 108915, -6480);

    fn setup_row(status: i32, respawn_time: i64, writes: bool) -> ScriptTest {
        let mut registry = ScriptRegistry::new();
        registry.add(Core);
        boss_test(
            registry,
            NPCS,
            "",
            &[("ai/bosses/Core/899099.html", "<html><body>Teleportation Cubic</body></html>")],
            vec![row(CORE_NPC_ID, status, respawn_time, CORE_AT)],
            writes,
        )
    }

    fn has_timer(t: &mut ScriptTest, name: &str) -> bool {
        let mut found = false;
        t.world.run_as_script("Core", |ctx| {
            found = ctx.has_quest_timer(name, None, None);
        });
        found
    }

    fn minion_count(t: &ScriptTest) -> usize {
        [MINION_DEATH_KNIGHT_ID, MINION_DOOM_WRAITH_ID, MINION_SUSCEPTOR_ID]
            .iter()
            .map(|id| {
                npcs_of(t, *id)
                    .into_iter()
                    .filter(|m| t.world.object(*m).is_some())
                    .count()
            })
            .sum()
    }

    fn status(t: &ScriptTest) -> Option<i32> {
        t.world.grand_boss_status(CORE_NPC_ID)
    }

    #[test]
    fn an_alive_row_brings_core_and_nineteen_minions_back() {
        let mut t = setup_row(STATUS_ALIVE, 0, true);
        let core = t.world.grand_boss_npc(CORE_NPC_ID).unwrap();
        let (x, y, _) = t.world.position_of(core).unwrap();
        assert_eq!((x, y), (CORE_AT.0, CORE_AT.1));
        assert!(t.world.is_immobilized(core));
        assert_eq!(npcs_of(&t, MINION_DEATH_KNIGHT_ID).len(), 10);
        assert_eq!(npcs_of(&t, MINION_DOOM_WRAITH_ID).len(), 5);
        assert_eq!(npcs_of(&t, MINION_SUSCEPTOR_ID).len(), 4);
        for id in npcs_of(&t, MINION_SUSCEPTOR_ID) {
            assert!(t.world.is_raid_minion(id));
        }
        // The constructor writes nothing.
        assert!(grand_writes(&t.take_boss()).is_empty());
    }

    #[test]
    fn a_dead_row_waits_or_spawns_core_at_its_default_place() {
        let mut t = setup_row(STATUS_DEAD, now_ms() + 3_600_000, true);
        assert!(npcs_of(&t, CORE_NPC_ID).is_empty());
        assert!(has_timer(&mut t, EVENT_CORE_UNLOCK));

        let t = setup_row(STATUS_DEAD, now_ms() - 1000, true);
        let core = t.world.grand_boss_npc(CORE_NPC_ID).unwrap();
        let (x, y, z) = t.world.position_of(core).unwrap();
        assert_eq!((x, y, z), CORE_DEFAULT_SPAWN);
        assert_eq!(status(&t), Some(STATUS_ALIVE));
        assert_eq!(minion_count(&t), 19);
    }

    #[test]
    fn with_the_switch_off_the_row_decides_the_unlock() {
        let mut t = setup_row(STATUS_DEAD, now_ms() - 1000, false);
        assert!(npcs_of(&t, CORE_NPC_ID).is_empty());
        let then = BossReadThen::ScriptEvent {
            script: "Core",
            event: EVENT_ROW_READ,
        };
        assert_eq!(
            rereads(&t.take_boss()),
            [(BossReadKind::GrandBoss(CORE_NPC_ID), then)]
        );
        answer(&mut t, then, CORE_NPC_ID, row(CORE_NPC_ID, STATUS_ALIVE, 0, CORE_AT));
        assert_eq!(status(&t), Some(STATUS_ALIVE));
        assert_eq!(npcs_of(&t, CORE_NPC_ID).len(), 1);
        assert_eq!(minion_count(&t), 19);
        assert!(grand_writes(&t.take_boss()).is_empty());

        // A row that stays dead keeps Core away.
        let mut t = setup_row(STATUS_DEAD, now_ms() - 1000, false);
        t.take_boss();
        answer(
            &mut t,
            then,
            CORE_NPC_ID,
            row(CORE_NPC_ID, STATUS_DEAD, now_ms() - 5, CORE_AT),
        );
        assert!(npcs_of(&t, CORE_NPC_ID).is_empty());

        // A dead row on the shared database does not make the AI run.
        let t = setup_row(STATUS_DEAD + 1, 0, false);
        assert!(npcs_of(&t, CORE_NPC_ID).is_empty());
    }

    #[test]
    fn the_first_attack_wakes_the_voice_lines() {
        let mut t = setup_row(STATUS_ALIVE, 0, true);
        t.enter_with(player_at(1, (CORE_AT.0 + 50, CORE_AT.1, CORE_AT.2), vec![]));
        let core = t.world.grand_boss_npc(CORE_NPC_ID).unwrap();
        t.take();
        let attack = |t: &mut ScriptTest| {
            t.world.run_as_script("Core", |ctx| {
                let npc = ctx.script_npc(core).unwrap();
                Core.on_attack(ctx, npc, Oid(1), 10, false, None);
            });
        };
        attack(&mut t);
        let names = names_to(&t.take(), 1);
        assert_eq!(names.iter().filter(|n| *n == "NPC_SAY").count(), 2);
        // The next attacks talk only at `getRandom(100) == 0`: never 2 lines at once.
        for _ in 0..300 {
            attack(&mut t);
        }
        let names = names_to(&t.take(), 1);
        assert!(names.iter().filter(|n| *n == "NPC_SAY").count() <= 10);
        // `onSave` keeps the flag in the script variables.
        t.world.run_as_script("Core", |ctx| Core.on_save(ctx));
        t.world.run_as_script("Core", |ctx| {
            assert!(ctx.global_variables().get_bool("Core_Attacked", false));
        });
    }

    #[test]
    fn core_death_leaves_cubes_a_row_and_the_minions_go() {
        let mut t = setup_row(STATUS_ALIVE, 0, true);
        t.enter_with(player_at(1, (CORE_AT.0 + 50, CORE_AT.1, CORE_AT.2), vec![]));
        let core = t.world.grand_boss_npc(CORE_NPC_ID).unwrap();
        t.take();
        t.take_boss();
        let before = now_ms();
        assert!(t.world.do_die(core, Some(Oid(1))));
        run_ms(&mut t, 2_600);
        let names = names_to(&t.take(), 1);
        assert!(names.contains(&"PLAY_SOUND".to_owned()));
        assert_eq!(names.iter().filter(|n| *n == "NPC_SAY").count(), 3);
        assert_eq!(status(&t), Some(STATUS_DEAD));
        assert_eq!(npcs_of(&t, TELEPORT_CUBE_NPC_ID).len(), 2);
        assert!(has_timer(&mut t, EVENT_CORE_UNLOCK));
        assert!(has_timer(&mut t, EVENT_DESPAWN_MINIONS));
        let writes = grand_writes(&t.take_boss());
        assert_eq!(
            writes[0],
            BossWrite::GrandBossStatus {
                boss_id: CORE_NPC_ID,
                status: STATUS_DEAD
            }
        );
        let BossWrite::GrandBoss(GrandBossData { respawn_time, .. }) = writes[1].clone() else {
            panic!("{writes:?}");
        };
        let hour = 3_600_000;
        assert!(respawn_time >= before + 36 * hour && respawn_time <= now_ms() + 84 * hour);

        run_ms(&mut t, 20_000);
        assert_eq!(minion_count(&t), 0);
    }

    #[test]
    fn a_dead_minion_comes_back_after_a_minute_but_not_after_core() {
        let mut t = setup_row(STATUS_ALIVE, 0, true);
        t.enter_with(player_at(1, (CORE_AT.0 + 50, CORE_AT.1, CORE_AT.2), vec![]));
        let minion = npcs_of(&t, MINION_SUSCEPTOR_ID)[0];
        let at = t.world.position_of(minion).unwrap();
        assert!(t.world.do_die(minion, Some(Oid(1))));
        run_ms(&mut t, 2_600);
        let state_minions = {
            let mut n = 0;
            t.world.run_as_script("Core", |ctx| n = ctx.state::<CoreState>().minions.len());
            n
        };
        assert_eq!(state_minions, 18, "{:?}", t.world.is_dead(minion));
        assert!(has_timer(&mut t, "spawn_minion_0"));
        assert_eq!(npcs_of(&t, MINION_SUSCEPTOR_ID).len(), 3);
        run_ms(&mut t, 60_000);
        let now = npcs_of(&t, MINION_SUSCEPTOR_ID);
        assert_eq!(now.len(), 4);
        assert!(now.iter().any(|m| t.world.position_of(*m) == Some(at)));
        assert!(!has_timer(&mut t, "spawn_minion_0"));

        // A second death takes the next id; Core's death cancels the pending timer.
        let minion = npcs_of(&t, MINION_DOOM_WRAITH_ID)[0];
        assert!(t.world.do_die(minion, Some(Oid(1))));
        run_ms(&mut t, 2_600);
        assert!(has_timer(&mut t, "spawn_minion_1"));
        let core = t.world.grand_boss_npc(CORE_NPC_ID).unwrap();
        assert!(t.world.do_die(core, Some(Oid(1))));
        run_ms(&mut t, 2_600);
        assert!(!has_timer(&mut t, "spawn_minion_1"));
    }

    #[test]
    fn the_cube_shows_its_page_and_the_teleports_move_the_player() {
        let mut t = setup_row(STATUS_ALIVE, 0, true);
        t.enter(1, 80, vec![]);
        let cube = t.spawn(TELEPORT_CUBE_NPC_ID, 20);
        t.talk(1, cube);
        t.take();
        t.world.run_as_script("Core", |ctx| {
            let npc = ctx.script_npc(cube).unwrap();
            assert_eq!(
                Core.on_first_talk(ctx, npc, Oid(1)).as_deref(),
                Some("899099.html")
            );
            Core.on_event(ctx, "TELEPORT floor3", Some(npc), Some(Oid(1)));
        });
        let (x, y, z) = t.world.position_of(Oid(1)).unwrap();
        // The ground height follows the geodata.
        assert_eq!((x, y), (LOCATION_CRUMA_CORE_FLOOR3.0, LOCATION_CRUMA_CORE_FLOOR3.1));
        let _ = z;
        t.world.run_as_script("Core", |ctx| {
            Core.on_event(ctx, "TELEPORT ground", None, Some(Oid(1)));
            // Unknown or missing destinations only log.
            Core.on_event(ctx, "TELEPORT nowhere", None, Some(Oid(1)));
            Core.on_event(ctx, "TELEPORT", None, Some(Oid(1)));
        });
        let (x, y, z) = t.world.position_of(Oid(1)).unwrap();
        assert_eq!((x, y), (LOCATION_CRUMA_ENTRANCE.0, LOCATION_CRUMA_ENTRANCE.1));
        let _ = z;
    }
}
