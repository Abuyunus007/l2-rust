//! Eilhalder von Hellmann walks the Forest of the Dead at night and leaves at dawn.
// Java: dist/game/data/scripts/ai/areas/ForestOfTheDead/EilhalderVonHellmann.java

use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

const EILHALDER_VON_HELLMANN: i32 = 25328;
const SPAWN_LOCATION: (i32, i32, i32) = (59090, -42188, -3003);

/// Java's `static Npc _npcInstance`.
#[derive(Default)]
struct EilhalderState {
    npc_instance: Option<ScriptNpc>,
}

/// Java `ai.areas.ForestOfTheDead.EilhalderVonHellmann`.
pub struct EilhalderVonHellmann;

impl Script for EilhalderVonHellmann {
    fn name(&self) -> &'static str {
        "EilhalderVonHellmann"
    }

    fn path(&self) -> &'static str {
        "ai/areas/ForestOfTheDead"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.set_on_day_night_change();
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        _event: &str,
        npc: Option<ScriptNpc>,
        _player: Option<ObjectId>,
    ) -> Option<String> {
        if let Some(npc) = npc {
            if ctx.is_in_combat(npc.object_id) {
                let instance = ctx.state::<EilhalderState>().npc_instance;
                ctx.start_quest_timer("despawn", 30000, instance, None, false);
            } else {
                ctx.delete_npc(npc);
                ctx.state::<EilhalderState>().npc_instance = None;
            }
        }
        None
    }

    fn on_day_night_change(&self, ctx: &mut ScriptCtx<'_>, is_night: bool) {
        let instance = ctx.state::<EilhalderState>().npc_instance;
        if !is_night {
            if let Some(instance) = instance {
                if !ctx.is_in_combat(instance.object_id) {
                    ctx.delete_npc(instance);
                    ctx.state::<EilhalderState>().npc_instance = None;
                } else {
                    ctx.start_quest_timer("despawn", 30000, Some(instance), None, false);
                }
            }
        } else if instance.is_none() {
            let (x, y, z) = SPAWN_LOCATION;
            let spawned = ctx.add_spawn(EILHALDER_VON_HELLMANN, x, y, z, 0, false, 0, 0);
            ctx.state::<EilhalderState>().npc_instance = spawned;
        }
    }
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::ScriptTest;

    const NPCS: &str = r#"<npc id="25328" type="RaidBoss" level="80" name="Eilhalder von Hellmann"><stats><vitals hp="1000" mp="100"/></stats></npc>"#;

    #[test]
    fn he_comes_at_night_once_and_goes_at_dawn() {
        let mut registry = ScriptRegistry::new();
        registry.add(EilhalderVonHellmann);
        let mut t = ScriptTest::new(registry, NPCS, "", &[]);
        let day_night = |t: &mut ScriptTest, night: bool| {
            t.world.run_as_script("EilhalderVonHellmann", |ctx| {
                EilhalderVonHellmann.on_day_night_change(ctx, night);
            });
        };
        day_night(&mut t, false);
        assert!(t.npcs_of(EILHALDER_VON_HELLMANN).is_empty());
        day_night(&mut t, true);
        day_night(&mut t, true);
        let eilhalder = t.npcs_of(EILHALDER_VON_HELLMANN);
        assert_eq!(eilhalder.len(), 1);
        day_night(&mut t, false);
        assert!(!t.in_world(eilhalder[0]));
        // The next night he comes again.
        day_night(&mut t, true);
        assert_eq!(t.npcs_of(EILHALDER_VON_HELLMANN).len(), 1);
    }
}
