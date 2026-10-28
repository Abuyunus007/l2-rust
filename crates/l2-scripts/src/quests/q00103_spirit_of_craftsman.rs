//! Quest 103: Spirit of Craftsman (Karoyd, a dark elf, sends the letter through Cecon and Harne,
//! the monsters give the bones and the soul catcher, and the steelbender's head ends it).
// Java: dist/game/data/scripts/quests/Q00103_SpiritOfCraftsman/Q00103_SpiritOfCraftsman.java
//
// Pattern: a monster drop chain where the kill goes to a random party member (`onKill` uses the
// member's state and player, `getRandomPartyMemberState`), with the NewbieGuide memo as in
// quest 104.

use l2_protocol_game::NpcStringId;
use l2_protocol_game::server::SocialAction;
use l2_util::ObjectId;
use l2_world::script::race;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const BLACKSMITH_KAROYD: i32 = 30307;
const CECON: i32 = 30132;
const HARNE: i32 = 30144;

// Items
const KAROYDS_LETTER: i32 = 968;
const CECKTINONS_VOUCHER1: i32 = 969;
const CECKTINONS_VOUCHER2: i32 = 970;
const SOUL_CATCHER: i32 = 971;
const PRESERVE_OIL: i32 = 972;
const ZOMBIE_HEAD: i32 = 973;
const STEELBENDERS_HEAD: i32 = 974;
const BONE_FRAGMENT: i32 = 1107;

// Monsters
const MARSH_ZOMBIE: i32 = 20015;
const DOOM_SOLDIER: i32 = 20455;
const SKELETON_HUNTER: i32 = 20517;
const SKELETON_HUNTER_ARCHER: i32 = 20518;

// Rewards
const BLOODSABER: i32 = 975;

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
const SOULSHOTS_NO_GRADE: (i32, i64) = (1835, 1000);
const SPIRITSHOTS_NO_GRADE: (i32, i64) = (2509, 500);

// Misc
const MIN_LEVEL: i32 = 10;
const GUIDE_MISSION: i32 = 41;
/// Java `NewbieGuide.class.getSimpleName()`.
const NEWBIE_GUIDE: &str = "NewbieGuide";

/// Java `Q00103_SpiritOfCraftsman`.
pub struct Q00103SpiritOfCraftsman;

impl Script for Q00103SpiritOfCraftsman {
    fn name(&self) -> &'static str {
        "Q00103_SpiritOfCraftsman"
    }

    fn path(&self) -> &'static str {
        "quests/Q00103_SpiritOfCraftsman"
    }

    fn id(&self) -> i32 {
        103
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[BLACKSMITH_KAROYD]);
        s.add_talk_id(&[BLACKSMITH_KAROYD, CECON, HARNE]);
        s.add_kill_id(&[
            MARSH_ZOMBIE,
            DOOM_SOLDIER,
            SKELETON_HUNTER,
            SKELETON_HUNTER_ARCHER,
        ]);
        s.register_quest_items(&[
            KAROYDS_LETTER,
            CECKTINONS_VOUCHER1,
            CECKTINONS_VOUCHER2,
            SOUL_CATCHER,
            PRESERVE_OIL,
            ZOMBIE_HEAD,
            STEELBENDERS_HEAD,
            BONE_FRAGMENT,
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
            "30307-04.htm" => Some(event.to_owned()),
            "30307-05.htm" => {
                if qs.is_created(ctx) {
                    qs.start_quest(ctx);
                    ctx.give_items(player, KAROYDS_LETTER, 1);
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
            BLACKSMITH_KAROYD => {
                if qs.is_created(ctx) {
                    htmltext = if ctx.player_race(talker) != race::DARK_ELF {
                        "30307-01.htm"
                    } else if ctx.player_level(talker) < MIN_LEVEL {
                        "30307-02.htm"
                    } else {
                        "30307-03.htm"
                    }
                    .to_owned();
                } else if qs.is_started(ctx) {
                    if ctx.has_at_least_one_quest_item(
                        talker,
                        &[KAROYDS_LETTER, CECKTINONS_VOUCHER1, CECKTINONS_VOUCHER2],
                    ) {
                        htmltext = "30307-06.html".to_owned();
                    } else if ctx.has_quest_items(talker, STEELBENDERS_HEAD) {
                        if ctx.player_level(talker) < 25 && !ctx.is_mage_class(talker) {
                            let (id, count) = SOULSHOTS_NO_GRADE_FOR_ROOKIES;
                            ctx.give_items(talker, id, count);
                            ctx.play_sound_named(talker, "tutorial_voice_026");
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
                        ctx.add_exp_and_sp(talker, 46663, 3999);
                        ctx.give_adena(talker, 19799, true);
                        ctx.reward_items(talker, BLOODSABER, 1);
                        qs.exit_quest(ctx, false, true);
                        ctx.world().send(
                            talker,
                            &SocialAction {
                                object_id: talker.0,
                                action_id: 3,
                            },
                        );
                        htmltext = "30307-07.html".to_owned();
                    }
                } else if qs.is_completed(ctx) {
                    htmltext = ctx.get_already_completed_msg(talker);
                }
            }
            CECON => {
                if qs.is_started(ctx) {
                    if ctx.has_quest_items(talker, KAROYDS_LETTER) {
                        qs.set_cond(ctx, 2, true);
                        ctx.take_items(talker, KAROYDS_LETTER, 1);
                        ctx.give_items(talker, CECKTINONS_VOUCHER1, 1);
                        htmltext = "30132-01.html".to_owned();
                    } else if ctx
                        .has_at_least_one_quest_item(talker, &[CECKTINONS_VOUCHER1, CECKTINONS_VOUCHER2])
                    {
                        htmltext = "30132-02.html".to_owned();
                    } else if ctx.has_quest_items(talker, SOUL_CATCHER) {
                        qs.set_cond(ctx, 6, true);
                        ctx.take_items(talker, SOUL_CATCHER, 1);
                        ctx.give_items(talker, PRESERVE_OIL, 1);
                        htmltext = "30132-03.html".to_owned();
                    } else if ctx.has_quest_items(talker, PRESERVE_OIL)
                        && !ctx.has_all_quest_items(talker, &[ZOMBIE_HEAD, STEELBENDERS_HEAD])
                    {
                        htmltext = "30132-04.html".to_owned();
                    } else if ctx.has_quest_items(talker, ZOMBIE_HEAD) {
                        qs.set_cond(ctx, 8, true);
                        ctx.take_items(talker, ZOMBIE_HEAD, 1);
                        ctx.give_items(talker, STEELBENDERS_HEAD, 1);
                        htmltext = "30132-05.html".to_owned();
                    } else if ctx.has_quest_items(talker, STEELBENDERS_HEAD) {
                        htmltext = "30132-06.html".to_owned();
                    }
                }
            }
            HARNE => {
                if qs.is_started(ctx) {
                    if ctx.has_quest_items(talker, CECKTINONS_VOUCHER1) {
                        qs.set_cond(ctx, 3, true);
                        ctx.take_items(talker, CECKTINONS_VOUCHER1, 1);
                        ctx.give_items(talker, CECKTINONS_VOUCHER2, 1);
                        htmltext = "30144-01.html".to_owned();
                    } else if ctx.has_quest_items(talker, CECKTINONS_VOUCHER2) {
                        if ctx.get_quest_items_count(talker, BONE_FRAGMENT) >= 10 {
                            qs.set_cond(ctx, 5, true);
                            ctx.take_items(talker, CECKTINONS_VOUCHER2, 1);
                            ctx.take_items(talker, BONE_FRAGMENT, 10);
                            ctx.give_items(talker, SOUL_CATCHER, 1);
                            htmltext = "30144-03.html".to_owned();
                        } else {
                            htmltext = "30144-02.html".to_owned();
                        }
                    } else if ctx.has_quest_items(talker, SOUL_CATCHER) {
                        htmltext = "30144-04.html".to_owned();
                    }
                }
            }
            _ => {}
        }
        Some(htmltext)
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        // The helper picks the party member that gets the drop: every step addresses that player.
        let Some(qs) = ctx.random_party_member_state(killer, -1, 3, Some(npc)) else {
            return;
        };
        let player = qs.player();
        match npc.id {
            MARSH_ZOMBIE => {
                if ctx.has_quest_items(player, PRESERVE_OIL) && ctx.get_random(10) < 5 {
                    ctx.give_items(player, ZOMBIE_HEAD, 1);
                    ctx.take_items(player, PRESERVE_OIL, -1);
                    qs.set_cond(ctx, 7, true);
                }
            }
            DOOM_SOLDIER | SKELETON_HUNTER | SKELETON_HUNTER_ARCHER => {
                if ctx.has_quest_items(player, CECKTINONS_VOUCHER2)
                    && ctx.give_item_randomly(player, Some(npc), BONE_FRAGMENT, 1, 1, 10, 1.0, true)
                {
                    qs.set_cond(ctx, 4, true);
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

    use super::{BLACKSMITH_KAROYD, BLOODSABER, CECON, HARNE, Q00103SpiritOfCraftsman as Quest};
    use crate::testing::{Pages, ScriptTest, rows};

    const NAME: &str = "Q00103_SpiritOfCraftsman";
    const LETTER: i32 = 968;
    const VOUCHER1: i32 = 969;
    const VOUCHER2: i32 = 970;
    const SOUL_CATCHER: i32 = 971;
    const PRESERVE_OIL: i32 = 972;
    const ZOMBIE_HEAD: i32 = 973;
    const STEELBENDERS_HEAD: i32 = 974;
    const BONE: i32 = 1107;

    const NPCS: &str = r#"
        <npc id="30307" type="Folk" level="70" name="Blacksmith Karoyd"/>
        <npc id="30132" type="Folk" level="70" name="Cecon"/>
        <npc id="30144" type="Folk" level="70" name="Harne"/>
        <npc id="20455" type="Monster" level="20" name="Doom Soldier"/>
        <npc id="20015" type="Monster" level="20" name="Marsh Zombie"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="968" type="EtcItem" name="Karoyd's Letter"><set name="is_questitem" val="true"/></item>
        <item id="969" type="EtcItem" name="Ceckt. Voucher 1"><set name="is_questitem" val="true"/></item>
        <item id="970" type="EtcItem" name="Ceckt. Voucher 2"><set name="is_questitem" val="true"/></item>
        <item id="971" type="EtcItem" name="Soul Catcher"><set name="is_questitem" val="true"/></item>
        <item id="972" type="EtcItem" name="Preserve Oil"><set name="is_questitem" val="true"/></item>
        <item id="973" type="EtcItem" name="Zombie Head"><set name="is_questitem" val="true"/></item>
        <item id="974" type="EtcItem" name="Steelbender's Head"><set name="is_questitem" val="true"/></item>
        <item id="1107" type="EtcItem" name="Bone Fragment"><set name="is_questitem" val="true"/></item>
        <item id="975" type="Weapon" name="Bloodsaber"/>
        <item id="1060" type="EtcItem" name="Lesser Healing Potion"><set name="is_stackable" val="true"/></item>
        <item id="1835" type="EtcItem" name="Soulshot"><set name="is_stackable" val="true"/></item>"#;

    fn pages() -> Pages {
        Pages::of(&Quest)
            .page("30307-03.htm", &["30307-04.htm"])
            .page("30307-04.htm", &["30307-05.htm"])
            .plain(&[
                "30307-01.htm",
                "30307-02.htm",
                "30307-05.htm",
                "30307-06.html",
                "30307-07.html",
                "30132-01.html",
                "30132-02.html",
                "30132-03.html",
                "30132-04.html",
                "30132-05.html",
                "30132-06.html",
                "30144-01.html",
                "30144-02.html",
                "30144-03.html",
                "30144-04.html",
            ])
            .npcs(&[BLACKSMITH_KAROYD, CECON, HARNE])
    }

    fn setup() -> (ScriptTest, Oid, Oid, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Quest);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages());
        let karoyd = t.spawn(BLACKSMITH_KAROYD, 20);
        let cecon = t.spawn(CECON, 30);
        let harne = t.spawn(HARNE, 40);
        (t, karoyd, cecon, harne)
    }

    #[test]
    fn a_dark_elf_of_level_10_starts_and_gets_the_letter() {
        let (mut t, karoyd, _, _) = setup();
        t.enter_as(1, 10, race::DARK_ELF, player_class::DARK_FIGHTER, vec![]);
        assert!(t.quest_page(1, karoyd).contains("30307-03.htm"));
        assert!(t.event(1, NAME, "30307-04.htm").contains("30307-04.htm"));
        assert!(t.event(1, NAME, "30307-05.htm").contains("30307-05.htm"));
        assert_eq!(t.count(1, LETTER), 1);
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
        let (mut t, karoyd, _, _) = setup();
        t.enter_as(1, 9, race::DARK_ELF, player_class::DARK_FIGHTER, vec![]);
        assert!(t.quest_page(1, karoyd).contains("30307-02.htm"));
        t.enter_as(2, 20, race::HUMAN, player_class::FIGHTER, vec![]);
        assert!(t.quest_page(2, karoyd).contains("30307-01.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_chain_of_vouchers_bones_soul_oil_zombie_and_the_reward() {
        let (mut t, karoyd, cecon, harne) = setup();
        t.enter_as(1, 20, race::DARK_ELF, player_class::DARK_FIGHTER, vec![]);
        assert!(t.quest_page(1, karoyd).contains("30307-03.htm"));
        t.event(1, NAME, "30307-04.htm");
        t.event(1, NAME, "30307-05.htm");
        t.take_writes();

        // Cecon: the letter goes, voucher 1 comes, cond 2.
        assert!(t.quest_page(1, cecon).contains("30132-01.html"));
        assert_eq!((t.count(1, LETTER), t.count(1, VOUCHER1)), (0, 1));
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);

        // Harne: voucher 1 goes, voucher 2 comes, cond 3.
        assert!(t.quest_page(1, harne).contains("30144-01.html"));
        assert_eq!((t.count(1, VOUCHER1), t.count(1, VOUCHER2)), (0, 1));
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "3")]);

        // The skeletons and doom soldiers drop bones for a random member with voucher 2: cond 4.
        for _ in 0..500 {
            if t.count(1, BONE) >= 10 {
                break;
            }
            let soldier = t.spawn(20455, 60);
            t.kill(1, soldier);
        }
        assert_eq!(t.count(1, BONE), 10);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "4")]);

        // Harne: voucher 2 and the bones go, the soul catcher comes, cond 5.
        assert!(t.quest_page(1, harne).contains("30144-03.html"));
        assert_eq!(
            (t.count(1, VOUCHER2), t.count(1, BONE), t.count(1, SOUL_CATCHER)),
            (0, 0, 1)
        );
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "5")]);

        // Cecon: the catcher goes for the preserve oil, cond 6.
        assert!(t.quest_page(1, cecon).contains("30132-03.html"));
        assert_eq!((t.count(1, SOUL_CATCHER), t.count(1, PRESERVE_OIL)), (0, 1));
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "6")]);

        // Marsh zombies: the oil goes for a zombie head by chance, cond 7.
        for _ in 0..500 {
            if t.count(1, ZOMBIE_HEAD) == 1 {
                break;
            }
            let zombie = t.spawn(20015, 60);
            t.kill(1, zombie);
        }
        assert_eq!((t.count(1, PRESERVE_OIL), t.count(1, ZOMBIE_HEAD)), (0, 1));
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "7")]);

        // Cecon: the head goes for the steelbender's head, cond 8.
        assert!(t.quest_page(1, cecon).contains("30132-05.html"));
        assert_eq!((t.count(1, ZOMBIE_HEAD), t.count(1, STEELBENDERS_HEAD)), (0, 1));
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "8")]);

        // Karoyd: the reward (the Bloodsaber), the exp and the exit.
        assert!(t.quest_page(1, karoyd).contains("30307-07.html"));
        assert_eq!(t.count(1, BLOODSABER), 1);
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
