//! Asher, the gatekeeper of the Primeval Isle: teleports to Rune for 50,000 adena.
// Java: dist/game/data/scripts/ai/others/Asher/Asher.java

use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const ASHER: i32 = 32714;

// Location
const LOCATION: (i32, i32, i32) = (43835, -47749, -792);

// Misc
const ADENA: i64 = 50000;
/// Java `Inventory.ADENA_ID`.
const ADENA_ID: i32 = 57;

/// Java `ai.others.Asher.Asher`.
pub struct Asher;

impl Script for Asher {
    fn name(&self) -> &'static str {
        "Asher"
    }

    fn path(&self) -> &'static str {
        "ai/others/Asher"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_first_talk_id(&[ASHER]);
        s.add_start_npc(&[ASHER]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        _npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        let player = player?;
        if event == "teleport" {
            if ctx.adena_of(player) >= ADENA {
                ctx.teleport_player(player, LOCATION.0, LOCATION.1, LOCATION.2, 0, false);
                ctx.take_items(player, ADENA_ID, ADENA);
            } else {
                return Some("32714-02.html".to_owned());
            }
        } else if event == "32714-01.html" {
            return Some(event.to_owned());
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
    use crate::testing::{ScriptTest, item, pages_to};

    const NPCS: &str = r#"<npc id="32714" type="Folk" level="70" name="Asher"/>"#;
    const ITEMS: &str =
        r#"<item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>"#;
    const DIR: &str = "data/scripts/ai/others/Asher";

    fn setup(adena: i64) -> (ScriptTest, ObjectId) {
        let mut registry = ScriptRegistry::new();
        registry.add(Asher);
        let main = r#"<html><body><a action="bypass -h Script Asher teleport">T</a><a action="bypass Script Asher 32714-01.html">I</a></body></html>"#;
        let pages = [
            (format!("{DIR}/32714.html"), main.to_owned()),
            (
                format!("{DIR}/32714-01.html"),
                "<html><body>01</body></html>".to_owned(),
            ),
            (
                format!("{DIR}/32714-02.html"),
                "<html><body>02</body></html>".to_owned(),
            ),
        ];
        let pages: Vec<(&str, &str)> = pages.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
        let mut t = ScriptTest::new(registry, NPCS, ITEMS, &pages);
        let asher = t.spawn(ASHER, 20);
        t.enter(1, 75, vec![item(900, ADENA_ID, adena)]);
        t.talk(1, asher);
        t.take();
        (t, asher)
    }

    #[test]
    fn the_first_page_is_the_npc_id() {
        let mut registry = ScriptRegistry::new();
        registry.add(Asher);
        let mut t = ScriptTest::new(
            registry,
            NPCS,
            ITEMS,
            &[(&format!("{DIR}/32714.html"), "<html><body>main</body></html>")],
        );
        let asher = t.spawn(ASHER, 20);
        t.enter(1, 75, vec![]);
        t.take();
        t.talk(1, asher);
        assert!(pages_to(&t.take(), 1).pop().unwrap().contains("main"));
    }

    #[test]
    fn teleport_costs_adena() {
        let (mut t, _) = setup(60000);
        t.bypass(1, "Script Asher teleport");
        assert_eq!(t.count(1, ADENA_ID), 10000);
        let at = t.world.position_of(ObjectId(1)).unwrap();
        assert_ne!(at, (crate::testing::X, crate::testing::Y, crate::testing::Z));
    }

    #[test]
    fn without_the_adena_a_page_explains() {
        let (mut t, _) = setup(49999);
        t.bypass(1, "Script Asher teleport");
        assert_eq!(t.count(1, ADENA_ID), 49999);
        assert!(pages_to(&t.take(), 1).pop().unwrap().contains("02"));
    }

    #[test]
    fn the_seal_page_is_a_link() {
        let (mut t, _) = setup(1);
        t.bypass(1, "Script Asher 32714-01.html");
        assert!(pages_to(&t.take(), 1).pop().unwrap().contains("01"));
    }
}
