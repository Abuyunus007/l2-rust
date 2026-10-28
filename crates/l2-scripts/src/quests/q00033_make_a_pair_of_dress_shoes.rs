//! Quest 33: Make a Pair of Dress Shoes (Woodley in the Town of Gludin sends a 60th level character to Leikar and Ian).
// Java: dist/game/data/scripts/quests/Q00033_MakeAPairOfDressShoes/Q00033_MakeAPairOfDressShoes.java
//
// Pattern: a crafting errand that pays in adena (`docs/design/scripts.md`, "Quest patterns"):
// Leikar starts the material hunt (cond 2), Woodley checks the leather, thread and adena and
// takes the adena for the first payment (cond 4), Ian takes the second payment (cond 5), and
// Woodley hands out the dress shoes box. One-time: `exit_quest(ctx, false, true)`.

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const IAN: i32 = 30164;
const WOODLEY: i32 = 30838;
const LEIKAR: i32 = 31520;

// Items
const LEATHER: i32 = 1882;
const THREAD: i32 = 1868;
const DRESS_SHOES_BOX: i32 = 7113;
/// Java `Inventory.ADENA_ID`.
const ADENA: i32 = 57;

// Misc
const MIN_LEVEL: i32 = 60;
const LEATHER_COUNT: i64 = 200;
const THREAD_COUNT: i64 = 600;
const ADENA_COUNT: i64 = 500_000;
const ADENA_COUNT2: i64 = 200_000;
const ADENA_COUNT3: i64 = 300_000;

/// Java `Q00033_MakeAPairOfDressShoes`.
pub struct Q00033MakeAPairOfDressShoes;

impl Script for Q00033MakeAPairOfDressShoes {
    fn name(&self) -> &'static str {
        "Q00033_MakeAPairOfDressShoes"
    }

    fn path(&self) -> &'static str {
        "quests/Q00033_MakeAPairOfDressShoes"
    }

    fn id(&self) -> i32 {
        33
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[WOODLEY]);
        s.add_talk_id(&[WOODLEY, IAN, LEIKAR]);
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
            "30838-03.htm" => qs.start_quest(ctx),
            "30838-06.html" => qs.set_cond(ctx, 3, true),
            "30838-09.html" => {
                if ctx.get_quest_items_count(player, LEATHER) >= LEATHER_COUNT
                    && ctx.get_quest_items_count(player, THREAD) >= THREAD_COUNT
                    && ctx.get_quest_items_count(player, ADENA) >= ADENA_COUNT2
                {
                    ctx.take_items(player, LEATHER, LEATHER_COUNT);
                    // NOTE(java-quirk): Java takes `LEATHER_COUNT` (200) threads here, not
                    // `THREAD_COUNT` (600), so 400 threads stay with the player.
                    ctx.take_items(player, THREAD, LEATHER_COUNT);
                    ctx.take_items(player, ADENA, ADENA_COUNT2);
                    qs.set_cond(ctx, 4, true);
                } else {
                    return Some("30838-10.html".to_owned());
                }
            }
            "30838-13.html" => {
                ctx.give_items(player, DRESS_SHOES_BOX, 1);
                qs.exit_quest(ctx, false, true);
            }
            "31520-02.html" => qs.set_cond(ctx, 2, true),
            "30164-02.html" => {
                if ctx.get_quest_items_count(player, ADENA) < ADENA_COUNT3 {
                    return Some("30164-03.html".to_owned());
                }
                ctx.take_items(player, ADENA, ADENA_COUNT3);
                qs.set_cond(ctx, 5, true);
            }
            _ => return None,
        }
        Some(event.to_owned())
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        match npc.id {
            WOODLEY => match qs.state(ctx) {
                CREATED => {
                    htmltext = if ctx.player_level(player) >= MIN_LEVEL {
                        "30838-01.htm"
                    } else {
                        "30838-02.html"
                    }
                    .to_owned();
                }
                STARTED => match qs.get_cond(ctx) {
                    1 => htmltext = "30838-04.html".to_owned(),
                    2 => htmltext = "30838-05.html".to_owned(),
                    3 => {
                        htmltext = if ctx.get_quest_items_count(player, LEATHER) >= LEATHER_COUNT
                            && ctx.get_quest_items_count(player, THREAD) >= THREAD_COUNT
                            && ctx.get_quest_items_count(player, ADENA) >= ADENA_COUNT
                        {
                            "30838-07.html"
                        } else {
                            "30838-08.html"
                        }
                        .to_owned();
                    }
                    4 => htmltext = "30838-11.html".to_owned(),
                    5 => htmltext = "30838-12.html".to_owned(),
                    _ => {}
                },
                COMPLETED => htmltext = ctx.get_already_completed_msg(player),
                _ => {}
            },
            LEIKAR => {
                if qs.is_started(ctx) {
                    if qs.is_cond(ctx, 1) {
                        htmltext = "31520-01.html".to_owned();
                    } else if qs.is_cond(ctx, 2) {
                        htmltext = "31520-03.html".to_owned();
                    }
                }
            }
            IAN => {
                if qs.is_started(ctx) {
                    if qs.is_cond(ctx, 4) {
                        htmltext = "30164-01.html".to_owned();
                    } else if qs.is_cond(ctx, 5) {
                        htmltext = "30164-04.html".to_owned();
                    }
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
    use crate::testing::{Pages, ScriptTest, item, rows};

    const NAME: &str = "Q00033_MakeAPairOfDressShoes";

    const NPCS: &str = r#"
        <npc id="30164" type="Folk" level="70" name="Ian"/>
        <npc id="30838" type="Folk" level="70" name="Woodley"/>
        <npc id="31520" type="Folk" level="70" name="Leikar"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="1882" type="EtcItem" name="Leather"><set name="is_stackable" val="true"/></item>
        <item id="1868" type="EtcItem" name="Thread"><set name="is_stackable" val="true"/></item>
        <item id="7113" type="EtcItem" name="Dress Shoes Box"/>"#;

    struct Npcs {
        woodley: Oid,
        leikar: Oid,
        ian: Oid,
    }

    fn setup() -> (ScriptTest, Npcs) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00033MakeAPairOfDressShoes);
        let pages = Pages::of(&Q00033MakeAPairOfDressShoes)
            .page("30838-01.htm", &["30838-03.htm"])
            .page("30838-05.html", &["30838-06.html"])
            .page("30838-07.html", &["30838-09.html"])
            .page("30838-12.html", &["30838-13.html"])
            .page("31520-01.html", &["31520-02.html"])
            .page("30164-01.html", &["30164-02.html"])
            .plain(&[
                "30838-02.html",
                "30838-03.htm",
                "30838-04.html",
                "30838-06.html",
                "30838-08.html",
                "30838-09.html",
                "30838-10.html",
                "30838-11.html",
                "30838-13.html",
                "31520-02.html",
                "31520-03.html",
                "30164-02.html",
                "30164-03.html",
                "30164-04.html",
            ])
            .npcs(&[WOODLEY, LEIKAR, IAN]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let npcs = Npcs {
            woodley: t.spawn(WOODLEY, 20),
            leikar: t.spawn(LEIKAR, 40),
            ian: t.spawn(IAN, 60),
        };
        (t, npcs)
    }

    /// The materials and the adena of the whole errand: 200 leather, 600 thread, 500000 adena.
    fn materials() -> Vec<l2_world::ItemData> {
        vec![
            item(100, LEATHER, 200),
            item(101, THREAD, 600),
            item(102, ADENA, ADENA_COUNT),
        ]
    }

    #[test]
    fn woodley_wants_level_60() {
        let (mut t, npcs) = setup();
        t.enter(1, 59, vec![]);
        assert!(t.quest_page(1, npcs.woodley).contains("30838-02.html"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_materials_the_two_payments_and_the_box() {
        let (mut t, npcs) = setup();
        t.enter(1, 60, materials());
        assert!(t.quest_page(1, npcs.woodley).contains("30838-01.htm"));
        assert!(t.event(1, NAME, "30838-03.htm").contains("30838-03.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );

        // Leikar sends the player on (cond 2) and Woodley to the materials (cond 3).
        assert!(t.quest_page(1, npcs.leikar).contains("31520-01.html"));
        assert!(t.event(1, NAME, "31520-02.html").contains("31520-02.html"));
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);
        assert!(t.quest_page(1, npcs.woodley).contains("30838-05.html"));
        assert!(t.event(1, NAME, "30838-06.html").contains("30838-06.html"));
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "3")]);

        // The first payment: 200 leather, 200 thread (the Java quirk), 200000 adena (cond 4).
        assert!(t.quest_page(1, npcs.woodley).contains("30838-07.html"));
        assert!(t.event(1, NAME, "30838-09.html").contains("30838-09.html"));
        assert_eq!(t.count(1, LEATHER), 0);
        assert_eq!(t.count(1, THREAD), 400);
        assert_eq!(t.count(1, ADENA), 300_000);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "4")]);
        assert!(t.quest_page(1, npcs.woodley).contains("30838-11.html"));

        // Ian takes the second payment (cond 5).
        assert!(t.quest_page(1, npcs.ian).contains("30164-01.html"));
        assert!(t.event(1, NAME, "30164-02.html").contains("30164-02.html"));
        assert_eq!(t.count(1, ADENA), 0);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "5")]);
        assert!(t.quest_page(1, npcs.ian).contains("30164-04.html"));

        // Woodley hands out the box: `exitQuest(false, true)`.
        assert!(t.quest_page(1, npcs.woodley).contains("30838-12.html"));
        assert!(t.event(1, NAME, "30838-13.html").contains("30838-13.html"));
        assert_eq!(t.count(1, DRESS_SHOES_BOX), 1);
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed")
            ]
        );
        assert!(
            t.quest_page(1, npcs.woodley)
                .contains("This quest has already been completed.")
        );
    }

    #[test]
    fn woodley_wants_the_materials_before_the_first_payment() {
        let (mut t, npcs) = setup();
        t.enter(1, 60, vec![item(100, LEATHER, 200)]);
        t.quest_page(1, npcs.woodley);
        t.event(1, NAME, "30838-03.htm");
        t.quest_page(1, npcs.leikar);
        t.event(1, NAME, "31520-02.html");
        t.quest_page(1, npcs.woodley);
        t.event(1, NAME, "30838-06.html");
        t.take_writes();
        // No thread and no adena: Woodley sends the player back for them.
        assert!(t.quest_page(1, npcs.woodley).contains("30838-08.html"));
        assert!(t.take_writes().is_empty());
    }
}
