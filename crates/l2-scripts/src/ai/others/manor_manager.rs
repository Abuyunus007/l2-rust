//! The manor managers of the castle towns: the menu page and the help pages.
// Java: dist/game/data/scripts/ai/others/ManorManager/ManorManager.java
//
// Left in the world (`manor/npc.rs`): `onNpcManorBypass`, an `ON_NPC_MANOR_BYPASS` annotation
// listener. The engine has no annotation listeners yet (`docs/design/scripts.md`, "Not in the
// engine yet"), so the `manor_menu_select` bypass still reaches those buttons from the world.

use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

const NPC: [i32; 14] = [
    35644, 35645, 35319, 35366, 36456, 35512, 35558, 35229, 35230, 35231, 35277, 35103, 35145, 35187,
];

/// Java `ai.others.ManorManager.ManorManager`.
pub struct ManorManager;

impl Script for ManorManager {
    fn name(&self) -> &'static str {
        "ManorManager"
    }

    fn path(&self) -> &'static str {
        "ai/others/ManorManager"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&NPC);
        s.add_first_talk_id(&NPC);
        s.add_talk_id(&NPC);
    }

    fn on_event(
        &self,
        _ctx: &mut ScriptCtx<'_>,
        event: &str,
        _npc: Option<ScriptNpc>,
        _player: Option<ObjectId>,
    ) -> Option<String> {
        let mut htmltext = None;
        match event {
            "manager-help-01.htm" | "manager-help-02.htm" | "manager-help-03.htm" => {
                htmltext = Some(event.to_owned());
            }
            _ => {}
        }
        htmltext
    }

    fn on_first_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        if ctx.world().manor_allowed() {
            let castle_id = ctx.world().npc_manor_id(npc.object_id);
            let is_gm = ctx.is_gm(player);
            let w = ctx.world();
            if !is_gm
                && w.is_clan_leader(player)
                && w.clan_of(player).is_some_and(|c| castle_id == c.castle_id)
            {
                return Some("manager-lord.htm".to_owned());
            }
            return Some("manager.htm".to_owned());
        }
        // The page's text itself, which `showResult` shows as html.
        ctx.get_htm(player, "data/html/npcdefault.htm")
    }
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::{ScriptTest, pages_to};

    const FARMER: i32 = 35644;
    const NPCS_XML: &str = r#"<npc id="35644" type="Merchant" level="70" name="Farmer"/>"#;
    const DIR: &str = "data/scripts/ai/others/ManorManager";

    #[test]
    fn the_first_talk_shows_the_menu_and_the_help_pages_follow() {
        let mut registry = ScriptRegistry::new();
        registry.add(ManorManager);
        let pages = [
            (
                "manager.htm",
                r#"<html><body>Manor %objectId% %playername% <a action="bypass Script ManorManager manager-help-01.htm">Help</a></body></html>"#,
            ),
            ("manager-help-01.htm", "<html><body>HELP 1</body></html>"),
        ]
        .map(|(f, t)| (format!("{DIR}/{f}"), t));
        let pages: Vec<(&str, &str)> = pages.iter().map(|(a, b)| (a.as_str(), *b)).collect();
        let mut t = ScriptTest::new(registry, NPCS_XML, "", &pages);
        let npc = t.spawn(FARMER, 20);
        t.enter(1, 40, vec![]);
        t.take();
        t.talk(1, npc);
        let page = pages_to(&t.take(), 1).pop().unwrap();
        assert!(page.starts_with("<html><body>Manor "));
        assert!(page.contains(&format!("{} P1", npc.0)), "{page}");

        t.bypass(1, "Script ManorManager manager-help-01.htm");
        assert_eq!(pages_to(&t.take(), 1), ["<html><body>HELP 1</body></html>"]);
        // Any other event answers nothing.
        t.bypass(1, "Script ManorManager manager-help-09.htm");
        assert!(pages_to(&t.take(), 1).is_empty());
    }
}
