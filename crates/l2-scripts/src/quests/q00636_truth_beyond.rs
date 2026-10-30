//! Quest 636: The Truth Beyond the Gate (Eliyah in the Hot Springs, Flauron at the Tower of
//! Insolence; the visitor's mark is faded when the character enters the zone).
// Java: dist/game/data/scripts/quests/Q00636_TruthBeyond/Q00636_TruthBeyond.java
//
// Pattern: a one-visit quest whose reward is a mark that the zone (30100) turns into the faded
// mark. Java's `destroyItemByItemId(.., false)` and `addItem(.., true)` in `onEnterZone` are
// ported as they are (no message on the destroy, the usual message on the add).

use l2_util::ObjectId;
use l2_world::script::{CREATED, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const ELIYAH: i32 = 31329;
const FLAURON: i32 = 32010;

// Zone
const ZONE: i32 = 30100;

// Items
const VISITOR_MARK: i32 = 8064;
const FADED_MARK: i32 = 8065;
const MARK: i32 = 8067;

/// Java `Q00636_TruthBeyond`.
pub struct Q00636TruthBeyond;

impl Script for Q00636TruthBeyond {
    fn name(&self) -> &'static str {
        "Q00636_TruthBeyond"
    }

    fn path(&self) -> &'static str {
        "quests/Q00636_TruthBeyond"
    }

    fn id(&self) -> i32 {
        636
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[ELIYAH]);
        s.add_talk_id(&[ELIYAH, FLAURON]);
        s.add_enter_zone_id(&[ZONE]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        _npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        let player = player?;
        let qs = ctx.get_quest_state(player, false)?;
        match event {
            "31329-04.htm" => qs.start_quest(ctx),
            "32010-02.htm" => {
                ctx.give_items(player, VISITOR_MARK, 1);
                qs.exit_quest(ctx, true, true);
            }
            _ => {}
        }
        Some(event.to_owned())
    }

    fn on_enter_zone(&self, ctx: &mut ScriptCtx<'_>, creature: ObjectId, _zone_id: i32) {
        // Java: `creature.isPlayer()`; the destroy is silent, the faded mark is added with its
        // message.
        if ctx.world().player(creature).is_none() {
            return;
        }
        if ctx
            .world_mut()
            .destroy_item_by_item_id(creature, VISITOR_MARK, 1, false)
        {
            ctx.world_mut().add_item(creature, FADED_MARK, 1, true);
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        if npc.id == ELIYAH {
            if ctx.has_quest_items(player, VISITOR_MARK)
                || ctx.has_quest_items(player, FADED_MARK)
                || ctx.has_quest_items(player, MARK)
            {
                qs.exit_quest(ctx, true, false);
                return Some("31329-mark.htm".to_owned());
            }
            let state = qs.state(ctx);
            if state == CREATED {
                if ctx.player_level(player) > 72 {
                    return Some("31329-02.htm".to_owned());
                }
                qs.exit_quest(ctx, true, false);
                return Some("31329-01.htm".to_owned());
            } else if state == STARTED {
                return Some("31329-05.htm".to_owned());
            }
        } else if qs.state(ctx) == STARTED {
            // Flauron only
            if qs.is_cond(ctx, 1) {
                return Some("32010-01.htm".to_owned());
            }
            qs.exit_quest(ctx, true, false);
            return Some("32010-03.htm".to_owned());
        }
        Some(ctx.get_no_quest_msg(player))
    }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::{ScriptTest, item, rows};

    const NAME: &str = "Q00636_TruthBeyond";

    const NPCS: &str = r#"
        <npc id="31329" type="Folk" level="70" name="Eliyah"/>
        <npc id="32010" type="Folk" level="70" name="Flauron"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="8064" type="EtcItem" name="Visitor's Mark"><set name="is_questitem" val="true"/></item>
        <item id="8065" type="EtcItem" name="Faded Visitor's Mark"><set name="is_questitem" val="true"/></item>
        <item id="8067" type="EtcItem" name="Mark of Truth"><set name="is_questitem" val="true"/></item>"#;

    const DIR: &str = "data/scripts/quests/Q00636_TruthBeyond";

    /// Stand-ins for the pages: the file names, and the links of the real pages.
    fn pages() -> Vec<(String, String)> {
        let link = |page: &str| format!(r#"<a action="bypass Script {NAME} {page}">x</a>"#);
        let mut pages: Vec<(String, String)> = [
            "31329-01.htm",
            "31329-04.htm",
            "31329-05.htm",
            "31329-mark.htm",
            "32010-02.htm",
            "32010-03.htm",
        ]
        .iter()
        .map(|p| (format!("{DIR}/{p}"), format!("<html><body>{p}</body></html>")))
        .collect();
        for (page, links) in [
            ("31329-02.htm", link("31329-03.htm")),
            ("31329-03.htm", link("31329-04.htm")),
            ("32010-01.htm", link("32010-02.htm")),
        ] {
            pages.push((
                format!("{DIR}/{page}"),
                format!("<html><body>{page}{links}</body></html>"),
            ));
        }
        for npc in [ELIYAH, FLAURON] {
            pages.push((
                format!("data/html/default/{npc}.htm"),
                format!(r#"<html><body>npc {npc} <a action="bypass Script">Quest</a></body></html>"#),
            ));
        }
        pages.push((
            "data/html/noquest.htm".to_owned(),
            "<html><body>I have nothing to say to you.</body></html>".to_owned(),
        ));
        pages
    }

    fn setup() -> (ScriptTest, Oid, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00636TruthBeyond);
        let pages = pages();
        let pages: Vec<(&str, &str)> = pages.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
        let mut t = ScriptTest::new(registry, NPCS, ITEMS, &pages);
        let eliyah = t.spawn(ELIYAH, 20);
        let flauron = t.spawn(FLAURON, 40);
        (t, eliyah, flauron)
    }

    #[test]
    fn a_character_of_level_72_or_less_is_sent_away() {
        let (mut t, eliyah, _) = setup();
        t.enter(1, 72, vec![]);
        assert!(t.quest_page(1, eliyah).contains("31329-01.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_visit_from_eliyah_to_the_visitor_mark() {
        let (mut t, eliyah, flauron) = setup();
        t.enter(1, 73, vec![]);
        assert!(t.quest_page(1, eliyah).contains("31329-02.htm"));
        assert!(t.event(1, NAME, "31329-03.htm").contains("31329-03.htm"));
        assert!(t.event(1, NAME, "31329-04.htm").contains("31329-04.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );

        // Eliyah talks to a started character.
        assert!(t.quest_page(1, eliyah).contains("31329-05.htm"));
        assert!(t.take_writes().is_empty());

        // Flauron at cond 1 sends the character on; the visitor's mark is given and the quest
        // is removed (`exitQuest(true, true)`).
        assert!(t.quest_page(1, flauron).contains("32010-01.htm"));
        assert!(t.event(1, NAME, "32010-02.htm").contains("32010-02.htm"));
        assert_eq!(t.count(1, VISITOR_MARK), 1);
        assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
        assert!(t.world.quest_list(Oid(1)).quests.is_empty());
    }

    #[test]
    fn flauron_sends_a_character_that_is_past_cond_1_away() {
        let (mut t, eliyah, flauron) = setup();
        t.enter(1, 80, vec![]);
        assert!(t.quest_page(1, eliyah).contains("31329-02.htm"));
        t.event(1, NAME, "31329-03.htm");
        t.event(1, NAME, "31329-04.htm");
        t.take_writes();
        // Cond 2 is past the visit: Flauron sends the character away.
        t.world.run_as_script(NAME, |ctx| {
            let qs = ctx.get_quest_state(Oid(1), false).unwrap();
            qs.set_cond(ctx, 2, false);
        });
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);
        assert!(t.quest_page(1, flauron).contains("32010-03.htm"));
        assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
        assert_eq!(t.count(1, VISITOR_MARK), 0);
    }

    #[test]
    fn a_character_with_the_mark_is_sent_to_the_mark_page() {
        let (mut t, eliyah, _) = setup();
        t.enter(1, 80, vec![item(100, VISITOR_MARK, 1)]);
        assert!(t.quest_page(1, eliyah).contains("31329-mark.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn entering_the_zone_fades_the_visitor_mark() {
        let (mut t, _, _) = setup();
        t.enter(1, 80, vec![item(100, VISITOR_MARK, 1)]);
        t.world.run_as_script(NAME, |ctx| {
            Q00636TruthBeyond.on_enter_zone(ctx, Oid(1), ZONE);
        });
        assert_eq!((t.count(1, VISITOR_MARK), t.count(1, FADED_MARK)), (0, 1));
        // A character without the visitor's mark gets nothing.
        t.enter(2, 80, vec![]);
        t.world.run_as_script(NAME, |ctx| {
            Q00636TruthBeyond.on_enter_zone(ctx, Oid(2), ZONE);
        });
        assert_eq!(t.count(2, FADED_MARK), 0);
    }
}
