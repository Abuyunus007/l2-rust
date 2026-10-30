//! Quest 211: Trial of the Challenger (Kash, Martian, Raldo, Filaur and the Chest of Shyslassys; the warrior's mark).
// Java: dist/game/data/scripts/quests/Q00211_TrialOfTheChallenger/Q00211_TrialOfTheChallenger.java

use l2_protocol_game::server::RadarControl;
use l2_util::ObjectId;
use l2_world::script::{QuestSound, STARTED};
use l2_world::subclass::CategoryType;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const FILAUR: i32 = 30535;
const KASH: i32 = 30644;
const MARTIAN: i32 = 30645;
const RALDO: i32 = 30646;
const CHEST_OF_SHYSLASSYS: i32 = 30647;

// Items
const LETTER_OF_KASH: i32 = 2628;
const WATCHERS_EYE1: i32 = 2629;
const WATCHERS_EYE2: i32 = 2630;
const SCROLL_OF_SHYSLASSYS: i32 = 2631;
const BROKEN_KEY: i32 = 2632;

// Monsters
const SHYSLASSYS: i32 = 27110;
const GORR: i32 = 27112;
const BARAHAM: i32 = 27113;
const QUEEN_OF_SUCCUBUS: i32 = 27114;

// Rewards
const ELVEN_NECKLACE_BEADS: i32 = 1904;
const WHITE_TUNIC_PATTERN: i32 = 1936;
const IRON_BOOTS_DESIGN: i32 = 1940;
const MANTICOR_SKIN_GAITERS_PATTERN: i32 = 1943;
const GAUNTLET_OF_REPOSE_PATTERN: i32 = 1946;
const MITHRIL_SCALE_GAITERS_MATERIAL: i32 = 2918;
const BRIGAMDINE_GAUNTLET_PATTERN: i32 = 2927;
const TOME_OF_BLOOD_PAGE: i32 = 2030;
const MARK_OF_CHALLENGER: i32 = 2627;
/// Java `ItemHolder(7562, 61)`.
const DIMENSIONAL_DIAMONDS: (i32, i64) = (7562, 61);

// Misc
const MIN_LEVEL: i32 = 35;
/// Java `player.getVariables()` key: the second-class diamonds are given once per character.
const DIAMOND_REWARD_VAR: &str = "2ND_CLASS_DIAMOND_REWARD";

/// Java `Q00211_TrialOfTheChallenger`.
pub struct Q00211TrialOfTheChallenger;

impl Script for Q00211TrialOfTheChallenger {
    fn name(&self) -> &'static str {
        "Q00211_TrialOfTheChallenger"
    }

    fn path(&self) -> &'static str {
        "quests/Q00211_TrialOfTheChallenger"
    }

    fn id(&self) -> i32 {
        211
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[KASH]);
        s.add_talk_id(&[FILAUR, KASH, MARTIAN, RALDO, CHEST_OF_SHYSLASSYS]);
        s.add_kill_id(&[SHYSLASSYS, GORR, BARAHAM, QUEEN_OF_SUCCUBUS]);
        s.register_quest_items(&[
            LETTER_OF_KASH,
            WATCHERS_EYE1,
            WATCHERS_EYE2,
            SCROLL_OF_SHYSLASSYS,
            BROKEN_KEY,
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

        let mut htmltext = None;
        match event {
            "30644-04.htm" => htmltext = Some(event.to_owned()),
            "30645-07.html" | "30645-08.html" | "30646-02.html" | "30646-03.html" => {
                if qs.is_started(ctx) {
                    htmltext = Some(event.to_owned());
                }
            }
            "30644-06.htm" => {
                if qs.is_created(ctx) {
                    if ctx.player_variable_int(player, DIAMOND_REWARD_VAR, 0) == 0 {
                        ctx.give_items(player, DIMENSIONAL_DIAMONDS.0, DIMENSIONAL_DIAMONDS.1);
                        ctx.set_player_variable(player, DIAMOND_REWARD_VAR, "1");
                        htmltext = Some(event.to_owned());
                    } else {
                        htmltext = Some("30644-05.htm".to_owned());
                    }
                    qs.start_quest(ctx);
                }
            }
            "30647-02.html" => {
                if qs.is_cond(ctx, 2) && ctx.has_quest_items(player, BROKEN_KEY) {
                    ctx.take_items(player, BROKEN_KEY, -1);
                    if ctx.get_random(10) < 2 {
                        ctx.play_sound(player, QuestSound::ItemsoundQuestJackpot);
                        let random = ctx.get_random(100);
                        if random > 90 {
                            ctx.reward_items(player, MITHRIL_SCALE_GAITERS_MATERIAL, 1);
                            ctx.reward_items(player, BRIGAMDINE_GAUNTLET_PATTERN, 1);
                            ctx.reward_items(player, MANTICOR_SKIN_GAITERS_PATTERN, 1);
                            ctx.reward_items(player, GAUNTLET_OF_REPOSE_PATTERN, 1);
                            ctx.reward_items(player, IRON_BOOTS_DESIGN, 1);
                        } else if random > 70 {
                            ctx.reward_items(player, TOME_OF_BLOOD_PAGE, 1);
                            ctx.reward_items(player, ELVEN_NECKLACE_BEADS, 1);
                        } else if random > 40 {
                            ctx.reward_items(player, WHITE_TUNIC_PATTERN, 1);
                        } else {
                            ctx.reward_items(player, IRON_BOOTS_DESIGN, 1);
                        }
                        htmltext = Some("30647-03.html".to_owned());
                    } else {
                        let adena = ctx.get_random(1000) + 1;
                        ctx.give_adena(player, i64::from(adena), true);
                        htmltext = Some(event.to_owned());
                    }
                } else {
                    htmltext = Some("30647-04.html".to_owned());
                }
            }
            "30645-02.html" => {
                if qs.is_cond(ctx, 3) && ctx.has_quest_items(player, LETTER_OF_KASH) {
                    qs.set_cond(ctx, 4, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "30646-04.html" | "30646-05.html" => {
                if qs.is_cond(ctx, 7) && ctx.has_quest_items(player, WATCHERS_EYE2) {
                    ctx.take_items(player, WATCHERS_EYE2, -1);
                    qs.set_cond(ctx, 8, true);
                    htmltext = Some(event.to_owned());
                }
            }
            _ => {}
        }
        htmltext
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, talker: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(talker, true)?;
        let mut htmltext = ctx.get_no_quest_msg(talker);
        match npc.id {
            KASH => {
                if qs.is_created(ctx) {
                    if !ctx.world().is_in_category(talker, CategoryType::WarriorGroup) {
                        htmltext = "30644-02.html".to_owned();
                    } else if ctx.player_level(talker) < MIN_LEVEL {
                        htmltext = "30644-01.html".to_owned();
                    } else {
                        htmltext = "30644-03.htm".to_owned();
                    }
                } else if qs.is_started(ctx) {
                    match qs.get_cond(ctx) {
                        1 => htmltext = "30644-07.html".to_owned(),
                        2 => {
                            if ctx.has_quest_items(talker, SCROLL_OF_SHYSLASSYS) {
                                ctx.take_items(talker, SCROLL_OF_SHYSLASSYS, -1);
                                ctx.give_items(talker, LETTER_OF_KASH, 1);
                                qs.set_cond(ctx, 3, true);
                                htmltext = "30644-08.html".to_owned();
                            }
                        }
                        3 => {
                            if ctx.has_quest_items(talker, LETTER_OF_KASH) {
                                htmltext = "30644-09.html".to_owned();
                            }
                        }
                        8..=10 => htmltext = "30644-10.html".to_owned(),
                        _ => {}
                    }
                } else if qs.is_completed(ctx) {
                    htmltext = ctx.get_already_completed_msg(talker);
                }
            }
            MARTIAN => match qs.get_cond(ctx) {
                3 => {
                    if ctx.has_quest_items(talker, LETTER_OF_KASH) {
                        htmltext = "30645-01.html".to_owned();
                    }
                }
                4 => htmltext = "30645-03.html".to_owned(),
                5 => {
                    if ctx.has_quest_items(talker, WATCHERS_EYE1) {
                        ctx.take_items(talker, WATCHERS_EYE1, -1);
                        qs.set_cond(ctx, 6, true);
                        htmltext = "30645-04.html".to_owned();
                    }
                }
                6 => htmltext = "30645-05.html".to_owned(),
                7 => htmltext = "30645-06.html".to_owned(),
                8 | 9 => htmltext = "30645-09.html".to_owned(),
                _ => {}
            },
            CHEST_OF_SHYSLASSYS => {
                if qs.is_started(ctx) {
                    htmltext = "30647-01.html".to_owned();
                }
            }
            RALDO => match qs.get_cond(ctx) {
                7 => {
                    if ctx.has_quest_items(talker, WATCHERS_EYE2) {
                        htmltext = "30646-01.html".to_owned();
                    }
                }
                8 => htmltext = "30646-06.html".to_owned(),
                10 => {
                    ctx.add_exp_and_sp(talker, 1_067_606, 69_242);
                    ctx.give_adena(talker, 194_556, true);
                    ctx.give_items(talker, MARK_OF_CHALLENGER, 1);

                    // redundant retail check - already rewarded at beginning of quest
                    if ctx.player_variable_int(talker, DIAMOND_REWARD_VAR, 0) == 0 {
                        ctx.give_items(talker, DIMENSIONAL_DIAMONDS.0, DIMENSIONAL_DIAMONDS.1);
                        ctx.set_player_variable(talker, DIAMOND_REWARD_VAR, "1");
                    }

                    ctx.send_social_action(talker, 3);
                    qs.exit_quest(ctx, false, true);
                    htmltext = "30646-07.html".to_owned();
                }
                _ => {}
            },
            FILAUR => match qs.get_cond(ctx) {
                8 => {
                    htmltext = "30535-01.html".to_owned();
                    qs.set_cond(ctx, 9, true);
                }
                9 => {
                    ctx.world().send(
                        talker,
                        &RadarControl {
                            show_radar: 0,
                            radar_type: 2,
                            x: 151_589,
                            y: -174_823,
                            z: -1_776,
                        },
                    );
                    htmltext = "30535-02.html".to_owned();
                }
                10 => htmltext = "30535-03.html".to_owned(),
                _ => {}
            },
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

        match npc.id {
            SHYSLASSYS => {
                if qs.is_cond(ctx, 1) {
                    if ctx.spawn_table_count(npc.id) < 10 {
                        ctx.add_spawn_at(CHEST_OF_SHYSLASSYS, npc.object_id, false, 200_000);
                    }
                    ctx.give_items(killer, SCROLL_OF_SHYSLASSYS, 1);
                    ctx.give_items(killer, BROKEN_KEY, 1);
                    qs.set_cond(ctx, 2, true);
                }
            }
            GORR => {
                if qs.is_cond(ctx, 4) {
                    ctx.give_items(killer, WATCHERS_EYE1, 1);
                    qs.set_cond(ctx, 5, true);
                }
            }
            BARAHAM => {
                if qs.is_cond(ctx, 6) {
                    if ctx.spawn_table_count(npc.id) < 10 {
                        ctx.add_spawn_at(RALDO, npc.object_id, false, 100_000);
                    }
                    ctx.give_items(killer, WATCHERS_EYE2, 1);
                    qs.set_cond(ctx, 7, true);
                }
            }
            QUEEN_OF_SUCCUBUS => {
                if qs.is_cond(ctx, 9) {
                    if ctx.spawn_table_count(npc.id) < 10 {
                        ctx.add_spawn_at(RALDO, npc.object_id, false, 100_000);
                    }
                    qs.set_cond(ctx, 10, true);
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

    use super::*;
    use crate::testing::{Pages, ScriptTest, names_to, rows};

    const NAME: &str = "Q00211_TrialOfTheChallenger";

    const NPCS: &str = r#"
        <npc id="30535" type="Folk" level="70" name="Filaur"/>
        <npc id="30644" type="Folk" level="70" name="Kash"/>
        <npc id="30645" type="Folk" level="70" name="Martian"/>
        <npc id="30646" type="Folk" level="70" name="Raldo"/>
        <npc id="30647" type="Folk" level="70" name="Chest of Shyslassys"/>
        <npc id="27110" type="Monster" level="40" name="Shyslassys"/>
        <npc id="27112" type="Monster" level="40" name="Gorr"/>
        <npc id="27113" type="Monster" level="40" name="Baraham"/>
        <npc id="27114" type="Monster" level="40" name="Queen of Succubus"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="7562" type="EtcItem" name="Dimensional Diamond"><set name="is_stackable" val="true"/></item>
        <item id="2627" type="EtcItem" name="Mark of Challenger"><set name="is_questitem" val="true"/></item>
        <item id="2628" type="EtcItem" name="Letter of Kash"><set name="is_questitem" val="true"/></item>
        <item id="2629" type="EtcItem" name="Watcher's Eye 1"><set name="is_questitem" val="true"/></item>
        <item id="2630" type="EtcItem" name="Watcher's Eye 2"><set name="is_questitem" val="true"/></item>
        <item id="2631" type="EtcItem" name="Scroll of Shyslassys"><set name="is_questitem" val="true"/></item>
        <item id="2632" type="EtcItem" name="Broken Key"><set name="is_questitem" val="true"/></item>"#;

    struct Npcs {
        kash: Oid,
        martian: Oid,
        raldo: Oid,
        filaur: Oid,
    }

    fn setup() -> (ScriptTest, Npcs) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00211TrialOfTheChallenger);
        let pages = Pages::of(&Q00211TrialOfTheChallenger)
            .plain(&["30644-01.html", "30644-02.html", "30644-05.htm", "30644-06.htm"])
            .page("30644-03.htm", &["30644-04.htm"])
            .page("30644-04.htm", &["30644-06.htm"])
            .plain(&["30644-07.html", "30644-08.html", "30644-09.html", "30644-10.html"])
            .page("30645-01.html", &["30645-02.html"])
            .plain(&["30645-02.html", "30645-03.html", "30645-04.html", "30645-05.html"])
            .page("30645-06.html", &["30645-07.html"])
            .page("30645-07.html", &["30645-08.html"])
            .plain(&["30645-08.html", "30645-09.html"])
            .page("30646-01.html", &["30646-02.html"])
            .page("30646-02.html", &["30646-03.html"])
            .page("30646-03.html", &["30646-04.html"])
            .plain(&["30646-04.html", "30646-06.html", "30646-07.html"])
            .plain(&["30535-01.html", "30535-02.html", "30535-03.html"])
            .page("30647-01.html", &["30647-02.html"])
            .plain(&["30647-02.html", "30647-03.html", "30647-04.html"])
            .npcs(&[KASH, MARTIAN, RALDO, FILAUR, CHEST_OF_SHYSLASSYS]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let npcs = Npcs {
            kash: t.spawn(KASH, 20),
            martian: t.spawn(MARTIAN, 30),
            raldo: t.spawn(RALDO, 40),
            filaur: t.spawn(FILAUR, 50),
        };
        (t, npcs)
    }

    #[test]
    fn only_a_warrior_of_level_35_is_offered_the_quest() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 35, race::HUMAN, player_class::MAGE, vec![]);
        assert!(t.quest_page(1, npcs.kash).contains("30644-02.html"));
        t.enter_as(2, 34, race::HUMAN, player_class::WARRIOR, vec![]);
        assert!(t.quest_page(2, npcs.kash).contains("30644-01.html"));
        t.enter_as(3, 35, race::HUMAN, player_class::WARRIOR, vec![]);
        assert!(t.quest_page(3, npcs.kash).contains("30644-03.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_whole_trial_to_the_mark() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 35, race::HUMAN, player_class::WARRIOR, vec![]);
        assert!(t.quest_page(1, npcs.kash).contains("30644-03.htm"));
        assert!(t.event(1, NAME, "30644-04.htm").contains("30644-04.htm"));
        assert!(t.event(1, NAME, "30644-06.htm").contains("30644-06.htm"));
        assert_eq!(t.count(1, 7562), 61);
        assert_eq!(
            t.take_writes()[..2],
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );

        // cond 1: Shyslassys drops the scroll and the key and the chest appears.
        let shyslassys = t.spawn(SHYSLASSYS, 60);
        t.kill(1, shyslassys);
        t.world.delete_npc(shyslassys);
        assert_eq!(t.count(1, SCROLL_OF_SHYSLASSYS), 1);
        assert_eq!(t.count(1, BROKEN_KEY), 1);
        assert_eq!(t.npcs_of(CHEST_OF_SHYSLASSYS).len(), 1);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);

        // The key opens the chest (the world RNG is seeded: some reward or adena comes).
        let chest = t.npcs_of(CHEST_OF_SHYSLASSYS)[0];
        assert!(t.quest_page(1, chest).contains("30647-01.html"));
        let answer = t.event(1, NAME, "30647-02.html");
        assert!(answer.contains("30647-02.html") || answer.contains("30647-03.html"));
        assert_eq!(t.count(1, BROKEN_KEY), 0);
        t.take_writes();

        // Kash takes the scroll (cond 3), Martian sends to Gorr (4), the eye (5), back (6).
        assert!(t.quest_page(1, npcs.kash).contains("30644-08.html"));
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "3")]);
        assert!(t.quest_page(1, npcs.martian).contains("30645-01.html"));
        assert!(t.event(1, NAME, "30645-02.html").contains("30645-02.html"));
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "4")]);
        let gorr = t.spawn(GORR, 60);
        t.kill(1, gorr);
        t.world.delete_npc(gorr);
        assert_eq!(t.count(1, WATCHERS_EYE1), 1);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "5")]);
        assert!(t.quest_page(1, npcs.martian).contains("30645-04.html"));
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "6")]);

        // Baraham: the second eye (7), Raldo appears; Raldo takes it (8).
        let baraham = t.spawn(BARAHAM, 60);
        t.kill(1, baraham);
        t.world.delete_npc(baraham);
        assert_eq!(t.count(1, WATCHERS_EYE2), 1);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "7")]);
        assert!(t.quest_page(1, npcs.raldo).contains("30646-01.html"));
        for e in ["30646-02.html", "30646-03.html", "30646-04.html"] {
            assert!(t.event(1, NAME, e).contains(e));
        }
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "8")]);

        // Filaur (9), the Queen of Succubus (10), Raldo pays.
        assert!(t.quest_page(1, npcs.filaur).contains("30535-01.html"));
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "9")]);
        t.take();
        assert!(t.quest_page(1, npcs.filaur).contains("30535-02.html"));
        let queen = t.spawn(QUEEN_OF_SUCCUBUS, 60);
        t.kill(1, queen);
        t.world.delete_npc(queen);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "10")]);

        t.talk(1, npcs.raldo);
        t.bypass(1, "Script");
        let out = t.take();
        assert!(names_to(&out, 1).contains(&"SOCIAL_ACTION".to_owned()));
        assert_eq!(t.count(1, MARK_OF_CHALLENGER), 1);
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed")
            ]
        );
        assert_eq!(t.count(1, 7562), 61);
    }
}
