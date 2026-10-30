//! Quest 617: Gather the Flames (Hilda and Vulcan in the Town of Gludin area, Rooney in the Hot
//! Springs area; the lava monsters drop torches, 1000 of them buy a random recipe, 1200 buy the
//! recipe the character picks).
// Java: dist/game/data/scripts/quests/Q00617_GatherTheFlames/Q00617_GatherTheFlames.java
//
// Pattern: a repeatable kill that drops one or two torches to a random member at cond 1 (the
// roll is per mille), and the trade of torches for a reward with a count check and a
// "no quest" answer when it is short. Two NPCs start the quest.
// NOTE(java-quirk): the events that fail their check answer the no-quest page, and an event
// with no quest state gives the no-quest page too (not null, as in most quests).

use l2_util::ObjectId;
use l2_world::script::{CREATED, QuestSound};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const HILDA: i32 = 31271;
const VULCAN: i32 = 31539;
const ROONEY: i32 = 32049;

// Item
const TORCH: i32 = 7264;

/// Java `REWARD`: the recipes Vulcan picks one from.
const REWARD: [i32; 10] = [6881, 6883, 6885, 6887, 6891, 6893, 6895, 6897, 6899, 7580];

/// Java `MOBS`: the double-torch chance (per mille) of each lava monster.
const MOBS: [(i32, i32); 16] = [
    (22634, 639),
    (22635, 611),
    (22636, 649),
    (22637, 639),
    (22638, 639),
    (22639, 645),
    (22640, 559),
    (22641, 588),
    (22642, 537),
    (22643, 618),
    (22644, 633),
    (22645, 550),
    (22646, 593),
    (22647, 688),
    (22648, 632),
    (22649, 685),
];

/// Java `Q00617_GatherTheFlames`.
pub struct Q00617GatherTheFlames;

impl Script for Q00617GatherTheFlames {
    fn name(&self) -> &'static str {
        "Q00617_GatherTheFlames"
    }

    fn path(&self) -> &'static str {
        "quests/Q00617_GatherTheFlames"
    }

    fn id(&self) -> i32 {
        617
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[HILDA, VULCAN]);
        s.add_talk_id(&[ROONEY, HILDA, VULCAN]);
        let mobs: Vec<i32> = MOBS.iter().map(|(id, _)| *id).collect();
        s.add_kill_id(&mobs);
        s.register_quest_items(&[TORCH]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        _npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        let player = player?;
        let Some(qs) = ctx.get_quest_state(player, false) else {
            return Some(ctx.get_no_quest_msg(player));
        };
        let mut htmltext = Some(event.to_owned());
        match event {
            "31539-03.htm" | "31271-03.htm" => qs.start_quest(ctx),
            "32049-02.html" | "31539-04.html" | "31539-06.html" => {}
            "31539-07.html" => {
                if ctx.get_quest_items_count(player, TORCH) < 1000 || !qs.is_started(ctx) {
                    return Some(ctx.get_no_quest_msg(player));
                }
                if let Some(reward) = ctx.get_random_entry(&REWARD) {
                    ctx.give_items(player, reward, 1);
                }
                ctx.take_items(player, TORCH, 1000);
            }
            "31539-08.html" => qs.exit_quest(ctx, true, true),
            "6883" | "6885" | "7580" | "6891" | "6893" | "6895" | "6897" | "6899" => {
                if ctx.get_quest_items_count(player, TORCH) < 1200 || !qs.is_started(ctx) {
                    return Some(ctx.get_no_quest_msg(player));
                }
                if let Ok(item) = event.parse::<i32>() {
                    ctx.give_items(player, item, 1);
                }
                ctx.take_items(player, TORCH, 1200);
                htmltext = Some("32049-04.html".to_owned());
            }
            "6887" | "6881" => {
                if ctx.get_quest_items_count(player, TORCH) < 1200 || !qs.is_started(ctx) {
                    return Some(ctx.get_no_quest_msg(player));
                }
                if let Ok(item) = event.parse::<i32>() {
                    ctx.give_items(player, item, 1);
                }
                ctx.take_items(player, TORCH, 1200);
                htmltext = Some("32049-03.html".to_owned());
            }
            _ => htmltext = None,
        }
        htmltext
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        // Java `getRandomPartyMember(player, 1)`: no range check.
        let Some(member) = ctx.get_random_party_member_cond(killer, 1) else {
            return;
        };
        let Some(&(_, chance)) = MOBS.iter().find(|(id, _)| *id == npc.id) else {
            return;
        };
        let amount = if ctx.get_random(1000) < chance { 2 } else { 1 };
        ctx.give_items(member, TORCH, amount);
        ctx.play_sound(member, QuestSound::ItemsoundQuestItemget);
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        match npc.id {
            ROONEY => {
                if qs.is_started(ctx) {
                    htmltext = if ctx.get_quest_items_count(player, TORCH) >= 1200 {
                        "32049-02.html".to_owned()
                    } else {
                        "32049-01.html".to_owned()
                    };
                }
            }
            VULCAN => {
                htmltext = if qs.state(ctx) == CREATED {
                    if ctx.player_level(player) >= 74 {
                        "31539-01.htm".to_owned()
                    } else {
                        "31539-02.htm".to_owned()
                    }
                } else if ctx.get_quest_items_count(player, TORCH) >= 1000 {
                    "31539-04.html".to_owned()
                } else {
                    "31539-05.html".to_owned()
                };
            }
            HILDA => {
                htmltext = if qs.state(ctx) == CREATED {
                    if ctx.player_level(player) >= 74 {
                        "31271-01.htm".to_owned()
                    } else {
                        "31271-02.htm".to_owned()
                    }
                } else {
                    "31271-04.html".to_owned()
                };
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

    const NAME: &str = "Q00617_GatherTheFlames";

    const NPCS: &str = r#"
        <npc id="31271" type="Folk" level="70" name="Hilda"/>
        <npc id="31539" type="Folk" level="70" name="Vulcan"/>
        <npc id="32049" type="Folk" level="70" name="Rooney"/>
        <npc id="22634" type="Monster" level="70" name="Lava Basilisk"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="7264" type="EtcItem" name="Torch"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="6881" type="EtcItem" name="Recipe 6881"/>
        <item id="6883" type="EtcItem" name="Recipe 6883"/>
        <item id="6885" type="EtcItem" name="Recipe 6885"/>
        <item id="6887" type="EtcItem" name="Recipe 6887"/>
        <item id="6891" type="EtcItem" name="Recipe 6891"/>
        <item id="6893" type="EtcItem" name="Recipe 6893"/>
        <item id="6895" type="EtcItem" name="Recipe 6895"/>
        <item id="6897" type="EtcItem" name="Recipe 6897"/>
        <item id="6899" type="EtcItem" name="Recipe 6899"/>
        <item id="7580" type="EtcItem" name="Recipe 7580"/>"#;

    fn setup() -> (ScriptTest, Oid, Oid, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00617GatherTheFlames);
        let pages = Pages::of(&Q00617GatherTheFlames)
            .page("31271-01.htm", &["31271-03.htm"])
            .plain(&["31271-02.htm", "31271-03.htm", "31271-04.html"])
            .page("31539-01.htm", &["31539-03.htm"])
            .plain(&["31539-02.htm", "31539-03.htm", "31539-06.html"])
            .page("31539-04.html", &["31539-06.html", "31539-07.html"])
            .plain(&["31539-07.html"])
            .page("31539-05.html", &["31539-06.html", "31539-08.html"])
            .plain(&["31539-08.html"])
            .page(
                "32049-02.html",
                &[
                    "6881", "6883", "6885", "6887", "6891", "6893", "6895", "6897", "6899", "7580",
                ],
            )
            .plain(&["32049-01.html", "32049-03.html", "32049-04.html"])
            .npcs(&[HILDA, VULCAN, ROONEY]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let hilda = t.spawn(HILDA, 20);
        let vulcan = t.spawn(VULCAN, 22);
        let rooney = t.spawn(ROONEY, 24);
        (t, hilda, vulcan, rooney)
    }

    #[test]
    fn the_level_74_gate_is_at_both_starters() {
        let (mut t, hilda, vulcan, _) = setup();
        t.enter(1, 73, vec![]);
        assert!(t.quest_page(1, hilda).contains("31271-02.htm"));
        assert!(t.quest_page(1, vulcan).contains("31539-02.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn vulcan_without_torches_offers_the_exit() {
        let (mut t, hilda, vulcan, _) = setup();
        t.enter(1, 74, vec![]);
        assert!(t.quest_page(1, hilda).contains("31271-01.htm"));
        assert!(t.event(1, NAME, "31271-03.htm").contains("31271-03.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );

        // Vulcan without 1000 torches: the "too few" page; the exit is the page's link.
        assert!(t.quest_page(1, vulcan).contains("31539-05.html"));
        assert!(t.event(1, NAME, "31539-08.html").contains("31539-08.html"));
        assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
        assert!(t.world.quest_list(Oid(1)).quests.is_empty());
    }

    #[test]
    fn rooney_trades_1200_torches_for_the_recipe_the_character_picks() {
        let (mut t, hilda, _, rooney) = setup();
        t.enter(1, 74, vec![]);
        t.quest_page(1, hilda);
        t.event(1, NAME, "31271-03.htm");
        t.take_writes();

        // 1199 torches by the script, then a kill: a torch (or two) more; 1200 is enough.
        t.world.run_as_script(NAME, |ctx| {
            ctx.give_items(Oid(1), TORCH, 1199);
        });
        t.take();
        let mob = t.spawn(22634, 40);
        t.kill(1, mob);
        let torches = t.count(1, TORCH);
        assert!(torches >= 1200);

        assert!(t.quest_page(1, rooney).contains("32049-02.html"));
        assert!(t.event(1, NAME, "6881").contains("32049-03.html"));
        assert_eq!(t.count(1, 6881), 1);
        assert_eq!(t.count(1, TORCH), torches - 1200);
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn vulcan_gives_a_random_recipe_for_1000_torches() {
        let (mut t, hilda, vulcan, _) = setup();
        t.enter(1, 74, vec![]);
        t.quest_page(1, hilda);
        t.event(1, NAME, "31271-03.htm");
        t.take_writes();
        t.world.run_as_script(NAME, |ctx| {
            ctx.give_items(Oid(1), TORCH, 1000);
        });
        t.take();

        assert!(t.quest_page(1, vulcan).contains("31539-04.html"));
        assert!(t.event(1, NAME, "31539-07.html").contains("31539-07.html"));
        assert_eq!(t.count(1, TORCH), 0);
        let recipes: i64 = REWARD.iter().map(|r| t.count(1, *r)).sum();
        assert_eq!(recipes, 1);
        assert!(t.take_writes().is_empty());
    }
}
