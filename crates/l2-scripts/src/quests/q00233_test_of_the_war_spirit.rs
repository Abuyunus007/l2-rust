//! Quest 233: Test of the War Spirit (the Orc Shaman's second class change: Seer Somak, Priestess
//! Vivyan, Trader Sarien, Seers Racoy, Manakia and Pekiron, Shadow Orim and Ancestor Martankus).
// Java: dist/game/data/scripts/quests/Q00233_TestOfTheWarSpirit/Q00233_TestOfTheWarSpirit.java
//
// Pattern: four "remains" are assembled from monster drops in parallel (no memo state), then
// the Tamlin orc heads; the reward is given by an event of the last NPC.

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, STARTED, player_class, race};
use l2_world::{QuestSound, Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const PRIESTESS_VIVYAN: i32 = 30030;
const TRADER_SARIEN: i32 = 30436;
const SEER_RACOY: i32 = 30507;
const SEER_SOMAK: i32 = 30510;
const SEER_MANAKIA: i32 = 30515;
const SHADOW_ORIM: i32 = 30630;
const ANCESTOR_MARTANKUS: i32 = 30649;
const SEER_PEKIRON: i32 = 30682;

// Items
const VENDETTA_TOTEM: i32 = 2880;
const TAMLIN_ORC_HEAD: i32 = 2881;
const WARSPIRIT_TOTEM: i32 = 2882;
const ORIMS_CONTRACT: i32 = 2883;
const PORTAS_EYE: i32 = 2884;
const EXCUROS_SCALE: i32 = 2885;
const MORDEOS_TALON: i32 = 2886;
const BRAKIS_REMAINS1: i32 = 2887;
const PEKIRONS_TOTEM: i32 = 2888;
const TONARS_SKULL: i32 = 2889;
const TONARS_RIB_BONE: i32 = 2890;
const TONARS_SPINE: i32 = 2891;
const TONARS_ARM_BONE: i32 = 2892;
const TONARS_THIGH_BONE: i32 = 2893;
const TONARS_REMAINS1: i32 = 2894;
const MANAKIAS_TOTEM: i32 = 2895;
const HERMODTS_SKULL: i32 = 2896;
const HERMODTS_RIB_BONE: i32 = 2897;
const HERMODTS_SPINE: i32 = 2898;
const HERMODTS_ARM_BONE: i32 = 2899;
const HERMODTS_THIGH_BONE: i32 = 2900;
const HERMODTS_REMAINS1: i32 = 2901;
const RACOYS_TOTEM: i32 = 2902;
const VIVIANTES_LETTER: i32 = 2903;
const INSECT_DIAGRAM_BOOK: i32 = 2904;
const KIRUNAS_SKULL: i32 = 2905;
const KIRUNAS_RIB_BONE: i32 = 2906;
const KIRUNAS_SPINE: i32 = 2907;
const KIRUNAS_ARM_BONE: i32 = 2908;
const KIRUNAS_THIGH_BONE: i32 = 2909;
const KIRUNAS_REMAINS1: i32 = 2910;
const BRAKIS_REMAINS2: i32 = 2911;
const TONARS_REMAINS2: i32 = 2912;
const HERMODTS_REMAINS2: i32 = 2913;
const KIRUNAS_REMAINS2: i32 = 2914;

// Reward
const MARK_OF_WARSPIRIT: i32 = 2879;
const DIMENSIONAL_DIAMOND: i32 = 7562;

// Monster
const NOBLE_ANT: i32 = 20089;
const NOBLE_ANT_LEADER: i32 = 20090;
const MEDUSA: i32 = 20158;
const PORTA: i32 = 20213;
const EXCURO: i32 = 20214;
const MORDERO: i32 = 20215;
const LETO_LIZARDMAN_SHAMAN: i32 = 20581;
const LETO_LIZARDMAN_OVERLORD: i32 = 20582;
const TAMLIN_ORC: i32 = 20601;
const TAMLIN_ORC_ARCHER: i32 = 20602;

// Quest Monster
const STENOA_GORGON_QUEEN: i32 = 27108;

// Misc
const MIN_LEVEL: i32 = 39;

/// Java `Q00233_TestOfTheWarSpirit`.
pub struct Q00233TestOfTheWarSpirit;

fn has(ctx: &ScriptCtx<'_>, p: ObjectId, item: i32) -> bool {
    ctx.has_quest_items(p, item)
}

fn all(ctx: &ScriptCtx<'_>, p: ObjectId, items: &[i32]) -> bool {
    ctx.has_all_quest_items(p, items)
}

fn any(ctx: &ScriptCtx<'_>, p: ObjectId, items: &[i32]) -> bool {
    ctx.has_at_least_one_quest_item(p, items)
}

/// The kill drops that give the first missing piece (`giveItems` + the middle sound): nothing when
/// one was given (Java chains `if (!has(a)) ... else if (!has(b))`).
fn give_first_missing(ctx: &mut ScriptCtx<'_>, killer: ObjectId, pieces: &[i32]) {
    for &piece in pieces {
        if !has(ctx, killer, piece) {
            ctx.give_items(killer, piece, 1);
            ctx.play_sound(killer, QuestSound::ItemsoundQuestMiddle);
            return;
        }
    }
}

impl Script for Q00233TestOfTheWarSpirit {
    fn name(&self) -> &'static str {
        "Q00233_TestOfTheWarSpirit"
    }

    fn path(&self) -> &'static str {
        "quests/Q00233_TestOfTheWarSpirit"
    }

    fn id(&self) -> i32 {
        233
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[SEER_SOMAK]);
        s.add_talk_id(&[
            SEER_SOMAK,
            PRIESTESS_VIVYAN,
            TRADER_SARIEN,
            SEER_RACOY,
            SEER_MANAKIA,
            SHADOW_ORIM,
            ANCESTOR_MARTANKUS,
            SEER_PEKIRON,
        ]);
        s.add_kill_id(&[
            NOBLE_ANT,
            NOBLE_ANT_LEADER,
            MEDUSA,
            PORTA,
            EXCURO,
            MORDERO,
            LETO_LIZARDMAN_SHAMAN,
            LETO_LIZARDMAN_OVERLORD,
            TAMLIN_ORC,
            TAMLIN_ORC_ARCHER,
            STENOA_GORGON_QUEEN,
        ]);
        s.register_quest_items(&[
            VENDETTA_TOTEM,
            TAMLIN_ORC_HEAD,
            WARSPIRIT_TOTEM,
            ORIMS_CONTRACT,
            PORTAS_EYE,
            EXCUROS_SCALE,
            MORDEOS_TALON,
            BRAKIS_REMAINS1,
            PEKIRONS_TOTEM,
            TONARS_SKULL,
            TONARS_RIB_BONE,
            TONARS_SPINE,
            TONARS_ARM_BONE,
            TONARS_THIGH_BONE,
            TONARS_REMAINS1,
            MANAKIAS_TOTEM,
            HERMODTS_SKULL,
            HERMODTS_RIB_BONE,
            HERMODTS_SPINE,
            HERMODTS_ARM_BONE,
            HERMODTS_THIGH_BONE,
            HERMODTS_REMAINS1,
            RACOYS_TOTEM,
            VIVIANTES_LETTER,
            INSECT_DIAGRAM_BOOK,
            KIRUNAS_SKULL,
            KIRUNAS_RIB_BONE,
            KIRUNAS_SPINE,
            KIRUNAS_ARM_BONE,
            KIRUNAS_THIGH_BONE,
            KIRUNAS_REMAINS1,
            BRAKIS_REMAINS2,
            TONARS_REMAINS2,
            HERMODTS_REMAINS2,
            KIRUNAS_REMAINS2,
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
                    if ctx.player_variable_int(player, "2ND_CLASS_DIAMOND_REWARD", 0) == 0 {
                        ctx.give_items(player, DIMENSIONAL_DIAMOND, 92);
                        ctx.set_player_variable(player, "2ND_CLASS_DIAMOND_REWARD", "1");
                        htmltext = Some("30510-05e.htm".to_owned());
                    } else {
                        htmltext = Some("30510-05.htm".to_owned());
                    }
                }
            }
            "30510-05a.html" | "30510-05b.html" | "30510-05c.html" | "30510-05d.html" | "30510-05.html"
            | "30030-02.html" | "30030-03.html" | "30630-02.html" | "30630-03.html" | "30649-02.html" => {
                htmltext = Some(event.to_owned());
            }
            "30030-04.html" => {
                ctx.give_items(player, VIVIANTES_LETTER, 1);
                htmltext = Some(event.to_owned());
            }
            "30507-02.html" => {
                ctx.give_items(player, RACOYS_TOTEM, 1);
                htmltext = Some(event.to_owned());
            }
            "30515-02.html" => {
                ctx.give_items(player, MANAKIAS_TOTEM, 1);
                htmltext = Some(event.to_owned());
            }
            "30630-04.html" => {
                ctx.give_items(player, ORIMS_CONTRACT, 1);
                htmltext = Some(event.to_owned());
            }
            "30649-03.html" => {
                if has(ctx, player, TONARS_REMAINS2) {
                    ctx.give_adena(player, 161_806, true);
                    ctx.give_items(player, MARK_OF_WARSPIRIT, 1);
                    ctx.add_exp_and_sp(player, 894_888, 61_408);
                    qs.exit_quest(ctx, false, true);
                    ctx.send_social_action(player, 3);
                    htmltext = Some(event.to_owned());
                }
            }
            "30682-02.html" => {
                ctx.give_items(player, PEKIRONS_TOTEM, 1);
                htmltext = Some(event.to_owned());
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
            NOBLE_ANT | NOBLE_ANT_LEADER => {
                if all(ctx, killer, &[RACOYS_TOTEM, INSECT_DIAGRAM_BOOK]) {
                    let i0 = ctx.get_random(100);
                    if i0 > 65 {
                        give_first_missing(ctx, killer, &[KIRUNAS_THIGH_BONE, KIRUNAS_ARM_BONE]);
                    } else if i0 > 30 {
                        give_first_missing(ctx, killer, &[KIRUNAS_SPINE, KIRUNAS_RIB_BONE]);
                    } else if i0 > 0 {
                        give_first_missing(ctx, killer, &[KIRUNAS_SKULL]);
                    }
                }
            }
            MEDUSA => {
                if has(ctx, killer, MANAKIAS_TOTEM) {
                    give_first_missing(
                        ctx,
                        killer,
                        &[
                            HERMODTS_RIB_BONE,
                            HERMODTS_SPINE,
                            HERMODTS_ARM_BONE,
                            HERMODTS_THIGH_BONE,
                        ],
                    );
                }
            }
            PORTA => {
                if has(ctx, killer, ORIMS_CONTRACT) {
                    ctx.give_item_randomly(killer, Some(npc), PORTAS_EYE, 2, 2, 10, 1.0, true);
                }
            }
            EXCURO => {
                if has(ctx, killer, ORIMS_CONTRACT) {
                    ctx.give_item_randomly(killer, Some(npc), EXCUROS_SCALE, 5, 5, 10, 1.0, true);
                }
            }
            MORDERO => {
                if has(ctx, killer, ORIMS_CONTRACT) {
                    ctx.give_item_randomly(killer, Some(npc), MORDEOS_TALON, 5, 5, 10, 1.0, true);
                }
            }
            LETO_LIZARDMAN_SHAMAN | LETO_LIZARDMAN_OVERLORD => {
                if has(ctx, killer, PEKIRONS_TOTEM) {
                    give_first_missing(
                        ctx,
                        killer,
                        &[
                            TONARS_SKULL,
                            TONARS_RIB_BONE,
                            TONARS_SPINE,
                            TONARS_ARM_BONE,
                            TONARS_THIGH_BONE,
                        ],
                    );
                }
            }
            TAMLIN_ORC | TAMLIN_ORC_ARCHER => {
                if has(ctx, killer, VENDETTA_TOTEM)
                    && ctx.give_item_randomly(killer, Some(npc), TAMLIN_ORC_HEAD, 1, 1, 13, 1.0, true)
                {
                    qs.set_cond(ctx, 4, true);
                }
            }
            STENOA_GORGON_QUEEN => {
                if has(ctx, killer, MANAKIAS_TOTEM) && !has(ctx, killer, HERMODTS_SKULL) {
                    ctx.give_items(killer, HERMODTS_SKULL, 1);
                    ctx.play_sound(killer, QuestSound::ItemsoundQuestMiddle);
                }
            }
            _ => {}
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        let state = qs.state(ctx);
        let p = player;
        if state == CREATED {
            if npc.id == SEER_SOMAK {
                htmltext = if ctx.player_race(player) == race::ORC {
                    if ctx.player_class_id(player) == player_class::ORC_SHAMAN {
                        if ctx.player_level(player) < MIN_LEVEL {
                            "30510-03.html"
                        } else {
                            "30510-04.htm"
                        }
                    } else {
                        "30510-02.html"
                    }
                } else {
                    "30510-01.html"
                }
                .to_owned();
            }
        } else if state == STARTED {
            match npc.id {
                SEER_SOMAK => {
                    if !any(ctx, p, &[VENDETTA_TOTEM, WARSPIRIT_TOTEM]) {
                        if all(
                            ctx,
                            p,
                            &[
                                BRAKIS_REMAINS1,
                                HERMODTS_REMAINS1,
                                KIRUNAS_REMAINS1,
                                TONARS_REMAINS1,
                            ],
                        ) {
                            ctx.give_items(p, VENDETTA_TOTEM, 1);
                            ctx.take_items(p, BRAKIS_REMAINS1, 1);
                            ctx.take_items(p, TONARS_REMAINS1, 1);
                            ctx.take_items(p, HERMODTS_REMAINS1, 1);
                            ctx.take_items(p, KIRUNAS_REMAINS1, 1);
                            qs.set_cond(ctx, 3, false);
                            htmltext = "30510-07.html".to_owned();
                        } else {
                            htmltext = "30510-06.html".to_owned();
                        }
                    } else if has(ctx, p, VENDETTA_TOTEM) {
                        if ctx.get_quest_items_count(p, TAMLIN_ORC_HEAD) < 13 {
                            htmltext = "30510-08.html".to_owned();
                        } else {
                            ctx.take_items(p, VENDETTA_TOTEM, 1);
                            ctx.give_items(p, WARSPIRIT_TOTEM, 1);
                            ctx.give_items(p, BRAKIS_REMAINS2, 1);
                            ctx.give_items(p, TONARS_REMAINS2, 1);
                            ctx.give_items(p, HERMODTS_REMAINS2, 1);
                            ctx.give_items(p, KIRUNAS_REMAINS2, 1);
                            qs.set_cond(ctx, 5, false);
                            htmltext = "30510-09.html".to_owned();
                        }
                    } else if has(ctx, p, WARSPIRIT_TOTEM) {
                        htmltext = "30510-10.html".to_owned();
                    }
                }
                PRIESTESS_VIVYAN => {
                    if has(ctx, p, RACOYS_TOTEM) && !any(ctx, p, &[VIVIANTES_LETTER, INSECT_DIAGRAM_BOOK]) {
                        htmltext = "30030-01.html".to_owned();
                    } else if all(ctx, p, &[RACOYS_TOTEM, VIVIANTES_LETTER])
                        && !has(ctx, p, INSECT_DIAGRAM_BOOK)
                    {
                        htmltext = "30030-05.html".to_owned();
                    } else if all(ctx, p, &[RACOYS_TOTEM, INSECT_DIAGRAM_BOOK])
                        && !has(ctx, p, VIVIANTES_LETTER)
                    {
                        htmltext = "30030-06.html".to_owned();
                    } else if !has(ctx, p, RACOYS_TOTEM)
                        && any(ctx, p, &[KIRUNAS_REMAINS1, KIRUNAS_REMAINS2, VENDETTA_TOTEM])
                    {
                        htmltext = "30030-07.html".to_owned();
                    }
                }
                TRADER_SARIEN => {
                    if all(ctx, p, &[RACOYS_TOTEM, VIVIANTES_LETTER]) && !has(ctx, p, INSECT_DIAGRAM_BOOK) {
                        ctx.take_items(p, VIVIANTES_LETTER, 1);
                        ctx.give_items(p, INSECT_DIAGRAM_BOOK, 1);
                        htmltext = "30436-01.html".to_owned();
                    } else if all(ctx, p, &[RACOYS_TOTEM, INSECT_DIAGRAM_BOOK])
                        && !has(ctx, p, VIVIANTES_LETTER)
                    {
                        htmltext = "30436-02.html".to_owned();
                    } else if !has(ctx, p, RACOYS_TOTEM)
                        && any(ctx, p, &[KIRUNAS_REMAINS1, KIRUNAS_REMAINS2, VENDETTA_TOTEM])
                    {
                        htmltext = "30436-03.html".to_owned();
                    }
                }
                SEER_RACOY => {
                    if !any(
                        ctx,
                        p,
                        &[RACOYS_TOTEM, KIRUNAS_REMAINS1, KIRUNAS_REMAINS2, VENDETTA_TOTEM],
                    ) {
                        htmltext = "30507-01.html".to_owned();
                    } else if has(ctx, p, RACOYS_TOTEM)
                        && !any(ctx, p, &[VIVIANTES_LETTER, INSECT_DIAGRAM_BOOK])
                    {
                        htmltext = "30507-03.html".to_owned();
                    } else if all(ctx, p, &[RACOYS_TOTEM, VIVIANTES_LETTER])
                        && !has(ctx, p, INSECT_DIAGRAM_BOOK)
                    {
                        htmltext = "30507-04.html".to_owned();
                    } else if all(ctx, p, &[RACOYS_TOTEM, INSECT_DIAGRAM_BOOK])
                        && !has(ctx, p, VIVIANTES_LETTER)
                    {
                        if all(
                            ctx,
                            p,
                            &[
                                KIRUNAS_SKULL,
                                KIRUNAS_RIB_BONE,
                                KIRUNAS_SPINE,
                                KIRUNAS_ARM_BONE,
                                KIRUNAS_THIGH_BONE,
                            ],
                        ) {
                            ctx.take_items(p, RACOYS_TOTEM, 1);
                            ctx.take_items(p, INSECT_DIAGRAM_BOOK, 1);
                            ctx.take_items(p, KIRUNAS_SKULL, 1);
                            ctx.take_items(p, KIRUNAS_RIB_BONE, 1);
                            ctx.take_items(p, KIRUNAS_SPINE, 1);
                            ctx.take_items(p, KIRUNAS_ARM_BONE, 1);
                            ctx.take_items(p, KIRUNAS_THIGH_BONE, 1);
                            ctx.give_items(p, KIRUNAS_REMAINS1, 1);
                            if all(ctx, p, &[BRAKIS_REMAINS1, HERMODTS_REMAINS1, TONARS_REMAINS1]) {
                                qs.set_cond(ctx, 2, false);
                            }
                            htmltext = "30507-06.html".to_owned();
                        } else {
                            htmltext = "30507-05.html".to_owned();
                        }
                    } else if !has(ctx, p, RACOYS_TOTEM)
                        && any(ctx, p, &[KIRUNAS_REMAINS1, KIRUNAS_REMAINS2, VENDETTA_TOTEM])
                    {
                        htmltext = "30507-07.html".to_owned();
                    }
                }
                SEER_MANAKIA => {
                    if !any(
                        ctx,
                        p,
                        &[
                            MANAKIAS_TOTEM,
                            HERMODTS_REMAINS2,
                            VENDETTA_TOTEM,
                            HERMODTS_REMAINS1,
                        ],
                    ) {
                        htmltext = "30515-01.html".to_owned();
                    } else if has(ctx, p, MANAKIAS_TOTEM) {
                        if all(
                            ctx,
                            p,
                            &[
                                HERMODTS_SKULL,
                                HERMODTS_RIB_BONE,
                                HERMODTS_SPINE,
                                HERMODTS_ARM_BONE,
                                HERMODTS_THIGH_BONE,
                            ],
                        ) {
                            ctx.take_items(p, MANAKIAS_TOTEM, 1);
                            ctx.take_items(p, HERMODTS_SKULL, 1);
                            ctx.take_items(p, HERMODTS_RIB_BONE, 1);
                            ctx.take_items(p, HERMODTS_SPINE, 1);
                            ctx.take_items(p, HERMODTS_ARM_BONE, 1);
                            ctx.take_items(p, HERMODTS_THIGH_BONE, 1);
                            ctx.give_items(p, HERMODTS_REMAINS1, 1);
                            if all(ctx, p, &[BRAKIS_REMAINS1, KIRUNAS_REMAINS1, TONARS_REMAINS1]) {
                                qs.set_cond(ctx, 2, false);
                            }
                            htmltext = "30515-04.html".to_owned();
                        } else {
                            htmltext = "30515-03.html".to_owned();
                        }
                    } else if !has(ctx, p, MANAKIAS_TOTEM)
                        && any(ctx, p, &[HERMODTS_REMAINS1, HERMODTS_REMAINS2, VENDETTA_TOTEM])
                    {
                        htmltext = "30515-05.html".to_owned();
                    }
                }
                SHADOW_ORIM => {
                    if !any(
                        ctx,
                        p,
                        &[ORIMS_CONTRACT, BRAKIS_REMAINS1, BRAKIS_REMAINS2, VENDETTA_TOTEM],
                    ) {
                        htmltext = "30630-01.html".to_owned();
                    } else if has(ctx, p, ORIMS_CONTRACT) {
                        if ctx.get_quest_items_count(p, PORTAS_EYE)
                            + ctx.get_quest_items_count(p, EXCUROS_SCALE)
                            + ctx.get_quest_items_count(p, MORDEOS_TALON)
                            < 30
                        {
                            htmltext = "30630-05.html".to_owned();
                        } else {
                            ctx.take_items(p, ORIMS_CONTRACT, 1);
                            ctx.take_items(p, PORTAS_EYE, -1);
                            ctx.take_items(p, EXCUROS_SCALE, -1);
                            ctx.take_items(p, MORDEOS_TALON, -1);
                            ctx.give_items(p, BRAKIS_REMAINS1, 1);
                            if all(ctx, p, &[HERMODTS_REMAINS1, KIRUNAS_REMAINS1, TONARS_REMAINS1]) {
                                qs.set_cond(ctx, 2, false);
                            }
                            htmltext = "30630-06.html".to_owned();
                        }
                    } else if !has(ctx, p, ORIMS_CONTRACT)
                        && any(ctx, p, &[BRAKIS_REMAINS1, BRAKIS_REMAINS2, VENDETTA_TOTEM])
                    {
                        htmltext = "30630-07.html".to_owned();
                    }
                }
                ANCESTOR_MARTANKUS => {
                    if has(ctx, p, WARSPIRIT_TOTEM) {
                        htmltext = "30649-01.html".to_owned();
                    }
                }
                SEER_PEKIRON => {
                    if !any(
                        ctx,
                        p,
                        &[PEKIRONS_TOTEM, TONARS_REMAINS1, TONARS_REMAINS2, VENDETTA_TOTEM],
                    ) {
                        htmltext = "30682-01.html".to_owned();
                    } else if has(ctx, p, PEKIRONS_TOTEM) {
                        if all(
                            ctx,
                            p,
                            &[
                                TONARS_SKULL,
                                TONARS_RIB_BONE,
                                TONARS_SPINE,
                                TONARS_ARM_BONE,
                                TONARS_THIGH_BONE,
                            ],
                        ) {
                            ctx.take_items(p, PEKIRONS_TOTEM, 1);
                            ctx.take_items(p, TONARS_SKULL, 1);
                            ctx.take_items(p, TONARS_RIB_BONE, 1);
                            ctx.take_items(p, TONARS_SPINE, 1);
                            ctx.take_items(p, TONARS_ARM_BONE, 1);
                            ctx.take_items(p, TONARS_THIGH_BONE, 1);
                            ctx.give_items(p, TONARS_REMAINS1, 1);
                            if all(ctx, p, &[BRAKIS_REMAINS1, HERMODTS_REMAINS1, KIRUNAS_REMAINS1]) {
                                qs.set_cond(ctx, 2, false);
                            }
                            htmltext = "30682-04.html".to_owned();
                        } else {
                            htmltext = "30682-03.html".to_owned();
                        }
                    } else if !has(ctx, p, PEKIRONS_TOTEM)
                        && any(ctx, p, &[TONARS_REMAINS1, TONARS_REMAINS2, VENDETTA_TOTEM])
                    {
                        htmltext = "30682-05.html".to_owned();
                    }
                }
                _ => {}
            }
        } else if state == COMPLETED && npc.id == SEER_SOMAK {
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

    const NAME: &str = "Q00233_TestOfTheWarSpirit";

    const NPCS: &str = r#"
        <npc id="30510" type="Folk" level="70" name="Seer Somak"/>
        <npc id="30630" type="Folk" level="70" name="Shadow Orim"/>
        <npc id="30649" type="Folk" level="70" name="Ancestor Martankus"/>
        <npc id="20213" type="Monster" level="40" name="Porta"/>
        <npc id="20601" type="Monster" level="40" name="Tamlin Orc"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="7562" type="EtcItem" name="Dimensional Diamond"><set name="is_stackable" val="true"/></item>
        <item id="2879" type="EtcItem" name="Mark of Warspirit"><set name="is_questitem" val="true"/></item>
        <item id="2880" type="EtcItem" name="Vendetta Totem"><set name="is_questitem" val="true"/></item>
        <item id="2881" type="EtcItem" name="Tamlin Orc Head"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="2882" type="EtcItem" name="Warspirit Totem"><set name="is_questitem" val="true"/></item>
        <item id="2883" type="EtcItem" name="Orim's Contract"><set name="is_questitem" val="true"/></item>
        <item id="2884" type="EtcItem" name="Porta's Eye"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="2885" type="EtcItem" name="Excuro's Scale"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="2886" type="EtcItem" name="Mordeo's Talon"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="2887" type="EtcItem" name="Braki's Remains 1"><set name="is_questitem" val="true"/></item>
        <item id="2894" type="EtcItem" name="Tonar's Remains 1"><set name="is_questitem" val="true"/></item>
        <item id="2901" type="EtcItem" name="Hermodt's Remains 1"><set name="is_questitem" val="true"/></item>
        <item id="2910" type="EtcItem" name="Kiruna's Remains 1"><set name="is_questitem" val="true"/></item>
        <item id="2911" type="EtcItem" name="Braki's Remains 2"><set name="is_questitem" val="true"/></item>
        <item id="2912" type="EtcItem" name="Tonar's Remains 2"><set name="is_questitem" val="true"/></item>
        <item id="2913" type="EtcItem" name="Hermodt's Remains 2"><set name="is_questitem" val="true"/></item>
        <item id="2914" type="EtcItem" name="Kiruna's Remains 2"><set name="is_questitem" val="true"/></item>"#;

    fn pages() -> Pages {
        Pages::of(&Q00233TestOfTheWarSpirit)
            .plain(&["30510-01.html", "30510-02.html", "30510-03.html"])
            .page("30510-04.htm", &["ACCEPT"])
            .page("30510-05e.htm", &["30510-05a.html"])
            .plain(&[
                "30510-05.htm",
                "30510-05a.html",
                "30510-06.html",
                "30510-07.html",
                "30510-08.html",
            ])
            .plain(&["30510-09.html", "30510-10.html"])
            .page("30630-01.html", &["30630-02.html"])
            .page("30630-02.html", &["30630-03.html"])
            .page("30630-03.html", &["30630-04.html"])
            .plain(&["30630-04.html", "30630-05.html", "30630-06.html", "30630-07.html"])
            .page("30649-01.html", &["30649-02.html"])
            .page("30649-02.html", &["30649-03.html"])
            .plain(&["30649-03.html"])
            .npcs(&[SEER_SOMAK, SHADOW_ORIM, ANCESTOR_MARTANKUS])
    }

    struct Npcs {
        somak: Oid,
        orim: Oid,
        martankus: Oid,
    }

    fn setup() -> (ScriptTest, Npcs) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00233TestOfTheWarSpirit);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages());
        let npcs = Npcs {
            somak: t.spawn(SEER_SOMAK, 20),
            orim: t.spawn(SHADOW_ORIM, 40),
            martankus: t.spawn(ANCESTOR_MARTANKUS, 60),
        };
        (t, npcs)
    }

    #[test]
    fn only_an_orc_shaman_of_level_39_is_offered_the_test() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 40, 0, player_class::ORC_SHAMAN, vec![]);
        assert!(t.quest_page(1, npcs.somak).contains("30510-01.html"));
        t.enter_as(2, 40, 3, player_class::FIGHTER, vec![]);
        assert!(t.quest_page(2, npcs.somak).contains("30510-02.html"));
        t.enter_as(3, 38, 3, player_class::ORC_SHAMAN, vec![]);
        assert!(t.quest_page(3, npcs.somak).contains("30510-03.html"));
        t.enter_as(4, 39, 3, player_class::ORC_SHAMAN, vec![]);
        assert!(t.quest_page(4, npcs.somak).contains("30510-04.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn accepting_gives_the_diamonds_once() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 39, 3, player_class::ORC_SHAMAN, vec![]);
        t.quest_page(1, npcs.somak);
        assert!(t.event(1, NAME, "ACCEPT").contains("30510-05e.htm"));
        assert_eq!(t.count(1, DIMENSIONAL_DIAMOND), 92);
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
    }

    #[test]
    fn orims_contract_is_filled_by_the_three_monsters() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 39, 3, player_class::ORC_SHAMAN, vec![]);
        t.quest_page(1, npcs.somak);
        t.event(1, NAME, "ACCEPT");
        // Without the contract Porta drops nothing.
        let porta = t.spawn(20213, 90);
        t.kill(1, porta);
        assert_eq!(t.count(1, PORTAS_EYE), 0);

        assert!(t.quest_page(1, npcs.orim).contains("30630-01.html"));
        t.event(1, NAME, "30630-02.html");
        t.event(1, NAME, "30630-03.html");
        assert!(t.event(1, NAME, "30630-04.html").contains("30630-04.html"));
        assert_eq!(t.count(1, ORIMS_CONTRACT), 1);
        let porta2 = t.spawn(20213, 100);
        t.kill(1, porta2);
        assert_eq!(t.count(1, PORTAS_EYE), 2);
        assert!(t.quest_page(1, npcs.orim).contains("30630-05.html"));
        // 30 pieces: the contract becomes Braki's remains.
        t.world
            .run_as_script(NAME, |ctx| ctx.give_items(Oid(1), EXCUROS_SCALE, 28));
        assert!(t.quest_page(1, npcs.orim).contains("30630-06.html"));
        assert_eq!((t.count(1, ORIMS_CONTRACT), t.count(1, BRAKIS_REMAINS1)), (0, 1));
    }

    #[test]
    fn the_heads_start_the_last_step_and_the_mark_ends_the_quest() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 39, 3, player_class::ORC_SHAMAN, vec![]);
        t.quest_page(1, npcs.somak);
        t.event(1, NAME, "ACCEPT");
        t.world.run_as_script(NAME, |ctx| {
            for item in [
                BRAKIS_REMAINS1,
                HERMODTS_REMAINS1,
                KIRUNAS_REMAINS1,
                TONARS_REMAINS1,
            ] {
                ctx.give_items(Oid(1), item, 1);
            }
        });
        t.take_writes();
        assert!(t.quest_page(1, npcs.somak).contains("30510-07.html"));
        assert_eq!(t.count(1, VENDETTA_TOTEM), 1);
        // cond 2 was skipped: the flags record it.
        assert_eq!(
            t.take_writes(),
            [
                rows::update(NAME, "cond", "3"),
                rows::insert(NAME, "__compltdStateFlags", "-2147483643")
            ]
        );

        // The thirteenth head sets cond 4; the limit stops the count at 13.
        for dx in 0..14 {
            let orc = t.spawn(20601, 110 + dx * 10);
            t.kill(1, orc);
        }
        assert_eq!(t.count(1, TAMLIN_ORC_HEAD), 13);
        assert!(t.quest_page(1, npcs.somak).contains("30510-09.html"));
        assert_eq!((t.count(1, WARSPIRIT_TOTEM), t.count(1, TONARS_REMAINS2)), (1, 1));

        t.quest_page(1, npcs.martankus);
        t.event(1, NAME, "30649-02.html");
        t.take_writes();
        t.take();
        assert!(t.event(1, NAME, "30649-03.html").contains("30649-03.html"));
        assert_eq!((t.count(1, MARK_OF_WARSPIRIT), t.count(1, 57)), (1, 161_806));
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed")
            ]
        );
    }
}
