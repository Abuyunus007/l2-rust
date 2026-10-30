//! Quest 380: Bring Out the Flavor of Ingredients (Rolland in the Town of Oren, the Ritron Dessert).
// Java: dist/game/data/scripts/quests/Q00380_BringOutTheFlavorOfIngredients/Q00380_BringOutTheFlavorOfIngredients.java
//
// Pattern: a cond chain over drops (`docs/design/scripts.md`, "Quest patterns"): the three
// ingredients drop with a limit to a random started party member (`random_party_member_state`,
// state -1); when all are in, the cond moves on (1 -> 5); the NPC walks the cond 5 -> 8 and gives
// the dessert, then either ends the quest (56%) or sends the player to the recipe (cond 9).

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, QuestSound};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const ROLLAND: i32 = 30069;

// Items
const ANTIDOTE: i32 = 1831;
const RITRON_FRUIT: i32 = 5895;
const MOON_FLOWER: i32 = 5896;
const LEECH_FLUIDS: i32 = 5897;

// Monsters: (monster, item, chance, limit)
const MONSTER_CHANCES: [(i32, i32, f64, i64); 3] = [
    (20205, RITRON_FRUIT, 0.1, 4),  // Dire Wolf
    (20206, MOON_FLOWER, 0.5, 20),  // Kadif Werewolf
    (20225, LEECH_FLUIDS, 0.5, 10), // Giant Mist Leech
];

// Rewards
const RITRON_RECIPE: i32 = 5959;
const RITRON_DESSERT: i32 = 5960;

// Misc
const MIN_LEVEL: i32 = 24;

/// Java `Q00380_BringOutTheFlavorOfIngredients`.
pub struct Q00380BringOutTheFlavorOfIngredients;

impl Script for Q00380BringOutTheFlavorOfIngredients {
    fn name(&self) -> &'static str {
        "Q00380_BringOutTheFlavorOfIngredients"
    }

    fn path(&self) -> &'static str {
        "quests/Q00380_BringOutTheFlavorOfIngredients"
    }

    fn id(&self) -> i32 {
        380
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[ROLLAND]);
        s.add_talk_id(&[ROLLAND]);
        let mobs: Vec<i32> = MONSTER_CHANCES.iter().map(|(id, ..)| *id).collect();
        s.add_kill_id(&mobs);
        s.register_quest_items(&[RITRON_FRUIT, MOON_FLOWER, LEECH_FLUIDS]);
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
            "30069-03.htm" | "30069-04.htm" | "30069-06.html" => {
                htmltext = Some(event.to_owned());
            }
            "30069-05.htm" => {
                if qs.is_created(ctx) {
                    qs.start_quest(ctx);
                    htmltext = Some(event.to_owned());
                }
            }
            "30069-13.html" => {
                if qs.is_cond(ctx, 9) {
                    ctx.reward_items(player, RITRON_RECIPE, 1);
                    qs.exit_quest(ctx, true, true);
                    htmltext = Some(event.to_owned());
                }
            }
            _ => {}
        }
        htmltext
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, talker: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(talker, true)?;
        let mut htmltext = ctx.get_no_quest_msg(talker);
        let state = qs.state(ctx);
        if qs.is_created(ctx) {
            htmltext = if ctx.player_level(talker) >= MIN_LEVEL {
                "30069-02.htm"
            } else {
                "30069-01.htm"
            }
            .to_owned();
        } else if qs.is_started(ctx) {
            match qs.get_cond(ctx) {
                1..=4 => {
                    if ctx.get_quest_items_count(talker, ANTIDOTE) >= 2
                        && ctx.get_quest_items_count(talker, RITRON_FRUIT) >= 4
                        && ctx.get_quest_items_count(talker, MOON_FLOWER) >= 20
                        && ctx.get_quest_items_count(talker, LEECH_FLUIDS) >= 10
                    {
                        ctx.take_items(talker, ANTIDOTE, 2);
                        ctx.take_items_many(talker, -1, &[RITRON_FRUIT, MOON_FLOWER, LEECH_FLUIDS]);
                        qs.set_cond(ctx, 5, true);
                        htmltext = "30069-08.html".to_owned();
                    } else {
                        htmltext = "30069-07.html".to_owned();
                    }
                }
                5 => {
                    qs.set_cond(ctx, 6, true);
                    htmltext = "30069-09.html".to_owned();
                }
                6 => {
                    qs.set_cond(ctx, 7, true);
                    htmltext = "30069-10.html".to_owned();
                }
                7 => {
                    qs.set_cond(ctx, 8, true);
                    htmltext = "30069-11.html".to_owned();
                }
                8 => {
                    ctx.reward_items(talker, RITRON_DESSERT, 1);
                    if ctx.get_random(100) < 56 {
                        htmltext = "30069-15.html".to_owned();
                        qs.exit_quest(ctx, true, true);
                    } else {
                        qs.set_cond(ctx, 9, true);
                        htmltext = "30069-12.html".to_owned();
                    }
                }
                9 => {
                    htmltext = "30069-12.html".to_owned();
                }
                _ => {}
            }
        } else if state == COMPLETED {
            htmltext = ctx.get_already_completed_msg(talker);
        }
        Some(htmltext)
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        let Some(qs) = ctx.random_party_member_state(killer, -1, 3, Some(npc)) else {
            return;
        };
        if qs.get_cond(ctx) < 4 {
            let Some(&(_, item_id, chance, limit)) = MONSTER_CHANCES.iter().find(|(id, ..)| *id == npc.id)
            else {
                return;
            };
            if ctx.give_item_randomly(qs.player(), Some(npc), item_id, 1, 1, limit, chance, false) {
                if ctx.get_quest_items_count(killer, RITRON_FRUIT) >= 3
                    && ctx.get_quest_items_count(killer, MOON_FLOWER) >= 20
                    && ctx.get_quest_items_count(killer, LEECH_FLUIDS) >= 10
                {
                    let cond = qs.get_cond(ctx);
                    qs.set_cond(ctx, cond + 1, true);
                } else {
                    ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::{Pages, ScriptTest, item, rows};

    const NAME: &str = "Q00380_BringOutTheFlavorOfIngredients";

    const NPCS: &str = r#"
        <npc id="30069" type="Folk" level="70" name="Rolland"/>
        <npc id="20205" type="Monster" level="20" name="Dire Wolf"/>
        <npc id="20206" type="Monster" level="20" name="Kadif Werewolf"/>
        <npc id="20225" type="Monster" level="20" name="Giant Mist Leech"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="1831" type="EtcItem" name="Antidote"><set name="is_stackable" val="true"/></item>
        <item id="5895" type="EtcItem" name="Ritron Fruit"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="5896" type="EtcItem" name="Moon Flower"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="5897" type="EtcItem" name="Leech Fluids"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="5959" type="EtcItem" name="Ritron Recipe"/>
        <item id="5960" type="EtcItem" name="Ritron Dessert"/>"#;

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00380BringOutTheFlavorOfIngredients);
        let pages = Pages::of(&Q00380BringOutTheFlavorOfIngredients)
            .page("30069-02.htm", &["30069-03.htm"])
            .page("30069-03.htm", &["30069-04.htm"])
            .page("30069-04.htm", &["30069-05.htm"])
            .page("30069-05.htm", &["30069-06.html"])
            .plain(&["30069-01.htm", "30069-06.html"])
            .page("30069-07.html", &["30069-06.html"])
            .plain(&["30069-08.html", "30069-09.html", "30069-10.html", "30069-11.html"])
            .plain(&["30069-15.html"])
            .page("30069-12.html", &["30069-13.html"])
            .plain(&["30069-13.html", "30069-14.html"])
            .npcs(&[ROLLAND]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let rolland = t.spawn(ROLLAND, 20);
        (t, rolland)
    }

    /// Accepts the quest: 02 -> 03 -> 04 -> 05 (cond 1).
    fn accept(t: &mut ScriptTest, rolland: Oid) {
        assert!(t.quest_page(1, rolland).contains("30069-02.htm"));
        for page in ["30069-03.htm", "30069-04.htm", "30069-05.htm"] {
            assert!(t.event(1, NAME, page).contains(page));
        }
        t.take_writes();
    }

    /// The ingredients the NPC takes from a player who has collected them all.
    fn ingredients() -> Vec<l2_world::ItemData> {
        vec![
            item(100, ANTIDOTE, 2),
            item(101, RITRON_FRUIT, 4),
            item(102, MOON_FLOWER, 20),
            item(103, LEECH_FLUIDS, 10),
        ]
    }

    #[test]
    fn a_level_23_character_is_sent_away() {
        let (mut t, rolland) = setup();
        t.enter(1, 23, vec![]);
        assert!(t.quest_page(1, rolland).contains("30069-01.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_ingredients_then_the_dessert_and_the_end() {
        let (mut t, rolland) = setup();
        t.enter(1, 24, ingredients());
        accept(&mut t, rolland);

        // Everything in: the items go, cond 5. The jump from cond 1 skips steps: `setCond` keeps
        // them in `__compltdStateFlags` (bit 4 and the skip bit 31: 0x80000011).
        assert!(t.quest_page(1, rolland).contains("30069-08.html"));
        assert_eq!(t.count(1, ANTIDOTE), 0);
        assert_eq!(t.count(1, RITRON_FRUIT), 0);
        assert_eq!(
            t.take_writes(),
            [
                rows::update(NAME, "cond", "5"),
                rows::insert(NAME, "__compltdStateFlags", "-2147483631"),
            ]
        );

        // The NPC walks the cond 5 -> 8; each step adds its bit to the flags.
        assert!(t.quest_page(1, rolland).contains("30069-09.html"));
        assert!(t.quest_page(1, rolland).contains("30069-10.html"));
        assert!(t.quest_page(1, rolland).contains("30069-11.html"));
        assert_eq!(
            t.take_writes(),
            [
                rows::update(NAME, "cond", "6"),
                rows::update(NAME, "__compltdStateFlags", "-2147483599"),
                rows::update(NAME, "cond", "7"),
                rows::update(NAME, "__compltdStateFlags", "-2147483535"),
                rows::update(NAME, "cond", "8"),
                rows::update(NAME, "__compltdStateFlags", "-2147483407"),
            ]
        );

        // The dessert; 56% the quest ends, else the recipe waits at cond 9.
        let page = t.quest_page(1, rolland);
        assert_eq!(t.count(1, RITRON_DESSERT), 1);
        if page.contains("30069-15.html") {
            assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
        } else {
            assert!(page.contains("30069-12.html"), "{page}");
            assert_eq!(
                t.take_writes(),
                [
                    rows::update(NAME, "cond", "9"),
                    rows::update(NAME, "__compltdStateFlags", "-2147483151"),
                ]
            );
            assert!(t.event(1, NAME, "30069-13.html").contains("30069-13.html"));
            assert_eq!(t.count(1, RITRON_RECIPE), 1);
            assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
        }
    }

    #[test]
    fn a_kill_gives_the_ingredients_to_a_started_member() {
        let (mut t, rolland) = setup();
        t.enter(1, 24, vec![]);
        accept(&mut t, rolland);
        for _ in 0..400 {
            if t.count(1, RITRON_FRUIT) >= 1 {
                break;
            }
            let wolf = t.spawn(20205, 60);
            t.kill(1, wolf);
        }
        assert_eq!(t.count(1, RITRON_FRUIT), 1);
        // Not all ingredients yet: no cond moves.
        assert!(t.take_writes().is_empty());
    }
}
