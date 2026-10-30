//! Quest 453: Not Strong Enough Alone (Klemis; three groups of monsters, one phase each).
// Java: dist/game/data/scripts/quests/Q00453_NotStrongEnoughAlone/Q00453_NotStrongEnoughAlone.java
//
// Pattern: party kills with a count per monster (every party member in range counts the kill, the
// log packet of the counts goes to each of them), the phase chosen by an event link of the
// previous page, and a reward roll at cond 5 (daily).

use l2_protocol_game::server::ExQuestNpcLogList;
use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, Qs, QuestSound, QuestType, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const KLEMIS: i32 = 32734;

// Monsters: each group folds its second half onto its first half (the first kill counts for the
// same log line), see `fold`.
const MONSTER1: [i32; 8] = [22746, 22747, 22748, 22749, 22750, 22751, 22752, 22753];
const MONSTER2: [i32; 6] = [22754, 22755, 22756, 22757, 22758, 22759];
const MONSTER3: [i32; 6] = [22760, 22761, 22762, 22763, 22764, 22765];

// Reward: one of the two groups, then one of its items.
const REWARD: [[i32; 11]; 2] = [
    [
        15815, 15816, 15817, 15818, 15819, 15820, 15821, 15822, 15823, 15824, 15825,
    ],
    [
        15634, 15635, 15636, 15637, 15638, 15639, 15640, 15641, 15642, 15643, 15644,
    ],
];

/// Java `Q10282_ToTheSeedOfAnnihilation.class.getSimpleName()`: the quest that must be done.
const PREVIOUS_QUEST: &str = "Q10282_ToTheSeedOfAnnihilation";

/// Java `Q00453_NotStrongEnoughAlone`.
pub struct Q00453NotStrongEnoughAlone;

impl Script for Q00453NotStrongEnoughAlone {
    fn name(&self) -> &'static str {
        "Q00453_NotStrongEnoughAlone"
    }

    fn path(&self) -> &'static str {
        "quests/Q00453_NotStrongEnoughAlone"
    }

    fn id(&self) -> i32 {
        453
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[KLEMIS]);
        s.add_talk_id(&[KLEMIS]);
        s.add_kill_id(&MONSTER1);
        s.add_kill_id(&MONSTER2);
        s.add_kill_id(&MONSTER3);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        _npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        // Java returns the event itself when there is no state.
        let Some(player) = player else {
            return Some(event.to_owned());
        };
        let Some(qs) = ctx.get_quest_state(player, false) else {
            return Some(event.to_owned());
        };

        // Java `equalsIgnoreCase`: the page names are plain ASCII.
        if event.eq_ignore_ascii_case("32734-06.htm") {
            qs.start_quest(ctx);
        } else if event.eq_ignore_ascii_case("32734-07.html") {
            qs.set_cond(ctx, 2, true);
        } else if event.eq_ignore_ascii_case("32734-08.html") {
            qs.set_cond(ctx, 3, true);
        } else if event.eq_ignore_ascii_case("32734-09.html") {
            qs.set_cond(ctx, 4, true);
        }
        Some(event.to_owned())
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        // Java: every party member (the killer alone without a party) counts the kill.
        for member in ctx.world().party_members_of(killer) {
            increase_kill(ctx, member, npc);
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let prev_completed = ctx
            .player_quest_state(player, PREVIOUS_QUEST)
            .is_some_and(|prev| prev.is_completed());
        let mut htmltext = ctx.get_no_quest_msg(player);

        match qs.state(ctx) {
            CREATED => {
                htmltext = if ctx.player_level(player) >= 84 && prev_completed {
                    "32734-01.htm"
                } else {
                    "32734-03.html"
                }
                .to_owned();
            }
            STARTED => match qs.get_cond(ctx) {
                1 => htmltext = "32734-10.html".to_owned(),
                2 => htmltext = "32734-11.html".to_owned(),
                3 => htmltext = "32734-12.html".to_owned(),
                4 => htmltext = "32734-13.html".to_owned(),
                5 => {
                    // Java `getRandomEntry(REWARD)[getRandom(REWARD[0].length)]`: the group, then
                    // the item (both groups have 11 items).
                    if let Some(group) = ctx.get_random_entry(&REWARD) {
                        let index = ctx.get_random(11) as usize;
                        ctx.give_items(player, group[index], 1);
                    }
                    qs.exit_quest_type(ctx, QuestType::Daily, true);
                    htmltext = "32734-14.html".to_owned();
                }
                _ => {}
            },
            COMPLETED => {
                if !qs.is_now_available(ctx) {
                    htmltext = "32734-02.htm".to_owned();
                } else {
                    qs.set_state(ctx, CREATED);
                    htmltext = if ctx.player_level(player) >= 84 && prev_completed {
                        "32734-01.htm"
                    } else {
                        "32734-03.html"
                    }
                    .to_owned();
                }
            }
            _ => {}
        }
        Some(htmltext)
    }
}

/// Java `increaseKill(player, npc)`: the count of the monster (up to its limit) in the quest
/// variable named after the monster's id, for the phase the quest is in.
fn increase_kill(ctx: &mut ScriptCtx<'_>, player: ObjectId, npc: ScriptNpc) {
    let Some(qs) = ctx.get_quest_state(player, false) else {
        return;
    };
    if !ctx.check_if_in_range(ctx.alt_party_range(), npc.object_id, player, false) {
        return;
    }

    let mut log = ExQuestNpcLogList::new(ctx.quest_id());
    if MONSTER1.contains(&npc.id) && qs.is_cond(ctx, 2) {
        let variable = fold(&MONSTER1, npc.id).to_string();
        let count = qs.get_int(ctx, &variable);
        if count < 15 {
            qs.set(ctx, &variable, &(count + 1).to_string());
            ctx.play_sound(player, QuestSound::ItemsoundQuestItemget);
        }
        check_progress(ctx, qs, 15, &MONSTER1[..4]);
        for mob in &MONSTER1[..4] {
            log.add_npc(*mob, qs.get_int(ctx, &mob.to_string()));
        }
    } else if MONSTER2.contains(&npc.id) && qs.is_cond(ctx, 3) {
        let variable = fold(&MONSTER2, npc.id).to_string();
        let count = qs.get_int(ctx, &variable);
        if count < 20 {
            qs.set(ctx, &variable, &(count + 1).to_string());
            ctx.play_sound(player, QuestSound::ItemsoundQuestItemget);
        }
        check_progress(ctx, qs, 20, &MONSTER2[..3]);
        for mob in &MONSTER2[..3] {
            log.add_npc(*mob, qs.get_int(ctx, &mob.to_string()));
        }
    } else if MONSTER3.contains(&npc.id) && qs.is_cond(ctx, 4) {
        let variable = fold(&MONSTER3, npc.id).to_string();
        let count = qs.get_int(ctx, &variable);
        if count < 20 {
            qs.set(ctx, &variable, &(count + 1).to_string());
            ctx.play_sound(player, QuestSound::ItemsoundQuestItemget);
        }
        check_progress(ctx, qs, 20, &MONSTER3[..3]);
        for mob in &MONSTER3[..3] {
            log.add_npc(*mob, qs.get_int(ctx, &mob.to_string()));
        }
    }
    ctx.world().send(player, &log);
}

/// Java's `if (npcId == MONSTER1[4]) npcId = MONSTER1[0]; ...`: the monsters of the second half
/// of a group count as the matching monster of its first half.
fn fold(group: &[i32], npc_id: i32) -> i32 {
    let half = group.len() / 2;
    match group.iter().position(|id| *id == npc_id) {
        Some(index) if index >= half => group[index - half],
        _ => npc_id,
    }
}

/// Java `checkProgress(qs, count, mobs...)`: when every monster of the group reached `count`, the
/// quest moves to cond 5.
// NOTE(java-quirk): the phases of cond 2, 3 and 4 all call `setCond(5, true)` once their group is
// complete. So the first group finished in cond 2 already moves the quest to the reward, and the
// later phases are skipped. Ported as Java has it.
fn check_progress(ctx: &mut ScriptCtx<'_>, qs: Qs, count: i32, mobs: &[i32]) {
    for mob in mobs {
        if qs.get_int(ctx, &mob.to_string()) < count {
            return;
        }
    }
    qs.set_cond(ctx, 5, true);
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::{QuestLoad, QuestWrite, Script, ScriptRegistry, ScriptSetup};

    use super::*;
    use crate::testing::{Pages, ScriptTest, player_data, rows};

    const NAME: &str = "Q00453_NotStrongEnoughAlone";

    const NPCS: &str = r#"
        <npc id="32734" type="Folk" level="70" name="Klemis"/>
        <npc id="22746" type="Monster" level="80" name="Ghoul"/>
        <npc id="22747" type="Monster" level="80" name="Ghoul"/>
        <npc id="22748" type="Monster" level="80" name="Ghoul"/>
        <npc id="22749" type="Monster" level="80" name="Ghoul"/>
        <npc id="22750" type="Monster" level="80" name="Ghoul"/>"#;

    /// Java `Q10282_ToTheSeedOfAnnihilation` as far as this quest needs it: a script of that name.
    struct PreviousQuest;

    impl Script for PreviousQuest {
        fn name(&self) -> &'static str {
            PREVIOUS_QUEST
        }
        fn path(&self) -> &'static str {
            "quests/Q10282_ToTheSeedOfAnnihilation"
        }
        fn setup(&self, _setup: &mut ScriptSetup) {}
    }

    fn items() -> String {
        let mut xml = String::from(
            r#"<item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>"#,
        );
        for id in REWARD.iter().flatten() {
            xml.push_str(&format!(
                r#"<item id="{id}" type="Armor" name="Reward {id}"><set name="bodypart" val="chest"/></item>"#
            ));
        }
        xml
    }

    /// Enters the character 1 of `level`; `done_previous` gives it the finished Q10282 row.
    fn enter(t: &mut ScriptTest, level: i32, done_previous: bool) {
        let mut data = player_data(1, level, vec![]);
        if done_previous {
            data.profile.quests = QuestLoad {
                states: vec![(PREVIOUS_QUEST.to_owned(), Some("Completed".to_owned()))],
                variables: vec![],
            };
        }
        t.enter_with(data);
    }

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(PreviousQuest);
        registry.add(Q00453NotStrongEnoughAlone);
        let pages = Pages::of(&Q00453NotStrongEnoughAlone)
            .page("32734-01.htm", &["32734-04.html"])
            .page("32734-04.html", &["32734-05.htm"])
            .page("32734-05.htm", &["32734-06.htm"])
            .page(
                "32734-06.htm",
                &["32734-07.html", "32734-08.html", "32734-09.html"],
            )
            .page(
                "32734-10.html",
                &["32734-07.html", "32734-08.html", "32734-09.html"],
            )
            .plain(&[
                "32734-02.htm",
                "32734-03.html",
                "32734-07.html",
                "32734-08.html",
                "32734-09.html",
                "32734-11.html",
                "32734-12.html",
                "32734-13.html",
                "32734-14.html",
            ])
            .npcs(&[KLEMIS]);
        let items = items();
        let mut t = ScriptTest::with_pages(registry, NPCS, &items, pages);
        let klemis = t.spawn(KLEMIS, 20);
        (t, klemis)
    }

    /// Kills `count` of a monster in range of the killer.
    fn kill_n(t: &mut ScriptTest, id: i32, count: usize) {
        for _ in 0..count {
            let mob = t.spawn(id, 60);
            t.kill(1, mob);
        }
    }

    #[test]
    fn without_the_previous_quest_the_character_is_sent_away() {
        let (mut t, klemis) = setup();
        enter(&mut t, 84, false);
        assert!(t.quest_page(1, klemis).contains("32734-03.html"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn a_level_83_character_is_sent_away() {
        let (mut t, klemis) = setup();
        enter(&mut t, 83, true);
        assert!(t.quest_page(1, klemis).contains("32734-03.html"));
    }

    #[test]
    fn the_first_group_then_the_reward() {
        let (mut t, klemis) = setup();
        enter(&mut t, 84, true);
        assert!(t.quest_page(1, klemis).contains("32734-01.htm"));
        assert!(t.event(1, NAME, "32734-04.html").contains("32734-04.html"));
        assert!(t.event(1, NAME, "32734-05.htm").contains("32734-05.htm"));
        assert!(t.event(1, NAME, "32734-06.htm").contains("32734-06.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );

        // Cond 2 (the link on the accept page): the first group, 15 of each monster.
        assert!(t.event(1, NAME, "32734-07.html").contains("32734-07.html"));
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);
        assert!(t.quest_page(1, klemis).contains("32734-11.html"));

        // The second half of the group counts for its first half: 22750 is the 22746 line, so
        // 10 + 5 kills make its 15 (the count stops at 15).
        kill_n(&mut t, 22746, 10);
        kill_n(&mut t, 22750, 5);
        let writes = t.take_writes();
        assert!(writes.contains(&rows::insert(NAME, "22746", "1")), "{writes:?}");
        assert!(writes.contains(&rows::update(NAME, "22746", "15")), "{writes:?}");

        // The other three lines; the last kill makes the group complete: cond 5.
        kill_n(&mut t, 22747, 15);
        kill_n(&mut t, 22748, 15);
        kill_n(&mut t, 22749, 15);
        let writes = t.take_writes();
        assert!(writes.contains(&rows::update(NAME, "cond", "5")), "{writes:?}");
        assert!(t.quest_page(1, klemis).contains("32734-14.html"));
        // The reward: one item of a group, then the daily quest.
        let rewards: i64 = REWARD.iter().flatten().map(|id| t.count(1, *id)).sum();
        assert_eq!(rewards, 1);
        let writes = t.take_writes();
        assert_eq!(
            writes[..2],
            [
                QuestWrite::DeleteQuest {
                    char_id: 1,
                    name: NAME.to_owned(),
                    repeatable: false,
                },
                rows::update(NAME, "<state>", "Completed"),
            ]
        );
        assert!(matches!(&writes[2], QuestWrite::InsertVar { var, .. } if var == "restartTime"));
        assert!(t.world.quest_list(Oid(1)).quests.is_empty());
        assert!(t.quest_page(1, klemis).contains("32734-02.htm"));
    }

    #[test]
    fn the_phase_link_decides_the_cond() {
        let (mut t, klemis) = setup();
        enter(&mut t, 84, true);
        assert!(t.quest_page(1, klemis).contains("32734-01.htm"));
        t.event(1, NAME, "32734-04.html");
        t.event(1, NAME, "32734-05.htm");
        t.event(1, NAME, "32734-06.htm");
        t.take_writes();
        // The third group's link: cond 4. Skipping cond 2 and 3 writes the completed-step flags
        // (Java `QuestState.setCond`, the `completedStateFlags` bits).
        assert!(t.event(1, NAME, "32734-09.html").contains("32734-09.html"));
        let writes = t.take_writes();
        assert_eq!(writes[0], rows::update(NAME, "cond", "4"));
        assert!(matches!(&writes[1], QuestWrite::InsertVar { var, .. } if var == "__compltdStateFlags"));
        assert!(t.quest_page(1, klemis).contains("32734-13.html"));
    }
}
