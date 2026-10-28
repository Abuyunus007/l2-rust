//! The symbol makers (dye sellers): the first page and the draw / remove lists.
// Java: dist/game/data/scripts/ai/others/SymbolMaker/SymbolMaker.java
//
// The packets behind the lists (`HennaEquipList`, `HennaRemoveList`) and the dye requests are the
// world's `henna.rs`.

use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const NPCS: [i32; 11] = [
    31046, // Marsden
    31047, // Kell
    31048, // McDermott
    31049, // Pepper
    31050, // Thora
    31051, // Keach
    31052, // Heid
    31053, // Kidder
    31264, // Olsun
    31308, // Achim
    31953, // Rankar
];

/// Java `ai.others.SymbolMaker.SymbolMaker`.
pub struct SymbolMaker;

impl Script for SymbolMaker {
    fn name(&self) -> &'static str {
        "SymbolMaker"
    }

    fn path(&self) -> &'static str {
        "ai/others/SymbolMaker"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_first_talk_id(&NPCS);
        s.add_start_npc(&NPCS);
        s.add_talk_id(&NPCS);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        _npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        let mut htmltext = None;
        match event {
            "symbol_maker.htm" | "symbol_maker-1.htm" | "symbol_maker-2.htm" | "symbol_maker-3.htm" => {
                htmltext = Some(event.to_owned());
            }
            "Draw" => {
                if let Some(player) = player {
                    ctx.world_mut().send_henna_equip_list(player);
                }
            }
            "Remove" => {
                if let Some(player) = player {
                    ctx.world_mut().send_henna_remove_list(player);
                }
            }
            _ => {}
        }
        htmltext
    }

    fn on_first_talk(&self, _ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, _player: ObjectId) -> Option<String> {
        Some("symbol_maker.htm".to_owned())
    }
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::{ScriptTest, names_to, pages_to};

    const MARSDEN: i32 = 31046;
    const NPCS_XML: &str = r#"<npc id="31046" type="Folk" level="70" name="Marsden"/>"#;
    const DIR: &str = "data/scripts/ai/others/SymbolMaker";

    fn setup() -> (ScriptTest, ObjectId) {
        let mut registry = ScriptRegistry::new();
        registry.add(SymbolMaker);
        let pages = [
            (
                "symbol_maker.htm",
                r#"<html><body>%objectId% <a action="bypass -h Script SymbolMaker Draw">Draw</a><a action="bypass -h Script SymbolMaker Remove">Remove</a><a action="bypass Script SymbolMaker symbol_maker-1.htm">More</a></body></html>"#,
            ),
            ("symbol_maker-1.htm", "<html><body>PAGE 1</body></html>"),
        ]
        .map(|(f, t)| (format!("{DIR}/{f}"), t));
        let pages: Vec<(&str, &str)> = pages.iter().map(|(a, b)| (a.as_str(), *b)).collect();
        let mut t = ScriptTest::new(registry, NPCS_XML, "", &pages);
        let npc = t.spawn(MARSDEN, 20);
        (t, npc)
    }

    #[test]
    fn the_first_talk_shows_the_main_page_and_the_links_work() {
        let (mut t, npc) = setup();
        t.enter(1, 40, vec![]);
        t.take();
        t.talk(1, npc);
        let page = pages_to(&t.take(), 1).pop().unwrap();
        assert!(page.contains(&npc.0.to_string()));

        t.bypass(1, "Script SymbolMaker symbol_maker-1.htm");
        assert_eq!(pages_to(&t.take(), 1), ["<html><body>PAGE 1</body></html>"]);

        // (A page without links replaces the actions the player may send: back to the NPC.)
        t.talk(1, npc);
        t.take();
        // The lists are packets, not pages.
        t.bypass(1, "Script SymbolMaker Remove");
        let out = t.take();
        let names = names_to(&out, 1);
        assert!(names.contains(&"HENNA_UNEQUIP_LIST".to_owned()), "{names:?}");
        assert!(pages_to(&out, 1).is_empty());
    }
}
