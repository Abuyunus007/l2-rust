//! Quest 378: Grand Feast (Ranspo in Rune, wines and recipes for a feast).
// Java: dist/game/data/scripts/quests/Q00378_GrandFeast/Q00378_GrandFeast.java
//
// Pattern: a delivery quest that sums the choices in `memoStateEx1` (wine 10/20/30 + dish 1/2/3)
// and picks the reward from that sum.

use l2_util::ObjectId;
use l2_world::script::{CREATED, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const RANSPO: i32 = 30594;

// Items
const JONAS_SALAD_RECIPE: i32 = 1455;
const JONAS_SAUCE_RECIPE: i32 = 1456;
const JONAS_STEAK_RECIPE: i32 = 1457;
const THEME_OF_THE_FEAST: i32 = 4421;
const OLD_WINE_15_YEAR: i32 = 5956;
const OLD_WINE_30_YEAR: i32 = 5957;
const OLD_WINE_60_YEAR: i32 = 5958;
const RITRONS_DESSERT_RECIPE: i32 = 5959;

// Rewards
const CORAL_EARRING: i32 = 846;
const RED_CRESCENT_EARRING: i32 = 847;
const ENCHANTED_EARRING: i32 = 848;
const ENCHANTED_RING: i32 = 879;
const RING_OF_DEVOTION: i32 = 890;
const BLUE_DIAMOND_NECKLACE: i32 = 909;
const NECKLACE_OF_DEVOTION: i32 = 910;

// Misc
const MIN_LEVEL: i32 = 20;

/// Java `Q00378_GrandFeast`.
pub struct Q00378GrandFeast;

/// One choice page: take `item` and add `add` to the memo, move to cond `cond`, else show `fail`.
fn choose(
    ctx: &mut ScriptCtx<'_>,
    player: ObjectId,
    qs: l2_world::script::Qs,
    event: &str,
    (item, add, cond, fail): (i32, i32, i32, &str),
) -> Option<String> {
    let i0 = qs.get_memo_state_ex(ctx, 1);
    if ctx.has_quest_items(player, item) {
        ctx.take_items(player, item, 1);
        if add != 0 {
            qs.set_memo_state_ex(ctx, 1, i0 + add);
        }
        qs.set_cond(ctx, cond, true);
        Some(event.to_owned())
    } else {
        Some(fail.to_owned())
    }
}

impl Script for Q00378GrandFeast {
    fn name(&self) -> &'static str {
        "Q00378_GrandFeast"
    }

    fn path(&self) -> &'static str {
        "quests/Q00378_GrandFeast"
    }

    fn id(&self) -> i32 {
        378
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[RANSPO]);
        s.add_talk_id(&[RANSPO]);
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
            "30594-02.htm" => {
                qs.set_memo_state_ex(ctx, 1, 0);
                qs.start_quest(ctx);
                Some(event.to_owned())
            }
            "30594-05.html" => choose(ctx, player, qs, event, (OLD_WINE_15_YEAR, 10, 2, "30594-08.html")),
            "30594-06.html" => choose(ctx, player, qs, event, (OLD_WINE_30_YEAR, 20, 2, "30594-08.html")),
            "30594-07.html" => choose(ctx, player, qs, event, (OLD_WINE_60_YEAR, 30, 2, "30594-08.html")),
            "30594-09.html" | "30594-18.html" => Some(event.to_owned()),
            "30594-12.html" => choose(
                ctx,
                player,
                qs,
                event,
                (THEME_OF_THE_FEAST, 0, 3, "30594-08.html"),
            ),
            "30594-14.html" => choose(
                ctx,
                player,
                qs,
                event,
                (JONAS_SALAD_RECIPE, 1, 4, "30594-17.html"),
            ),
            "30594-15.html" => choose(
                ctx,
                player,
                qs,
                event,
                (JONAS_SAUCE_RECIPE, 2, 4, "30594-17.html"),
            ),
            "30594-16.html" => choose(
                ctx,
                player,
                qs,
                event,
                (JONAS_STEAK_RECIPE, 3, 4, "30594-17.html"),
            ),
            _ => None,
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        match qs.state(ctx) {
            CREATED => {
                htmltext = if ctx.player_level(player) >= MIN_LEVEL {
                    "30594-01.htm"
                } else {
                    "30594-03.html"
                }
                .to_owned();
            }
            STARTED => match qs.get_cond(ctx) {
                1 => htmltext = "30594-04.html".to_owned(),
                2 => {
                    htmltext = if ctx.has_quest_items(player, THEME_OF_THE_FEAST) {
                        "30594-11.html"
                    } else {
                        "30594-10.html"
                    }
                    .to_owned();
                }
                3 => htmltext = "30594-13.html".to_owned(),
                4 => {
                    if ctx.has_quest_items(player, RITRONS_DESSERT_RECIPE) {
                        ctx.take_items(player, RITRONS_DESSERT_RECIPE, 1);
                        // (item, quantity, adena); an unknown sum gives nothing (Java's zeros).
                        let (item, quantity, adena) = match qs.get_memo_state_ex(ctx, 1) {
                            11 => (RED_CRESCENT_EARRING, 1, 5700),
                            12 => (CORAL_EARRING, 2, 1200),
                            13 => (ENCHANTED_RING, 1, 8100),
                            21 => (CORAL_EARRING, 2, 0),
                            22 => (ENCHANTED_RING, 1, 6900),
                            23 => (NECKLACE_OF_DEVOTION, 1, 0),
                            31 => (BLUE_DIAMOND_NECKLACE, 1, 25400),
                            32 => (RING_OF_DEVOTION, 2, 8500),
                            33 => (ENCHANTED_EARRING, 1, 2200),
                            _ => (0, 0, 0),
                        };
                        ctx.give_items(player, item, quantity);
                        ctx.give_adena(player, adena, true);
                        qs.exit_quest(ctx, true, true);
                        htmltext = "30594-20.html".to_owned();
                    } else {
                        htmltext = "30594-19.html".to_owned();
                    }
                }
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

    const NAME: &str = "Q00378_GrandFeast";

    const NPCS: &str = r#"<npc id="30594" type="Folk" level="70" name="Ranspo"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="1455" type="EtcItem" name="Salad"><set name="is_questitem" val="true"/></item>
        <item id="1456" type="EtcItem" name="Sauce"><set name="is_questitem" val="true"/></item>
        <item id="1457" type="EtcItem" name="Steak"><set name="is_questitem" val="true"/></item>
        <item id="4421" type="EtcItem" name="Theme"><set name="is_questitem" val="true"/></item>
        <item id="5956" type="EtcItem" name="Wine 15"><set name="is_questitem" val="true"/></item>
        <item id="5957" type="EtcItem" name="Wine 30"><set name="is_questitem" val="true"/></item>
        <item id="5958" type="EtcItem" name="Wine 60"><set name="is_questitem" val="true"/></item>
        <item id="5959" type="EtcItem" name="Dessert"><set name="is_questitem" val="true"/></item>
        <item id="846" type="Armor" name="Coral Earring"><set name="bodypart" val="rear;lear"/></item>
        <item id="847" type="Armor" name="Red Crescent Earring"><set name="bodypart" val="rear;lear"/></item>"#;

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00378GrandFeast);
        let pages = Pages::of(&Q00378GrandFeast)
            .page("30594-01.htm", &["30594-02.htm"])
            .page(
                "30594-04.html",
                &["30594-05.html", "30594-06.html", "30594-07.html", "30594-09.html"],
            )
            .page("30594-11.html", &["30594-12.html"])
            .page(
                "30594-13.html",
                &["30594-14.html", "30594-15.html", "30594-16.html", "30594-18.html"],
            )
            .plain(&[
                "30594-02.htm",
                "30594-03.html",
                "30594-05.html",
                "30594-08.html",
                "30594-10.html",
                "30594-12.html",
                "30594-14.html",
                "30594-17.html",
                "30594-19.html",
                "30594-20.html",
            ])
            .npcs(&[RANSPO]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let ranspo = t.spawn(RANSPO, 20);
        (t, ranspo)
    }

    #[test]
    fn a_low_level_character_is_sent_away() {
        let (mut t, ranspo) = setup();
        t.enter(1, 19, vec![]);
        assert!(t.quest_page(1, ranspo).contains("30594-03.html"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn wine_theme_and_dish_lead_to_the_reward() {
        let (mut t, ranspo) = setup();
        t.enter(
            1,
            20,
            vec![
                item(100, OLD_WINE_15_YEAR, 1),
                item(101, THEME_OF_THE_FEAST, 1),
                item(102, JONAS_SALAD_RECIPE, 1),
                item(103, RITRONS_DESSERT_RECIPE, 1),
            ],
        );
        assert!(t.quest_page(1, ranspo).contains("30594-01.htm"));
        assert!(t.event(1, NAME, "30594-02.htm").contains("30594-02.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "memoStateEx1", "0"),
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        assert!(t.quest_page(1, ranspo).contains("30594-04.html"));
        assert!(t.event(1, NAME, "30594-05.html").contains("30594-05.html"));
        assert_eq!(
            t.take_writes(),
            [
                rows::update(NAME, "memoStateEx1", "10"),
                rows::update(NAME, "cond", "2")
            ]
        );
        assert!(t.quest_page(1, ranspo).contains("30594-11.html"));
        assert!(t.event(1, NAME, "30594-12.html").contains("30594-12.html"));
        assert!(t.quest_page(1, ranspo).contains("30594-13.html"));
        assert!(t.event(1, NAME, "30594-14.html").contains("30594-14.html"));
        t.take_writes();
        // 10 + 1: one Red Crescent Earring and 5700 adena.
        t.take();
        assert!(t.quest_page(1, ranspo).contains("30594-20.html"));
        assert_eq!((t.count(1, RED_CRESCENT_EARRING), t.count(1, 57)), (1, 5700));
        assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
    }

    #[test]
    fn a_missing_wine_shows_the_refusal() {
        let (mut t, ranspo) = setup();
        t.enter(1, 20, vec![]);
        t.quest_page(1, ranspo);
        t.event(1, NAME, "30594-02.htm");
        t.quest_page(1, ranspo);
        t.take_writes();
        assert!(t.event(1, NAME, "30594-07.html").contains("30594-08.html"));
        assert!(t.take_writes().is_empty());
    }
}
