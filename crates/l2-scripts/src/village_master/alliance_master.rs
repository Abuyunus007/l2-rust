//! The alliance master script of the village masters: the alliance commands' pages.
// Java: dist/game/data/scripts/village_master/AllianceMaster/AllianceMaster.java
//
// `bypass -h Script AllianceMaster` is `onTalk` (`9001-01.htm`); the pages after it are
// `onEvent`. The commands on those pages are the world's `clan/village_master.rs`.

use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// @formatter:off
const NPCS: [i32; 120] = [
    30026, 30031, 30037, 30066, 30070, 30109, 30115, 30120, 30154, 30174, 30175, 30176, 30187, 30191, 30195,
    30288, 30289, 30290, 30297, 30358, 30373, 30462, 30474, 30498, 30499, 30500, 30503, 30504, 30505, 30508,
    30511, 30512, 30513, 30520, 30525, 30565, 30594, 30595, 30676, 30677, 30681, 30685, 30687, 30689, 30694,
    30699, 30704, 30845, 30847, 30849, 30854, 30857, 30862, 30865, 30894, 30897, 30900, 30905, 30910, 30913,
    31269, 31272, 31276, 31279, 31285, 31288, 31314, 31317, 31321, 31324, 31326, 31328, 31331, 31334, 31336,
    31755, 31958, 31961, 31965, 31968, 31974, 31977, 31996, 32092, 32093, 32094, 32095, 32096, 32097, 32098,
    32145, 32146, 32147, 32150, 32153, 32154, 32157, 32158, 32160, 32171, 32193, 32196, 32199, 32202, 32205,
    32206, 32209, 32210, 32213, 32214, 32217, 32218, 32221, 32222, 32225, 32226, 32229, 32230, 32233, 32234,
];
// @formatter:on

/// Java `village_master.AllianceMaster.AllianceMaster`.
pub struct AllianceMaster;

impl Script for AllianceMaster {
    fn name(&self) -> &'static str {
        "AllianceMaster"
    }

    fn path(&self) -> &'static str {
        "village_master/AllianceMaster"
    }

    fn setup(&self, s: &mut ScriptSetup) {
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
        let player = player?;
        if event != "9001-01.htm" && ctx.world().clan_of(player).is_none() {
            return Some("9001-04.htm".to_owned());
        }
        Some(event.to_owned())
    }

    fn on_talk(&self, _ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, _player: ObjectId) -> Option<String> {
        Some("9001-01.htm".to_owned())
    }
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::{ScriptTest, pages_to};

    const MASTER: i32 = 30026;
    const NPCS_XML: &str = r#"<npc id="30026" type="Folk" level="70" name="Master"/>"#;
    const DIR: &str = "data/scripts/village_master/AllianceMaster";

    #[test]
    fn a_player_without_a_clan_gets_the_no_clan_page() {
        let mut registry = ScriptRegistry::new();
        registry.add(AllianceMaster);
        let pages = [
            (
                "9001-01.htm",
                r#"<html><body>AM <a action="bypass Script AllianceMaster 9001-02.htm">create</a></body></html>"#,
            ),
            ("9001-02.htm", "<html><body>PAGE 9001-02</body></html>"),
            ("9001-04.htm", "<html><body>PAGE 9001-04</body></html>"),
        ]
        .map(|(f, t)| (format!("{DIR}/{f}"), t));
        let mut pages: Vec<(String, &str)> = pages.into_iter().collect();
        pages.push((
            "data/html/default/30026.htm".to_owned(),
            r#"<html><body><a action="bypass -h Script AllianceMaster">Alliance</a></body></html>"#,
        ));
        let pages: Vec<(&str, &str)> = pages.iter().map(|(a, b)| (a.as_str(), *b)).collect();
        let mut t = ScriptTest::new(registry, NPCS_XML, "", &pages);
        let master = t.spawn(MASTER, 20);
        t.enter(1, 75, vec![]);
        t.talk(1, master);
        t.take();
        t.bypass(1, "Script AllianceMaster");
        assert!(
            pages_to(&t.take(), 1)
                .pop()
                .unwrap()
                .starts_with("<html><body>AM ")
        );
        // The first page is open to everybody, the next one needs a clan.
        t.bypass(1, "Script AllianceMaster 9001-02.htm");
        assert_eq!(pages_to(&t.take(), 1), ["<html><body>PAGE 9001-04</body></html>"]);
    }
}
