//! Quest 125: The Name of Evil - 1 (Mushika, Karakawei and the three Kaimu on Primeval Isle).
// Java: dist/game/data/scripts/quests/Q00125_TheNameOfEvil1/Q00125_TheNameOfEvil1.java
//
// Pattern: a chain of NPCs with a spelling puzzle. The Kaimu pages send one letter per event; the
// letters are kept as quest variables ("T", "E", "P", "U") and "Memo" says the puzzle is solved.

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, QuestSound, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const MUSHIKA: i32 = 32114;
const KARAKAWEI: i32 = 32117;
const ULU_KAIMU: i32 = 32119;
const BALU_KAIMU: i32 = 32120;
const CHUTA_KAIMU: i32 = 32121;

// Items
const ORNITHOMIMUS_CLAW: i32 = 8779;
const DEINONYCHUS_BONE: i32 = 8780;
const EPITAPH_OF_WISDOM: i32 = 8781;
const GAZKH_FRAGMENT: i32 = 8782;

// Skills
const REPRESENTATION_ENTER_THE_SAILREN_NEST_QUEST_ID: i32 = 5089;

const MEETING_THE_ELROKI: &str = "Q00124_MeetingTheElroki";

/// Java `ORNITHOMIMUS` (npc id -> chance out of 1000).
fn ornithomimus(npc_id: i32) -> Option<i32> {
    match npc_id {
        22200 => Some(661),
        22201 => Some(330),
        22202 => Some(661),
        22219 => Some(327),
        22224 => Some(327),
        _ => None,
    }
}

/// Java `DEINONYCHUS` (npc id -> chance out of 1000).
fn deinonychus(npc_id: i32) -> Option<i32> {
    match npc_id {
        22203 => Some(651),
        22204 => Some(326),
        22205 => Some(651),
        22220 => Some(319),
        22225 => Some(319),
        _ => None,
    }
}

/// Java `Q00125_TheNameOfEvil1`.
pub struct Q00125TheNameOfEvil1;

impl Script for Q00125TheNameOfEvil1 {
    fn name(&self) -> &'static str {
        "Q00125_TheNameOfEvil1"
    }

    fn path(&self) -> &'static str {
        "quests/Q00125_TheNameOfEvil1"
    }

    fn id(&self) -> i32 {
        125
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[MUSHIKA]);
        s.add_talk_id(&[MUSHIKA, KARAKAWEI, ULU_KAIMU, BALU_KAIMU, CHUTA_KAIMU]);
        s.add_kill_id(&[22200, 22201, 22202, 22219, 22224]);
        s.add_kill_id(&[22203, 22204, 22205, 22220, 22225]);
        s.register_quest_items(&[
            ORNITHOMIMUS_CLAW,
            DEINONYCHUS_BONE,
            EPITAPH_OF_WISDOM,
            GAZKH_FRAGMENT,
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

        let mut htmltext = event;
        match event {
            "32114-05.html" => qs.start_quest(ctx),
            "32114-08.html" => {
                if qs.is_cond(ctx, 1) {
                    ctx.give_items(player, GAZKH_FRAGMENT, 1);
                    qs.set_cond(ctx, 2, true);
                }
            }
            "32117-09.html" => {
                if qs.is_cond(ctx, 2) {
                    qs.set_cond(ctx, 3, true);
                }
            }
            "32117-15.html" => {
                if qs.is_cond(ctx, 4) {
                    qs.set_cond(ctx, 5, true);
                }
            }
            "T_One" => {
                qs.set(ctx, "T", "1");
                htmltext = "32119-04.html";
            }
            "E_One" => {
                qs.set(ctx, "E", "1");
                htmltext = "32119-05.html";
            }
            "P_One" => {
                qs.set(ctx, "P", "1");
                htmltext = "32119-06.html";
            }
            "U_One" => {
                qs.set(ctx, "U", "1");
                if qs.is_cond(ctx, 5)
                    && qs.get_int(ctx, "T") > 0
                    && qs.get_int(ctx, "E") > 0
                    && qs.get_int(ctx, "P") > 0
                    && qs.get_int(ctx, "U") > 0
                {
                    htmltext = "32119-08.html";
                    qs.set(ctx, "Memo", "1");
                } else {
                    htmltext = "32119-07.html";
                }

                for letter in ["T", "E", "P", "U"] {
                    qs.unset(ctx, letter);
                }
            }
            "32119-07.html" => {
                for letter in ["T", "E", "P", "U"] {
                    qs.unset(ctx, letter);
                }
            }
            "32119-18.html" => {
                if qs.is_cond(ctx, 5) {
                    qs.set_cond(ctx, 6, true);
                    qs.unset(ctx, "Memo");
                }
            }
            "T_Two" => {
                qs.set(ctx, "T", "1");
                htmltext = "32120-04.html";
            }
            "O_Two" => {
                qs.set(ctx, "O", "1");
                htmltext = "32120-05.html";
            }
            "O2_Two" => {
                qs.set(ctx, "O2", "1");
                htmltext = "32120-06.html";
            }
            "N_Two" => {
                qs.set(ctx, "N", "1");
                if qs.is_cond(ctx, 6)
                    && qs.get_int(ctx, "T") > 0
                    && qs.get_int(ctx, "O") > 0
                    && qs.get_int(ctx, "O2") > 0
                    && qs.get_int(ctx, "N") > 0
                {
                    htmltext = "32120-08.html";
                    qs.set(ctx, "Memo", "1");
                } else {
                    htmltext = "32120-07.html";
                }

                for letter in ["T", "O", "O2", "N"] {
                    qs.unset(ctx, letter);
                }
            }
            "32120-07.html" => {
                for letter in ["T", "O", "O2", "N"] {
                    qs.unset(ctx, letter);
                }
            }
            "32120-17.html" => {
                if qs.is_cond(ctx, 6) {
                    qs.set_cond(ctx, 7, true);
                    qs.unset(ctx, "Memo");
                }
            }
            "W_Three" => {
                qs.set(ctx, "W", "1");
                htmltext = "32121-04.html";
            }
            "A_Three" => {
                qs.set(ctx, "A", "1");
                htmltext = "32121-05.html";
            }
            "G_Three" => {
                qs.set(ctx, "G", "1");
                htmltext = "32121-06.html";
            }
            "U_Three" => {
                qs.set(ctx, "U", "1");
                if qs.is_cond(ctx, 7)
                    && qs.get_int(ctx, "W") > 0
                    && qs.get_int(ctx, "A") > 0
                    && qs.get_int(ctx, "G") > 0
                    && qs.get_int(ctx, "U") > 0
                {
                    htmltext = "32121-08.html";
                    qs.set(ctx, "Memo", "1");
                } else {
                    htmltext = "32121-07.html";
                }

                for letter in ["W", "A", "G", "U"] {
                    qs.unset(ctx, letter);
                }
            }
            "32121-07.html" => {
                for letter in ["W", "A", "G", "U"] {
                    qs.unset(ctx, letter);
                }
            }
            "32121-11.html" => qs.set(ctx, "Memo", "2"),
            "32121-16.html" => qs.set(ctx, "Memo", "3"),
            "32121-18.html" => {
                if qs.is_cond(ctx, 7) && ctx.has_quest_items(player, GAZKH_FRAGMENT) {
                    ctx.give_items(player, EPITAPH_OF_WISDOM, 1);
                    ctx.take_items(player, GAZKH_FRAGMENT, -1);
                    qs.set_cond(ctx, 8, true);
                    qs.unset(ctx, "Memo");
                }
            }
            _ => {}
        }

        Some(htmltext.to_owned())
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId, _is_summon: bool) {
        let Some(party_member) = ctx.get_random_party_member_cond(player, 3) else {
            return;
        };

        let qs = ctx.get_quest_state(party_member, false);
        // NOTE(java-quirk): the items are counted and given to `player` (the killer), not to the
        // party member that was drawn.
        if let Some(chance) = ornithomimus(npc.id) {
            if ctx.get_quest_items_count(player, ORNITHOMIMUS_CLAW) < 2 && ctx.get_random(1000) < chance {
                ctx.give_items(player, ORNITHOMIMUS_CLAW, 1);
                ctx.play_sound(player, QuestSound::ItemsoundQuestItemget);
            }
        } else if let Some(chance) = deinonychus(npc.id)
            && ctx.get_quest_items_count(player, DEINONYCHUS_BONE) < 2
            && ctx.get_random(1000) < chance
        {
            ctx.give_items(player, DEINONYCHUS_BONE, 1);
            ctx.play_sound(player, QuestSound::ItemsoundQuestItemget);
        }

        if ctx.get_quest_items_count(player, ORNITHOMIMUS_CLAW) == 2
            && ctx.get_quest_items_count(player, DEINONYCHUS_BONE) == 2
            && let Some(qs) = qs
        {
            qs.set_cond(ctx, 4, true);
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let mut htmltext = ctx.get_no_quest_msg(player);
        let qs = ctx.get_quest_state(player, true)?;

        match npc.id {
            MUSHIKA => match qs.state(ctx) {
                CREATED => {
                    if ctx.player_level(player) < 76 {
                        htmltext = "32114-01a.htm".to_owned();
                    } else {
                        let done = ctx
                            .player_quest_state(player, MEETING_THE_ELROKI)
                            .is_some_and(|q| q.is_completed());
                        htmltext = if done { "32114-01.htm" } else { "32114-01b.htm" }.to_owned();
                    }
                }
                STARTED => match qs.get_cond(ctx) {
                    1 => htmltext = "32114-09.html".to_owned(),
                    2 => htmltext = "32114-10.html".to_owned(),
                    3..=7 => htmltext = "32114-11.html".to_owned(),
                    8 => {
                        if ctx.has_quest_items(player, EPITAPH_OF_WISDOM) {
                            htmltext = "32114-12.html".to_owned();
                            ctx.add_exp_and_sp(player, 859_195, 86_603);
                            qs.exit_quest(ctx, false, true);
                        }
                    }
                    _ => {}
                },
                COMPLETED => htmltext = ctx.get_already_completed_msg(player),
                _ => {}
            },
            KARAKAWEI => {
                if qs.is_started(ctx) {
                    match qs.get_cond(ctx) {
                        1 => htmltext = "32117-01.html".to_owned(),
                        2 => htmltext = "32117-02.html".to_owned(),
                        3 => htmltext = "32117-10.html".to_owned(),
                        4 => {
                            if ctx.get_quest_items_count(player, ORNITHOMIMUS_CLAW) >= 2
                                && ctx.get_quest_items_count(player, DEINONYCHUS_BONE) >= 2
                            {
                                ctx.take_items(player, ORNITHOMIMUS_CLAW, -1);
                                ctx.take_items(player, DEINONYCHUS_BONE, -1);
                                htmltext = "32117-11.html".to_owned();
                            }
                        }
                        5 => htmltext = "32117-16.html".to_owned(),
                        6 | 7 => htmltext = "32117-17.html".to_owned(),
                        8 => htmltext = "32117-18.html".to_owned(),
                        _ => {}
                    }
                }
            }
            ULU_KAIMU => {
                if qs.is_started(ctx) {
                    match qs.get_cond(ctx) {
                        1..=4 => htmltext = "32119-01.html".to_owned(),
                        5 => {
                            if qs.get(ctx, "Memo").is_none() {
                                kaimu_sign(ctx, npc, player);
                                for letter in ["T", "E", "P", "U"] {
                                    qs.unset(ctx, letter);
                                }
                                htmltext = "32119-02.html".to_owned();
                            } else {
                                htmltext = "32119-09.html".to_owned();
                            }
                        }
                        6 => htmltext = "32119-18.html".to_owned(),
                        _ => htmltext = "32119-19.html".to_owned(),
                    }
                }
            }
            BALU_KAIMU => {
                if qs.is_started(ctx) {
                    match qs.get_cond(ctx) {
                        1..=5 => htmltext = "32120-01.html".to_owned(),
                        6 => {
                            if qs.get(ctx, "Memo").is_none() {
                                kaimu_sign(ctx, npc, player);
                                for letter in ["T", "O", "O2", "N"] {
                                    qs.unset(ctx, letter);
                                }
                                htmltext = "32120-02.html".to_owned();
                            } else {
                                htmltext = "32120-09.html".to_owned();
                            }
                        }
                        7 => htmltext = "32120-17.html".to_owned(),
                        // NOTE(java-quirk): Balu Kaimu answers with Ulu Kaimu's page 18 at cond 8.
                        _ => htmltext = "32119-18.html".to_owned(),
                    }
                }
            }
            CHUTA_KAIMU => {
                if qs.is_started(ctx) {
                    match qs.get_cond(ctx) {
                        1..=6 => htmltext = "32121-01.html".to_owned(),
                        7 => {
                            if qs.get(ctx, "Memo").is_none() {
                                kaimu_sign(ctx, npc, player);
                                for letter in ["W", "A", "G", "U"] {
                                    qs.unset(ctx, letter);
                                }
                                htmltext = "32121-02.html".to_owned();
                            } else {
                                match qs.get_int(ctx, "Memo") {
                                    1 => htmltext = "32121-09.html".to_owned(),
                                    2 => htmltext = "32121-19.html".to_owned(),
                                    3 => htmltext = "32121-20.html".to_owned(),
                                    _ => {}
                                }
                            }
                        }
                        8 => htmltext = "32121-21.html".to_owned(),
                        _ => {}
                    }
                }
            }
            _ => {}
        }
        Some(htmltext)
    }
}

/// Java `npc.broadcastPacket(new MagicSkillUse(npc, player, 5089, 1, 1000, 0))`.
fn kaimu_sign(ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) {
    ctx.broadcast_magic_skill_use(
        npc.object_id,
        npc.object_id,
        player,
        REPRESENTATION_ENTER_THE_SAILREN_NEST_QUEST_ID,
        1,
        1000,
        0,
    );
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::rows::{delete, insert, update};
    use crate::testing::{Pages, ScriptTest};

    const NPCS: &str = r#"
        <npc id="32114" type="Folk" level="75" name="Mushika"/>
        <npc id="32117" type="Folk" level="75" name="Karakawei"/>
        <npc id="32119" type="Folk" level="75" name="Ulu Kaimu"/>
        <npc id="32120" type="Folk" level="75" name="Balu Kaimu"/>
        <npc id="32121" type="Folk" level="75" name="Chuta Kaimu"/>
        <npc id="22200" type="Monster" level="75" name="Ornithomimus"/>
        <npc id="22203" type="Monster" level="75" name="Deinonychus"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="8779" type="EtcItem" name="Claw"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="8780" type="EtcItem" name="Bone"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="8781" type="EtcItem" name="Epitaph"><set name="is_questitem" val="true"/></item>
        <item id="8782" type="EtcItem" name="Fragment"><set name="is_questitem" val="true"/></item>"#;

    const NAME: &str = "Q00125_TheNameOfEvil1";

    struct Npcs {
        mushika: Oid,
        karakawei: Oid,
        ulu: Oid,
        balu: Oid,
        chuta: Oid,
    }

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

    fn setup() -> (ScriptTest, Npcs) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00125TheNameOfEvil1);
        // The stand-in for quest 124, so Mushika can look at its state.
        registry.add(StandIn(MEETING_THE_ELROKI));
        let pages = Pages::of(&Q00125TheNameOfEvil1)
            .page("32114-01.htm", &["32114-05.html"])
            .plain(&["32114-01a.htm", "32114-01b.htm", "32114-09.html", "32114-10.html"])
            .page("32114-05.html", &["32114-08.html"])
            .plain(&["32114-08.html", "32114-11.html", "32114-12.html"])
            .page("32117-02.html", &["32117-09.html"])
            .plain(&["32117-09.html", "32117-10.html", "32117-15.html", "32117-16.html"])
            .page("32117-11.html", &["32117-15.html"])
            .page("32119-02.html", &["T_One", "E_One", "P_One", "U_One"])
            .plain(&["32119-04.html", "32119-05.html", "32119-06.html", "32119-07.html"])
            .plain(&["32119-08.html", "32119-18.html"])
            .page("32119-09.html", &["32119-18.html"])
            .npcs(&[MUSHIKA, KARAKAWEI, ULU_KAIMU, BALU_KAIMU, CHUTA_KAIMU])
            .file("data/html/noquest.htm", "<html><body>noquest</body></html>");
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let npcs = Npcs {
            mushika: t.spawn(MUSHIKA, 20),
            karakawei: t.spawn(KARAKAWEI, 30),
            ulu: t.spawn(ULU_KAIMU, 40),
            balu: t.spawn(BALU_KAIMU, 50),
            chuta: t.spawn(CHUTA_KAIMU, 60),
        };
        (t, npcs)
    }

    fn call(t: &mut ScriptTest, event: &str) -> Option<String> {
        let mut out = None;
        t.world.run_as_script(NAME, |ctx| {
            out = Q00125TheNameOfEvil1.on_event(ctx, event, None, Some(Oid(1)));
        });
        out
    }

    #[test]
    fn level_and_quest_124_gate_the_start() {
        let (mut t, npcs) = setup();
        t.enter(1, 75, vec![]);
        assert!(t.quest_page(1, npcs.mushika).contains("32114-01a.htm"));
        t.enter(2, 76, vec![]);
        // Quest 124 is not completed.
        assert!(t.quest_page(2, npcs.mushika).contains("32114-01b.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_first_steps_and_the_kill_drops() {
        let (mut t, npcs) = setup();
        t.enter(1, 76, vec![]);
        t.world.run_as_script(MEETING_THE_ELROKI, |ctx| {
            let q = ctx.get_quest_state(Oid(1), true).unwrap();
            q.start_quest(ctx);
            q.exit_quest(ctx, false, false);
        });
        t.take_writes();
        let page = t.quest_page(1, npcs.mushika);
        assert!(page.contains("32114-01.htm"), "{page}");
        assert!(t.event(1, NAME, "32114-05.html").contains("32114-05.html"));
        assert_eq!(
            t.take_writes(),
            [insert(NAME, "cond", "1"), insert(NAME, "<state>", "Started")]
        );
        assert!(t.event(1, NAME, "32114-08.html").contains("32114-08.html"));
        assert_eq!(t.count(1, GAZKH_FRAGMENT), 1);
        assert_eq!(t.take_writes(), [update(NAME, "cond", "2")]);
        assert!(t.quest_page(1, npcs.karakawei).contains("32117-02.html"));
        assert!(t.event(1, NAME, "32117-09.html").contains("32117-09.html"));
        assert_eq!(t.take_writes(), [update(NAME, "cond", "3")]);

        // Two claws and two bones set cond 4.
        for _ in 0..200 {
            if t.count(1, ORNITHOMIMUS_CLAW) == 2 {
                break;
            }
            let mob = t.spawn(22200, 80);
            t.kill(1, mob);
        }
        for _ in 0..200 {
            if t.count(1, DEINONYCHUS_BONE) == 2 {
                break;
            }
            let mob = t.spawn(22203, 80);
            t.kill(1, mob);
        }
        assert_eq!(t.count(1, ORNITHOMIMUS_CLAW), 2);
        assert_eq!(t.count(1, DEINONYCHUS_BONE), 2);
        assert!(t.take_writes().contains(&update(NAME, "cond", "4")));
        assert!(t.quest_page(1, npcs.karakawei).contains("32117-11.html"));
        assert_eq!(t.count(1, ORNITHOMIMUS_CLAW), 0);
        assert!(t.event(1, NAME, "32117-15.html").contains("32117-15.html"));
        assert_eq!(t.take_writes(), [update(NAME, "cond", "5")]);
    }

    #[test]
    fn the_ulu_kaimu_puzzle_and_the_reward() {
        let (mut t, npcs) = setup();
        t.enter(1, 76, vec![]);
        t.world.run_as_script(NAME, |ctx| {
            let q = ctx.get_quest_state(Oid(1), true).unwrap();
            q.start_quest(ctx);
            q.set_cond(ctx, 5, false);
        });
        t.take_writes();
        // The four letters in the wrong order of events still need all four set.
        assert!(t.quest_page(1, npcs.ulu).contains("32119-02.html"));
        assert_eq!(call(&mut t, "T_One").as_deref(), Some("32119-04.html"));
        assert_eq!(call(&mut t, "E_One").as_deref(), Some("32119-05.html"));
        assert_eq!(call(&mut t, "U_One").as_deref(), Some("32119-07.html"));
        t.take_writes();
        for e in ["T_One", "E_One", "P_One"] {
            call(&mut t, e);
        }
        assert_eq!(call(&mut t, "U_One").as_deref(), Some("32119-08.html"));
        t.take_writes();
        assert!(t.quest_page(1, npcs.ulu).contains("32119-09.html"));
        assert!(t.event(1, NAME, "32119-18.html").contains("32119-18.html"));
        assert!(t.take_writes().contains(&update(NAME, "cond", "6")));

        // Cond 8 with the epitaph pays Mushika's reward.
        t.world.run_as_script(NAME, |ctx| {
            let q = ctx.get_quest_state(Oid(1), false).unwrap();
            ctx.give_items(Oid(1), EPITAPH_OF_WISDOM, 1);
            q.set_cond(ctx, 8, false);
        });
        t.take_writes();
        assert!(t.quest_page(1, npcs.mushika).contains("32114-12.html"));
        assert_eq!(
            t.take_writes(),
            [delete(NAME, false), update(NAME, "<state>", "Completed")]
        );
        let _ = (npcs.balu, npcs.chuta);
    }
}
