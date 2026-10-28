//! Quest 26: Tired of Waiting (Isael Silvershadow in the Town of Giran sends an 80th level character to Kitzka).
// Java: dist/game/data/scripts/quests/Q00026_TiredOfWaiting/Q00026_TiredOfWaiting.java
//
// Pattern: a delivery to one NPC with a choice of rewards (`docs/design/scripts.md`, "Quest
// patterns"): Isael gives the delivery box on the start, Kitzka takes it on the page that names
// the reward, and the reward item comes with `exit_quest(ctx, false, true)`.

use l2_util::ObjectId;
use l2_world::script::{CREATED, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const ISAEL_SILVERSHADOW: i32 = 30655;
const KITZKA: i32 = 31045;

// Items
const DELIVERY_BOX: i32 = 17281;
/// Java `REWARDS`: the reward page of Kitzka and its item (Large Dragon Bone, Will of Antharas,
/// Sealed Blood Crystal).
const REWARDS: [(&str, i32); 3] = [
    ("31045-10.html", 17248),
    ("31045-11.html", 17266),
    ("31045-12.html", 17267),
];

// Misc
const MIN_LEVEL: i32 = 80;

/// Java `Q00026_TiredOfWaiting`.
pub struct Q00026TiredOfWaiting;

impl Script for Q00026TiredOfWaiting {
    fn name(&self) -> &'static str {
        "Q00026_TiredOfWaiting"
    }

    fn path(&self) -> &'static str {
        "quests/Q00026_TiredOfWaiting"
    }

    fn id(&self) -> i32 {
        26
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[ISAEL_SILVERSHADOW]);
        s.add_talk_id(&[ISAEL_SILVERSHADOW, KITZKA]);
        s.register_quest_items(&[DELIVERY_BOX]);
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
            "30655-02.htm" | "30655-03.htm" | "30655-05.html" | "30655-06.html" | "31045-02.html"
            | "31045-03.html" | "31045-05.html" | "31045-06.html" | "31045-07.html" | "31045-08.html"
            | "31045-09.html" => {}
            "30655-04.html" => {
                if !qs.is_created(ctx) {
                    return None;
                }
                ctx.give_items(player, DELIVERY_BOX, 1);
                qs.start_quest(ctx);
            }
            "31045-04.html" => {
                if !qs.is_started(ctx) {
                    return None;
                }
                ctx.take_items(player, DELIVERY_BOX, -1);
            }
            "31045-10.html" | "31045-11.html" | "31045-12.html" => {
                if !qs.is_started(ctx) {
                    return None;
                }
                if let Some((_, item)) = REWARDS.iter().find(|(page, _)| *page == event) {
                    ctx.give_items(player, *item, 1);
                }
                qs.exit_quest(ctx, false, true);
            }
            _ => return None,
        }
        Some(event.to_owned())
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        match npc.id {
            ISAEL_SILVERSHADOW => {
                let state = qs.state(ctx);
                if state == CREATED {
                    htmltext = if ctx.player_level(player) >= MIN_LEVEL {
                        "30655-01.htm"
                    } else {
                        "30655-00.html"
                    }
                    .to_owned();
                } else if state == STARTED {
                    htmltext = "30655-07.html".to_owned();
                } else {
                    // NOTE(java-quirk): a completed quest gets this page, not the "already
                    // completed" message (the Java `else` covers every other state).
                    htmltext = "30655-08.html".to_owned();
                }
            }
            KITZKA => {
                if qs.is_started(ctx) {
                    htmltext = if ctx.has_quest_items(player, DELIVERY_BOX) {
                        "31045-01.html"
                    } else {
                        "31045-09.html"
                    }
                    .to_owned();
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

    const NAME: &str = "Q00026_TiredOfWaiting";

    const NPCS: &str = r#"
        <npc id="30655" type="Folk" level="70" name="Isael Silvershadow"/>
        <npc id="31045" type="Folk" level="70" name="Kitzka"/>"#;

    const ITEMS: &str = r#"
        <item id="17281" type="EtcItem" name="Delivery Box"><set name="is_questitem" val="true"/></item>
        <item id="17248" type="EtcItem" name="Large Dragon Bone"/>
        <item id="17266" type="EtcItem" name="Will of Antharas"/>
        <item id="17267" type="EtcItem" name="Sealed Blood Crystal"/>"#;

    struct Npcs {
        isael: Oid,
        kitzka: Oid,
    }

    fn setup() -> (ScriptTest, Npcs) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00026TiredOfWaiting);
        let pages = Pages::of(&Q00026TiredOfWaiting)
            .page("30655-01.htm", &["30655-02.htm"])
            .page("30655-02.htm", &["30655-03.htm"])
            .page("30655-03.htm", &["30655-04.html"])
            .page("30655-04.html", &["30655-05.html"])
            .page("30655-05.html", &["30655-06.html"])
            .plain(&["30655-00.html", "30655-06.html", "30655-07.html", "30655-08.html"])
            .page("31045-01.html", &["31045-02.html"])
            .page("31045-02.html", &["31045-03.html", "31045-04.html"])
            .page("31045-04.html", &["31045-05.html"])
            .page(
                "31045-05.html",
                &["31045-06.html", "31045-07.html", "31045-08.html"],
            )
            .page("31045-08.html", &["31045-12.html", "31045-09.html"])
            .plain(&[
                "31045-03.html",
                "31045-06.html",
                "31045-07.html",
                "31045-09.html",
                "31045-12.html",
            ])
            .npcs(&[ISAEL_SILVERSHADOW, KITZKA]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let npcs = Npcs {
            isael: t.spawn(ISAEL_SILVERSHADOW, 20),
            kitzka: t.spawn(KITZKA, 40),
        };
        (t, npcs)
    }

    #[test]
    fn isael_wants_level_80() {
        let (mut t, npcs) = setup();
        t.enter(1, 79, vec![]);
        assert!(t.quest_page(1, npcs.isael).contains("30655-00.html"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn kitzka_has_nothing_more_to_say_once_the_box_is_handed_over() {
        let (mut t, npcs) = setup();
        t.enter(1, 80, vec![]);
        t.quest_page(1, npcs.isael);
        t.event(1, NAME, "30655-02.htm");
        t.event(1, NAME, "30655-03.htm");
        t.event(1, NAME, "30655-04.html");
        t.take_writes();
        assert!(t.quest_page(1, npcs.kitzka).contains("31045-01.html"));
        assert!(t.event(1, NAME, "31045-02.html").contains("31045-02.html"));
        assert!(t.event(1, NAME, "31045-04.html").contains("31045-04.html"));
        assert_eq!(t.count(1, DELIVERY_BOX), 0);
        assert!(t.quest_page(1, npcs.kitzka).contains("31045-09.html"));
    }

    #[test]
    fn the_box_from_isael_to_kitzka_and_the_crystal() {
        let (mut t, npcs) = setup();
        t.enter(1, 80, vec![]);
        assert!(t.quest_page(1, npcs.isael).contains("30655-01.htm"));
        assert!(t.event(1, NAME, "30655-02.htm").contains("30655-02.htm"));
        assert!(t.event(1, NAME, "30655-03.htm").contains("30655-03.htm"));
        assert!(t.event(1, NAME, "30655-04.html").contains("30655-04.html"));
        // The box is given before the quest starts (Java's order).
        assert_eq!(t.count(1, DELIVERY_BOX), 1);
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        assert!(t.quest_page(1, npcs.isael).contains("30655-07.html"));

        // Kitzka: the box is in hand, so the box page; the box goes on the page that names it.
        assert!(t.quest_page(1, npcs.kitzka).contains("31045-01.html"));
        assert!(t.event(1, NAME, "31045-02.html").contains("31045-02.html"));
        assert!(t.event(1, NAME, "31045-04.html").contains("31045-04.html"));
        assert_eq!(t.count(1, DELIVERY_BOX), 0);
        assert!(t.take_writes().is_empty());

        // The reward page: the Sealed Blood Crystal and `exitQuest(false, true)`.
        assert!(t.event(1, NAME, "31045-05.html").contains("31045-05.html"));
        assert!(t.event(1, NAME, "31045-08.html").contains("31045-08.html"));
        assert!(t.event(1, NAME, "31045-12.html").contains("31045-12.html"));
        assert_eq!(t.count(1, 17267), 1);
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed")
            ]
        );

        // NOTE(java-quirk): a completed quest sees page 08 from Isael, not the completed message.
        assert!(t.quest_page(1, npcs.isael).contains("30655-08.html"));
    }
}
