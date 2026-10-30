//! Quest 415: Path of the Orc Monk (Gantaki, Kasman, Rosheek, Toruku, Moira and Aren).
// Java: dist/game/data/scripts/quests/Q00415_PathOfTheOrcMonk/Q00415_PathOfTheOrcMonk.java
//
// Pattern: leather pouches. Each pouch is filled by killing a monster that drops the same item
// (up to four), the fourth kill turns the pouch into its "full" form, and the next NPC swaps the
// full pouch for the next one. Monsters count only for a player who hit them first, with fists
// or no weapon (`onAttack` sets the NPC's script value, `checkWeapon`). The memo state marks the
// second half (Aren and Moira). `docs/design/scripts.md`, "Quest patterns".

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, QuestSound, STARTED, player_class};
use l2_world::{Qs, Script, ScriptCtx, ScriptNpc, ScriptSetup, SkillKey};

// NPCs
const PREFECT_KASMAN: i32 = 30501;
const GANTAKI_ZU_URUTU: i32 = 30587;
const KHAVATARI_ROSHEEK: i32 = 30590;
const KHAVATARI_TORUKU: i32 = 30591;
const SEER_MOIRA: i32 = 31979;
const KHAVATARI_AREN: i32 = 32056;

// Items
const POMEGRANATE: i32 = 1593;
const LEATHER_POUCH_1ST: i32 = 1594;
const LEATHER_POUCH_2ND: i32 = 1595;
const LEATHER_POUCH_3RD: i32 = 1596;
const LEATHER_POUCH_1ST_FULL: i32 = 1597;
const LEATHER_POUCH_2ND_FULL: i32 = 1598;
const LEATHER_POUCH_3RD_FULL: i32 = 1599;
const KASHA_BEAR_CLAW: i32 = 1600;
const KASHA_BLADE_SPIDER_TALON: i32 = 1601;
const SCARLET_SALAMANDER_SCALE: i32 = 1602;
const FIERY_SPIRIT_SCROLL: i32 = 1603;
const ROSHEEKS_LETTER: i32 = 1604;
const GANTAKIS_LETTRT_OF_RECOMMENDATION: i32 = 1605;
const FIG: i32 = 1606;
const LEATHER_POUCH_4TF: i32 = 1607;
const LEATHER_POUCH_4TF_FULL: i32 = 1608;
const VUKU_ORK_TUSK: i32 = 1609;
const RATMAN_FANG: i32 = 1610;
const LANGK_LIZARDMAN_TOOTH: i32 = 1611;
const FELIM_LIZARDMAN_TOOTH: i32 = 1612;
const IRON_WILL_SCROLL: i32 = 1613;
const TORUKUS_LETTER: i32 = 1614;
const KASHA_SPIDERS_TOOTH: i32 = 8545;
const HORN_OF_BAAR_DRE_VANUL: i32 = 8546;

// Reward
const KHAVATARI_TOTEM: i32 = 1615;

// Monsters
const FELIM_LIZARDMAN_WARRIOR: i32 = 20014;
const VUKU_ORC_FIGHTER: i32 = 20017;
const LANGK_LIZZARDMAN_WARRIOR: i32 = 20024;
const RATMAN_WARRIOR: i32 = 20359;
const SCARLET_SALAMANDER: i32 = 20415;
const KASHA_FANG_SPIDER: i32 = 20476;
const KASHA_BLADE_SPIDER: i32 = 20478;
const KASHA_BEAR: i32 = 20479;
const BAAR_DRE_VANUL: i32 = 21118;

// Misc
const MIN_LEVEL: i32 = 18;

/// The monsters of the quest, all of them with an attack hook and a kill hook.
const MONSTERS: [i32; 9] = [
    FELIM_LIZARDMAN_WARRIOR,
    VUKU_ORC_FIGHTER,
    LANGK_LIZZARDMAN_WARRIOR,
    RATMAN_WARRIOR,
    SCARLET_SALAMANDER,
    KASHA_FANG_SPIDER,
    KASHA_BLADE_SPIDER,
    KASHA_BEAR,
    BAAR_DRE_VANUL,
];

/// Java `Q00415_PathOfTheOrcMonk`.
pub struct Q00415PathOfTheOrcMonk;

impl Script for Q00415PathOfTheOrcMonk {
    fn name(&self) -> &'static str {
        "Q00415_PathOfTheOrcMonk"
    }

    fn path(&self) -> &'static str {
        "quests/Q00415_PathOfTheOrcMonk"
    }

    fn id(&self) -> i32 {
        415
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[GANTAKI_ZU_URUTU]);
        s.add_talk_id(&[
            GANTAKI_ZU_URUTU,
            PREFECT_KASMAN,
            KHAVATARI_ROSHEEK,
            KHAVATARI_TORUKU,
            SEER_MOIRA,
            KHAVATARI_AREN,
        ]);
        s.add_attack_id(&MONSTERS);
        s.add_kill_id(&MONSTERS);
        s.register_quest_items(&[
            POMEGRANATE,
            LEATHER_POUCH_1ST,
            LEATHER_POUCH_2ND,
            LEATHER_POUCH_3RD,
            LEATHER_POUCH_1ST_FULL,
            LEATHER_POUCH_2ND_FULL,
            LEATHER_POUCH_3RD_FULL,
            KASHA_BEAR_CLAW,
            KASHA_BLADE_SPIDER_TALON,
            SCARLET_SALAMANDER_SCALE,
            FIERY_SPIRIT_SCROLL,
            ROSHEEKS_LETTER,
            GANTAKIS_LETTRT_OF_RECOMMENDATION,
            FIG,
            LEATHER_POUCH_4TF,
            LEATHER_POUCH_4TF_FULL,
            VUKU_ORK_TUSK,
            RATMAN_FANG,
            LANGK_LIZARDMAN_TOOTH,
            FELIM_LIZARDMAN_TOOTH,
            IRON_WILL_SCROLL,
            TORUKUS_LETTER,
            KASHA_SPIDERS_TOOTH,
            HORN_OF_BAAR_DRE_VANUL,
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
                htmltext = Some(
                    if ctx.player_class_id(player) == player_class::ORC_FIGHTER {
                        if ctx.player_level(player) >= MIN_LEVEL {
                            if ctx.has_quest_items(player, KHAVATARI_TOTEM) {
                                "30587-04.htm"
                            } else {
                                "30587-05.htm"
                            }
                        } else {
                            "30587-03.htm"
                        }
                    } else if ctx.player_class_id(player) == player_class::ORC_MONK {
                        "30587-02a.htm"
                    } else {
                        "30587-02.htm"
                    }
                    .to_owned(),
                );
            }
            "30587-06.htm" => {
                qs.start_quest(ctx);
                ctx.give_items(player, POMEGRANATE, 1);
                htmltext = Some(event.to_owned());
            }
            "30587-09b.html" => {
                if ctx.has_all_quest_items(player, &[FIERY_SPIRIT_SCROLL, ROSHEEKS_LETTER]) {
                    ctx.take_items(player, ROSHEEKS_LETTER, 1);
                    ctx.give_items(player, GANTAKIS_LETTRT_OF_RECOMMENDATION, 1);
                    qs.set_cond(ctx, 9, false);
                    htmltext = Some(event.to_owned());
                }
            }
            "30587-09c.html" => {
                if ctx.has_all_quest_items(player, &[FIERY_SPIRIT_SCROLL, ROSHEEKS_LETTER]) {
                    ctx.take_items(player, ROSHEEKS_LETTER, 1);
                    qs.set_memo_state(ctx, 2);
                    qs.set_cond(ctx, 14, false);
                    htmltext = Some(event.to_owned());
                }
            }
            "31979-02.html" => {
                if qs.is_memo_state(ctx, 5) {
                    htmltext = Some(event.to_owned());
                }
            }
            "31979-03.html" => {
                if qs.is_memo_state(ctx, 5) {
                    ctx.give_adena(player, 81_900, true);
                    ctx.give_items(player, KHAVATARI_TOTEM, 1);
                    let level = ctx.player_level(player);
                    if level >= 20 {
                        ctx.add_exp_and_sp(player, 160_267, 12_646);
                    } else if level == 19 {
                        ctx.add_exp_and_sp(player, 228_064, 15_995);
                    } else {
                        ctx.add_exp_and_sp(player, 295_862, 19_344);
                    }
                    qs.exit_quest(ctx, false, true);
                    ctx.world_mut().send_social_action(player, 3);
                    htmltext = Some(event.to_owned());
                }
            }
            "31979-04.html" => {
                if qs.is_memo_state(ctx, 5) {
                    qs.set_cond(ctx, 20, false);
                    htmltext = Some(event.to_owned());
                }
            }
            "32056-02.html" => {
                if qs.is_memo_state(ctx, 2) {
                    htmltext = Some(event.to_owned());
                }
            }
            "32056-03.html" => {
                if qs.is_memo_state(ctx, 2) {
                    qs.set_memo_state(ctx, 3);
                    qs.set_cond(ctx, 15, false);
                    htmltext = Some(event.to_owned());
                }
            }
            "32056-08.html" => {
                if qs.is_memo_state(ctx, 4) && ctx.get_quest_items_count(player, HORN_OF_BAAR_DRE_VANUL) >= 1
                {
                    ctx.take_items(player, HORN_OF_BAAR_DRE_VANUL, -1);
                    qs.set_memo_state(ctx, 5);
                    qs.set_cond(ctx, 19, false);
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
        let Some(qs) = ctx.get_quest_state(attacker, false) else {
            return;
        };
        if !qs.is_started(ctx) {
            return;
        }
        match ctx.get_script_value(npc) {
            0 => {
                if !ctx.has_no_weapon_or_fist(attacker) {
                    ctx.set_script_value(npc, 2);
                } else {
                    ctx.set_script_value(npc, 1);
                    ctx.world_mut()
                        .npc_variables_mut(npc.object_id)
                        .set_int("Q00415_last_attacker", attacker.0);
                }
            }
            1 => {
                let last = ctx
                    .world()
                    .npc_variables(npc.object_id)
                    .map_or(0, |v| v.get_int("Q00415_last_attacker", 0));
                if last != attacker.0 || !ctx.has_no_weapon_or_fist(attacker) {
                    ctx.set_script_value(npc, 2);
                }
            }
            _ => {}
        }
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        let Some(qs) = ctx.get_quest_state(killer, false) else {
            return;
        };
        if !(qs.is_started(ctx)
            && ctx.is_script_value(npc, 1)
            && ctx.check_if_in_range(ctx.alt_party_range(), npc.object_id, killer, true))
        {
            return;
        }
        let item_count = ctx.get_quest_items_count_of(
            killer,
            &[
                RATMAN_FANG,
                LANGK_LIZARDMAN_TOOTH,
                FELIM_LIZARDMAN_TOOTH,
                VUKU_ORK_TUSK,
            ],
        );
        match npc.id {
            FELIM_LIZARDMAN_WARRIOR => {
                if ctx.has_quest_items(killer, LEATHER_POUCH_4TF)
                    && ctx.get_quest_items_count(killer, FELIM_LIZARDMAN_TOOTH) < 3
                {
                    if item_count >= 11 {
                        make_full_pouch_4(ctx, killer, qs);
                    } else {
                        ctx.give_items(killer, FELIM_LIZARDMAN_TOOTH, 1);
                        ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
                    }
                }
            }
            VUKU_ORC_FIGHTER => {
                if ctx.has_quest_items(killer, LEATHER_POUCH_4TF)
                    && ctx.get_quest_items_count(killer, VUKU_ORK_TUSK) < 3
                {
                    if item_count >= 11 {
                        make_full_pouch_4(ctx, killer, qs);
                    } else {
                        ctx.give_items(killer, VUKU_ORK_TUSK, 1);
                        ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
                    }
                }
            }
            LANGK_LIZZARDMAN_WARRIOR => {
                if ctx.has_quest_items(killer, LEATHER_POUCH_4TF)
                    && ctx.get_quest_items_count(killer, LANGK_LIZARDMAN_TOOTH) < 3
                {
                    if item_count >= 11 {
                        make_full_pouch_4(ctx, killer, qs);
                    } else {
                        ctx.give_items(killer, LANGK_LIZARDMAN_TOOTH, 1);
                        ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
                    }
                }
            }
            RATMAN_WARRIOR => {
                if ctx.has_quest_items(killer, LEATHER_POUCH_4TF)
                    && ctx.get_quest_items_count(killer, RATMAN_FANG) < 3
                {
                    if item_count >= 11 {
                        make_full_pouch_4(ctx, killer, qs);
                    } else {
                        ctx.give_items(killer, RATMAN_FANG, 1);
                        ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
                    }
                }
            }
            SCARLET_SALAMANDER => {
                if ctx.has_quest_items(killer, LEATHER_POUCH_3RD) {
                    if ctx.get_quest_items_count(killer, SCARLET_SALAMANDER_SCALE) == 4 {
                        ctx.take_items(killer, LEATHER_POUCH_3RD, 1);
                        ctx.give_items(killer, LEATHER_POUCH_3RD_FULL, 1);
                        ctx.take_items(killer, SCARLET_SALAMANDER_SCALE, -1);
                        qs.set_cond(ctx, 7, true);
                    } else {
                        ctx.give_items(killer, SCARLET_SALAMANDER_SCALE, 1);
                        ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
                    }
                }
            }
            KASHA_FANG_SPIDER => {
                if qs.is_memo_state(ctx, 3)
                    && ctx.get_quest_items_count(killer, KASHA_SPIDERS_TOOTH) < 6
                    && ctx.get_random(100) < 70
                {
                    ctx.give_items(killer, KASHA_SPIDERS_TOOTH, 1);
                    if ctx.get_quest_items_count(killer, KASHA_SPIDERS_TOOTH) >= 6 {
                        qs.set_cond(ctx, 16, true);
                    } else {
                        ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
                    }
                }
            }
            KASHA_BLADE_SPIDER => {
                if ctx.has_quest_items(killer, LEATHER_POUCH_2ND) {
                    if ctx.get_quest_items_count(killer, KASHA_BLADE_SPIDER_TALON) == 4 {
                        ctx.take_items(killer, LEATHER_POUCH_2ND, 1);
                        ctx.give_items(killer, LEATHER_POUCH_2ND_FULL, 1);
                        ctx.take_items(killer, KASHA_BLADE_SPIDER_TALON, -1);
                        qs.set_cond(ctx, 5, true);
                    } else {
                        ctx.give_items(killer, KASHA_BLADE_SPIDER_TALON, 1);
                        ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
                    }
                } else if qs.is_memo_state(ctx, 3)
                    && ctx.get_quest_items_count(killer, KASHA_SPIDERS_TOOTH) < 6
                    && ctx.get_random(100) < 70
                {
                    ctx.give_items(killer, KASHA_SPIDERS_TOOTH, 1);
                    if ctx.get_quest_items_count(killer, KASHA_SPIDERS_TOOTH) == 6 {
                        qs.set_cond(ctx, 16, true);
                    } else {
                        ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
                    }
                }
            }
            KASHA_BEAR => {
                if ctx.has_quest_items(killer, LEATHER_POUCH_1ST) {
                    if ctx.get_quest_items_count(killer, KASHA_BEAR_CLAW) == 4 {
                        ctx.take_items(killer, LEATHER_POUCH_1ST, 1);
                        ctx.give_items(killer, LEATHER_POUCH_1ST_FULL, 1);
                        ctx.take_items(killer, KASHA_BEAR_CLAW, -1);
                        qs.set_cond(ctx, 3, true);
                    } else {
                        ctx.give_items(killer, KASHA_BEAR_CLAW, 1);
                        ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
                    }
                }
            }
            BAAR_DRE_VANUL => {
                if qs.is_memo_state(ctx, 4)
                    && !ctx.has_quest_items(killer, HORN_OF_BAAR_DRE_VANUL)
                    && ctx.get_random(100) < 90
                {
                    ctx.give_items(killer, HORN_OF_BAAR_DRE_VANUL, 1);
                    qs.set_cond(ctx, 18, true);
                }
            }
            _ => {}
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let memo_state = qs.get_memo_state(ctx);
        let mut htmltext = ctx.get_no_quest_msg(player);
        let state = qs.state(ctx);
        if state == CREATED || state == COMPLETED {
            if npc.id == GANTAKI_ZU_URUTU {
                htmltext = "30587-01.htm".to_owned();
            }
        } else if state == STARTED {
            match npc.id {
                GANTAKI_ZU_URUTU => {
                    let letter_count = ctx.get_quest_items_count_of(
                        player,
                        &[
                            LEATHER_POUCH_1ST,
                            LEATHER_POUCH_2ND,
                            LEATHER_POUCH_3RD,
                            LEATHER_POUCH_1ST_FULL,
                            LEATHER_POUCH_2ND_FULL,
                            LEATHER_POUCH_3RD_FULL,
                        ],
                    );
                    if memo_state == 2 {
                        htmltext = "30587-09c.html".to_owned();
                    } else if ctx.has_quest_items(player, POMEGRANATE)
                        && !ctx.has_at_least_one_quest_item(
                            player,
                            &[
                                FIERY_SPIRIT_SCROLL,
                                GANTAKIS_LETTRT_OF_RECOMMENDATION,
                                ROSHEEKS_LETTER,
                            ],
                        )
                        && letter_count == 0
                    {
                        htmltext = "30587-07.html".to_owned();
                    } else if !ctx.has_at_least_one_quest_item(
                        player,
                        &[
                            FIERY_SPIRIT_SCROLL,
                            POMEGRANATE,
                            GANTAKIS_LETTRT_OF_RECOMMENDATION,
                            ROSHEEKS_LETTER,
                        ],
                    ) && letter_count == 1
                    {
                        htmltext = "30587-08.html".to_owned();
                    } else if ctx.has_all_quest_items(player, &[FIERY_SPIRIT_SCROLL, ROSHEEKS_LETTER])
                        && !ctx.has_at_least_one_quest_item(
                            player,
                            &[POMEGRANATE, GANTAKIS_LETTRT_OF_RECOMMENDATION],
                        )
                        && letter_count == 0
                    {
                        htmltext = "30587-09a.html".to_owned();
                    } else if memo_state < 2 {
                        if ctx.has_all_quest_items(
                            player,
                            &[FIERY_SPIRIT_SCROLL, GANTAKIS_LETTRT_OF_RECOMMENDATION],
                        ) && !ctx.has_at_least_one_quest_item(player, &[POMEGRANATE, ROSHEEKS_LETTER])
                            && letter_count == 0
                        {
                            htmltext = "30587-10.html".to_owned();
                        } else if ctx.has_quest_items(player, FIERY_SPIRIT_SCROLL)
                            && !ctx.has_at_least_one_quest_item(
                                player,
                                &[POMEGRANATE, GANTAKIS_LETTRT_OF_RECOMMENDATION, ROSHEEKS_LETTER],
                            )
                            && letter_count == 0
                        {
                            htmltext = "30587-11.html".to_owned();
                        }
                    }
                }
                PREFECT_KASMAN => {
                    if ctx.has_quest_items(player, GANTAKIS_LETTRT_OF_RECOMMENDATION) {
                        ctx.take_items(player, GANTAKIS_LETTRT_OF_RECOMMENDATION, 1);
                        ctx.give_items(player, FIG, 1);
                        qs.set_cond(ctx, 10, false);
                        htmltext = "30501-01.html".to_owned();
                    } else if ctx.has_quest_items(player, FIG)
                        && !ctx
                            .has_at_least_one_quest_item(player, &[LEATHER_POUCH_4TF, LEATHER_POUCH_4TF_FULL])
                    {
                        htmltext = "30501-02.html".to_owned();
                    } else if !ctx.has_quest_items(player, FIG)
                        && ctx
                            .has_at_least_one_quest_item(player, &[LEATHER_POUCH_4TF, LEATHER_POUCH_4TF_FULL])
                    {
                        htmltext = "30501-03.html".to_owned();
                    } else if ctx.has_quest_items(player, IRON_WILL_SCROLL) {
                        ctx.give_adena(player, 163_800, true);
                        ctx.give_items(player, KHAVATARI_TOTEM, 1);
                        let level = ctx.player_level(player);
                        if level >= 20 {
                            ctx.add_exp_and_sp(player, 320_534, 25_292);
                        } else if level == 19 {
                            ctx.add_exp_and_sp(player, 456_128, 31_990);
                        } else {
                            ctx.add_exp_and_sp(player, 591_724, 38_688);
                        }
                        qs.exit_quest(ctx, false, true);
                        ctx.world_mut().send_social_action(player, 3);
                        htmltext = "30501-04.html".to_owned();
                    }
                }
                KHAVATARI_ROSHEEK => {
                    // Each pair of checks: the pouch the player holds, then its full form.
                    let has = |ctx: &ScriptCtx<'_>, item: i32| ctx.has_quest_items(player, item);
                    if has(ctx, POMEGRANATE) {
                        ctx.take_items(player, POMEGRANATE, 1);
                        ctx.give_items(player, LEATHER_POUCH_1ST, 1);
                        qs.set_cond(ctx, 2, false);
                        htmltext = "30590-01.html".to_owned();
                    } else if has(ctx, LEATHER_POUCH_1ST) && !has(ctx, LEATHER_POUCH_1ST_FULL) {
                        htmltext = "30590-02.html".to_owned();
                    } else if !has(ctx, LEATHER_POUCH_1ST) && has(ctx, LEATHER_POUCH_1ST_FULL) {
                        ctx.give_items(player, LEATHER_POUCH_2ND, 1);
                        ctx.take_items(player, LEATHER_POUCH_1ST_FULL, 1);
                        qs.set_cond(ctx, 4, false);
                        htmltext = "30590-03.html".to_owned();
                    } else if has(ctx, LEATHER_POUCH_2ND) && !has(ctx, LEATHER_POUCH_2ND_FULL) {
                        htmltext = "30590-04.html".to_owned();
                    } else if !has(ctx, LEATHER_POUCH_2ND) && has(ctx, LEATHER_POUCH_2ND_FULL) {
                        ctx.give_items(player, LEATHER_POUCH_3RD, 1);
                        ctx.take_items(player, LEATHER_POUCH_2ND_FULL, 1);
                        qs.set_cond(ctx, 6, false);
                        htmltext = "30590-05.html".to_owned();
                    } else if has(ctx, LEATHER_POUCH_3RD) && !has(ctx, LEATHER_POUCH_3RD_FULL) {
                        htmltext = "30590-06.html".to_owned();
                    } else if !has(ctx, LEATHER_POUCH_3RD) && has(ctx, LEATHER_POUCH_3RD_FULL) {
                        ctx.take_items(player, LEATHER_POUCH_3RD_FULL, 1);
                        ctx.give_items(player, FIERY_SPIRIT_SCROLL, 1);
                        ctx.give_items(player, ROSHEEKS_LETTER, 1);
                        qs.set_cond(ctx, 8, false);
                        htmltext = "30590-07.html".to_owned();
                    } else if ctx.has_all_quest_items(player, &[ROSHEEKS_LETTER, FIERY_SPIRIT_SCROLL]) {
                        htmltext = "30590-08.html".to_owned();
                    } else if !has(ctx, ROSHEEKS_LETTER) && has(ctx, FIERY_SPIRIT_SCROLL) {
                        htmltext = "30590-09.html".to_owned();
                    }
                }
                KHAVATARI_TORUKU => {
                    if ctx.has_quest_items(player, FIG) {
                        ctx.take_items(player, FIG, 1);
                        ctx.give_items(player, LEATHER_POUCH_4TF, 1);
                        qs.set_cond(ctx, 11, false);
                        htmltext = "30591-01.html".to_owned();
                    } else if ctx.has_quest_items(player, LEATHER_POUCH_4TF)
                        && !ctx.has_quest_items(player, LEATHER_POUCH_4TF_FULL)
                    {
                        htmltext = "30591-02.html".to_owned();
                    } else if !ctx.has_quest_items(player, LEATHER_POUCH_4TF)
                        && ctx.has_quest_items(player, LEATHER_POUCH_4TF_FULL)
                    {
                        ctx.take_items(player, LEATHER_POUCH_4TF_FULL, 1);
                        ctx.give_items(player, IRON_WILL_SCROLL, 1);
                        ctx.give_items(player, TORUKUS_LETTER, 1);
                        qs.set_cond(ctx, 13, false);
                        htmltext = "30591-03.html".to_owned();
                    } else if ctx.has_all_quest_items(player, &[IRON_WILL_SCROLL, TORUKUS_LETTER]) {
                        htmltext = "30591-04.html".to_owned();
                    }
                }
                SEER_MOIRA => {
                    if memo_state == 5 {
                        htmltext = "31979-01.html".to_owned();
                    }
                }
                KHAVATARI_AREN => {
                    if memo_state == 2 {
                        htmltext = "32056-01.html".to_owned();
                    } else if memo_state == 3 {
                        if ctx.get_quest_items_count(player, KASHA_SPIDERS_TOOTH) < 6 {
                            htmltext = "32056-04.html".to_owned();
                        } else {
                            ctx.take_items(player, KASHA_SPIDERS_TOOTH, -1);
                            qs.set_memo_state(ctx, 4);
                            qs.set_cond(ctx, 17, false);
                            htmltext = "32056-05.html".to_owned();
                        }
                    } else if memo_state == 4 {
                        if !ctx.has_quest_items(player, HORN_OF_BAAR_DRE_VANUL) {
                            htmltext = "32056-06.html".to_owned();
                        } else {
                            htmltext = "32056-07.html".to_owned();
                        }
                    } else if memo_state == 5 {
                        htmltext = "32056-09.html".to_owned();
                    }
                }
                _ => {}
            }
        }
        Some(htmltext)
    }
}

/// Java's fourth-kill branch of the Vuku, Felim, Langk and Ratman monsters: the pouch becomes
/// full and the four kinds of tooth and fang are used up; cond 12.
fn make_full_pouch_4(ctx: &mut ScriptCtx<'_>, killer: ObjectId, qs: Qs) {
    ctx.take_items(killer, LEATHER_POUCH_4TF, 1);
    ctx.give_items(killer, LEATHER_POUCH_4TF_FULL, 1);
    ctx.take_items(killer, VUKU_ORK_TUSK, -1);
    ctx.take_items(killer, RATMAN_FANG, -1);
    ctx.take_items(killer, LANGK_LIZARDMAN_TOOTH, -1);
    ctx.take_items(killer, FELIM_LIZARDMAN_TOOTH, -1);
    qs.set_cond(ctx, 12, true);
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::ScriptRegistry;
    use l2_world::script::race;

    use super::*;
    use crate::testing::{Pages, ScriptTest, item, rows};

    const NAME: &str = "Q00415_PathOfTheOrcMonk";

    const NPCS: &str = r#"
        <npc id="30587" type="Folk" level="70" name="Gantaki Zu Urutu"/>
        <npc id="30501" type="Folk" level="70" name="Prefect Kasman"/>
        <npc id="30590" type="Folk" level="70" name="Khavatari Rosheek"/>
        <npc id="30591" type="Folk" level="70" name="Khavatari Toruku"/>
        <npc id="20479" type="Monster" level="20" name="Kasha Bear"/>
        <npc id="20478" type="Monster" level="20" name="Kasha Blade Spider"/>
        <npc id="20415" type="Monster" level="20" name="Scarlet Salamander"/>
        <npc id="20359" type="Monster" level="20" name="Ratman Warrior"/>
        <npc id="20014" type="Monster" level="20" name="Felim Lizardman Warrior"/>
        <npc id="20017" type="Monster" level="20" name="Vuku Orc Fighter"/>
        <npc id="20024" type="Monster" level="20" name="Langk Lizardman Warrior"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="1593" type="EtcItem" name="Pomegranate"><set name="is_questitem" val="true"/></item>
        <item id="1594" type="EtcItem" name="Leather Pouch 1st"><set name="is_questitem" val="true"/></item>
        <item id="1595" type="EtcItem" name="Leather Pouch 2nd"><set name="is_questitem" val="true"/></item>
        <item id="1596" type="EtcItem" name="Leather Pouch 3rd"><set name="is_questitem" val="true"/></item>
        <item id="1597" type="EtcItem" name="Leather Pouch 1st Full"><set name="is_questitem" val="true"/></item>
        <item id="1598" type="EtcItem" name="Leather Pouch 2nd Full"><set name="is_questitem" val="true"/></item>
        <item id="1599" type="EtcItem" name="Leather Pouch 3rd Full"><set name="is_questitem" val="true"/></item>
        <item id="1600" type="EtcItem" name="Kasha Bear Claw"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="1601" type="EtcItem" name="Kasha Blade Spider Talon"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="1602" type="EtcItem" name="Scarlet Salamander Scale"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="1603" type="EtcItem" name="Fiery Spirit Scroll"><set name="is_questitem" val="true"/></item>
        <item id="1604" type="EtcItem" name="Rosheek's Letter"><set name="is_questitem" val="true"/></item>
        <item id="1605" type="EtcItem" name="Gantaki's Letter of Recommendation"><set name="is_questitem" val="true"/></item>
        <item id="1606" type="EtcItem" name="Fig"><set name="is_questitem" val="true"/></item>
        <item id="1607" type="EtcItem" name="Leather Pouch 4th"><set name="is_questitem" val="true"/></item>
        <item id="1608" type="EtcItem" name="Leather Pouch 4th Full"><set name="is_questitem" val="true"/></item>
        <item id="1609" type="EtcItem" name="Vuku Orc Tusk"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="1610" type="EtcItem" name="Ratman Fang"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="1611" type="EtcItem" name="Langk Lizardman Tooth"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="1612" type="EtcItem" name="Felim Lizardman Tooth"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="1613" type="EtcItem" name="Iron Will Scroll"><set name="is_questitem" val="true"/></item>
        <item id="1614" type="EtcItem" name="Toruku's Letter"><set name="is_questitem" val="true"/></item>
        <item id="1615" type="EtcItem" name="Khavatari Totem"/>"#;

    struct Npcs {
        gantaki: Oid,
        kasman: Oid,
        rosheek: Oid,
        toruku: Oid,
    }

    fn pages() -> Pages {
        Pages::of(&Q00415PathOfTheOrcMonk)
            .page("30587-01.htm", &["ACCEPT"])
            .page("30587-05.htm", &["30587-06.htm"])
            .page("30587-09a.html", &["30587-09b.html", "30587-09c.html"])
            .plain(&[
                "30587-02.htm",
                "30587-02a.htm",
                "30587-03.htm",
                "30587-04.htm",
                "30587-06.htm",
                "30587-07.html",
                "30587-08.html",
                "30587-09b.html",
                "30587-09c.html",
                "30587-10.html",
                "30587-11.html",
                "30501-01.html",
                "30501-02.html",
                "30501-03.html",
                "30501-04.html",
                "30590-01.html",
                "30590-02.html",
                "30590-03.html",
                "30590-04.html",
                "30590-05.html",
                "30590-06.html",
                "30590-07.html",
                "30590-08.html",
                "30590-09.html",
                "30591-01.html",
                "30591-02.html",
                "30591-03.html",
                "30591-04.html",
            ])
            .npcs(&[
                GANTAKI_ZU_URUTU,
                PREFECT_KASMAN,
                KHAVATARI_ROSHEEK,
                KHAVATARI_TORUKU,
            ])
    }

    fn setup() -> (ScriptTest, Npcs) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00415PathOfTheOrcMonk);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages());
        let npcs = Npcs {
            gantaki: t.spawn(GANTAKI_ZU_URUTU, 20),
            kasman: t.spawn(PREFECT_KASMAN, 40),
            rosheek: t.spawn(KHAVATARI_ROSHEEK, 60),
            toruku: t.spawn(KHAVATARI_TORUKU, 80),
        };
        (t, npcs)
    }

    /// Hits a monster as the player (`onAttack` with fists, the player's own hit) and then kills
    /// it with the hook called directly (no ticks, see the other quests' `kill_now`).
    fn kill_now(t: &mut ScriptTest, npc_id: i32) {
        let mob = t.spawn(npc_id, 80);
        t.world.run_as_script(NAME, |ctx| {
            if let Some(npc) = ctx.script_npc(mob) {
                Q00415PathOfTheOrcMonk.on_attack(ctx, npc, Oid(1), 1, false, None);
                Q00415PathOfTheOrcMonk.on_kill(ctx, npc, Oid(1), false);
            }
        });
        t.world.delete_npc(mob);
    }

    #[test]
    fn the_accept_page_depends_on_class_and_level() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 17, race::HUMAN, player_class::ORC_FIGHTER, vec![]);
        assert!(t.quest_page(1, npcs.gantaki).contains("30587-01.htm"));
        assert!(t.event(1, NAME, "ACCEPT").contains("30587-03.htm"));
        assert!(t.take_writes().is_empty());

        let (mut t, npcs) = setup();
        t.enter_as(1, 30, race::HUMAN, player_class::ORC_MONK, vec![]);
        t.quest_page(1, npcs.gantaki);
        assert!(t.event(1, NAME, "ACCEPT").contains("30587-02a.htm"));

        let (mut t, npcs) = setup();
        t.enter_as(1, 30, race::HUMAN, player_class::ORC_MAGE, vec![]);
        t.quest_page(1, npcs.gantaki);
        assert!(t.event(1, NAME, "ACCEPT").contains("30587-02.htm"));

        let (mut t, npcs) = setup();
        t.enter_as(
            1,
            30,
            race::HUMAN,
            player_class::ORC_FIGHTER,
            vec![item(5000, 1615, 1)],
        );
        t.quest_page(1, npcs.gantaki);
        assert!(t.event(1, NAME, "ACCEPT").contains("30587-04.htm"));
    }

    #[test]
    fn the_pouches_from_the_pomegranate_to_the_iron_will() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 18, race::HUMAN, player_class::ORC_FIGHTER, vec![]);
        assert!(t.quest_page(1, npcs.gantaki).contains("30587-01.htm"));
        assert!(t.event(1, NAME, "ACCEPT").contains("30587-05.htm"));
        assert!(t.event(1, NAME, "30587-06.htm").contains("30587-06.htm"));
        assert_eq!(t.count(1, POMEGRANATE), 1);
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );

        // Rosheek gives the first pouch: cond 2.
        assert!(t.quest_page(1, npcs.rosheek).contains("30590-01.html"));
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);

        // Five kasha bears: four claws, then the fifth makes the full pouch: cond 3.
        for _ in 0..5 {
            kill_now(&mut t, KASHA_BEAR);
        }
        assert_eq!(t.count(1, LEATHER_POUCH_1ST_FULL), 1);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "3")]);

        // Rosheek gives the second pouch: cond 4; five blade spiders make the next full pouch: cond 5.
        assert!(t.quest_page(1, npcs.rosheek).contains("30590-03.html"));
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "4")]);
        for _ in 0..5 {
            kill_now(&mut t, KASHA_BLADE_SPIDER);
        }
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "5")]);

        // Rosheek's third pouch: cond 6; five scarlet salamanders: cond 7.
        assert!(t.quest_page(1, npcs.rosheek).contains("30590-05.html"));
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "6")]);
        for _ in 0..5 {
            kill_now(&mut t, SCARLET_SALAMANDER);
        }
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "7")]);

        // Rosheek takes the full pouch and gives the fiery spirit scroll and his letter: cond 8.
        assert!(t.quest_page(1, npcs.rosheek).contains("30590-07.html"));
        assert_eq!(
            (t.count(1, FIERY_SPIRIT_SCROLL), t.count(1, ROSHEEKS_LETTER)),
            (1, 1)
        );
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "8")]);

        // Gantaki's recommendation for the scroll and the letter: cond 9; Kasman gives the fig: cond 10.
        assert!(t.quest_page(1, npcs.gantaki).contains("30587-09a.html"));
        assert!(t.event(1, NAME, "30587-09b.html").contains("30587-09b.html"));
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "9")]);
        assert!(t.quest_page(1, npcs.kasman).contains("30501-01.html"));
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "10")]);

        // Toruku gives the fourth pouch: cond 11.
        assert!(t.quest_page(1, npcs.toruku).contains("30591-01.html"));
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "11")]);

        // Teeth and fangs: eleven of them in all (three of each kind at most), then one more
        // kill makes the full pouch: cond 12.
        for _ in 0..3 {
            kill_now(&mut t, FELIM_LIZARDMAN_WARRIOR);
            kill_now(&mut t, VUKU_ORC_FIGHTER);
            kill_now(&mut t, LANGK_LIZZARDMAN_WARRIOR);
        }
        for _ in 0..2 {
            kill_now(&mut t, RATMAN_WARRIOR);
        }
        assert!(t.take_writes().is_empty());
        kill_now(&mut t, RATMAN_WARRIOR);
        assert_eq!(t.count(1, LEATHER_POUCH_4TF_FULL), 1);
        assert_eq!(t.count(1, FELIM_LIZARDMAN_TOOTH), 0);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "12")]);

        // Toruku takes the full pouch and gives the iron will scroll and the letter: cond 13.
        assert!(t.quest_page(1, npcs.toruku).contains("30591-03.html"));
        assert_eq!((t.count(1, IRON_WILL_SCROLL), t.count(1, TORUKUS_LETTER)), (1, 1));
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "13")]);

        // Kasman's reward: adena, the khavatari totem, the quest ends, the bow.
        assert!(t.quest_page(1, npcs.kasman).contains("30501-04.html"));
        assert_eq!(t.count(1, 57), 163_800);
        assert_eq!(t.count(1, KHAVATARI_TOTEM), 1);
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed")
            ]
        );
        assert!(t.world.quest_list(Oid(1)).quests.is_empty());
    }
}
