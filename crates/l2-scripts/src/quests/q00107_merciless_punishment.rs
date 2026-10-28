//! Quest 107: Merciless Punishment (Urutu Chief Hatos, an orc, sends orders through Centurion
//! Parugon and the Baranka messengers, and each messenger's letter moves the order on).
// Java: dist/game/data/scripts/quests/Q00107_MercilessPunishment/Q00107_MercilessPunishment.java
//
// Pattern: a delivery chain where the monster gives the next letter for the item in hand
// (`onKill` by cond), and the give-up page that exits repeatable (`exitQuest(true)`).

use l2_protocol_game::NpcStringId;
use l2_protocol_game::server::SocialAction;
use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, QuestSound, STARTED, race};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// Npc
const URUTU_CHIEF_HATOS: i32 = 30568;
const CENTURION_PARUGON: i32 = 30580;

// Items
const HATOSS_ORDER_1: i32 = 1553;
const HATOSS_ORDER_2: i32 = 1554;
const HATOSS_ORDER_3: i32 = 1555;
const LETTER_TO_DARK_ELF: i32 = 1556;
const LETTER_TO_HUMAN: i32 = 1557;
const LETTER_TO_ELF: i32 = 1558;

// Monster
const BARANKA_MESSENGER: i32 = 27041;

// Rewards
const BUTCHER: i32 = 1510;

/// Java `REWARDS` (`ItemHolder`s): item id and count.
const REWARDS: [(i32, i64); 6] = [
    (1060, 100), // Lesser Healing Potion
    (4412, 10),  // Echo Crystal - Theme of Battle
    (4413, 10),  // Echo Crystal - Theme of Love
    (4414, 10),  // Echo Crystal - Theme of Solitude
    (4415, 10),  // Echo Crystal - Theme of Feast
    (4416, 10),  // Echo Crystal - Theme of Celebration
];
const SOULSHOTS_NO_GRADE_FOR_ROOKIES: (i32, i64) = (5789, 7000);

// Misc
const MIN_LEVEL: i32 = 10;
const GUIDE_MISSION: i32 = 41;
/// Java `NewbieGuide.class.getSimpleName()`.
const NEWBIE_GUIDE: &str = "NewbieGuide";

/// Java `Q00107_MercilessPunishment`.
pub struct Q00107MercilessPunishment;

impl Script for Q00107MercilessPunishment {
    fn name(&self) -> &'static str {
        "Q00107_MercilessPunishment"
    }

    fn path(&self) -> &'static str {
        "quests/Q00107_MercilessPunishment"
    }

    fn id(&self) -> i32 {
        107
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[URUTU_CHIEF_HATOS]);
        s.add_talk_id(&[URUTU_CHIEF_HATOS, CENTURION_PARUGON]);
        s.add_kill_id(&[BARANKA_MESSENGER]);
        s.register_quest_items(&[
            HATOSS_ORDER_1,
            HATOSS_ORDER_2,
            HATOSS_ORDER_3,
            LETTER_TO_DARK_ELF,
            LETTER_TO_HUMAN,
            LETTER_TO_ELF,
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
            "30568-04.htm" => {
                if qs.is_created(ctx) {
                    qs.start_quest(ctx);
                    ctx.give_items(player, HATOSS_ORDER_1, 1);
                    return Some(event.to_owned());
                }
                None
            }
            "30568-07.html" => {
                ctx.give_adena(player, 200, true);
                ctx.play_sound(player, QuestSound::ItemsoundQuestGiveup);
                qs.exit_quest(ctx, true, false);
                Some(event.to_owned())
            }
            "30568-08.html" => {
                if qs.is_cond(ctx, 3) && ctx.has_quest_items(player, HATOSS_ORDER_1) {
                    qs.set_cond(ctx, 4, false);
                    ctx.take_items(player, HATOSS_ORDER_1, -1);
                    ctx.give_items(player, HATOSS_ORDER_2, 1);
                    return Some(event.to_owned());
                }
                None
            }
            "30568-10.html" => {
                if qs.is_cond(ctx, 5) && ctx.has_quest_items(player, HATOSS_ORDER_2) {
                    qs.set_cond(ctx, 6, false);
                    ctx.take_items(player, HATOSS_ORDER_2, -1);
                    ctx.give_items(player, HATOSS_ORDER_3, 1);
                    return Some(event.to_owned());
                }
                None
            }
            _ => None,
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, talker: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(talker, true)?;
        let mut htmltext = ctx.get_no_quest_msg(talker);
        match npc.id {
            URUTU_CHIEF_HATOS => match qs.state(ctx) {
                CREATED => {
                    htmltext = if ctx.player_race(talker) != race::ORC {
                        "30568-01.htm"
                    } else if ctx.player_level(talker) < MIN_LEVEL {
                        "30568-02.htm"
                    } else {
                        "30568-03.htm"
                    }
                    .to_owned();
                }
                STARTED => match qs.get_cond(ctx) {
                    1 | 2 => {
                        if ctx.has_quest_items(talker, HATOSS_ORDER_1) {
                            htmltext = "30568-05.html".to_owned();
                        }
                    }
                    3 => {
                        if ctx.has_all_quest_items(talker, &[HATOSS_ORDER_1, LETTER_TO_HUMAN]) {
                            htmltext = "30568-06.html".to_owned();
                        }
                    }
                    4 => {
                        if ctx.has_all_quest_items(talker, &[HATOSS_ORDER_2, LETTER_TO_HUMAN]) {
                            htmltext = "30568-08.html".to_owned();
                        }
                    }
                    5 => {
                        if ctx.has_all_quest_items(
                            talker,
                            &[HATOSS_ORDER_2, LETTER_TO_HUMAN, LETTER_TO_DARK_ELF],
                        ) {
                            htmltext = "30568-09.html".to_owned();
                        }
                    }
                    6 => {
                        if ctx.has_all_quest_items(
                            talker,
                            &[HATOSS_ORDER_3, LETTER_TO_HUMAN, LETTER_TO_DARK_ELF],
                        ) {
                            htmltext = "30568-10.html".to_owned();
                        }
                    }
                    7 => {
                        if ctx.has_all_quest_items(
                            talker,
                            &[HATOSS_ORDER_3, LETTER_TO_HUMAN, LETTER_TO_DARK_ELF, LETTER_TO_ELF],
                        ) {
                            if ctx.player_level(talker) < 25 {
                                let (id, count) = SOULSHOTS_NO_GRADE_FOR_ROOKIES;
                                ctx.give_items(talker, id, count);
                                ctx.play_sound_named(talker, "tutorial_voice_026");
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
                                } else if (ctx.get_nr_memo_state(guide, GUIDE_MISSION) % 1_000_000) / 100_000
                                    != 1
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
                            ctx.add_exp_and_sp(talker, 34565, 2962);
                            ctx.give_adena(talker, 14666, true);
                            for (id, count) in REWARDS {
                                ctx.give_items(talker, id, count);
                            }
                            ctx.give_items(talker, BUTCHER, 1);
                            qs.exit_quest(ctx, false, true);
                            ctx.world().send(
                                talker,
                                &SocialAction {
                                    object_id: talker.0,
                                    action_id: 3,
                                },
                            );
                            htmltext = "30568-11.html".to_owned();
                        }
                    }
                    _ => {}
                },
                COMPLETED => htmltext = ctx.get_already_completed_msg(talker),
                _ => {}
            },
            CENTURION_PARUGON => {
                if qs.is_started(ctx) && qs.is_cond(ctx, 1) && ctx.has_quest_items(talker, HATOSS_ORDER_1) {
                    qs.set_cond(ctx, 2, true);
                    htmltext = "30580-01.html".to_owned();
                }
            }
            _ => {}
        }
        Some(htmltext)
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        let Some(qs) = ctx.get_quest_state(killer, false) else {
            return;
        };
        if !ctx.check_if_in_range(ctx.alt_party_range(), npc.object_id, killer, true) {
            return;
        }
        match qs.get_cond(ctx) {
            2 => {
                if ctx.has_quest_items(killer, HATOSS_ORDER_1) {
                    ctx.give_items(killer, LETTER_TO_HUMAN, 1);
                    qs.set_cond(ctx, 3, true);
                }
            }
            4 => {
                if ctx.has_quest_items(killer, HATOSS_ORDER_2) {
                    ctx.give_items(killer, LETTER_TO_DARK_ELF, 1);
                    qs.set_cond(ctx, 5, true);
                }
            }
            6 => {
                if ctx.has_quest_items(killer, HATOSS_ORDER_3) {
                    ctx.give_items(killer, LETTER_TO_ELF, 1);
                    qs.set_cond(ctx, 7, true);
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::ScriptRegistry;
    use l2_world::script::{player_class, race};

    use super::{CENTURION_PARUGON, Q00107MercilessPunishment as Quest, URUTU_CHIEF_HATOS};
    use crate::testing::{Pages, ScriptTest, rows};

    const NAME: &str = "Q00107_MercilessPunishment";
    const ORDER1: i32 = 1553;
    const ORDER2: i32 = 1554;
    const ORDER3: i32 = 1555;
    const TO_DARK_ELF: i32 = 1556;
    const TO_HUMAN: i32 = 1557;
    const LETTER_TO_HUMAN: i32 = 1557;
    const TO_ELF: i32 = 1558;
    const BARANKA: i32 = 27041;

    const NPCS: &str = r#"
        <npc id="30568" type="Folk" level="70" name="Urutu Chief Hatos"/>
        <npc id="30580" type="Folk" level="70" name="Centurion Parugon"/>
        <npc id="27041" type="Monster" level="30" name="Baranka Messenger"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="1553" type="EtcItem" name="Hatos' Order 1"><set name="is_questitem" val="true"/></item>
        <item id="1554" type="EtcItem" name="Hatos' Order 2"><set name="is_questitem" val="true"/></item>
        <item id="1555" type="EtcItem" name="Hatos' Order 3"><set name="is_questitem" val="true"/></item>
        <item id="1556" type="EtcItem" name="Letter to Dark Elf"><set name="is_questitem" val="true"/></item>
        <item id="1557" type="EtcItem" name="Letter to Human"><set name="is_questitem" val="true"/></item>
        <item id="1558" type="EtcItem" name="Letter to Elf"><set name="is_questitem" val="true"/></item>
        <item id="1510" type="Weapon" name="Butcher"/>
        <item id="1060" type="EtcItem" name="Lesser Healing Potion"><set name="is_stackable" val="true"/></item>"#;

    fn pages() -> Pages {
        Pages::of(&Quest)
            .page("30568-03.htm", &["30568-04.htm"])
            .page("30568-06.html", &["30568-07.html", "30568-08.html"])
            .page("30568-09.html", &["30568-07.html", "30568-10.html"])
            .plain(&[
                "30568-01.htm",
                "30568-02.htm",
                "30568-04.htm",
                "30568-05.html",
                "30568-07.html",
                "30568-08.html",
                "30568-10.html",
                "30568-11.html",
                "30580-01.html",
            ])
            .npcs(&[URUTU_CHIEF_HATOS, CENTURION_PARUGON])
    }

    fn setup() -> (ScriptTest, Oid, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Quest);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages());
        let hatos = t.spawn(URUTU_CHIEF_HATOS, 20);
        let parugon = t.spawn(CENTURION_PARUGON, 30);
        (t, hatos, parugon)
    }

    #[test]
    fn an_orc_of_level_10_starts_and_gets_the_first_order() {
        let (mut t, hatos, _) = setup();
        t.enter_as(1, 10, race::ORC, player_class::ORC_FIGHTER, vec![]);
        assert!(t.quest_page(1, hatos).contains("30568-03.htm"));
        assert!(t.event(1, NAME, "30568-04.htm").contains("30568-04.htm"));
        assert_eq!(t.count(1, ORDER1), 1);
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
    }

    #[test]
    fn the_start_is_refused_below_10_and_for_other_races() {
        let (mut t, hatos, _) = setup();
        t.enter_as(1, 9, race::ORC, player_class::ORC_FIGHTER, vec![]);
        assert!(t.quest_page(1, hatos).contains("30568-02.htm"));
        t.enter_as(2, 20, race::HUMAN, player_class::FIGHTER, vec![]);
        assert!(t.quest_page(2, hatos).contains("30568-01.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_three_orders_and_their_letters_then_the_reward() {
        let (mut t, hatos, parugon) = setup();
        t.enter_as(1, 20, race::ORC, player_class::ORC_FIGHTER, vec![]);
        assert!(t.quest_page(1, hatos).contains("30568-03.htm"));
        t.event(1, NAME, "30568-04.htm");
        t.take_writes();

        // Parugon: cond 2.
        assert!(t.quest_page(1, parugon).contains("30580-01.html"));
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);

        // A messenger with order 1 drops the letter to the human: cond 3.
        let messenger = t.spawn(BARANKA, 60);
        t.kill(1, messenger);
        assert_eq!(t.count(1, TO_HUMAN), 1);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "3")]);

        // Hatos: order 1 becomes order 2 at cond 4 (the page 08 is a link of 06).
        assert!(t.quest_page(1, hatos).contains("30568-06.html"));
        assert!(t.event(1, NAME, "30568-08.html").contains("30568-08.html"));
        assert_eq!((t.count(1, ORDER1), t.count(1, ORDER2)), (0, 1));
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "4")]);

        // A messenger with order 2 drops the letter to the dark elf: cond 5.
        let messenger = t.spawn(BARANKA, 60);
        t.kill(1, messenger);
        assert_eq!(t.count(1, TO_DARK_ELF), 1);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "5")]);

        // Hatos: order 2 becomes order 3 at cond 6.
        assert!(t.quest_page(1, hatos).contains("30568-09.html"));
        assert!(t.event(1, NAME, "30568-10.html").contains("30568-10.html"));
        assert_eq!((t.count(1, ORDER2), t.count(1, ORDER3)), (0, 1));
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "6")]);

        // A messenger with order 3 drops the letter to the elf: cond 7.
        let messenger = t.spawn(BARANKA, 60);
        t.kill(1, messenger);
        assert_eq!(t.count(1, TO_ELF), 1);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "7")]);

        // Hatos: the reward (the Butcher), the exp and the exit.
        assert!(t.quest_page(1, hatos).contains("30568-11.html"));
        assert_eq!(t.count(1, 1510), 1);
        assert_eq!(t.count(1, 1060), 100);
        assert_eq!(t.count(1, 57), 14666);
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed")
            ]
        );
    }

    #[test]
    fn giving_up_pays_200_adena_and_leaves_the_quest_repeatable() {
        let (mut t, hatos, _) = setup();
        t.enter_as(1, 20, race::ORC, player_class::ORC_FIGHTER, vec![]);
        assert!(t.quest_page(1, hatos).contains("30568-03.htm"));
        t.event(1, NAME, "30568-04.htm");
        t.take_writes();
        // Cond 3 with the order and the human's letter: page 06 offers the give-up link.
        t.world.run_as_script(NAME, |ctx| {
            let qs = ctx.get_quest_state(Oid(1), false).unwrap();
            qs.set_cond(ctx, 3, false);
            ctx.give_items(Oid(1), LETTER_TO_HUMAN, 1);
        });
        t.take_writes();
        assert!(t.quest_page(1, hatos).contains("30568-06.html"));
        assert!(t.event(1, NAME, "30568-07.html").contains("30568-07.html"));
        assert_eq!(t.count(1, 57), 200);
        assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
    }
}
