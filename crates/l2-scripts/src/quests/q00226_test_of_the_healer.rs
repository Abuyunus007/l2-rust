//! Quest 226: Test of the Healer (Priest Bandellos in Talking Island: the Cleric, Oracle and
//! Knight second class change, through Perrin, Father Gupu, Windy, Saint Kristina and the Lero
//! Lizardmen).
// Java: dist/game/data/scripts/quests/Q00226_TestOfTheHealer/Q00226_TestOfTheHealer.java
//
// Pattern: the test chain of the "Test of ..." quests (see Q00222) with a long memo state, monsters
// that spawn helpers at the NPC (`addSpawn(npc, id, npc, ..)`), a random orphan girl and a
// trader's price of 100000 adena (`takeItems(ADENA, 100000)`).
// FOLLOW-UP(engine): `npc.getSummonedNpcCount()` (the summoned NPCs are not counted) and the
// summoner of `addSpawn(npc, id, npc, ..)` (only the place is kept); `player.isInCategory(
// WHITE_MAGIC_GROUP)` is not kept by the category table yet (`subclass/categories.rs`), so the
// start page is always "not a healer" until it is.

use l2_protocol_game::server::SocialAction;
use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, STARTED, player_class};
use l2_world::subclass::CategoryType;
use l2_world::{QuestSound, Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const MASTER_SORIUS: i32 = 30327;
const ALLANA: i32 = 30424;
const PERRIN: i32 = 30428;
const PRIEST_BANDELLOS: i32 = 30473;
const FATHER_GUPU: i32 = 30658;
const ORPHAN_GIRL: i32 = 30659;
const WINDY_SHAORING: i32 = 30660;
const MYSTERIOUS_DARK_ELF: i32 = 30661;
const PIPER_LONGBOW: i32 = 30662;
const SLEIN_SHINING_BLADE: i32 = 30663;
const CAIN_FLYING_KNIFE: i32 = 30664;
const SAINT_KRISTINA: i32 = 30665;
const DAURIN_HAMMERCRUSH: i32 = 30674;

// Items
const ADENA: i32 = 57;
const REPORT_OF_PERRIN: i32 = 2810;
const CRISTINAS_LETTER: i32 = 2811;
const PICTURE_OF_WINDY: i32 = 2812;
const GOLDEN_STATUE: i32 = 2813;
const WINDYS_PEBBLES: i32 = 2814;
const ORDER_OF_SORIUS: i32 = 2815;
const SECRET_LETTER1: i32 = 2816;
const SECRET_LETTER2: i32 = 2817;
const SECRET_LETTER3: i32 = 2818;
const SECRET_LETTER4: i32 = 2819;

// Reward
const MARK_OF_HEALER: i32 = 2820;
const DIMENSIONAL_DIAMOND: i32 = 7562;

// Quest monsters
const LERO_LIZARDMAN_AGENT: i32 = 27122;
const LERO_LIZARDMAN_LEADER: i32 = 27123;
const LERO_LIZARDMAN_ASSASSIN: i32 = 27124;
const LERO_LIZARDMAN_SNIPER: i32 = 27125;
const LERO_LIZARDMAN_WIZARD: i32 = 27126;
const LERO_LIZARDMAN_LORD: i32 = 27127;
const TATOMA: i32 = 27134;

// Misc
const MIN_LEVEL: i32 = 39;

/// Java `Q00226_TestOfTheHealer`.
pub struct Q00226TestOfTheHealer;

impl Script for Q00226TestOfTheHealer {
    fn name(&self) -> &'static str {
        "Q00226_TestOfTheHealer"
    }

    fn path(&self) -> &'static str {
        "quests/Q00226_TestOfTheHealer"
    }

    fn id(&self) -> i32 {
        226
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[PRIEST_BANDELLOS]);
        s.add_talk_id(&[
            PRIEST_BANDELLOS,
            MASTER_SORIUS,
            ALLANA,
            PERRIN,
            FATHER_GUPU,
            ORPHAN_GIRL,
            WINDY_SHAORING,
            MYSTERIOUS_DARK_ELF,
            PIPER_LONGBOW,
            SLEIN_SHINING_BLADE,
            CAIN_FLYING_KNIFE,
            SAINT_KRISTINA,
            DAURIN_HAMMERCRUSH,
        ]);
        s.add_kill_id(&[
            LERO_LIZARDMAN_AGENT,
            LERO_LIZARDMAN_LEADER,
            LERO_LIZARDMAN_ASSASSIN,
            LERO_LIZARDMAN_SNIPER,
            LERO_LIZARDMAN_WIZARD,
            LERO_LIZARDMAN_LORD,
            TATOMA,
        ]);
        s.register_quest_items(&[
            REPORT_OF_PERRIN,
            CRISTINAS_LETTER,
            PICTURE_OF_WINDY,
            GOLDEN_STATUE,
            WINDYS_PEBBLES,
            ORDER_OF_SORIUS,
            SECRET_LETTER1,
            SECRET_LETTER2,
            SECRET_LETTER3,
            SECRET_LETTER4,
        ]);
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
            "ACCEPT" => {
                if qs.is_created(ctx) {
                    qs.start_quest(ctx);
                    qs.set_memo_state(ctx, 1);
                    ctx.play_sound(player, QuestSound::ItemsoundQuestMiddle);
                    ctx.give_items(player, REPORT_OF_PERRIN, 1);
                    if ctx.player_variable_int(player, "2ND_CLASS_DIAMOND_REWARD", 0) == 0 {
                        let diamonds = match ctx.player_class_id(player) {
                            player_class::CLERIC => 60,
                            player_class::KNIGHT => 104,
                            player_class::ORACLE => 45,
                            _ => 72,
                        };
                        ctx.give_items(player, DIMENSIONAL_DIAMOND, diamonds);
                        ctx.set_player_variable(player, "2ND_CLASS_DIAMOND_REWARD", "1");
                        htmltext = Some("30473-04a.htm".to_owned());
                    } else {
                        htmltext = Some("30473-04.htm".to_owned());
                    }
                }
            }
            "30473-08.html" => {
                if qs.is_memo_state(ctx, 10) && ctx.has_quest_items(player, GOLDEN_STATUE) {
                    htmltext = Some(event.to_owned());
                }
            }
            "30473-09.html" => {
                if qs.is_memo_state(ctx, 10) && ctx.has_quest_items(player, GOLDEN_STATUE) {
                    ctx.give_adena(player, 233_490, true);
                    ctx.give_items(player, MARK_OF_HEALER, 1);
                    ctx.add_exp_and_sp(player, 738_283, 50_662);
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
            "30428-02.html" => {
                if qs.is_memo_state(ctx, 1) && ctx.has_quest_items(player, REPORT_OF_PERRIN) {
                    qs.set_cond(ctx, 2, true);
                    let perrin = npc?;
                    if summoned_npc_count(perrin) < 1 {
                        if let Some(tatoma) = ctx.add_spawn_at(TATOMA, perrin.object_id, true, 200_000) {
                            ctx.add_attack_desire(tatoma, player, 999);
                        }
                    }
                }
                htmltext = Some(event.to_owned());
            }
            "30658-02.html" => {
                if qs.is_memo_state(ctx, 4)
                    && !ctx.has_at_least_one_quest_item(
                        player,
                        &[PICTURE_OF_WINDY, WINDYS_PEBBLES, GOLDEN_STATUE],
                    )
                {
                    if ctx.get_quest_items_count(player, ADENA) >= 100_000 {
                        ctx.take_items(player, ADENA, 100_000);
                        ctx.give_items(player, PICTURE_OF_WINDY, 1);
                        qs.set_cond(ctx, 7, true);
                        htmltext = Some(event.to_owned());
                    } else {
                        htmltext = Some("30658-05.html".to_owned());
                    }
                }
            }
            "30658-03.html" => {
                if qs.is_memo_state(ctx, 4)
                    && !ctx.has_at_least_one_quest_item(
                        player,
                        &[PICTURE_OF_WINDY, WINDYS_PEBBLES, GOLDEN_STATUE],
                    )
                {
                    qs.set_memo_state(ctx, 5);
                    htmltext = Some(event.to_owned());
                }
            }
            "30658-07.html" => {
                htmltext = Some(event.to_owned());
            }
            "30660-02.html" => {
                if ctx.has_quest_items(player, PICTURE_OF_WINDY) {
                    htmltext = Some(event.to_owned());
                }
            }
            "30660-03.html" => {
                if ctx.has_quest_items(player, PICTURE_OF_WINDY) {
                    ctx.take_items(player, PICTURE_OF_WINDY, 1);
                    ctx.give_items(player, WINDYS_PEBBLES, 1);
                    qs.set_cond(ctx, 8, true);
                    if let Some(windy) = npc {
                        ctx.delete_npc(windy);
                    }
                    htmltext = Some(event.to_owned());
                }
            }
            "30665-02.html" => {
                if ctx.get_quest_items_count(player, SECRET_LETTER1)
                    + ctx.get_quest_items_count(player, SECRET_LETTER2)
                    + ctx.get_quest_items_count(player, SECRET_LETTER3)
                    + ctx.get_quest_items_count(player, SECRET_LETTER4)
                    == 4
                {
                    ctx.give_items(player, CRISTINAS_LETTER, 1);
                    ctx.take_items(player, SECRET_LETTER1, 1);
                    ctx.take_items(player, SECRET_LETTER2, 1);
                    ctx.take_items(player, SECRET_LETTER3, 1);
                    ctx.take_items(player, SECRET_LETTER4, 1);
                    qs.set_memo_state(ctx, 9);
                    qs.set_cond(ctx, 22, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "30674-02.html" => {
                if qs.is_memo_state(ctx, 6) {
                    qs.set_cond(ctx, 11, false);
                    ctx.take_items(player, ORDER_OF_SORIUS, 1);
                    let daurin = npc?;
                    ctx.add_spawn_at(LERO_LIZARDMAN_AGENT, daurin.object_id, true, 200_000);
                    ctx.add_spawn_at(LERO_LIZARDMAN_AGENT, daurin.object_id, true, 200_000);
                    ctx.add_spawn_at(LERO_LIZARDMAN_LEADER, daurin.object_id, true, 200_000);
                    ctx.play_sound(player, QuestSound::ItemsoundQuestBeforeBattle);
                    htmltext = Some(event.to_owned());
                }
            }
            _ => {}
        }
        htmltext
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        let Some(qs) = ctx.get_quest_state(killer, false) else {
            return;
        };
        if qs.is_started(ctx) && ctx.check_if_in_range(ctx.alt_party_range(), npc.object_id, killer, true) {
            match npc.id {
                LERO_LIZARDMAN_LEADER => {
                    if qs.is_memo_state(ctx, 6) && !ctx.has_quest_items(killer, SECRET_LETTER1) {
                        ctx.give_items(killer, SECRET_LETTER1, 1);
                        qs.set_cond(ctx, 12, true);
                    }
                }
                LERO_LIZARDMAN_ASSASSIN => {
                    if qs.is_memo_state(ctx, 8)
                        && ctx.has_quest_items(killer, SECRET_LETTER1)
                        && !ctx.has_quest_items(killer, SECRET_LETTER2)
                    {
                        ctx.give_items(killer, SECRET_LETTER2, 1);
                        qs.set_cond(ctx, 15, true);
                    }
                }
                LERO_LIZARDMAN_SNIPER => {
                    if qs.is_memo_state(ctx, 8)
                        && ctx.has_quest_items(killer, SECRET_LETTER1)
                        && !ctx.has_quest_items(killer, SECRET_LETTER3)
                    {
                        ctx.give_items(killer, SECRET_LETTER3, 1);
                        qs.set_cond(ctx, 17, true);
                    }
                }
                LERO_LIZARDMAN_LORD => {
                    if qs.is_memo_state(ctx, 8)
                        && ctx.has_quest_items(killer, SECRET_LETTER1)
                        && !ctx.has_quest_items(killer, SECRET_LETTER4)
                    {
                        ctx.give_items(killer, SECRET_LETTER4, 1);
                        qs.set_cond(ctx, 19, true);
                    }
                }
                TATOMA => {
                    if qs.is_memo_state(ctx, 1) {
                        qs.set_memo_state(ctx, 2);
                        qs.set_cond(ctx, 3, true);
                        ctx.play_sound(killer, QuestSound::ItemsoundQuestMiddle);
                    }
                }
                _ => {}
            }
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let memo_state = qs.get_memo_state(ctx);
        let mut htmltext = ctx.get_no_quest_msg(player);
        let state = qs.state(ctx);
        if state == CREATED {
            if npc.id == PRIEST_BANDELLOS {
                if ctx.world().is_in_category(player, CategoryType::WhiteMagicGroup) {
                    if ctx.player_level(player) >= MIN_LEVEL {
                        htmltext = "30473-03.htm".to_owned();
                    } else {
                        htmltext = "30473-01.html".to_owned();
                    }
                } else {
                    htmltext = "30473-02.html".to_owned();
                }
            }
        } else if state == STARTED {
            match npc.id {
                PRIEST_BANDELLOS => {
                    if (1..10).contains(&memo_state) {
                        htmltext = "30473-05.html".to_owned();
                    } else if memo_state == 10 {
                        if ctx.has_quest_items(player, GOLDEN_STATUE) {
                            htmltext = "30473-07.html".to_owned();
                        } else {
                            ctx.give_adena(player, 266_980, true);
                            ctx.give_items(player, MARK_OF_HEALER, 1);
                            ctx.add_exp_and_sp(player, 1_476_566, 101_324);
                            qs.exit_quest(ctx, false, true);
                            ctx.world().send(
                                player,
                                &SocialAction {
                                    object_id: player.0,
                                    action_id: 3,
                                },
                            );
                            htmltext = "30473-06.html".to_owned();
                        }
                    }
                }
                MASTER_SORIUS => {
                    if memo_state == 5 {
                        ctx.give_items(player, ORDER_OF_SORIUS, 1);
                        qs.set_memo_state(ctx, 6);
                        qs.set_cond(ctx, 10, true);
                        htmltext = "30327-01.html".to_owned();
                    } else if (6..9).contains(&memo_state) {
                        htmltext = "30327-02.html".to_owned();
                    } else if memo_state == 9 {
                        if ctx.has_quest_items(player, CRISTINAS_LETTER) {
                            ctx.take_items(player, CRISTINAS_LETTER, 1);
                            qs.set_memo_state(ctx, 10);
                            qs.set_cond(ctx, 23, true);
                            htmltext = "30327-03.html".to_owned();
                        }
                    } else if memo_state >= 10 {
                        htmltext = "30327-04.html".to_owned();
                    }
                }
                ALLANA => {
                    if memo_state == 3 {
                        qs.set_memo_state(ctx, 4);
                        qs.set_cond(ctx, 5, true);
                        htmltext = "30424-01.html".to_owned();
                    } else if memo_state == 4 {
                        qs.set_memo_state(ctx, 4);
                        htmltext = "30424-02.html".to_owned();
                    }
                }
                PERRIN => {
                    if memo_state == 1 {
                        if ctx.has_quest_items(player, REPORT_OF_PERRIN) {
                            htmltext = "30428-01.html".to_owned();
                        }
                    } else if memo_state == 2 {
                        ctx.take_items(player, REPORT_OF_PERRIN, 1);
                        qs.set_memo_state(ctx, 3);
                        qs.set_cond(ctx, 4, true);
                        htmltext = "30428-03.html".to_owned();
                    } else if memo_state == 3 {
                        htmltext = "30428-04.html".to_owned();
                    }
                }
                FATHER_GUPU => {
                    if memo_state == 4 {
                        if !ctx.has_at_least_one_quest_item(
                            player,
                            &[PICTURE_OF_WINDY, WINDYS_PEBBLES, GOLDEN_STATUE],
                        ) {
                            qs.set_cond(ctx, 6, true);
                            htmltext = "30658-01.html".to_owned();
                        } else if ctx.has_quest_items(player, PICTURE_OF_WINDY) {
                            htmltext = "30658-04.html".to_owned();
                        } else if ctx.has_quest_items(player, WINDYS_PEBBLES) {
                            ctx.give_items(player, GOLDEN_STATUE, 1);
                            ctx.take_items(player, WINDYS_PEBBLES, 1);
                            qs.set_memo_state(ctx, 5);
                            htmltext = "30658-06.html".to_owned();
                        }
                    } else if memo_state == 5 {
                        qs.set_cond(ctx, 9, true);
                        htmltext = "30658-07.html".to_owned();
                    }
                }
                ORPHAN_GIRL => {
                    htmltext = match ctx.get_random(5) {
                        0 => "30659-01.html",
                        1 => "30659-02.html",
                        2 => "30659-03.html",
                        3 => "30659-04.html",
                        _ => "30659-05.html",
                    }
                    .to_owned();
                }
                WINDY_SHAORING => {
                    if ctx.has_quest_items(player, PICTURE_OF_WINDY) {
                        htmltext = "30660-01.html".to_owned();
                    } else if ctx.has_quest_items(player, WINDYS_PEBBLES) {
                        htmltext = "30660-04.html".to_owned();
                    }
                }
                MYSTERIOUS_DARK_ELF => {
                    if memo_state == 8 {
                        if ctx.has_quest_items(player, SECRET_LETTER1)
                            && !ctx.has_quest_items(player, SECRET_LETTER2)
                        {
                            if summoned_npc_count(npc) < 36 {
                                spawn_helpers(ctx, npc, LERO_LIZARDMAN_ASSASSIN, 3);
                                ctx.play_sound(player, QuestSound::ItemsoundQuestBeforeBattle);
                            }
                            qs.set_cond(ctx, 14, false);
                            ctx.delete_npc(npc);
                            htmltext = "30661-01.html".to_owned();
                        } else if ctx.has_quest_items(player, SECRET_LETTER1)
                            && ctx.has_quest_items(player, SECRET_LETTER2)
                            && !ctx.has_quest_items(player, SECRET_LETTER3)
                        {
                            if summoned_npc_count(npc) < 36 {
                                spawn_helpers(ctx, npc, LERO_LIZARDMAN_SNIPER, 3);
                                ctx.play_sound(player, QuestSound::ItemsoundQuestBeforeBattle);
                            }
                            qs.set_cond(ctx, 16, false);
                            ctx.delete_npc(npc);
                            htmltext = "30661-02.html".to_owned();
                        } else if ctx
                            .has_all_quest_items(player, &[SECRET_LETTER1, SECRET_LETTER2, SECRET_LETTER3])
                            && !ctx.has_quest_items(player, SECRET_LETTER4)
                        {
                            if summoned_npc_count(npc) < 36 {
                                spawn_helpers(ctx, npc, LERO_LIZARDMAN_WIZARD, 2);
                                spawn_helpers(ctx, npc, LERO_LIZARDMAN_LORD, 1);
                                ctx.play_sound(player, QuestSound::ItemsoundQuestBeforeBattle);
                            }
                            qs.set_cond(ctx, 18, false);
                            ctx.delete_npc(npc);
                            htmltext = "30661-03.html".to_owned();
                        } else if ctx.get_quest_items_count(player, SECRET_LETTER1)
                            + ctx.get_quest_items_count(player, SECRET_LETTER2)
                            + ctx.get_quest_items_count(player, SECRET_LETTER3)
                            + ctx.get_quest_items_count(player, SECRET_LETTER4)
                            == 4
                        {
                            qs.set_cond(ctx, 20, true);
                            htmltext = "30661-04.html".to_owned();
                        }
                    }
                }
                PIPER_LONGBOW | SLEIN_SHINING_BLADE | CAIN_FLYING_KNIFE => {
                    if memo_state == 8 {
                        // The three elves differ only in the letter they lack (Cain: the fourth).
                        let (first_missing, page_1, page_2, page_3) = match npc.id {
                            PIPER_LONGBOW => {
                                (SECRET_LETTER2, "30662-01.html", "30662-02.html", "30662-03.html")
                            }
                            SLEIN_SHINING_BLADE => {
                                (SECRET_LETTER2, "30663-01.html", "30663-02.html", "30663-03.html")
                            }
                            _ => (SECRET_LETTER4, "30664-01.html", "30664-02.html", "30664-03.html"),
                        };
                        if ctx.has_quest_items(player, SECRET_LETTER1)
                            && !ctx.has_quest_items(player, first_missing)
                        {
                            htmltext = page_1.to_owned();
                        } else if ctx.has_quest_items(player, SECRET_LETTER2)
                            && !ctx.has_all_quest_items(player, &[SECRET_LETTER3, SECRET_LETTER4])
                        {
                            htmltext = page_2.to_owned();
                        } else if ctx
                            .has_all_quest_items(player, &[SECRET_LETTER2, SECRET_LETTER3, SECRET_LETTER4])
                        {
                            qs.set_cond(ctx, 21, true);
                            htmltext = page_3.to_owned();
                        }
                    }
                }
                SAINT_KRISTINA => {
                    let letters = ctx.get_quest_items_count(player, SECRET_LETTER1)
                        + ctx.get_quest_items_count(player, SECRET_LETTER2)
                        + ctx.get_quest_items_count(player, SECRET_LETTER3)
                        + ctx.get_quest_items_count(player, SECRET_LETTER4);
                    if letters == 4 {
                        htmltext = "30665-01.html".to_owned();
                    } else if memo_state < 9 {
                        if letters < 4 {
                            htmltext = "30665-03.html".to_owned();
                        }
                    } else if memo_state >= 9 {
                        htmltext = "30665-04.html".to_owned();
                    }
                }
                DAURIN_HAMMERCRUSH => {
                    if memo_state == 6 {
                        if ctx.has_quest_items(player, ORDER_OF_SORIUS) {
                            htmltext = "30674-01.html".to_owned();
                        } else if !ctx.has_at_least_one_quest_item(player, &[SECRET_LETTER1, ORDER_OF_SORIUS])
                        {
                            if summoned_npc_count(npc) < 4 {
                                ctx.add_spawn_at(LERO_LIZARDMAN_AGENT, npc.object_id, true, 200_000);
                                ctx.add_spawn_at(LERO_LIZARDMAN_LEADER, npc.object_id, true, 200_000);
                            }
                            htmltext = "30674-02a.html".to_owned();
                        } else if ctx.has_quest_items(player, SECRET_LETTER1) {
                            qs.set_memo_state(ctx, 8);
                            qs.set_cond(ctx, 13, true);
                            htmltext = "30674-03.html".to_owned();
                        }
                    } else if memo_state >= 8 {
                        htmltext = "30674-04.html".to_owned();
                    }
                }
                _ => {}
            }
        } else if state == COMPLETED && npc.id == PRIEST_BANDELLOS {
            htmltext = ctx.get_already_completed_msg(player);
        }
        Some(htmltext)
    }
}

/// FOLLOW-UP(engine): Java `npc.getSummonedNpcCount()`, the NPCs a monster summoned that are alive.
/// The world keeps no summoner link yet, so this is 0.
fn summoned_npc_count(_npc: ScriptNpc) -> i32 {
    0
}

/// Java `addSpawn(npc, id, npc, true, 200000)`, `count` times: a helper at the NPC that lives 200 s.
fn spawn_helpers(ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, helper_id: i32, count: i32) {
    for _ in 0..count {
        ctx.add_spawn_at(helper_id, npc.object_id, true, 200_000);
    }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::ScriptRegistry;
    use l2_world::script::{player_class, race};

    use super::*;
    use crate::testing::{Pages, ScriptTest, names_to, pages_to, rows};

    const NAME: &str = "Q00226_TestOfTheHealer";

    const NPCS: &str = r#"
        <npc id="30473" type="Folk" level="70" name="Priest Bandellos"/>
        <npc id="30428" type="Folk" level="70" name="Perrin"/>
        <npc id="30327" type="Folk" level="70" name="Master Sorius"/>
        <npc id="30658" type="Folk" level="70" name="Father Gupu"/>
        <npc id="30660" type="Folk" level="70" name="Windy Shaoring"/>
        <npc id="27134" type="Monster" level="40" name="Tatoma"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="7562" type="EtcItem" name="Dimensional Diamond"><set name="is_stackable" val="true"/></item>
        <item id="2810" type="EtcItem" name="Report of Perrin"><set name="is_questitem" val="true"/></item>
        <item id="2811" type="EtcItem" name="Cristina's Letter"><set name="is_questitem" val="true"/></item>
        <item id="2812" type="EtcItem" name="Picture of Windy"><set name="is_questitem" val="true"/></item>
        <item id="2813" type="EtcItem" name="Golden Statue"><set name="is_questitem" val="true"/></item>
        <item id="2814" type="EtcItem" name="Windy's Pebbles"><set name="is_questitem" val="true"/></item>
        <item id="2815" type="EtcItem" name="Order of Sorius"><set name="is_questitem" val="true"/></item>
        <item id="2820" type="EtcItem" name="Mark of Healer"><set name="is_questitem" val="true"/></item>"#;

    /// The pages of the quest with the links of the real pages.
    fn pages() -> Pages {
        Pages::of(&Q00226TestOfTheHealer)
            .plain(&[
                "30473-01.html",
                "30473-02.html",
                "30473-04.htm",
                "30473-04a.htm",
                "30473-05.html",
                "30473-06.html",
                "30473-07.html",
                "30473-09.html",
            ])
            .page("30473-03.htm", &["ACCEPT"])
            .page("30473-08.html", &["30473-09.html"])
            .page("30428-01.html", &["30428-02.html"])
            .plain(&["30428-02.html", "30428-03.html", "30428-04.html"])
            .plain(&["30327-01.html", "30327-02.html", "30327-03.html", "30327-04.html"])
            .plain(&[
                "30658-01.html",
                "30658-02.html",
                "30658-03.html",
                "30658-04.html",
                "30658-05.html",
                "30658-06.html",
                "30658-07.html",
            ])
            .plain(&["30660-01.html", "30660-02.html", "30660-03.html", "30660-04.html"])
            .npcs(&[
                PRIEST_BANDELLOS,
                PERRIN,
                MASTER_SORIUS,
                FATHER_GUPU,
                WINDY_SHAORING,
            ])
    }

    struct Npcs {
        bandellos: Oid,
        perrin: Oid,
    }

    fn setup() -> (ScriptTest, Npcs) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00226TestOfTheHealer);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages());
        let npcs = Npcs {
            bandellos: t.spawn(PRIEST_BANDELLOS, 20),
            perrin: t.spawn(PERRIN, 30),
        };
        (t, npcs)
    }

    /// The quest started by hand (the class gate is a FOLLOW-UP(engine), see the module comment):
    /// memo state `memo` and the report of Perrin.
    fn start_by_hand(t: &mut ScriptTest, memo: i32) {
        t.world.run_as_script(NAME, |ctx| {
            if let Some(qs) = ctx.get_quest_state(Oid(1), true) {
                qs.start_quest(ctx);
                qs.set_memo_state(ctx, memo);
            }
            ctx.give_items(Oid(1), REPORT_OF_PERRIN, 1);
        });
    }

    #[test]
    fn a_fighter_is_told_that_he_is_no_healer() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 40, race::HUMAN, player_class::FIGHTER, vec![]);
        assert!(t.quest_page(1, npcs.bandellos).contains("30473-02.html"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn perrin_sends_the_healer_to_tatoma_and_the_kill_moves_on() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 39, race::HUMAN, player_class::CLERIC, vec![]);
        start_by_hand(&mut t, 1);
        t.take_writes();

        assert!(t.quest_page(1, npcs.perrin).contains("30428-01.html"));
        assert!(t.event(1, NAME, "30428-02.html").contains("30428-02.html"));
        assert!(t.take_writes().contains(&rows::update(NAME, "cond", "2")));
        // Perrin's Tatoma comes at the healer (the summoner is not linked, see the module comment).
        let tatoma = t.npcs_of(TATOMA);
        assert_eq!(tatoma.len(), 1);
        t.kill(1, tatoma[0]);
        t.world.delete_npc(tatoma[0]);
        assert!(t.take_writes().contains(&rows::update(NAME, "cond", "3")));

        // Perrin takes the report (memo 3, cond 4).
        assert!(t.quest_page(1, npcs.perrin).contains("30428-03.html"));
        assert_eq!(t.count(1, REPORT_OF_PERRIN), 0);
        assert_eq!(
            t.take_writes(),
            [
                rows::update(NAME, "memoState", "3"),
                rows::update(NAME, "cond", "4"),
            ]
        );
    }

    #[test]
    fn the_reward_is_the_mark_after_the_golden_statue_is_handed_in() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 39, race::HUMAN, player_class::CLERIC, vec![]);
        // Memo state 10 without the statue: Bandellos gives the reward.
        start_by_hand(&mut t, 10);
        t.world.run_as_script(NAME, |ctx| {
            ctx.take_items(Oid(1), REPORT_OF_PERRIN, -1);
        });
        t.take_writes();
        t.take();

        t.talk(1, npcs.bandellos);
        t.bypass(1, "Script");
        let out = t.take();
        assert!(pages_to(&out, 1).last().unwrap().contains("30473-06.html"));
        assert!(names_to(&out, 1).contains(&"SOCIAL_ACTION".to_owned()));
        assert_eq!(t.count(1, MARK_OF_HEALER), 1);
        assert_eq!(t.count(1, 57), 266_980);
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed"),
            ]
        );
        assert!(
            t.quest_page(1, npcs.bandellos)
                .contains("This quest has already been completed.")
        );
    }
}
