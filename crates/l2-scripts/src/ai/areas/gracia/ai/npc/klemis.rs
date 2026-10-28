//! Officer Klemis, who sends level 80 characters into the Seed of Annihilation.
// Java: dist/game/data/scripts/ai/areas/Gracia/AI/NPC/Klemis/Klemis.java

use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const KLEMIS: i32 = 32734; // Klemis

// Location
const LOCATION: (i32, i32, i32) = (-180218, 185923, -10576);

// Misc
const MIN_LV: i32 = 80;

/// Java `ai.areas.Gracia.AI.NPC.Klemis.Klemis`.
pub struct Klemis;

impl Script for Klemis {
    fn name(&self) -> &'static str {
        "Klemis"
    }

    fn path(&self) -> &'static str {
        "ai/areas/Gracia/AI/NPC/Klemis"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[KLEMIS]);
        s.add_talk_id(&[KLEMIS]);
        s.add_first_talk_id(&[KLEMIS]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        _npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        let player = player?;
        if event == "portInside" {
            if ctx.player_level(player) >= MIN_LV {
                // `teleToLocation(LOCATION)`: no random offset, the location's instance (0).
                ctx.teleport_player(player, LOCATION.0, LOCATION.1, LOCATION.2, 0, false);
            } else {
                return Some("32734-01.html".to_owned());
            }
        }

        // `super.onEvent(event, npc, player)`
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
    use crate::testing::{ScriptTest, pages_to};

    const NPCS: &str = r#"<npc id="32734" type="Folk" level="70" name="Klemis"/>"#;
    const DIR: &str = "data/scripts/ai/areas/Gracia/AI/NPC/Klemis";

    fn setup(level: i32) -> (ScriptTest, l2_util::ObjectId) {
        let mut registry = ScriptRegistry::new();
        registry.add(Klemis);
        let pages = [
            (
                format!("{DIR}/32734.html"),
                r#"<html><body>32734 <a action="bypass -h Script Klemis portInside">x</a></body></html>"#
                    .to_owned(),
            ),
            (
                format!("{DIR}/32734-01.html"),
                "<html><body>32734-01</body></html>".to_owned(),
            ),
        ];
        let pages: Vec<(&str, &str)> = pages.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
        let mut t = ScriptTest::new(registry, NPCS, "", &pages);
        let npc = t.spawn(KLEMIS, 20);
        t.enter(1, level, vec![]);
        t.talk(1, npc);
        assert!(pages_to(&t.take(), 1).pop().unwrap().contains("32734 "));
        (t, npc)
    }

    #[test]
    fn a_low_level_character_is_refused() {
        let (mut t, _) = setup(79);
        t.bypass(1, "Script Klemis portInside");
        assert!(pages_to(&t.take(), 1).pop().unwrap().contains("32734-01"));
    }

    #[test]
    fn a_level_80_character_is_teleported() {
        let (mut t, _) = setup(80);
        t.bypass(1, "Script Klemis portInside");
        let out = t.take();
        assert!(pages_to(&out, 1).is_empty());
        assert!(
            crate::testing::names_to(&out, 1)
                .iter()
                .any(|n| n.contains("TELEPORT")),
            "{:?}",
            crate::testing::names_to(&out, 1)
        );
    }
}
