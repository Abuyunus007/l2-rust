//! The manor managers of the castle towns: the menu page, the help pages and the
//! `manor_menu_select` buttons (`ON_NPC_MANOR_BYPASS`).
// Java: dist/game/data/scripts/ai/others/ManorManager/ManorManager.java
//
// The buttons' packets are `World::manor_manager_bypass` (`manor/npc.rs`, the manor logic).

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
        s.add_npc_manor_bypass_id(&NPC);
    }

    fn on_npc_manor_bypass(
        &self,
        ctx: &mut ScriptCtx<'_>,
        player: ObjectId,
        npc: ScriptNpc,
        request: i32,
        manor_id: i32,
        next_period: bool,
    ) {
        ctx.world_mut()
            .manor_manager_bypass(player, npc.object_id, request, manor_id, next_period);
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
    use l2_world::clan::{ClanDataRecord, ClanMemberRecord, ClanPrivilegesRef, PlayerClanState};
    use l2_world::manor::ManorSettings;
    use l2_world::{ClanRecord, ScriptRegistry};

    use super::*;
    use crate::testing::{ScriptTest, pages_to, player_data};

    const FARMER: i32 = 35644;
    /// Gludio's manager (`manor_id` 1) and Dion's (2).
    const NPCS_XML: &str = r#"
        <npc id="35644" type="Merchant" level="70" name="Manor Manager">
          <parameters><param name="manor_id" value="1"/></parameters></npc>
        <npc id="35645" type="Merchant" level="70" name="Manor Manager">
          <parameters><param name="manor_id" value="2"/></parameters></npc>"#;
    const DIR: &str = "data/scripts/ai/others/ManorManager";
    const LIONS: i32 = 500;
    const LORD: i32 = 3;

    fn setup() -> (ScriptTest, ObjectId) {
        let mut registry = ScriptRegistry::new();
        registry.add(ManorManager);
        let pages = [
            (
                "manager.htm",
                r#"<html><body>Manor %objectId% %playername% <a action="bypass Script ManorManager manager-help-01.htm">Help</a></body></html>"#,
            ),
            ("manager-lord.htm", "<html><body>My Lord %objectId%</body></html>"),
            ("manager-help-01.htm", "<html><body>HELP 1</body></html>"),
        ]
        .map(|(f, t)| (format!("{DIR}/{f}"), t));
        let mut pages: Vec<(String, &str)> = pages.into_iter().collect();
        pages.push((
            "data/html/npcdefault.htm".to_owned(),
            "<html><body>Default page</body></html>",
        ));
        let pages: Vec<(&str, &str)> = pages.iter().map(|(a, b)| (a.as_str(), *b)).collect();
        let mut t = ScriptTest::new(registry, NPCS_XML, "", &pages);
        // The Lions, a clan with Gludio (castle 1), led by player 3.
        t.world.load_clans(
            vec![ClanRecord {
                clan_id: LIONS,
                data: Some(ClanDataRecord {
                    name: Some("Lions".to_owned()),
                    level: 5,
                    castle_id: 1,
                    leader_id: LORD,
                    ..ClanDataRecord::default()
                }),
                members: vec![ClanMemberRecord {
                    object_id: LORD,
                    name: "P3".to_owned(),
                    level: 40,
                    power_grade: 1,
                    ..ClanMemberRecord::default()
                }],
                ..ClanRecord::default()
            }],
            Vec::new(),
        );
        let npc = t.spawn(FARMER, 20);
        (t, npc)
    }

    fn enter_lord(t: &mut ScriptTest) {
        let mut data = player_data(LORD, 40, vec![]);
        data.profile.clan = Some(PlayerClanState {
            clan_id: LIONS,
            privileges: ClanPrivilegesRef::Own(0x00FF_FFFF),
            ..PlayerClanState::default()
        });
        t.enter_with(data);
    }

    #[test]
    fn the_first_talk_shows_the_menu_and_the_help_pages_follow() {
        let (mut t, npc) = setup();
        t.enter(1, 40, vec![]);
        t.take();
        t.talk(1, npc);
        let page = pages_to(&t.take(), 1).pop().unwrap();
        assert!(page.starts_with("<html><body>Manor "));
        assert!(page.contains(&format!("{} P1", npc.0)), "{page}");

        t.bypass(1, "Script ManorManager manager-help-01.htm");
        assert_eq!(pages_to(&t.take(), 1), ["<html><body>HELP 1</body></html>"]);
        // Any other event answers nothing.
        t.talk(1, npc);
        t.take();
        t.bypass(1, "Script ManorManager manager-help-09.htm");
        assert!(pages_to(&t.take(), 1).is_empty());
    }

    #[test]
    fn the_lord_of_the_castle_gets_the_lord_page() {
        let (mut t, npc) = setup();
        enter_lord(&mut t);
        t.take();
        t.talk(LORD, npc);
        assert_eq!(
            pages_to(&t.take(), LORD),
            [format!("<html><body>My Lord {}</body></html>", npc.0)]
        );
        // The manager of another castle shows the plain menu to him.
        let dion = t.spawn(35645, 25);
        t.talk(LORD, dion);
        let page = pages_to(&t.take(), LORD).pop().unwrap();
        assert!(page.starts_with("<html><body>Manor "), "{page}");
    }

    #[test]
    fn without_the_manor_the_default_page_is_shown() {
        let (mut t, npc) = setup();
        t.world.set_manor_settings(ManorSettings {
            allow_manor: false,
            ..ManorSettings::default()
        });
        t.enter(1, 40, vec![]);
        t.take();
        t.talk(1, npc);
        assert_eq!(pages_to(&t.take(), 1), ["<html><body>Default page</body></html>"]);
    }
}
