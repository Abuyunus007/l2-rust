//! Quest 267: Wrath of Verdure (Treant Bremec in the Elven Village hands out the goblin club quest).
// Java: dist/game/data/scripts/quests/Q00267_WrathOfVerdure/Q00267_WrathOfVerdure.java
//
// Pattern: a collect-and-turn-in quest with a drop chance (`docs/design/scripts.md`, "Quest
// patterns"): every Goblin Raider drops a goblin club half of the time; the NPC pays the leaves
// for the clubs (and adena at 10 clubs or more) and the quest repeats.

use l2_util::ObjectId;
use l2_world::script::{CREATED, QuestSound, STARTED, race};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const TREANT_BREMEC: i32 = 31853;

// Item
const GOBLIN_CLUB: i32 = 1335;

// Monster
const GOBLIN_RAIDER: i32 = 20325;

// Reward
const SILVERY_LEAF: i32 = 1340;

// Misc
const MIN_LEVEL: i32 = 4;

/// Java `Q00267_WrathOfVerdure`.
pub struct Q00267WrathOfVerdure;

impl Script for Q00267WrathOfVerdure {
    fn name(&self) -> &'static str {
        "Q00267_WrathOfVerdure"
    }

    fn path(&self) -> &'static str {
        "quests/Q00267_WrathOfVerdure"
    }

    fn id(&self) -> i32 {
        267
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[TREANT_BREMEC]);
        s.add_talk_id(&[TREANT_BREMEC]);
        s.add_kill_id(&[GOBLIN_RAIDER]);
        s.register_quest_items(&[GOBLIN_CLUB]);
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
            "31853-04.htm" => {
                qs.start_quest(ctx);
                htmltext = Some(event.to_owned());
            }
            "31853-07.html" => {
                qs.exit_quest(ctx, true, true);
                htmltext = Some(event.to_owned());
            }
            "31853-08.html" => htmltext = Some(event.to_owned()),
            _ => {}
        }
        htmltext
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        let qs = ctx.get_quest_state(killer, false);
        if qs.is_some() && ctx.get_random(10) < 5 {
            ctx.give_items(killer, GOBLIN_CLUB, 1);
            ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        match qs.state(ctx) {
            CREATED => {
                htmltext = if ctx.player_race(player) == race::ELF {
                    if ctx.player_level(player) >= MIN_LEVEL {
                        "31853-03.htm"
                    } else {
                        "31853-02.htm"
                    }
                } else {
                    "31853-01.htm"
                }
                .to_owned();
            }
            STARTED => {
                if ctx.has_quest_items(player, GOBLIN_CLUB) {
                    let count = ctx.get_quest_items_count(player, GOBLIN_CLUB);
                    ctx.reward_items(player, SILVERY_LEAF, count);
                    if count >= 10 {
                        ctx.give_adena(player, 600, true);
                    }
                    ctx.take_items(player, GOBLIN_CLUB, -1);
                    htmltext = "31853-06.html".to_owned();
                } else {
                    htmltext = "31853-05.html".to_owned();
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
    use l2_world::script::player_class;

    use super::*;
    use crate::testing::{Pages, ScriptTest, rows};

    const NAME: &str = "Q00267_WrathOfVerdure";

    const NPCS: &str = r#"
        <npc id="31853" type="Folk" level="70" name="Treant Bremec"/>
        <npc id="20325" type="Monster" level="3" name="Goblin Raider"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="1335" type="EtcItem" name="Goblin Club"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="1340" type="EtcItem" name="Silvery Leaf"><set name="is_stackable" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00267WrathOfVerdure);
        let pages = Pages::of(&Q00267WrathOfVerdure)
            .page("31853-03.htm", &["31853-04.htm"])
            .page("31853-05.html", &["31853-07.html", "31853-08.html"])
            .page("31853-06.html", &["31853-07.html", "31853-08.html"])
            .plain(&[
                "31853-01.htm",
                "31853-02.htm",
                "31853-04.htm",
                "31853-07.html",
                "31853-08.html",
            ])
            .npcs(&[TREANT_BREMEC]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let bremec = t.spawn(TREANT_BREMEC, 20);
        (t, bremec)
    }

    /// Kills Goblin Raiders until `clubs` clubs are in the inventory; returns the kills.
    fn hunt(t: &mut ScriptTest, clubs: i64) -> usize {
        for kills in 1..200 {
            let raider = t.spawn(GOBLIN_RAIDER, 60);
            t.kill(1, raider);
            if t.count(1, GOBLIN_CLUB) >= clubs {
                return kills;
            }
        }
        panic!("no {clubs} clubs after 200 kills");
    }

    #[test]
    fn only_elves_of_level_four_can_take_it() {
        let (mut t, bremec) = setup();
        t.enter_as(1, 10, race::HUMAN, player_class::FIGHTER, vec![]);
        assert!(t.quest_page(1, bremec).contains("31853-01.htm"));
        t.enter_as(2, 3, race::ELF, player_class::ELVEN_FIGHTER, vec![]);
        assert!(t.quest_page(2, bremec).contains("31853-02.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn clubs_for_leaves_and_adena_then_the_quest_repeats() {
        let (mut t, bremec) = setup();
        t.enter_as(1, 4, race::ELF, player_class::ELVEN_FIGHTER, vec![]);
        assert!(t.quest_page(1, bremec).contains("31853-03.htm"));
        assert!(t.event(1, NAME, "31853-04.htm").contains("31853-04.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );

        // No club yet: the NPC asks for more.
        assert!(t.quest_page(1, bremec).contains("31853-05.html"));

        // Clubs drop half of the time; the leaves are paid for each club, adena from 10 on.
        let kills = hunt(&mut t, 10);
        assert!(kills >= 10, "{kills}");
        let clubs = t.count(1, GOBLIN_CLUB);
        assert!(clubs >= 10);
        assert!(t.quest_page(1, bremec).contains("31853-06.html"));
        assert_eq!(t.count(1, GOBLIN_CLUB), 0);
        assert_eq!(t.count(1, SILVERY_LEAF), clubs);
        assert_eq!(t.count(1, 57), 600);
        assert!(t.take_writes().is_empty());

        // `exitQuest(true, true)`: the rows go, the quest can be taken again.
        assert!(t.event(1, NAME, "31853-07.html").contains("31853-07.html"));
        assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
    }
}
