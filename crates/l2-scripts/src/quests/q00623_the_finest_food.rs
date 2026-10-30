//! Quest 623: The Finest Food (Jeremy in the Hot Springs area; the thermal monsters drop the meat,
//! the leaf and the horn of the dish; 100 of each give one of the four rewards).
// Java: dist/game/data/scripts/quests/Q00623_TheFinestFood/Q00623_TheFinestFood.java
//
// Pattern: a kill that gives one item per kill up to a limit of 100 (`giveItemRandomly` with
// its limit), cond 2 once all three items are at 100, and a random reward (one roll of 1000
// over four bands) that ends the quest.

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const JEREMY: i32 = 31521;

// Monsters
const THERMAL_BUFFALO: i32 = 21315;
const THERMAL_FLAVA: i32 = 21316;
const THERMAL_ANTELOPE: i32 = 21318;

// Items: the ingredient and the count each needs.
const LEAF_OF_FLAVA: i32 = 7199;
const BUFFALO_MEAT: i32 = 7200;
const HORN_OF_ANTELOPE: i32 = 7201;
const INGREDIENT_COUNT: i64 = 100;

// Rewards
const RING_OF_AURAKYRA: i32 = 6849;
const SEALED_SANDDRAGONS_EARING: i32 = 6847;
const DRAGON_NECKLACE: i32 = 6851;

// Misc
const MIN_LEVEL: i32 = 71;

/// Java `MONSTER_DROPS`: the ingredient of each thermal monster.
const MONSTER_DROPS: [(i32, i32); 3] = [
    (THERMAL_BUFFALO, BUFFALO_MEAT),
    (THERMAL_FLAVA, LEAF_OF_FLAVA),
    (THERMAL_ANTELOPE, HORN_OF_ANTELOPE),
];

/// Java `hasAllItems(player, true, ...)`: each ingredient at its count.
fn has_all_ingredients(ctx: &ScriptCtx<'_>, player: ObjectId) -> bool {
    [LEAF_OF_FLAVA, BUFFALO_MEAT, HORN_OF_ANTELOPE]
        .iter()
        .all(|item| ctx.get_quest_items_count(player, *item) >= INGREDIENT_COUNT)
}

/// Java `Q00623_TheFinestFood`.
pub struct Q00623TheFinestFood;

impl Script for Q00623TheFinestFood {
    fn name(&self) -> &'static str {
        "Q00623_TheFinestFood"
    }

    fn path(&self) -> &'static str {
        "quests/Q00623_TheFinestFood"
    }

    fn id(&self) -> i32 {
        623
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[JEREMY]);
        s.add_talk_id(&[JEREMY]);
        s.add_kill_id(&[THERMAL_BUFFALO, THERMAL_FLAVA, THERMAL_ANTELOPE]);
        s.register_quest_items(&[LEAF_OF_FLAVA, BUFFALO_MEAT, HORN_OF_ANTELOPE]);
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
            "31521-03.htm" => {
                if qs.is_created(ctx) {
                    qs.start_quest(ctx);
                    htmltext = Some(event.to_owned());
                }
            }
            "31521-06.html" => {
                if qs.is_cond(ctx, 2) {
                    if has_all_ingredients(ctx, player) {
                        let random = ctx.get_random(1000);
                        if random < 120 {
                            ctx.give_adena(player, 25_000, true);
                            ctx.reward_items(player, RING_OF_AURAKYRA, 1);
                        } else if random < 240 {
                            ctx.give_adena(player, 65_000, true);
                            ctx.reward_items(player, SEALED_SANDDRAGONS_EARING, 1);
                        } else if random < 340 {
                            ctx.give_adena(player, 25_000, true);
                            ctx.reward_items(player, DRAGON_NECKLACE, 1);
                        } else if random < 940 {
                            ctx.give_adena(player, 73_000, true);
                            ctx.add_exp_and_sp(player, 230_000, 18200);
                        }
                        qs.exit_quest(ctx, true, true);
                        htmltext = Some(event.to_owned());
                    } else {
                        htmltext = Some("31521-07.html".to_owned());
                    }
                }
            }
            _ => {}
        }
        htmltext
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        let state = qs.state(ctx);
        if state == CREATED {
            htmltext = if ctx.player_level(player) >= MIN_LEVEL {
                "31521-01.htm".to_owned()
            } else {
                "31521-02.htm".to_owned()
            };
        } else if state == STARTED {
            if qs.is_cond(ctx, 1) {
                htmltext = "31521-04.html".to_owned();
            } else if qs.is_cond(ctx, 2) {
                htmltext = "31521-05.html".to_owned();
            }
        } else if state == COMPLETED {
            htmltext = ctx.get_already_completed_msg(player);
        }
        Some(htmltext)
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        // Java `getRandomPartyMemberState(killer, 1, 3, npc)`.
        let Some(qs) = ctx.random_party_member_state(killer, 1, 3, Some(npc)) else {
            return;
        };
        let Some(&(_, item)) = MONSTER_DROPS.iter().find(|(id, _)| *id == npc.id) else {
            return;
        };
        let member = qs.player();
        // The limit is 100 of the ingredient; the roll is always 1 (100 %).
        if ctx.give_item_randomly(member, Some(npc), item, 1, 1, INGREDIENT_COUNT, 1.0, true)
            && has_all_ingredients(ctx, member)
        {
            // Java `setCond(2)`: no sound.
            qs.set_cond(ctx, 2, false);
        }
    }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::{Pages, ScriptTest, rows};

    const NAME: &str = "Q00623_TheFinestFood";

    const NPCS: &str = r#"
        <npc id="31521" type="Folk" level="70" name="Jeremy"/>
        <npc id="21316" type="Monster" level="70" name="Thermal Flava"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="7199" type="EtcItem" name="Leaf of Flava"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="7200" type="EtcItem" name="Buffalo Meat"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="7201" type="EtcItem" name="Horn of Antelope"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="6849" type="Armor" name="Ring of Aurakyra"><set name="bodypart" val="rfinger"/></item>
        <item id="6847" type="Armor" name="Sealed Sanddragon's Earing"><set name="bodypart" val="lfinger"/></item>
        <item id="6851" type="Armor" name="Dragon Necklace"><set name="bodypart" val="neck"/></item>"#;

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00623TheFinestFood);
        let pages = Pages::of(&Q00623TheFinestFood)
            .page("31521-01.htm", &["31521-03.htm"])
            .plain(&["31521-02.htm", "31521-03.htm", "31521-04.html", "31521-07.html"])
            .page("31521-05.html", &["31521-06.html"])
            .plain(&["31521-06.html"])
            .npcs(&[JEREMY]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let jeremy = t.spawn(JEREMY, 20);
        (t, jeremy)
    }

    #[test]
    fn a_character_below_level_71_is_sent_away() {
        let (mut t, jeremy) = setup();
        t.enter(1, 70, vec![]);
        assert!(t.quest_page(1, jeremy).contains("31521-02.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_hundredth_leaf_sets_cond_2_and_the_reward_ends_the_quest() {
        let (mut t, jeremy) = setup();
        t.enter(1, 71, vec![]);
        assert!(t.quest_page(1, jeremy).contains("31521-01.htm"));
        assert!(t.event(1, NAME, "31521-03.htm").contains("31521-03.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        assert!(t.quest_page(1, jeremy).contains("31521-04.html"));

        // Buffalo meat and horns to 100, leaves to 99; the 100th leaf is the kill.
        t.world.run_as_script(NAME, |ctx| {
            ctx.give_items(Oid(1), BUFFALO_MEAT, 100);
            ctx.give_items(Oid(1), HORN_OF_ANTELOPE, 100);
            ctx.give_items(Oid(1), LEAF_OF_FLAVA, 99);
        });
        t.take();
        let mob = t.spawn(THERMAL_FLAVA, 40);
        t.kill(1, mob);
        assert_eq!(t.count(1, LEAF_OF_FLAVA), 100);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);

        assert!(t.quest_page(1, jeremy).contains("31521-05.html"));
        assert!(t.event(1, NAME, "31521-06.html").contains("31521-06.html"));
        assert_eq!(
            (
                t.count(1, BUFFALO_MEAT),
                t.count(1, HORN_OF_ANTELOPE),
                t.count(1, LEAF_OF_FLAVA)
            ),
            (0, 0, 0)
        );
        // One of the four rewards: the adena and the exp are the only ones counted here.
        let rewards = [6849, 6847, 6851];
        let got: i64 = rewards.iter().map(|r| t.count(1, *r)).sum();
        assert!(got <= 1);
        assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
        assert!(t.world.quest_list(Oid(1)).quests.is_empty());
    }

    #[test]
    fn the_exit_page_refuses_without_all_three_ingredients() {
        let (mut t, jeremy) = setup();
        t.enter(1, 71, vec![]);
        t.quest_page(1, jeremy);
        t.event(1, NAME, "31521-03.htm");
        t.take_writes();
        t.world.run_as_script(NAME, |ctx| {
            ctx.give_items(Oid(1), BUFFALO_MEAT, 100);
            ctx.give_items(Oid(1), HORN_OF_ANTELOPE, 100);
            let qs = ctx.get_quest_state(Oid(1), false).unwrap();
            qs.set_cond(ctx, 2, false);
        });
        t.take();
        t.take_writes();
        assert!(t.quest_page(1, jeremy).contains("31521-05.html"));
        assert!(t.event(1, NAME, "31521-06.html").contains("31521-07.html"));
        assert!(t.take_writes().is_empty());
        assert_eq!(t.count(1, BUFFALO_MEAT), 100);
    }
}
