//! Quest 653: Wild Maiden (Suki, the wild maiden out of the village, and Galibredo in Talking Island).
// Java: dist/game/data/scripts/quests/Q00653_WildMaiden/Q00653_WildMaiden.java
//
// Pattern: a level-gated quest that takes the Scroll of Escape on start and deletes the start
// NPC. The start page is a coin toss (`get_random(2)`): a second one is shown half the time.

use l2_util::ObjectId;
use l2_world::script::{CREATED, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const GALIBREDO: i32 = 30181;
const SUKI: i32 = 32013;

// Item
const SOE: i32 = 736;

// Misc
const MIN_LEVEL: i32 = 36;

/// Java `Q00653_WildMaiden`.
pub struct Q00653WildMaiden;

impl Script for Q00653WildMaiden {
    fn name(&self) -> &'static str {
        "Q00653_WildMaiden"
    }

    fn path(&self) -> &'static str {
        "quests/Q00653_WildMaiden"
    }

    fn id(&self) -> i32 {
        653
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[SUKI]);
        s.add_talk_id(&[GALIBREDO, SUKI]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        let player = player?;
        let qs = ctx.get_quest_state(player, false)?;

        let mut htmltext = None;
        if event == "32013-03.html" {
            htmltext = Some(event.to_owned());
        } else if event == "32013-04.htm" {
            if !ctx.has_quest_items(player, SOE) {
                return Some("32013-05.htm".to_owned());
            }

            qs.start_quest(ctx);
            ctx.take_items(player, SOE, 1);
            // Java `npc.deleteMe()`: Suki leaves with the scroll.
            if let Some(npc) = npc {
                ctx.delete_npc(npc);
            }
            htmltext = Some(if ctx.get_random(2) == 0 {
                event.to_owned()
            } else {
                "32013-04a.htm".to_owned()
            });
        }
        htmltext
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        match npc.id {
            SUKI => match qs.state(ctx) {
                CREATED => {
                    htmltext = if ctx.player_level(player) >= MIN_LEVEL {
                        "32013-01.htm"
                    } else {
                        "32013-01a.htm"
                    }
                    .to_owned();
                }
                STARTED => htmltext = "32013-02.htm".to_owned(),
                _ => {}
            },
            GALIBREDO => {
                if qs.is_started(ctx) {
                    ctx.give_adena(player, 2553, true);
                    qs.exit_quest(ctx, true, true);
                    htmltext = "30181-01.html".to_owned();
                }
            }
            _ => {}
        }
        Some(htmltext)
    }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::{Pages, ScriptTest, rows};

    const NAME: &str = "Q00653_WildMaiden";

    const NPCS: &str = r#"
        <npc id="30181" type="Folk" level="1" name="Galibredo"/>
        <npc id="32013" type="Folk" level="1" name="Suki"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="736" type="EtcItem" name="Scroll of Escape"><set name="is_questitem" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00653WildMaiden);
        let pages = Pages::of(&Q00653WildMaiden)
            .page("32013-01.htm", &["32013-04.htm", "32013-03.html"])
            .plain(&[
                "32013-01a.htm",
                "32013-02.htm",
                "32013-03.html",
                "32013-04.htm",
                "32013-04a.htm",
                "32013-05.htm",
                "30181-01.html",
            ])
            .npcs(&[GALIBREDO, SUKI]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let suki = t.spawn(SUKI, 20);
        let galibredo = t.spawn(GALIBREDO, 30);
        (t, suki, galibredo)
    }

    #[test]
    fn a_level_36_character_starts_with_the_scroll() {
        let (mut t, suki, _) = setup();
        t.enter(1, 36, vec![]);
        t.world.run_as_script(NAME, |ctx| {
            ctx.give_items(Oid(1), SOE, 1);
        });
        assert!(t.quest_page(1, suki).contains("32013-01.htm"));
        t.take_writes();

        // The start page is 32013-04.htm or 32013-04a.htm (the coin toss).
        let page = t.event(1, NAME, "32013-04.htm");
        assert!(
            page.contains("32013-04.htm") || page.contains("32013-04a.htm"),
            "{page}"
        );
        assert_eq!(t.count(1, SOE), 0);
        assert!(!t.in_world(suki), "the maiden leaves");
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
    }

    #[test]
    fn a_level_35_character_is_told_to_come_back() {
        let (mut t, suki, _) = setup();
        t.enter(1, 35, vec![]);
        assert!(t.quest_page(1, suki).contains("32013-01a.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn without_the_scroll_the_maiden_refuses() {
        let (mut t, suki, _) = setup();
        t.enter(1, 36, vec![]);
        assert!(t.quest_page(1, suki).contains("32013-01.htm"));
        assert!(t.event(1, NAME, "32013-04.htm").contains("32013-05.htm"));
        assert!(t.in_world(suki));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn galibredo_pays_adena_and_the_quest_ends() {
        let (mut t, suki, galibredo) = setup();
        t.enter(1, 36, vec![]);
        t.world.run_as_script(NAME, |ctx| {
            ctx.give_items(Oid(1), SOE, 1);
        });
        t.quest_page(1, suki);
        t.event(1, NAME, "32013-04.htm");
        t.take();
        t.take_writes();

        assert!(t.quest_page(1, galibredo).contains("30181-01.html"));
        assert_eq!(t.count(1, 57), 2553);
        assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
    }
}
