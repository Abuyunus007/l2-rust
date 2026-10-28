//! Quest 153: Deliver Goods (Arnold in Talking Island sends parcels to Jackson, Silvia and Rant).
// Java: dist/game/data/scripts/quests/Q00153_DeliverGoods/Q00153_DeliverGoods.java
//
// Pattern: a delivery chain over several NPCs (`docs/design/scripts.md`, "Quest patterns"): the
// start NPC hands out the parcels, every other talk NPC swaps its parcel for a receipt, and the
// quest moves on when all receipts are in. One-time: `exit_quest(ctx, false, ..)`.

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const JACKSON_ID: i32 = 30002;
const SILVIA_ID: i32 = 30003;
const ARNOLD_ID: i32 = 30041;
const RANT_ID: i32 = 30054;

// Items
const DELIVERY_LIST_ID: i32 = 1012;
const HEAVY_WOOD_BOX_ID: i32 = 1013;
const CLOTH_BUNDLE_ID: i32 = 1014;
const CLAY_POT_ID: i32 = 1015;
const JACKSONS_RECEIPT_ID: i32 = 1016;
const SILVIAS_RECEIPT_ID: i32 = 1017;
const RANTS_RECEIPT_ID: i32 = 1018;

// Rewards
/// You get 3 Soulshots no grade.
const SOULSHOT_NO_GRADE_ID: i32 = 1835;
const RING_OF_KNOWLEDGE_ID: i32 = 875;
const XP_REWARD_AMOUNT: i64 = 600;

/// Java `Q00153_DeliverGoods`.
pub struct Q00153DeliverGoods;

impl Script for Q00153DeliverGoods {
    fn name(&self) -> &'static str {
        "Q00153_DeliverGoods"
    }

    fn path(&self) -> &'static str {
        "quests/Q00153_DeliverGoods"
    }

    fn id(&self) -> i32 {
        153
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[ARNOLD_ID]);
        s.add_talk_id(&[JACKSON_ID, SILVIA_ID, ARNOLD_ID, RANT_ID]);
        s.register_quest_items(&[
            DELIVERY_LIST_ID,
            HEAVY_WOOD_BOX_ID,
            CLOTH_BUNDLE_ID,
            CLAY_POT_ID,
            JACKSONS_RECEIPT_ID,
            SILVIAS_RECEIPT_ID,
            RANTS_RECEIPT_ID,
        ]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        let player = player?;
        let qs = ctx.get_quest_state(player, false);
        if let Some(qs) = qs
            && npc.is_some_and(|n| n.id == ARNOLD_ID)
            && event.eq_ignore_ascii_case("30041-02.html")
        {
            qs.start_quest(ctx);
            ctx.give_items(player, DELIVERY_LIST_ID, 1);
            ctx.give_items(player, HEAVY_WOOD_BOX_ID, 1);
            ctx.give_items(player, CLOTH_BUNDLE_ID, 1);
            ctx.give_items(player, CLAY_POT_ID, 1);
        }
        // NOTE(java-quirk): every event is shown back as a page, also one this quest does not
        // know (and with no quest state).
        Some(event.to_owned())
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        let page = |name: &str| name.to_owned();
        if npc.id == ARNOLD_ID {
            match qs.state(ctx) {
                CREATED => {
                    htmltext = page(if ctx.player_level(player) >= 2 {
                        "30041-01.htm"
                    } else {
                        "30041-00.htm"
                    });
                }
                STARTED => {
                    if qs.is_cond(ctx, 1) {
                        htmltext = page("30041-03.html");
                    } else if qs.is_cond(ctx, 2) {
                        ctx.take_items(player, DELIVERY_LIST_ID, -1);
                        ctx.take_items(player, JACKSONS_RECEIPT_ID, -1);
                        ctx.take_items(player, SILVIAS_RECEIPT_ID, -1);
                        ctx.take_items(player, RANTS_RECEIPT_ID, -1);

                        // On retail it gives 2 rings but one at the time.
                        ctx.give_items(player, RING_OF_KNOWLEDGE_ID, 1);
                        ctx.give_items(player, RING_OF_KNOWLEDGE_ID, 1);
                        ctx.add_exp_and_sp(player, XP_REWARD_AMOUNT, 0);
                        // `exitQuest(false)`: one-time, no finish sound.
                        qs.exit_quest(ctx, false, false);
                        htmltext = page("30041-04.html");
                    }
                }
                COMPLETED => htmltext = ctx.get_already_completed_msg(player),
                _ => {}
            }
        } else {
            if npc.id == JACKSON_ID {
                if ctx.has_quest_items(player, HEAVY_WOOD_BOX_ID) {
                    ctx.take_items(player, HEAVY_WOOD_BOX_ID, -1);
                    ctx.give_items(player, JACKSONS_RECEIPT_ID, 1);
                    htmltext = page("30002-01.html");
                } else {
                    htmltext = page("30002-02.html");
                }
            } else if npc.id == SILVIA_ID {
                if ctx.has_quest_items(player, CLOTH_BUNDLE_ID) {
                    ctx.take_items(player, CLOTH_BUNDLE_ID, -1);
                    ctx.give_items(player, SILVIAS_RECEIPT_ID, 1);
                    ctx.give_items(player, SOULSHOT_NO_GRADE_ID, 3);
                    htmltext = page("30003-01.html");
                } else {
                    htmltext = page("30003-02.html");
                }
            } else if npc.id == RANT_ID {
                if ctx.has_quest_items(player, CLAY_POT_ID) {
                    ctx.take_items(player, CLAY_POT_ID, -1);
                    ctx.give_items(player, RANTS_RECEIPT_ID, 1);
                    htmltext = page("30054-01.html");
                } else {
                    htmltext = page("30054-02.html");
                }
            }

            if qs.is_cond(ctx, 1)
                && ctx.has_quest_items(player, JACKSONS_RECEIPT_ID)
                && ctx.has_quest_items(player, SILVIAS_RECEIPT_ID)
                && ctx.has_quest_items(player, RANTS_RECEIPT_ID)
            {
                qs.set_cond(ctx, 2, true);
            }
        }
        Some(htmltext)
    }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::{Pages, ScriptTest, names_to, rows};

    const NAME: &str = "Q00153_DeliverGoods";

    const NPCS: &str = r#"
        <npc id="30002" type="Folk" level="70" name="Jackson"/>
        <npc id="30003" type="Folk" level="70" name="Silvia"/>
        <npc id="30041" type="Folk" level="70" name="Arnold"/>
        <npc id="30054" type="Folk" level="70" name="Rant"/>"#;

    const ITEMS: &str = r#"
        <item id="1012" type="EtcItem" name="Delivery List"><set name="is_questitem" val="true"/></item>
        <item id="1013" type="EtcItem" name="Heavy Wood Box"><set name="is_questitem" val="true"/></item>
        <item id="1014" type="EtcItem" name="Cloth Bundle"><set name="is_questitem" val="true"/></item>
        <item id="1015" type="EtcItem" name="Clay Pot"><set name="is_questitem" val="true"/></item>
        <item id="1016" type="EtcItem" name="Jackson's Receipt"><set name="is_questitem" val="true"/></item>
        <item id="1017" type="EtcItem" name="Silvia's Receipt"><set name="is_questitem" val="true"/></item>
        <item id="1018" type="EtcItem" name="Rant's Receipt"><set name="is_questitem" val="true"/></item>
        <item id="1835" type="EtcItem" name="Soulshot: No Grade"><set name="is_stackable" val="true"/></item>
        <item id="875" type="Armor" name="Ring of Knowledge"><set name="bodypart" val="rfinger;lfinger"/></item>"#;

    struct Npcs {
        jackson: Oid,
        silvia: Oid,
        arnold: Oid,
        rant: Oid,
    }

    fn setup() -> (ScriptTest, Npcs) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00153DeliverGoods);
        let pages = Pages::of(&Q00153DeliverGoods)
            .page("30041-01.htm", &["30041-02.html"])
            .plain(&[
                "30041-00.htm",
                "30041-02.html",
                "30041-03.html",
                "30041-04.html",
                "30002-01.html",
                "30002-02.html",
                "30003-01.html",
                "30003-02.html",
                "30054-01.html",
                "30054-02.html",
            ])
            .npcs(&[JACKSON_ID, SILVIA_ID, ARNOLD_ID, RANT_ID]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let npcs = Npcs {
            jackson: t.spawn(JACKSON_ID, 20),
            silvia: t.spawn(SILVIA_ID, 40),
            arnold: t.spawn(ARNOLD_ID, 60),
            rant: t.spawn(RANT_ID, 80),
        };
        (t, npcs)
    }

    #[test]
    fn a_level_one_character_is_sent_away() {
        let (mut t, npcs) = setup();
        t.enter(1, 1, vec![]);
        assert!(t.quest_page(1, npcs.arnold).contains("30041-00.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn three_parcels_three_receipts_two_rings() {
        let (mut t, npcs) = setup();
        t.enter(1, 2, vec![]);
        assert!(t.quest_page(1, npcs.arnold).contains("30041-01.htm"));
        assert!(t.event(1, NAME, "30041-02.html").contains("30041-02.html"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        for parcel in [DELIVERY_LIST_ID, HEAVY_WOOD_BOX_ID, CLOTH_BUNDLE_ID, CLAY_POT_ID] {
            assert_eq!(t.count(1, parcel), 1);
        }
        assert!(t.quest_page(1, npcs.arnold).contains("30041-03.html"));

        // Jackson and Silvia: parcel for receipt (Silvia adds 3 soulshots); a second talk has
        // nothing to swap.
        assert!(t.quest_page(1, npcs.jackson).contains("30002-01.html"));
        assert!(t.quest_page(1, npcs.jackson).contains("30002-02.html"));
        assert!(t.quest_page(1, npcs.silvia).contains("30003-01.html"));
        assert_eq!(t.count(1, SOULSHOT_NO_GRADE_ID), 3);
        assert!(t.take_writes().is_empty());

        // Rant's receipt is the last one: cond 2 with the middle sound.
        assert!(t.quest_page(1, npcs.rant).contains("30054-01.html"));
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);
        for receipt in [JACKSONS_RECEIPT_ID, SILVIAS_RECEIPT_ID, RANTS_RECEIPT_ID] {
            assert_eq!(t.count(1, receipt), 1);
        }

        // Arnold: the receipts and the list go, two rings, `exitQuest(false)` (no finish sound).
        t.talk(1, npcs.arnold);
        t.bypass(1, "Script");
        let out = t.take();
        assert!(!names_to(&out, 1).contains(&"PLAY_SOUND".to_owned()));
        assert_eq!(t.count(1, RING_OF_KNOWLEDGE_ID), 2);
        for item in [
            DELIVERY_LIST_ID,
            JACKSONS_RECEIPT_ID,
            SILVIAS_RECEIPT_ID,
            RANTS_RECEIPT_ID,
        ] {
            assert_eq!(t.count(1, item), 0);
        }
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed")
            ]
        );
        assert!(
            t.quest_page(1, npcs.arnold)
                .contains("This quest has already been completed.")
        );
    }
}
