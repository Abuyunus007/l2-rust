//! Quest 329: Curiosity of a Dwarf (Trader Rolento in the Rune town, the golems).
// Java: dist/game/data/scripts/quests/Q00329_CuriosityOfADwarf/Q00329_CuriosityOfADwarf.java
//
// Pattern: a kill-count quest with drop chances per monster (`docs/design/scripts.md`, "Quest
// patterns"). One roll per kill (`getRandom(100)`) is compared with each drop of the monster in
// order; the first one that the roll is below gives its item (chance 1.0, no limit). The NPC pays
// for everything the player brings, with a bonus at ten stones.

use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const TRADER_ROLENTO: i32 = 30437;

// Items
const GOLEM_HEARTSTONE: i32 = 1346;
const BROKEN_HEARTSTONE: i32 = 1365;

// Misc
const MIN_LEVEL: i32 = 33;

// Monsters: (id, [(item, the roll must be below this)]) in Java's order
const GRANITIC_GOLEM: i32 = 20083;
const PUNCHER: i32 = 20085;
const MONSTER_DROPS: [(i32, [(i32, i32); 2]); 2] = [
    (GRANITIC_GOLEM, [(GOLEM_HEARTSTONE, 3), (BROKEN_HEARTSTONE, 54)]),
    (PUNCHER, [(GOLEM_HEARTSTONE, 3), (BROKEN_HEARTSTONE, 58)]),
];

/// Java `Q00329_CuriosityOfADwarf`.
pub struct Q00329CuriosityOfADwarf;

impl Script for Q00329CuriosityOfADwarf {
    fn name(&self) -> &'static str {
        "Q00329_CuriosityOfADwarf"
    }

    fn path(&self) -> &'static str {
        "quests/Q00329_CuriosityOfADwarf"
    }

    fn id(&self) -> i32 {
        329
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[TRADER_ROLENTO]);
        s.add_talk_id(&[TRADER_ROLENTO]);
        let monsters: Vec<i32> = MONSTER_DROPS.iter().map(|(id, _)| *id).collect();
        s.add_kill_id(&monsters);
        s.register_quest_items(&[GOLEM_HEARTSTONE, BROKEN_HEARTSTONE]);
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
            "30437-03.htm" => {
                if qs.is_created(ctx) {
                    qs.start_quest(ctx);
                    htmltext = Some(event.to_owned());
                }
            }
            "30437-06.html" => {
                qs.exit_quest(ctx, true, true);
                htmltext = Some(event.to_owned());
            }
            "30437-07.html" => {
                htmltext = Some(event.to_owned());
            }
            _ => {}
        }
        htmltext
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        let qs = ctx.get_quest_state(killer, false);
        if qs.is_some() && ctx.check_if_in_range(ctx.alt_party_range(), npc.object_id, killer, true) {
            let rnd = ctx.get_random(100);
            if let Some((_, drops)) = MONSTER_DROPS.iter().find(|(id, _)| *id == npc.id).copied() {
                for (item, below) in drops {
                    if rnd < below {
                        ctx.give_item_randomly(killer, Some(npc), item, 1, 1, 0, 1.0, true);
                        break;
                    }
                }
            }
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        if qs.is_created(ctx) {
            htmltext = if ctx.player_level(player) >= MIN_LEVEL {
                "30437-02.htm"
            } else {
                "30437-01.htm"
            }
            .to_owned();
        } else if qs.is_started(ctx) {
            let items = [GOLEM_HEARTSTONE, BROKEN_HEARTSTONE];
            if ctx.has_at_least_one_quest_item(player, &items) {
                let broken = ctx.get_quest_items_count(player, BROKEN_HEARTSTONE);
                let golem = ctx.get_quest_items_count(player, GOLEM_HEARTSTONE);
                let bonus = if broken + golem >= 10 { 1183 } else { 0 };
                ctx.give_adena(player, broken * 50 + golem * 1000 + bonus, true);
                ctx.take_items_many(player, -1, &items);
                htmltext = "30437-05.html".to_owned();
            } else {
                htmltext = "30437-04.html".to_owned();
            }
        }
        Some(htmltext)
    }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::ScriptRegistry;
    use l2_world::script::{player_class, race};

    use super::*;
    use crate::testing::{Pages, ScriptTest, item, names_to, pages_to, rows};

    const NAME: &str = "Q00329_CuriosityOfADwarf";

    const NPCS: &str = r#"
        <npc id="30437" type="Folk" level="1" name="Trader Rolento"/>
        <npc id="20083" type="Monster" level="34" name="Granitic Golem"/>
        <npc id="20085" type="Monster" level="34" name="Puncher"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="1346" type="EtcItem" name="Golem Heartstone"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="1365" type="EtcItem" name="Broken Heartstone"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00329CuriosityOfADwarf);
        let pages = Pages::of(&Q00329CuriosityOfADwarf)
            .page("30437-02.htm", &["30437-03.htm"])
            .plain(&["30437-01.htm", "30437-03.htm"])
            .page("30437-04.html", &["30437-06.html", "30437-07.html"])
            .page("30437-05.html", &["30437-06.html", "30437-07.html"])
            .plain(&["30437-06.html", "30437-07.html"])
            .npcs(&[TRADER_ROLENTO]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let rolento = t.spawn(TRADER_ROLENTO, 20);
        (t, rolento)
    }

    #[test]
    fn a_level_thirty_two_character_is_sent_away() {
        let (mut t, rolento) = setup();
        t.enter_as(1, 32, race::HUMAN, player_class::FIGHTER, vec![]);
        assert!(t.quest_page(1, rolento).contains("30437-01.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn a_heartstone_then_the_reward() {
        let (mut t, rolento) = setup();
        t.enter_as(1, 33, race::HUMAN, player_class::FIGHTER, vec![]);
        assert!(t.quest_page(1, rolento).contains("30437-02.htm"));
        assert!(t.event(1, NAME, "30437-03.htm").contains("30437-03.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );

        // Nothing to hand in yet.
        assert!(t.quest_page(1, rolento).contains("30437-04.html"));

        // Golems drop a broken heartstone at 54% (a golem one at 3%): the first one ends the hunt.
        for _ in 0..400 {
            let golem = t.spawn(GRANITIC_GOLEM, 60);
            t.kill(1, golem);
            if t.count(1, BROKEN_HEARTSTONE) >= 1 {
                break;
            }
        }
        assert_eq!(t.count(1, BROKEN_HEARTSTONE), 1);
        assert!(t.quest_page(1, rolento).contains("30437-05.html"));
        assert_eq!(t.count(1, 57), 50);
        assert_eq!(t.count(1, BROKEN_HEARTSTONE), 0);
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn ten_stones_get_the_bonus_and_the_end_is_repeatable() {
        let (mut t, rolento) = setup();
        t.enter_as(
            1,
            33,
            race::HUMAN,
            player_class::FIGHTER,
            vec![item(900, 1365, 9), item(901, 1346, 1)],
        );
        t.quest_page(1, rolento);
        assert!(t.event(1, NAME, "30437-03.htm").contains("30437-03.htm"));
        t.take_writes();

        // 9 broken (50 each), 1 golem (1000), the bonus of ten stones (1183).
        assert!(t.quest_page(1, rolento).contains("30437-05.html"));
        assert_eq!(t.count(1, 57), 9 * 50 + 1000 + 1183);
        assert_eq!(
            (t.count(1, BROKEN_HEARTSTONE), t.count(1, GOLEM_HEARTSTONE)),
            (0, 0)
        );
        assert!(t.take_writes().is_empty());

        // The end: the quest ends repeatable with the finish sound.
        t.bypass(1, &format!("Script {NAME} 30437-06.html"));
        let out = t.take();
        assert!(pages_to(&out, 1).pop().unwrap().contains("30437-06.html"));
        assert!(names_to(&out, 1).contains(&"PLAY_SOUND".to_owned()));
        assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
        assert!(t.world.quest_list(Oid(1)).quests.is_empty());
    }
}
