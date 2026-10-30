//! Quest 651: Runaway Youth (Ivan, the boy who ran from home, and his father Batidae in Schuttgart).
// Java: dist/game/data/scripts/quests/Q00651_RunawayYouth/Q00651_RunawayYouth.java
//
// Pattern: a level-gated quest that takes an item on start (the Scroll of Escape), deletes the
// start NPC when the quest begins (`delete_npc`), and pays adena from a second NPC.

use l2_util::ObjectId;
use l2_world::script::{CREATED, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const BATIDAE: i32 = 31989;
const IVAN: i32 = 32014;

// Item
const SOE: i32 = 736;

// Misc
const MIN_LEVEL: i32 = 26;

/// Java `Q00651_RunawayYouth`.
pub struct Q00651RunawayYouth;

impl Script for Q00651RunawayYouth {
    fn name(&self) -> &'static str {
        "Q00651_RunawayYouth"
    }

    fn path(&self) -> &'static str {
        "quests/Q00651_RunawayYouth"
    }

    fn id(&self) -> i32 {
        651
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[IVAN]);
        s.add_talk_id(&[BATIDAE, IVAN]);
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
        if event == "32014-03.html" {
            htmltext = Some(event.to_owned());
        } else if event == "32014-04.htm" {
            if !ctx.has_quest_items(player, SOE) {
                return Some("32014-05.htm".to_owned());
            }

            qs.start_quest(ctx);
            ctx.take_items(player, SOE, 1);
            // The boy leaves with the scroll: Java `npc.deleteMe()`.
            if let Some(npc) = npc {
                ctx.delete_npc(npc);
            }
            htmltext = Some(event.to_owned());
        }
        htmltext
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        match npc.id {
            IVAN => match qs.state(ctx) {
                CREATED => {
                    htmltext = if ctx.player_level(player) >= MIN_LEVEL {
                        "32014-01.htm"
                    } else {
                        "32014-01a.htm"
                    }
                    .to_owned();
                }
                STARTED => htmltext = "32014-02.html".to_owned(),
                _ => {}
            },
            BATIDAE => {
                if qs.is_started(ctx) {
                    ctx.give_adena(player, 2883, true);
                    qs.exit_quest(ctx, true, true);
                    htmltext = "31989-01.html".to_owned();
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

    const NAME: &str = "Q00651_RunawayYouth";

    const NPCS: &str = r#"
        <npc id="31989" type="Folk" level="30" name="Batidae"/>
        <npc id="32014" type="Folk" level="1" name="Ivan"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="736" type="EtcItem" name="Scroll of Escape"><set name="is_questitem" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00651RunawayYouth);
        let pages = Pages::of(&Q00651RunawayYouth)
            .page("32014-01.htm", &["32014-04.htm", "32014-03.html"])
            .plain(&[
                "32014-01a.htm",
                "32014-02.html",
                "32014-03.html",
                "32014-04.htm",
                "32014-05.htm",
                "31989-01.html",
            ])
            .npcs(&[BATIDAE, IVAN]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let ivan = t.spawn(IVAN, 20);
        let batidae = t.spawn(BATIDAE, 30);
        (t, ivan, batidae)
    }

    #[test]
    fn a_level_26_character_starts_with_the_scroll() {
        let (mut t, ivan, _) = setup();
        t.enter(1, 26, vec![]);
        t.world.run_as_script(NAME, |ctx| {
            ctx.give_items(Oid(1), SOE, 1);
        });
        assert!(t.quest_page(1, ivan).contains("32014-01.htm"));
        t.take_writes();

        assert!(t.event(1, NAME, "32014-04.htm").contains("32014-04.htm"));
        assert_eq!(t.count(1, SOE), 0);
        assert!(!t.in_world(ivan), "the boy leaves");
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
    }

    #[test]
    fn a_level_25_character_is_told_to_come_back_and_no_scroll_means_no_start() {
        let (mut t, ivan, _) = setup();
        t.enter(1, 25, vec![]);
        assert!(t.quest_page(1, ivan).contains("32014-01a.htm"));
        assert!(t.take_writes().is_empty());
        assert!(t.in_world(ivan));
    }

    #[test]
    fn without_the_scroll_the_boy_refuses_and_stays() {
        let (mut t, ivan, _) = setup();
        t.enter(1, 26, vec![]);
        assert!(t.quest_page(1, ivan).contains("32014-01.htm"));
        assert!(t.event(1, NAME, "32014-04.htm").contains("32014-05.htm"));
        assert!(t.in_world(ivan));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_father_pays_adena_and_the_quest_ends() {
        let (mut t, ivan, batidae) = setup();
        t.enter(1, 26, vec![]);
        t.world.run_as_script(NAME, |ctx| {
            ctx.give_items(Oid(1), SOE, 1);
        });
        t.quest_page(1, ivan);
        t.event(1, NAME, "32014-04.htm");
        t.take();
        t.take_writes();

        assert!(t.quest_page(1, batidae).contains("31989-01.html"));
        assert_eq!(t.count(1, 57), 2883);
        assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
    }
}
