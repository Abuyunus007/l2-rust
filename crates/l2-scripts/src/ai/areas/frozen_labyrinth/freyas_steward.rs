//! Freya's Steward teleports characters of level 82 and above into the Frozen Labyrinth.
// Java: dist/game/data/scripts/ai/areas/FrozenLabyrinth/FreyasSteward/FreyasSteward.java

use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const FREYAS_STEWARD: i32 = 32029;
// Location
const TELEPORT_LOC: (i32, i32, i32) = (103045, -124361, -2768);
// Misc
const MIN_LEVEL: i32 = 82;

/// Java `ai.areas.FrozenLabyrinth.FreyasSteward.FreyasSteward`.
pub struct FreyasSteward;

impl Script for FreyasSteward {
    fn name(&self) -> &'static str {
        "FreyasSteward"
    }

    fn path(&self) -> &'static str {
        "ai/areas/FrozenLabyrinth/FreyasSteward"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[FREYAS_STEWARD]);
        s.add_first_talk_id(&[FREYAS_STEWARD]);
        s.add_talk_id(&[FREYAS_STEWARD]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        _npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        if event.eq_ignore_ascii_case("ENTER") {
            let player = player?;
            if ctx.player_level(player) >= MIN_LEVEL {
                ctx.teleport_player(player, TELEPORT_LOC.0, TELEPORT_LOC.1, TELEPORT_LOC.2, 0, false);
                return None;
            }
            // This is beyond you. Wait for the right time.
            return Some("32029-1.html".to_owned());
        }
        None
    }

    fn on_first_talk(&self, _ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, _player: ObjectId) -> Option<String> {
        Some("32029.html".to_owned())
    }
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::{Pages, ScriptTest, X, pages_to};

    const NPCS: &str = r#"<npc id="32029" type="Folk" level="70" name="Steward"/>"#;

    #[test]
    fn the_steward_lets_only_high_levels_in() {
        let mut registry = ScriptRegistry::new();
        registry.add(FreyasSteward);
        let pages = Pages::of(&FreyasSteward)
            .page("32029.html", &["ENTER"])
            .plain(&["32029-1.html"]);
        let mut t = ScriptTest::with_pages(registry, NPCS, "", pages);
        let steward = t.spawn(FREYAS_STEWARD, 20);
        t.enter(1, 82, vec![]);
        t.enter(2, 81, vec![]);
        t.talk(1, steward);
        assert!(pages_to(&t.take(), 1).pop().unwrap().contains("32029.html"));
        t.talk(2, steward);
        t.take();
        assert!(t.event(2, "FreyasSteward", "ENTER").contains("32029-1.html"));
        assert_eq!(t.world.position_of(ObjectId(2)).unwrap().0, X);
        assert_eq!(t.event(1, "FreyasSteward", "ENTER"), "");
        let (x, y, _) = t.world.position_of(ObjectId(1)).unwrap();
        assert!((x - TELEPORT_LOC.0).abs() < 5 && (y - TELEPORT_LOC.1).abs() < 5);
    }
}
