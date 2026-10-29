//! Quest 161: Fruit of the Mother Tree (Andellia sends an Elf to Thalia for the fruit).
// Java: dist/game/data/scripts/quests/Q00161_FruitOfTheMotherTree/Q00161_FruitOfTheMotherTree.java
//
// Pattern: a delivery chain over two NPCs. The start NPC gives the letter, Thalia swaps it for
// the fruit and sets cond 2, the start NPC takes the fruit and gives adena and exp (one-time).
// Elves only (race gate).

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, STARTED, race};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const ANDELLIA: i32 = 30362;
const THALIA: i32 = 30371;

// Items
const ANDELLRIAS_LETTER: i32 = 1036;
const MOTHERTREE_FRUIT: i32 = 1037;

// Misc
const MIN_LEVEL: i32 = 3;

/// Java `Q00161_FruitOfTheMotherTree`.
pub struct Q00161FruitOfTheMotherTree;

impl Script for Q00161FruitOfTheMotherTree {
    fn name(&self) -> &'static str {
        "Q00161_FruitOfTheMotherTree"
    }

    fn path(&self) -> &'static str {
        "quests/Q00161_FruitOfTheMotherTree"
    }

    fn id(&self) -> i32 {
        161
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[ANDELLIA]);
        s.add_talk_id(&[ANDELLIA, THALIA]);
        s.register_quest_items(&[ANDELLRIAS_LETTER, MOTHERTREE_FRUIT]);
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
            "30362-04.htm" => {
                qs.start_quest(ctx);
                ctx.give_items(player, ANDELLRIAS_LETTER, 1);
                Some(event.to_owned())
            }
            "30371-03.html" => Some(event.to_owned()),
            _ => None,
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        match npc.id {
            ANDELLIA => match qs.state(ctx) {
                CREATED => {
                    htmltext = if ctx.player_race(player) == race::ELF {
                        if ctx.player_level(player) >= MIN_LEVEL {
                            "30362-03.htm".to_owned()
                        } else {
                            "30362-02.htm".to_owned()
                        }
                    } else {
                        "30362-01.htm".to_owned()
                    };
                }
                STARTED => {
                    if qs.is_cond(ctx, 1) {
                        htmltext = "30362-05.html".to_owned();
                    } else if qs.is_cond(ctx, 2) && ctx.has_quest_items(player, MOTHERTREE_FRUIT) {
                        ctx.give_adena(player, 1_000, true);
                        ctx.add_exp_and_sp(player, 1_000, 0);
                        qs.exit_quest(ctx, false, true);
                        htmltext = "30362-06.html".to_owned();
                    }
                }
                COMPLETED => htmltext = ctx.get_already_completed_msg(player),
                _ => {}
            },
            THALIA => {
                if qs.is_started(ctx) {
                    if qs.is_cond(ctx, 1) && ctx.has_quest_items(player, ANDELLRIAS_LETTER) {
                        ctx.take_items(player, ANDELLRIAS_LETTER, -1);
                        ctx.give_items(player, MOTHERTREE_FRUIT, 1);
                        qs.set_cond(ctx, 2, true);
                        htmltext = "30371-01.html".to_owned();
                    } else if qs.is_cond(ctx, 2) && ctx.has_quest_items(player, MOTHERTREE_FRUIT) {
                        htmltext = "30371-02.html".to_owned();
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
    use crate::testing::{Pages, ScriptTest, rows};

    const NAME: &str = "Q00161_FruitOfTheMotherTree";
    const ADENA: i32 = 57;

    const NPCS: &str = r#"
        <npc id="30362" type="Folk" level="1" name="Andellia"/>
        <npc id="30371" type="Folk" level="1" name="Thalia"/>"#;

    const ITEMS: &str = r#"
        <item id="1036" type="EtcItem" name="Andellia's Letter"><set name="is_questitem" val="true"/></item>
        <item id="1037" type="EtcItem" name="Mothertree Fruit"><set name="is_questitem" val="true"/></item>
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>"#;

    struct Npcs {
        andellia: Oid,
        thalia: Oid,
    }

    fn setup() -> (ScriptTest, Npcs) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00161FruitOfTheMotherTree);
        let pages = Pages::of(&Q00161FruitOfTheMotherTree)
            .page("30362-03.htm", &["30362-04.htm"])
            .page("30371-01.html", &["30371-03.html"])
            .page("30371-02.html", &["30371-03.html"])
            .plain(&[
                "30362-01.htm",
                "30362-02.htm",
                "30362-04.htm",
                "30362-05.html",
                "30362-06.html",
                "30371-03.html",
            ])
            .npcs(&[ANDELLIA, THALIA]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let npcs = Npcs {
            andellia: t.spawn(ANDELLIA, 20),
            thalia: t.spawn(THALIA, 40),
        };
        (t, npcs)
    }

    #[test]
    fn only_an_elf_of_level_3_starts_the_quest() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 3, race::HUMAN, 0, vec![]);
        assert!(t.quest_page(1, npcs.andellia).contains("30362-01.htm"));
        t.enter_as(2, 2, race::ELF, 0, vec![]);
        assert!(t.quest_page(2, npcs.andellia).contains("30362-02.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_letter_the_fruit_and_the_reward() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 3, race::ELF, 0, vec![]);
        assert!(t.quest_page(1, npcs.andellia).contains("30362-03.htm"));
        assert!(t.event(1, NAME, "30362-04.htm").contains("30362-04.htm"));
        assert_eq!(t.count(1, ANDELLRIAS_LETTER), 1);
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        assert!(t.quest_page(1, npcs.andellia).contains("30362-05.html"));

        // Thalia swaps the letter for the fruit: cond 2.
        assert!(t.quest_page(1, npcs.thalia).contains("30371-01.html"));
        assert_eq!(t.count(1, ANDELLRIAS_LETTER), 0);
        assert_eq!(t.count(1, MOTHERTREE_FRUIT), 1);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);
        // A second talk with the fruit in hand.
        assert!(t.quest_page(1, npcs.thalia).contains("30371-02.html"));
        assert!(t.take_writes().is_empty());

        // Andellia: 1000 adena and 1000 exp, `exitQuest(false, true)`.
        assert!(t.quest_page(1, npcs.andellia).contains("30362-06.html"));
        assert_eq!(t.count(1, ADENA), 1_000);
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed")
            ]
        );
    }
}
