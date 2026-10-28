//! Quest 40: A Special Order (Helvetia in the Gludin Village, Ofulle the fisherman and Gesto
//! the seed merchant, who fill the two boxes).
// Java: dist/game/data/scripts/quests/Q00040_ASpecialOrder/Q00040_ASpecialOrder.java
//
// Pattern: a delivery chain with a random branch (`docs/design/scripts.md`, "Quest patterns"):
// the accept picks one of two chains at random (cond 2 for the fish, cond 5 for the seeds), and
// Helvetia's reward ends both.

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, QuestSound, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const HELVETIA: i32 = 30081;
const OFULLE: i32 = 31572;
const GESTO: i32 = 30511;

// Items
const ORANGE_SWIFT_FISH: i32 = 6450;
const ORANGE_UGLY_FISH: i32 = 6451;
const ORANGE_WIDE_FISH: i32 = 6452;
const GOLDEN_COBOL: i32 = 5079;
const BUR_COBOL: i32 = 5082;
const GREAT_COBOL: i32 = 5084;
const WONDROUS_CUBIC: i32 = 10632;
const BOX_OF_FISH: i32 = 12764;
const BOX_OF_SEED: i32 = 12765;

// Misc
const MIN_LEVEL: i32 = 40;

/// Java `Q00040_ASpecialOrder`.
pub struct Q00040ASpecialOrder;

impl Script for Q00040ASpecialOrder {
    fn name(&self) -> &'static str {
        "Q00040_ASpecialOrder"
    }

    fn path(&self) -> &'static str {
        "quests/Q00040_ASpecialOrder"
    }

    fn id(&self) -> i32 {
        40
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[HELVETIA]);
        s.add_talk_id(&[HELVETIA, OFULLE, GESTO]);
        s.register_quest_items(&[BOX_OF_FISH, BOX_OF_SEED]);
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

        let mut htmltext = None;
        match event {
            "accept" => {
                qs.set_state(ctx, STARTED);
                ctx.play_sound(player, QuestSound::ItemsoundQuestAccept);
                if ctx.get_random_boolean() {
                    qs.set_cond(ctx, 2, false);
                    htmltext = Some("30081-03.html".to_owned());
                } else {
                    qs.set_cond(ctx, 5, false);
                    htmltext = Some("30081-04.html".to_owned());
                }
            }
            "30081-07.html" => {
                if qs.is_cond(ctx, 4) && ctx.has_quest_items(player, BOX_OF_FISH) {
                    ctx.reward_items(player, WONDROUS_CUBIC, 1);
                    qs.exit_quest(ctx, false, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "30081-10.html" => {
                if qs.is_cond(ctx, 7) && ctx.has_quest_items(player, BOX_OF_SEED) {
                    ctx.reward_items(player, WONDROUS_CUBIC, 1);
                    qs.exit_quest(ctx, false, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "31572-02.html" | "30511-02.html" => {
                htmltext = Some(event.to_owned());
            }
            "31572-03.html" => {
                if qs.is_cond(ctx, 2) {
                    qs.set_cond(ctx, 3, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "30511-03.html" => {
                if qs.is_cond(ctx, 5) {
                    qs.set_cond(ctx, 6, true);
                    htmltext = Some(event.to_owned());
                }
            }
            _ => {}
        }
        htmltext
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        match npc.id {
            HELVETIA => match qs.state(ctx) {
                CREATED => {
                    htmltext = if ctx.player_level(player) >= MIN_LEVEL {
                        "30081-01.htm"
                    } else {
                        "30081-02.htm"
                    }
                    .to_owned();
                }
                STARTED => match qs.get_cond(ctx) {
                    2 | 3 => htmltext = "30081-05.html".to_owned(),
                    4 => {
                        if ctx.has_quest_items(player, BOX_OF_FISH) {
                            htmltext = "30081-06.html".to_owned();
                        }
                    }
                    5 | 6 => htmltext = "30081-08.html".to_owned(),
                    7 => {
                        if ctx.has_quest_items(player, BOX_OF_SEED) {
                            htmltext = "30081-09.html".to_owned();
                        }
                    }
                    _ => {}
                },
                COMPLETED => htmltext = ctx.get_already_completed_msg(player),
                _ => {}
            },
            OFULLE => match qs.get_cond(ctx) {
                2 => htmltext = "31572-01.html".to_owned(),
                3 => {
                    if ctx.get_quest_items_count(player, ORANGE_SWIFT_FISH) >= 10
                        && ctx.get_quest_items_count(player, ORANGE_UGLY_FISH) >= 10
                        && ctx.get_quest_items_count(player, ORANGE_WIDE_FISH) >= 10
                    {
                        qs.set_cond(ctx, 4, true);
                        ctx.give_items(player, BOX_OF_FISH, 1);
                        ctx.take_items_many(
                            player,
                            10,
                            &[ORANGE_SWIFT_FISH, ORANGE_UGLY_FISH, ORANGE_WIDE_FISH],
                        );
                        htmltext = "31572-05.html".to_owned();
                    } else {
                        htmltext = "31572-04.html".to_owned();
                    }
                }
                4 => htmltext = "31572-06.html".to_owned(),
                _ => {}
            },
            GESTO => match qs.get_cond(ctx) {
                5 => htmltext = "30511-01.html".to_owned(),
                6 => {
                    if ctx.get_quest_items_count(player, GOLDEN_COBOL) >= 40
                        && ctx.get_quest_items_count(player, BUR_COBOL) >= 40
                        && ctx.get_quest_items_count(player, GREAT_COBOL) >= 40
                    {
                        qs.set_cond(ctx, 7, true);
                        ctx.give_items(player, BOX_OF_SEED, 1);
                        ctx.take_items_many(player, 40, &[GOLDEN_COBOL, BUR_COBOL, GREAT_COBOL]);
                        htmltext = "30511-05.html".to_owned();
                    } else {
                        htmltext = "30511-04.html".to_owned();
                    }
                }
                7 => htmltext = "30511-06.html".to_owned(),
                _ => {}
            },
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

    const NAME: &str = "Q00040_ASpecialOrder";

    const NPCS: &str = r#"
        <npc id="30081" type="Folk" level="70" name="Helvetia"/>
        <npc id="31572" type="Folk" level="70" name="Ofulle"/>
        <npc id="30511" type="Folk" level="70" name="Gesto"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="6450" type="EtcItem" name="Orange Swift Fish"><set name="is_stackable" val="true"/></item>
        <item id="6451" type="EtcItem" name="Orange Ugly Fish"><set name="is_stackable" val="true"/></item>
        <item id="6452" type="EtcItem" name="Orange Wide Fish"><set name="is_stackable" val="true"/></item>
        <item id="5079" type="EtcItem" name="Golden Cobol"><set name="is_stackable" val="true"/></item>
        <item id="5082" type="EtcItem" name="Bur Cobol"><set name="is_stackable" val="true"/></item>
        <item id="5084" type="EtcItem" name="Great Cobol"><set name="is_stackable" val="true"/></item>
        <item id="10632" type="EtcItem" name="Wondrous Cubic"/>
        <item id="12764" type="EtcItem" name="Box of Fish"><set name="is_questitem" val="true"/></item>
        <item id="12765" type="EtcItem" name="Box of Seed"><set name="is_questitem" val="true"/></item>"#;

    struct Npcs {
        helvetia: Oid,
        ofulle: Oid,
        gesto: Oid,
    }

    fn setup() -> (ScriptTest, Npcs) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00040ASpecialOrder);
        let pages = Pages::of(&Q00040ASpecialOrder)
            .page("30081-01.htm", &["accept"])
            .page("30081-06.html", &["30081-07.html"])
            .page("30081-09.html", &["30081-10.html"])
            .page("31572-01.html", &["31572-02.html"])
            .page("31572-02.html", &["31572-03.html"])
            .page("30511-01.html", &["30511-02.html"])
            .page("30511-02.html", &["30511-03.html"])
            .plain(&[
                "30081-02.htm",
                "30081-03.html",
                "30081-04.html",
                "30081-05.html",
                "30081-07.html",
                "30081-08.html",
                "30081-10.html",
                "31572-03.html",
                "31572-04.html",
                "31572-05.html",
                "31572-06.html",
                "30511-03.html",
                "30511-04.html",
                "30511-05.html",
                "30511-06.html",
            ])
            .npcs(&[HELVETIA, OFULLE, GESTO]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let npcs = Npcs {
            helvetia: t.spawn(HELVETIA, 20),
            ofulle: t.spawn(OFULLE, 40),
            gesto: t.spawn(GESTO, 60),
        };
        (t, npcs)
    }

    /// Calls a script event directly (for the refusals whose page is not linked yet).
    fn direct(t: &mut ScriptTest, event: &str) -> Option<String> {
        let mut out = None;
        t.world.run_as_script(NAME, |ctx| {
            out = Q00040ASpecialOrder.on_event(ctx, event, None, Some(Oid(1)));
        });
        out
    }

    /// Sets the player's cond directly (the other face of the random accept).
    fn set_cond(t: &mut ScriptTest, cond: i32) {
        t.world.run_as_script(NAME, |ctx| {
            if let Some(qs) = ctx.get_quest_state(Oid(1), false) {
                qs.set_cond(ctx, cond, false);
            }
        });
        t.take_writes();
    }

    #[test]
    fn a_level_39_character_is_sent_away() {
        let (mut t, npcs) = setup();
        t.enter(1, 39, vec![]);
        assert!(t.quest_page(1, npcs.helvetia).contains("30081-02.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_fish_chain_then_the_cubic() {
        let (mut t, npcs) = setup();
        t.enter(
            1,
            40,
            vec![
                item(1001, ORANGE_SWIFT_FISH, 10),
                item(1002, ORANGE_UGLY_FISH, 10),
                item(1003, ORANGE_WIDE_FISH, 10),
            ],
        );
        assert!(t.quest_page(1, npcs.helvetia).contains("30081-01.htm"));
        // The accept draws the branch; under the seed 7 it is the seed chain (see the test
        // below), so the fish face of the coin is set here directly.
        assert!(t.event(1, NAME, "accept").contains("30081-04.html"));
        set_cond(&mut t, 2);
        assert!(t.quest_page(1, npcs.helvetia).contains("30081-05.html"));

        // Ofulle: cond 3, then the boxes of fish for the 30 fish.
        assert!(t.quest_page(1, npcs.ofulle).contains("31572-01.html"));
        assert!(t.event(1, NAME, "31572-02.html").contains("31572-02.html"));
        assert!(t.event(1, NAME, "31572-03.html").contains("31572-03.html"));
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "3")]);
        assert!(t.quest_page(1, npcs.ofulle).contains("31572-05.html"));
        assert_eq!(t.count(1, BOX_OF_FISH), 1);
        assert_eq!(
            (
                t.count(1, ORANGE_SWIFT_FISH),
                t.count(1, ORANGE_UGLY_FISH),
                t.count(1, ORANGE_WIDE_FISH)
            ),
            (0, 0, 0)
        );
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "4")]);

        // Helvetia takes the box: the cubic, the end.
        assert!(t.quest_page(1, npcs.helvetia).contains("30081-06.html"));
        assert!(t.event(1, NAME, "30081-07.html").contains("30081-07.html"));
        assert_eq!(t.count(1, WONDROUS_CUBIC), 1);
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed")
            ]
        );
        assert!(
            t.quest_page(1, npcs.helvetia)
                .contains("This quest has already been completed.")
        );
    }

    #[test]
    fn the_seed_chain_is_the_other_branch() {
        let (mut t, npcs) = setup();
        t.enter(
            1,
            40,
            vec![
                item(1001, GOLDEN_COBOL, 40),
                item(1002, BUR_COBOL, 40),
                item(1003, GREAT_COBOL, 40),
            ],
        );
        t.quest_page(1, npcs.helvetia);
        assert!(t.event(1, NAME, "accept").contains("30081-04.html"));
        // cond 1 to 5 skips steps: `setCond` records the skipped steps in `__compltdStateFlags`.
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "<state>", "Started"),
                rows::insert(NAME, "cond", "5"),
                rows::insert(NAME, "__compltdStateFlags", "-2147483631"),
            ]
        );
        assert!(t.quest_page(1, npcs.helvetia).contains("30081-08.html"));

        // Gesto: cond 6, then the box of seeds for the 120 cobols.
        assert!(t.quest_page(1, npcs.gesto).contains("30511-01.html"));
        assert!(t.event(1, NAME, "30511-02.html").contains("30511-02.html"));
        assert!(t.event(1, NAME, "30511-03.html").contains("30511-03.html"));
        assert_eq!(
            t.take_writes(),
            [
                rows::update(NAME, "cond", "6"),
                rows::update(NAME, "__compltdStateFlags", "-2147483599")
            ]
        );
        assert!(t.quest_page(1, npcs.gesto).contains("30511-05.html"));
        assert_eq!(t.count(1, BOX_OF_SEED), 1);
        assert_eq!(
            (
                t.count(1, GOLDEN_COBOL),
                t.count(1, BUR_COBOL),
                t.count(1, GREAT_COBOL)
            ),
            (0, 0, 0)
        );
        assert_eq!(
            t.take_writes(),
            [
                rows::update(NAME, "cond", "7"),
                rows::update(NAME, "__compltdStateFlags", "-2147483535")
            ]
        );

        // Helvetia takes the seed box: the cubic, the end.
        assert!(t.quest_page(1, npcs.helvetia).contains("30081-09.html"));
        assert!(t.event(1, NAME, "30081-10.html").contains("30081-10.html"));
        assert_eq!(t.count(1, WONDROUS_CUBIC), 1);
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed")
            ]
        );
    }

    #[test]
    fn short_of_fish_or_seeds_is_refused() {
        let (mut t, npcs) = setup();
        t.enter(
            1,
            40,
            vec![
                item(1001, ORANGE_SWIFT_FISH, 9),
                item(1002, ORANGE_UGLY_FISH, 10),
                item(1003, ORANGE_WIDE_FISH, 10),
            ],
        );
        t.quest_page(1, npcs.helvetia);
        t.event(1, NAME, "accept");
        set_cond(&mut t, 2);
        t.quest_page(1, npcs.ofulle);
        t.event(1, NAME, "31572-02.html");
        t.event(1, NAME, "31572-03.html");
        t.take_writes();

        // Ofulle with nine swift fish: the page for it, nothing taken or given.
        assert!(t.quest_page(1, npcs.ofulle).contains("31572-04.html"));
        assert_eq!(t.count(1, BOX_OF_FISH), 0);
        assert_eq!(t.count(1, ORANGE_SWIFT_FISH), 9);
        assert!(t.take_writes().is_empty());

        // The reward without the box: the event does nothing.
        assert_eq!(direct(&mut t, "30081-07.html"), None);
        assert_eq!(t.count(1, WONDROUS_CUBIC), 0);
        assert!(t.take_writes().is_empty());
    }
}
