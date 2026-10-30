//! The ghost of the Turka Commander shouts when it wakes, five seconds later, and when it dies.
// Java: dist/game/data/scripts/ai/areas/CryptsOfDisgrace/TurkaCommanderGhost.java

use l2_protocol_game::NpcStringId;
use l2_protocol_game::server::ChatType;
use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const TURKA_COMMANDER_GHOST: i32 = 22707;
// Misc
const SHOUT_TIMER: &str = "shout";

/// Java `ai.areas.CryptsOfDisgrace.TurkaCommanderGhost`.
pub struct TurkaCommanderGhost;

impl Script for TurkaCommanderGhost {
    fn name(&self) -> &'static str {
        "TurkaCommanderGhost"
    }

    fn path(&self) -> &'static str {
        "ai/areas/CryptsOfDisgrace"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_spawn_id(&[TURKA_COMMANDER_GHOST]);
        s.add_kill_id(&[TURKA_COMMANDER_GHOST]);
    }

    fn on_spawn(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc) {
        ctx.broadcast_npc_say(
            npc,
            ChatType::NpcShout,
            NpcStringId::WhoHasAwakenedUsFromOurSlumber,
            &[],
        );
        ctx.start_quest_timer(SHOUT_TIMER, 5000, Some(npc), None, false);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        _player: Option<ObjectId>,
    ) -> Option<String> {
        if event == SHOUT_TIMER
            && let Some(npc) = npc
            && !ctx.is_dead(npc.object_id)
        {
            ctx.broadcast_npc_say(
                npc,
                ChatType::NpcShout,
                NpcStringId::AllWillPayASeverePriceToMeAndTheseHere,
                &[],
            );
        }
        None
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, _killer: ObjectId, _is_summon: bool) {
        ctx.broadcast_npc_say(
            npc,
            ChatType::NpcShout,
            NpcStringId::AllIsVanityButThisCannotBeTheEnd,
            &[],
        );
    }
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::{ScriptTest, names_to};

    const NPCS: &str = r#"<npc id="22707" type="Monster" level="70" name="Turka Commander Ghost"><stats><vitals hp="1000" mp="100"/></stats></npc>"#;

    #[test]
    fn the_ghost_shouts_three_times() {
        let mut registry = ScriptRegistry::new();
        registry.add(TurkaCommanderGhost);
        let mut t = ScriptTest::new(registry, NPCS, "", &[]);
        t.enter(1, 70, vec![]);
        t.take();
        let ghost = t.spawn(TURKA_COMMANDER_GHOST, 30);
        t.ticks(2);
        let says = |t: &ScriptTest| {
            names_to(&t.take(), 1)
                .iter()
                .filter(|n| n.as_str() == "NPC_SAY")
                .count()
        };
        assert_eq!(says(&t), 1);
        t.wait_ms(5_200);
        assert_eq!(says(&t), 1);
        t.kill(1, ghost);
        assert_eq!(says(&t), 1);
    }
}
