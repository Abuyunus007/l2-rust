//! Quest 647: Influx of Machines (Gutenhagen in the Rune Town/Gludio area; the golems of the
//! Gludio/Rune ruins drop broken golem fragments, 500 of them buy one random recipe).
// Java: dist/game/data/scripts/quests/Q00647_InfluxOfMachines/Q00647_InfluxOfMachines.java
//
// Pattern: a party kill count with a drop chance per golem (per mille, the member at cond 1),
// cond 2 at the limit, and a reward of one random recipe (`getRandomEntry`, the world RNG).

use l2_util::ObjectId;
use l2_world::script::{CREATED, QuestSound, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const GUTENHAGEN: i32 = 32069;

// Item
const BROKEN_GOLEM_FRAGMENT: i32 = 15521;

/// Java `RECIPES`: one of them is given (60 % recipes).
const RECIPES: [i32; 10] = [
    6881, // Recipe: Forgotten Blade (60%)
    6883, // Recipe: Basalt Battlehammer (60%)
    6885, // Recipe: Imperial Staff (60%)
    6887, // Recipe: Angel Slayer (60%)
    6891, // Recipe: Dragon Hunter Axe (60%)
    6893, // Recipe: Saint Spear (60%)
    6895, // Recipe: Demon Splinter (60%)
    6897, // Recipe: Heavens Divider (60%)
    6899, // Recipe: Arcana Mace (60%)
    7580, // Recipe: Draconic Bow (60%)
];

/// Java `MOBS`: the fragment drop chance (per mille) of each golem.
const MOBS: [(i32, i32); 12] = [
    (22801, 280), // Cruel Pincer Golem
    (22802, 227), // Cruel Pincer Golem
    (22803, 286), // Cruel Pincer Golem
    (22804, 288), // Horrifying Jackhammer Golem
    (22805, 235), // Horrifying Jackhammer Golem
    (22806, 295), // Horrifying Jackhammer Golem
    (22807, 273), // Scout-type Golem No. 28
    (22808, 143), // Scout-type Golem No. 2
    (22809, 629), // Guard Golem
    (22810, 465), // Micro Scout Golem
    (22811, 849), // Great Chaos Golem
    (22812, 463), // Boom Golem
];

// Misc
const MIN_LEVEL: i32 = 70;
const FRAGMENT_COUNT: i64 = 500;

/// Java `Q00647_InfluxOfMachines`.
pub struct Q00647InfluxOfMachines;

impl Script for Q00647InfluxOfMachines {
    fn name(&self) -> &'static str {
        "Q00647_InfluxOfMachines"
    }

    fn path(&self) -> &'static str {
        "quests/Q00647_InfluxOfMachines"
    }

    fn id(&self) -> i32 {
        647
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[GUTENHAGEN]);
        s.add_talk_id(&[GUTENHAGEN]);
        let mobs: Vec<i32> = MOBS.iter().map(|(id, _)| *id).collect();
        s.add_kill_id(&mobs);
        s.register_quest_items(&[BROKEN_GOLEM_FRAGMENT]);
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
            "32069-03.htm" => {
                qs.start_quest(ctx);
                htmltext = Some(event.to_owned());
            }
            "32069-06.html" => {
                if qs.is_cond(ctx, 2)
                    && ctx.get_quest_items_count(player, BROKEN_GOLEM_FRAGMENT) >= FRAGMENT_COUNT
                {
                    if let Some(recipe) = ctx.get_random_entry(&RECIPES) {
                        ctx.give_items(player, recipe, 1);
                    }
                    qs.exit_quest(ctx, true, true);
                    htmltext = Some(event.to_owned());
                } else {
                    htmltext = Some("32069-07.html".to_owned());
                }
            }
            _ => {}
        }
        htmltext
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        // Java `getRandomPartyMember(player, 1)`: no range check here.
        let Some(member) = ctx.get_random_party_member_cond(killer, 1) else {
            return;
        };
        let Some(qs) = ctx.get_quest_state(member, false) else {
            return;
        };
        let Some(&(_, chance)) = MOBS.iter().find(|(id, _)| *id == npc.id) else {
            return;
        };
        if qs.is_cond(ctx, 1) && ctx.get_random(1000) < chance {
            ctx.give_items(member, BROKEN_GOLEM_FRAGMENT, 1);
            if ctx.get_quest_items_count(member, BROKEN_GOLEM_FRAGMENT) >= FRAGMENT_COUNT {
                qs.set_cond(ctx, 2, true);
            } else {
                ctx.play_sound(member, QuestSound::ItemsoundQuestItemget);
            }
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        let state = qs.state(ctx);
        if state == CREATED {
            htmltext = if ctx.player_level(player) >= MIN_LEVEL {
                "32069-01.htm".to_owned()
            } else {
                "32069-02.htm".to_owned()
            };
        } else if state == STARTED {
            if qs.is_cond(ctx, 1) {
                htmltext = "32069-04.html".to_owned();
            } else if qs.is_cond(ctx, 2)
                && ctx.get_quest_items_count(player, BROKEN_GOLEM_FRAGMENT) >= FRAGMENT_COUNT
            {
                htmltext = "32069-05.html".to_owned();
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
    use crate::testing::{Pages, ScriptTest, rows};

    const NAME: &str = "Q00647_InfluxOfMachines";

    const NPCS: &str = r#"
        <npc id="32069" type="Folk" level="70" name="Gutenhagen"/>
        <npc id="22811" type="Monster" level="70" name="Great Chaos Golem"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="15521" type="EtcItem" name="Broken Golem Fragment"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
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

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00647InfluxOfMachines);
        let pages = Pages::of(&Q00647InfluxOfMachines)
            .page("32069-01.htm", &["32069-03.htm"])
            .plain(&["32069-02.htm", "32069-03.htm", "32069-04.html", "32069-07.html"])
            .page("32069-05.html", &["32069-06.html"])
            .plain(&["32069-06.html"])
            .npcs(&[GUTENHAGEN]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let gutenhagen = t.spawn(GUTENHAGEN, 20);
        (t, gutenhagen)
    }

    #[test]
    fn a_character_below_level_70_is_sent_away() {
        let (mut t, gutenhagen) = setup();
        t.enter(1, 69, vec![]);
        assert!(t.quest_page(1, gutenhagen).contains("32069-02.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn five_hundred_fragments_buy_one_recipe_and_end_the_quest() {
        let (mut t, gutenhagen) = setup();
        t.enter(1, 70, vec![]);
        assert!(t.quest_page(1, gutenhagen).contains("32069-01.htm"));
        assert!(t.event(1, NAME, "32069-03.htm").contains("32069-03.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        assert!(t.quest_page(1, gutenhagen).contains("32069-04.html"));

        // 499 fragments, then the drops of a great chaos golem (849 per mille) reach 500.
        t.world.run_as_script(NAME, |ctx| {
            ctx.give_items(Oid(1), BROKEN_GOLEM_FRAGMENT, 499);
        });
        t.take();
        let mut kills = 0;
        while t.count(1, BROKEN_GOLEM_FRAGMENT) < FRAGMENT_COUNT {
            let golem = t.spawn(22811, 40);
            t.kill(1, golem);
            kills += 1;
            assert!(kills < 50, "no fragment dropped");
        }
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);

        // The page gives a random recipe, the fragments go, the quest ends.
        assert!(t.quest_page(1, gutenhagen).contains("32069-05.html"));
        assert!(t.event(1, NAME, "32069-06.html").contains("32069-06.html"));
        let recipes: i64 = RECIPES.iter().map(|r| t.count(1, *r)).sum();
        assert_eq!(recipes, 1);
        assert_eq!(t.count(1, BROKEN_GOLEM_FRAGMENT), 0);
        assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
        assert!(t.world.quest_list(Oid(1)).quests.is_empty());
    }
}
