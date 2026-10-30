//! Quest 175: The Way of the Warrior (Kekropus and Perwan in the Kamael village; the Mountain Werewolf and the Muertos).
// Java: dist/game/data/scripts/quests/Q00175_TheWayOfTheWarrior/Q00175_TheWayOfTheWarrior.java

use l2_protocol_game::NpcStringId;
use l2_protocol_game::server::SocialAction;
use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, STARTED, race};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup, SkillKey};

// NPCs
const PERWAN: i32 = 32133;
const KEKROPUS: i32 = 32138;

// Items (Java `ItemHolder`: id and count)
const WOLF_TAIL: (i32, i64) = (9807, 5);
const MUERTOS_CLAW: (i32, i64) = (9808, 10);

// Buff: `UNSEALED_ALTAR`
const UNSEALED_ALTAR: SkillKey = SkillKey { id: 4549, level: 1 };

// Rewards
const WARRIORS_SWORD: i32 = 9720;
const SOULSHOTS_NO_GRADE_FOR_ROOKIES: (i32, i64) = (5789, 7000);
const REWARDS: [(i32, i64); 6] = [
    (1060, 100), // Lesser Healing Potion
    (4412, 10),  // Echo Crystal - Theme of Battle
    (4413, 10),  // Echo Crystal - Theme of Love
    (4414, 10),  // Echo Crystal - Theme of Solitude
    (4415, 10),  // Echo Crystal - Theme of Feast
    (4416, 10),  // Echo Crystal - Theme of Celebration
];

// Monsters
const MOUNTAIN_WEREWOLF: i32 = 22235;
const MONSTERS: [i32; 7] = [
    22236, // Muertos Archer
    22239, // Muertos Guard
    22240, // Muertos Scout
    22242, // Muertos Warrior
    22243, // Muertos Captain
    22245, // Muertos Lieutenant
    22246, // Muertos Commander
];

// Misc
const MIN_LEVEL: i32 = 10;
const GUIDE_MISSION: i32 = 41;
/// Java `NewbieGuide.class.getSimpleName()`.
const NEWBIE_GUIDE: &str = "NewbieGuide";

/// Java `hasItem(player, ItemHolder)`: the count is at least the holder's count.
fn has_holder(ctx: &ScriptCtx<'_>, player: ObjectId, holder: (i32, i64)) -> bool {
    ctx.get_quest_items_count(player, holder.0) >= holder.1
}

/// Java `Q00175_TheWayOfTheWarrior`.
pub struct Q00175TheWayOfTheWarrior;

impl Script for Q00175TheWayOfTheWarrior {
    fn name(&self) -> &'static str {
        "Q00175_TheWayOfTheWarrior"
    }

    fn path(&self) -> &'static str {
        "quests/Q00175_TheWayOfTheWarrior"
    }

    fn id(&self) -> i32 {
        175
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[KEKROPUS]);
        s.add_talk_id(&[KEKROPUS, PERWAN]);
        s.add_kill_id(&[MOUNTAIN_WEREWOLF]);
        s.add_kill_id(&MONSTERS);
        s.register_quest_items(&[WOLF_TAIL.0, MUERTOS_CLAW.0]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        let player = player?;
        let qs = ctx.get_quest_state(player, false)?;

        let mut htmltext = None;
        match event {
            "32138-02.htm" => htmltext = Some(event.to_owned()),
            "32138-05.htm" => {
                if qs.is_created(ctx) {
                    qs.start_quest(ctx);
                    qs.set_memo_state(ctx, 1);
                    htmltext = Some(event.to_owned());
                }
            }
            "32138-10.html" => {
                qs.set_memo_state(ctx, 6);
                qs.set_cond(ctx, 7, true);
                htmltext = Some(event.to_owned());
            }
            "32138-13.html" => {
                if has_holder(ctx, player, MUERTOS_CLAW) {
                    ctx.take_items(player, MUERTOS_CLAW.0, MUERTOS_CLAW.1);
                    ctx.give_items(player, WARRIORS_SWORD, 1);
                    if ctx.player_level(player) < 25 {
                        ctx.play_sound_named(player, "tutorial_voice_026");
                        ctx.give_items(
                            player,
                            SOULSHOTS_NO_GRADE_FOR_ROOKIES.0,
                            SOULSHOTS_NO_GRADE_FOR_ROOKIES.1,
                        );
                    }
                    // Newbie Guide: the mission counter of the guide, when it is in the game.
                    if let Some(guide) = ctx.quest_state_of(NEWBIE_GUIDE, player, true) {
                        if !ctx.have_nr_memo(guide, GUIDE_MISSION) {
                            ctx.set_nr_memo(guide, GUIDE_MISSION);
                            ctx.set_nr_memo_state(guide, GUIDE_MISSION, 100_000);
                            ctx.show_on_screen_npc_string(
                                player,
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
                                player,
                                NpcStringId::AcquisitionOfRaceSpecificWeaponCompleteNGoFindTheNewbieGuide,
                                2,
                                5000,
                                &[],
                            );
                        }
                    }
                    ctx.add_exp_and_sp(player, 20_739, 1777);
                    ctx.give_adena(player, 8799, true);
                    for (item_id, count) in REWARDS {
                        ctx.give_items(player, item_id, count);
                    }
                    qs.exit_quest(ctx, false, true);
                    ctx.world().send(
                        player,
                        &SocialAction {
                            object_id: player.0,
                            action_id: 3,
                        },
                    );
                    htmltext = Some(event.to_owned());
                }
            }
            "32133-06.html" => {
                qs.set_memo_state(ctx, 5);
                qs.set_cond(ctx, 6, true);
                if let Some(npc) = npc {
                    ctx.cast_skill(npc, player, UNSEALED_ALTAR);
                }
                htmltext = Some(event.to_owned());
            }
            _ => {}
        }
        htmltext
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        if npc.id == MOUNTAIN_WEREWOLF {
            if let Some(qs) = ctx.random_party_member_state(killer, 2, 3, Some(npc))
                && ctx.give_item_randomly(qs.player(), Some(npc), WOLF_TAIL.0, 1, WOLF_TAIL.1, 0, 0.5, true)
            {
                qs.set_cond(ctx, 3, true);
            }
        } else if let Some(qs) = ctx.random_party_member_state(killer, 7, 3, Some(npc))
            && ctx.give_item_randomly(
                qs.player(),
                Some(npc),
                MUERTOS_CLAW.0,
                1,
                MUERTOS_CLAW.1,
                1,
                1.0,
                true,
            )
        {
            qs.set_cond(ctx, 8, true);
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        match npc.id {
            KEKROPUS => match qs.state(ctx) {
                CREATED => {
                    htmltext = if ctx.player_race(player) != race::KAMAEL {
                        "32138-04.htm"
                    } else if ctx.player_level(player) >= MIN_LEVEL {
                        "32138-01.htm"
                    } else {
                        "32138-03.htm"
                    }
                    .to_owned();
                }
                STARTED => match qs.get_cond(ctx) {
                    1..=3 => htmltext = "32138-06.html".to_owned(),
                    4 => {
                        qs.set_memo_state(ctx, 4);
                        qs.set_cond(ctx, 5, true);
                        htmltext = "32138-07.html".to_owned();
                    }
                    5 => htmltext = "32138-08.html".to_owned(),
                    6 => htmltext = "32138-09.html".to_owned(),
                    7 => htmltext = "32138-11.html".to_owned(),
                    8 => {
                        if has_holder(ctx, player, MUERTOS_CLAW) {
                            htmltext = "32138-12.html".to_owned();
                        }
                    }
                    _ => {}
                },
                COMPLETED => htmltext = ctx.get_already_completed_msg(player),
                _ => {}
            },
            PERWAN => match qs.get_cond(ctx) {
                1 => {
                    qs.set_memo_state(ctx, 2);
                    qs.set_cond(ctx, 2, true);
                    htmltext = "32133-01.html".to_owned();
                }
                2 => htmltext = "32133-02.html".to_owned(),
                3 => {
                    if has_holder(ctx, player, WOLF_TAIL) {
                        ctx.take_items(player, WOLF_TAIL.0, WOLF_TAIL.1);
                        qs.set_memo_state(ctx, 3);
                        qs.set_cond(ctx, 4, true);
                        htmltext = "32133-03.html".to_owned();
                    }
                }
                4 => htmltext = "32133-04.html".to_owned(),
                5 => htmltext = "32133-05.html".to_owned(),
                6 => htmltext = "32133-07.html".to_owned(),
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
    use l2_world::script::race;

    use super::*;
    use crate::testing::{Pages, ScriptTest, names_to, rows};

    const NAME: &str = "Q00175_TheWayOfTheWarrior";

    const NPCS: &str = r#"
        <npc id="32133" type="Folk" level="70" name="Perwan"/>
        <npc id="32138" type="Folk" level="70" name="Kekropus"/>
        <npc id="22235" type="Monster" level="30" name="Mountain Werewolf"/>
        <npc id="22236" type="Monster" level="30" name="Muertos Archer"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="9807" type="EtcItem" name="Wolf Tail"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="9808" type="EtcItem" name="Muertos Claw"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="9720" type="Weapon" name="Warrior's Sword"><set name="bodypart" val="rhand"/></item>
        <item id="5789" type="EtcItem" name="Soulshot D for Rookies"><set name="is_stackable" val="true"/></item>
        <item id="1060" type="EtcItem" name="Lesser Healing Potion"><set name="is_stackable" val="true"/></item>
        <item id="4412" type="EtcItem" name="Echo Crystal A"><set name="is_stackable" val="true"/></item>
        <item id="4413" type="EtcItem" name="Echo Crystal B"><set name="is_stackable" val="true"/></item>
        <item id="4414" type="EtcItem" name="Echo Crystal C"><set name="is_stackable" val="true"/></item>
        <item id="4415" type="EtcItem" name="Echo Crystal D"><set name="is_stackable" val="true"/></item>
        <item id="4416" type="EtcItem" name="Echo Crystal E"><set name="is_stackable" val="true"/></item>"#;

    /// Java `NewbieGuide` as far as this quest writes it: a script of that name.
    struct NewbieGuide;

    impl Script for NewbieGuide {
        fn name(&self) -> &'static str {
            NEWBIE_GUIDE
        }
        fn path(&self) -> &'static str {
            "ai/others/NewbieGuide"
        }
        fn setup(&self, _setup: &mut ScriptSetup) {}
    }

    struct Npcs {
        kekropus: Oid,
        perwan: Oid,
    }

    fn setup() -> (ScriptTest, Npcs) {
        let mut registry = ScriptRegistry::new();
        registry.add(NewbieGuide);
        registry.add(Q00175TheWayOfTheWarrior);
        let pages = Pages::of(&Q00175TheWayOfTheWarrior)
            .page("32138-01.htm", &["32138-02.htm"])
            .page("32138-02.htm", &["32138-05.htm"])
            .plain(&["32138-03.htm", "32138-04.htm", "32138-05.htm"])
            .plain(&["32138-06.html", "32138-07.html", "32138-08.html"])
            .page("32138-09.html", &["32138-10.html"])
            .plain(&["32138-10.html", "32138-11.html"])
            .page("32138-12.html", &["32138-13.html"])
            .plain(&["32138-13.html"])
            .plain(&[
                "32133-01.html",
                "32133-02.html",
                "32133-03.html",
                "32133-04.html",
                "32133-07.html",
            ])
            .page("32133-05.html", &["32133-06.html"])
            .plain(&["32133-06.html"])
            .npcs(&[KEKROPUS, PERWAN]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let npcs = Npcs {
            kekropus: t.spawn(KEKROPUS, 20),
            perwan: t.spawn(PERWAN, 40),
        };
        (t, npcs)
    }

    /// Gives the player the missing items up to the holder's count (the drops are random).
    fn give_up_to(t: &mut ScriptTest, holder: (i32, i64)) {
        let missing = holder.1 - t.count(1, holder.0);
        if missing > 0 {
            t.world.run_as_script(NAME, |ctx| {
                ctx.give_items(Oid(1), holder.0, missing);
            });
        }
    }

    #[test]
    fn the_kamael_of_level_10_is_offered_the_quest() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 9, race::KAMAEL, 123, vec![]);
        assert!(t.quest_page(1, npcs.kekropus).contains("32138-03.htm"));
        t.enter_as(2, 10, race::KAMAEL, 123, vec![]);
        assert!(t.quest_page(2, npcs.kekropus).contains("32138-01.htm"));
        t.enter_as(3, 10, race::HUMAN, 0, vec![]);
        assert!(t.quest_page(3, npcs.kekropus).contains("32138-04.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_werewolf_tails_the_muertos_claws_and_the_reward() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 10, race::KAMAEL, 123, vec![]);
        assert!(t.quest_page(1, npcs.kekropus).contains("32138-01.htm"));
        assert!(t.event(1, NAME, "32138-02.htm").contains("32138-02.htm"));
        assert!(t.event(1, NAME, "32138-05.htm").contains("32138-05.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started"),
                rows::insert(NAME, "memoState", "1"),
            ]
        );

        // Perwan: memo state 2 and cond 2.
        assert!(t.quest_page(1, npcs.perwan).contains("32133-01.html"));
        assert_eq!(
            t.take_writes(),
            [
                rows::update(NAME, "memoState", "2"),
                rows::update(NAME, "cond", "2")
            ]
        );

        // The werewolf drops the tails (cond 3 with the first drop).
        for _ in 0..200 {
            if t.count(1, WOLF_TAIL.0) > 0 {
                break;
            }
            let wolf = t.spawn(MOUNTAIN_WEREWOLF, 60);
            t.kill(1, wolf);
        }
        assert!(t.count(1, WOLF_TAIL.0) > 0);
        assert!(t.take_writes().contains(&rows::update(NAME, "cond", "3")));

        // Five tails for Perwan (memo state 3, cond 4, the five go).
        give_up_to(&mut t, WOLF_TAIL);
        t.take_writes();
        assert!(t.quest_page(1, npcs.perwan).contains("32133-03.html"));
        assert_eq!(t.count(1, WOLF_TAIL.0), 0);
        assert_eq!(
            t.take_writes(),
            [
                rows::update(NAME, "memoState", "3"),
                rows::update(NAME, "cond", "4")
            ]
        );

        // Kekropus: memo state 4, cond 5; Perwan sends the player on (memo state 5, cond 6, the altar).
        assert!(t.quest_page(1, npcs.kekropus).contains("32138-07.html"));
        assert_eq!(
            t.take_writes(),
            [
                rows::update(NAME, "memoState", "4"),
                rows::update(NAME, "cond", "5")
            ]
        );
        assert!(t.quest_page(1, npcs.perwan).contains("32133-05.html"));
        assert!(t.event(1, NAME, "32133-06.html").contains("32133-06.html"));
        assert_eq!(
            t.take_writes(),
            [
                rows::update(NAME, "memoState", "5"),
                rows::update(NAME, "cond", "6")
            ]
        );

        // Kekropus: the event to cond 7 (memo state 6).
        assert!(t.quest_page(1, npcs.kekropus).contains("32138-09.html"));
        assert!(t.event(1, NAME, "32138-10.html").contains("32138-10.html"));
        assert_eq!(
            t.take_writes(),
            [
                rows::update(NAME, "memoState", "6"),
                rows::update(NAME, "cond", "7")
            ]
        );
        assert!(t.quest_page(1, npcs.kekropus).contains("32138-11.html"));

        // A Muertos drops the first claw (cond 8); ten claws in all for Kekropus.
        let muertos = t.spawn(MONSTERS[0], 60);
        t.kill(1, muertos);
        assert!(t.count(1, MUERTOS_CLAW.0) >= 1);
        assert!(t.take_writes().contains(&rows::update(NAME, "cond", "8")));
        give_up_to(&mut t, MUERTOS_CLAW);
        t.take_writes();
        assert!(t.quest_page(1, npcs.kekropus).contains("32138-12.html"));
        t.bypass(1, &format!("Script {NAME} 32138-13.html"));

        // The sword, the rookie soulshots, the guide's mission, the rewards, `exitQuest(false, true)`.
        assert_eq!(t.count(1, WARRIORS_SWORD), 1);
        assert_eq!(
            t.count(1, SOULSHOTS_NO_GRADE_FOR_ROOKIES.0),
            SOULSHOTS_NO_GRADE_FOR_ROOKIES.1
        );
        assert_eq!(t.count(1, 57), 8799);
        assert_eq!(t.count(1, MUERTOS_CLAW.0), 0);
        for (item_id, count) in REWARDS {
            assert_eq!(t.count(1, item_id), count, "{item_id}");
        }
        let out = t.take();
        assert!(names_to(&out, 1).contains(&"SOCIAL_ACTION".to_owned()));
        let writes = t.take_writes();
        let guide = |var: &str, value: &str| rows::insert(NEWBIE_GUIDE, var, value);
        assert_eq!(
            writes[..2],
            [guide("NRmemo", "41"), guide("NRmemoState41", "100000")]
        );
        assert_eq!(
            writes[2..],
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed")
            ]
        );
        assert!(t.quest_page(1, npcs.kekropus).contains("already been completed"));
    }
}
