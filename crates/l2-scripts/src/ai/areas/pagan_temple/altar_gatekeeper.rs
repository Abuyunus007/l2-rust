//! Altar Gatekeeper: shouts that the door to the 3rd floor is open when it spawns.
// Java: dist/game/data/scripts/ai/areas/PaganTemple/AltarGatekeeper.java

use l2_protocol_game::NpcStringId;
use l2_protocol_game::server::ChatType;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const ALTAR_GATEKEEPER: i32 = 32051;
const SHOUT_COOLDOWN: i64 = 30_000;

/// Java `private static volatile long _lastShoutAt` (0 = never; Java compares with the epoch
/// clock, so the first spawn always shouts, which `None` keeps).
#[derive(Default)]
struct LastShout(Option<i64>);

/// Java `ai.areas.PaganTemple.AltarGatekeeper`.
pub struct AltarGatekeeper;

impl Script for AltarGatekeeper {
    fn name(&self) -> &'static str {
        "AltarGatekeeper"
    }

    fn path(&self) -> &'static str {
        "ai/areas/PaganTemple"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_spawn_id(&[ALTAR_GATEKEEPER]);
    }

    fn on_spawn(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc) {
        let now = ctx.now_ms();
        let last = ctx.shared_state::<LastShout>();
        if last.0.is_some_and(|at| (now - at) < SHOUT_COOLDOWN) {
            return;
        }

        last.0 = Some(now);
        ctx.broadcast_npc_say(
            npc,
            ChatType::NpcGeneral,
            NpcStringId::TheDoorToThe3rdFloorOfTheAltarIsNowOpen,
            &[],
        );
    }
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::{ScriptTest, names_to};

    const NPCS: &str = r#"<npc id="32051" type="Folk" level="70" name="Altar Gatekeeper"/>"#;

    #[test]
    fn the_shout_has_a_30_second_cooldown_for_all_gatekeepers() {
        let mut registry = ScriptRegistry::new();
        registry.add(AltarGatekeeper);
        let mut t = ScriptTest::new(registry, NPCS, "", &[]);
        t.enter(1, 75, vec![]);
        // The three gatekeepers spawn together: the first shout (not seen yet by the player)
        // starts the cooldown.
        let a = t.spawn(ALTAR_GATEKEEPER, 20);
        let b = t.spawn(ALTAR_GATEKEEPER, 30);
        t.ticks(5);
        t.take();
        let says = |t: &mut ScriptTest| {
            t.ticks(1);
            names_to(&t.take(), 1).iter().filter(|n| *n == "NPC_SAY").count()
        };
        let spawn = |t: &mut ScriptTest, npc| {
            t.world.run_as_script("AltarGatekeeper", |ctx| {
                let npc = ctx.script_npc(npc).unwrap();
                AltarGatekeeper.on_spawn(ctx, npc);
            });
        };
        // The spawns above started the cooldown: no shout now.
        spawn(&mut t, a);
        assert_eq!(says(&mut t), 0);

        // Java compares with the epoch clock, which a test cannot move: age the last shout.
        t.world.run_as_script("AltarGatekeeper", |ctx| {
            ctx.shared_state::<LastShout>().0 = Some(0);
        });
        spawn(&mut t, a);
        assert_eq!(says(&mut t), 1);
        spawn(&mut t, b);
        assert_eq!(says(&mut t), 0);
    }
}
