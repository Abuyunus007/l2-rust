//! Quest 217: Testimony of Trust (High Priest Hollint's letters to the elves, dark elves, dwarves and orcs; the human's mark).
// Java: dist/game/data/scripts/quests/Q00217_TestimonyOfTrust/Q00217_TestimonyOfTrust.java

use l2_util::ObjectId;
use l2_world::script::{QuestSound, race};
use l2_world::subclass::CategoryType;
use l2_world::{Qs, Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const HIGH_PRIEST_BIOTIN: i32 = 30031;
const HIERARCH_ASTERIOS: i32 = 30154;
const HIGH_PRIEST_HOLLINT: i32 = 30191;
const TETRARCH_THIFIELL: i32 = 30358;
const MAGISTER_CLAYTON: i32 = 30464;
const SEER_MANAKIA: i32 = 30515;
const IRON_GATES_LOCKIRIN: i32 = 30531;
const FLAME_LORD_KAKAI: i32 = 30565;
const MAESTRO_NIKOLA: i32 = 30621;
const CARDINAL_SERESIN: i32 = 30657;

// Items
const LETTER_TO_ELF: i32 = 2735;
const LETTER_TO_DARKELF: i32 = 2736;
const LETTER_TO_DWARF: i32 = 2737;
const LETTER_TO_ORC: i32 = 2738;
const LETTER_TO_SERESIN: i32 = 2739;
const SCROLL_OF_DARKELF_TRUST: i32 = 2740;
const SCROLL_OF_ELF_TRUST: i32 = 2741;
const SCROLL_OF_DWARF_TRUST: i32 = 2742;
const SCROLL_OF_ORC_TRUST: i32 = 2743;
const RECOMMENDATION_OF_HOLLIN: i32 = 2744;
const ORDER_OF_ASTERIOS: i32 = 2745;
const BREATH_OF_WINDS: i32 = 2746;
const SEED_OF_VERDURE: i32 = 2747;
const LETTER_OF_THIFIELL: i32 = 2748;
const BLOOD_OF_GUARDIAN_BASILISK: i32 = 2749;
const GIANT_APHID: i32 = 2750;
const STAKATOS_FLUIDS: i32 = 2751;
const BASILISK_PLASMA: i32 = 2752;
const HONEY_DEW: i32 = 2753;
const STAKATO_ICHOR: i32 = 2754;
const ORDER_OF_CLAYTON: i32 = 2755;
const PARASITE_OF_LOTA: i32 = 2756;
const LETTER_TO_MANAKIA: i32 = 2757;
const LETTER_OF_MANAKIA: i32 = 2758;
const LETTER_TO_NICHOLA: i32 = 2759;
const ORDER_OF_NICHOLA: i32 = 2760;
const HEART_OF_PORTA: i32 = 2761;

// Reward
const MARK_OF_TRUST: i32 = 2734;
const DIMENSIONAL_DIAMOND: i32 = 7562;

// Monsters
const DRYAD: i32 = 20013;
const DRYAD_ELDER: i32 = 20019;
const LIREIN: i32 = 20036;
const LIREIN_ELDER: i32 = 20044;
const ANT_RECRUIT: i32 = 20082;
const ANT_PATROL: i32 = 20084;
const ANT_GUARD: i32 = 20086;
const ANT_SOLDIER: i32 = 20087;
const ANT_WARRIOR_CAPTAIN: i32 = 20088;
const MARSH_STAKATO: i32 = 20157;
const PORTA: i32 = 20213;
const MARSH_STAKATO_WORKER: i32 = 20230;
const MARSH_STAKATO_SOLDIER: i32 = 20232;
const MARSH_STAKATO_DRONE: i32 = 20234;
const GUARDIAN_BASILISK: i32 = 20550;
const WINDSUS: i32 = 20553;

// Quest monsters
const LUELL_OF_ZEPHYR_WINDS: i32 = 27120;
const ACTEA_OF_VERDANT_WILDS: i32 = 27121;

// Misc
const MIN_LEVEL: i32 = 37;
/// Java `player.getVariables()` key: the second-class diamonds are given once per character.
const DIAMOND_REWARD_VAR: &str = "2ND_CLASS_DIAMOND_REWARD";

/// Java `Q00217_TestimonyOfTrust`.
pub struct Q00217TestimonyOfTrust;

fn has(ctx: &ScriptCtx<'_>, player: ObjectId, item: i32) -> bool {
    ctx.has_quest_items(player, item)
}

fn count(ctx: &ScriptCtx<'_>, player: ObjectId, item: i32) -> i64 {
    ctx.get_quest_items_count(player, item)
}

/// The three-products part of the cond 6 step (`memoState == 6`): `raw` pieces are gathered up to
/// `limit` (4 of them and the next kill turns them into one `product`); when the other two products
/// are there, cond 7 is set (no sound).
fn gather(
    ctx: &mut ScriptCtx<'_>,
    qs: Qs,
    killer: ObjectId,
    raw: i32,
    limit: i64,
    product: i32,
    others: &[i32],
) {
    if qs.is_memo_state(ctx, 6)
        && count(ctx, killer, raw) < limit
        && has(ctx, killer, ORDER_OF_CLAYTON)
        && !has(ctx, killer, product)
    {
        if count(ctx, killer, raw) >= 4 {
            ctx.give_items(killer, product, 1);
            ctx.take_items(killer, raw, -1);
            ctx.play_sound(killer, QuestSound::ItemsoundQuestMiddle);
            if ctx.has_all_quest_items(killer, others) {
                qs.set_cond(ctx, 7, false);
            }
        } else {
            ctx.give_items(killer, raw, 1);
            ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
        }
    }
}

impl Script for Q00217TestimonyOfTrust {
    fn name(&self) -> &'static str {
        "Q00217_TestimonyOfTrust"
    }

    fn path(&self) -> &'static str {
        "quests/Q00217_TestimonyOfTrust"
    }

    fn id(&self) -> i32 {
        217
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[HIGH_PRIEST_HOLLINT]);
        s.add_talk_id(&[
            HIGH_PRIEST_HOLLINT,
            HIGH_PRIEST_BIOTIN,
            HIERARCH_ASTERIOS,
            TETRARCH_THIFIELL,
            MAGISTER_CLAYTON,
            SEER_MANAKIA,
            IRON_GATES_LOCKIRIN,
            FLAME_LORD_KAKAI,
            MAESTRO_NIKOLA,
            CARDINAL_SERESIN,
        ]);
        s.add_kill_id(&[
            DRYAD,
            DRYAD_ELDER,
            LIREIN,
            LIREIN_ELDER,
            ANT_RECRUIT,
            ANT_PATROL,
            ANT_GUARD,
            ANT_SOLDIER,
            ANT_WARRIOR_CAPTAIN,
            MARSH_STAKATO,
            PORTA,
            MARSH_STAKATO_WORKER,
            MARSH_STAKATO_SOLDIER,
            MARSH_STAKATO_DRONE,
            GUARDIAN_BASILISK,
            WINDSUS,
            LUELL_OF_ZEPHYR_WINDS,
            ACTEA_OF_VERDANT_WILDS,
        ]);
        s.register_quest_items(&[
            LETTER_TO_ELF,
            LETTER_TO_DARKELF,
            LETTER_TO_DWARF,
            LETTER_TO_ORC,
            LETTER_TO_SERESIN,
            SCROLL_OF_DARKELF_TRUST,
            SCROLL_OF_ELF_TRUST,
            SCROLL_OF_DWARF_TRUST,
            SCROLL_OF_ORC_TRUST,
            RECOMMENDATION_OF_HOLLIN,
            ORDER_OF_ASTERIOS,
            BREATH_OF_WINDS,
            SEED_OF_VERDURE,
            LETTER_OF_THIFIELL,
            BLOOD_OF_GUARDIAN_BASILISK,
            GIANT_APHID,
            STAKATOS_FLUIDS,
            BASILISK_PLASMA,
            HONEY_DEW,
            STAKATO_ICHOR,
            ORDER_OF_CLAYTON,
            PARASITE_OF_LOTA,
            LETTER_TO_MANAKIA,
            LETTER_OF_MANAKIA,
            LETTER_TO_NICHOLA,
            ORDER_OF_NICHOLA,
            HEART_OF_PORTA,
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
                if qs.is_created(ctx) {
                    qs.start_quest(ctx);
                    qs.set_memo_state(ctx, 1);
                    ctx.give_items(player, LETTER_TO_ELF, 1);
                    ctx.give_items(player, LETTER_TO_DARKELF, 1);
                    ctx.play_sound(player, QuestSound::ItemsoundQuestMiddle);
                    if ctx.player_variable_int(player, DIAMOND_REWARD_VAR, 0) == 0 {
                        ctx.give_items(player, DIMENSIONAL_DIAMOND, 96);
                        ctx.set_player_variable(player, DIAMOND_REWARD_VAR, "1");
                        htmltext = Some("30191-04a.htm".to_owned());
                    } else {
                        htmltext = Some("30191-04.htm".to_owned());
                    }
                }
            }
            "30154-02.html" | "30657-02.html" => htmltext = Some(event.to_owned()),
            "30154-03.html" => {
                if has(ctx, player, LETTER_TO_ELF) {
                    ctx.take_items(player, LETTER_TO_ELF, 1);
                    ctx.give_items(player, ORDER_OF_ASTERIOS, 1);
                    qs.set_memo_state(ctx, 2);
                    qs.set_cond(ctx, 2, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "30358-02.html" => {
                if has(ctx, player, LETTER_TO_DARKELF) {
                    ctx.take_items(player, LETTER_TO_DARKELF, 1);
                    ctx.give_items(player, LETTER_OF_THIFIELL, 1);
                    qs.set_memo_state(ctx, 5);
                    qs.set_cond(ctx, 5, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "30515-02.html" => {
                if has(ctx, player, LETTER_TO_MANAKIA) {
                    ctx.take_items(player, LETTER_TO_MANAKIA, 1);
                    qs.set_memo_state(ctx, 11);
                    qs.set_cond(ctx, 14, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "30531-02.html" => {
                if has(ctx, player, LETTER_TO_DWARF) {
                    ctx.take_items(player, LETTER_TO_DWARF, 1);
                    ctx.give_items(player, LETTER_TO_NICHOLA, 1);
                    qs.set_memo_state(ctx, 15);
                    qs.set_cond(ctx, 18, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "30565-02.html" => {
                if has(ctx, player, LETTER_TO_ORC) {
                    ctx.take_items(player, LETTER_TO_ORC, 1);
                    ctx.give_items(player, LETTER_TO_MANAKIA, 1);
                    qs.set_memo_state(ctx, 10);
                    qs.set_cond(ctx, 13, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "30621-02.html" => {
                if has(ctx, player, LETTER_TO_NICHOLA) {
                    ctx.take_items(player, LETTER_TO_NICHOLA, 1);
                    ctx.give_items(player, ORDER_OF_NICHOLA, 1);
                    qs.set_memo_state(ctx, 16);
                    qs.set_cond(ctx, 19, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "30657-03.html" => {
                if qs.is_memo_state(ctx, 8) && has(ctx, player, LETTER_TO_SERESIN) {
                    ctx.give_items(player, LETTER_TO_DWARF, 1);
                    ctx.give_items(player, LETTER_TO_ORC, 1);
                    ctx.take_items(player, LETTER_TO_SERESIN, 1);
                    qs.set_memo_state(ctx, 9);
                    qs.set_cond(ctx, 12, true);
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
        if !(qs.is_started(ctx) && ctx.check_if_in_range(ctx.alt_party_range(), npc.object_id, killer, true))
        {
            return;
        }

        match npc.id {
            DRYAD | DRYAD_ELDER => {
                if qs.is_memo_state(ctx, 2) {
                    // NOTE(java-quirk): nothing ever sets the player variable "flag", so it is 1
                    // and the quest monster appears with a chance of 33%.
                    let flag = ctx.player_variable_int(killer, "flag", 1);
                    if ctx.get_random(100) < flag * 33 {
                        ctx.add_spawn_at(ACTEA_OF_VERDANT_WILDS, npc.object_id, true, 200_000);
                        ctx.play_sound(killer, QuestSound::ItemsoundQuestBeforeBattle);
                    }
                }
            }
            LIREIN | LIREIN_ELDER => {
                if qs.is_memo_state(ctx, 2) {
                    let flag = ctx.player_variable_int(killer, "flag", 1);
                    if ctx.get_random(100) < flag * 33 {
                        ctx.add_spawn_at(LUELL_OF_ZEPHYR_WINDS, npc.object_id, true, 200_000);
                        ctx.play_sound(killer, QuestSound::ItemsoundQuestBeforeBattle);
                    }
                }
            }
            ANT_RECRUIT | ANT_GUARD => {
                gather(
                    ctx,
                    qs,
                    killer,
                    GIANT_APHID,
                    5,
                    HONEY_DEW,
                    &[BASILISK_PLASMA, STAKATO_ICHOR],
                );
            }
            ANT_PATROL | ANT_SOLDIER | ANT_WARRIOR_CAPTAIN => {
                gather(
                    ctx,
                    qs,
                    killer,
                    GIANT_APHID,
                    10,
                    HONEY_DEW,
                    &[BASILISK_PLASMA, STAKATO_ICHOR],
                );
            }
            MARSH_STAKATO | MARSH_STAKATO_WORKER => {
                gather(
                    ctx,
                    qs,
                    killer,
                    STAKATOS_FLUIDS,
                    10,
                    STAKATO_ICHOR,
                    &[BASILISK_PLASMA, HONEY_DEW],
                );
            }
            MARSH_STAKATO_SOLDIER | MARSH_STAKATO_DRONE => {
                gather(
                    ctx,
                    qs,
                    killer,
                    STAKATOS_FLUIDS,
                    5,
                    STAKATO_ICHOR,
                    &[BASILISK_PLASMA, HONEY_DEW],
                );
            }
            PORTA => {
                if qs.is_memo_state(ctx, 16) && !has(ctx, killer, HEART_OF_PORTA) {
                    ctx.give_items(killer, HEART_OF_PORTA, 1);
                    if has(ctx, killer, HEART_OF_PORTA) {
                        qs.set_cond(ctx, 20, true);
                    }
                }
            }
            GUARDIAN_BASILISK => {
                gather(
                    ctx,
                    qs,
                    killer,
                    BLOOD_OF_GUARDIAN_BASILISK,
                    10,
                    BASILISK_PLASMA,
                    &[STAKATO_ICHOR, HONEY_DEW],
                );
            }
            WINDSUS => {
                if qs.is_memo_state(ctx, 11) && count(ctx, killer, PARASITE_OF_LOTA) < 10 {
                    ctx.give_items(killer, PARASITE_OF_LOTA, 2);
                    if count(ctx, killer, PARASITE_OF_LOTA) == 10 {
                        qs.set_memo_state(ctx, 12);
                        qs.set_cond(ctx, 15, true);
                    } else {
                        ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
                    }
                }
            }
            LUELL_OF_ZEPHYR_WINDS => {
                if qs.is_memo_state(ctx, 2) && !has(ctx, killer, BREATH_OF_WINDS) {
                    if has(ctx, killer, SEED_OF_VERDURE) {
                        ctx.give_items(killer, BREATH_OF_WINDS, 1);
                        qs.set_memo_state(ctx, 3);
                        qs.set_cond(ctx, 3, true);
                    } else {
                        ctx.give_items(killer, BREATH_OF_WINDS, 1);
                        ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
                    }
                }
            }
            ACTEA_OF_VERDANT_WILDS => {
                if qs.is_memo_state(ctx, 2) && !has(ctx, killer, SEED_OF_VERDURE) {
                    if has(ctx, killer, BREATH_OF_WINDS) {
                        ctx.give_items(killer, SEED_OF_VERDURE, 1);
                        qs.set_memo_state(ctx, 3);
                        ctx.play_sound(killer, QuestSound::ItemsoundQuestMiddle);
                    } else {
                        ctx.give_items(killer, SEED_OF_VERDURE, 1);
                        ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
                    }
                }
            }
            _ => {}
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let memo_state = qs.get_memo_state(ctx);
        let mut htmltext = ctx.get_no_quest_msg(player);
        if qs.is_created(ctx) {
            if npc.id == HIGH_PRIEST_HOLLINT {
                let human = ctx.player_race(player) == race::HUMAN;
                let level_ok = ctx.player_level(player) >= MIN_LEVEL;
                if human && level_ok && ctx.world().is_in_category(player, CategoryType::Human2ndGroup) {
                    htmltext = "30191-03.htm".to_owned();
                } else if human
                    && level_ok
                    && ctx.world().is_in_category(player, CategoryType::FirstClassGroup)
                {
                    htmltext = "30191-01a.html".to_owned();
                } else if human && level_ok {
                    htmltext = "30191-01b.html".to_owned();
                } else if human {
                    htmltext = "30191-01.html".to_owned();
                } else {
                    htmltext = "30191-02.html".to_owned();
                }
            }
        } else if qs.is_started(ctx) {
            match npc.id {
                HIGH_PRIEST_HOLLINT => {
                    if memo_state == 7 {
                        if ctx.has_all_quest_items(player, &[SCROLL_OF_ELF_TRUST, SCROLL_OF_DARKELF_TRUST]) {
                            ctx.give_items(player, LETTER_TO_SERESIN, 1);
                            ctx.take_items(player, SCROLL_OF_DARKELF_TRUST, 1);
                            ctx.take_items(player, SCROLL_OF_ELF_TRUST, 1);
                            qs.set_memo_state(ctx, 8);
                            qs.set_cond(ctx, 10, true);
                            htmltext = "30191-05.html".to_owned();
                        }
                    } else if memo_state == 18 {
                        if ctx.has_all_quest_items(player, &[SCROLL_OF_DWARF_TRUST, SCROLL_OF_ORC_TRUST]) {
                            ctx.take_items(player, SCROLL_OF_DWARF_TRUST, 1);
                            ctx.take_items(player, SCROLL_OF_ORC_TRUST, 1);
                            ctx.give_items(player, RECOMMENDATION_OF_HOLLIN, 1);
                            qs.set_memo_state(ctx, 19);
                            qs.set_cond(ctx, 23, true);
                            htmltext = "30191-06.html".to_owned();
                        }
                    } else if memo_state == 19 {
                        htmltext = "30191-07.html".to_owned();
                    } else if memo_state == 1 {
                        htmltext = "30191-08.html".to_owned();
                    } else if memo_state == 8 {
                        htmltext = "30191-09.html".to_owned();
                    }
                }
                HIGH_PRIEST_BIOTIN => {
                    if memo_state == 19 && has(ctx, player, RECOMMENDATION_OF_HOLLIN) {
                        ctx.give_adena(player, 252_212, true);
                        ctx.give_items(player, MARK_OF_TRUST, 1);
                        ctx.add_exp_and_sp(player, 1_390_298, 92_782);
                        qs.exit_quest(ctx, false, true);
                        ctx.send_social_action(player, 3);
                        htmltext = "30031-01.html".to_owned();
                    }
                }
                HIERARCH_ASTERIOS => {
                    if memo_state == 1 {
                        if has(ctx, player, LETTER_TO_ELF) {
                            htmltext = "30154-01.html".to_owned();
                        }
                    } else if memo_state == 2 {
                        if has(ctx, player, ORDER_OF_ASTERIOS) {
                            htmltext = "30154-04.html".to_owned();
                        }
                    } else if memo_state == 3 {
                        if ctx.has_all_quest_items(player, &[BREATH_OF_WINDS, SEED_OF_VERDURE]) {
                            ctx.give_items(player, SCROLL_OF_ELF_TRUST, 1);
                            ctx.take_items(player, ORDER_OF_ASTERIOS, 1);
                            ctx.take_items(player, BREATH_OF_WINDS, 1);
                            ctx.take_items(player, SEED_OF_VERDURE, 1);
                            qs.set_memo_state(ctx, 4);
                            qs.set_cond(ctx, 4, true);
                            htmltext = "30154-05.html".to_owned();
                        }
                    } else if memo_state == 4 {
                        htmltext = "30154-06.html".to_owned();
                    }
                }
                TETRARCH_THIFIELL => {
                    if memo_state == 4 {
                        if has(ctx, player, LETTER_TO_DARKELF) {
                            htmltext = "30358-01.html".to_owned();
                        }
                    } else if memo_state == 6 {
                        if has(ctx, player, ORDER_OF_CLAYTON)
                            && count(ctx, player, STAKATO_ICHOR)
                                + count(ctx, player, HONEY_DEW)
                                + count(ctx, player, BASILISK_PLASMA)
                                == 3
                        {
                            ctx.give_items(player, SCROLL_OF_DARKELF_TRUST, 1);
                            ctx.take_items(player, BASILISK_PLASMA, -1);
                            ctx.take_items(player, HONEY_DEW, -1);
                            ctx.take_items(player, STAKATO_ICHOR, -1);
                            ctx.take_items(player, ORDER_OF_CLAYTON, 1);
                            qs.set_memo_state(ctx, 7);
                            qs.set_cond(ctx, 9, true);
                            htmltext = "30358-03.html".to_owned();
                        }
                    } else if memo_state == 7 {
                        htmltext = "30358-04.html".to_owned();
                    } else if memo_state == 5 {
                        htmltext = "30358-05.html".to_owned();
                    }
                }
                MAGISTER_CLAYTON => {
                    if memo_state == 5 {
                        if has(ctx, player, LETTER_OF_THIFIELL) {
                            ctx.take_items(player, LETTER_OF_THIFIELL, 1);
                            ctx.give_items(player, ORDER_OF_CLAYTON, 1);
                            qs.set_memo_state(ctx, 6);
                            qs.set_cond(ctx, 6, true);
                            htmltext = "30464-01.html".to_owned();
                        }
                    } else if memo_state == 6 {
                        if has(ctx, player, ORDER_OF_CLAYTON)
                            && count(ctx, player, STAKATO_ICHOR)
                                + count(ctx, player, HONEY_DEW)
                                + count(ctx, player, BASILISK_PLASMA)
                                < 3
                        {
                            htmltext = "30464-02.html".to_owned();
                        } else {
                            qs.set_cond(ctx, 8, true);
                            htmltext = "30464-03.html".to_owned();
                        }
                    }
                }
                SEER_MANAKIA => {
                    if has(ctx, player, LETTER_TO_MANAKIA) {
                        htmltext = "30515-01.html".to_owned();
                    } else if memo_state == 11 {
                        htmltext = "30515-03.html".to_owned();
                    } else if memo_state == 12 {
                        if count(ctx, player, PARASITE_OF_LOTA) == 10 {
                            ctx.take_items(player, PARASITE_OF_LOTA, -1);
                            ctx.give_items(player, LETTER_OF_MANAKIA, 1);
                            qs.set_memo_state(ctx, 13);
                            qs.set_cond(ctx, 16, true);
                            htmltext = "30515-04.html".to_owned();
                        }
                    } else if memo_state == 13 {
                        htmltext = "30515-05.html".to_owned();
                    }
                }
                IRON_GATES_LOCKIRIN => {
                    if memo_state == 14 {
                        if has(ctx, player, LETTER_TO_DWARF) {
                            htmltext = "30531-01.html".to_owned();
                        }
                    } else if memo_state == 15 {
                        htmltext = "30531-03.html".to_owned();
                    } else if memo_state == 17 {
                        ctx.give_items(player, SCROLL_OF_DWARF_TRUST, 1);
                        qs.set_memo_state(ctx, 18);
                        qs.set_cond(ctx, 22, true);
                        htmltext = "30531-04.html".to_owned();
                    } else if memo_state == 18 {
                        htmltext = "30531-05.html".to_owned();
                    }
                }
                FLAME_LORD_KAKAI => {
                    if memo_state == 9 {
                        if has(ctx, player, LETTER_TO_ORC) {
                            htmltext = "30565-01.html".to_owned();
                        }
                    } else if memo_state == 10 {
                        htmltext = "30565-03.html".to_owned();
                    } else if memo_state == 13 {
                        ctx.give_items(player, SCROLL_OF_ORC_TRUST, 1);
                        ctx.take_items(player, LETTER_OF_MANAKIA, 1);
                        qs.set_memo_state(ctx, 14);
                        qs.set_cond(ctx, 17, true);
                        htmltext = "30565-04.html".to_owned();
                    } else if memo_state == 14 {
                        htmltext = "30565-05.html".to_owned();
                    }
                }
                MAESTRO_NIKOLA => {
                    if memo_state == 15 {
                        if has(ctx, player, LETTER_TO_NICHOLA) {
                            htmltext = "30621-01.html".to_owned();
                        }
                    } else if memo_state == 16 {
                        if !has(ctx, player, HEART_OF_PORTA) {
                            htmltext = "30621-03.html".to_owned();
                        } else {
                            ctx.take_items(player, ORDER_OF_NICHOLA, 1);
                            ctx.take_items(player, HEART_OF_PORTA, 1);
                            qs.set_memo_state(ctx, 17);
                            qs.set_cond(ctx, 21, true);
                            htmltext = "30621-04.html".to_owned();
                        }
                    } else if memo_state == 17 {
                        htmltext = "30621-05.html".to_owned();
                    }
                }
                CARDINAL_SERESIN => {
                    if memo_state == 8 {
                        if has(ctx, player, LETTER_TO_SERESIN) {
                            htmltext = "30657-01.html".to_owned();
                        }
                    } else if memo_state == 9 {
                        htmltext = "30657-04.html".to_owned();
                    } else if memo_state == 18 {
                        htmltext = "30657-05.html".to_owned();
                    }
                }
                _ => {}
            }
        } else if qs.is_completed(ctx) && npc.id == HIGH_PRIEST_HOLLINT {
            htmltext = ctx.get_already_completed_msg(player);
        }
        Some(htmltext)
    }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::ScriptRegistry;
    use l2_world::script::player_class;

    use super::*;
    use crate::testing::{Pages, ScriptTest, rows};

    const NAME: &str = "Q00217_TestimonyOfTrust";

    const NPCS: &str = r#"
        <npc id="30031" type="Folk" level="70" name="High Priest Biotin"/>
        <npc id="30154" type="Folk" level="70" name="Hierarch Asterios"/>
        <npc id="30191" type="Folk" level="70" name="High Priest Hollint"/>
        <npc id="30657" type="Folk" level="70" name="Cardinal Seresin"/>
        <npc id="20553" type="Monster" level="30" name="Windsus"/>
        <npc id="20213" type="Monster" level="30" name="Porta"/>
        <npc id="27120" type="Monster" level="30" name="Luell of Zephyr Winds"/>
        <npc id="27121" type="Monster" level="30" name="Actea of Verdant Wilds"/>"#;

    fn items() -> String {
        let mut s = String::from(
            r#"<item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="7562" type="EtcItem" name="Dimensional Diamond"><set name="is_stackable" val="true"/></item>
        <item id="2734" type="EtcItem" name="Mark of Trust"><set name="is_questitem" val="true"/></item>"#,
        );
        for id in 2735..=2761 {
            let stack = if matches!(id, 2749..=2751 | 2756) {
                r#"<set name="is_stackable" val="true"/>"#
            } else {
                ""
            };
            s.push_str(&format!(
                r#"<item id="{id}" type="EtcItem" name="Q217 {id}"><set name="is_questitem" val="true"/>{stack}</item>"#
            ));
        }
        s
    }

    const EVENTS: [&str; 4] = ["ACCEPT", "30154-03.html", "30657-03.html", "30154-02.html"];
    const FILES: [&str; 11] = [
        "30191-01.html",
        "30191-01a.html",
        "30191-01b.html",
        "30191-02.html",
        "30191-03.htm",
        "30191-04a.htm",
        "30191-05.html",
        "30191-09.html",
        "30154-01.html",
        "30657-01.html",
        "30031-01.html",
    ];

    struct Npcs {
        hollint: Oid,
        asterios: Oid,
        biotin: Oid,
        seresin: Oid,
    }

    fn setup() -> (ScriptTest, Npcs) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00217TestimonyOfTrust);
        let mut pages = Pages::of(&Q00217TestimonyOfTrust);
        for file in FILES {
            pages = pages.page(file, &EVENTS);
        }
        for event in EVENTS.iter().filter(|e| **e != "ACCEPT") {
            pages = pages.page(event, &EVENTS);
        }
        let pages = pages.npcs(&[
            HIGH_PRIEST_HOLLINT,
            HIGH_PRIEST_BIOTIN,
            HIERARCH_ASTERIOS,
            CARDINAL_SERESIN,
        ]);
        let mut t = ScriptTest::with_pages(registry, NPCS, &items(), pages);
        let npcs = Npcs {
            hollint: t.spawn(HIGH_PRIEST_HOLLINT, 20),
            asterios: t.spawn(HIERARCH_ASTERIOS, 30),
            biotin: t.spawn(HIGH_PRIEST_BIOTIN, 40),
            seresin: t.spawn(CARDINAL_SERESIN, 50),
        };
        (t, npcs)
    }

    #[test]
    fn only_a_human_of_level_37_in_a_second_class_is_offered_the_quest() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 37, race::ELF, player_class::ELVEN_KNIGHT, vec![]);
        assert!(t.quest_page(1, npcs.hollint).contains("30191-02.html"));
        t.enter_as(2, 36, race::HUMAN, player_class::KNIGHT, vec![]);
        assert!(t.quest_page(2, npcs.hollint).contains("30191-01.html"));
        t.enter_as(3, 37, race::HUMAN, player_class::FIGHTER, vec![]);
        assert!(t.quest_page(3, npcs.hollint).contains("30191-01a.html"));
        t.enter_as(4, 37, race::HUMAN, player_class::KNIGHT, vec![]);
        assert!(t.quest_page(4, npcs.hollint).contains("30191-03.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_elven_task_gives_the_scroll_of_elf_trust() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 37, race::HUMAN, player_class::KNIGHT, vec![]);
        assert!(t.quest_page(1, npcs.hollint).contains("30191-03.htm"));
        assert!(t.event(1, NAME, "ACCEPT").contains("30191-04a.htm"));
        assert_eq!(t.count(1, DIMENSIONAL_DIAMOND), 96);
        assert_eq!(t.count(1, LETTER_TO_ELF), 1);
        assert_eq!(t.count(1, LETTER_TO_DARKELF), 1);
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started"),
                rows::insert(NAME, "memoState", "1"),
            ]
        );

        assert!(t.quest_page(1, npcs.asterios).contains("30154-01.html"));
        assert!(t.event(1, NAME, "30154-03.html").contains("30154-03.html"));
        assert_eq!(
            t.take_writes(),
            [
                rows::update(NAME, "memoState", "2"),
                rows::update(NAME, "cond", "2")
            ]
        );

        // The quest monsters (spawned by hand: their appearance is a 33% roll on a dryad kill).
        let luell = t.spawn(LUELL_OF_ZEPHYR_WINDS, 60);
        t.kill(1, luell);
        t.world.delete_npc(luell);
        let actea = t.spawn(ACTEA_OF_VERDANT_WILDS, 60);
        t.kill(1, actea);
        t.world.delete_npc(actea);
        assert_eq!(t.count(1, BREATH_OF_WINDS), 1);
        assert_eq!(t.count(1, SEED_OF_VERDURE), 1);
        t.take_writes();

        t.quest_page(1, npcs.asterios);
        assert_eq!(t.count(1, SCROLL_OF_ELF_TRUST), 1);
        assert_eq!(t.count(1, ORDER_OF_ASTERIOS), 0);
    }

    #[test]
    fn seresin_hands_the_dwarf_and_orc_letters() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 37, race::HUMAN, player_class::KNIGHT, vec![]);
        assert!(t.quest_page(1, npcs.hollint).contains("30191-03.htm"));
        assert!(t.event(1, NAME, "ACCEPT").contains("30191-04a.htm"));
        t.world.run_as_script(NAME, |ctx| {
            let qs = ctx.get_quest_state(Oid(1), false).unwrap();
            qs.set_memo_state(ctx, 8);
            ctx.give_items(Oid(1), LETTER_TO_SERESIN, 1);
        });
        t.take_writes();
        assert!(t.quest_page(1, npcs.seresin).contains("30657-01.html"));
        assert!(t.event(1, NAME, "30657-03.html").contains("30657-03.html"));
        assert_eq!(t.count(1, LETTER_TO_DWARF), 1);
        assert_eq!(t.count(1, LETTER_TO_ORC), 1);
        // Cond 8 to 12 skips steps: the engine also writes the completed-steps flags.
        let writes = t.take_writes();
        assert_eq!(
            writes[..2],
            [
                rows::update(NAME, "memoState", "9"),
                rows::update(NAME, "cond", "12")
            ]
        );
    }

    #[test]
    fn biotin_pays_the_mark_for_the_recommendation() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 37, race::HUMAN, player_class::KNIGHT, vec![]);
        assert!(t.quest_page(1, npcs.hollint).contains("30191-03.htm"));
        assert!(t.event(1, NAME, "ACCEPT").contains("30191-04a.htm"));
        t.world.run_as_script(NAME, |ctx| {
            let qs = ctx.get_quest_state(Oid(1), false).unwrap();
            qs.set_memo_state(ctx, 19);
            ctx.give_items(Oid(1), RECOMMENDATION_OF_HOLLIN, 1);
        });
        t.take_writes();
        assert!(t.quest_page(1, npcs.biotin).contains("30031-01.html"));
        assert_eq!(t.count(1, MARK_OF_TRUST), 1);
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed")
            ]
        );
        assert!(t.quest_page(1, npcs.hollint).contains("already been completed"));
    }
}
