//! Quest 358: Illegitimate Child of the Goddess (Grand Master Oltlin, Falibati and Trives).
// Java: dist/game/data/scripts/quests/Q00358_IllegitimateChildOfTheGoddess/Q00358_IllegitimateChildOfTheGoddess.java
//
// Pattern: a kill-count quest with a drop chance per monster up to a limit, and a random reward
// (`docs/design/scripts.md`, "Quest patterns"): `on_kill` gives the snake scales with
// `give_item_randomly` and moves the random cond-1 party member to cond 2 at the limit.

use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const OLTRAN: i32 = 30862;

// Item
const SNAKE_SCALE: i32 = 5868;

// Misc
const MIN_LEVEL: i32 = 63;
const SNAKE_SCALE_COUNT: i64 = 108;

// Rewards
const REWARDS: [i32; 8] = [
    5364, // Recipe: Sealed Dark Crystal Shield(60%)
    5366, // Recipe: Sealed Shield of Nightmare(60%)
    6329, // Recipe: Sealed Phoenix Necklace(70%)
    6331, // Recipe: Sealed Phoenix Earring(70%)
    6333, // Recipe: Sealed Phoenix Ring(70%)
    6335, // Recipe: Sealed Majestic Necklace(70%)
    6337, // Recipe: Sealed Majestic Earring(70%)
    6339, // Recipe: Sealed Majestic Ring(70%)
];

// Mobs (drop chance)
const MOBS: [(i32, f64); 2] = [
    (20672, 0.71), // trives
    (20673, 0.74), // falibati
];

/// Java `Q00358_IllegitimateChildOfTheGoddess`.
pub struct Q00358IllegitimateChildOfTheGoddess;

impl Script for Q00358IllegitimateChildOfTheGoddess {
    fn name(&self) -> &'static str {
        "Q00358_IllegitimateChildOfTheGoddess"
    }

    fn path(&self) -> &'static str {
        "quests/Q00358_IllegitimateChildOfTheGoddess"
    }

    fn id(&self) -> i32 {
        358
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[OLTRAN]);
        s.add_talk_id(&[OLTRAN]);
        let mobs: Vec<i32> = MOBS.iter().map(|(id, _)| *id).collect();
        s.add_kill_id(&mobs);
        s.register_quest_items(&[SNAKE_SCALE]);
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
            "30862-02.htm" | "30862-03.htm" => {
                htmltext = Some(event.to_owned());
            }
            "30862-04.htm" => {
                qs.start_quest(ctx);
                htmltext = Some(event.to_owned());
            }
            _ => {}
        }
        htmltext
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        let qs = ctx.random_party_member_state(killer, 1, 3, Some(npc));
        // NOTE(java-quirk): the scale goes to the killer, the cond 2 to the random party member
        // at cond 1 (`giveItemRandomly(player, ..)` next to `qs.setCond(2, true)`).
        let chance = MOBS.iter().find(|(id, _)| *id == npc.id).map_or(0.0, |(_, c)| *c);
        if let Some(qs) = qs
            && ctx.give_item_randomly(
                killer,
                Some(npc),
                SNAKE_SCALE,
                1,
                1,
                SNAKE_SCALE_COUNT,
                chance,
                true,
            )
        {
            qs.set_cond(ctx, 2, true);
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        if qs.is_created(ctx) {
            htmltext = if ctx.player_level(player) >= MIN_LEVEL {
                "30862-01.htm"
            } else {
                "30862-05.html"
            }
            .to_owned();
        } else if qs.is_started(ctx) {
            if ctx.get_quest_items_count(player, SNAKE_SCALE) < SNAKE_SCALE_COUNT {
                htmltext = "30862-06.html".to_owned();
            } else {
                if let Some(reward) = ctx.get_random_entry(&REWARDS) {
                    ctx.reward_items(player, reward, 1);
                }
                qs.exit_quest(ctx, true, true);
                htmltext = "30862-07.html".to_owned();
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

    const NAME: &str = "Q00358_IllegitimateChildOfTheGoddess";

    const NPCS: &str = r#"
        <npc id="30862" type="Folk" level="70" name="Grand Master Oltlin"/>
        <npc id="20672" type="Monster" level="60" name="Trives"/>
        <npc id="20673" type="Monster" level="60" name="Falibati"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="5868" type="EtcItem" name="Snake Scale"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="5364" type="EtcItem" name="Recipe: Sealed Dark Crystal Shield"/>
        <item id="5366" type="EtcItem" name="Recipe: Sealed Shield of Nightmare"/>
        <item id="6329" type="EtcItem" name="Recipe: Sealed Phoenix Necklace"/>
        <item id="6331" type="EtcItem" name="Recipe: Sealed Phoenix Earring"/>
        <item id="6333" type="EtcItem" name="Recipe: Sealed Phoenix Ring"/>
        <item id="6335" type="EtcItem" name="Recipe: Sealed Majestic Necklace"/>
        <item id="6337" type="EtcItem" name="Recipe: Sealed Majestic Earring"/>
        <item id="6339" type="EtcItem" name="Recipe: Sealed Majestic Ring"/>"#;

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00358IllegitimateChildOfTheGoddess);
        let pages = Pages::of(&Q00358IllegitimateChildOfTheGoddess)
            .page("30862-01.htm", &["30862-02.htm"])
            .page("30862-02.htm", &["30862-03.htm"])
            .page("30862-03.htm", &["30862-04.htm"])
            .plain(&["30862-04.htm", "30862-05.html", "30862-06.html", "30862-07.html"])
            .npcs(&[OLTRAN]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let oltran = t.spawn(OLTRAN, 20);
        (t, oltran)
    }

    /// Kills Trives and Falibati until the player has the 108 scales; returns the kills.
    fn hunt(t: &mut ScriptTest) -> usize {
        for kills in 1..400 {
            let mob = t.spawn(MOBS[kills % 2].0, 60);
            t.kill(1, mob);
            if t.count(1, SNAKE_SCALE) >= SNAKE_SCALE_COUNT {
                return kills;
            }
        }
        panic!("no 108 scales after 400 kills");
    }

    #[test]
    fn a_level_62_character_is_sent_away() {
        let (mut t, oltran) = setup();
        t.enter(1, 62, vec![]);
        assert!(t.quest_page(1, oltran).contains("30862-05.html"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_talk_then_108_scales_then_a_recipe() {
        let (mut t, oltran) = setup();
        t.enter(1, 63, vec![]);
        assert!(t.quest_page(1, oltran).contains("30862-01.htm"));
        assert!(t.event(1, NAME, "30862-02.htm").contains("30862-02.htm"));
        assert!(t.event(1, NAME, "30862-03.htm").contains("30862-03.htm"));
        assert!(t.event(1, NAME, "30862-04.htm").contains("30862-04.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        assert!(t.quest_page(1, oltran).contains("30862-06.html"));

        // Each kill gives a scale with the monster's chance; the 108th sets cond 2.
        let kills = hunt(&mut t);
        assert!(kills >= 108, "{kills}");
        assert_eq!(t.count(1, SNAKE_SCALE), SNAKE_SCALE_COUNT);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);

        // Enough scales: one recipe at random, the scales go, the quest ends repeatable.
        assert!(t.quest_page(1, oltran).contains("30862-07.html"));
        let recipes: i64 = REWARDS.iter().map(|id| t.count(1, *id)).sum();
        assert_eq!(recipes, 1);
        assert_eq!(t.count(1, SNAKE_SCALE), 0);
        assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
        assert!(t.quest_page(1, oltran).contains("30862-01.htm"));
    }

    #[test]
    fn a_kill_goes_to_a_started_party_member() {
        let (mut t, oltran) = setup();
        t.enter(1, 63, vec![]);
        t.enter(2, 63, vec![]);
        assert!(t.quest_page(2, oltran).contains("30862-01.htm"));
        t.event(2, NAME, "30862-02.htm");
        t.event(2, NAME, "30862-03.htm");
        t.event(2, NAME, "30862-04.htm");
        t.party(1, 2);
        for _ in 0..20 {
            let mob = t.spawn(MOBS[0].0, 60);
            t.kill(1, mob);
        }
        // The scale goes to the killer (player 1), the cond to the started member (player 2).
        assert!(t.count(1, SNAKE_SCALE) > 0);
        assert_eq!(t.count(2, SNAKE_SCALE), 0);
    }
}
