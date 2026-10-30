//! Quest 134: Temple Missionary (Glyvka and Rouke; fragments, tools and three reports).
// Java: dist/game/data/scripts/quests/Q00134_TempleMissionary/Q00134_TempleMissionary.java
//
// Pattern: party kills (`get_random_party_member_cond`) that drop a fragment, turn a tool into a
// traitor spawn, and the traitor drops the reports. Items are counted on the killer (Java quirk).

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, QuestSound, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const GLYVKA: i32 = 30067;
const ROUKE: i32 = 31418;

// Items
const GIANTS_EXPERIMENTAL_TOOL_FRAGMENT: i32 = 10335;
const GIANTS_EXPERIMENTAL_TOOL: i32 = 10336;
const GIANTS_TECHNOLOGY_REPORT: i32 = 10337;
const ROUKES_REPOT: i32 = 10338;
const BADGE_TEMPLE_MISSIONARY: i32 = 10339;

// Monsters
const CRUMA_MARSHLANDS_TRAITOR: i32 = 27339;

/// Java `MOBS`: the chance (out of 100) of a fragment per monster.
fn mob_chance(npc_id: i32) -> Option<i32> {
    match npc_id {
        20157 => Some(78), // Marsh Stakato
        20229 => Some(75), // Stinger Wasp
        20230 => Some(86), // Marsh Stakato Worker
        20231 => Some(83), // Toad Lord
        20232 => Some(81), // Marsh Stakato Soldier
        20233 => Some(95), // Marsh Spider
        20234 => Some(96), // Marsh Stakato Drone
        _ => None,
    }
}

// Misc
const MIN_LEVEL: i32 = 35;
const MAX_REWARD_LEVEL: i32 = 41;
const FRAGMENT_COUNT: i64 = 10;
const REPORT_COUNT: i64 = 3;

/// Java `Q00134_TempleMissionary`.
pub struct Q00134TempleMissionary;

impl Script for Q00134TempleMissionary {
    fn name(&self) -> &'static str {
        "Q00134_TempleMissionary"
    }

    fn path(&self) -> &'static str {
        "quests/Q00134_TempleMissionary"
    }

    fn id(&self) -> i32 {
        134
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[GLYVKA]);
        s.add_talk_id(&[GLYVKA, ROUKE]);
        s.add_kill_id(&[CRUMA_MARSHLANDS_TRAITOR]);
        s.add_kill_id(&[20157, 20229, 20230, 20231, 20232, 20233, 20234]);
        s.register_quest_items(&[
            GIANTS_EXPERIMENTAL_TOOL_FRAGMENT,
            GIANTS_EXPERIMENTAL_TOOL,
            GIANTS_TECHNOLOGY_REPORT,
            ROUKES_REPOT,
        ]);
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
            "30067-05.html" | "30067-09.html" | "31418-07.html" => {}
            "30067-03.htm" => qs.start_quest(ctx),
            "30067-06.html" => qs.set_cond(ctx, 2, true),
            "31418-03.html" => qs.set_cond(ctx, 3, true),
            "31418-08.html" => {
                qs.set_cond(ctx, 5, true);
                ctx.give_items(player, ROUKES_REPOT, 1);
                qs.unset(ctx, "talk");
            }
            "30067-10.html" => {
                ctx.give_items(player, BADGE_TEMPLE_MISSIONARY, 1);
                ctx.give_adena(player, 15_100, true);
                if ctx.player_level(player) < MAX_REWARD_LEVEL {
                    ctx.add_exp_and_sp(player, 30_000, 2_000);
                }

                qs.exit_quest(ctx, false, true);
            }
            _ => htmltext = None,
        }
        htmltext
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId, _is_summon: bool) {
        let Some(member) = ctx.get_random_party_member_cond(player, 3) else {
            return;
        };

        let qs = ctx.get_quest_state(member, false);
        if npc.id == CRUMA_MARSHLANDS_TRAITOR {
            // NOTE(java-quirk): the reports are counted and given on the killer, not the member.
            ctx.give_items(player, GIANTS_TECHNOLOGY_REPORT, 1);
            if ctx.get_quest_items_count(player, GIANTS_TECHNOLOGY_REPORT) >= REPORT_COUNT {
                if let Some(qs) = qs {
                    qs.set_cond(ctx, 4, true);
                }
            } else {
                ctx.play_sound(player, QuestSound::ItemsoundQuestItemget);
            }
        } else if ctx.has_quest_items(player, GIANTS_EXPERIMENTAL_TOOL) {
            ctx.take_items(player, GIANTS_EXPERIMENTAL_TOOL, 1);
            if ctx.get_random(100) != 0 {
                let heading = ctx.heading_of(npc.object_id);
                if let Some((x, y, z)) = ctx.world().position_of(npc.object_id) {
                    ctx.add_spawn(
                        CRUMA_MARSHLANDS_TRAITOR,
                        x + 20,
                        y + 20,
                        z,
                        heading,
                        false,
                        60_000,
                        0,
                    );
                }
            }
        } else if let Some(chance) = mob_chance(npc.id)
            && ctx.get_random(100) < chance
        {
            ctx.give_items(player, GIANTS_EXPERIMENTAL_TOOL_FRAGMENT, 1);
            ctx.play_sound(player, QuestSound::ItemsoundQuestItemget);
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        match npc.id {
            GLYVKA => match qs.state(ctx) {
                CREATED => {
                    htmltext = if ctx.player_level(player) >= MIN_LEVEL {
                        "30067-01.htm"
                    } else {
                        "30067-02.htm"
                    }
                    .to_owned();
                }
                STARTED => match qs.get_cond(ctx) {
                    1 => htmltext = "30067-04.html".to_owned(),
                    2..=4 => htmltext = "30067-07.html".to_owned(),
                    5 => {
                        if qs.is_set(ctx, "talk") {
                            htmltext = "30067-09.html".to_owned();
                        } else {
                            ctx.take_items(player, ROUKES_REPOT, -1);
                            qs.set(ctx, "talk", "1");
                            htmltext = "30067-08.html".to_owned();
                        }
                    }
                    _ => {}
                },
                COMPLETED => htmltext = ctx.get_already_completed_msg(player),
                _ => {}
            },
            ROUKE => {
                if qs.is_started(ctx) {
                    match qs.get_cond(ctx) {
                        1 => htmltext = "31418-01.html".to_owned(),
                        2 => htmltext = "31418-02.html".to_owned(),
                        3 => {
                            if ctx.get_quest_items_count(player, GIANTS_EXPERIMENTAL_TOOL_FRAGMENT)
                                < FRAGMENT_COUNT
                                && ctx.get_quest_items_count(player, GIANTS_TECHNOLOGY_REPORT) < REPORT_COUNT
                            {
                                htmltext = "31418-04.html".to_owned();
                            } else if ctx.get_quest_items_count(player, GIANTS_EXPERIMENTAL_TOOL_FRAGMENT)
                                >= FRAGMENT_COUNT
                            {
                                let count =
                                    ctx.get_quest_items_count(player, GIANTS_EXPERIMENTAL_TOOL_FRAGMENT) / 10;
                                ctx.take_items(player, GIANTS_EXPERIMENTAL_TOOL_FRAGMENT, count * 10);
                                ctx.give_items(player, GIANTS_EXPERIMENTAL_TOOL, count);
                                htmltext = "31418-05.html".to_owned();
                            }
                        }
                        4 => {
                            if qs.is_set(ctx, "talk") {
                                htmltext = "31418-07.html".to_owned();
                            } else if ctx.get_quest_items_count(player, GIANTS_TECHNOLOGY_REPORT)
                                >= REPORT_COUNT
                            {
                                ctx.take_items(player, GIANTS_EXPERIMENTAL_TOOL_FRAGMENT, -1);
                                ctx.take_items(player, GIANTS_EXPERIMENTAL_TOOL, -1);
                                ctx.take_items(player, GIANTS_TECHNOLOGY_REPORT, -1);
                                qs.set(ctx, "talk", "1");
                                htmltext = "31418-06.html".to_owned();
                            }
                        }
                        5 => htmltext = "31418-09.html".to_owned(),
                        _ => {}
                    }
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

    use super::*;
    use crate::testing::rows::{delete, insert, update};
    use crate::testing::{Pages, ScriptTest};

    const NPCS: &str = r#"
        <npc id="30067" type="Folk" level="40" name="Glyvka"/>
        <npc id="31418" type="Folk" level="40" name="Rouke"/>
        <npc id="20157" type="Monster" level="38" name="Marsh Stakato"/>
        <npc id="27339" type="Monster" level="38" name="Cruma Marshlands Traitor"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="10335" type="EtcItem" name="Fragment"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="10336" type="EtcItem" name="Tool"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="10337" type="EtcItem" name="Report"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="10338" type="EtcItem" name="Rouke's Report"><set name="is_questitem" val="true"/></item>
        <item id="10339" type="EtcItem" name="Badge"/>"#;

    const NAME: &str = "Q00134_TempleMissionary";

    fn setup() -> (ScriptTest, Oid, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00134TempleMissionary);
        let pages = Pages::of(&Q00134TempleMissionary)
            .page("30067-01.htm", &["30067-03.htm"])
            .plain(&["30067-02.htm", "30067-03.htm", "30067-07.html"])
            .page("30067-04.html", &["30067-05.html"])
            .page("30067-05.html", &["30067-06.html"])
            .plain(&["30067-06.html", "30067-08.html"])
            .page("30067-09.html", &["30067-10.html"])
            .plain(&["30067-10.html"])
            .plain(&["31418-01.html", "31418-04.html", "31418-05.html"])
            .page("31418-02.html", &["31418-03.html"])
            .plain(&["31418-03.html", "31418-06.html"])
            .page("31418-07.html", &["31418-08.html"])
            .plain(&["31418-08.html", "31418-09.html"])
            .npcs(&[GLYVKA, ROUKE])
            .file("data/html/noquest.htm", "<html><body>noquest</body></html>");
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let glyvka = t.spawn(GLYVKA, 20);
        let rouke = t.spawn(ROUKE, 40);
        (t, glyvka, rouke)
    }

    #[test]
    fn glyvka_gates_level_35() {
        let (mut t, glyvka, _) = setup();
        t.enter(1, 34, vec![]);
        assert!(t.quest_page(1, glyvka).contains("30067-02.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_fragments_tools_reports_and_the_reward() {
        let (mut t, glyvka, rouke) = setup();
        t.enter(1, 38, vec![]);
        assert!(t.quest_page(1, glyvka).contains("30067-01.htm"));
        assert!(t.event(1, NAME, "30067-03.htm").contains("30067-03.htm"));
        assert_eq!(
            t.take_writes(),
            [insert(NAME, "cond", "1"), insert(NAME, "<state>", "Started")]
        );
        assert!(t.quest_page(1, glyvka).contains("30067-04.html"));
        assert!(t.event(1, NAME, "30067-05.html").contains("30067-05.html"));
        assert!(t.event(1, NAME, "30067-06.html").contains("30067-06.html"));
        assert_eq!(t.take_writes(), [update(NAME, "cond", "2")]);

        assert!(t.quest_page(1, rouke).contains("31418-02.html"));
        assert!(t.event(1, NAME, "31418-03.html").contains("31418-03.html"));
        assert_eq!(t.take_writes(), [update(NAME, "cond", "3")]);

        // Ten fragments (a chance per kill), then Rouke turns them into one tool.
        for _ in 0..500 {
            if t.count(1, GIANTS_EXPERIMENTAL_TOOL_FRAGMENT) >= 10 {
                break;
            }
            let mob = t.spawn(20157, 80);
            t.kill(1, mob);
        }
        assert!(t.count(1, GIANTS_EXPERIMENTAL_TOOL_FRAGMENT) >= 10);
        assert!(t.quest_page(1, rouke).contains("31418-05.html"));
        assert_eq!(t.count(1, GIANTS_EXPERIMENTAL_TOOL), 1);

        // A kill with a tool in the bag takes it and (99 times of 100) spawns the traitor; three
        // traitor kills give the three reports and cond 4.
        t.take_writes();
        for _ in 0..50 {
            if t.count(1, GIANTS_TECHNOLOGY_REPORT) >= 3 {
                break;
            }
            if t.count(1, GIANTS_EXPERIMENTAL_TOOL) == 0 {
                t.world.run_as_script(NAME, |ctx| {
                    ctx.give_items(Oid(1), GIANTS_EXPERIMENTAL_TOOL, 1);
                });
            }
            let mob = t.spawn(20157, 80);
            t.kill(1, mob);
            for traitor in t.npcs_of(CRUMA_MARSHLANDS_TRAITOR) {
                t.kill(1, traitor);
            }
        }
        assert_eq!(t.count(1, GIANTS_TECHNOLOGY_REPORT), 3);
        assert!(t.take_writes().contains(&update(NAME, "cond", "4")));

        // Rouke takes the items and sets "talk"; 31418-08 gives his report (cond 5).
        assert!(t.quest_page(1, rouke).contains("31418-06.html"));
        assert_eq!(t.count(1, GIANTS_TECHNOLOGY_REPORT), 0);
        t.take_writes();
        assert!(t.quest_page(1, rouke).contains("31418-07.html"));
        assert!(t.event(1, NAME, "31418-08.html").contains("31418-08.html"));
        assert_eq!(t.count(1, ROUKES_REPOT), 1);

        // Glyvka takes the report, then pays: badge, adena and exp (below level 41).
        t.take_writes();
        assert!(t.quest_page(1, glyvka).contains("30067-08.html"));
        assert_eq!(t.count(1, ROUKES_REPOT), 0);
        assert!(t.quest_page(1, glyvka).contains("30067-09.html"));
        assert!(t.event(1, NAME, "30067-10.html").contains("30067-10.html"));
        assert_eq!(t.count(1, BADGE_TEMPLE_MISSIONARY), 1);
        assert_eq!(t.count(1, 57), 15_100);
        let writes = t.take_writes();
        assert!(writes.ends_with(&[delete(NAME, false), update(NAME, "<state>", "Completed")]));
    }
}
