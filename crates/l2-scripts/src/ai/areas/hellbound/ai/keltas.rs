//! Keltas: spawns and despawns Darion's Enforcers and Executioners with him.
// Java: dist/game/data/scripts/ai/areas/Hellbound/AI/Keltas.java

use l2_protocol_game::NpcStringId;
use l2_protocol_game::server::ChatType;
use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const KELTAS: i32 = 22341;
const ENFORCER: i32 = 22342;
const EXECUTIONER: i32 = 22343;

// Locations
const ENFORCER_SPAWN_POINTS: [(i32, i32, i32); 26] = [
    (-24540, 251404, -3320),
    (-24100, 252578, -3060),
    (-24607, 252443, -3074),
    (-23962, 252041, -3275),
    (-24381, 252132, -3090),
    (-23652, 251838, -3370),
    (-23838, 252603, -3095),
    (-23257, 251671, -3360),
    (-27127, 251106, -3523),
    (-27118, 251203, -3523),
    (-27052, 251205, -3523),
    (-26999, 250818, -3523),
    (-29613, 252888, -3523),
    (-29765, 253009, -3523),
    (-29594, 252570, -3523),
    (-29770, 252658, -3523),
    (-27816, 252008, -3527),
    (-27930, 252011, -3523),
    (-28702, 251986, -3523),
    (-27357, 251987, -3527),
    (-28859, 251081, -3527),
    (-28607, 250397, -3523),
    (-28801, 250462, -3523),
    (-29123, 250387, -3472),
    (-25376, 252368, -3257),
    (-25376, 252208, -3257),
];
const EXECUTIONER_SPAWN_POINTS: [(i32, i32, i32); 25] = [
    (-24419, 251395, -3340),
    (-24912, 252160, -3310),
    (-25027, 251941, -3300),
    (-24127, 252657, -3058),
    (-25120, 252372, -3270),
    (-24456, 252651, -3060),
    (-24844, 251614, -3295),
    (-28675, 252008, -3523),
    (-27943, 251238, -3523),
    (-27827, 251984, -3523),
    (-27276, 251995, -3523),
    (-28769, 251955, -3523),
    (-27969, 251073, -3523),
    (-27233, 250938, -3523),
    (-26835, 250914, -3523),
    (-26802, 251276, -3523),
    (-29671, 252781, -3527),
    (-29536, 252831, -3523),
    (-29419, 253214, -3523),
    (-27923, 251965, -3523),
    (-28499, 251882, -3527),
    (-28194, 251915, -3523),
    (-28358, 251078, -3527),
    (-28580, 251071, -3527),
    (-28492, 250704, -3523),
];

/// Java `_spawnedKeltas` and `_spawnedMonsters` (the `Spawn`s as indices).
#[derive(Default)]
struct KeltasState {
    spawned_keltas: Option<ScriptNpc>,
    spawned_monsters: Vec<usize>,
}

/// Java `ai.areas.Hellbound.AI.Keltas`.
pub struct Keltas;

impl Keltas {
    /// Java `spawnMinions()`.
    fn spawn_minions(ctx: &mut ScriptCtx<'_>) {
        for (npc_id, points, delay) in [
            (ENFORCER, &ENFORCER_SPAWN_POINTS[..], 60),
            (EXECUTIONER, &EXECUTIONER_SPAWN_POINTS[..], 80),
        ] {
            for &(x, y, z) in points {
                // `addSpawn(npcId, loc, false, 0, false)` and then the spawn of the minion
                // respawns after `delay` seconds.
                let Some(minion) = ctx.add_spawn(npc_id, x, y, z, 0, false, 0, 0) else {
                    continue;
                };
                let Some(spawn) = ctx.world().npc_spawn_index(minion.object_id) else {
                    continue;
                };
                ctx.world_mut().set_spawn_respawn_delay(spawn, delay);
                ctx.world_mut().start_spawn_respawn(spawn);
                ctx.state::<KeltasState>().spawned_monsters.push(spawn);
            }
        }
    }

    /// Java `despawnMinions()`.
    fn despawn_minions(ctx: &mut ScriptCtx<'_>) {
        if ctx.state::<KeltasState>().spawned_monsters.is_empty() {
            return;
        }

        let spawns = std::mem::take(&mut ctx.state::<KeltasState>().spawned_monsters);
        for spawn in spawns {
            ctx.world_mut().stop_spawn_respawn(spawn);
            if let Some(minion) = ctx.world().last_spawn(spawn)
                && !ctx.is_dead(minion)
                && ctx.world().npc(minion).is_some_and(|n| !n.decayed)
            {
                ctx.world_mut().delete_npc(minion);
            }
        }
    }
}

impl Script for Keltas {
    fn name(&self) -> &'static str {
        "Keltas"
    }

    fn path(&self) -> &'static str {
        "ai/areas/Hellbound/AI"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_kill_id(&[KELTAS]);
        s.add_spawn_id(&[KELTAS]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        _npc: Option<ScriptNpc>,
        _player: Option<ObjectId>,
    ) -> Option<String> {
        if event.eq_ignore_ascii_case("despawn") {
            let keltas = ctx.state::<KeltasState>().spawned_keltas;
            if let Some(keltas) = keltas
                && !ctx.is_dead(keltas.object_id)
            {
                ctx.broadcast_npc_say(
                    keltas,
                    ChatType::NpcShout,
                    NpcStringId::ThatIsItForTodayLetSRetreatEveryonePullBack,
                    &[],
                );
                // `keltas.deleteMe()` and `getSpawn().decreaseCount(keltas)`: one count less.
                ctx.world_mut().delete_npc(keltas.object_id);
                Self::despawn_minions(ctx);
            }
        }

        None
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, _killer: ObjectId, _is_summon: bool) {
        ctx.cancel_quest_timers("despawn");
        Self::despawn_minions(ctx);
    }

    fn on_spawn(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc) {
        ctx.state::<KeltasState>().spawned_keltas = Some(npc);
        ctx.broadcast_npc_say(npc, ChatType::NpcShout, NpcStringId::GuysShowThemOurPower, &[]);
        Self::spawn_minions(ctx);
        ctx.start_quest_timer("despawn", 1_800_000, None, None, false);
    }
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::ScriptTest;

    const NPCS: &str = r#"<npc id="22341" type="Monster" level="80" name="Keltas"/>
        <npc id="22342" type="Monster" level="80" name="Darion's Enforcer"/>
        <npc id="22343" type="Monster" level="80" name="Darion's Executioner"/>"#;

    fn setup() -> ScriptTest {
        let mut registry = ScriptRegistry::new();
        registry.add(Keltas);
        let mut t = ScriptTest::new(registry, NPCS, "", &[]);
        t.enter(1, 80, vec![]);
        t
    }

    #[test]
    fn keltas_brings_fifty_one_guards_and_the_timer() {
        let mut t = setup();
        crate::ai::areas::hellbound::testing::spawn(&mut t, KELTAS, 20);
        assert_eq!(t.npcs_of(ENFORCER).len(), 26);
        assert_eq!(t.npcs_of(EXECUTIONER).len(), 25);
        t.world.run_as_script("Keltas", |ctx| {
            assert!(ctx.has_quest_timer("despawn", None, None));
        });
    }

    #[test]
    fn the_guards_go_when_keltas_dies() {
        let mut t = setup();
        let keltas = crate::ai::areas::hellbound::testing::spawn(&mut t, KELTAS, 20);
        t.kill(1, keltas);
        assert!(t.npcs_of(ENFORCER).is_empty());
        assert!(t.npcs_of(EXECUTIONER).is_empty());
        t.world.run_as_script("Keltas", |ctx| {
            assert!(!ctx.has_quest_timer("despawn", None, None));
        });
    }

    #[test]
    fn keltas_retreats_after_half_an_hour() {
        let mut t = setup();
        let keltas = crate::ai::areas::hellbound::testing::spawn(&mut t, KELTAS, 20);
        t.wait_ms(1_800_100);
        assert!(!t.in_world(keltas));
        assert!(t.npcs_of(ENFORCER).is_empty());
    }

    #[test]
    fn a_dead_guard_comes_back_after_its_respawn_delay() {
        let mut t = setup();
        crate::ai::areas::hellbound::testing::spawn(&mut t, KELTAS, 20);
        let guard = t.npcs_of(ENFORCER)[0];
        t.world.do_die(guard, Some(ObjectId(1)));
        t.wait_ms(70_000);
        assert_eq!(t.npcs_of(ENFORCER).len(), 26);
    }
}
