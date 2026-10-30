//! Quest 265: Bonds of Slavery (Kristin in the Dark Elven area buys the imp shackles).
// Java: dist/game/data/scripts/quests/Q00265_BondsOfSlavery/Q00265_BondsOfSlavery.java
//
// Pattern: a repeatable collect quest (`docs/design/scripts.md`, "Quest patterns"): imps drop
// shackles on a roll of ten; Kristin pays per shackle, with a bonus from 10 shackles, takes them
// all and gives a beginner's shot pack once (level below 25, one-time flag 57). The Newbie Guide's
// mission hears about it.

use l2_protocol_game::NpcStringId;
use l2_util::ObjectId;
use l2_world::script::{CREATED, QuestSound, STARTED, race};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const KRISTIN: i32 = 30357;

// Monsters: the imps and the roll of ten they drop on (`getRandom(10) < n`).
fn shackle_chance(npc_id: i32) -> i32 {
    match npc_id {
        20004 => 5, // Imp
        20005 => 6, // Imp Elder
        _ => 0,
    }
}

// Items
const IMP_SHACKLES: i32 = 1368;
const SPIRITSHOTS_NO_GRADE_FOR_ROOKIES: (i32, i64) = (5790, 3000);
const SOULSHOTS_NO_GRADE_FOR_ROOKIES: (i32, i64) = (5789, 6000);

// Misc
const MIN_LEVEL: i32 = 6;
const GUIDE_MISSION: i32 = 41;
/// Java `NewbieGuide.class.getSimpleName()`.
const NEWBIE_GUIDE: &str = "NewbieGuide";

/// Java `Q00265_BondsOfSlavery`.
pub struct Q00265BondsOfSlavery;

impl Script for Q00265BondsOfSlavery {
    fn name(&self) -> &'static str {
        "Q00265_BondsOfSlavery"
    }

    fn path(&self) -> &'static str {
        "quests/Q00265_BondsOfSlavery"
    }

    fn id(&self) -> i32 {
        265
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[KRISTIN]);
        s.add_talk_id(&[KRISTIN]);
        s.add_kill_id(&[20004, 20005]);
        s.register_quest_items(&[IMP_SHACKLES]);
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
            "30357-04.htm" => {
                qs.start_quest(ctx);
                htmltext = Some(event.to_owned());
            }
            "30357-07.html" => {
                qs.exit_quest(ctx, true, true);
                htmltext = Some(event.to_owned());
            }
            "30357-08.html" => htmltext = Some(event.to_owned()),
            _ => {}
        }
        htmltext
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        let qs = ctx.get_quest_state(killer, false);
        if qs.is_some() && ctx.get_random(10) < shackle_chance(npc.id) {
            ctx.give_items(killer, IMP_SHACKLES, 1);
            ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        match qs.state(ctx) {
            CREATED => {
                htmltext = if ctx.player_race(player) == race::DARK_ELF {
                    if ctx.player_level(player) >= MIN_LEVEL {
                        "30357-03.htm"
                    } else {
                        "30357-02.html"
                    }
                } else {
                    "30357-01.html"
                }
                .to_owned();
            }
            STARTED => {
                if ctx.has_quest_items(player, IMP_SHACKLES) {
                    let shackles = ctx.get_quest_items_count(player, IMP_SHACKLES);
                    let bonus = if shackles >= 10 { 500 } else { 0 };
                    ctx.give_adena(player, shackles * 12 + bonus, true);
                    ctx.take_items(player, IMP_SHACKLES, -1);

                    if ctx.player_level(player) < 25 && ctx.get_one_time_quest_flag(player, 57) == 0 {
                        if ctx.is_mage_class(player) {
                            ctx.give_items(
                                player,
                                SPIRITSHOTS_NO_GRADE_FOR_ROOKIES.0,
                                SPIRITSHOTS_NO_GRADE_FOR_ROOKIES.1,
                            );
                            ctx.play_sound_named(player, "tutorial_voice_027");
                        } else {
                            ctx.give_items(
                                player,
                                SOULSHOTS_NO_GRADE_FOR_ROOKIES.0,
                                SOULSHOTS_NO_GRADE_FOR_ROOKIES.1,
                            );
                            ctx.play_sound_named(player, "tutorial_voice_026");
                        }
                        ctx.set_one_time_quest_flag(player, 57, 1);
                    }

                    // Newbie Guide: only when `ai/others/NewbieGuide` is installed.
                    if let Some(guide) = ctx.quest_state_of(NEWBIE_GUIDE, player, true) {
                        if !ctx.have_nr_memo(guide, GUIDE_MISSION) {
                            ctx.set_nr_memo(guide, GUIDE_MISSION);
                            ctx.set_nr_memo_state(guide, GUIDE_MISSION, 1000);
                            ctx.show_on_screen_npc_string(
                                player,
                                NpcStringId::AcquisitionOfSoulshotForBeginnersCompleteNGoFindTheNewbieGuide,
                                2,
                                5000,
                                &[],
                            );
                        } else if (ctx.get_nr_memo_state(guide, GUIDE_MISSION) % 10_000) / 1000 != 1 {
                            ctx.set_nr_memo(guide, GUIDE_MISSION);
                            let state = ctx.get_nr_memo_state(guide, GUIDE_MISSION);
                            ctx.set_nr_memo_state(guide, GUIDE_MISSION, state + 1000);
                            ctx.show_on_screen_npc_string(
                                player,
                                NpcStringId::AcquisitionOfSoulshotForBeginnersCompleteNGoFindTheNewbieGuide,
                                2,
                                5000,
                                &[],
                            );
                        }
                    }

                    htmltext = "30357-06.html".to_owned();
                } else {
                    htmltext = "30357-05.html".to_owned();
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
    use l2_world::script::{player_class, race};
    use l2_world::{Script, ScriptRegistry, ScriptSetup};

    use super::*;
    use crate::testing::{Pages, ScriptTest, rows};

    const NAME: &str = "Q00265_BondsOfSlavery";

    const NPCS: &str = r#"
        <npc id="30357" type="Folk" level="70" name="Kristin"/>
        <npc id="20004" type="Monster" level="2" name="Imp"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="1368" type="EtcItem" name="Imp Shackles"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="5789" type="EtcItem" name="Soulshot: No Grade"><set name="is_stackable" val="true"/></item>
        <item id="5790" type="EtcItem" name="Spiritshot: No Grade"><set name="is_stackable" val="true"/></item>"#;

    /// Java `ai.others.NewbieGuide` as far as this quest needs it: a script of that name.
    struct NewbieGuide;

    impl Script for NewbieGuide {
        fn name(&self) -> &'static str {
            "NewbieGuide"
        }
        fn path(&self) -> &'static str {
            "ai/others/NewbieGuide"
        }
        fn setup(&self, _setup: &mut ScriptSetup) {}
    }

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(NewbieGuide);
        registry.add(Q00265BondsOfSlavery);
        let pages = Pages::of(&Q00265BondsOfSlavery)
            .page("30357-03.htm", &["30357-04.htm"])
            .page("30357-05.html", &["30357-07.html", "30357-08.html"])
            .page("30357-06.html", &["30357-07.html", "30357-08.html"])
            .plain(&[
                "30357-01.html",
                "30357-02.html",
                "30357-04.htm",
                "30357-07.html",
                "30357-08.html",
            ])
            .npcs(&[KRISTIN]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let kristin = t.spawn(KRISTIN, 20);
        (t, kristin)
    }

    #[test]
    fn only_dark_elves_of_level_six_can_take_it() {
        let (mut t, kristin) = setup();
        t.enter_as(1, 10, race::HUMAN, player_class::FIGHTER, vec![]);
        assert!(t.quest_page(1, kristin).contains("30357-01.html"));
        t.enter_as(2, 5, race::DARK_ELF, player_class::DARK_FIGHTER, vec![]);
        assert!(t.quest_page(2, kristin).contains("30357-02.html"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn shackles_for_adena_then_the_one_time_shots_and_the_newbie_guide() {
        let (mut t, kristin) = setup();
        t.enter_as(1, 6, race::DARK_ELF, player_class::DARK_FIGHTER, vec![]);
        assert!(t.quest_page(1, kristin).contains("30357-03.htm"));
        assert!(t.event(1, NAME, "30357-04.htm").contains("30357-04.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );

        // No shackles yet.
        assert!(t.quest_page(1, kristin).contains("30357-05.html"));

        // Imps drop shackles on a roll of ten; no condition is set.
        for _ in 0..300 {
            if t.count(1, IMP_SHACKLES) >= 10 {
                break;
            }
            let imp = t.spawn(20004, 60);
            t.kill(1, imp);
        }
        let shackles = t.count(1, IMP_SHACKLES);
        assert!(shackles >= 10, "{shackles}");
        assert!(t.take_writes().is_empty());

        // 12 adena a shackle and 500 more from 10 on; soulshots once (soldiers, below level 25).
        assert!(t.quest_page(1, kristin).contains("30357-06.html"));
        assert_eq!(t.count(1, 57), shackles * 12 + 500);
        assert_eq!(t.count(1, IMP_SHACKLES), 0);
        assert_eq!(t.count(1, 5789), 6000);
        assert_eq!(t.count(1, 5790), 0);
        let writes = t.take_writes();
        assert_eq!(
            writes[..2],
            [
                rows::insert(NEWBIE_GUIDE, "NRmemo", "41"),
                rows::insert(NEWBIE_GUIDE, "NRmemoState41", "1000"),
            ]
        );

        // The next talk gives no more shots: the one-time flag 57 is set.
        assert!(t.quest_page(1, kristin).contains("30357-05.html"));

        // `exitQuest(true, true)` from the last page.
        assert!(t.event(1, NAME, "30357-07.html").contains("30357-07.html"));
        assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
    }
}
