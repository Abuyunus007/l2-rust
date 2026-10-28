//! Quest 18: Meeting with the Golden Ram (Donal and Daisy in the Town of Gludio, Abercrombie in the Gludin Village).
// Java: dist/game/data/scripts/quests/Q00018_MeetingWithTheGoldenRam/Q00018_MeetingWithTheGoldenRam.java
//
// Pattern: a delivery chain (`docs/design/scripts.md`, "Quest patterns"): Donal starts it at
// level 66, Daisy hands out the box (cond 2), and Abercrombie takes it for the reward. One-time:
// `exit_quest(ctx, false, true)`.

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const DONAL: i32 = 31314;
const DAISY: i32 = 31315;
const ABERCROMBIE: i32 = 31555;

// Item
const BOX: i32 = 7245;

// Misc
const MIN_LEVEL: i32 = 66;

/// Java `Q00018_MeetingWithTheGoldenRam`.
pub struct Q00018MeetingWithTheGoldenRam;

impl Script for Q00018MeetingWithTheGoldenRam {
    fn name(&self) -> &'static str {
        "Q00018_MeetingWithTheGoldenRam"
    }

    fn path(&self) -> &'static str {
        "quests/Q00018_MeetingWithTheGoldenRam"
    }

    fn id(&self) -> i32 {
        18
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[DONAL]);
        s.add_talk_id(&[DONAL, DAISY, ABERCROMBIE]);
        s.register_quest_items(&[BOX]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        _npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        // NOTE(java-quirk): without a quest state `onEvent` returns the event itself (not null),
        // so the event's page is shown anyway.
        let Some(player) = player else {
            return Some(event.to_owned());
        };
        let Some(qs) = ctx.get_quest_state(player, false) else {
            return Some(event.to_owned());
        };

        let mut htmltext = event.to_owned();
        match event {
            "31314-03.html" => {
                if ctx.player_level(player) >= MIN_LEVEL {
                    qs.start_quest(ctx);
                } else {
                    htmltext = "31314-02.html".to_owned();
                }
            }
            "31315-02.html" => {
                qs.set_cond(ctx, 2, true);
                ctx.give_items(player, BOX, 1);
            }
            "31555-02.html" => {
                if ctx.has_quest_items(player, BOX) {
                    ctx.give_adena(player, 40_000, true);
                    ctx.add_exp_and_sp(player, 126_668, 11_731);
                    qs.exit_quest(ctx, false, true);
                }
            }
            _ => {}
        }
        Some(htmltext)
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        let npc_id = npc.id;

        match qs.state(ctx) {
            COMPLETED => htmltext = ctx.get_already_completed_msg(player),
            CREATED => {
                if npc_id == DONAL {
                    htmltext = "31314-01.htm".to_owned();
                }
            }
            STARTED => {
                if npc_id == DONAL {
                    htmltext = "31314-04.html".to_owned();
                } else if npc_id == DAISY {
                    htmltext = if qs.get_cond(ctx) < 2 {
                        "31315-01.html"
                    } else {
                        "31315-03.html"
                    }
                    .to_owned();
                } else if npc_id == ABERCROMBIE && qs.is_cond(ctx, 2) && ctx.has_quest_items(player, BOX) {
                    htmltext = "31555-01.html".to_owned();
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

    const NAME: &str = "Q00018_MeetingWithTheGoldenRam";

    const NPCS: &str = r#"
        <npc id="31314" type="Folk" level="70" name="Donal"/>
        <npc id="31315" type="Folk" level="70" name="Daisy"/>
        <npc id="31555" type="Folk" level="70" name="Abercrombie"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="7245" type="EtcItem" name="Box"><set name="is_questitem" val="true"/></item>"#;

    struct Npcs {
        donal: Oid,
        daisy: Oid,
        abercrombie: Oid,
    }

    fn setup() -> (ScriptTest, Npcs) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00018MeetingWithTheGoldenRam);
        let pages = Pages::of(&Q00018MeetingWithTheGoldenRam)
            .page("31314-01.htm", &["31314-03.html"])
            .page("31315-01.html", &["31315-02.html"])
            .page("31555-01.html", &["31555-02.html"])
            .plain(&[
                "31314-02.html",
                "31314-03.html",
                "31314-04.html",
                "31315-02.html",
                "31315-03.html",
                "31555-02.html",
            ])
            .npcs(&[DONAL, DAISY, ABERCROMBIE]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let npcs = Npcs {
            donal: t.spawn(DONAL, 20),
            daisy: t.spawn(DAISY, 40),
            abercrombie: t.spawn(ABERCROMBIE, 60),
        };
        (t, npcs)
    }

    #[test]
    fn donal_wants_level_66() {
        let (mut t, npcs) = setup();
        t.enter(1, 65, vec![]);
        assert!(t.quest_page(1, npcs.donal).contains("31314-01.htm"));
        assert!(t.event(1, NAME, "31314-03.html").contains("31314-02.html"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_box_from_daisy_to_abercrombie() {
        let (mut t, npcs) = setup();
        t.enter(1, 66, vec![]);
        assert!(t.quest_page(1, npcs.donal).contains("31314-01.htm"));
        assert!(t.event(1, NAME, "31314-03.html").contains("31314-03.html"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        assert!(t.quest_page(1, npcs.donal).contains("31314-04.html"));

        // Daisy gives the box (cond 2).
        assert!(t.quest_page(1, npcs.daisy).contains("31315-01.html"));
        assert!(t.event(1, NAME, "31315-02.html").contains("31315-02.html"));
        assert_eq!(t.count(1, BOX), 1);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);
        assert!(t.quest_page(1, npcs.daisy).contains("31315-03.html"));

        // Abercrombie: the adena and the exp, `exitQuest(false, true)`.
        assert!(t.quest_page(1, npcs.abercrombie).contains("31555-01.html"));
        assert!(t.event(1, NAME, "31555-02.html").contains("31555-02.html"));
        assert_eq!(t.count(1, 57), 40_000);
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed")
            ]
        );
        assert!(
            t.quest_page(1, npcs.donal)
                .contains("This quest has already been completed.")
        );
    }

    #[test]
    fn abercrombie_says_nothing_before_the_box() {
        let (mut t, npcs) = setup();
        t.enter(1, 66, vec![]);
        t.quest_page(1, npcs.donal);
        t.event(1, NAME, "31314-03.html");
        t.take_writes();
        assert!(!t.quest_page(1, npcs.abercrombie).contains("31555-01.html"));
        assert!(t.take_writes().is_empty());
    }
}
