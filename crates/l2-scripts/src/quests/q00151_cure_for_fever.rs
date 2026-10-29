//! Quest 151: Cure for Fever (Elias sends the player to kill spiders, then Yohanes for medicine).
// Java: dist/game/data/scripts/quests/Q00151_CureForFever/Q00151_CureForFever.java
//
// Pattern: a kill quest with a drop chance and a delivery chain. A spider drops the poison sac
// on a 1-in-5 roll (`getRandom(5) == 0`) until cond 2; Yohanes swaps the sac for the medicine
// (cond 3); Elias pays and ends the quest (one-time). The Newbie Guide's memo is counted and the
// guide quest is set completed, like Java.

use l2_protocol_game::NpcStringId;
use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const ELLIAS: i32 = 30050;
const YOHANES: i32 = 30032;

// Monsters
const MOBS: [i32; 3] = [
    20103, // Giant Spider
    20106, // Talon Spider
    20108, // Blade Spider
];

// Items
const ROUND_SHIELD: i32 = 102;
const POISON_SAC: i32 = 703;
const FEVER_MEDICINE: i32 = 704;

// Misc
const MIN_LEVEL: i32 = 15;
const CHANCE: i32 = 0;
const GUIDE_MISSION: i32 = 41;
/// Java `NewbieGuide.class.getSimpleName()`.
const NEWBIE_GUIDE: &str = "NewbieGuide";

/// Java `Q00151_CureForFever`.
pub struct Q00151CureForFever;

impl Script for Q00151CureForFever {
    fn name(&self) -> &'static str {
        "Q00151_CureForFever"
    }

    fn path(&self) -> &'static str {
        "quests/Q00151_CureForFever"
    }

    fn id(&self) -> i32 {
        151
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[ELLIAS]);
        s.add_talk_id(&[ELLIAS, YOHANES]);
        s.add_kill_id(&MOBS);
        s.register_quest_items(&[POISON_SAC, FEVER_MEDICINE]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        _npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        let player = player?;
        let qs = ctx.get_quest_state(player, false);
        if let Some(qs) = qs
            && event.eq_ignore_ascii_case("30050-03.htm")
        {
            qs.start_quest(ctx);
            return Some(event.to_owned());
        }
        None
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        let qs = ctx.get_quest_state(killer, false);
        if let Some(qs) = qs
            && qs.is_cond(ctx, 1)
            && ctx.get_random(5) == CHANCE
        {
            ctx.give_items(killer, POISON_SAC, 1);
            qs.set_cond(ctx, 2, true);
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        match npc.id {
            ELLIAS => match qs.state(ctx) {
                CREATED => {
                    htmltext = if ctx.player_level(player) >= MIN_LEVEL {
                        "30050-02.htm".to_owned()
                    } else {
                        "30050-01.htm".to_owned()
                    };
                }
                STARTED => {
                    if qs.is_cond(ctx, 3) && ctx.has_quest_items(player, FEVER_MEDICINE) {
                        // Newbie Guide: its memo counts the last duty and the guide quest is
                        // then set completed (Java skips the block when the script is missing).
                        if let Some(guide) = ctx.quest_state_of(NEWBIE_GUIDE, player, true) {
                            if !ctx.have_nr_memo(guide, GUIDE_MISSION) {
                                ctx.set_nr_memo(guide, GUIDE_MISSION);
                                ctx.set_nr_memo_state(guide, GUIDE_MISSION, 100_000);
                                ctx.show_on_screen_npc_string(
                                    player,
                                    NpcStringId::LastDutyCompleteNGoFindTheNewbieGuide,
                                    2,
                                    5000,
                                    &[],
                                );
                            } else if (ctx.get_nr_memo_state(guide, GUIDE_MISSION) % 100_000_000) / 10_000_000
                                != 1
                            {
                                ctx.set_nr_memo(guide, GUIDE_MISSION);
                                let state = ctx.get_nr_memo_state(guide, GUIDE_MISSION);
                                ctx.set_nr_memo_state(guide, GUIDE_MISSION, state + 10_000_000);
                                ctx.show_on_screen_npc_string(
                                    player,
                                    NpcStringId::LastDutyCompleteNGoFindTheNewbieGuide,
                                    2,
                                    5000,
                                    &[],
                                );
                            }
                            guide.set_state(ctx, COMPLETED);
                        }

                        ctx.give_items(player, ROUND_SHIELD, 1);
                        ctx.add_exp_and_sp(player, 13_106, 613);
                        qs.exit_quest(ctx, false, true);
                        htmltext = "30050-06.html".to_owned();
                    } else if qs.is_cond(ctx, 2) && ctx.has_quest_items(player, POISON_SAC) {
                        htmltext = "30050-05.html".to_owned();
                    } else {
                        htmltext = "30050-04.html".to_owned();
                    }
                }
                COMPLETED => htmltext = ctx.get_already_completed_msg(player),
                _ => {}
            },
            YOHANES => {
                if qs.is_started(ctx) {
                    if qs.is_cond(ctx, 2) && ctx.has_quest_items(player, POISON_SAC) {
                        qs.set_cond(ctx, 3, true);
                        ctx.take_items(player, POISON_SAC, -1);
                        ctx.give_items(player, FEVER_MEDICINE, 1);
                        htmltext = "30032-01.html".to_owned();
                    } else if qs.is_cond(ctx, 3) && ctx.has_quest_items(player, FEVER_MEDICINE) {
                        htmltext = "30032-02.html".to_owned();
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
    use l2_world::{Script, ScriptRegistry, ScriptSetup};

    use super::*;
    use crate::testing::{Pages, ScriptTest, rows};

    const NAME: &str = "Q00151_CureForFever";

    const NPCS: &str = r#"
        <npc id="30050" type="Folk" level="1" name="Elias"/>
        <npc id="30032" type="Folk" level="1" name="Yohanes"/>
        <npc id="20103" type="Monster" level="2" name="Giant Spider"/>"#;

    const ITEMS: &str = r#"
        <item id="102" type="Weapon" name="Round Shield"/>
        <item id="703" type="EtcItem" name="Poison Sac"><set name="is_questitem" val="true"/></item>
        <item id="704" type="EtcItem" name="Fever Medicine"><set name="is_questitem" val="true"/></item>"#;

    /// The Newbie Guide, read only here: a script of that name (its memo rows are the test's).
    struct NewbieGuide;

    impl Script for NewbieGuide {
        fn name(&self) -> &'static str {
            NEWBIE_GUIDE
        }

        fn path(&self) -> &'static str {
            "ai/others/NewbieGuide"
        }

        fn setup(&self, _s: &mut ScriptSetup) {}
    }

    fn setup() -> (ScriptTest, Oid, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(NewbieGuide);
        registry.add(Q00151CureForFever);
        let pages = Pages::of(&Q00151CureForFever)
            .page("30050-02.htm", &["30050-03.htm"])
            .plain(&[
                "30050-01.htm",
                "30050-03.htm",
                "30050-04.html",
                "30050-05.html",
                "30050-06.html",
                "30032-01.html",
                "30032-02.html",
            ])
            .npcs(&[ELLIAS, YOHANES]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let elias = t.spawn(ELLIAS, 20);
        let yohanes = t.spawn(YOHANES, 40);
        (t, elias, yohanes)
    }

    #[test]
    fn a_character_below_level_15_is_sent_away() {
        let (mut t, elias, _) = setup();
        t.enter(1, 14, vec![]);
        assert!(t.quest_page(1, elias).contains("30050-01.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_spider_sac_the_medicine_and_the_reward() {
        let (mut t, elias, yohanes) = setup();
        t.enter(1, 15, vec![]);
        assert!(t.quest_page(1, elias).contains("30050-02.htm"));
        assert!(t.event(1, NAME, "30050-03.htm").contains("30050-03.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );

        // Spiders until the poison sac drops (a 1-in-5 roll): cond 2.
        let mut kills = 0;
        while t.count(1, POISON_SAC) < 1 {
            let spider = t.spawn(MOBS[0], 60);
            t.kill(1, spider);
            kills += 1;
            assert!(kills < 500, "the spiders never dropped a sac");
        }
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);
        assert!(t.quest_page(1, elias).contains("30050-05.html"));

        // Yohanes swaps the sac for the medicine: cond 3.
        assert!(t.quest_page(1, yohanes).contains("30032-01.html"));
        assert_eq!(t.count(1, FEVER_MEDICINE), 1);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "3")]);
        assert!(t.quest_page(1, yohanes).contains("30032-02.html"));

        // Elias: the Newbie Guide's memo, the round shield, exp and sp, `exitQuest(false, true)`.
        assert!(t.quest_page(1, elias).contains("30050-06.html"));
        assert_eq!(t.count(1, ROUND_SHIELD), 1);
        let writes = t.take_writes();
        assert!(
            writes.contains(&rows::insert(NEWBIE_GUIDE, "NRmemo", "41")),
            "{writes:?}"
        );
        assert!(
            writes.contains(&rows::insert(NEWBIE_GUIDE, "NRmemoState41", "100000")),
            "{writes:?}"
        );
        assert!(
            writes.contains(&rows::insert(NEWBIE_GUIDE, "<state>", "Completed")),
            "{writes:?}"
        );
        assert!(writes.contains(&rows::delete(NAME, false)), "{writes:?}");
        assert!(
            writes.contains(&rows::update(NAME, "<state>", "Completed")),
            "{writes:?}"
        );
    }
}
