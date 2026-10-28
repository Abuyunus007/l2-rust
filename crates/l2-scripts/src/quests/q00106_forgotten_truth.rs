//! Quest 106: Forgotten Truth (Thifiell sends a dark elf to Karta, who sends it to the Tumran
//! Orc Brigands for the scroll and the clay tablet, and back to Karta for the translation).
// Java: dist/game/data/scripts/quests/Q00106_ForgottenTruth/Q00106_ForgottenTruth.java
//
// Pattern: a kill chance with a limit (`getRandom(100) < 20`, the scroll first, then the tablet),
// the items of one NPC taken together (`takeItems(player, -1, ...)`), and the NewbieGuide memo
// as in quest 104.

use l2_protocol_game::NpcStringId;
use l2_protocol_game::server::SocialAction;
use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, QuestSound, STARTED, race};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const THIFIELL: i32 = 30358;
const KARTA: i32 = 30133;

// Monster
const TUMRAN_ORC_BRIGAND: i32 = 27070;

// Items
const ONYX_TALISMAN1: i32 = 984;
const ONYX_TALISMAN2: i32 = 985;
const ANCIENT_SCROLL: i32 = 986;
const ANCIENT_CLAY_TABLET: i32 = 987;
const KARTAS_TRANSLATION: i32 = 988;

// Rewards
const ELDRITCH_DAGGER: i32 = 989;
const ELDRITCH_STAFF: i32 = 2373;

/// Java `REWARDS` (`ItemHolder`s): item id and count.
const REWARDS: [(i32, i64); 6] = [
    (1060, 100), // Lesser Healing Potion
    (4412, 10),  // Echo Crystal - Theme of Battle
    (4413, 10),  // Echo Crystal - Theme of Love
    (4414, 10),  // Echo Crystal - Theme of Solitude
    (4415, 10),  // Echo Crystal - Theme of Feast
    (4416, 10),  // Echo Crystal - Theme of Celebration
];
const SPIRITSHOTS_NO_GRADE_FOR_ROOKIES: (i32, i64) = (5790, 3000);
const SOULSHOTS_NO_GRADE: (i32, i64) = (1835, 1000);
const SPIRITSHOTS_NO_GRADE: (i32, i64) = (2509, 500);

// Misc
const MIN_LEVEL: i32 = 10;
const GUIDE_MISSION: i32 = 41;
/// Java `NewbieGuide.class.getSimpleName()`.
const NEWBIE_GUIDE: &str = "NewbieGuide";

/// Java `Q00106_ForgottenTruth`.
pub struct Q00106ForgottenTruth;

impl Script for Q00106ForgottenTruth {
    fn name(&self) -> &'static str {
        "Q00106_ForgottenTruth"
    }

    fn path(&self) -> &'static str {
        "quests/Q00106_ForgottenTruth"
    }

    fn id(&self) -> i32 {
        106
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[THIFIELL]);
        s.add_talk_id(&[THIFIELL, KARTA]);
        s.add_kill_id(&[TUMRAN_ORC_BRIGAND]);
        s.register_quest_items(&[
            KARTAS_TRANSLATION,
            ONYX_TALISMAN1,
            ONYX_TALISMAN2,
            ANCIENT_SCROLL,
            ANCIENT_CLAY_TABLET,
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
        match event {
            "30358-04.htm" => Some(event.to_owned()),
            "30358-05.htm" => {
                if qs.is_created(ctx) {
                    qs.start_quest(ctx);
                    ctx.give_items(player, ONYX_TALISMAN1, 1);
                    return Some(event.to_owned());
                }
                None
            }
            _ => None,
        }
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        let Some(qs) = ctx.get_quest_state(killer, false) else {
            return;
        };
        if qs.is_cond(ctx, 2)
            && ctx.check_if_in_range(ctx.alt_party_range(), npc.object_id, killer, true)
            && ctx.get_random(100) < 20
            && ctx.has_quest_items(killer, ONYX_TALISMAN2)
        {
            if !ctx.has_quest_items(killer, ANCIENT_SCROLL) {
                ctx.give_items(killer, ANCIENT_SCROLL, 1);
                ctx.play_sound(killer, QuestSound::ItemsoundQuestMiddle);
            } else if !ctx.has_quest_items(killer, ANCIENT_CLAY_TABLET) {
                qs.set_cond(ctx, 3, true);
                ctx.give_items(killer, ANCIENT_CLAY_TABLET, 1);
            }
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, talker: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(talker, true)?;
        let mut htmltext = ctx.get_no_quest_msg(talker);
        match npc.id {
            THIFIELL => match qs.state(ctx) {
                CREATED => {
                    htmltext = if ctx.player_race(talker) == race::DARK_ELF {
                        if ctx.player_level(talker) >= MIN_LEVEL {
                            "30358-03.htm"
                        } else {
                            "30358-02.htm"
                        }
                    } else {
                        "30358-01.htm"
                    }
                    .to_owned();
                }
                STARTED => {
                    if ctx.has_at_least_one_quest_item(talker, &[ONYX_TALISMAN1, ONYX_TALISMAN2])
                        && !ctx.has_quest_items(talker, KARTAS_TRANSLATION)
                    {
                        htmltext = "30358-06.html".to_owned();
                    } else if qs.is_cond(ctx, 4) && ctx.has_quest_items(talker, KARTAS_TRANSLATION) {
                        if ctx.player_level(talker) < 25 && ctx.is_mage_class(talker) {
                            let (id, count) = SPIRITSHOTS_NO_GRADE_FOR_ROOKIES;
                            ctx.give_items(talker, id, count);
                            ctx.play_sound_named(talker, "tutorial_voice_027");
                        }
                        if !ctx.is_mage_class(talker) {
                            let (id, count) = SOULSHOTS_NO_GRADE;
                            ctx.give_items(talker, id, count);
                        } else {
                            let (id, count) = SPIRITSHOTS_NO_GRADE;
                            ctx.give_items(talker, id, count);
                        }
                        for (id, count) in REWARDS {
                            ctx.give_items(talker, id, count);
                        }
                        // Newbie Guide.
                        if let Some(guide) = ctx.quest_state_of(NEWBIE_GUIDE, talker, true) {
                            if !ctx.have_nr_memo(guide, GUIDE_MISSION) {
                                ctx.set_nr_memo(guide, GUIDE_MISSION);
                                ctx.set_nr_memo_state(guide, GUIDE_MISSION, 100_000);
                                ctx.show_on_screen_npc_string(
                                    talker,
                                    NpcStringId::AcquisitionOfRaceSpecificWeaponCompleteNGoFindTheNewbieGuide,
                                    2,
                                    5000,
                                    &[],
                                );
                            } else if (ctx.get_nr_memo_state(guide, GUIDE_MISSION) % 1_000_000) / 100_000 != 1
                            {
                                ctx.set_nr_memo(guide, GUIDE_MISSION);
                                let state = ctx.get_nr_memo_state(guide, GUIDE_MISSION);
                                ctx.set_nr_memo_state(guide, GUIDE_MISSION, state + 100_000);
                                ctx.show_on_screen_npc_string(
                                    talker,
                                    NpcStringId::AcquisitionOfRaceSpecificWeaponCompleteNGoFindTheNewbieGuide,
                                    2,
                                    5000,
                                    &[],
                                );
                            }
                        }
                        ctx.give_adena(talker, 10266, true);
                        ctx.add_exp_and_sp(talker, 24195, 2074);
                        let weapon = if ctx.is_mage_class(talker) {
                            ELDRITCH_STAFF
                        } else {
                            ELDRITCH_DAGGER
                        };
                        ctx.give_items(talker, weapon, 1);
                        qs.exit_quest(ctx, false, true);
                        ctx.world().send(
                            talker,
                            &SocialAction {
                                object_id: talker.0,
                                action_id: 3,
                            },
                        );
                        htmltext = "30358-07.html".to_owned();
                    }
                }
                COMPLETED => htmltext = ctx.get_already_completed_msg(talker),
                _ => {}
            },
            KARTA => {
                if qs.is_started(ctx) {
                    match qs.get_cond(ctx) {
                        1 => {
                            if ctx.has_quest_items(talker, ONYX_TALISMAN1) {
                                qs.set_cond(ctx, 2, true);
                                ctx.take_items(talker, ONYX_TALISMAN1, -1);
                                ctx.give_items(talker, ONYX_TALISMAN2, 1);
                                htmltext = "30133-01.html".to_owned();
                            }
                        }
                        2 => {
                            if ctx.has_quest_items(talker, ONYX_TALISMAN2) {
                                htmltext = "30133-02.html".to_owned();
                            }
                        }
                        3 => {
                            if ctx.has_all_quest_items(talker, &[ANCIENT_SCROLL, ANCIENT_CLAY_TABLET]) {
                                qs.set_cond(ctx, 4, true);
                                ctx.take_items_many(
                                    talker,
                                    -1,
                                    &[ANCIENT_SCROLL, ANCIENT_CLAY_TABLET, ONYX_TALISMAN2],
                                );
                                ctx.give_items(talker, KARTAS_TRANSLATION, 1);
                                htmltext = "30133-03.html".to_owned();
                            }
                        }
                        4 => {
                            if ctx.has_quest_items(talker, KARTAS_TRANSLATION) {
                                htmltext = "30133-04.html".to_owned();
                            }
                        }
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
    use l2_world::script::{player_class, race};

    use super::{KARTA, Q00106ForgottenTruth as Quest, THIFIELL, TUMRAN_ORC_BRIGAND};
    use crate::testing::{Pages, ScriptTest, rows};

    const NAME: &str = "Q00106_ForgottenTruth";
    const TALISMAN1: i32 = 984;
    const TALISMAN2: i32 = 985;
    const SCROLL: i32 = 986;
    const TABLET: i32 = 987;
    const TRANSLATION: i32 = 988;

    const NPCS: &str = r#"
        <npc id="30358" type="Folk" level="70" name="Thifiell"/>
        <npc id="30133" type="Folk" level="70" name="Karta"/>
        <npc id="27070" type="Monster" level="76" name="Tumran Orc Brigand"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="984" type="EtcItem" name="Onyx Talisman 1"><set name="is_questitem" val="true"/></item>
        <item id="985" type="EtcItem" name="Onyx Talisman 2"><set name="is_questitem" val="true"/></item>
        <item id="986" type="EtcItem" name="Ancient Scroll"><set name="is_questitem" val="true"/></item>
        <item id="987" type="EtcItem" name="Ancient Clay Tablet"><set name="is_questitem" val="true"/></item>
        <item id="988" type="EtcItem" name="Karta's Translation"><set name="is_questitem" val="true"/></item>
        <item id="989" type="Weapon" name="Eldritch Dagger"/>
        <item id="1060" type="EtcItem" name="Lesser Healing Potion"><set name="is_stackable" val="true"/></item>"#;

    fn pages() -> Pages {
        Pages::of(&Quest)
            .page("30358-03.htm", &["30358-04.htm"])
            .page("30358-04.htm", &["30358-05.htm"])
            .plain(&[
                "30358-01.htm",
                "30358-02.htm",
                "30358-05.htm",
                "30358-06.html",
                "30358-07.html",
                "30133-01.html",
                "30133-02.html",
                "30133-03.html",
                "30133-04.html",
            ])
            .npcs(&[THIFIELL, KARTA])
    }

    fn setup() -> (ScriptTest, Oid, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Quest);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages());
        let thifiell = t.spawn(THIFIELL, 20);
        let karta = t.spawn(KARTA, 30);
        (t, thifiell, karta)
    }

    #[test]
    fn a_dark_elf_of_level_10_starts_and_gets_the_first_talisman() {
        let (mut t, thifiell, _) = setup();
        t.enter_as(1, 10, race::DARK_ELF, player_class::DARK_FIGHTER, vec![]);
        assert!(t.quest_page(1, thifiell).contains("30358-03.htm"));
        assert!(t.event(1, NAME, "30358-04.htm").contains("30358-04.htm"));
        assert!(t.event(1, NAME, "30358-05.htm").contains("30358-05.htm"));
        assert_eq!(t.count(1, TALISMAN1), 1);
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
    }

    #[test]
    fn the_start_is_refused_for_other_races_and_low_levels() {
        let (mut t, thifiell, _) = setup();
        t.enter_as(1, 9, race::DARK_ELF, player_class::DARK_FIGHTER, vec![]);
        assert!(t.quest_page(1, thifiell).contains("30358-02.htm"));
        t.enter_as(2, 20, race::HUMAN, player_class::FIGHTER, vec![]);
        assert!(t.quest_page(2, thifiell).contains("30358-01.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_talisman_scroll_tablet_translation_and_the_reward() {
        let (mut t, thifiell, karta) = setup();
        t.enter_as(1, 20, race::DARK_ELF, player_class::DARK_FIGHTER, vec![]);
        assert!(t.quest_page(1, thifiell).contains("30358-03.htm"));
        assert!(t.event(1, NAME, "30358-04.htm").contains("30358-04.htm"));
        assert!(t.event(1, NAME, "30358-05.htm").contains("30358-05.htm"));
        assert_eq!(t.count(1, TALISMAN1), 1);
        t.take_writes();

        // Karta takes the first talisman and gives the second: cond 2.
        assert!(t.quest_page(1, karta).contains("30133-01.html"));
        assert_eq!(t.count(1, TALISMAN1), 0);
        assert_eq!(t.count(1, TALISMAN2), 1);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);

        // The brigands: a kill gives the scroll with a 20% chance, then the tablet (cond 3).
        for _ in 0..500 {
            if t.count(1, TABLET) == 1 {
                break;
            }
            let brigand = t.spawn(TUMRAN_ORC_BRIGAND, 60);
            t.kill(1, brigand);
        }
        assert_eq!(t.count(1, SCROLL), 1);
        assert_eq!(t.count(1, TABLET), 1);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "3")]);

        // Karta: the scroll, the tablet and the talisman go, the translation comes: cond 4.
        assert!(t.quest_page(1, karta).contains("30133-03.html"));
        assert_eq!(
            (t.count(1, SCROLL), t.count(1, TABLET), t.count(1, TALISMAN2)),
            (0, 0, 0)
        );
        assert_eq!(t.count(1, TRANSLATION), 1);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "4")]);
        assert!(t.quest_page(1, karta).contains("30133-04.html"));

        // Thifiell: the reward (a dagger for a fighter), the exit.
        assert!(t.quest_page(1, thifiell).contains("30358-07.html"));
        assert_eq!(t.count(1, 989), 1);
        assert_eq!(t.count(1, 1060), 100);
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed")
            ]
        );
    }
}
