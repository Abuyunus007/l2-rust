//! Quest 105: Skirmish with Orcs (Kendnell gives one of four Kendell's orders; the Kaboo chiefs
//! give torques, and the last torque brings the reward).
// Java: dist/game/data/scripts/quests/Q00105_SkirmishWithOrcs/Q00105_SkirmishWithOrcs.java
//
// Pattern: a kill chain by orders (`MONSTER_DROP`), a random order from a range (`getRandom(min,
// max)`, inclusive), and the NewbieGuide memo as in quest 104.

use l2_protocol_game::NpcStringId;
use l2_protocol_game::server::SocialAction;
use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, STARTED, race};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const KENDNELL: i32 = 30218;

// Items
const KENDELLS_1ST_ORDER: i32 = 1836;
const KENDELLS_2ND_ORDER: i32 = 1837;
const KENDELLS_3RD_ORDER: i32 = 1838;
const KENDELLS_4TH_ORDER: i32 = 1839;
const KENDELLS_5TH_ORDER: i32 = 1840;
const KENDELLS_6TH_ORDER: i32 = 1841;
const KENDELLS_7TH_ORDER: i32 = 1842;
const KENDELLS_8TH_ORDER: i32 = 1843;
const KABOO_CHIEFS_1ST_TORQUE: i32 = 1844;
const KABOO_CHIEFS_2ST_TORQUE: i32 = 1845;

/// Java `MONSTER_DROP`: the Kaboo chief -> the order it is killed for.
const MONSTER_DROP: [(i32, i32); 8] = [
    (27059, KENDELLS_1ST_ORDER), // Uoph (Kaboo Chief)
    (27060, KENDELLS_2ND_ORDER), // Kracha (Kaboo Chief)
    (27061, KENDELLS_3RD_ORDER), // Batoh (Kaboo Chief)
    (27062, KENDELLS_4TH_ORDER), // Tanukia (Kaboo Chief)
    (27064, KENDELLS_5TH_ORDER), // Turel (Kaboo Chief)
    (27065, KENDELLS_6TH_ORDER), // Roko (Kaboo Chief)
    (27067, KENDELLS_7TH_ORDER), // Kamut (Kaboo Chief)
    (27068, KENDELLS_8TH_ORDER), // Murtika (Kaboo Chief)
];

const KENDNELLS_ORDERS: [i32; 8] = [
    KENDELLS_1ST_ORDER,
    KENDELLS_2ND_ORDER,
    KENDELLS_3RD_ORDER,
    KENDELLS_4TH_ORDER,
    KENDELLS_5TH_ORDER,
    KENDELLS_6TH_ORDER,
    KENDELLS_7TH_ORDER,
    KENDELLS_8TH_ORDER,
];

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
const SOULSHOTS_NO_GRADE_FOR_ROOKIES: (i32, i64) = (5789, 7000);
const SOULSHOTS_NO_GRADE: (i32, i64) = (1835, 1000);
const SPIRITSHOTS_NO_GRADE: (i32, i64) = (2509, 500);
const RED_SUNSET_SWORD: (i32, i64) = (981, 1);
const RED_SUNSET_STAFF: (i32, i64) = (754, 1);

// Misc
const MIN_LEVEL: i32 = 10;
const GUIDE_MISSION: i32 = 41;
/// Java `NewbieGuide.class.getSimpleName()`.
const NEWBIE_GUIDE: &str = "NewbieGuide";

/// Java `MONSTER_DROP.get(npcId)`: the order a Kaboo chief is killed for (registered kills only).
fn drop_of(npc_id: i32) -> Option<i32> {
    MONSTER_DROP
        .iter()
        .find(|(monster, _)| *monster == npc_id)
        .map(|(_, order)| *order)
}

/// Java `Q00105_SkirmishWithOrcs`.
pub struct Q00105SkirmishWithOrcs;

impl Script for Q00105SkirmishWithOrcs {
    fn name(&self) -> &'static str {
        "Q00105_SkirmishWithOrcs"
    }

    fn path(&self) -> &'static str {
        "quests/Q00105_SkirmishWithOrcs"
    }

    fn id(&self) -> i32 {
        105
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[KENDNELL]);
        s.add_talk_id(&[KENDNELL]);
        let monsters: Vec<i32> = MONSTER_DROP.iter().map(|(id, _)| *id).collect();
        s.add_kill_id(&monsters);
        s.register_quest_items(&KENDNELLS_ORDERS);
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
            "30218-04.html" => {
                if qs.is_created(ctx) {
                    qs.start_quest(ctx);
                    // Java `getRandom(0, 3)`: inclusive.
                    let index = ctx.get_random_range(0, 3);
                    ctx.give_items(player, KENDNELLS_ORDERS[index as usize], 1);
                    return Some(event.to_owned());
                }
                None
            }
            "30218-05.html" => Some(event.to_owned()),
            _ => None,
        }
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        let Some(qs) = ctx.get_quest_state(killer, false) else {
            return;
        };
        if !ctx.check_if_in_range(ctx.alt_party_range(), npc.object_id, killer, true) {
            return;
        }
        let Some(order) = drop_of(npc.id) else {
            return;
        };
        match npc.id {
            27059..=27062 => {
                if qs.is_cond(ctx, 1) && ctx.has_quest_items(killer, order) {
                    ctx.give_items(killer, KABOO_CHIEFS_1ST_TORQUE, 1);
                    qs.set_cond(ctx, 2, true);
                }
            }
            27064 | 27065 | 27067 | 27068 => {
                if qs.is_cond(ctx, 3) && ctx.has_quest_items(killer, order) {
                    ctx.give_items(killer, KABOO_CHIEFS_2ST_TORQUE, 1);
                    qs.set_cond(ctx, 4, true);
                }
            }
            _ => {}
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, talker: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(talker, true)?;
        let mut htmltext = ctx.get_no_quest_msg(talker);
        match qs.state(ctx) {
            CREATED => {
                htmltext = if ctx.player_race(talker) == race::ELF {
                    if ctx.player_level(talker) >= MIN_LEVEL {
                        "30218-03.htm"
                    } else {
                        "30218-02.htm"
                    }
                } else {
                    "30218-01.htm"
                }
                .to_owned();
            }
            STARTED => {
                if ctx.has_at_least_one_quest_item(
                    talker,
                    &[
                        KENDELLS_1ST_ORDER,
                        KENDELLS_2ND_ORDER,
                        KENDELLS_3RD_ORDER,
                        KENDELLS_4TH_ORDER,
                    ],
                ) {
                    htmltext = "30218-06.html".to_owned();
                }
                if qs.is_cond(ctx, 2) && ctx.has_quest_items(talker, KABOO_CHIEFS_1ST_TORQUE) {
                    for order in &KENDNELLS_ORDERS[0..4] {
                        ctx.take_items(talker, *order, -1);
                    }
                    ctx.take_items(talker, KABOO_CHIEFS_1ST_TORQUE, 1);
                    // Java `getRandom(4, 7)`: inclusive.
                    let index = ctx.get_random_range(4, 7);
                    ctx.give_items(talker, KENDNELLS_ORDERS[index as usize], 1);
                    qs.set_cond(ctx, 3, true);
                    htmltext = "30218-07.html".to_owned();
                }
                if ctx.has_at_least_one_quest_item(
                    talker,
                    &[
                        KENDELLS_5TH_ORDER,
                        KENDELLS_6TH_ORDER,
                        KENDELLS_7TH_ORDER,
                        KENDELLS_8TH_ORDER,
                    ],
                ) {
                    htmltext = "30218-08.html".to_owned();
                }
                if qs.is_cond(ctx, 4) && ctx.has_quest_items(talker, KABOO_CHIEFS_2ST_TORQUE) {
                    for (id, count) in REWARDS {
                        ctx.give_items(talker, id, count);
                    }
                    if !ctx.is_mage_class(talker) {
                        let (id, count) = SOULSHOTS_NO_GRADE;
                        ctx.give_items(talker, id, count);
                    } else {
                        let (id, count) = SPIRITSHOTS_NO_GRADE;
                        ctx.give_items(talker, id, count);
                    }
                    if !ctx.is_mage_class(talker) && !qs.is_completed(ctx) {
                        let (id, count) = RED_SUNSET_SWORD;
                        ctx.give_items(talker, id, count);
                    } else if !qs.is_completed(ctx) {
                        let (id, count) = RED_SUNSET_STAFF;
                        ctx.give_items(talker, id, count);
                    }
                    if ctx.player_level(talker) < 25 {
                        if ctx.is_mage_class(talker) {
                            let (id, count) = SPIRITSHOTS_NO_GRADE_FOR_ROOKIES;
                            ctx.give_items(talker, id, count);
                            ctx.play_sound_named(talker, "tutorial_voice_027");
                        } else {
                            let (id, count) = SOULSHOTS_NO_GRADE_FOR_ROOKIES;
                            ctx.give_items(talker, id, count);
                            ctx.play_sound_named(talker, "tutorial_voice_026");
                        }
                    }
                    ctx.world().send(
                        talker,
                        &SocialAction {
                            object_id: talker.0,
                            action_id: 3,
                        },
                    );
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
                        } else if (ctx.get_nr_memo_state(guide, GUIDE_MISSION) % 1_000_000) / 100_000 != 1 {
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
                    ctx.give_adena(talker, 17599, true);
                    ctx.add_exp_and_sp(talker, 41478, 3555);
                    qs.exit_quest(ctx, false, true);
                    htmltext = "30218-09.html".to_owned();
                }
            }
            COMPLETED => htmltext = ctx.get_already_completed_msg(talker),
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

    use super::{KENDNELL, KENDNELLS_ORDERS, Q00105SkirmishWithOrcs as Quest};
    use crate::testing::{Pages, ScriptTest, rows};

    const NAME: &str = "Q00105_SkirmishWithOrcs";
    const TORQUE1: i32 = 1844;
    const TORQUE2: i32 = 1845;

    const NPCS: &str = r#"
        <npc id="30218" type="Folk" level="70" name="Kendnell"/>
        <npc id="27059" type="Monster" level="76" name="Uoph"/>
        <npc id="27060" type="Monster" level="76" name="Kracha"/>
        <npc id="27061" type="Monster" level="76" name="Batoh"/>
        <npc id="27062" type="Monster" level="76" name="Tanukia"/>
        <npc id="27064" type="Monster" level="76" name="Turel"/>
        <npc id="27065" type="Monster" level="76" name="Roko"/>
        <npc id="27067" type="Monster" level="76" name="Kamut"/>
        <npc id="27068" type="Monster" level="76" name="Murtika"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="1836" type="EtcItem" name="Kendell's 1st Order"><set name="is_questitem" val="true"/></item>
        <item id="1837" type="EtcItem" name="Kendell's 2nd Order"><set name="is_questitem" val="true"/></item>
        <item id="1838" type="EtcItem" name="Kendell's 3rd Order"><set name="is_questitem" val="true"/></item>
        <item id="1839" type="EtcItem" name="Kendell's 4th Order"><set name="is_questitem" val="true"/></item>
        <item id="1840" type="EtcItem" name="Kendell's 5th Order"><set name="is_questitem" val="true"/></item>
        <item id="1841" type="EtcItem" name="Kendell's 6th Order"><set name="is_questitem" val="true"/></item>
        <item id="1842" type="EtcItem" name="Kendell's 7th Order"><set name="is_questitem" val="true"/></item>
        <item id="1843" type="EtcItem" name="Kendell's 8th Order"><set name="is_questitem" val="true"/></item>
        <item id="1844" type="EtcItem" name="Kaboo Chief's 1st Torque"><set name="is_questitem" val="true"/></item>
        <item id="1845" type="EtcItem" name="Kaboo Chief's 2nd Torque"><set name="is_questitem" val="true"/></item>
        <item id="1060" type="EtcItem" name="Lesser Healing Potion"><set name="is_stackable" val="true"/></item>
        <item id="981" type="Weapon" name="Red Sunset Sword"/>"#;

    fn pages() -> Pages {
        Pages::of(&Quest)
            .page("30218-03.htm", &["30218-04.html"])
            .page("30218-04.html", &["30218-05.html"])
            .plain(&[
                "30218-01.htm",
                "30218-02.htm",
                "30218-05.html",
                "30218-06.html",
                "30218-07.html",
                "30218-08.html",
                "30218-09.html",
            ])
            .npcs(&[KENDNELL])
    }

    /// The Kaboo chief that is killed for the order `order` (Java `MONSTER_DROP`).
    fn chief_for(order: i32) -> i32 {
        match order {
            1836 => 27059,
            1837 => 27060,
            1838 => 27061,
            1839 => 27062,
            1840 => 27064,
            1841 => 27065,
            1842 => 27067,
            _ => 27068,
        }
    }

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Quest);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages());
        let kendnell = t.spawn(KENDNELL, 20);
        (t, kendnell)
    }

    /// The order a started character holds (one of `range`), by item id.
    fn order_in(t: &ScriptTest, range: std::ops::Range<usize>) -> i32 {
        KENDNELLS_ORDERS[range]
            .iter()
            .copied()
            .find(|id| t.count(1, *id) == 1)
            .expect("an order of the range")
    }

    #[test]
    fn an_elf_of_level_10_gets_an_order_and_the_chain_starts() {
        let (mut t, kendnell) = setup();
        t.enter_as(1, 10, race::ELF, player_class::ELVEN_FIGHTER, vec![]);
        assert!(t.quest_page(1, kendnell).contains("30218-03.htm"));
        assert!(t.event(1, NAME, "30218-04.html").contains("30218-04.html"));
        let first = order_in(&t, 0..4);
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );

        // The Kaboo chief for that order gives the first torque and cond 2.
        let chief = t.spawn(chief_for(first), 60);
        t.kill(1, chief);
        assert_eq!(t.count(1, TORQUE1), 1);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);
    }

    #[test]
    fn the_start_is_refused_for_other_races_and_low_levels() {
        let (mut t, kendnell) = setup();
        t.enter_as(1, 9, race::ELF, player_class::ELVEN_FIGHTER, vec![]);
        assert!(t.quest_page(1, kendnell).contains("30218-02.htm"));
        t.enter_as(2, 20, race::HUMAN, player_class::FIGHTER, vec![]);
        assert!(t.quest_page(2, kendnell).contains("30218-01.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_second_order_and_the_reward() {
        let (mut t, kendnell) = setup();
        t.enter_as(1, 20, race::ELF, player_class::ELVEN_FIGHTER, vec![]);
        // Cond 2 with the first torque: the first four orders go, a fifth to eighth one comes.
        t.world.run_as_script(NAME, |ctx| {
            let qs = ctx.get_quest_state(Oid(1), true).unwrap();
            qs.start_quest(ctx);
            ctx.give_items(Oid(1), TORQUE1, 1);
            qs.set_cond(ctx, 2, false);
        });
        ctx_give_order(&mut t, 1836);
        t.take_writes();
        // The new order is one of 5th to 8th, so Java's last `if` (08) replaces the 07 page.
        assert!(t.quest_page(1, kendnell).contains("30218-08.html"));
        assert_eq!(t.count(1, TORQUE1), 0);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "3")]);
        let second = order_in(&t, 4..8);
        let chief = t.spawn(chief_for(second), 60);
        t.kill(1, chief);
        assert_eq!(t.count(1, TORQUE2), 1);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "4")]);

        // Cond 4 with the second torque: the reward, the exp and the exit.
        assert!(t.quest_page(1, kendnell).contains("30218-09.html"));
        assert_eq!(t.count(1, 1060), 100);
        assert_eq!(t.count(1, 981), 1);
        assert_eq!(t.count(1, 57), 17599);
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed")
            ]
        );
    }

    /// Gives the order `id` (the test does not roll the start).
    fn ctx_give_order(t: &mut ScriptTest, id: i32) {
        t.world.run_as_script(NAME, |ctx| {
            ctx.give_items(Oid(1), id, 1);
        });
    }

    #[test]
    fn a_finished_character_gets_the_completed_page() {
        let (mut t, kendnell) = setup();
        t.enter_as(1, 20, race::ELF, player_class::ELVEN_FIGHTER, vec![]);
        t.world.run_as_script(NAME, |ctx| {
            let qs = ctx.get_quest_state(Oid(1), true).unwrap();
            qs.start_quest(ctx);
            qs.exit_quest(ctx, false, false);
        });
        t.take_writes();
        assert!(t.quest_page(1, kendnell).contains("already been completed"));
    }
}
