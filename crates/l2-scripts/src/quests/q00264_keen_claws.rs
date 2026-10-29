//! Quest 264: Keen Claws (Paint in the Talking Island area collects wolf claws from the wolves).
// Java: dist/game/data/scripts/quests/Q00264_KeenClaws/Q00264_KeenClaws.java
//
// Pattern: a kill-count quest with a graded drop (`docs/design/scripts.md`, "Quest patterns"): a
// roll of 100 per kill gives 2 or 8 wolf claws (wolves) or 1 or 2 (kasha/wolf chief monsters) up to
// 50 claws; the 50th sets cond 2. Paint pays one random reward out of a table of seventeen.

use l2_util::ObjectId;
use l2_world::script::{CREATED, QuestSound, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// Npc
const PAINT: i32 = 30136;

// Item
const WOLF_CLAW: i32 = 1367;

// Misc
const MIN_LEVEL: i32 = 3;
const WOLF_CLAW_COUNT: i64 = 50;

/// Java `MONSTER_CHANCES`: per monster, `(claws, threshold)` pairs. The first pair whose
/// threshold is above the roll (of 100) gives its amount of claws.
fn monster_chances(npc_id: i32) -> &'static [(i64, i32)] {
    match npc_id {
        20003 => &[(2, 25), (8, 50)],
        20456 => &[(1, 80), (2, 100)],
        _ => &[],
    }
}

/// Java `REWARDS` in the order `HashMap<Integer, ..>` iterates it. The keys 1, 2, 5, 8, 11, 14 and
/// 17 sit in a 16-slot table, so 17 shares bucket 1 with 1 and comes right after it. The keys are
/// `chance <` thresholds: the first one above the roll wins, so a roll of 1..=16 gives the key-17
/// reward (never the key-2 .. key-14 ones) and only a roll of 0 gives the key-1 reward.
// NOTE(java-quirk): the table order of `HashMap` is kept on purpose; the rewards of keys 2, 5, 8,
// 11 and 14 are never given.
const REWARDS: [(i32, &[(i32, i64)]); 7] = [
    (1, &[(4633, 1)]),
    (17, &[(35, 1), (57, 50)]),
    (2, &[(57, 2000)]),
    (5, &[(5140, 1)]),
    (8, &[(735, 1), (57, 50)]),
    (11, &[(737, 1)]),
    (14, &[(734, 1)]),
];

/// Java `Q00264_KeenClaws`.
pub struct Q00264KeenClaws;

impl Script for Q00264KeenClaws {
    fn name(&self) -> &'static str {
        "Q00264_KeenClaws"
    }

    fn path(&self) -> &'static str {
        "quests/Q00264_KeenClaws"
    }

    fn id(&self) -> i32 {
        264
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[PAINT]);
        s.add_talk_id(&[PAINT]);
        s.add_kill_id(&[20003, 20456]);
        s.register_quest_items(&[WOLF_CLAW]);
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
        if event == "30136-03.htm" {
            qs.start_quest(ctx);
            return Some(event.to_owned());
        }
        None
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        let qs = ctx.get_quest_state(killer, false);
        if let Some(qs) = qs
            && qs.is_cond(ctx, 1)
        {
            let random = ctx.get_random(100);
            for &(amount, threshold) in monster_chances(npc.id) {
                if random < threshold {
                    // Java's 6-argument overload: no NPC, limit 50, always dropped.
                    if ctx.give_item_randomly(
                        killer,
                        None,
                        WOLF_CLAW,
                        amount,
                        amount,
                        WOLF_CLAW_COUNT,
                        1.0,
                        true,
                    ) {
                        // `setCond(2)`: no sound of its own.
                        qs.set_cond(ctx, 2, false);
                    }
                    break;
                }
            }
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        match qs.state(ctx) {
            CREATED => {
                htmltext = if ctx.player_level(player) >= MIN_LEVEL {
                    "30136-02.htm"
                } else {
                    "30136-01.htm"
                }
                .to_owned();
            }
            STARTED => match qs.get_cond(ctx) {
                1 => htmltext = "30136-04.html".to_owned(),
                2 => {
                    if ctx.get_quest_items_count(player, WOLF_CLAW) >= WOLF_CLAW_COUNT {
                        let chance = ctx.get_random(17);
                        for &(key, items) in &REWARDS {
                            if chance < key {
                                for &(item, count) in items {
                                    ctx.reward_items(player, item, count);
                                }
                                if chance == 0 {
                                    ctx.play_sound(player, QuestSound::ItemsoundQuestJackpot);
                                }
                                break;
                            }
                        }
                        qs.exit_quest(ctx, true, true);
                        htmltext = "30136-05.html".to_owned();
                    }
                }
                _ => {}
            },
            _ => {}
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
    use crate::testing::{Pages, ScriptTest, rows};

    const NAME: &str = "Q00264_KeenClaws";

    const NPCS: &str = r#"
        <npc id="30136" type="Folk" level="70" name="Paint"/>
        <npc id="20003" type="Monster" level="2" name="Wolf"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="1367" type="EtcItem" name="Wolf Claw"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="4633" type="EtcItem" name="Reward A"/>
        <item id="35" type="EtcItem" name="Reward B"/>"#;

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00264KeenClaws);
        let pages = Pages::of(&Q00264KeenClaws)
            .page("30136-02.htm", &["30136-03.htm"])
            .plain(&["30136-01.htm", "30136-03.htm", "30136-04.html", "30136-05.html"])
            .npcs(&[PAINT]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let paint = t.spawn(PAINT, 20);
        (t, paint)
    }

    #[test]
    fn a_level_two_character_is_sent_away() {
        let (mut t, paint) = setup();
        t.enter_as(1, 2, race::HUMAN, player_class::FIGHTER, vec![]);
        assert!(t.quest_page(1, paint).contains("30136-01.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn fifty_claws_then_one_of_the_rewards() {
        let (mut t, paint) = setup();
        t.enter_as(1, 3, race::HUMAN, player_class::FIGHTER, vec![]);
        assert!(t.quest_page(1, paint).contains("30136-02.htm"));
        assert!(t.event(1, NAME, "30136-03.htm").contains("30136-03.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        assert!(t.quest_page(1, paint).contains("30136-04.html"));

        // The claws come 2 or 8 at a time; the 50th sets cond 2 (`setCond(2)`).
        for _ in 0..200 {
            if t.count(1, WOLF_CLAW) >= WOLF_CLAW_COUNT {
                break;
            }
            let wolf = t.spawn(20003, 60);
            t.kill(1, wolf);
        }
        assert_eq!(t.count(1, WOLF_CLAW), WOLF_CLAW_COUNT);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);

        // Paint gives one reward: the roll of 0 gives the 4633, 1..=16 the 35 and 50 adena.
        assert!(t.quest_page(1, paint).contains("30136-05.html"));
        let first = t.count(1, 4633) == 1;
        let second = t.count(1, 35) == 1 && t.count(1, 57) == 50;
        assert!(first ^ second, "one reward out of the two possible");
        assert_eq!(t.count(1, WOLF_CLAW), 0);
        assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
    }
}
