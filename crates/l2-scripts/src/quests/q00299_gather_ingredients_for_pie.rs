//! Quest 299: Gather Ingredients for Pie (Emilly in the Town of Gludio sends the player for honey, spice and fruit).
// Java: dist/game/data/scripts/quests/Q00299_GatherIngredientsForPie/Q00299_GatherIngredientsForPie.java
//
// Pattern: a delivery chain over three NPCs with a party drop in the middle (`docs/design/scripts.md`,
// "Quest patterns"): the wasps drop honey pouches for a random party member in cond 1, the 100th
// pouch sets cond 2; Emilly takes them (cond 3), Lars gives the avellan spice (cond 4), Emilly
// takes it (cond 5), Bright gives the fruit basket (cond 6) and Emilly trades it for one reward
// picked by a roll of a thousand, ending the quest.

use l2_util::ObjectId;
use l2_world::script::{CREATED, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const LARS: i32 = 30063;
const BRIGHT: i32 = 30466;
const EMILLY: i32 = 30620;

// Items
const FRUIT_BASKET: i32 = 7136;
const AVELLAN_SPICE: i32 = 7137;
const HONEY_POUCH: i32 = 7138;

// Misc
const MIN_LEVEL: i32 = 34;

/// Java `MONSTERS_CHANCES`: the wasps and their drop chance out of 1000.
fn honey_chance(npc_id: i32) -> i32 {
    match npc_id {
        20934 => 700, // Wasp Worker
        20935 => 770, // Wasp Leader
        _ => 0,
    }
}

/// Java `REWARDS`: `(item, threshold out of 1000, count)`, the first threshold above the roll wins.
const REWARDS: [(i32, i32, i64); 5] = [
    (57, 400, 2500),  // Adena
    (1865, 550, 50),  // Varnish
    (1870, 700, 50),  // Coal
    (1869, 850, 50),  // Iron Ore
    (1871, 1000, 50), // Charcoal
];

/// Java `Q00299_GatherIngredientsForPie`.
pub struct Q00299GatherIngredientsForPie;

impl Script for Q00299GatherIngredientsForPie {
    fn name(&self) -> &'static str {
        "Q00299_GatherIngredientsForPie"
    }

    fn path(&self) -> &'static str {
        "quests/Q00299_GatherIngredientsForPie"
    }

    fn id(&self) -> i32 {
        299
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[EMILLY]);
        s.add_talk_id(&[LARS, BRIGHT, EMILLY]);
        s.add_kill_id(&[20934, 20935]);
        s.register_quest_items(&[FRUIT_BASKET, HONEY_POUCH, AVELLAN_SPICE]);
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
        let mut html = None;
        match event {
            "30063-02.html" => {
                if qs.is_cond(ctx, 3) {
                    ctx.give_items(player, AVELLAN_SPICE, 1);
                    qs.set_cond(ctx, 4, true);
                    html = Some(event.to_owned());
                }
            }
            "30466-02.html" => {
                if qs.is_cond(ctx, 5) {
                    ctx.give_items(player, FRUIT_BASKET, 1);
                    qs.set_cond(ctx, 6, true);
                    html = Some(event.to_owned());
                }
            }
            "30620-03.htm" => {
                if qs.is_created(ctx) {
                    qs.start_quest(ctx);
                    html = Some(event.to_owned());
                }
            }
            "30620-06.html" => {
                if qs.is_cond(ctx, 2) && ctx.get_quest_items_count(player, HONEY_POUCH) >= 100 {
                    ctx.take_items(player, HONEY_POUCH, -1);
                    qs.set_cond(ctx, 3, true);
                    html = Some(event.to_owned());
                } else {
                    html = Some("30620-07.html".to_owned());
                }
            }
            "30620-10.html" => {
                if qs.is_cond(ctx, 4) && ctx.has_quest_items(player, AVELLAN_SPICE) {
                    ctx.take_items(player, AVELLAN_SPICE, -1);
                    qs.set_cond(ctx, 5, true);
                    html = Some(event.to_owned());
                } else {
                    html = Some("30620-11.html".to_owned());
                }
            }
            "30620-14.html" => {
                if qs.is_cond(ctx, 6) && ctx.has_quest_items(player, FRUIT_BASKET) {
                    ctx.take_items(player, FRUIT_BASKET, -1);
                    let chance = ctx.get_random(1000);
                    for (item, threshold, count) in REWARDS {
                        if threshold > chance {
                            ctx.reward_items(player, item, count);
                            break;
                        }
                    }
                    qs.exit_quest(ctx, true, true);
                    html = Some(event.to_owned());
                } else {
                    html = Some("30620-15.html".to_owned());
                }
            }
            _ => {}
        }
        html
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        let Some(member) = ctx.random_party_member_state(killer, 1, 3, Some(npc)) else {
            return;
        };
        if ctx.get_random(1000) < honey_chance(npc.id)
            && ctx.get_quest_items_count(killer, HONEY_POUCH) < 100
            && ctx.give_item_randomly(killer, Some(npc), HONEY_POUCH, 1, 2, 100, 1.0, true)
        {
            // `setCond(2)` on the member drawn.
            member.set_cond(ctx, 2, false);
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, talker: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(talker, true)?;
        let mut html = ctx.get_no_quest_msg(talker);
        match npc.id {
            LARS => match qs.get_cond(ctx) {
                3 => html = "30063-01.html".to_owned(),
                4 => html = "30063-03.html".to_owned(),
                _ => {}
            },
            BRIGHT => match qs.get_cond(ctx) {
                5 => html = "30466-01.html".to_owned(),
                6 => html = "30466-03.html".to_owned(),
                _ => {}
            },
            EMILLY => match qs.state(ctx) {
                CREATED => {
                    html = if ctx.player_level(talker) >= MIN_LEVEL {
                        "30620-01.htm"
                    } else {
                        "30620-02.htm"
                    }
                    .to_owned();
                }
                STARTED => match qs.get_cond(ctx) {
                    1 => html = "30620-05.html".to_owned(),
                    2 => {
                        if ctx.get_quest_items_count(talker, HONEY_POUCH) >= 100 {
                            html = "30620-04.html".to_owned();
                        }
                    }
                    3 => html = "30620-08.html".to_owned(),
                    4 => {
                        if ctx.has_quest_items(talker, AVELLAN_SPICE) {
                            html = "30620-09.html".to_owned();
                        }
                    }
                    5 => html = "30620-12.html".to_owned(),
                    6 => {
                        if ctx.has_quest_items(talker, FRUIT_BASKET) {
                            html = "30620-13.html".to_owned();
                        }
                    }
                    _ => {}
                },
                _ => {}
            },
            _ => {}
        }
        Some(html)
    }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::ScriptRegistry;
    use l2_world::script::{player_class, race};

    use super::*;
    use crate::testing::{Pages, ScriptTest, rows};

    const NAME: &str = "Q00299_GatherIngredientsForPie";

    const NPCS: &str = r#"
        <npc id="30063" type="Folk" level="70" name="Lars"/>
        <npc id="30466" type="Folk" level="70" name="Bright"/>
        <npc id="30620" type="Folk" level="70" name="Emilly"/>
        <npc id="20934" type="Monster" level="40" name="Wasp Worker"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="7136" type="EtcItem" name="Fruit Basket"><set name="is_questitem" val="true"/></item>
        <item id="7137" type="EtcItem" name="Avellan Spice"><set name="is_questitem" val="true"/></item>
        <item id="7138" type="EtcItem" name="Honey Pouch"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="1865" type="EtcItem" name="Varnish"><set name="is_stackable" val="true"/></item>
        <item id="1870" type="EtcItem" name="Coal"><set name="is_stackable" val="true"/></item>
        <item id="1869" type="EtcItem" name="Iron Ore"><set name="is_stackable" val="true"/></item>
        <item id="1871" type="EtcItem" name="Charcoal"><set name="is_stackable" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid, Oid, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00299GatherIngredientsForPie);
        let pages = Pages::of(&Q00299GatherIngredientsForPie)
            .page("30620-01.htm", &["30620-03.htm"])
            .page("30620-04.html", &["30620-06.html"])
            .page("30620-09.html", &["30620-10.html"])
            .page("30620-13.html", &["30620-14.html"])
            .plain(&[
                "30620-02.htm",
                "30620-03.htm",
                "30620-05.html",
                "30620-06.html",
                "30620-07.html",
                "30620-08.html",
                "30620-10.html",
                "30620-11.html",
                "30620-12.html",
                "30620-14.html",
                "30620-15.html",
            ])
            .page("30063-01.html", &["30063-02.html"])
            .plain(&["30063-02.html", "30063-03.html"])
            .page("30466-01.html", &["30466-02.html"])
            .plain(&["30466-02.html", "30466-03.html"])
            .npcs(&[LARS, BRIGHT, EMILLY]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let lars = t.spawn(LARS, 20);
        let bright = t.spawn(BRIGHT, 40);
        let emilly = t.spawn(EMILLY, 60);
        (t, lars, bright, emilly)
    }

    #[test]
    fn a_level_thirty_three_character_is_sent_away() {
        let (mut t, _lars, _bright, emilly) = setup();
        t.enter_as(1, 33, race::HUMAN, player_class::FIGHTER, vec![]);
        assert!(t.quest_page(1, emilly).contains("30620-02.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn honey_spice_and_fruit_then_one_reward() {
        let (mut t, lars, bright, emilly) = setup();
        t.enter_as(1, 34, race::HUMAN, player_class::FIGHTER, vec![]);
        assert!(t.quest_page(1, emilly).contains("30620-01.htm"));
        assert!(t.event(1, NAME, "30620-03.htm").contains("30620-03.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        assert!(t.quest_page(1, emilly).contains("30620-05.html"));

        // Wasps drop honey pouches (1 or 2 at a time); the 100th pouch sets cond 2.
        for _ in 0..2000 {
            if t.count(1, HONEY_POUCH) >= 100 {
                break;
            }
            let wasp = t.spawn(20934, 60);
            t.kill(1, wasp);
        }
        assert_eq!(t.count(1, HONEY_POUCH), 100);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);

        // Emilly takes the pouches: cond 3.
        assert!(t.quest_page(1, emilly).contains("30620-04.html"));
        assert!(t.event(1, NAME, "30620-06.html").contains("30620-06.html"));
        assert_eq!(t.count(1, HONEY_POUCH), 0);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "3")]);

        // Lars gives the spice: cond 4; Emilly takes it: cond 5.
        assert!(t.quest_page(1, lars).contains("30063-01.html"));
        assert!(t.event(1, NAME, "30063-02.html").contains("30063-02.html"));
        assert_eq!(t.count(1, AVELLAN_SPICE), 1);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "4")]);
        assert!(t.quest_page(1, emilly).contains("30620-09.html"));
        assert!(t.event(1, NAME, "30620-10.html").contains("30620-10.html"));
        assert_eq!(t.count(1, AVELLAN_SPICE), 0);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "5")]);

        // Bright gives the basket: cond 6; Emilly trades it for one reward and ends the quest.
        assert!(t.quest_page(1, bright).contains("30466-01.html"));
        assert!(t.event(1, NAME, "30466-02.html").contains("30466-02.html"));
        assert_eq!(t.count(1, FRUIT_BASKET), 1);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "6")]);
        assert!(t.quest_page(1, emilly).contains("30620-13.html"));
        assert!(t.event(1, NAME, "30620-14.html").contains("30620-14.html"));
        assert_eq!(t.count(1, FRUIT_BASKET), 0);
        let rewards: Vec<i64> = REWARDS.iter().map(|&(item, _, _)| t.count(1, item)).collect();
        assert_eq!(rewards.iter().filter(|&&n| n > 0).count(), 1, "{rewards:?}");
        assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
    }
}
