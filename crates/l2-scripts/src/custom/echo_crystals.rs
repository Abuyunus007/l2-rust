//! The Echo Crystal exchange: a score for a crystal, 200 adena.
// Java: dist/game/data/scripts/custom/EchoCrystals/EchoCrystals.java

use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

const NPCS: [i32; 2] = [31042, 31043];

const ADENA: i32 = 57;
const COST: i64 = 200;

/// Java `ScoreData`: the crystal and the three page suffixes.
struct ScoreData {
    crystal_id: i32,
    ok_msg: &'static str,
    no_adena_msg: &'static str,
    no_score_msg: &'static str,
}

/// Java `SCORES`: score item id to its data.
fn score_data(score: i32) -> Option<ScoreData> {
    let (crystal_id, ok_msg, no_adena_msg, no_score_msg) = match score {
        4410 => (4411, "01", "02", "03"),
        4409 => (4412, "04", "05", "06"),
        4408 => (4413, "07", "08", "09"),
        4420 => (4414, "10", "11", "12"),
        4421 => (4415, "13", "14", "15"),
        4419 => (4417, "16", "05", "06"),
        4418 => (4416, "17", "05", "06"),
        _ => return None,
    };
    Some(ScoreData {
        crystal_id,
        ok_msg,
        no_adena_msg,
        no_score_msg,
    })
}

/// Java `custom.EchoCrystals.EchoCrystals`.
pub struct EchoCrystals;

impl Script for EchoCrystals {
    fn name(&self) -> &'static str {
        "EchoCrystals"
    }

    fn path(&self) -> &'static str {
        "custom/EchoCrystals"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        for npc in NPCS {
            s.add_start_npc(&[npc]);
            s.add_talk_id(&[npc]);
        }
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        let mut htmltext = String::new();
        // `StringUtil.isNumeric(event)`: digits only; an out-of-range number is a
        // `NumberFormatException` in Java, which ends the event.
        if !event.is_empty() && event.chars().all(|c| c.is_ascii_digit()) {
            let score: i32 = event.parse().ok()?;
            if let Some(entry) = score_data(score) {
                let player = player?;
                let npc = npc?;
                if !ctx.has_quest_items(player, score) {
                    htmltext = format!("{}-{}.htm", npc.id, entry.no_score_msg);
                } else if ctx.get_quest_items_count(player, ADENA) < COST {
                    htmltext = format!("{}-{}.htm", npc.id, entry.no_adena_msg);
                } else {
                    ctx.take_items(player, ADENA, COST);
                    ctx.give_items(player, entry.crystal_id, 1);
                    htmltext = format!("{}-{}.htm", npc.id, entry.ok_msg);
                }
            }
        }

        Some(htmltext)
    }

    fn on_talk(&self, _ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, _player: ObjectId) -> Option<String> {
        Some("1.htm".to_owned())
    }
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::{Pages, ScriptTest, item};

    const NPCS_XML: &str = r#"<npc id="31042" type="Folk" level="70" name="Echo"/>"#;
    const ITEMS: &str = r#"<item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="4410" type="EtcItem" name="Score"/><item id="4411" type="EtcItem" name="Crystal"/>"#;

    fn scene(adena: i64, score: bool) -> (ScriptTest, ObjectId) {
        let mut registry = ScriptRegistry::new();
        registry.add(EchoCrystals);
        let pages = Pages::of(&EchoCrystals)
            .page("1.htm", &["4410"])
            .plain(&["31042-01.htm", "31042-02.htm", "31042-03.htm"])
            .npcs_by_name(&[31042]);
        let mut t = ScriptTest::with_pages(registry, NPCS_XML, ITEMS, pages);
        let mut items = vec![item(100, 57, adena)];
        if score {
            items.push(item(101, 4410, 1));
        }
        t.enter(1, 40, items);
        let npc = t.spawn(31042, 20);
        (t, npc)
    }

    #[test]
    fn the_exchange_takes_adena_and_gives_the_crystal() {
        let (mut t, npc) = scene(500, true);
        assert!(t.script_page(1, npc, "EchoCrystals").contains("1.htm"));
        assert!(t.event(1, "EchoCrystals", "4410").contains("31042-01.htm"));
        assert_eq!(t.count(1, 57), 300);
        assert_eq!(t.count(1, 4411), 1);
        // The score stays: Java never takes it.
        assert_eq!(t.count(1, 4410), 1);
    }

    #[test]
    fn no_adena_and_no_score_answer_with_their_pages() {
        let (mut t, npc) = scene(100, true);
        t.script_page(1, npc, "EchoCrystals");
        assert!(t.event(1, "EchoCrystals", "4410").contains("31042-02.htm"));
        assert_eq!(t.count(1, 4411), 0);

        let (mut t, npc) = scene(500, false);
        t.script_page(1, npc, "EchoCrystals");
        assert!(t.event(1, "EchoCrystals", "4410").contains("31042-03.htm"));
        assert_eq!(t.count(1, 57), 500);
    }

    #[test]
    fn the_score_table_matches_java() {
        assert_eq!(
            score_data(4419).map(|s| (s.crystal_id, s.ok_msg)),
            Some((4417, "16"))
        );
        assert!(score_data(4411).is_none());
    }
}
