//! Quest 383: Treasure Hunt (Espen, the pirate's chest opened with a thief key).
// Java: dist/game/data/scripts/quests/Q00383_TreasureHunt/Q00383_TreasureHunt.java
//
// Pattern: a repeatable quest whose reward is four independent rolls (armor, dye, jewel, score);
// every roll that gives nothing adds adena.

use l2_util::ObjectId;
use l2_world::script::{CREATED, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const ESPEN: i32 = 30890;
const PIRATES_CHEST: i32 = 31148;

// Items
const THIEF_KEY: i32 = 1661;
const PIRATES_TREASURE_MAP: i32 = 5915;

// Misc
const MIN_LEVEL: i32 = 42;

// Rewards, `ItemHolder(id, count)`.
const SCROLL_ENCHANT_ARMOR_C: (i32, i64) = (952, 1);
const SCROLL_ENCHANT_ARMOR_D: (i32, i64) = (956, 1);
const EMERALD: (i32, i64) = (1337, 1);
const BLUE_ONYX: (i32, i64) = (1338, 2);
const ONYX: (i32, i64) = (1339, 2);
const MITHRIL_GLOVES: (i32, i64) = (2450, 1);
const SAGES_WORN_GLOVES: (i32, i64) = (2451, 1);
const MOONSTONE: (i32, i64) = (3447, 2);
const ALEXANDRITE: (i32, i64) = (3450, 1);
const FIRE_EMERALD: (i32, i64) = (3453, 1);
const IMPERIAL_DIAMOND: (i32, i64) = (3456, 1);
const MUSICAL_SCORE_THEME_OF_LOVE: (i32, i64) = (4408, 1);
const MUSICAL_SCORE_THEME_OF_BATTLE: (i32, i64) = (4409, 1);
const MUSICAL_SCORE_THEME_OF_CELEBRATION: (i32, i64) = (4418, 1);
const MUSICAL_SCORE_THEME_OF_COMEDY: (i32, i64) = (4419, 1);
// Greater dyes: STR/CON/DEX/INT/MEN/WIT, each with two minus stats.
const DYE_S1C3_C: (i32, i64) = (4481, 1);
const DYE_S1D3_C: (i32, i64) = (4482, 1);
const DYE_C1S3_C: (i32, i64) = (4483, 1);
const DYE_C1C3_C: (i32, i64) = (4484, 1);
const DYE_D1S3_C: (i32, i64) = (4485, 1);
const DYE_D1C3_C: (i32, i64) = (4486, 1);
const DYE_I1M3_C: (i32, i64) = (4487, 1);
const DYE_I1W3_C: (i32, i64) = (4488, 1);
const DYE_M1I3_C: (i32, i64) = (4489, 1);
const DYE_M1W3_C: (i32, i64) = (4490, 1);
const DYE_W1I3_C: (i32, i64) = (4491, 1);
const DYE_W1M3_C: (i32, i64) = (4492, 1);

/// Java `rewardItems(player, ItemHolder)`.
fn reward(ctx: &mut ScriptCtx<'_>, player: ObjectId, (id, count): (i32, i64)) {
    ctx.reward_items(player, id, count);
}

/// One roll of the reward: the first row whose limit is above `random` gives its item; past the
/// last limit the roll gives `bonus` adena instead (returned).
fn roll(
    ctx: &mut ScriptCtx<'_>,
    player: ObjectId,
    random: i32,
    table: &[(i32, (i32, i64))],
    bonus: i64,
) -> i64 {
    match table.iter().find(|(limit, _)| random < *limit) {
        Some((_, item)) => {
            reward(ctx, player, *item);
            0
        }
        None => bonus,
    }
}

/// Java `Q00383_TreasureHunt`.
pub struct Q00383TreasureHunt;

impl Script for Q00383TreasureHunt {
    fn name(&self) -> &'static str {
        "Q00383_TreasureHunt"
    }

    fn path(&self) -> &'static str {
        "quests/Q00383_TreasureHunt"
    }

    fn id(&self) -> i32 {
        383
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[ESPEN]);
        s.add_talk_id(&[ESPEN, PIRATES_CHEST]);
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
            "30890-04.htm" => Some(event.to_owned()),
            "30890-05.htm" => {
                if ctx.has_quest_items(player, PIRATES_TREASURE_MAP) {
                    ctx.give_adena(player, 1000, false);
                    ctx.take_items(player, PIRATES_TREASURE_MAP, 1);
                    return Some(event.to_owned());
                }
                None
            }
            "30890-06.htm" => Some(if ctx.has_quest_items(player, PIRATES_TREASURE_MAP) {
                event.to_owned()
            } else {
                "30890-12.html".to_owned()
            }),
            "30890-07.htm" => {
                if ctx.has_quest_items(player, PIRATES_TREASURE_MAP) {
                    qs.start_quest(ctx);
                    ctx.take_items(player, PIRATES_TREASURE_MAP, 1);
                    return Some(event.to_owned());
                }
                None
            }
            "30890-08.html" | "30890-09.html" | "30890-10.html" => {
                qs.is_cond(ctx, 1).then(|| event.to_owned())
            }
            "30890-11.html" => {
                if qs.is_cond(ctx, 1) {
                    qs.set_cond(ctx, 2, true);
                    return Some(event.to_owned());
                }
                None
            }
            "31148-02.html" => {
                if !qs.is_cond(ctx, 2) {
                    return None;
                }
                if !ctx.has_quest_items(player, THIEF_KEY) {
                    return Some("31148-03.html".to_owned());
                }
                ctx.take_items(player, THIEF_KEY, 1);
                qs.exit_quest(ctx, true, true);
                let mut bonus = 0;

                let random = ctx.get_random(100);
                bonus += roll(
                    ctx,
                    player,
                    random,
                    &[
                        (5, MITHRIL_GLOVES),
                        (6, SAGES_WORN_GLOVES),
                        (18, SCROLL_ENCHANT_ARMOR_D),
                        (28, SCROLL_ENCHANT_ARMOR_C),
                    ],
                    500,
                );

                let random = ctx.get_random(1000);
                bonus += roll(
                    ctx,
                    player,
                    random,
                    &[
                        (25, DYE_S1C3_C),
                        (50, DYE_S1D3_C),
                        (75, DYE_C1S3_C),
                        (100, DYE_C1C3_C),
                        (125, DYE_D1S3_C),
                        (150, DYE_D1C3_C),
                        (175, DYE_I1M3_C),
                        (200, DYE_I1W3_C),
                        (225, DYE_M1I3_C),
                        (250, DYE_M1W3_C),
                        (275, DYE_W1I3_C),
                        (300, DYE_W1M3_C),
                    ],
                    300,
                );

                let random = ctx.get_random(100);
                bonus += roll(
                    ctx,
                    player,
                    random,
                    &[
                        (4, EMERALD),
                        (8, BLUE_ONYX),
                        (12, ONYX),
                        (16, MOONSTONE),
                        (20, ALEXANDRITE),
                        (25, FIRE_EMERALD),
                        (27, IMPERIAL_DIAMOND),
                    ],
                    500,
                );

                let random = ctx.get_random(100);
                bonus += roll(
                    ctx,
                    player,
                    random,
                    &[
                        (20, MUSICAL_SCORE_THEME_OF_LOVE),
                        (40, MUSICAL_SCORE_THEME_OF_BATTLE),
                        (60, MUSICAL_SCORE_THEME_OF_CELEBRATION),
                        (80, MUSICAL_SCORE_THEME_OF_COMEDY),
                    ],
                    500,
                );

                ctx.give_adena(player, bonus, true);
                Some(event.to_owned())
            }
            _ => None,
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        match qs.state(ctx) {
            CREATED => {
                htmltext = if ctx.player_level(player) < MIN_LEVEL {
                    "30890-01.html"
                } else if !ctx.has_quest_items(player, PIRATES_TREASURE_MAP) {
                    "30890-02.html"
                } else {
                    "30890-03.htm"
                }
                .to_owned();
            }
            STARTED => {
                if npc.id == ESPEN {
                    if qs.is_cond(ctx, 1) {
                        htmltext = "30890-13.html".to_owned();
                    } else if qs.is_cond(ctx, 2) {
                        htmltext = "30890-14.html".to_owned();
                    }
                } else if qs.is_cond(ctx, 2) {
                    htmltext = "31148-01.html".to_owned();
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

    const NAME: &str = "Q00383_TreasureHunt";

    const NPCS: &str = r#"
        <npc id="30890" type="Folk" level="70" name="Espen"/>
        <npc id="31148" type="Folk" level="70" name="Pirate's Chest"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="1661" type="EtcItem" name="Thief Key"><set name="is_questitem" val="true"/></item>
        <item id="5915" type="EtcItem" name="Pirate's Treasure Map"><set name="is_questitem" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00383TreasureHunt);
        let pages = Pages::of(&Q00383TreasureHunt)
            .page("30890-03.htm", &["30890-04.htm"])
            .page("30890-04.htm", &["30890-05.htm", "30890-06.htm"])
            .page("30890-06.htm", &["30890-07.htm"])
            .page("30890-07.htm", &["30890-08.html"])
            .page("30890-08.html", &["30890-09.html"])
            .page("30890-09.html", &["30890-10.html"])
            .page("30890-10.html", &["30890-11.html"])
            .page("31148-01.html", &["31148-02.html"])
            .plain(&[
                "30890-01.html",
                "30890-02.html",
                "30890-05.htm",
                "30890-11.html",
                "30890-12.html",
                "30890-13.html",
                "30890-14.html",
                "31148-02.html",
                "31148-03.html",
            ])
            .npcs(&[ESPEN, PIRATES_CHEST]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let espen = t.spawn(ESPEN, 20);
        let chest = t.spawn(PIRATES_CHEST, 30);
        (t, espen, chest)
    }

    #[test]
    fn the_level_and_the_map_gate_the_start() {
        let (mut t, espen, _chest) = setup();
        t.enter(1, 41, vec![item(100, PIRATES_TREASURE_MAP, 1)]);
        assert!(t.quest_page(1, espen).contains("30890-01.html"));
        t.enter(2, 42, vec![]);
        assert!(t.quest_page(2, espen).contains("30890-02.html"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_chest_gives_the_reward_for_a_thief_key() {
        let (mut t, espen, chest) = setup();
        t.enter(
            1,
            42,
            vec![item(100, PIRATES_TREASURE_MAP, 1), item(101, THIEF_KEY, 1)],
        );
        assert!(t.quest_page(1, espen).contains("30890-03.htm"));
        assert!(t.event(1, NAME, "30890-04.htm").contains("30890-04.htm"));
        assert!(t.event(1, NAME, "30890-06.htm").contains("30890-06.htm"));
        assert!(t.event(1, NAME, "30890-07.htm").contains("30890-07.htm"));
        assert_eq!(t.count(1, PIRATES_TREASURE_MAP), 0);
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        assert!(t.event(1, NAME, "30890-08.html").contains("30890-08.html"));
        assert!(t.event(1, NAME, "30890-09.html").contains("30890-09.html"));
        assert!(t.event(1, NAME, "30890-10.html").contains("30890-10.html"));
        assert!(t.event(1, NAME, "30890-11.html").contains("30890-11.html"));
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);
        assert!(t.quest_page(1, espen).contains("30890-14.html"));
        assert!(t.quest_page(1, chest).contains("31148-01.html"));
        assert!(t.event(1, NAME, "31148-02.html").contains("31148-02.html"));
        assert_eq!(t.count(1, THIEF_KEY), 0);
        // Four rolls: whatever misses pays adena, so at least 300 adena or an item came back.
        assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
    }

    #[test]
    fn the_map_can_be_sold_for_adena() {
        let (mut t, espen, _chest) = setup();
        t.enter(1, 42, vec![item(100, PIRATES_TREASURE_MAP, 1)]);
        t.quest_page(1, espen);
        t.event(1, NAME, "30890-04.htm");
        assert!(t.event(1, NAME, "30890-05.htm").contains("30890-05.htm"));
        assert_eq!((t.count(1, 57), t.count(1, PIRATES_TREASURE_MAP)), (1000, 0));
    }

    #[test]
    fn the_chest_wants_the_key() {
        let (mut t, espen, chest) = setup();
        t.enter(1, 42, vec![item(100, PIRATES_TREASURE_MAP, 1)]);
        t.quest_page(1, espen);
        t.event(1, NAME, "30890-04.htm");
        t.event(1, NAME, "30890-06.htm");
        t.event(1, NAME, "30890-07.htm");
        t.event(1, NAME, "30890-08.html");
        t.event(1, NAME, "30890-09.html");
        t.event(1, NAME, "30890-10.html");
        t.event(1, NAME, "30890-11.html");
        t.take_writes();
        assert!(t.quest_page(1, chest).contains("31148-01.html"));
        assert!(t.event(1, NAME, "31148-02.html").contains("31148-03.html"));
        assert!(t.take_writes().is_empty());
    }
}
