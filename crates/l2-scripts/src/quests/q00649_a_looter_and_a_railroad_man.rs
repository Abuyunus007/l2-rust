//! Quest 649: A Looter and a Railroad Man (Railman Obi in the Rune Town station; the bandits of
//! Gludio's and Rune's thieves' guild drop the thieves' guild marks).
// Java: dist/game/data/scripts/quests/Q00649_ALooterAndARailroadMan/Q00649_ALooterAndARailroadMan.java
//
// Pattern: a kill count with a drop chance per monster (per mille, the world RNG) up to a limit
// of 200 marks, the party range check of `ALT_PARTY_RANGE`, and a reward of adena and exit.

use l2_util::ObjectId;
use l2_world::script::{CREATED, QuestSound, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const RAILMAN_OBI: i32 = 32052;

// Item
const THIEF_GUILD_MARK: i32 = 8099;

// Misc
const MIN_LEVEL: i32 = 30;

/// Java `MONSTERS`: the drop chance in per mille of each monster.
const MONSTERS: [(i32, i32); 8] = [
    (22017, 529),  // Bandit Sweeper
    (22018, 452),  // Bandit Hound
    (22019, 606),  // Bandit Watchman
    (22021, 615),  // Bandit Undertaker
    (22022, 721),  // Bandit Assassin
    (22023, 827),  // Bandit Warrior
    (22024, 779),  // Bandit Inspector
    (22026, 1000), // Bandit Captain
];

/// Java `Q00649_ALooterAndARailroadMan`.
pub struct Q00649ALooterAndARailroadMan;

impl Script for Q00649ALooterAndARailroadMan {
    fn name(&self) -> &'static str {
        "Q00649_ALooterAndARailroadMan"
    }

    fn path(&self) -> &'static str {
        "quests/Q00649_ALooterAndARailroadMan"
    }

    fn id(&self) -> i32 {
        649
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[RAILMAN_OBI]);
        s.add_talk_id(&[RAILMAN_OBI]);
        let monsters: Vec<i32> = MONSTERS.iter().map(|(id, _)| *id).collect();
        s.add_kill_id(&monsters);
        s.register_quest_items(&[THIEF_GUILD_MARK]);
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
            "32052-03.htm" => {
                if qs.is_created(ctx) {
                    qs.start_quest(ctx);
                    htmltext = Some(event.to_owned());
                }
            }
            "32052-06.html" => {
                if qs.is_cond(ctx, 2) && ctx.has_quest_items(player, THIEF_GUILD_MARK) {
                    ctx.give_adena(player, 21698, true);
                    qs.exit_quest(ctx, true, true);
                    htmltext = Some(event.to_owned());
                }
            }
            _ => {}
        }
        htmltext
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        match qs.state(ctx) {
            CREATED => {
                htmltext = if ctx.player_level(player) >= MIN_LEVEL {
                    "32052-01.htm".to_owned()
                } else {
                    "32052-02.htm".to_owned()
                };
            }
            STARTED => {
                htmltext = if ctx.get_quest_items_count(player, THIEF_GUILD_MARK) == 200 {
                    "32052-04.html".to_owned()
                } else {
                    "32052-05.html".to_owned()
                };
            }
            _ => {}
        }
        Some(htmltext)
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        let Some(qs) = ctx.get_quest_state(killer, false) else {
            return;
        };
        // Java's `&&` order: the range and the roll only when the state is at cond 1.
        if !qs.is_cond(ctx, 1) || !ctx.check_if_in_range(ctx.alt_party_range(), npc.object_id, killer, false)
        {
            return;
        }
        let Some(chance) = MONSTERS.iter().find(|(id, _)| *id == npc.id).map(|(_, c)| *c) else {
            return;
        };
        if ctx.get_random(1000) < chance {
            ctx.give_items(killer, THIEF_GUILD_MARK, 1);
            if ctx.get_quest_items_count(killer, THIEF_GUILD_MARK) == 200 {
                qs.set_cond(ctx, 2, true);
            } else {
                ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::{Pages, ScriptTest, rows};

    const NAME: &str = "Q00649_ALooterAndARailroadMan";

    const NPCS: &str = r#"
        <npc id="32052" type="Folk" level="70" name="Railman Obi"/>
        <npc id="22026" type="Monster" level="60" name="Bandit Captain"/>
        <npc id="22017" type="Monster" level="60" name="Bandit Sweeper"/>"#;

    // The mark is stackable, as in Java: 200 marks are one stack (the 80% inventory check
    // counts slots).
    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="8099" type="EtcItem" name="Thief Guild Mark"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00649ALooterAndARailroadMan);
        let pages = Pages::of(&Q00649ALooterAndARailroadMan)
            .page("32052-01.htm", &["32052-03.htm"])
            .plain(&["32052-02.htm", "32052-03.htm", "32052-05.html", "32052-06.html"])
            .page("32052-04.html", &["32052-06.html"])
            .npcs(&[RAILMAN_OBI]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let obi = t.spawn(RAILMAN_OBI, 20);
        (t, obi)
    }

    /// Kills a bandit captain (drop chance 1000 per mille) next to the player.
    fn kill_captain(t: &mut ScriptTest) {
        let mob = t.spawn(22026, 40);
        t.kill(1, mob);
    }

    #[test]
    fn a_character_below_level_30_is_sent_away() {
        let (mut t, obi) = setup();
        t.enter(1, 29, vec![]);
        assert!(t.quest_page(1, obi).contains("32052-02.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_thieves_guild_marks_end_at_200_and_the_reward_is_adena() {
        let (mut t, obi) = setup();
        t.enter(1, 30, vec![]);
        assert!(t.quest_page(1, obi).contains("32052-01.htm"));
        assert!(t.event(1, NAME, "32052-03.htm").contains("32052-03.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        assert!(t.quest_page(1, obi).contains("32052-05.html"));

        // Each captain drops one mark and plays the item sound; the 200th moves to cond 2.
        for _ in 0..199 {
            kill_captain(&mut t);
        }
        assert_eq!(t.count(1, THIEF_GUILD_MARK), 199);
        assert!(t.take_writes().is_empty());
        assert!(t.quest_page(1, obi).contains("32052-05.html"));

        kill_captain(&mut t);
        assert_eq!(t.count(1, THIEF_GUILD_MARK), 200);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);

        assert!(t.quest_page(1, obi).contains("32052-04.html"));
        assert!(t.event(1, NAME, "32052-06.html").contains("32052-06.html"));
        assert_eq!(t.count(1, THIEF_GUILD_MARK), 0);
        assert_eq!(t.count(1, 57), 21698);
        assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
        assert!(t.world.quest_list(Oid(1)).quests.is_empty());
    }

    #[test]
    fn a_kill_before_the_quest_gives_no_mark() {
        let (mut t, _) = setup();
        t.enter(1, 30, vec![]);
        kill_captain(&mut t);
        assert_eq!(t.count(1, THIEF_GUILD_MARK), 0);
        assert!(t.take_writes().is_empty());
    }
}
