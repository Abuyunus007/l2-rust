//! Quest 407: Path of the Elven Scout (Master Reoria, Guard Babenco, Guard Moretti and Prias).
// Java: dist/game/data/scripts/quests/Q00407_PathOfTheElvenScout/Q00407_PathOfTheElvenScout.java
//
// Pattern: a party-free kill step where the monster must have been attacked by the killer
// first (the NPC's script value holds the attacker's id, `onAttack`), then letters from Ol Mahum
// patrols, a rusted key from the quest monster Ol Mahum Sentry, and the scout's recommendation.

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, QuestSound, STARTED, player_class};
use l2_world::{Qs, Script, ScriptCtx, ScriptNpc, ScriptSetup, SkillKey};

// NPCs
const MASTER_REORIA: i32 = 30328;
const GUARD_BABENCO: i32 = 30334;
const GUARD_MORETTI: i32 = 30337;
const PRIAS: i32 = 30426;

// Items
const REISAS_LETTER: i32 = 1207;
const PRIASS_1ND_TORN_LETTER: i32 = 1208;
const PRIASS_2ND_TORN_LETTER: i32 = 1209;
const PRIASS_3ND_TORN_LETTER: i32 = 1210;
const PRIASS_4ND_TORN_LETTER: i32 = 1211;
const MORETTIES_HERB: i32 = 1212;
const MORETTIS_LETTER: i32 = 1214;
const PRIASS_LETTER: i32 = 1215;
const HONORARY_GUARD: i32 = 1216;
const REISAS_RECOMMENDATION: i32 = 1217;
const RUSTED_KEY: i32 = 1293;

/// The four torn letters of Prias, in the order the patrols drop them.
const TORN_LETTERS: [i32; 4] = [
    PRIASS_1ND_TORN_LETTER,
    PRIASS_2ND_TORN_LETTER,
    PRIASS_3ND_TORN_LETTER,
    PRIASS_4ND_TORN_LETTER,
];

// Monsters
const OL_MAHUM_PATROL: i32 = 20053;
/// Quest monster.
const OL_MAHUM_SENTRY: i32 = 27031;

// Misc
const MIN_LEVEL: i32 = 18;

/// Java `Q00407_PathOfTheElvenScout`.
pub struct Q00407PathOfTheElvenScout;

impl Script for Q00407PathOfTheElvenScout {
    fn name(&self) -> &'static str {
        "Q00407_PathOfTheElvenScout"
    }

    fn path(&self) -> &'static str {
        "quests/Q00407_PathOfTheElvenScout"
    }

    fn id(&self) -> i32 {
        407
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[MASTER_REORIA]);
        s.add_talk_id(&[MASTER_REORIA, GUARD_BABENCO, GUARD_MORETTI, PRIAS]);
        s.add_kill_id(&[OL_MAHUM_PATROL, OL_MAHUM_SENTRY]);
        s.add_attack_id(&[OL_MAHUM_PATROL, OL_MAHUM_SENTRY]);
        s.register_quest_items(&[
            REISAS_LETTER,
            PRIASS_1ND_TORN_LETTER,
            PRIASS_2ND_TORN_LETTER,
            PRIASS_3ND_TORN_LETTER,
            PRIASS_4ND_TORN_LETTER,
            MORETTIES_HERB,
            MORETTIS_LETTER,
            PRIASS_LETTER,
            HONORARY_GUARD,
            RUSTED_KEY,
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
            "ACCEPT" => {
                if ctx.player_class_id(player) == player_class::ELVEN_FIGHTER {
                    if ctx.player_level(player) >= MIN_LEVEL {
                        if ctx.has_quest_items(player, REISAS_RECOMMENDATION) {
                            htmltext = Some("30328-04.htm".to_owned());
                        } else {
                            qs.start_quest(ctx);
                            qs.unset(ctx, "variable");
                            ctx.give_items(player, REISAS_LETTER, 1);
                            htmltext = Some("30328-05.htm".to_owned());
                        }
                    } else {
                        htmltext = Some("30328-03.htm".to_owned());
                    }
                } else if ctx.player_class_id(player) == player_class::ELVEN_SCOUT {
                    htmltext = Some("30328-02a.htm".to_owned());
                } else {
                    htmltext = Some("30328-02.htm".to_owned());
                }
            }
            "30337-02.html" => {
                htmltext = Some(event.to_owned());
            }
            "30337-03.html" => {
                if ctx.has_quest_items(player, REISAS_LETTER) {
                    ctx.take_items(player, REISAS_LETTER, -1);
                    qs.set_int(ctx, "variable", 1);
                    qs.set_cond(ctx, 2, true);
                    htmltext = Some(event.to_owned());
                }
            }
            _ => {}
        }
        htmltext
    }

    fn on_attack(
        &self,
        ctx: &mut ScriptCtx<'_>,
        npc: ScriptNpc,
        attacker: ObjectId,
        _damage: i32,
        _is_summon: bool,
        _skill: Option<SkillKey>,
    ) {
        if let Some(qs) = ctx.get_quest_state(attacker, false)
            && qs.is_started(ctx)
        {
            ctx.set_script_value(npc, attacker.0);
        }
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        if ctx.is_script_value(npc, killer.0)
            && ctx.check_if_in_range(ctx.alt_party_range(), npc.object_id, killer, false)
        {
            let Some(qs) = ctx.get_quest_state(killer, false) else {
                return;
            };
            if npc.id == OL_MAHUM_SENTRY {
                if qs.is_cond(ctx, 5)
                    && ctx.get_random(10) < 6
                    && ctx.has_all_quest_items(killer, &[MORETTIES_HERB, MORETTIS_LETTER])
                    && !ctx.has_quest_items(killer, RUSTED_KEY)
                {
                    ctx.give_items(killer, RUSTED_KEY, 1);
                    qs.set_cond(ctx, 6, true);
                }
            } else if qs.is_cond(ctx, 2) {
                let has_1st_letter = ctx.has_quest_items(killer, PRIASS_1ND_TORN_LETTER);
                let has_2nd_letter = ctx.has_quest_items(killer, PRIASS_2ND_TORN_LETTER);
                let has_3rd_letter = ctx.has_quest_items(killer, PRIASS_3ND_TORN_LETTER);
                let has_4th_letter = ctx.has_quest_items(killer, PRIASS_4ND_TORN_LETTER);
                if !(has_1st_letter && has_2nd_letter && has_3rd_letter && has_4th_letter) {
                    if !has_1st_letter {
                        give_letter_and_check_state(ctx, killer, qs, PRIASS_1ND_TORN_LETTER);
                    } else if !has_2nd_letter {
                        give_letter_and_check_state(ctx, killer, qs, PRIASS_2ND_TORN_LETTER);
                    } else if !has_3rd_letter {
                        give_letter_and_check_state(ctx, killer, qs, PRIASS_3ND_TORN_LETTER);
                    } else if !has_4th_letter {
                        give_letter_and_check_state(ctx, killer, qs, PRIASS_4ND_TORN_LETTER);
                    }
                }
            }
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        let state = qs.state(ctx);
        if state == CREATED || state == COMPLETED {
            if npc.id == MASTER_REORIA {
                htmltext = "30328-01.htm".to_owned();
            }
        } else if state == STARTED {
            match npc.id {
                MASTER_REORIA => {
                    if ctx.has_quest_items(player, REISAS_LETTER) {
                        htmltext = "30328-06.html".to_owned();
                    } else if qs.get_int(ctx, "variable") == 1
                        && !ctx.has_at_least_one_quest_item(player, &[REISAS_LETTER, HONORARY_GUARD])
                    {
                        htmltext = "30328-08.html".to_owned();
                    } else if ctx.has_quest_items(player, HONORARY_GUARD) {
                        ctx.take_items(player, HONORARY_GUARD, -1);
                        ctx.give_items(player, REISAS_RECOMMENDATION, 1);
                        let level = ctx.player_level(player);
                        if level >= 20 {
                            ctx.add_exp_and_sp(player, 320_534, 19_932);
                        } else if level == 19 {
                            ctx.add_exp_and_sp(player, 456_128, 26_630);
                        } else {
                            ctx.add_exp_and_sp(player, 591_724, 33_328);
                        }
                        ctx.give_adena(player, 163_800, true);
                        qs.exit_quest(ctx, false, true);
                        ctx.world_mut().send_social_action(player, 3);
                        htmltext = "30328-07.html".to_owned();
                    }
                }
                GUARD_BABENCO => {
                    if qs.get_int(ctx, "variable") == 1 {
                        htmltext = "30334-01.html".to_owned();
                    }
                }
                GUARD_MORETTI => {
                    let letter_count = ctx.get_quest_items_count_of(player, &TORN_LETTERS);
                    if ctx.has_quest_items(player, REISAS_LETTER) && letter_count == 0 {
                        htmltext = "30337-01.html".to_owned();
                    } else if qs.get_int(ctx, "variable") == 1
                        && !ctx.has_at_least_one_quest_item(
                            player,
                            &[MORETTIS_LETTER, PRIASS_LETTER, HONORARY_GUARD],
                        )
                    {
                        if letter_count == 0 {
                            htmltext = "30337-04.html".to_owned();
                        } else if letter_count < 4 {
                            htmltext = "30337-05.html".to_owned();
                        } else {
                            ctx.take_items_many(player, -1, &TORN_LETTERS);
                            ctx.give_items(player, MORETTIES_HERB, 1);
                            ctx.give_items(player, MORETTIS_LETTER, 1);
                            qs.set_cond(ctx, 4, true);
                            htmltext = "30337-06.html".to_owned();
                        }
                    } else if ctx.has_quest_items(player, PRIASS_LETTER) {
                        ctx.take_items(player, PRIASS_LETTER, -1);
                        ctx.give_items(player, HONORARY_GUARD, 1);
                        qs.set_cond(ctx, 8, true);
                        htmltext = "30337-07.html".to_owned();
                    } else if ctx.has_all_quest_items(player, &[MORETTIES_HERB, MORETTIS_LETTER]) {
                        htmltext = "30337-09.html".to_owned();
                    } else if ctx.has_quest_items(player, HONORARY_GUARD) {
                        htmltext = "30337-08.html".to_owned();
                    }
                }
                PRIAS => {
                    if ctx.has_all_quest_items(player, &[MORETTIS_LETTER, MORETTIES_HERB]) {
                        if !ctx.has_quest_items(player, RUSTED_KEY) {
                            qs.set_cond(ctx, 5, true);
                            htmltext = "30426-01.html".to_owned();
                        } else {
                            ctx.take_items_many(player, -1, &[RUSTED_KEY, MORETTIES_HERB, MORETTIS_LETTER]);
                            ctx.give_items(player, PRIASS_LETTER, 1);
                            qs.set_cond(ctx, 7, true);
                            htmltext = "30426-02.html".to_owned();
                        }
                    } else if ctx.has_quest_items(player, PRIASS_LETTER) {
                        htmltext = "30426-04.html".to_owned();
                    }
                }
                _ => {}
            }
        }
        Some(htmltext)
    }
}

/// Java `giveLetterAndCheckState(letterId, qs)`: the letter, then cond 3 once all four are in.
fn give_letter_and_check_state(ctx: &mut ScriptCtx<'_>, player: ObjectId, qs: Qs, letter: i32) {
    ctx.give_items(player, letter, 1);
    if ctx.get_quest_items_count_of(player, &TORN_LETTERS) >= 4 {
        qs.set_cond(ctx, 3, true);
    } else {
        ctx.play_sound(player, QuestSound::ItemsoundQuestItemget);
    }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::ScriptRegistry;
    use l2_world::script::race;

    use super::*;
    use crate::testing::{Pages, ScriptTest, item, rows};

    const NAME: &str = "Q00407_PathOfTheElvenScout";

    const NPCS: &str = r#"
        <npc id="30328" type="Folk" level="70" name="Master Reoria"/>
        <npc id="30334" type="Folk" level="70" name="Guard Babenco"/>
        <npc id="30337" type="Folk" level="70" name="Guard Moretti"/>
        <npc id="30426" type="Folk" level="70" name="Prias"/>
        <npc id="20053" type="Monster" level="20" name="Ol Mahum Patrol"/>
        <npc id="27031" type="Monster" level="30" name="Ol Mahum Sentry"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="1207" type="EtcItem" name="Reisa's Letter"><set name="is_questitem" val="true"/></item>
        <item id="1208" type="EtcItem" name="Torn Letter 1"><set name="is_questitem" val="true"/></item>
        <item id="1209" type="EtcItem" name="Torn Letter 2"><set name="is_questitem" val="true"/></item>
        <item id="1210" type="EtcItem" name="Torn Letter 3"><set name="is_questitem" val="true"/></item>
        <item id="1211" type="EtcItem" name="Torn Letter 4"><set name="is_questitem" val="true"/></item>
        <item id="1212" type="EtcItem" name="Moretti's Herb"><set name="is_questitem" val="true"/></item>
        <item id="1214" type="EtcItem" name="Moretti's Letter"><set name="is_questitem" val="true"/></item>
        <item id="1215" type="EtcItem" name="Prias' Letter"><set name="is_questitem" val="true"/></item>
        <item id="1216" type="EtcItem" name="Honorary Guard"><set name="is_questitem" val="true"/></item>
        <item id="1217" type="EtcItem" name="Reisa's Recommendation"/>
        <item id="1293" type="EtcItem" name="Rusted Key"><set name="is_questitem" val="true"/></item>"#;

    struct Npcs {
        reoria: Oid,
        babenco: Oid,
        moretti: Oid,
        prias: Oid,
    }

    fn pages() -> Pages {
        Pages::of(&Q00407PathOfTheElvenScout)
            .page("30328-01.htm", &["ACCEPT"])
            .page("30337-01.html", &["30337-02.html"])
            .page("30337-02.html", &["30337-03.html"])
            .plain(&[
                "30328-02.htm",
                "30328-02a.htm",
                "30328-03.htm",
                "30328-04.htm",
                "30328-05.htm",
                "30328-06.html",
                "30328-07.html",
                "30328-08.html",
                "30334-01.html",
                "30337-03.html",
                "30337-04.html",
                "30337-05.html",
                "30337-06.html",
                "30337-07.html",
                "30337-08.html",
                "30337-09.html",
                "30426-01.html",
                "30426-02.html",
                "30426-04.html",
            ])
            .npcs(&[MASTER_REORIA, GUARD_BABENCO, GUARD_MORETTI, PRIAS])
    }

    fn setup() -> (ScriptTest, Npcs) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00407PathOfTheElvenScout);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages());
        let npcs = Npcs {
            reoria: t.spawn(MASTER_REORIA, 20),
            babenco: t.spawn(GUARD_BABENCO, 40),
            moretti: t.spawn(GUARD_MORETTI, 60),
            prias: t.spawn(PRIAS, 80),
        };
        (t, npcs)
    }

    /// Attacks a patrol or sentry as the player (`onAttack`), then kills it.
    fn attack_and_kill(t: &mut ScriptTest, npc_id: i32) {
        let mob = t.spawn(npc_id, 80);
        t.world.run_as_script(NAME, |ctx| {
            if let Some(npc) = ctx.script_npc(mob) {
                Q00407PathOfTheElvenScout.on_attack(ctx, npc, Oid(1), 1, false, None);
            }
        });
        t.kill(1, mob);
    }

    #[test]
    fn the_accept_page_depends_on_class_level_and_recommendation() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 17, race::ELF, player_class::ELVEN_FIGHTER, vec![]);
        assert!(t.quest_page(1, npcs.reoria).contains("30328-01.htm"));
        assert!(t.event(1, NAME, "ACCEPT").contains("30328-03.htm"));
        assert!(t.take_writes().is_empty());

        let (mut t, npcs) = setup();
        t.enter_as(1, 30, race::ELF, player_class::ELVEN_SCOUT, vec![]);
        t.quest_page(1, npcs.reoria);
        assert!(t.event(1, NAME, "ACCEPT").contains("30328-02a.htm"));

        let (mut t, npcs) = setup();
        t.enter_as(1, 30, race::ELF, player_class::ELVEN_KNIGHT, vec![]);
        t.quest_page(1, npcs.reoria);
        assert!(t.event(1, NAME, "ACCEPT").contains("30328-02.htm"));

        let (mut t, npcs) = setup();
        t.enter_as(
            1,
            30,
            race::ELF,
            player_class::ELVEN_FIGHTER,
            vec![item(5000, 1217, 1)],
        );
        assert!(t.quest_page(1, npcs.reoria).contains("30328-01.htm"));
        assert!(t.event(1, NAME, "ACCEPT").contains("30328-04.htm"));
    }

    #[test]
    fn the_whole_path_from_the_letter_to_the_recommendation() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 18, race::ELF, player_class::ELVEN_FIGHTER, vec![]);
        assert!(t.quest_page(1, npcs.reoria).contains("30328-01.htm"));
        assert!(t.event(1, NAME, "ACCEPT").contains("30328-05.htm"));
        assert_eq!(t.count(1, REISAS_LETTER), 1);
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );

        // Moretti takes Reisa's letter: the variable is set, cond 2.
        assert!(t.quest_page(1, npcs.moretti).contains("30337-01.html"));
        assert!(t.event(1, NAME, "30337-02.html").contains("30337-02.html"));
        assert!(t.event(1, NAME, "30337-03.html").contains("30337-03.html"));
        assert_eq!(t.count(1, REISAS_LETTER), 0);
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "variable", "1"),
                rows::update(NAME, "cond", "2")
            ]
        );
        assert!(t.quest_page(1, npcs.babenco).contains("30334-01.html"));
        assert!(t.quest_page(1, npcs.reoria).contains("30328-08.html"));
        assert!(t.quest_page(1, npcs.moretti).contains("30337-04.html"));

        // Four torn letters from the patrols; the fourth sets cond 3.
        for _ in 0..4 {
            attack_and_kill(&mut t, OL_MAHUM_PATROL);
        }
        assert_eq!(t.count(1, PRIASS_1ND_TORN_LETTER), 1);
        assert_eq!(t.count(1, PRIASS_4ND_TORN_LETTER), 1);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "3")]);

        // Moretti takes the four letters and gives the herb and his letter: cond 4.
        assert!(t.quest_page(1, npcs.moretti).contains("30337-06.html"));
        assert_eq!(
            (
                t.count(1, PRIASS_1ND_TORN_LETTER),
                t.count(1, MORETTIES_HERB),
                t.count(1, MORETTIS_LETTER)
            ),
            (0, 1, 1)
        );
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "4")]);

        // Prias: cond 5 while there is no key.
        assert!(t.quest_page(1, npcs.prias).contains("30426-01.html"));
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "5")]);

        // The sentry drops the rusted key (60% a kill): cond 6.
        let mut sentries = 0;
        while t.count(1, RUSTED_KEY) == 0 && sentries < 2000 {
            attack_and_kill(&mut t, OL_MAHUM_SENTRY);
            sentries += 1;
        }
        assert_eq!(t.count(1, RUSTED_KEY), 1);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "6")]);

        // Prias takes the key, the herb and the letter and gives his letter: cond 7.
        assert!(t.quest_page(1, npcs.prias).contains("30426-02.html"));
        assert_eq!(
            (
                t.count(1, RUSTED_KEY),
                t.count(1, MORETTIES_HERB),
                t.count(1, PRIASS_LETTER)
            ),
            (0, 0, 1)
        );
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "7")]);
        assert!(t.quest_page(1, npcs.prias).contains("30426-04.html"));

        // Moretti takes Prias' letter and gives the honorary guard: cond 8.
        assert!(t.quest_page(1, npcs.moretti).contains("30337-07.html"));
        assert_eq!((t.count(1, PRIASS_LETTER), t.count(1, HONORARY_GUARD)), (0, 1));
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "8")]);

        // Reoria's reward: the recommendation, the exp, the adena, the quest ends, the bow.
        assert!(t.quest_page(1, npcs.reoria).contains("30328-07.html"));
        assert_eq!(t.count(1, REISAS_RECOMMENDATION), 1);
        assert_eq!(t.count(1, HONORARY_GUARD), 0);
        assert_eq!(t.count(1, 57), 163_800);
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed")
            ]
        );
        assert!(t.world.quest_list(Oid(1)).quests.is_empty());

        // Done: Reoria shows his first page again.
        assert!(t.quest_page(1, npcs.reoria).contains("30328-01.htm"));
    }

    #[test]
    fn a_patrol_only_gives_letters_to_the_one_who_attacked_it() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 18, race::ELF, player_class::ELVEN_FIGHTER, vec![]);
        t.quest_page(1, npcs.reoria);
        t.event(1, NAME, "ACCEPT");
        t.quest_page(1, npcs.moretti);
        t.event(1, NAME, "30337-02.html");
        t.event(1, NAME, "30337-03.html");
        t.take_writes();
        // Killed without an attack: the NPC's script value is not the player's id.
        let mob = t.spawn(OL_MAHUM_PATROL, 80);
        t.kill(1, mob);
        assert_eq!(t.count(1, PRIASS_1ND_TORN_LETTER), 0);
        assert!(t.take_writes().is_empty());
    }
}
