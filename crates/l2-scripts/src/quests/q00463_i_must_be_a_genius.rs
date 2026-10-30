//! Quest 463: I Must Be a Genius (Gutenhagen in the Rune Castle Town, Corpse Logs of the rogues).
// Java: dist/game/data/scripts/quests/Q00463_IMustBeaGenius/Q00463_IMustBeaGenius.java
//
// Pattern: a daily kill quest with a count target. The start draws a number (550..600) and a
// chance; the rogues add or take corpse logs (a `special` chance for some of them rolls 1..100
// instead). The log count must reach the number exactly (NOTE(java-quirk) below); then the
// collection is handed in for the daily experience.

use l2_protocol_game::NpcStringId;
use l2_protocol_game::server::ChatType;
use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, QuestSound, QuestType, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const GUTENHAGEN: i32 = 32069;

// Items
const CORPSE_LOG: i32 = 15510;
const COLLECTION: i32 = 15511;

/// Java `DropInfo`: the log count of a kill (negative: taken away), and the chance value that
/// rolls 1..100 instead (`special_chance`, -1 when never).
#[derive(Clone, Copy)]
struct DropInfo {
    count: i32,
    special_chance: i32,
}

/// Java `MOBS`: the rogues of the quest.
fn mob(npc_id: i32) -> Option<DropInfo> {
    let (count, special_chance) = match npc_id {
        22801 => (5, 0),
        22802 => (5, 0),
        22803 => (5, 0),
        22804 => (-2, 1),
        22805 => (-2, 1),
        22806 => (-2, 1),
        22807 => (-1, -1),
        22809 => (2, 2),
        22810 => (-3, 3),
        22811 => (3, -1),
        22812 => (1, -1),
        _ => return None,
    };
    Some(DropInfo {
        count,
        special_chance,
    })
}

/// Java `REWARD`: (exp, sp, html) per draw; the html number is the page `32069-NN.html`.
const REWARD: [(i64, i32, i32); 10] = [
    (198_725, 15_892, 8),
    (278_216, 22_249, 8),
    (317_961, 25_427, 8),
    (357_706, 28_606, 9),
    (397_451, 31_784, 9),
    (596_176, 47_677, 9),
    (715_411, 57_212, 10),
    (794_901, 63_569, 10),
    (914_137, 73_104, 10),
    (1_192_352, 95_353, 11),
];

// Misc
const MIN_LEVEL: i32 = 70;

/// Java `Q00463_IMustBeaGenius`.
pub struct Q00463IMustBeaGenius;

impl Script for Q00463IMustBeaGenius {
    fn name(&self) -> &'static str {
        "Q00463_IMustBeaGenius"
    }

    fn path(&self) -> &'static str {
        "quests/Q00463_IMustBeaGenius"
    }

    fn id(&self) -> i32 {
        463
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[GUTENHAGEN]);
        s.add_talk_id(&[GUTENHAGEN]);
        s.add_kill_id(&[
            22801, 22802, 22803, 22804, 22805, 22806, 22807, 22809, 22810, 22811, 22812,
        ]);
        s.register_quest_items(&[COLLECTION, CORPSE_LOG]);
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
        let mut htmltext = Some(event.to_owned());
        match event {
            "32069-03.htm" => {
                qs.start_quest(ctx);
                let number = ctx.get_random(51) + 550;
                qs.set(ctx, "number", &number.to_string());
                let chance = ctx.get_random(4);
                qs.set(ctx, "chance", &chance.to_string());
                htmltext = Some(
                    ctx.get_htm(player, event)
                        .unwrap_or_default()
                        .replace("%num%", &number.to_string()),
                );
            }
            "32069-05.htm" => {
                let number = qs.get(ctx, "number").unwrap_or_default();
                htmltext = Some(
                    ctx.get_htm(player, event)
                        .unwrap_or_default()
                        .replace("%num%", &number),
                );
            }
            "reward" => {
                // NOTE(java-quirk): without the collection the event falls through and the
                // answer is the event name itself, which the NPC says in chat.
                if qs.is_cond(ctx, 2) {
                    let rnd = ctx.get_random(REWARD.len() as i32) as usize;
                    let (exp, sp, html) = REWARD[rnd];
                    let str = if html < 10 {
                        format!("0{html}")
                    } else {
                        html.to_string()
                    };
                    ctx.add_exp_and_sp(player, exp, sp);
                    qs.exit_quest_type(ctx, QuestType::Daily, true);
                    htmltext = Some(format!("32069-{str}.html"));
                }
            }
            "32069-02.htm" => {}
            _ => htmltext = None,
        }
        htmltext
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        let Some(qs) = ctx.get_quest_state(killer, false) else {
            return;
        };
        if !qs.is_cond(ctx, 1) {
            return;
        }
        let Some(info) = mob(npc.id) else {
            return;
        };
        let number = if info.special_chance == qs.get_int(ctx, "chance") {
            ctx.get_random(100) + 1
        } else {
            info.count
        };
        let mut msg = false;
        if number > 0 {
            ctx.give_items(killer, CORPSE_LOG, i64::from(number));
            msg = true;
        } else if number < 0 && ctx.get_quest_items_count(killer, CORPSE_LOG) + i64::from(number) > 0 {
            ctx.take_items(killer, CORPSE_LOG, i64::from(number.abs()));
            msg = true;
        }
        if msg {
            let name = ctx.player_name(killer);
            let count = number.to_string();
            ctx.broadcast_npc_say(
                npc,
                ChatType::NpcGeneral,
                NpcStringId::AttAttackS1RoRogueS2,
                &[&name, &count],
            );
            ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
            // NOTE(java-quirk): the count must hit the number exactly; a jump over it (the 1..100
            // rolls) leaves the quest at cond 1 for good, as in Java.
            if ctx.get_quest_items_count(killer, CORPSE_LOG) == i64::from(qs.get_int(ctx, "number")) {
                ctx.take_items(killer, CORPSE_LOG, -1);
                ctx.give_items(killer, COLLECTION, 1);
                qs.set_cond(ctx, 2, true);
            }
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        let mut state = qs.state(ctx);
        if state == COMPLETED {
            if !qs.is_now_available(ctx) {
                return Some("32069-07.htm".to_owned());
            }
            qs.set_state(ctx, CREATED);
            state = CREATED;
        }
        match state {
            CREATED => {
                htmltext = if ctx.player_level(player) >= MIN_LEVEL {
                    "32069-01.htm"
                } else {
                    "32069-00.htm"
                }
                .to_owned();
            }
            STARTED => {
                if qs.is_cond(ctx, 1) {
                    htmltext = "32069-04.html".to_owned();
                } else if qs.get_int(ctx, "var") == 1 {
                    htmltext = "32069-06a.html".to_owned();
                } else {
                    ctx.take_items(player, COLLECTION, -1);
                    qs.set(ctx, "var", "1");
                    htmltext = "32069-06.html".to_owned();
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
    use l2_world::{QuestWrite, ScriptRegistry};

    use super::*;
    use crate::testing::{Pages, ScriptTest, rows};

    const NAME: &str = "Q00463_IMustBeaGenius";
    const DIR: &str = "data/scripts/quests/Q00463_IMustBeaGenius";

    const NPCS: &str = r#"
        <npc id="32069" type="Folk" level="70" name="Gutenhagen"/>
        <npc id="22812" type="Monster" level="70" name="Rogue"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="15510" type="EtcItem" name="Corpse Log"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="15511" type="EtcItem" name="Collection"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00463IMustBeaGenius);
        let pages = Pages::of(&Q00463IMustBeaGenius)
            .page("32069-01.htm", &["32069-02.htm"])
            .page("32069-02.htm", &["32069-03.htm"])
            .file(
                &format!("{DIR}/32069-03.htm"),
                "<html><body>Rogues: %num%</body></html>",
            )
            .file(
                &format!("{DIR}/32069-05.htm"),
                "<html><body>Bring %num% logs</body></html>",
            )
            .plain(&["32069-00.htm", "32069-04.html", "32069-07.htm"])
            .page("32069-06.html", &["reward"])
            .page("32069-06a.html", &["reward"])
            .plain(&["32069-08.html", "32069-09.html", "32069-10.html", "32069-11.html"])
            .page("32069-04.html", &["32069-05.htm"])
            .npcs(&[GUTENHAGEN]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let gutenhagen = t.spawn(GUTENHAGEN, 20);
        (t, gutenhagen)
    }

    /// The `number` and `chance` the start drew, from the writes of the start.
    fn drawn(writes: &[QuestWrite]) -> (i64, i64) {
        let value = |var: &str| {
            writes
                .iter()
                .find_map(|w| match w {
                    QuestWrite::InsertVar { var: v, value, .. } if v == var => value.parse::<i64>().ok(),
                    _ => None,
                })
                .unwrap()
        };
        (value("number"), value("chance"))
    }

    #[test]
    fn a_level_69_character_is_sent_away() {
        let (mut t, gutenhagen) = setup();
        t.enter(1, 69, vec![]);
        assert!(t.quest_page(1, gutenhagen).contains("32069-00.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_start_draws_the_number_and_the_chance() {
        let (mut t, gutenhagen) = setup();
        t.enter(1, 70, vec![]);
        assert!(t.quest_page(1, gutenhagen).contains("32069-01.htm"));
        assert!(t.event(1, NAME, "32069-02.htm").contains("32069-02.htm"));
        assert!(t.event(1, NAME, "32069-03.htm").contains("Rogues: 5"));
        let writes = t.take_writes();
        assert_eq!(
            writes[..2],
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        let (number, chance) = drawn(&writes);
        assert!((550..=600).contains(&number), "{number}");
        assert!((0..4).contains(&chance), "{chance}");
    }

    #[test]
    fn the_logs_reach_the_number_then_the_collection_and_the_reward() {
        let (mut t, gutenhagen) = setup();
        t.enter(1, 70, vec![]);
        t.quest_page(1, gutenhagen);
        t.event(1, NAME, "32069-02.htm");
        t.event(1, NAME, "32069-03.htm");
        let (number, _chance) = drawn(&t.take_writes());
        t.take();
        // The rogue 22812 adds one log for each kill (never special), so the count is exact.
        for _ in 0..number {
            let rogue = t.spawn(22812, 60);
            t.kill(1, rogue);
        }
        assert_eq!(t.count(1, CORPSE_LOG), 0);
        assert_eq!(t.count(1, COLLECTION), 1);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);

        // Talk: the collection goes in, the variable `var` is set (`06.html`), then `06a.html`.
        assert!(t.quest_page(1, gutenhagen).contains("32069-06.html"));
        assert_eq!(t.count(1, COLLECTION), 0);
        assert_eq!(t.take_writes(), [rows::insert(NAME, "var", "1")]);
        assert!(t.quest_page(1, gutenhagen).contains("32069-06a.html"));

        // The reward: one of the ten draws, the daily lock.
        t.take();
        let out = t.event(1, NAME, "reward");
        assert!(
            ["08", "09", "10", "11"]
                .iter()
                .any(|n| out.contains(&format!("32069-{n}.html"))),
            "{out}"
        );
        let writes = t.take_writes();
        assert_eq!(
            writes[..2],
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed")
            ]
        );
        assert!(matches!(&writes[2], QuestWrite::InsertVar { var, .. } if var == "restartTime"));
        assert!(t.quest_page(1, gutenhagen).contains("32069-07.htm"));
    }

    #[test]
    fn the_reward_without_the_collection_answers_the_event_name() {
        let (mut t, gutenhagen) = setup();
        t.enter(1, 70, vec![]);
        t.quest_page(1, gutenhagen);
        t.event(1, NAME, "32069-02.htm");
        t.event(1, NAME, "32069-03.htm");
        t.take_writes();
        t.world.run_as_script(NAME, |ctx| {
            assert_eq!(
                Q00463IMustBeaGenius.on_event(ctx, "reward", None, Some(Oid(1))),
                Some("reward".to_owned())
            );
        });
        assert!(t.take_writes().is_empty());
    }
}
