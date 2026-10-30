//! Quest 138: Temple Champion - 2 (Sylvain, Pupina, Angus and Sla; recommendations and 10 relics).
// Java: dist/game/data/scripts/quests/Q00138_TempleChampionPart2/Q00138_TempleChampionPart2.java
//
// Pattern: like Q00137, a chain of four NPCs that echoes every event (Java `return event`);
// the start page depends on Q00137 being completed.

use l2_util::ObjectId;
use l2_world::script::QuestSound;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const SYLVAIN: i32 = 30070;
const PUPINA: i32 = 30118;
const ANGUS: i32 = 30474;
const SLA: i32 = 30666;
const MOBS: [i32; 4] = [
    20176, // Wyrm
    20550, // Guardian Basilisk
    20551, // Road Scavenger
    20552, // Fettered Soul
];

// Items
const TEMPLE_MANIFESTO: i32 = 10341;
const RELICS_OF_THE_DARK_ELF_TRAINEE: i32 = 10342;
const ANGUS_RECOMMENDATION: i32 = 10343;
const PUPINAS_RECOMMENDATION: i32 = 10344;

const TEMPLE_CHAMPION_PART1: &str = "Q00137_TempleChampionPart1";

/// Java `Q00138_TempleChampionPart2`.
pub struct Q00138TempleChampionPart2;

impl Script for Q00138TempleChampionPart2 {
    fn name(&self) -> &'static str {
        "Q00138_TempleChampionPart2"
    }

    fn path(&self) -> &'static str {
        "quests/Q00138_TempleChampionPart2"
    }

    fn id(&self) -> i32 {
        138
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[SYLVAIN]);
        s.add_talk_id(&[SYLVAIN, PUPINA, ANGUS, SLA]);
        s.add_kill_id(&MOBS);
        s.register_quest_items(&[
            TEMPLE_MANIFESTO,
            RELICS_OF_THE_DARK_ELF_TRAINEE,
            ANGUS_RECOMMENDATION,
            PUPINAS_RECOMMENDATION,
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
        let Some(qs) = ctx.get_quest_state(player, false) else {
            return Some(ctx.get_no_quest_msg(player));
        };

        match event {
            "30070-02.htm" => {
                qs.start_quest(ctx);
                ctx.give_items(player, TEMPLE_MANIFESTO, 1);
            }
            "30070-05.html" => {
                ctx.give_adena(player, 84_593, true);
                if ctx.player_level(player) < 42 {
                    ctx.add_exp_and_sp(player, 187_062, 11_307);
                }

                qs.exit_quest(ctx, false, true);
            }
            "30070-03.html" => qs.set_cond(ctx, 2, true),
            "30118-06.html" => qs.set_cond(ctx, 3, true),
            "30118-09.html" => {
                qs.set_cond(ctx, 6, true);
                ctx.give_items(player, PUPINAS_RECOMMENDATION, 1);
            }
            "30474-02.html" => qs.set_cond(ctx, 4, true),
            "30666-02.html" => {
                if ctx.has_quest_items(player, PUPINAS_RECOMMENDATION) {
                    qs.set(ctx, "talk", "1");
                    ctx.take_items(player, PUPINAS_RECOMMENDATION, -1);
                }
            }
            "30666-03.html" => {
                if ctx.has_quest_items(player, TEMPLE_MANIFESTO) {
                    qs.set(ctx, "talk", "2");
                    ctx.take_items(player, TEMPLE_MANIFESTO, -1);
                }
            }
            "30666-08.html" => {
                qs.set_cond(ctx, 7, true);
                qs.unset(ctx, "talk");
            }
            _ => {}
        }

        Some(event.to_owned())
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId, _is_summon: bool) {
        let Some(qs) = ctx.get_quest_state(player, false) else {
            return;
        };
        if qs.is_started(ctx)
            && qs.is_cond(ctx, 4)
            && ctx.get_quest_items_count(player, RELICS_OF_THE_DARK_ELF_TRAINEE) < 10
        {
            ctx.give_items(player, RELICS_OF_THE_DARK_ELF_TRAINEE, 1);
            if ctx.get_quest_items_count(player, RELICS_OF_THE_DARK_ELF_TRAINEE) >= 10 {
                ctx.play_sound(player, QuestSound::ItemsoundQuestMiddle);
            } else {
                ctx.play_sound(player, QuestSound::ItemsoundQuestItemget);
            }
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        match npc.id {
            SYLVAIN => match qs.get_cond(ctx) {
                // NOTE(java-quirk): cond 1 answers with the start page `30070-02.htm`.
                1 => htmltext = "30070-02.htm".to_owned(),
                2..=6 => htmltext = "30070-03.html".to_owned(),
                7 => htmltext = "30070-04.html".to_owned(),
                _ => {
                    if qs.is_completed(ctx) {
                        return Some(ctx.get_already_completed_msg(player));
                    }

                    let done = ctx
                        .player_quest_state(player, TEMPLE_CHAMPION_PART1)
                        .is_some_and(|q| q.is_completed());
                    htmltext = if ctx.player_level(player) >= 36 {
                        if done { "30070-01.htm" } else { "30070-00a.htm" }
                    } else {
                        "30070-00.htm"
                    }
                    .to_owned();
                }
            },
            PUPINA => match qs.get_cond(ctx) {
                2 => htmltext = "30118-01.html".to_owned(),
                3 | 4 => htmltext = "30118-07.html".to_owned(),
                5 => {
                    htmltext = "30118-08.html".to_owned();
                    if ctx.has_quest_items(player, ANGUS_RECOMMENDATION) {
                        ctx.take_items(player, ANGUS_RECOMMENDATION, -1);
                    }
                }
                6 => htmltext = "30118-10.html".to_owned(),
                _ => {}
            },
            ANGUS => match qs.get_cond(ctx) {
                3 => htmltext = "30474-01.html".to_owned(),
                4 => {
                    if ctx.get_quest_items_count(player, RELICS_OF_THE_DARK_ELF_TRAINEE) >= 10 {
                        ctx.take_items(player, RELICS_OF_THE_DARK_ELF_TRAINEE, -1);
                        ctx.give_items(player, ANGUS_RECOMMENDATION, 1);
                        qs.set_cond(ctx, 5, true);
                        htmltext = "30474-04.html".to_owned();
                    } else {
                        htmltext = "30474-03.html".to_owned();
                    }
                }
                5 => htmltext = "30474-05.html".to_owned(),
                _ => {}
            },
            SLA => match qs.get_cond(ctx) {
                6 => match qs.get_int(ctx, "talk") {
                    1 => htmltext = "30666-02.html".to_owned(),
                    2 => htmltext = "30666-03.html".to_owned(),
                    _ => htmltext = "30666-01.html".to_owned(),
                },
                7 => htmltext = "30666-09.html".to_owned(),
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

    use super::*;
    use crate::testing::rows::{delete, insert, update};
    use crate::testing::{Pages, ScriptTest};

    const NPCS: &str = r#"
        <npc id="30070" type="Folk" level="40" name="Sylvain"/>
        <npc id="30118" type="Folk" level="40" name="Pupina"/>
        <npc id="30474" type="Folk" level="40" name="Angus"/>
        <npc id="30666" type="Folk" level="40" name="Sla"/>
        <npc id="20176" type="Monster" level="30" name="Wyrm"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="10341" type="EtcItem" name="Manifesto"><set name="is_questitem" val="true"/></item>
        <item id="10342" type="EtcItem" name="Relics"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="10343" type="EtcItem" name="Angus Recommendation"><set name="is_questitem" val="true"/></item>
        <item id="10344" type="EtcItem" name="Pupina Recommendation"><set name="is_questitem" val="true"/></item>"#;

    const NAME: &str = "Q00138_TempleChampionPart2";

    struct StandIn(&'static str);

    impl Script for StandIn {
        fn name(&self) -> &'static str {
            self.0
        }
        fn path(&self) -> &'static str {
            "quests"
        }
        fn setup(&self, _setup: &mut ScriptSetup) {}
    }

    struct Npcs {
        sylvain: Oid,
        pupina: Oid,
        angus: Oid,
        sla: Oid,
    }

    fn setup() -> (ScriptTest, Npcs) {
        let mut registry = ScriptRegistry::new();
        registry.add(StandIn(TEMPLE_CHAMPION_PART1));
        registry.add(Q00138TempleChampionPart2);
        let pages = Pages::of(&Q00138TempleChampionPart2)
            .page("30070-01.htm", &["30070-02.htm"])
            .page("30070-02.htm", &["30070-03.html"])
            .plain(&["30070-00.htm", "30070-00a.htm", "30070-03.html"])
            .page("30070-04.html", &["30070-05.html"])
            .plain(&["30070-05.html"])
            .page("30118-01.html", &["30118-06.html"])
            .plain(&["30118-06.html", "30118-07.html"])
            .page("30118-08.html", &["30118-09.html"])
            .plain(&["30118-09.html", "30118-10.html"])
            .page("30474-01.html", &["30474-02.html"])
            .plain(&["30474-02.html", "30474-03.html", "30474-04.html", "30474-05.html"])
            .page("30666-01.html", &["30666-02.html"])
            .page("30666-02.html", &["30666-03.html"])
            .page("30666-03.html", &["30666-08.html"])
            .plain(&["30666-08.html", "30666-09.html"])
            .npcs(&[SYLVAIN, PUPINA, ANGUS, SLA])
            .file("data/html/noquest.htm", "<html><body>noquest</body></html>");
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let npcs = Npcs {
            sylvain: t.spawn(SYLVAIN, 20),
            pupina: t.spawn(PUPINA, 30),
            angus: t.spawn(ANGUS, 40),
            sla: t.spawn(SLA, 50),
        };
        (t, npcs)
    }

    fn finish_part1(t: &mut ScriptTest, player: i32) {
        t.world.run_as_script(TEMPLE_CHAMPION_PART1, |ctx| {
            let q = ctx.get_quest_state(Oid(player), true).unwrap();
            q.start_quest(ctx);
            q.exit_quest(ctx, false, false);
        });
        t.take_writes();
    }

    #[test]
    fn level_36_and_quest_137_gate_the_start() {
        let (mut t, npcs) = setup();
        t.enter(1, 35, vec![]);
        assert!(t.quest_page(1, npcs.sylvain).contains("30070-00.htm"));
        t.enter(2, 36, vec![]);
        assert!(t.quest_page(2, npcs.sylvain).contains("30070-00a.htm"));
        finish_part1(&mut t, 2);
        assert!(t.quest_page(2, npcs.sylvain).contains("30070-01.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_whole_chain_to_the_adena_reward() {
        let (mut t, npcs) = setup();
        t.enter(1, 40, vec![]);
        finish_part1(&mut t, 1);
        assert!(t.quest_page(1, npcs.sylvain).contains("30070-01.htm"));
        assert!(t.event(1, NAME, "30070-02.htm").contains("30070-02.htm"));
        assert_eq!(t.count(1, TEMPLE_MANIFESTO), 1);
        assert_eq!(
            t.take_writes(),
            [insert(NAME, "cond", "1"), insert(NAME, "<state>", "Started")]
        );
        assert!(t.event(1, NAME, "30070-03.html").contains("30070-03.html"));
        assert_eq!(t.take_writes(), [update(NAME, "cond", "2")]);

        // Pupina: cond 3; Angus: cond 4; ten relics; Angus: cond 5.
        assert!(t.quest_page(1, npcs.pupina).contains("30118-01.html"));
        assert!(t.event(1, NAME, "30118-06.html").contains("30118-06.html"));
        assert!(t.quest_page(1, npcs.angus).contains("30474-01.html"));
        assert!(t.event(1, NAME, "30474-02.html").contains("30474-02.html"));
        t.take_writes();
        for _ in 0..10 {
            let wyrm = t.spawn(20176, 80);
            t.kill(1, wyrm);
        }
        assert_eq!(t.count(1, RELICS_OF_THE_DARK_ELF_TRAINEE), 10);
        assert!(t.quest_page(1, npcs.angus).contains("30474-04.html"));
        assert_eq!(t.count(1, ANGUS_RECOMMENDATION), 1);
        assert_eq!(t.take_writes().last(), Some(&update(NAME, "cond", "5")));

        // Pupina takes it and gives hers: cond 6.
        assert!(t.quest_page(1, npcs.pupina).contains("30118-08.html"));
        assert_eq!(t.count(1, ANGUS_RECOMMENDATION), 0);
        assert!(t.event(1, NAME, "30118-09.html").contains("30118-09.html"));
        assert_eq!(t.count(1, PUPINAS_RECOMMENDATION), 1);
        t.take_writes();

        // Sla: the recommendation, then the manifesto, then cond 7.
        assert!(t.quest_page(1, npcs.sla).contains("30666-01.html"));
        assert!(t.event(1, NAME, "30666-02.html").contains("30666-02.html"));
        assert_eq!(t.count(1, PUPINAS_RECOMMENDATION), 0);
        assert!(t.event(1, NAME, "30666-03.html").contains("30666-03.html"));
        assert_eq!(t.count(1, TEMPLE_MANIFESTO), 0);
        assert!(t.event(1, NAME, "30666-08.html").contains("30666-08.html"));
        assert!(t.take_writes().contains(&update(NAME, "cond", "7")));

        // Sylvain pays.
        assert!(t.quest_page(1, npcs.sylvain).contains("30070-04.html"));
        assert!(t.event(1, NAME, "30070-05.html").contains("30070-05.html"));
        assert_eq!(t.count(1, 57), 84_593);
        assert_eq!(
            t.take_writes(),
            [delete(NAME, false), update(NAME, "<state>", "Completed")]
        );
    }
}
