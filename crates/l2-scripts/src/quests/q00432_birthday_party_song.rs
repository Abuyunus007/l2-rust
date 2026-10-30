//! Quest 432: Birthday Party Song (Octavia; the Golems drop the red crystals).
// Java: dist/game/data/scripts/quests/Q00432_BirthdayPartySong/Q00432_BirthdayPartySong.java
//
// Pattern: a kill-count quest with a drop chance up to a limit (the killer collects, cond 2 at
// 50) and a turn-in that gives the reward and ends the quest (repeatable).

use l2_util::ObjectId;
use l2_world::script::{CREATED, QuestSound, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const OCTAVIA: i32 = 31043;

// Monster
const GOLEM: i32 = 21103;

// Item
const RED_CRYSTAL: i32 = 7541;

// Reward
const ECHO_CRYSTAL: i32 = 7061;

// Misc
const MIN_LEVEL: i32 = 31;
const CRYSTAL_COUNT: i64 = 50;

/// Java `Q00432_BirthdayPartySong`.
pub struct Q00432BirthdayPartySong;

impl Script for Q00432BirthdayPartySong {
    fn name(&self) -> &'static str {
        "Q00432_BirthdayPartySong"
    }

    fn path(&self) -> &'static str {
        "quests/Q00432_BirthdayPartySong"
    }

    fn id(&self) -> i32 {
        432
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[OCTAVIA]);
        s.add_talk_id(&[OCTAVIA]);
        s.add_kill_id(&[GOLEM]);
        s.register_quest_items(&[RED_CRYSTAL]);
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

        let htmltext = match event {
            "31043-02.htm" => {
                qs.start_quest(ctx);
                event
            }
            "31043-05.html" => {
                if ctx.get_quest_items_count(player, RED_CRYSTAL) < CRYSTAL_COUNT {
                    return Some("31043-06.html".to_owned());
                }
                ctx.give_items(player, ECHO_CRYSTAL, 25);
                qs.exit_quest(ctx, true, true);
                event
            }
            _ => return None,
        };
        Some(htmltext.to_owned())
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        if let Some(qs) = ctx.get_quest_state(killer, false)
            && qs.is_cond(ctx, 1)
            && ctx.get_random_boolean()
        {
            ctx.give_items(killer, RED_CRYSTAL, 1);
            if ctx.get_quest_items_count(killer, RED_CRYSTAL) == CRYSTAL_COUNT {
                qs.set_cond(ctx, 2, true);
            } else {
                ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
            }
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        match qs.state(ctx) {
            CREATED => {
                htmltext = if ctx.player_level(player) >= MIN_LEVEL {
                    "31043-01.htm"
                } else {
                    "31043-00.htm"
                }
                .to_owned();
            }
            STARTED => {
                htmltext = if qs.is_cond(ctx, 1) {
                    "31043-03.html"
                } else {
                    "31043-04.html"
                }
                .to_owned();
            }
            _ => {}
        }
        Some(htmltext)
    }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::{Script, ScriptRegistry};

    use super::*;
    use crate::testing::{Pages, ScriptTest, rows};

    const NAME: &str = "Q00432_BirthdayPartySong";

    const NPCS: &str = r#"
        <npc id="31043" type="Folk" level="70" name="Octavia"/>
        <npc id="21103" type="Monster" level="40" name="Golem"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="7541" type="EtcItem" name="Red Crystal"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="7061" type="EtcItem" name="Echo Crystal"><set name="is_stackable" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00432BirthdayPartySong);
        let pages = Pages::of(&Q00432BirthdayPartySong)
            .page("31043-01.htm", &["31043-02.htm"])
            .plain(&[
                "31043-00.htm",
                "31043-02.htm",
                "31043-03.html",
                "31043-05.html",
                "31043-06.html",
            ])
            .page("31043-04.html", &["31043-05.html"])
            .npcs(&[OCTAVIA]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let octavia = t.spawn(OCTAVIA, 20);
        (t, octavia)
    }

    /// Kills golems until 50 red crystals are in the bag; returns the number of kills.
    fn hunt(t: &mut ScriptTest) -> usize {
        for kills in 1..1_000 {
            let golem = t.spawn(GOLEM, 60);
            t.kill(1, golem);
            if t.count(1, RED_CRYSTAL) >= CRYSTAL_COUNT {
                return kills;
            }
        }
        panic!("no 50 crystals after 1000 kills");
    }

    #[test]
    fn a_character_below_level_31_is_sent_away() {
        let (mut t, octavia) = setup();
        t.enter(1, 30, vec![]);
        assert!(t.quest_page(1, octavia).contains("31043-00.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn fifty_red_crystals_then_the_echo_crystals() {
        let (mut t, octavia) = setup();
        t.enter(1, 31, vec![]);
        assert!(t.quest_page(1, octavia).contains("31043-01.htm"));
        assert!(t.event(1, NAME, "31043-02.htm").contains("31043-02.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        assert!(t.quest_page(1, octavia).contains("31043-03.html"));

        // The 50th crystal sets cond 2 (`setCond(2, true)`: the middle sound).
        hunt(&mut t);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);
        assert!(t.quest_page(1, octavia).contains("31043-04.html"));

        // The turn-in: 25 echo crystals, the quest ends (repeatable).
        assert!(t.event(1, NAME, "31043-05.html").contains("31043-05.html"));
        assert_eq!(t.count(1, ECHO_CRYSTAL), 25);
        assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
        assert!(t.world.quest_list(Oid(1)).quests.is_empty());
    }

    #[test]
    fn a_turn_in_with_too_few_crystals_gets_the_page_only() {
        // The turn-in link is only on the cond 2 page, so this branch is called directly.
        let (mut t, octavia) = setup();
        t.enter(1, 31, vec![]);
        assert!(t.quest_page(1, octavia).contains("31043-01.htm"));
        assert!(t.event(1, NAME, "31043-02.htm").contains("31043-02.htm"));
        t.take_writes();
        let mut out = None;
        t.world.run_as_script(NAME, |ctx| {
            out = Q00432BirthdayPartySong.on_event(ctx, "31043-05.html", None, Some(Oid(1)));
        });
        assert_eq!(out.as_deref(), Some("31043-06.html"));
        assert_eq!(t.count(1, ECHO_CRYSTAL), 0);
        assert!(t.take_writes().is_empty());
    }
}
