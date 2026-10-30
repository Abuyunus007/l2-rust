//! Quest 273: Invaders of the Holy Land (Varkees in the Orc Village buys the soulstones of the rakeclaw imps).
// Java: dist/game/data/scripts/quests/Q00273_InvadersOfTheHolyLand/Q00273_InvadersOfTheHolyLand.java
//
// Pattern: a repeatable collect quest (`docs/design/scripts.md`, "Quest patterns"): every rakeclaw
// imp drops a black soulstone or, on the other end of a roll of 100, a red one; Varkees pays per
// stone (red ones more), with a bonus from 10 stones, and gives a beginner's soulshot pack once
// (level below 25, one-time flag 57). The Newbie Guide's mission hears about it.

use l2_protocol_game::NpcStringId;
use l2_util::ObjectId;
use l2_world::script::{CREATED, QuestSound, STARTED, race};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const VARKEES: i32 = 30566;

// Monsters: the imps and their black soulstone threshold (`getRandom(100) <= n` gives black).
fn black_chance(npc_id: i32) -> i32 {
    match npc_id {
        20311 => 90, // Rakeclaw Imp
        20312 => 87, // Rakeclaw Imp Hunter
        20313 => 77, // Rakeclaw Imp Chieftain
        _ => 0,
    }
}

// Items
const BLACK_SOULSTONE: i32 = 1475;
const RED_SOULSTONE: i32 = 1476;
const SOULSHOTS_NO_GRADE_FOR_ROOKIES: (i32, i64) = (5789, 6000);

// Misc
const MIN_LEVEL: i32 = 6;
const GUIDE_MISSION: i32 = 41;
/// Java `NewbieGuide.class.getSimpleName()`.
const NEWBIE_GUIDE: &str = "NewbieGuide";

/// Java `Q00273_InvadersOfTheHolyLand`.
pub struct Q00273InvadersOfTheHolyLand;

impl Script for Q00273InvadersOfTheHolyLand {
    fn name(&self) -> &'static str {
        "Q00273_InvadersOfTheHolyLand"
    }

    fn path(&self) -> &'static str {
        "quests/Q00273_InvadersOfTheHolyLand"
    }

    fn id(&self) -> i32 {
        273
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[VARKEES]);
        s.add_talk_id(&[VARKEES]);
        s.add_kill_id(&[20311, 20312, 20313]);
        s.register_quest_items(&[BLACK_SOULSTONE, RED_SOULSTONE]);
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
            "30566-04.htm" => {
                qs.start_quest(ctx);
                htmltext = Some(event.to_owned());
            }
            "30566-08.html" => {
                qs.exit_quest(ctx, true, true);
                htmltext = Some(event.to_owned());
            }
            "30566-09.html" => htmltext = Some(event.to_owned()),
            _ => {}
        }
        htmltext
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        if ctx.get_quest_state(killer, false).is_some() {
            if ctx.get_random(100) <= black_chance(npc.id) {
                ctx.give_items(killer, BLACK_SOULSTONE, 1);
            } else {
                ctx.give_items(killer, RED_SOULSTONE, 1);
            }
            ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        match qs.state(ctx) {
            CREATED => {
                htmltext = if ctx.player_race(player) == race::ORC {
                    if ctx.player_level(player) >= MIN_LEVEL {
                        "30566-03.htm"
                    } else {
                        "30566-02.htm"
                    }
                } else {
                    "30566-01.htm"
                }
                .to_owned();
            }
            STARTED => {
                let stones = [BLACK_SOULSTONE, RED_SOULSTONE];
                if ctx.has_at_least_one_quest_item(player, &stones) {
                    let black = ctx.get_quest_items_count(player, BLACK_SOULSTONE);
                    let red = ctx.get_quest_items_count(player, RED_SOULSTONE);
                    let bonus = if red > 0 {
                        if red + black >= 10 { 1800 } else { 0 }
                    } else if black >= 10 {
                        1500
                    } else {
                        0
                    };
                    ctx.give_adena(player, red * 10 + black * 3 + bonus, true);
                    ctx.take_items_many(player, -1, &stones);

                    if ctx.player_level(player) < 25 && ctx.get_one_time_quest_flag(player, 57) == 0 {
                        ctx.give_items(
                            player,
                            SOULSHOTS_NO_GRADE_FOR_ROOKIES.0,
                            SOULSHOTS_NO_GRADE_FOR_ROOKIES.1,
                        );
                        ctx.play_sound_named(player, "tutorial_voice_026");
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

                    htmltext = if red > 0 { "30566-07.html" } else { "30566-06.html" }.to_owned();
                } else {
                    htmltext = "30566-05.html".to_owned();
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

    const NAME: &str = "Q00273_InvadersOfTheHolyLand";

    const NPCS: &str = r#"
        <npc id="30566" type="Folk" level="70" name="Varkees"/>
        <npc id="20311" type="Monster" level="10" name="Rakeclaw Imp"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="1475" type="EtcItem" name="Black Soulstone"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="1476" type="EtcItem" name="Red Soulstone"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="5789" type="EtcItem" name="Soulshot: No Grade"><set name="is_stackable" val="true"/></item>"#;

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
        registry.add(Q00273InvadersOfTheHolyLand);
        let pages = Pages::of(&Q00273InvadersOfTheHolyLand)
            .page("30566-03.htm", &["30566-04.htm"])
            .page("30566-05.html", &["30566-08.html", "30566-09.html"])
            .page("30566-06.html", &["30566-08.html", "30566-09.html"])
            .page("30566-07.html", &["30566-08.html", "30566-09.html"])
            .plain(&[
                "30566-01.htm",
                "30566-02.htm",
                "30566-04.htm",
                "30566-08.html",
                "30566-09.html",
            ])
            .npcs(&[VARKEES]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let varkees = t.spawn(VARKEES, 20);
        (t, varkees)
    }

    #[test]
    fn only_orcs_of_level_six_can_take_it() {
        let (mut t, varkees) = setup();
        t.enter_as(1, 10, race::HUMAN, player_class::FIGHTER, vec![]);
        assert!(t.quest_page(1, varkees).contains("30566-01.htm"));
        t.enter_as(2, 5, race::ORC, player_class::ORC_FIGHTER, vec![]);
        assert!(t.quest_page(2, varkees).contains("30566-02.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn soulstones_for_adena_then_the_soulshots_and_the_newbie_guide() {
        let (mut t, varkees) = setup();
        t.enter_as(1, 6, race::ORC, player_class::ORC_FIGHTER, vec![]);
        assert!(t.quest_page(1, varkees).contains("30566-03.htm"));
        assert!(t.event(1, NAME, "30566-04.htm").contains("30566-04.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        assert!(t.quest_page(1, varkees).contains("30566-05.html"));

        // Rakeclaw imps drop a black or, rarely, a red soulstone.
        for _ in 0..30 {
            let imp = t.spawn(20311, 60);
            t.kill(1, imp);
        }
        let black = t.count(1, BLACK_SOULSTONE);
        let red = t.count(1, RED_SOULSTONE);
        assert_eq!(black + red, 30);
        assert!(t.take_writes().is_empty());

        // Varkees: 10 adena a red and 3 a black, a bonus from ten stones; the soulshots once.
        let bonus = if red > 0 {
            if black + red >= 10 { 1800 } else { 0 }
        } else if black >= 10 {
            1500
        } else {
            0
        };
        let expected = red * 10 + black * 3 + bonus;
        let page = if red > 0 { "30566-07.html" } else { "30566-06.html" };
        assert!(t.quest_page(1, varkees).contains(page));
        assert_eq!(t.count(1, 57), expected);
        assert_eq!((t.count(1, BLACK_SOULSTONE), t.count(1, RED_SOULSTONE)), (0, 0));
        assert_eq!(t.count(1, 5789), 6000);
        let writes = t.take_writes();
        assert_eq!(
            writes[..2],
            [
                rows::insert(NEWBIE_GUIDE, "NRmemo", "41"),
                rows::insert(NEWBIE_GUIDE, "NRmemoState41", "1000"),
            ]
        );

        // `exitQuest(true, true)` from the last page.
        assert!(t.event(1, NAME, "30566-08.html").contains("30566-08.html"));
        assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
    }
}
