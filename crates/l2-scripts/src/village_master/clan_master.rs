//! The clan master script of the village masters: the clan commands' pages.
// Java: dist/game/data/scripts/village_master/ClanMaster/ClanMaster.java
//
// A village master's page links to `bypass -h Script ClanMaster` (`onTalk`: `9000-01.htm`); the
// pages after it send `bypass Script ClanMaster <page>.htm` (`onEvent`). The commands on those
// pages (`npc_<id>_create_clan ...`) are the world's `clan/village_master.rs`.

use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// @formatter:off
const NPCS: [i32; 120] = [
    30026, 30031, 30037, 30066, 30070, 30109, 30115, 30120, 30154, 30174,
    30175, 30176, 30187, 30191, 30195, 30288, 30289, 30290, 30297, 30358,
    30373, 30462, 30474, 30498, 30499, 30500, 30503, 30504, 30505, 30508,
    30511, 30512, 30513, 30520, 30525, 30565, 30594, 30595, 30676, 30677,
    30681, 30685, 30687, 30689, 30694, 30699, 30704, 30845, 30847, 30849,
    30854, 30857, 30862, 30865, 30894, 30897, 30900, 30905, 30910, 30913,
    31269, 31272, 31276, 31279, 31285, 31288, 31314, 31317, 31321, 31324,
    31326, 31328, 31331, 31334, 31336, 31755, 31958, 31961, 31965, 31968,
    31974, 31977, 31996, 32092, 32093, 32094, 32095, 32096, 32097, 32098,
    32145, 32146, 32147, 32150, 32153, 32154, 32157, 32158, 32160, 32171,
    32193, 32196, 32199, 32202, 32205, 32206, 32209, 32210, 32213, 32214,
    32217, 32218, 32221, 32222, 32225, 32226, 32229, 32230, 32233, 32234,
];
// @formatter:on

/// Java `ClanMaster.LEADER_REQUIRED`: the page, and the page a player who is not a clan leader
/// gets.
const LEADER_REQUIRED: [(&str, &str); 11] = [
    ("9000-03.htm", "9000-03-no.htm"),
    ("9000-04.htm", "9000-04-no.htm"),
    ("9000-05.htm", "9000-05-no.htm"),
    ("9000-07.htm", "9000-07-no.htm"),
    ("9000-12a.htm", "9000-07-no.htm"),
    ("9000-12b.htm", "9000-07-no.htm"),
    ("9000-13a.htm", "9000-07-no.htm"),
    ("9000-13b.htm", "9000-07-no.htm"),
    ("9000-14a.htm", "9000-07-no.htm"),
    ("9000-14b.htm", "9000-07-no.htm"),
    ("9000-15.htm", "9000-07-no.htm"),
];

/// Java `village_master.ClanMaster.ClanMaster`.
pub struct ClanMaster;

impl Script for ClanMaster {
    fn name(&self) -> &'static str {
        "ClanMaster"
    }

    fn path(&self) -> &'static str {
        "village_master/ClanMaster"
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
        // Java would throw on a missing player; a timer without one never names these events.
        let player = player?;
        if let Some((_, no_page)) = LEADER_REQUIRED.iter().find(|(page, _)| *page == event) {
            if !ctx.world().is_clan_leader(player) {
                return Some((*no_page).to_owned());
            }
        }
        Some(event.to_owned())
    }

    fn on_talk(&self, _ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, _player: ObjectId) -> Option<String> {
        Some("9000-01.htm".to_owned())
    }
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::{ScriptTest, pages_to};

    const MASTER: i32 = 30026;
    const NPCS_XML: &str = r#"<npc id="30026" type="Folk" level="70" name="Master"/>"#;
    const DIR: &str = "data/scripts/village_master/ClanMaster";

    fn setup() -> (ScriptTest, ObjectId) {
        let mut registry = ScriptRegistry::new();
        registry.add(ClanMaster);
        let link = |p: &str| format!(r#"<a action="bypass Script ClanMaster {p}">x</a>"#);
        let pages = [
            (
                "9000-01.htm",
                format!(
                    "<html><body>CM %objectId% %playername%{}{}{}</body></html>",
                    link("9000-03.htm"),
                    link("9000-12a.htm"),
                    link("9000-02.htm")
                ),
            ),
            ("9000-02.htm", "<html><body>PAGE 9000-02</body></html>".to_owned()),
            ("9000-03.htm", "<html><body>PAGE 9000-03</body></html>".to_owned()),
            (
                "9000-03-no.htm",
                "<html><body>PAGE 9000-03-no</body></html>".to_owned(),
            ),
            (
                "9000-07-no.htm",
                "<html><body>PAGE 9000-07-no</body></html>".to_owned(),
            ),
            ("9000-12a.htm", "<html><body>PAGE 9000-12a</body></html>".to_owned()),
        ]
        .map(|(f, t)| (format!("{DIR}/{f}"), t));
        let mut pages: Vec<(String, String)> = pages.into_iter().collect();
        pages.push((
            "data/html/default/30026.htm".to_owned(),
            r#"<html><body><a action="bypass -h Script ClanMaster">Clan</a></body></html>"#.to_owned(),
        ));
        let pages: Vec<(&str, &str)> = pages.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
        let mut t = ScriptTest::new(registry, NPCS_XML, "", &pages);
        let master = t.spawn(MASTER, 20);
        (t, master)
    }

    /// Opens the script's `onTalk` page the way the village master's page link does.
    fn open(t: &mut ScriptTest, player: i32, master: ObjectId) -> String {
        t.talk(player, master);
        t.take();
        t.bypass(player, "Script ClanMaster");
        pages_to(&t.take(), player).pop().unwrap()
    }

    #[test]
    fn on_talk_shows_the_first_page() {
        let (mut t, master) = setup();
        t.enter(1, 75, vec![]);
        let page = open(&mut t, 1, master);
        assert!(page.starts_with("<html><body>CM "));
        assert!(page.contains(&format!("{} P1", master.0)), "{page}");
    }

    #[test]
    fn leader_pages_need_a_clan_leader() {
        let (mut t, master) = setup();
        t.enter(1, 75, vec![]);
        open(&mut t, 1, master);
        // Not a clan leader (no clan): the "no" page of the table.
        t.bypass(1, "Script ClanMaster 9000-03.htm");
        assert_eq!(pages_to(&t.take(), 1), ["<html><body>PAGE 9000-03-no</body></html>"]);
        open(&mut t, 1, master);
        t.bypass(1, "Script ClanMaster 9000-12a.htm");
        assert_eq!(pages_to(&t.take(), 1), ["<html><body>PAGE 9000-07-no</body></html>"]);
        // A page nobody needs a leader for.
        open(&mut t, 1, master);
        t.bypass(1, "Script ClanMaster 9000-02.htm");
        assert_eq!(pages_to(&t.take(), 1), ["<html><body>PAGE 9000-02</body></html>"]);
    }

    #[test]
    fn the_tables_are_java_s() {
        assert_eq!(LEADER_REQUIRED.len(), 11);
        assert_eq!(NPCS.len(), 120);
    }
}
