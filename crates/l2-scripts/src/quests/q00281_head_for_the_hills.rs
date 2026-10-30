//! Quest 281: Head for the Hills! (Mercela in the Town of Dion buys the claws of the mountain monsters).
// Java: dist/game/data/scripts/quests/Q00281_HeadForTheHills/Q00281_HeadForTheHills.java
//
// Pattern: a repeatable collect quest with a drop chance (`docs/design/scripts.md`, "Quest
// patterns"): the mountain and muertos monsters drop a claw with a chance; Mercela pays 23 adena a
// claw (400 more from 10 claws), or for 50 claws one reward: 361 times in a thousand one of nine
// armour pieces, otherwise the scroll of escape. Both payments give the beginner's shots once
// (level below 25, one-time flag 57) and tell the Newbie Guide.

use l2_protocol_game::NpcStringId;
use l2_util::ObjectId;
use l2_world::script::{CREATED, QuestSound, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const MERCELA: i32 = 32173;

// Item
const CLAWS: i32 = 9796;

// Rewards
const REWARDS: [i32; 10] = [
    115,  // Earring of Wisdom
    876,  // Ring of Anguish
    907,  // Necklace of Anguish
    22,   // Leather Shirt
    428,  // Feriotic Tunic
    1100, // Cotton Tunic
    29,   // Leather Pants
    463,  // Feriotic Stockings
    1103, // Cotton Stockings
    736,  // Scroll of Escape
];
const SOULSHOTS_NO_GRADE_FOR_ROOKIES: (i32, i64) = (5789, 6000);
const SPIRITSHOTS_NO_GRADE_FOR_ROOKIES: (i32, i64) = (5790, 3000);

/// Java `MONSTERS`: the chance out of a thousand of a claw.
fn claw_chance(npc_id: i32) -> i32 {
    match npc_id {
        22234 => 390, // Green Goblin
        22235 => 450, // Mountain Werewolf
        22236 => 650, // Muertos Archer
        22237 => 720, // Mountain Fungus
        22238 => 920, // Mountain Werewolf Chief
        22239 => 990, // Muertos Guard
        _ => 0,
    }
}

// Misc
const MIN_LEVEL: i32 = 6;
const GUIDE_MISSION: i32 = 41;
/// Java `NewbieGuide.class.getSimpleName()`.
const NEWBIE_GUIDE: &str = "NewbieGuide";

/// Java's rookie block (`if level < 25 && flag 57 == 0`) and the Newbie Guide block, both done
/// after each payment.
fn rookie_shots_and_guide(ctx: &mut ScriptCtx<'_>, player: ObjectId) {
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
}

/// Java `Q00281_HeadForTheHills`.
pub struct Q00281HeadForTheHills;

impl Script for Q00281HeadForTheHills {
    fn name(&self) -> &'static str {
        "Q00281_HeadForTheHills"
    }

    fn path(&self) -> &'static str {
        "quests/Q00281_HeadForTheHills"
    }

    fn id(&self) -> i32 {
        281
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[MERCELA]);
        s.add_talk_id(&[MERCELA]);
        s.add_kill_id(&[22234, 22235, 22236, 22237, 22238, 22239]);
        s.register_quest_items(&[CLAWS]);
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
            "32173-03.htm" => {
                qs.start_quest(ctx);
                htmltext = Some(event.to_owned());
            }
            "32173-06.html" => {
                if ctx.has_quest_items(player, CLAWS) {
                    let claws = ctx.get_quest_items_count(player, CLAWS);
                    let bonus = if claws >= 10 { 400 } else { 0 };
                    ctx.give_adena(player, claws * 23 + bonus, true);
                    ctx.take_items(player, CLAWS, -1);
                    rookie_shots_and_guide(ctx, player);
                    htmltext = Some(event.to_owned());
                } else {
                    htmltext = Some("32173-07.html".to_owned());
                }
            }
            "32173-08.html" => htmltext = Some(event.to_owned()),
            "32173-09.html" => {
                qs.exit_quest(ctx, true, true);
                htmltext = Some(event.to_owned());
            }
            "32173-11.html" => {
                if ctx.get_quest_items_count(player, CLAWS) >= 50 {
                    if ctx.get_random(1000) <= 360 {
                        let index = ctx.get_random(9) as usize;
                        ctx.give_items(player, REWARDS[index], 1);
                    } else {
                        ctx.give_items(player, REWARDS[9], 1);
                    }
                    ctx.take_items(player, CLAWS, 50);
                    rookie_shots_and_guide(ctx, player);
                    htmltext = Some(event.to_owned());
                } else {
                    htmltext = Some("32173-10.html".to_owned());
                }
            }
            _ => {}
        }
        htmltext
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        let qs = ctx.get_quest_state(killer, false);
        if qs.is_some() && ctx.get_random(1000) <= claw_chance(npc.id) {
            ctx.give_items(killer, CLAWS, 1);
            ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        match qs.state(ctx) {
            CREATED => {
                htmltext = if ctx.player_level(player) >= MIN_LEVEL {
                    "32173-01.htm"
                } else {
                    "32173-02.htm"
                }
                .to_owned();
            }
            STARTED => {
                htmltext = if ctx.has_quest_items(player, CLAWS) {
                    "32173-05.html"
                } else {
                    "32173-04.html"
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
    use l2_world::ScriptRegistry;
    use l2_world::script::{player_class, race};
    use l2_world::{Script, ScriptSetup};

    use super::*;
    use crate::testing::{Pages, ScriptTest, rows};

    const NAME: &str = "Q00281_HeadForTheHills";

    const NPCS: &str = r#"
        <npc id="32173" type="Folk" level="70" name="Mercela"/>
        <npc id="22239" type="Monster" level="40" name="Muertos Guard"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="9796" type="EtcItem" name="Claws"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="5789" type="EtcItem" name="Soulshot: No Grade"><set name="is_stackable" val="true"/></item>
        <item id="115" type="Armor" name="Earring of Wisdom"><set name="bodypart" val="rear"/></item>
        <item id="876" type="Armor" name="Ring of Anguish"><set name="bodypart" val="rfinger;lfinger"/></item>
        <item id="907" type="Armor" name="Necklace of Anguish"><set name="bodypart" val="neck"/></item>
        <item id="22" type="Armor" name="Leather Shirt"><set name="bodypart" val="chest"/></item>
        <item id="428" type="Armor" name="Feriotic Tunic"><set name="bodypart" val="chest"/></item>
        <item id="1100" type="Armor" name="Cotton Tunic"><set name="bodypart" val="chest"/></item>
        <item id="29" type="Armor" name="Leather Pants"><set name="bodypart" val="legs"/></item>
        <item id="463" type="Armor" name="Feriotic Stockings"><set name="bodypart" val="legs"/></item>
        <item id="1103" type="Armor" name="Cotton Stockings"><set name="bodypart" val="legs"/></item>
        <item id="736" type="EtcItem" name="Scroll of Escape"><set name="is_stackable" val="true"/></item>"#;

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
        registry.add(Q00281HeadForTheHills);
        let pages = Pages::of(&Q00281HeadForTheHills)
            .page("32173-01.htm", &["32173-03.htm"])
            .plain(&[
                "32173-02.htm",
                "32173-03.htm",
                "32173-04.html",
                "32173-07.html",
                "32173-10.html",
            ])
            .page("32173-05.html", &["32173-06.html", "32173-11.html"])
            .page("32173-06.html", &["32173-08.html", "32173-09.html"])
            .page("32173-11.html", &["32173-08.html", "32173-09.html"])
            .plain(&["32173-08.html", "32173-09.html"])
            .npcs(&[MERCELA]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let mercela = t.spawn(MERCELA, 20);
        (t, mercela)
    }

    /// Kills Muertos Guards until `claws` claws are in the inventory.
    fn hunt(t: &mut ScriptTest, claws: i64) {
        for _ in 0..1000 {
            if t.count(1, CLAWS) >= claws {
                return;
            }
            let guard = t.spawn(22239, 60);
            t.kill(1, guard);
        }
        panic!("no {claws} claws after 1000 kills");
    }

    #[test]
    fn a_level_five_character_is_sent_away() {
        let (mut t, mercela) = setup();
        t.enter_as(1, 5, race::HUMAN, player_class::FIGHTER, vec![]);
        assert!(t.quest_page(1, mercela).contains("32173-02.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn fifty_claws_for_a_reward_then_claws_for_adena() {
        let (mut t, mercela) = setup();
        t.enter_as(1, 6, race::HUMAN, player_class::FIGHTER, vec![]);
        assert!(t.quest_page(1, mercela).contains("32173-01.htm"));
        assert!(t.event(1, NAME, "32173-03.htm").contains("32173-03.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        assert!(t.quest_page(1, mercela).contains("32173-04.html"));

        // Claws come with a chance per kill; no condition is set.
        hunt(&mut t, 50);
        assert!(t.take_writes().is_empty());

        // 50 claws: one reward, the claws go; a beginner fighter gets soulshots once and the
        // Newbie Guide hears about it.
        assert!(t.quest_page(1, mercela).contains("32173-05.html"));
        assert!(t.event(1, NAME, "32173-11.html").contains("32173-11.html"));
        assert_eq!(t.count(1, CLAWS), 0);
        let rewards: i64 = REWARDS.iter().map(|&item| t.count(1, item)).sum();
        assert_eq!(rewards, 1);
        assert_eq!(t.count(1, 5789), 6000);
        let writes = t.take_writes();
        assert_eq!(
            writes[..2],
            [
                rows::insert(NEWBIE_GUIDE, "NRmemo", "41"),
                rows::insert(NEWBIE_GUIDE, "NRmemoState41", "1000"),
            ]
        );

        // Claws again: 23 adena each, the guide's mission was told once already.
        hunt(&mut t, 3);
        assert!(t.quest_page(1, mercela).contains("32173-05.html"));
        assert!(t.event(1, NAME, "32173-06.html").contains("32173-06.html"));
        assert_eq!(t.count(1, 57), 3 * 23);
        assert_eq!(t.count(1, CLAWS), 0);
        assert!(t.take_writes().is_empty());

        // `exitQuest(true, true)` from the last page.
        assert!(t.event(1, NAME, "32173-09.html").contains("32173-09.html"));
        assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
    }
}
