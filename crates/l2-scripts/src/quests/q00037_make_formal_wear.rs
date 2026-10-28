//! Quest 37: Make Formal Wear (Alexis in the Talking Island, the Gludin and Dark Elven errand
//! NPCs Leikar, Jeremy and Mist).
// Java: dist/game/data/scripts/quests/Q00037_MakeFormalWear/Q00037_MakeFormalWear.java
//
// Pattern: a delivery chain over several NPCs (`docs/design/scripts.md`, "Quest patterns"): the
// NPCs hand out and take items in turn, the sewing kit, the jewel box and the mysterious cloth
// come from other quests, and the end is one-time.

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const ALEXIS: i32 = 30842;
const LEIKAR: i32 = 31520;
const JEREMY: i32 = 31521;
const MIST: i32 = 31627;

// Items
const FORMAL_WEAR: i32 = 6408;
const MYSTERIOUS_CLOTH: i32 = 7076;
const JEWEL_BOX: i32 = 7077;
const SEWING_KIT: i32 = 7078;
const DRESS_SHOES_BOX: i32 = 7113;
const BOX_OF_COOKIES: i32 = 7159;
const ICE_WINE: i32 = 7160;
const SIGNET_RING: i32 = 7164;

// Misc
const MIN_LEVEL: i32 = 60;

/// Java `Q00037_MakeFormalWear`.
pub struct Q00037MakeFormalWear;

impl Script for Q00037MakeFormalWear {
    fn name(&self) -> &'static str {
        "Q00037_MakeFormalWear"
    }

    fn path(&self) -> &'static str {
        "quests/Q00037_MakeFormalWear"
    }

    fn id(&self) -> i32 {
        37
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[ALEXIS]);
        s.add_talk_id(&[ALEXIS, JEREMY, LEIKAR, MIST]);
        s.register_quest_items(&[SIGNET_RING, ICE_WINE, BOX_OF_COOKIES]);
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

        let mut htmltext = Some(event.to_owned());
        match event {
            "30842-03.htm" => {
                qs.start_quest(ctx);
            }
            "31520-02.html" => {
                ctx.give_items(player, SIGNET_RING, 1);
                qs.set_cond(ctx, 2, true);
            }
            "31521-02.html" => {
                ctx.give_items(player, ICE_WINE, 1);
                qs.set_cond(ctx, 3, true);
            }
            "31627-02.html" => {
                if !ctx.has_quest_items(player, ICE_WINE) {
                    return Some(ctx.get_no_quest_msg(player));
                }
                ctx.take_items(player, ICE_WINE, 1);
                qs.set_cond(ctx, 4, true);
            }
            "31521-05.html" => {
                ctx.give_items(player, BOX_OF_COOKIES, 1);
                qs.set_cond(ctx, 5, true);
            }
            "31520-05.html" => {
                if !ctx.has_quest_items(player, BOX_OF_COOKIES) {
                    return Some(ctx.get_no_quest_msg(player));
                }
                ctx.take_items(player, BOX_OF_COOKIES, 1);
                qs.set_cond(ctx, 6, true);
            }
            "31520-08.html" => {
                if !ctx.has_all_quest_items(player, &[SEWING_KIT, JEWEL_BOX, MYSTERIOUS_CLOTH]) {
                    return Some("31520-09.html".to_owned());
                }
                ctx.take_items(player, SEWING_KIT, 1);
                ctx.take_items(player, JEWEL_BOX, 1);
                ctx.take_items(player, MYSTERIOUS_CLOTH, 1);
                qs.set_cond(ctx, 7, true);
            }
            "31520-12.html" => {
                if !ctx.has_quest_items(player, DRESS_SHOES_BOX) {
                    return Some("31520-13.html".to_owned());
                }
                ctx.take_items(player, DRESS_SHOES_BOX, 1);
                ctx.give_items(player, FORMAL_WEAR, 1);
                qs.exit_quest(ctx, false, true);
            }
            _ => htmltext = None,
        }
        htmltext
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        match npc.id {
            ALEXIS => match qs.state(ctx) {
                CREATED => {
                    htmltext = if ctx.player_level(player) >= MIN_LEVEL {
                        "30842-01.htm"
                    } else {
                        "30842-02.html"
                    }
                    .to_owned();
                }
                STARTED => {
                    if qs.is_cond(ctx, 1) {
                        htmltext = "30842-04.html".to_owned();
                    }
                }
                COMPLETED => htmltext = ctx.get_already_completed_msg(player),
                _ => {}
            },
            LEIKAR => {
                if qs.is_started(ctx) {
                    match qs.get_cond(ctx) {
                        1 => htmltext = "31520-01.html".to_owned(),
                        2 => htmltext = "31520-03.html".to_owned(),
                        5 => htmltext = "31520-04.html".to_owned(),
                        6 => {
                            htmltext = if ctx
                                .has_all_quest_items(player, &[SEWING_KIT, JEWEL_BOX, MYSTERIOUS_CLOTH])
                            {
                                "31520-06.html"
                            } else {
                                "31520-07.html"
                            }
                            .to_owned();
                        }
                        7 => {
                            htmltext = if ctx.has_quest_items(player, DRESS_SHOES_BOX) {
                                "31520-10.html"
                            } else {
                                "31520-11.html"
                            }
                            .to_owned();
                        }
                        _ => {}
                    }
                }
            }
            JEREMY => {
                if qs.is_started(ctx) {
                    match qs.get_cond(ctx) {
                        2 => htmltext = "31521-01.html".to_owned(),
                        3 => htmltext = "31521-03.html".to_owned(),
                        4 => htmltext = "31521-04.html".to_owned(),
                        5 => htmltext = "31521-06.html".to_owned(),
                        _ => {}
                    }
                }
            }
            MIST => {
                if qs.is_started(ctx) {
                    match qs.get_cond(ctx) {
                        3 => htmltext = "31627-01.html".to_owned(),
                        4 => htmltext = "31627-03.html".to_owned(),
                        _ => {}
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

    const NAME: &str = "Q00037_MakeFormalWear";

    const NPCS: &str = r#"
        <npc id="30842" type="Folk" level="70" name="Alexis"/>
        <npc id="31520" type="Folk" level="70" name="Leikar"/>
        <npc id="31521" type="Folk" level="70" name="Jeremy"/>
        <npc id="31627" type="Folk" level="70" name="Mist"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="6408" type="Armor" name="Formal Wear"><set name="bodypart" val="alldress"/></item>
        <item id="7076" type="EtcItem" name="Mysterious Cloth"/>
        <item id="7077" type="EtcItem" name="Jewel Box"/>
        <item id="7078" type="EtcItem" name="Sewing Kit"/>
        <item id="7113" type="EtcItem" name="Dress Shoes Box"/>
        <item id="7159" type="EtcItem" name="Box of Cookies"><set name="is_questitem" val="true"/></item>
        <item id="7160" type="EtcItem" name="Ice Wine"><set name="is_questitem" val="true"/></item>
        <item id="7164" type="EtcItem" name="Signet Ring"><set name="is_questitem" val="true"/></item>"#;

    struct Npcs {
        alexis: Oid,
        leikar: Oid,
        jeremy: Oid,
        mist: Oid,
    }

    fn setup() -> (ScriptTest, Npcs) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00037MakeFormalWear);
        let pages = Pages::of(&Q00037MakeFormalWear)
            .page("30842-01.htm", &["30842-03.htm"])
            .page("31520-01.html", &["31520-02.html"])
            .page("31521-01.html", &["31521-02.html"])
            .page("31627-01.html", &["31627-02.html"])
            .page("31521-04.html", &["31521-05.html"])
            .page("31520-04.html", &["31520-05.html"])
            .page("31520-06.html", &["31520-08.html"])
            .page("31520-10.html", &["31520-12.html"])
            .plain(&[
                "30842-02.html",
                "30842-03.htm",
                "30842-04.html",
                "31520-02.html",
                "31520-03.html",
                "31520-05.html",
                "31520-07.html",
                "31520-08.html",
                "31520-09.html",
                "31520-11.html",
                "31520-12.html",
                "31520-13.html",
                "31521-02.html",
                "31521-03.html",
                "31521-05.html",
                "31521-06.html",
                "31627-02.html",
                "31627-03.html",
            ])
            .npcs(&[ALEXIS, LEIKAR, JEREMY, MIST]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let npcs = Npcs {
            alexis: t.spawn(ALEXIS, 20),
            leikar: t.spawn(LEIKAR, 40),
            jeremy: t.spawn(JEREMY, 60),
            mist: t.spawn(MIST, 80),
        };
        (t, npcs)
    }

    /// Calls a script event directly (for the branches whose page is not linked yet).
    fn direct(t: &mut ScriptTest, event: &str) -> Option<String> {
        let mut out = None;
        t.world.run_as_script(NAME, |ctx| {
            out = Q00037MakeFormalWear.on_event(ctx, event, None, Some(Oid(1)));
        });
        out
    }

    #[test]
    fn a_level_59_character_is_sent_away() {
        let (mut t, npcs) = setup();
        t.enter(1, 59, vec![]);
        assert!(t.quest_page(1, npcs.alexis).contains("30842-02.html"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_errands_then_the_formal_wear() {
        let (mut t, npcs) = setup();
        t.enter(
            1,
            60,
            vec![
                item(1001, SEWING_KIT, 1),
                item(1002, JEWEL_BOX, 1),
                item(1003, MYSTERIOUS_CLOTH, 1),
                item(1004, DRESS_SHOES_BOX, 1),
            ],
        );
        assert!(t.quest_page(1, npcs.alexis).contains("30842-01.htm"));
        assert!(t.event(1, NAME, "30842-03.htm").contains("30842-03.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        assert!(t.quest_page(1, npcs.alexis).contains("30842-04.html"));

        // Leikar gives the signet ring: cond 2.
        assert!(t.quest_page(1, npcs.leikar).contains("31520-01.html"));
        assert!(t.event(1, NAME, "31520-02.html").contains("31520-02.html"));
        assert_eq!(t.count(1, SIGNET_RING), 1);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);

        // Jeremy gives the ice wine: cond 3; Mist takes it: cond 4.
        assert!(t.quest_page(1, npcs.jeremy).contains("31521-01.html"));
        assert!(t.event(1, NAME, "31521-02.html").contains("31521-02.html"));
        assert_eq!(t.count(1, ICE_WINE), 1);
        assert!(t.quest_page(1, npcs.mist).contains("31627-01.html"));
        assert!(t.event(1, NAME, "31627-02.html").contains("31627-02.html"));
        assert_eq!(t.count(1, ICE_WINE), 0);
        assert_eq!(
            t.take_writes(),
            [rows::update(NAME, "cond", "3"), rows::update(NAME, "cond", "4")]
        );

        // Jeremy gives the box of cookies: cond 5; Leikar takes it: cond 6.
        assert!(t.quest_page(1, npcs.jeremy).contains("31521-04.html"));
        assert!(t.event(1, NAME, "31521-05.html").contains("31521-05.html"));
        assert_eq!(t.count(1, BOX_OF_COOKIES), 1);
        assert!(t.quest_page(1, npcs.leikar).contains("31520-04.html"));
        assert!(t.event(1, NAME, "31520-05.html").contains("31520-05.html"));
        assert_eq!(t.count(1, BOX_OF_COOKIES), 0);
        assert_eq!(
            t.take_writes(),
            [rows::update(NAME, "cond", "5"), rows::update(NAME, "cond", "6")]
        );

        // The sewing kit, the jewel box and the cloth go: cond 7.
        assert!(t.quest_page(1, npcs.leikar).contains("31520-06.html"));
        assert!(t.event(1, NAME, "31520-08.html").contains("31520-08.html"));
        assert_eq!(
            (
                t.count(1, SEWING_KIT),
                t.count(1, JEWEL_BOX),
                t.count(1, MYSTERIOUS_CLOTH)
            ),
            (0, 0, 0)
        );
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "7")]);

        // The dress shoes box for the formal wear: the end.
        assert!(t.quest_page(1, npcs.leikar).contains("31520-10.html"));
        assert!(t.event(1, NAME, "31520-12.html").contains("31520-12.html"));
        assert_eq!(t.count(1, FORMAL_WEAR), 1);
        assert_eq!(t.count(1, DRESS_SHOES_BOX), 0);
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed")
            ]
        );
        assert!(
            t.quest_page(1, npcs.alexis)
                .contains("This quest has already been completed.")
        );
    }

    #[test]
    fn missing_items_are_refused() {
        let (mut t, npcs) = setup();
        t.enter(1, 60, vec![]);
        t.quest_page(1, npcs.alexis);
        t.event(1, NAME, "30842-03.htm");
        t.take_writes();

        // No ice wine for Mist, no cookies for Leikar: the no-quest page, nothing taken.
        assert!(
            direct(&mut t, "31627-02.html")
                .unwrap()
                .contains("not on a quest")
        );
        assert!(
            direct(&mut t, "31520-05.html")
                .unwrap()
                .contains("not on a quest")
        );
        // Without the kit, jewel box and cloth, and without the dress shoes box: their pages.
        assert_eq!(direct(&mut t, "31520-08.html").as_deref(), Some("31520-09.html"));
        assert_eq!(direct(&mut t, "31520-12.html").as_deref(), Some("31520-13.html"));
        assert!(t.take_writes().is_empty());
        assert_eq!(t.count(1, FORMAL_WEAR), 0);
    }
}
