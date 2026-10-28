//! Fortune-Teller Mine: 1000 adena for a line of fortune.
// Java: dist/game/data/scripts/ai/areas/Gracia/AI/NPC/FortuneTelling/FortuneTelling.java

use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const MINE: i32 = 32616;

// Misc
const COST: i64 = 1000;
/// Java `Inventory.ADENA_ID`.
const ADENA_ID: i32 = 57;

/// Java `ai.areas.Gracia.AI.NPC.FortuneTelling.FortuneTelling`.
pub struct FortuneTelling;

impl Script for FortuneTelling {
    fn name(&self) -> &'static str {
        "FortuneTelling"
    }

    fn path(&self) -> &'static str {
        "ai/areas/Gracia/AI/NPC/FortuneTelling"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[MINE]);
        s.add_talk_id(&[MINE]);
    }

    // NOTE(java-quirk): `onTalk` is only reached through the quest window, which lists the quests of
    // ids 1 to 19999; this script has id -1, so no click of a player gets here. Ported as it is.
    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        if ctx.world().adena_of(player) < COST {
            Some("lowadena.htm".to_owned())
        } else {
            ctx.take_items(player, ADENA_ID, COST);
            let fortune = ctx.get_random_range(1800309, 1800695);
            // `getHtm(..)` is null when the page is missing and Java throws.
            ctx.get_htm(player, "fortune.htm")
                .map(|page| page.replace("%fortune%", &fortune.to_string()))
        }
    }
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::{ScriptTest, item};

    const NPCS: &str = r#"<npc id="32616" type="Folk" level="70" name="Mine"/>"#;
    const ITEMS: &str =
        r#"<item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>"#;
    const DIR: &str = "data/scripts/ai/areas/Gracia/AI/NPC/FortuneTelling";

    fn setup(adena: i64) -> (ScriptTest, ObjectId) {
        let mut registry = ScriptRegistry::new();
        registry.add(FortuneTelling);
        let pages = [
            (
                format!("{DIR}/fortune.htm"),
                "<html><body>fortune <fstring>%fortune%</fstring></body></html>".to_owned(),
            ),
            (
                format!("{DIR}/lowadena.htm"),
                "<html><body>lowadena</body></html>".to_owned(),
            ),
        ];
        let pages: Vec<(&str, &str)> = pages.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
        let mut t = ScriptTest::new(registry, NPCS, ITEMS, &pages);
        let npc = t.spawn(MINE, 20);
        t.enter(1, 80, vec![item(1000, ADENA_ID, adena)]);
        (t, npc)
    }

    /// The hook is called the way `notifyTalk` would (see the quirk note above).
    fn talk(t: &mut ScriptTest, npc: ObjectId) -> Option<String> {
        let mut result = None;
        t.world.run_as_script("FortuneTelling", |ctx| {
            let n = ctx.script_npc(npc).unwrap();
            result = FortuneTelling.on_talk(ctx, n, ObjectId(1));
        });
        result
    }

    #[test]
    fn a_poor_character_is_sent_away() {
        let (mut t, npc) = setup(999);
        assert_eq!(talk(&mut t, npc).as_deref(), Some("lowadena.htm"));
        assert_eq!(t.count(1, ADENA_ID), 999);
    }

    #[test]
    fn the_fortune_costs_1000_adena() {
        let (mut t, npc) = setup(1500);
        let page = talk(&mut t, npc).unwrap();
        assert_eq!(t.count(1, ADENA_ID), 500);
        assert!(!page.contains("%fortune%"), "{page}");
        let number: i32 = page
            .split("<fstring>")
            .nth(1)
            .unwrap()
            .split("</fstring>")
            .next()
            .unwrap()
            .parse()
            .unwrap();
        assert!((1800309..=1800695).contains(&number));
        // Exactly 1000 adena is enough.
        let (mut t, npc) = setup(1000);
        assert!(talk(&mut t, npc).unwrap().contains("fortune"));
        assert_eq!(t.count(1, ADENA_ID), 0);
    }
}
