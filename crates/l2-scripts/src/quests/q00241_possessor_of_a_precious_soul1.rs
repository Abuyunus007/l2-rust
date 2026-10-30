//! Quest 241: Possessor of a Precious Soul 1 (Talien sends a sub-class character through the
//! Gabrielle/Gilmore errand, Baraham, the Malruk succubus claws, the poetry book, the crimson
//! moss and Caradine's reward).
// Java: dist/game/data/scripts/quests/Q00241_PossessorOfAPreciousSoul1/Q00241_PossessorOfAPreciousSoul1.java
//
// Pattern: a sub-class quest (`no_sub.html`) driven by `cond` alone, with party kills through
// `getRandomPartyMember(player, cond)` (same shape as `q00246_possessor_of_a_precious_soul3.rs`).

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, QuestSound, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const STEDMIEL: i32 = 30692;
const GABRIELLE: i32 = 30753;
const GILMORE: i32 = 30754;
const KANTABILON: i32 = 31042;
const RAHORAKTI: i32 = 31336;
const TALIEN: i32 = 31739;
const CARADINE: i32 = 31740;
const VIRGIL: i32 = 31742;
const KASSANDRA: i32 = 31743;
const OGMAR: i32 = 31744;

// Mobs
const BARAHAM: i32 = 27113;
const MALRUK_SUCCUBUS_1: i32 = 20244;
const MALRUK_SUCCUBUS_TUREN_1: i32 = 20245;
const MALRUK_SUCCUBUS_2: i32 = 20283;
const MALRUK_SUCCUBUS_TUREN_2: i32 = 20284;
const TAIK_ORC_SUPPLY_LEADER: i32 = 20669;

// Items
const LEGEND_OF_SEVENTEEN: i32 = 7587;
const MALRUK_SUCCUBUS_CLAW: i32 = 7597;
const ECHO_CRYSTAL: i32 = 7589;
const POETRY_BOOK: i32 = 7588;
const CRIMSON_MOSS: i32 = 7598;
const RAHORAKTIS_MEDICINE: i32 = 7599;
const VIRGILS_LETTER: i32 = 7677;

// Rewards
const CRIMSON_MOSS_CHANCE: i32 = 30;
const MALRUK_SUCCUBUS_CLAW_CHANCE: i32 = 60;

/// Java `Q00241_PossessorOfAPreciousSoul1`.
pub struct Q00241PossessorOfAPreciousSoul1;

impl Script for Q00241PossessorOfAPreciousSoul1 {
    fn name(&self) -> &'static str {
        "Q00241_PossessorOfAPreciousSoul1"
    }

    fn path(&self) -> &'static str {
        "quests/Q00241_PossessorOfAPreciousSoul1"
    }

    fn id(&self) -> i32 {
        241
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[TALIEN]);
        s.add_talk_id(&[
            TALIEN, STEDMIEL, GABRIELLE, GILMORE, KANTABILON, RAHORAKTI, CARADINE, KASSANDRA, VIRGIL, OGMAR,
        ]);
        s.add_kill_id(&[
            BARAHAM,
            MALRUK_SUCCUBUS_1,
            MALRUK_SUCCUBUS_TUREN_1,
            MALRUK_SUCCUBUS_2,
            MALRUK_SUCCUBUS_TUREN_2,
            TAIK_ORC_SUPPLY_LEADER,
        ]);
        s.register_quest_items(&[
            LEGEND_OF_SEVENTEEN,
            MALRUK_SUCCUBUS_CLAW,
            ECHO_CRYSTAL,
            POETRY_BOOK,
            CRIMSON_MOSS,
            RAHORAKTIS_MEDICINE,
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
        let Some(qs) = ctx.get_quest_state(player, false) else {
            return Some(ctx.get_no_quest_msg(player));
        };
        if !ctx.is_sub_class_active(player) {
            return Some("no_sub.html".to_owned());
        }

        match event {
            "31739-02.html" => {
                qs.start_quest(ctx);
            }
            "30753-02.html" => {
                if qs.is_cond(ctx, 1) {
                    qs.set_cond(ctx, 2, true);
                }
            }
            "30754-02.html" => {
                if qs.is_cond(ctx, 2) {
                    qs.set_cond(ctx, 3, true);
                }
            }
            "31739-05.html" => {
                if qs.is_cond(ctx, 4) && ctx.has_quest_items(player, LEGEND_OF_SEVENTEEN) {
                    ctx.take_items(player, LEGEND_OF_SEVENTEEN, -1);
                    qs.set_cond(ctx, 5, true);
                }
            }
            "31042-02.html" => {
                if qs.is_cond(ctx, 5) {
                    qs.set_cond(ctx, 6, true);
                }
            }
            "31042-05.html" => {
                if qs.is_cond(ctx, 7) && ctx.get_quest_items_count(player, MALRUK_SUCCUBUS_CLAW) >= 10 {
                    ctx.take_items(player, MALRUK_SUCCUBUS_CLAW, -1);
                    ctx.give_items(player, ECHO_CRYSTAL, 1);
                    qs.set_cond(ctx, 8, true);
                }
            }
            "31739-08.html" => {
                if qs.is_cond(ctx, 8) && ctx.has_quest_items(player, ECHO_CRYSTAL) {
                    ctx.take_items(player, ECHO_CRYSTAL, -1);
                    qs.set_cond(ctx, 9, true);
                }
            }
            "30692-02.html" => {
                if qs.is_cond(ctx, 9) && !ctx.has_quest_items(player, POETRY_BOOK) {
                    ctx.give_items(player, POETRY_BOOK, 1);
                    qs.set_cond(ctx, 10, true);
                }
            }
            "31739-11.html" => {
                if qs.is_cond(ctx, 10) && ctx.has_quest_items(player, POETRY_BOOK) {
                    ctx.take_items(player, POETRY_BOOK, -1);
                    qs.set_cond(ctx, 11, true);
                }
            }
            "31742-02.html" => {
                if qs.is_cond(ctx, 11) {
                    qs.set_cond(ctx, 12, true);
                }
            }
            "31744-02.html" => {
                if qs.is_cond(ctx, 12) {
                    qs.set_cond(ctx, 13, true);
                }
            }
            "31336-02.html" => {
                if qs.is_cond(ctx, 13) {
                    qs.set_cond(ctx, 14, true);
                }
            }
            "31336-05.html" => {
                if qs.is_cond(ctx, 15) && ctx.get_quest_items_count(player, CRIMSON_MOSS) >= 5 {
                    ctx.take_items(player, CRIMSON_MOSS, -1);
                    ctx.give_items(player, RAHORAKTIS_MEDICINE, 1);
                    qs.set_cond(ctx, 16, true);
                }
            }
            "31743-02.html" => {
                if qs.is_cond(ctx, 16) && ctx.has_quest_items(player, RAHORAKTIS_MEDICINE) {
                    ctx.take_items(player, RAHORAKTIS_MEDICINE, -1);
                    qs.set_cond(ctx, 17, true);
                }
            }
            "31742-05.html" => {
                if qs.is_cond(ctx, 17) {
                    qs.set_cond(ctx, 18, true);
                }
            }
            "31740-05.html" => {
                if qs.get_cond(ctx) >= 18 {
                    ctx.give_items(player, VIRGILS_LETTER, 1);
                    ctx.add_exp_and_sp(player, 263_043, 0);
                    qs.exit_quest(ctx, false, true);
                }
            }
            _ => {}
        }

        Some(event.to_owned())
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId, _is_summon: bool) {
        match npc.id {
            BARAHAM => {
                let Some(party_member) = ctx.get_random_party_member_cond(player, 3) else {
                    return;
                };
                let Some(qs) = ctx.get_quest_state(party_member, false) else {
                    return;
                };
                // NOTE(java-quirk): the item goes to the killer, the cond to the party member.
                ctx.give_items(player, LEGEND_OF_SEVENTEEN, 1);
                qs.set_cond(ctx, 4, true);
            }
            MALRUK_SUCCUBUS_1 | MALRUK_SUCCUBUS_TUREN_1 | MALRUK_SUCCUBUS_2 | MALRUK_SUCCUBUS_TUREN_2 => {
                let Some(party_member) = ctx.get_random_party_member_cond(player, 6) else {
                    return;
                };
                let Some(qs) = ctx.get_quest_state(party_member, false) else {
                    return;
                };
                if MALRUK_SUCCUBUS_CLAW_CHANCE >= ctx.get_random(100)
                    && ctx.get_quest_items_count(party_member, MALRUK_SUCCUBUS_CLAW) < 10
                {
                    ctx.give_items(party_member, MALRUK_SUCCUBUS_CLAW, 1);
                    if ctx.get_quest_items_count(party_member, MALRUK_SUCCUBUS_CLAW) == 10 {
                        qs.set_cond(ctx, 7, true);
                    } else {
                        ctx.play_sound(party_member, QuestSound::ItemsoundQuestItemget);
                    }
                }
            }
            TAIK_ORC_SUPPLY_LEADER => {
                let Some(party_member) = ctx.get_random_party_member_cond(player, 14) else {
                    return;
                };
                let Some(qs) = ctx.get_quest_state(party_member, false) else {
                    return;
                };
                if CRIMSON_MOSS_CHANCE >= ctx.get_random(100)
                    && ctx.get_quest_items_count(party_member, CRIMSON_MOSS) < 5
                {
                    ctx.give_items(party_member, CRIMSON_MOSS, 1);
                    if ctx.get_quest_items_count(party_member, CRIMSON_MOSS) == 5 {
                        qs.set_cond(ctx, 15, true);
                    } else {
                        ctx.play_sound(party_member, QuestSound::ItemsoundQuestItemget);
                    }
                }
            }
            _ => {}
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        if qs.is_started(ctx) && !ctx.is_sub_class_active(player) {
            return Some("no_sub.html".to_owned());
        }

        let mut htmltext = ctx.get_no_quest_msg(player);
        let cond = qs.get_cond(ctx);
        match npc.id {
            TALIEN => {
                let state = qs.state(ctx);
                if state == CREATED {
                    htmltext = if ctx.player_level(player) >= 50 && ctx.is_sub_class_active(player) {
                        "31739-01.htm"
                    } else {
                        "31739-00.htm"
                    }
                    .to_owned();
                } else if state == STARTED {
                    match cond {
                        1 => htmltext = "31739-03.html".to_owned(),
                        4 => {
                            if ctx.has_quest_items(player, LEGEND_OF_SEVENTEEN) {
                                htmltext = "31739-04.html".to_owned();
                            }
                        }
                        5 => htmltext = "31739-06.html".to_owned(),
                        8 => {
                            if ctx.has_quest_items(player, ECHO_CRYSTAL) {
                                htmltext = "31739-07.html".to_owned();
                            }
                        }
                        9 => htmltext = "31739-09.html".to_owned(),
                        10 => {
                            if ctx.has_quest_items(player, POETRY_BOOK) {
                                htmltext = "31739-10.html".to_owned();
                            }
                        }
                        11 => htmltext = "31739-12.html".to_owned(),
                        _ => {}
                    }
                } else if state == COMPLETED {
                    htmltext = ctx.get_already_completed_msg(player);
                }
            }
            GABRIELLE => match cond {
                1 => htmltext = "30753-01.html".to_owned(),
                2 => htmltext = "30753-03.html".to_owned(),
                _ => {}
            },
            GILMORE => match cond {
                2 => htmltext = "30754-01.html".to_owned(),
                3 => htmltext = "30754-03.html".to_owned(),
                _ => {}
            },
            KANTABILON => match cond {
                5 => htmltext = "31042-01.html".to_owned(),
                6 => htmltext = "31042-04.html".to_owned(),
                7 => {
                    if ctx.get_quest_items_count(player, MALRUK_SUCCUBUS_CLAW) >= 10 {
                        htmltext = "31042-03.html".to_owned();
                    }
                }
                8 => htmltext = "31042-06.html".to_owned(),
                _ => {}
            },
            STEDMIEL => match cond {
                9 => htmltext = "30692-01.html".to_owned(),
                10 => htmltext = "30692-03.html".to_owned(),
                _ => {}
            },
            VIRGIL => match cond {
                11 => htmltext = "31742-01.html".to_owned(),
                12 => htmltext = "31742-03.html".to_owned(),
                17 => htmltext = "31742-04.html".to_owned(),
                18 => htmltext = "31742-06.html".to_owned(),
                _ => {}
            },
            OGMAR => match cond {
                12 => htmltext = "31744-01.html".to_owned(),
                13 => htmltext = "31744-03.html".to_owned(),
                _ => {}
            },
            RAHORAKTI => match cond {
                13 => htmltext = "31336-01.html".to_owned(),
                14 => htmltext = "31336-04.html".to_owned(),
                15 => {
                    if ctx.get_quest_items_count(player, CRIMSON_MOSS) >= 5 {
                        htmltext = "31336-03.html".to_owned();
                    }
                }
                16 => htmltext = "31336-06.html".to_owned(),
                _ => {}
            },
            KASSANDRA => match cond {
                16 => {
                    if ctx.has_quest_items(player, RAHORAKTIS_MEDICINE) {
                        htmltext = "31743-01.html".to_owned();
                    }
                }
                17 => htmltext = "31743-03.html".to_owned(),
                _ => {}
            },
            CARADINE => {
                if cond >= 18 {
                    htmltext = "31740-01.html".to_owned();
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

    use super::*;
    use crate::testing::{Pages, ScriptTest, item, player_data, rows};

    const NAME: &str = "Q00241_PossessorOfAPreciousSoul1";

    const NPCS: &str = r#"
        <npc id="30753" type="Folk" level="70" name="Gabrielle"/>
        <npc id="31739" type="Folk" level="70" name="Talien"/>
        <npc id="31740" type="Folk" level="70" name="Caradine"/>
        <npc id="27113" type="Monster" level="50" name="Baraham"/>
        <npc id="20244" type="Monster" level="50" name="Malruk Succubus"/>"#;

    const ITEMS: &str = r#"
        <item id="7587" type="EtcItem" name="Legend of Seventeen"><set name="is_questitem" val="true"/></item>
        <item id="7597" type="EtcItem" name="Malruk Succubus Claw"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="7677" type="EtcItem" name="Virgil's Letter"/>"#;

    fn setup() -> (ScriptTest, Oid, Oid, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00241PossessorOfAPreciousSoul1);
        let pages = Pages::of(&Q00241PossessorOfAPreciousSoul1)
            .plain(&["31739-00.htm", "no_sub.html"])
            .page("31739-01.htm", &["31739-02.html"])
            .plain(&["31739-02.html", "31739-03.html", "31739-04.html"])
            .page("30753-01.html", &["30753-02.html"])
            .plain(&["30753-02.html", "30753-03.html"])
            .page("31740-01.html", &["31740-04.html"])
            .page("31740-04.html", &["31740-05.html"])
            .plain(&["31740-05.html"])
            .npcs(&[GABRIELLE, TALIEN, CARADINE]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let talien = t.spawn(TALIEN, 20);
        let gabrielle = t.spawn(GABRIELLE, 40);
        let caradine = t.spawn(CARADINE, 60);
        (t, talien, gabrielle, caradine)
    }

    fn sub_class(t: &mut ScriptTest, id: i32, level: i32) {
        let mut data = player_data(id, level, vec![item(10 * id, 57, 1)]);
        data.profile.class_index = 1;
        t.enter_with(data);
    }

    #[test]
    fn the_quest_needs_an_active_sub_class_of_level_50() {
        let (mut t, talien, _, _) = setup();
        t.enter_with(player_data(1, 60, vec![]));
        assert!(t.quest_page(1, talien).contains("31739-00.htm"));
        sub_class(&mut t, 2, 49);
        assert!(t.quest_page(2, talien).contains("31739-00.htm"));
        sub_class(&mut t, 3, 50);
        assert!(t.quest_page(3, talien).contains("31739-01.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_first_steps_set_the_cond() {
        let (mut t, talien, gabrielle, _) = setup();
        sub_class(&mut t, 1, 50);
        t.quest_page(1, talien);
        assert!(t.event(1, NAME, "31739-02.html").contains("31739-02.html"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        assert!(t.quest_page(1, talien).contains("31739-03.html"));
        assert!(t.quest_page(1, gabrielle).contains("30753-01.html"));
        assert!(t.event(1, NAME, "30753-02.html").contains("30753-02.html"));
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);
        assert!(t.quest_page(1, gabrielle).contains("30753-03.html"));
    }

    #[test]
    fn baraham_drops_the_legend_only_at_cond_3() {
        let (mut t, talien, _, _) = setup();
        sub_class(&mut t, 1, 50);
        t.quest_page(1, talien);
        t.event(1, NAME, "31739-02.html");
        let baraham = t.spawn(BARAHAM, 90);
        t.kill(1, baraham);
        assert_eq!(t.count(1, LEGEND_OF_SEVENTEEN), 0);
        t.world.run_as_script(NAME, |ctx| {
            if let Some(qs) = ctx.get_quest_state(Oid(1), false) {
                qs.set_cond(ctx, 3, false);
            }
        });
        t.take_writes();
        let baraham2 = t.spawn(BARAHAM, 100);
        t.kill(1, baraham2);
        assert_eq!(t.count(1, LEGEND_OF_SEVENTEEN), 1);
        assert!(t.take_writes().contains(&rows::update(NAME, "cond", "4")));
        assert!(t.quest_page(1, talien).contains("31739-04.html"));
    }

    #[test]
    fn caradine_gives_virgils_letter_and_ends_the_quest() {
        let (mut t, talien, _, caradine) = setup();
        sub_class(&mut t, 1, 50);
        t.quest_page(1, talien);
        t.event(1, NAME, "31739-02.html");
        t.world.run_as_script(NAME, |ctx| {
            if let Some(qs) = ctx.get_quest_state(Oid(1), false) {
                qs.set_cond(ctx, 18, false);
            }
        });
        t.take_writes();
        assert!(t.quest_page(1, caradine).contains("31740-01.html"));
        assert!(t.event(1, NAME, "31740-04.html").contains("31740-04.html"));
        assert!(t.event(1, NAME, "31740-05.html").contains("31740-05.html"));
        assert_eq!(t.count(1, VIRGILS_LETTER), 1);
        assert_eq!(
            t.take_writes().last(),
            Some(&rows::update(NAME, "<state>", "Completed"))
        );
        assert!(
            t.quest_page(1, talien)
                .contains("This quest has already been completed.")
        );
    }
}
