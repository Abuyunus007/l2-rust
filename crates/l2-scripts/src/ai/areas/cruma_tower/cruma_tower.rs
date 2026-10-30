//! Mozella, the gatekeeper of the Cruma Tower: teleports characters up to level 55 inside.
// Java: dist/game/data/scripts/ai/areas/CrumaTower/CrumaTower.java

use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const MOZELLA: i32 = 30483;
// Locations
const TELEPORT_LOC1: (i32, i32, i32) = (17776, 113968, -11671);
const TELEPORT_LOC2: (i32, i32, i32) = (17680, 113968, -11671);
// Misc
const MAX_LEVEL: i32 = 55;

/// Java `ai.areas.CrumaTower.CrumaTower`.
pub struct CrumaTower;

impl Script for CrumaTower {
    fn name(&self) -> &'static str {
        "CrumaTower"
    }

    fn path(&self) -> &'static str {
        "ai/areas/CrumaTower"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_first_talk_id(&[MOZELLA]);
        s.add_start_npc(&[MOZELLA]);
        s.add_talk_id(&[MOZELLA]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        _npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        if event == "ENTER" {
            let player = player?;
            if ctx.player_level(player) <= MAX_LEVEL {
                let (x, y, z) = if ctx.get_random_boolean() {
                    TELEPORT_LOC1
                } else {
                    TELEPORT_LOC2
                };
                ctx.teleport_player(player, x, y, z, 0, false);
                return None;
            }
            // Characters who are level 56 and above cannot enter the Cruma Tower.
            return Some("30483-1.html".to_owned());
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
    use crate::testing::{Pages, ScriptTest};

    const NPCS: &str = r#"<npc id="30483" type="Folk" level="70" name="Mozella"/>"#;

    #[test]
    fn a_low_level_character_is_teleported_and_a_high_one_is_refused() {
        let mut registry = ScriptRegistry::new();
        registry.add(CrumaTower);
        let pages = Pages::of(&CrumaTower)
            .page("30483.html", &["ENTER"])
            .plain(&["30483-1.html"]);
        let mut t = ScriptTest::with_pages(registry, NPCS, "", pages);
        let mozella = t.spawn(MOZELLA, 20);
        t.enter(1, 55, vec![]);
        t.enter(2, 56, vec![]);

        t.talk(1, mozella);
        assert!(
            crate::testing::pages_to(&t.take(), 1)
                .pop()
                .unwrap()
                .contains("30483.html")
        );
        assert_eq!(t.event(1, "CrumaTower", "ENTER"), "");
        let (x, y, _) = t.world.position_of(ObjectId(1)).unwrap();
        assert!(
            [TELEPORT_LOC1, TELEPORT_LOC2]
                .iter()
                .any(|l| (l.0 - x).abs() < 50 && (l.1 - y).abs() < 50)
        );

        t.talk(2, mozella);
        t.take();
        assert!(t.event(2, "CrumaTower", "ENTER").contains("30483-1.html"));
        assert_eq!(t.world.position_of(ObjectId(2)).unwrap().0, crate::testing::X);
    }
}
