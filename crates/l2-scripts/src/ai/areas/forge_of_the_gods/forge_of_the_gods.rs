//! Forge of the Gods AI: a run of kills in a short time calls a Lavasaurus on the killer.
// Java: dist/game/data/scripts/ai/areas/ForgeOfTheGods/ForgeOfTheGods.java

use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const FOG_MOBS: [i32; 16] = [
    22634, // Scarlet Stakato Worker
    22635, // Scarlet Stakato Soldier
    22636, // Scarlet Stakato Noble
    22637, // Tepra Scorpion
    22638, // Tepra Scarab
    22639, // Assassin Beetle
    22640, // Mercenary of Destruction
    22641, // Knight of Destruction
    22642, // Lavastone Golem
    22643, // Magma Golem
    22644, // Arimanes of Destruction
    22645, // Balor of Destruction
    22646, // Ashuras of Destruction
    22647, // Lavasillisk
    22648, // Blazing Ifrit
    22649, // Magma Drake
];

const LAVASAURUSES: [i32; 5] = [
    18799, // Newborn Lavasaurus
    18800, // Fledgling Lavasaurus
    18801, // Adult Lavasaurus
    18802, // Elderly Lavasaurus
    18803, // Ancient Lavasaurus
];

const REFRESH: i64 = 15;

const MOBCOUNT_BONUS_MIN: i32 = 3;

const BONUS_UPPER_LV01: i32 = 5;
const BONUS_UPPER_LV02: i32 = 10;
const BONUS_UPPER_LV03: i32 = 15;
const BONUS_UPPER_LV04: i32 = 20;
const BONUS_UPPER_LV05: i32 = 35;

const BONUS_LOWER_LV01: i32 = 5;
const BONUS_LOWER_LV02: i32 = 10;
const BONUS_LOWER_LV03: i32 = 15;

const FORGE_BONUS01: i32 = 20;
const FORGE_BONUS02: i32 = 40;

/// Java `private static int _npcCount`: the kills since the last "refresh" (every 15 s).
#[derive(Default)]
struct KillCount(i32);

/// Java `ai.areas.ForgeOfTheGods.ForgeOfTheGods`.
pub struct ForgeOfTheGods;

/// Java `spawnLavasaurus(npc, rand, mobs...)` for the two-mob calls.
fn spawn_lavasaurus(ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, rand: i32, mobs: [i32; 2]) -> Option<ScriptNpc> {
    let mut mob = None;
    if rand <= FORGE_BONUS01 {
        mob = ctx.add_spawn_at(mobs[0], npc.object_id, true, 0);
    } else if rand <= FORGE_BONUS02 {
        mob = ctx.add_spawn_at(mobs[1], npc.object_id, true, 0);
    }

    mob
}

impl Script for ForgeOfTheGods {
    fn name(&self) -> &'static str {
        "ForgeOfTheGods"
    }

    fn path(&self) -> &'static str {
        "ai/areas/ForgeOfTheGods"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_kill_id(&FOG_MOBS);
        s.add_spawn_id(&LAVASAURUSES);
    }

    // The Java constructor starts the repeating timer.
    fn on_load(&self, ctx: &mut ScriptCtx<'_>) {
        ctx.start_quest_timer("refresh", REFRESH * 1000, None, None, true);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        _player: Option<ObjectId>,
    ) -> Option<String> {
        match event {
            "suicide" => {
                if let Some(npc) = npc {
                    ctx.world_mut().do_die(npc.object_id, None);
                }
            }
            "refresh" => {
                ctx.state::<KillCount>().0 = 0;
            }
            _ => {}
        }

        None
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        let rand = ctx.get_random(100);
        let mut mob = None;
        let count = {
            let state = ctx.state::<KillCount>();
            state.0 += 1;
            state.0
        };

        // Java `npc.getSpawn().getZ()` (an NPC without a spawn would throw there).
        let Some((_, _, spawn_z)) = ctx.npc_spawn_point(npc) else {
            return;
        };

        // For monsters at Forge of the Gods - Lower level
        if spawn_z < -5000 {
            if (count > BONUS_LOWER_LV03) && (rand <= FORGE_BONUS02) {
                mob = ctx.add_spawn_at(LAVASAURUSES[4], npc.object_id, true, 0);
            } else if count > BONUS_LOWER_LV02 {
                mob = spawn_lavasaurus(ctx, npc, rand, [LAVASAURUSES[4], LAVASAURUSES[3]]);
            } else if count > BONUS_LOWER_LV01 {
                mob = spawn_lavasaurus(ctx, npc, rand, [LAVASAURUSES[3], LAVASAURUSES[2]]);
            } else if count >= MOBCOUNT_BONUS_MIN {
                mob = spawn_lavasaurus(ctx, npc, rand, [LAVASAURUSES[2], LAVASAURUSES[1]]);
            }
        } else if (count > BONUS_UPPER_LV05) && (rand <= FORGE_BONUS02) {
            mob = ctx.add_spawn_at(LAVASAURUSES[1], npc.object_id, true, 0);
        } else if count > BONUS_UPPER_LV04 {
            mob = spawn_lavasaurus(ctx, npc, rand, [LAVASAURUSES[4], LAVASAURUSES[3]]);
        } else if count > BONUS_UPPER_LV03 {
            mob = spawn_lavasaurus(ctx, npc, rand, [LAVASAURUSES[3], LAVASAURUSES[2]]);
        } else if count > BONUS_UPPER_LV02 {
            mob = spawn_lavasaurus(ctx, npc, rand, [LAVASAURUSES[2], LAVASAURUSES[1]]);
        } else if count > BONUS_UPPER_LV01 {
            mob = spawn_lavasaurus(ctx, npc, rand, [LAVASAURUSES[1], LAVASAURUSES[0]]);
        } else if (count >= MOBCOUNT_BONUS_MIN) && (rand <= FORGE_BONUS01) {
            mob = ctx.add_spawn_at(LAVASAURUSES[0], npc.object_id, true, 0);
        }

        if let Some(mob) = mob {
            ctx.add_attack_desire(mob, killer, 9999);
        }
    }

    fn on_spawn(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc) {
        ctx.start_quest_timer("suicide", 60_000, Some(npc), None, false);
    }
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::ScriptTest;

    const NPCS: &str = r#"
        <npc id="22634" type="Monster" level="70" name="Scarlet Stakato Worker"/>
        <npc id="18799" type="Monster" level="70" name="Newborn Lavasaurus"/>"#;

    #[test]
    fn the_third_kill_in_a_short_time_can_call_a_newborn_lavasaurus_that_dies_after_a_minute() {
        let mut registry = ScriptRegistry::new();
        registry.add(ForgeOfTheGods);
        let mut t = ScriptTest::new(registry, NPCS, "", &[]);
        t.enter(1, 80, vec![]);
        // Seeded RNG: kill until the 20% chance gives one (the count resets every 15 s).
        for _ in 0..60 {
            if !t.npcs_of(LAVASAURUSES[0]).is_empty() {
                break;
            }
            let mob = t.spawn(FOG_MOBS[0], 20);
            t.kill(1, mob);
        }
        let lava = t.npcs_of(LAVASAURUSES[0]);
        assert!(!lava.is_empty());
        // "suicide" after 60 s (`doDie(null)`).
        t.wait_ms(60_100);
        assert!(t.world.is_dead(lava[0]));
    }
}
