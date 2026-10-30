//! Elroki teleport AI: Orahochin and Gariachin send a character across the isle.
// Java: dist/game/data/scripts/ai/areas/PrimevalIsle/ElrokiTeleporters/ElrokiTeleporters.java

use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const ORAHOCHIN: i32 = 32111;
const GARIACHIN: i32 = 32112;

// Locations
const TELEPORT_ORAHOCIN: (i32, i32, i32) = (5361, -2638, -3176);
const TELEPORT_GARIACHIN: (i32, i32, i32) = (7327, -5501, -3232);

/// Java `ai.areas.PrimevalIsle.ElrokiTeleporters.ElrokiTeleporters`.
pub struct ElrokiTeleporters;

impl Script for ElrokiTeleporters {
    fn name(&self) -> &'static str {
        "ElrokiTeleporters"
    }

    fn path(&self) -> &'static str {
        "ai/areas/PrimevalIsle/ElrokiTeleporters"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_first_talk_id(&[ORAHOCHIN, GARIACHIN]);
        s.add_start_npc(&[ORAHOCHIN, GARIACHIN]);
        s.add_talk_id(&[ORAHOCHIN, GARIACHIN]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        if event == "TELEPORT" {
            let (npc, player) = (npc?, player?);
            if !ctx.is_in_combat(player) {
                let (x, y, z) = if npc.id == ORAHOCHIN {
                    TELEPORT_ORAHOCIN
                } else {
                    TELEPORT_GARIACHIN
                };
                ctx.teleport_player(player, x, y, z, 0, false);
                return None;
            }

            return Some(format!("{}-no.html", npc.id)); // Player is in combat.
        }

        None
    }

    fn on_first_talk(&self, _ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, _player: ObjectId) -> Option<String> {
        Some(format!("{}.html", npc.id))
    }
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::{Pages, ScriptTest, X};

    const NPCS: &str = r#"<npc id="32111" type="Folk" level="70" name="Orahochin"/>"#;

    #[test]
    fn teleport_moves_the_character_away() {
        let mut registry = ScriptRegistry::new();
        registry.add(ElrokiTeleporters);
        let pages = Pages::of(&ElrokiTeleporters)
            .page("32111.html", &["TELEPORT"])
            .plain(&["32111-no.html"]);
        let mut t = ScriptTest::with_pages(registry, NPCS, "", pages);
        let orahochin = t.spawn(ORAHOCHIN, 20);
        t.enter(1, 75, vec![]);
        t.talk(1, orahochin);
        t.take();
        t.event(1, "ElrokiTeleporters", "TELEPORT");
        t.ticks(40);
        let (x, _, _) = t.world.position_of(ObjectId(1)).unwrap();
        assert_ne!(x, X);
    }
}
