//! Quest 218: Testimony of Life (Master Cardien, Asterios, Pushkin, Thalia, Arkenia, Adonius and Isael; the elf's mark).
// Java: dist/game/data/scripts/quests/Q00218_TestimonyOfLife/Q00218_TestimonyOfLife.java

use l2_util::ObjectId;
use l2_world::script::{QuestSound, race};
use l2_world::subclass::CategoryType;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const HIERARCH_ASTERIOS: i32 = 30154;
const BLACKSMITH_PUSHKIN: i32 = 30300;
const THALIA: i32 = 30371;
const PRIEST_ADONIUS: i32 = 30375;
const ARKENIA: i32 = 30419;
const MASTER_CARDIEN: i32 = 30460;
const ISAEL_SILVERSHADOW: i32 = 30655;

// Items
const TALINS_SPEAR: i32 = 3026;
const CARDIENS_LETTER: i32 = 3141;
const CAMOMILE_CHARM: i32 = 3142;
const HIERARCHS_LETTER: i32 = 3143;
const MOONFLOWER_CHARM: i32 = 3144;
const GRAIL_DIAGRAM: i32 = 3145;
const THALIAS_1ST_LETTER: i32 = 3146;
const THALIAS_2ND_LETTER: i32 = 3147;
const THALIAS_INSTRUCTIONS: i32 = 3148;
const PUSHKINS_LIST: i32 = 3149;
const PURE_MITHRIL_CUP: i32 = 3150;
const ARKENIAS_CONTRACT: i32 = 3151;
const ARKENIAS_INSTRUCTIONS: i32 = 3152;
const ADONIUS_LIST: i32 = 3153;
const ANDARIEL_SCRIPTURE_COPY: i32 = 3154;
const STARDUST: i32 = 3155;
const ISAELS_INSTRUCTIONS: i32 = 3156;
const ISAELS_LETTER: i32 = 3157;
const GRAIL_OF_PURITY: i32 = 3158;
const TEARS_OF_UNICORN: i32 = 3159;
const WATER_OF_LIFE: i32 = 3160;
const PURE_MITHRIL_ORE: i32 = 3161;
const ANT_SOLDIER_ACID: i32 = 3162;
const WYRMS_TALON: i32 = 3163;
const SPIDER_ICHOR: i32 = 3164;
const HARPYS_DOWN: i32 = 3165;
const TALINS_SPEAR_BLADE: i32 = 3166;
const TALINS_SPEAR_SHAFT: i32 = 3167;
const TALINS_RUBY: i32 = 3168;
const TALINS_AQUAMARINE: i32 = 3169;
const TALINS_AMETHYST: i32 = 3170;
const TALINS_PERIDOT: i32 = 3171;

// Reward
const MARK_OF_LIFE: i32 = 3140;
const DIMENSIONAL_DIAMOND: i32 = 7562;

// Monsters
const ANT_RECRUIT: i32 = 20082;
const ANT_PATROL: i32 = 20084;
const ANT_GUARD: i32 = 20086;
const ANT_SOLDIER: i32 = 20087;
const ANT_WARRIOR_CAPTAIN: i32 = 20088;
const HARPY: i32 = 20145;
const WYRM: i32 = 20176;
const MARSH_SPIDER: i32 = 20233;
const GUARDIAN_BASILISK: i32 = 20550;
const LETO_LIZARDMAN_SHAMAN: i32 = 20581;
const LETO_LIZARDMAN_OVERLORD: i32 = 20582;

// Quest monster
const UNICORN_OF_EVA: i32 = 27077;

// Misc
const MIN_LEVEL: i32 = 37;
const LEVEL: i32 = 38;
/// Java `player.getVariables()` key: the second-class diamonds are given once per character.
const DIAMOND_REWARD_VAR: &str = "2ND_CLASS_DIAMOND_REWARD";

/// Java `Q00218_TestimonyOfLife`.
pub struct Q00218TestimonyOfLife;

fn has(ctx: &ScriptCtx<'_>, player: ObjectId, item: i32) -> bool {
    ctx.has_quest_items(player, item)
}

fn all(ctx: &ScriptCtx<'_>, player: ObjectId, items: &[i32]) -> bool {
    ctx.has_all_quest_items(player, items)
}

fn any(ctx: &ScriptCtx<'_>, player: ObjectId, items: &[i32]) -> bool {
    ctx.has_at_least_one_quest_item(player, items)
}

fn count(ctx: &ScriptCtx<'_>, player: ObjectId, item: i32) -> i64 {
    ctx.get_quest_items_count(player, item)
}

impl Script for Q00218TestimonyOfLife {
    fn name(&self) -> &'static str {
        "Q00218_TestimonyOfLife"
    }

    fn path(&self) -> &'static str {
        "quests/Q00218_TestimonyOfLife"
    }

    fn id(&self) -> i32 {
        218
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[MASTER_CARDIEN]);
        s.add_talk_id(&[
            MASTER_CARDIEN,
            HIERARCH_ASTERIOS,
            BLACKSMITH_PUSHKIN,
            THALIA,
            PRIEST_ADONIUS,
            ARKENIA,
            ISAEL_SILVERSHADOW,
        ]);
        s.add_kill_id(&[
            ANT_RECRUIT,
            ANT_PATROL,
            ANT_GUARD,
            ANT_SOLDIER,
            ANT_WARRIOR_CAPTAIN,
            HARPY,
            WYRM,
            MARSH_SPIDER,
            GUARDIAN_BASILISK,
            LETO_LIZARDMAN_SHAMAN,
            LETO_LIZARDMAN_OVERLORD,
            UNICORN_OF_EVA,
        ]);
        s.register_quest_items(&[
            TALINS_SPEAR,
            CARDIENS_LETTER,
            CAMOMILE_CHARM,
            HIERARCHS_LETTER,
            MOONFLOWER_CHARM,
            GRAIL_DIAGRAM,
            THALIAS_1ST_LETTER,
            THALIAS_2ND_LETTER,
            THALIAS_INSTRUCTIONS,
            PUSHKINS_LIST,
            PURE_MITHRIL_CUP,
            ARKENIAS_CONTRACT,
            ARKENIAS_INSTRUCTIONS,
            ADONIUS_LIST,
            ANDARIEL_SCRIPTURE_COPY,
            STARDUST,
            ISAELS_INSTRUCTIONS,
            ISAELS_LETTER,
            GRAIL_OF_PURITY,
            TEARS_OF_UNICORN,
            WATER_OF_LIFE,
            PURE_MITHRIL_ORE,
            ANT_SOLDIER_ACID,
            WYRMS_TALON,
            SPIDER_ICHOR,
            HARPYS_DOWN,
            TALINS_SPEAR_BLADE,
            TALINS_SPEAR_SHAFT,
            TALINS_RUBY,
            TALINS_AQUAMARINE,
            TALINS_AMETHYST,
            TALINS_PERIDOT,
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
                    if !has(ctx, player, CARDIENS_LETTER) {
                        ctx.give_items(player, CARDIENS_LETTER, 1);
                    }
                    ctx.play_sound(player, QuestSound::ItemsoundQuestMiddle);
                    if ctx.player_variable_int(player, DIAMOND_REWARD_VAR, 0) == 0 {
                        ctx.give_items(player, DIMENSIONAL_DIAMOND, 102);
                        ctx.set_player_variable(player, DIAMOND_REWARD_VAR, "1");
                        htmltext = Some("30460-04a.htm".to_owned());
                    } else {
                        htmltext = Some("30460-04.htm".to_owned());
                    }
                }
            }
            "30154-02.html" | "30154-03.html" | "30154-04.html" | "30154-05.html" | "30154-06.html"
            | "30300-02.html" | "30300-03.html" | "30300-04.html" | "30300-05.html" | "30300-09.html"
            | "30300-07a.html" | "30371-02.html" | "30371-10.html" | "30419-02.html" | "30419-03.html" => {
                htmltext = Some(event.to_owned());
            }
            "30154-07.html" => {
                if has(ctx, player, CARDIENS_LETTER) {
                    ctx.take_items(player, CARDIENS_LETTER, 1);
                    ctx.give_items(player, HIERARCHS_LETTER, 1);
                    ctx.give_items(player, MOONFLOWER_CHARM, 1);
                    qs.set_cond(ctx, 2, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "30300-06.html" => {
                if has(ctx, player, GRAIL_DIAGRAM) {
                    ctx.take_items(player, GRAIL_DIAGRAM, 1);
                    ctx.give_items(player, PUSHKINS_LIST, 1);
                    qs.set_cond(ctx, 4, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "30300-10.html" => {
                if has(ctx, player, PUSHKINS_LIST) {
                    ctx.take_items(player, PUSHKINS_LIST, 1);
                    ctx.give_items(player, PURE_MITHRIL_CUP, 1);
                    ctx.take_items(player, PURE_MITHRIL_ORE, -1);
                    ctx.take_items(player, ANT_SOLDIER_ACID, -1);
                    ctx.take_items(player, WYRMS_TALON, -1);
                    qs.set_cond(ctx, 6, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "30371-03.html" => {
                if has(ctx, player, HIERARCHS_LETTER) {
                    ctx.take_items(player, HIERARCHS_LETTER, 1);
                    ctx.give_items(player, GRAIL_DIAGRAM, 1);
                    qs.set_cond(ctx, 3, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "30371-11.html" => {
                if has(ctx, player, STARDUST) {
                    ctx.give_items(player, THALIAS_2ND_LETTER, 1);
                    ctx.take_items(player, STARDUST, 1);
                    qs.set_cond(ctx, 14, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "30419-04.html" => {
                if has(ctx, player, THALIAS_1ST_LETTER) {
                    ctx.take_items(player, THALIAS_1ST_LETTER, 1);
                    ctx.give_items(player, ARKENIAS_CONTRACT, 1);
                    ctx.give_items(player, ARKENIAS_INSTRUCTIONS, 1);
                    qs.set_cond(ctx, 8, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "30375-02.html" => {
                if has(ctx, player, ARKENIAS_INSTRUCTIONS) {
                    ctx.take_items(player, ARKENIAS_INSTRUCTIONS, 1);
                    ctx.give_items(player, ADONIUS_LIST, 1);
                    qs.set_cond(ctx, 9, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "30655-02.html" => {
                if has(ctx, player, THALIAS_2ND_LETTER) {
                    ctx.take_items(player, THALIAS_2ND_LETTER, 1);
                    ctx.give_items(player, ISAELS_INSTRUCTIONS, 1);
                    qs.set_cond(ctx, 15, true);
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
            ANT_RECRUIT | ANT_PATROL | ANT_GUARD | ANT_SOLDIER | ANT_WARRIOR_CAPTAIN => {
                if all(ctx, killer, &[MOONFLOWER_CHARM, PUSHKINS_LIST])
                    && count(ctx, killer, ANT_SOLDIER_ACID) < 20
                {
                    ctx.give_items(killer, ANT_SOLDIER_ACID, 2);
                    if count(ctx, killer, ANT_SOLDIER_ACID) == 20 {
                        ctx.play_sound(killer, QuestSound::ItemsoundQuestMiddle);
                        if count(ctx, killer, PURE_MITHRIL_ORE) >= 10 && count(ctx, killer, WYRMS_TALON) >= 20
                        {
                            qs.set_cond(ctx, 5, false);
                        }
                    } else {
                        ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
                    }
                }
            }
            HARPY => {
                if all(ctx, killer, &[MOONFLOWER_CHARM, ADONIUS_LIST]) && count(ctx, killer, HARPYS_DOWN) < 20
                {
                    ctx.give_items(killer, HARPYS_DOWN, 4);
                    if count(ctx, killer, HARPYS_DOWN) == 20 {
                        ctx.play_sound(killer, QuestSound::ItemsoundQuestMiddle);
                        if count(ctx, killer, SPIDER_ICHOR) >= 20 {
                            qs.set_cond(ctx, 10, false);
                        }
                    } else {
                        ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
                    }
                }
            }
            WYRM => {
                if all(ctx, killer, &[MOONFLOWER_CHARM, PUSHKINS_LIST])
                    && count(ctx, killer, WYRMS_TALON) < 20
                {
                    ctx.give_items(killer, WYRMS_TALON, 4);
                    if count(ctx, killer, WYRMS_TALON) == 20 {
                        ctx.play_sound(killer, QuestSound::ItemsoundQuestMiddle);
                        if count(ctx, killer, PURE_MITHRIL_ORE) >= 10
                            && count(ctx, killer, ANT_SOLDIER_ACID) >= 20
                        {
                            qs.set_cond(ctx, 5, false);
                        }
                    } else {
                        ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
                    }
                }
            }
            MARSH_SPIDER => {
                if all(ctx, killer, &[MOONFLOWER_CHARM, ADONIUS_LIST])
                    && count(ctx, killer, SPIDER_ICHOR) < 20
                {
                    ctx.give_items(killer, SPIDER_ICHOR, 4);
                    if count(ctx, killer, SPIDER_ICHOR) == 20 {
                        ctx.play_sound(killer, QuestSound::ItemsoundQuestMiddle);
                        if count(ctx, killer, HARPYS_DOWN) >= 20 {
                            qs.set_cond(ctx, 10, false);
                        }
                    } else {
                        ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
                    }
                }
            }
            GUARDIAN_BASILISK => {
                if all(ctx, killer, &[MOONFLOWER_CHARM, PUSHKINS_LIST])
                    && count(ctx, killer, PURE_MITHRIL_ORE) < 10
                {
                    ctx.give_items(killer, PURE_MITHRIL_ORE, 2);
                    if count(ctx, killer, PURE_MITHRIL_ORE) == 10 {
                        ctx.play_sound(killer, QuestSound::ItemsoundQuestMiddle);
                        if count(ctx, killer, WYRMS_TALON) >= 20 && count(ctx, killer, ANT_SOLDIER_ACID) >= 20
                        {
                            qs.set_cond(ctx, 5, false);
                        }
                    } else {
                        ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
                    }
                }
            }
            LETO_LIZARDMAN_SHAMAN | LETO_LIZARDMAN_OVERLORD => {
                if has(ctx, killer, ISAELS_INSTRUCTIONS) {
                    // The six parts of Talin's spear, one per kill, in this order.
                    let parts = [
                        TALINS_SPEAR_BLADE,
                        TALINS_SPEAR_SHAFT,
                        TALINS_RUBY,
                        TALINS_AQUAMARINE,
                        TALINS_AMETHYST,
                        TALINS_PERIDOT,
                    ];
                    if let Some(part) = parts.into_iter().find(|&p| !has(ctx, killer, p)) {
                        ctx.give_items(killer, part, 1);
                        ctx.play_sound(killer, QuestSound::ItemsoundQuestMiddle);
                    }
                }
            }
            UNICORN_OF_EVA => {
                if !has(ctx, killer, TEARS_OF_UNICORN)
                    && all(ctx, killer, &[MOONFLOWER_CHARM, TALINS_SPEAR, GRAIL_OF_PURITY])
                    && ctx.killing_blow_weapon(npc) == TALINS_SPEAR
                {
                    ctx.take_items(killer, TALINS_SPEAR, 1);
                    ctx.take_items(killer, GRAIL_OF_PURITY, 1);
                    ctx.give_items(killer, TEARS_OF_UNICORN, 1);
                    qs.set_cond(ctx, 19, true);
                }
            }
            _ => {}
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        if qs.is_created(ctx) {
            if npc.id == MASTER_CARDIEN {
                if ctx.player_race(player) == race::ELF {
                    if ctx.player_level(player) >= MIN_LEVEL
                        && ctx.world().is_in_category(player, CategoryType::Elf2ndGroup)
                    {
                        htmltext = "30460-03.htm".to_owned();
                    } else if ctx.player_level(player) >= MIN_LEVEL {
                        htmltext = "30460-01a.html".to_owned();
                    } else {
                        htmltext = "30460-02.html".to_owned();
                    }
                } else {
                    htmltext = "30460-01.html".to_owned();
                }
            }
        } else if qs.is_started(ctx) {
            match npc.id {
                MASTER_CARDIEN => {
                    if has(ctx, player, CARDIENS_LETTER) {
                        htmltext = "30460-05.html".to_owned();
                    } else if has(ctx, player, MOONFLOWER_CHARM) {
                        htmltext = "30460-06.html".to_owned();
                    } else if has(ctx, player, CAMOMILE_CHARM) {
                        ctx.give_adena(player, 342_288, true);
                        ctx.give_items(player, MARK_OF_LIFE, 1);
                        ctx.add_exp_and_sp(player, 1_886_832, 125_918);
                        qs.exit_quest(ctx, false, true);
                        ctx.send_social_action(player, 3);
                        htmltext = "30460-07.html".to_owned();
                    }
                }
                HIERARCH_ASTERIOS => {
                    if has(ctx, player, CARDIENS_LETTER) {
                        htmltext = "30154-01.html".to_owned();
                    } else if has(ctx, player, MOONFLOWER_CHARM) {
                        if !has(ctx, player, WATER_OF_LIFE) {
                            htmltext = "30154-08.html".to_owned();
                        } else {
                            ctx.give_items(player, CAMOMILE_CHARM, 1);
                            ctx.take_items(player, MOONFLOWER_CHARM, 1);
                            ctx.take_items(player, WATER_OF_LIFE, 1);
                            qs.set_cond(ctx, 21, true);
                            htmltext = "30154-09.html".to_owned();
                        }
                    } else if has(ctx, player, CAMOMILE_CHARM) {
                        htmltext = "30154-10.html".to_owned();
                    }
                }
                BLACKSMITH_PUSHKIN => {
                    if has(ctx, player, MOONFLOWER_CHARM) {
                        if has(ctx, player, GRAIL_DIAGRAM) {
                            htmltext = "30300-01.html".to_owned();
                        } else if has(ctx, player, PUSHKINS_LIST) {
                            if count(ctx, player, PURE_MITHRIL_ORE) >= 10
                                && count(ctx, player, ANT_SOLDIER_ACID) >= 20
                                && count(ctx, player, WYRMS_TALON) >= 20
                            {
                                htmltext = "30300-08.html".to_owned();
                            } else {
                                htmltext = "30300-07.html".to_owned();
                            }
                        } else if has(ctx, player, PURE_MITHRIL_CUP) {
                            htmltext = "30300-11.html".to_owned();
                        } else if !any(ctx, player, &[GRAIL_DIAGRAM, PUSHKINS_LIST, PURE_MITHRIL_CUP]) {
                            htmltext = "30300-12.html".to_owned();
                        }
                    }
                }
                THALIA => {
                    if has(ctx, player, MOONFLOWER_CHARM) {
                        if has(ctx, player, HIERARCHS_LETTER) {
                            htmltext = "30371-01.html".to_owned();
                        } else if has(ctx, player, GRAIL_DIAGRAM) {
                            htmltext = "30371-04.html".to_owned();
                        } else if has(ctx, player, PUSHKINS_LIST) {
                            htmltext = "30371-05.html".to_owned();
                        } else if has(ctx, player, PURE_MITHRIL_CUP) {
                            ctx.give_items(player, THALIAS_1ST_LETTER, 1);
                            ctx.take_items(player, PURE_MITHRIL_CUP, 1);
                            qs.set_cond(ctx, 7, true);
                            htmltext = "30371-06.html".to_owned();
                        } else if has(ctx, player, THALIAS_1ST_LETTER) {
                            htmltext = "30371-07.html".to_owned();
                        } else if has(ctx, player, ARKENIAS_CONTRACT) {
                            htmltext = "30371-08.html".to_owned();
                        } else if has(ctx, player, STARDUST) {
                            htmltext = "30371-09.html".to_owned();
                        } else if has(ctx, player, THALIAS_INSTRUCTIONS) {
                            if ctx.player_level(player) >= LEVEL {
                                ctx.take_items(player, THALIAS_INSTRUCTIONS, 1);
                                ctx.give_items(player, THALIAS_2ND_LETTER, 1);
                                qs.set_cond(ctx, 14, true);
                                htmltext = "30371-13.html".to_owned();
                            } else {
                                htmltext = "30371-12.html".to_owned();
                            }
                        } else if has(ctx, player, THALIAS_2ND_LETTER) {
                            htmltext = "30371-14.html".to_owned();
                        } else if has(ctx, player, ISAELS_INSTRUCTIONS) {
                            htmltext = "30371-15.html".to_owned();
                        } else if all(ctx, player, &[TALINS_SPEAR, ISAELS_LETTER]) {
                            ctx.take_items(player, ISAELS_LETTER, 1);
                            ctx.give_items(player, GRAIL_OF_PURITY, 1);
                            qs.set_cond(ctx, 18, true);
                            htmltext = "30371-16.html".to_owned();
                        } else if all(ctx, player, &[TALINS_SPEAR, GRAIL_OF_PURITY]) {
                            htmltext = "30371-17.html".to_owned();
                        } else if has(ctx, player, TEARS_OF_UNICORN) {
                            ctx.take_items(player, TEARS_OF_UNICORN, 1);
                            ctx.give_items(player, WATER_OF_LIFE, 1);
                            qs.set_cond(ctx, 20, true);
                            htmltext = "30371-18.html".to_owned();
                        } else if any(ctx, player, &[CAMOMILE_CHARM, WATER_OF_LIFE]) {
                            htmltext = "30371-19.html".to_owned();
                        }
                    }
                }
                ARKENIA => {
                    if has(ctx, player, MOONFLOWER_CHARM) {
                        if has(ctx, player, THALIAS_1ST_LETTER) {
                            htmltext = "30419-01.html".to_owned();
                        } else if any(ctx, player, &[ARKENIAS_INSTRUCTIONS, ADONIUS_LIST]) {
                            htmltext = "30419-05.html".to_owned();
                        } else if has(ctx, player, ANDARIEL_SCRIPTURE_COPY) {
                            ctx.take_items(player, ARKENIAS_CONTRACT, 1);
                            ctx.take_items(player, ANDARIEL_SCRIPTURE_COPY, 1);
                            ctx.give_items(player, STARDUST, 1);
                            qs.set_cond(ctx, 12, true);
                            htmltext = "30419-06.html".to_owned();
                        } else if has(ctx, player, STARDUST) {
                            htmltext = "30419-07.html".to_owned();
                        } else if !any(
                            ctx,
                            player,
                            &[
                                THALIAS_1ST_LETTER,
                                ARKENIAS_CONTRACT,
                                ANDARIEL_SCRIPTURE_COPY,
                                STARDUST,
                            ],
                        ) {
                            htmltext = "30419-08.html".to_owned();
                        }
                    }
                }
                PRIEST_ADONIUS => {
                    if has(ctx, player, MOONFLOWER_CHARM) {
                        if has(ctx, player, ARKENIAS_INSTRUCTIONS) {
                            htmltext = "30375-01.html".to_owned();
                        } else if has(ctx, player, ADONIUS_LIST) {
                            if count(ctx, player, SPIDER_ICHOR) >= 20 && count(ctx, player, HARPYS_DOWN) >= 20
                            {
                                ctx.take_items(player, ADONIUS_LIST, 1);
                                ctx.give_items(player, ANDARIEL_SCRIPTURE_COPY, 1);
                                ctx.take_items(player, SPIDER_ICHOR, -1);
                                ctx.take_items(player, HARPYS_DOWN, -1);
                                qs.set_cond(ctx, 11, true);
                                htmltext = "30375-04.html".to_owned();
                            } else {
                                htmltext = "30375-03.html".to_owned();
                            }
                        } else if has(ctx, player, ANDARIEL_SCRIPTURE_COPY) {
                            htmltext = "30375-05.html".to_owned();
                        } else if !any(
                            ctx,
                            player,
                            &[ARKENIAS_INSTRUCTIONS, ADONIUS_LIST, ANDARIEL_SCRIPTURE_COPY],
                        ) {
                            htmltext = "30375-06.html".to_owned();
                        }
                    }
                }
                ISAEL_SILVERSHADOW => {
                    if has(ctx, player, MOONFLOWER_CHARM) {
                        if has(ctx, player, THALIAS_2ND_LETTER) {
                            htmltext = "30655-01.html".to_owned();
                        } else if has(ctx, player, ISAELS_INSTRUCTIONS) {
                            if all(
                                ctx,
                                player,
                                &[
                                    TALINS_SPEAR_BLADE,
                                    TALINS_SPEAR_SHAFT,
                                    TALINS_RUBY,
                                    TALINS_AQUAMARINE,
                                    TALINS_AMETHYST,
                                    TALINS_PERIDOT,
                                ],
                            ) {
                                ctx.give_items(player, TALINS_SPEAR, 1);
                                ctx.take_items(player, ISAELS_INSTRUCTIONS, 1);
                                ctx.give_items(player, ISAELS_LETTER, 1);
                                ctx.take_items(player, TALINS_SPEAR_BLADE, 1);
                                ctx.take_items(player, TALINS_SPEAR_SHAFT, 1);
                                ctx.take_items(player, TALINS_RUBY, 1);
                                ctx.take_items(player, TALINS_AQUAMARINE, 1);
                                ctx.take_items(player, TALINS_AMETHYST, 1);
                                ctx.take_items(player, TALINS_PERIDOT, 1);
                                qs.set_cond(ctx, 17, true);
                                htmltext = "30655-04.html".to_owned();
                            } else {
                                htmltext = "30655-03.html".to_owned();
                            }
                        } else if all(ctx, player, &[TALINS_SPEAR, ISAELS_LETTER]) {
                            htmltext = "30655-05.html".to_owned();
                        } else if any(ctx, player, &[GRAIL_OF_PURITY, WATER_OF_LIFE, CAMOMILE_CHARM]) {
                            htmltext = "30655-06.html".to_owned();
                        }
                    }
                }
                _ => {}
            }
        } else if qs.is_completed(ctx) && npc.id == MASTER_CARDIEN {
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

    const NAME: &str = "Q00218_TestimonyOfLife";

    const NPCS: &str = r#"
        <npc id="30154" type="Folk" level="70" name="Hierarch Asterios"/>
        <npc id="30300" type="Folk" level="70" name="Blacksmith Pushkin"/>
        <npc id="30371" type="Folk" level="70" name="Thalia"/>
        <npc id="30460" type="Folk" level="70" name="Master Cardien"/>
        <npc id="20176" type="Monster" level="30" name="Wyrm"/>
        <npc id="20581" type="Monster" level="30" name="Leto Lizardman Shaman"/>
        <npc id="27077" type="Monster" level="30" name="Unicorn of Eva"/>"#;

    fn items() -> String {
        let mut s = String::from(
            r#"<item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="7562" type="EtcItem" name="Dimensional Diamond"><set name="is_stackable" val="true"/></item>
        <item id="3026" type="EtcItem" name="Talin's Spear"><set name="is_questitem" val="true"/></item>
        <item id="3140" type="EtcItem" name="Mark of Life"><set name="is_questitem" val="true"/></item>"#,
        );
        for id in 3141..=3171 {
            let stack = if matches!(id, 3161..=3165) {
                r#"<set name="is_stackable" val="true"/>"#
            } else {
                ""
            };
            s.push_str(&format!(
                r#"<item id="{id}" type="EtcItem" name="Q218 {id}"><set name="is_questitem" val="true"/>{stack}</item>"#
            ));
        }
        s
    }

    const EVENTS: [&str; 4] = ["ACCEPT", "30154-07.html", "30371-03.html", "30300-06.html"];
    const FILES: [&str; 11] = [
        "30460-01.html",
        "30460-01a.html",
        "30460-02.html",
        "30460-03.htm",
        "30460-04a.htm",
        "30460-07.html",
        "30154-01.html",
        "30371-01.html",
        "30300-01.html",
        "30371-04.html",
        "30460-06.html",
    ];

    struct Npcs {
        cardien: Oid,
        asterios: Oid,
        thalia: Oid,
    }

    fn setup() -> (ScriptTest, Npcs) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00218TestimonyOfLife);
        let mut pages = Pages::of(&Q00218TestimonyOfLife);
        for file in FILES {
            pages = pages.page(file, &EVENTS);
        }
        for event in EVENTS.iter().filter(|e| **e != "ACCEPT") {
            pages = pages.page(event, &EVENTS);
        }
        let pages = pages.npcs(&[MASTER_CARDIEN, HIERARCH_ASTERIOS, THALIA, BLACKSMITH_PUSHKIN]);
        let mut t = ScriptTest::with_pages(registry, NPCS, &items(), pages);
        let npcs = Npcs {
            cardien: t.spawn(MASTER_CARDIEN, 20),
            asterios: t.spawn(HIERARCH_ASTERIOS, 30),
            thalia: t.spawn(THALIA, 40),
        };
        (t, npcs)
    }

    #[test]
    fn only_an_elf_of_level_37_in_a_second_class_is_offered_the_quest() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 37, race::HUMAN, player_class::KNIGHT, vec![]);
        assert!(t.quest_page(1, npcs.cardien).contains("30460-01.html"));
        t.enter_as(2, 36, race::ELF, player_class::ELVEN_KNIGHT, vec![]);
        assert!(t.quest_page(2, npcs.cardien).contains("30460-02.html"));
        t.enter_as(3, 37, race::ELF, player_class::ELVEN_FIGHTER, vec![]);
        assert!(t.quest_page(3, npcs.cardien).contains("30460-01a.html"));
        t.enter_as(4, 37, race::ELF, player_class::ELVEN_KNIGHT, vec![]);
        assert!(t.quest_page(4, npcs.cardien).contains("30460-03.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_hierarch_and_thalia_give_the_first_steps() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 37, race::ELF, player_class::ELVEN_KNIGHT, vec![]);
        assert!(t.quest_page(1, npcs.cardien).contains("30460-03.htm"));
        assert!(t.event(1, NAME, "ACCEPT").contains("30460-04a.htm"));
        assert_eq!(t.count(1, DIMENSIONAL_DIAMOND), 102);
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        assert!(t.quest_page(1, npcs.asterios).contains("30154-01.html"));
        assert!(t.event(1, NAME, "30154-07.html").contains("30154-07.html"));
        assert_eq!(t.count(1, MOONFLOWER_CHARM), 1);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);
        assert!(t.quest_page(1, npcs.thalia).contains("30371-01.html"));
        assert!(t.event(1, NAME, "30371-03.html").contains("30371-03.html"));
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "3")]);
    }

    #[test]
    fn wyrms_give_four_talons_each_and_the_lizardman_the_spear_parts_in_order() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 37, race::ELF, player_class::ELVEN_KNIGHT, vec![]);
        assert!(t.quest_page(1, npcs.cardien).contains("30460-03.htm"));
        assert!(t.event(1, NAME, "ACCEPT").contains("30460-04a.htm"));
        t.world.run_as_script(NAME, |ctx| {
            ctx.give_items(Oid(1), MOONFLOWER_CHARM, 1);
            ctx.give_items(Oid(1), PUSHKINS_LIST, 1);
            ctx.give_items(Oid(1), ISAELS_INSTRUCTIONS, 1);
        });
        t.take_writes();
        let wyrm = t.spawn(WYRM, 60);
        t.kill(1, wyrm);
        t.world.delete_npc(wyrm);
        assert_eq!(t.count(1, WYRMS_TALON), 4);
        for _ in 0..2 {
            let lizard = t.spawn(LETO_LIZARDMAN_SHAMAN, 60);
            t.kill(1, lizard);
            t.world.delete_npc(lizard);
        }
        assert_eq!(t.count(1, TALINS_SPEAR_BLADE), 1);
        assert_eq!(t.count(1, TALINS_SPEAR_SHAFT), 1);
        assert_eq!(t.count(1, TALINS_RUBY), 0);
    }

    #[test]
    fn a_unicorn_not_killed_with_the_spear_gives_no_tears_and_cardien_pays_the_mark() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 37, race::ELF, player_class::ELVEN_KNIGHT, vec![]);
        assert!(t.quest_page(1, npcs.cardien).contains("30460-03.htm"));
        assert!(t.event(1, NAME, "ACCEPT").contains("30460-04a.htm"));
        t.world.run_as_script(NAME, |ctx| {
            ctx.give_items(Oid(1), MOONFLOWER_CHARM, 1);
            ctx.give_items(Oid(1), TALINS_SPEAR, 1);
            ctx.give_items(Oid(1), GRAIL_OF_PURITY, 1);
        });
        t.take_writes();
        let unicorn = t.spawn(UNICORN_OF_EVA, 60);
        t.kill(1, unicorn);
        t.world.delete_npc(unicorn);
        assert_eq!(t.count(1, TEARS_OF_UNICORN), 0);

        // The camomile charm is the end: Cardien pays.
        t.world.run_as_script(NAME, |ctx| {
            ctx.take_items(Oid(1), MOONFLOWER_CHARM, -1);
            ctx.take_items(Oid(1), CARDIENS_LETTER, -1);
            ctx.give_items(Oid(1), CAMOMILE_CHARM, 1);
        });
        t.take_writes();
        assert!(t.quest_page(1, npcs.cardien).contains("30460-07.html"));
        assert_eq!(t.count(1, MARK_OF_LIFE), 1);
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed")
            ]
        );
    }
}
