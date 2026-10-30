//! Quest 228: Test of Magus (Bard Rukal in Gludio: the Wizard, Elven Wizard and Dark Wizard second
//! class change, through the four elemental elders and the singing flowers).
// Java: dist/game/data/scripts/quests/Q00228_TestOfMagus/Q00228_TestOfMagus.java
//
// Pattern: the test chain of the "Test of ..." quests (see Q00222), with four elements. Each
// element is a charm from its elder, a hunt, and a tone; the singing flowers talk when they die.

use l2_protocol_game::NpcStringId;
use l2_protocol_game::server::{ChatType, SocialAction};
use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, STARTED, player_class};
use l2_world::{QuestSound, Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const PARINA: i32 = 30391;
const EARTH_SNAKE: i32 = 30409;
const FLAME_SALAMANDER: i32 = 30411;
const WIND_SYLPH: i32 = 30412;
const WATER_UNDINE: i32 = 30413;
const ELDER_CASIAN: i32 = 30612;
const BARD_RUKAL: i32 = 30629;

// Items
const RUKALS_LETTER: i32 = 2841;
const PARINAS_LETTER: i32 = 2842;
const LILAC_CHARM: i32 = 2843;
const GOLDEN_SEED_1ST: i32 = 2844;
const GOLDEN_SEED_2ND: i32 = 2845;
const GOLDEN_SEED_3RD: i32 = 2846;
const SCORE_OF_ELEMENTS: i32 = 2847;
const DAZZLING_DROP: i32 = 2848;
const FLAME_CRYSTAL: i32 = 2849;
const HARPYS_FEATHER: i32 = 2850;
const WYRMS_WINGBONE: i32 = 2851;
const WINDSUS_MANE: i32 = 2852;
const ENCHANTED_MONSTER_EYE_SHELL: i32 = 2853;
const ENCHANTED_GOLEM_POWDER: i32 = 2854;
const ENCHANTED_IRON_GOLEM_SCRAP: i32 = 2855;
const TONE_OF_WATER: i32 = 2856;
const TONE_OF_FIRE: i32 = 2857;
const TONE_OF_WIND: i32 = 2858;
const TONE_OF_EARTH: i32 = 2859;
const SALAMANDER_CHARM: i32 = 2860;
const SYLPH_CHARM: i32 = 2861;
const UNDINE_CHARM: i32 = 2862;
const SERPENT_CHARM: i32 = 2863;

// Reward
const MARK_OF_MAGUS: i32 = 2840;
const DIMENSIONAL_DIAMOND: i32 = 7562;

// Monsters
const HARPY: i32 = 20145;
const MARSH_STAKATO: i32 = 20157;
const WYRM: i32 = 20176;
const MARSH_STAKATO_WORKER: i32 = 20230;
const TOAD_LORD: i32 = 20231;
const MARSH_STAKATO_SOLDIER: i32 = 20232;
const MARSH_STAKATO_DRONE: i32 = 20234;
const WINDSUS: i32 = 20553;
const ENCHANTED_MONSTEREYE: i32 = 20564;
const ENCHANTED_STOLEN_GOLEM: i32 = 20565;
const ENCHANTED_IRON_GOLEM: i32 = 20566;

// Quest monsters
const SINGING_FLOWER_PHANTASM: i32 = 27095;
const SINGING_FLOWER_NIGTMATE: i32 = 27096;
const SINGING_FLOWER_DARKLING: i32 = 27097;
const GHOST_FIRE: i32 = 27098;

// Misc
const MIN_LEVEL: i32 = 39;

/// Java `Q00228_TestOfMagus`.
pub struct Q00228TestOfMagus;

impl Script for Q00228TestOfMagus {
    fn name(&self) -> &'static str {
        "Q00228_TestOfMagus"
    }

    fn path(&self) -> &'static str {
        "quests/Q00228_TestOfMagus"
    }

    fn id(&self) -> i32 {
        228
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[BARD_RUKAL]);
        s.add_talk_id(&[
            BARD_RUKAL,
            PARINA,
            EARTH_SNAKE,
            FLAME_SALAMANDER,
            WIND_SYLPH,
            WATER_UNDINE,
            ELDER_CASIAN,
        ]);
        s.add_kill_id(&[
            HARPY,
            MARSH_STAKATO,
            WYRM,
            MARSH_STAKATO_WORKER,
            TOAD_LORD,
            MARSH_STAKATO_SOLDIER,
            MARSH_STAKATO_DRONE,
            WINDSUS,
            ENCHANTED_MONSTEREYE,
            ENCHANTED_STOLEN_GOLEM,
            ENCHANTED_IRON_GOLEM,
            SINGING_FLOWER_PHANTASM,
            SINGING_FLOWER_NIGTMATE,
            SINGING_FLOWER_DARKLING,
            GHOST_FIRE,
        ]);
        s.register_quest_items(&[
            RUKALS_LETTER,
            PARINAS_LETTER,
            LILAC_CHARM,
            GOLDEN_SEED_1ST,
            GOLDEN_SEED_2ND,
            GOLDEN_SEED_3RD,
            SCORE_OF_ELEMENTS,
            DAZZLING_DROP,
            FLAME_CRYSTAL,
            HARPYS_FEATHER,
            WYRMS_WINGBONE,
            WINDSUS_MANE,
            ENCHANTED_MONSTER_EYE_SHELL,
            ENCHANTED_GOLEM_POWDER,
            ENCHANTED_IRON_GOLEM_SCRAP,
            TONE_OF_WATER,
            TONE_OF_FIRE,
            TONE_OF_WIND,
            TONE_OF_EARTH,
            SALAMANDER_CHARM,
            SYLPH_CHARM,
            UNDINE_CHARM,
            SERPENT_CHARM,
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
                    ctx.play_sound(player, QuestSound::ItemsoundQuestMiddle);
                    ctx.give_items(player, RUKALS_LETTER, 1);
                    if ctx.player_variable_int(player, "2ND_CLASS_DIAMOND_REWARD", 0) == 0 {
                        ctx.give_items(player, DIMENSIONAL_DIAMOND, 122);
                        ctx.set_player_variable(player, "2ND_CLASS_DIAMOND_REWARD", "1");
                        htmltext = Some("30629-04a.htm".to_owned());
                    } else {
                        htmltext = Some("30629-04.htm".to_owned());
                    }
                }
            }
            "30629-09.html" | "30409-02.html" => {
                htmltext = Some(event.to_owned());
            }
            "30629-10.html" => {
                if ctx.has_quest_items(player, GOLDEN_SEED_3RD) {
                    ctx.take_items(player, LILAC_CHARM, 1);
                    ctx.take_items(player, GOLDEN_SEED_1ST, 1);
                    ctx.take_items(player, GOLDEN_SEED_2ND, 1);
                    ctx.take_items(player, GOLDEN_SEED_3RD, 1);
                    ctx.give_items(player, SCORE_OF_ELEMENTS, 1);
                    qs.set_cond(ctx, 5, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "30391-02.html" => {
                if ctx.has_quest_items(player, RUKALS_LETTER) {
                    ctx.take_items(player, RUKALS_LETTER, 1);
                    ctx.give_items(player, PARINAS_LETTER, 1);
                    qs.set_cond(ctx, 2, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "30409-03.html" => {
                ctx.give_items(player, SERPENT_CHARM, 1);
                htmltext = Some(event.to_owned());
            }
            "30412-02.html" => {
                ctx.give_items(player, SYLPH_CHARM, 1);
                htmltext = Some(event.to_owned());
            }
            "30612-02.html" => {
                ctx.take_items(player, PARINAS_LETTER, 1);
                ctx.give_items(player, LILAC_CHARM, 1);
                qs.set_cond(ctx, 3, true);
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
        if qs.is_started(ctx) && ctx.check_if_in_range(ctx.alt_party_range(), npc.object_id, killer, true) {
            match npc.id {
                HARPY => {
                    if ctx.has_quest_items(killer, SCORE_OF_ELEMENTS)
                        && ctx.has_quest_items(killer, SYLPH_CHARM)
                        && ctx.get_quest_items_count(killer, HARPYS_FEATHER) < 20
                    {
                        ctx.give_items(killer, HARPYS_FEATHER, 1);
                        play_collect_sound(
                            ctx,
                            killer,
                            ctx.get_quest_items_count(killer, HARPYS_FEATHER),
                            20,
                        );
                    }
                }
                MARSH_STAKATO
                | MARSH_STAKATO_WORKER
                | TOAD_LORD
                | MARSH_STAKATO_SOLDIER
                | MARSH_STAKATO_DRONE => {
                    if ctx.has_quest_items(killer, SCORE_OF_ELEMENTS)
                        && ctx.has_quest_items(killer, UNDINE_CHARM)
                        && ctx.get_quest_items_count(killer, DAZZLING_DROP) < 20
                    {
                        ctx.give_items(killer, DAZZLING_DROP, 1);
                        play_collect_sound(ctx, killer, ctx.get_quest_items_count(killer, DAZZLING_DROP), 20);
                    }
                }
                WYRM => {
                    if ctx.has_quest_items(killer, SCORE_OF_ELEMENTS)
                        && ctx.has_quest_items(killer, SYLPH_CHARM)
                        && ctx.get_quest_items_count(killer, WYRMS_WINGBONE) < 10
                        && ctx.get_random_boolean()
                    {
                        ctx.give_items(killer, WYRMS_WINGBONE, 1);
                        play_collect_sound(
                            ctx,
                            killer,
                            ctx.get_quest_items_count(killer, WYRMS_WINGBONE),
                            10,
                        );
                    }
                }
                WINDSUS => {
                    if ctx.has_quest_items(killer, SCORE_OF_ELEMENTS)
                        && ctx.has_quest_items(killer, SYLPH_CHARM)
                        && ctx.get_quest_items_count(killer, WINDSUS_MANE) < 10
                        && ctx.get_random_boolean()
                    {
                        ctx.give_items(killer, WINDSUS_MANE, 1);
                        play_collect_sound(ctx, killer, ctx.get_quest_items_count(killer, WINDSUS_MANE), 10);
                    }
                }
                ENCHANTED_MONSTEREYE => {
                    if ctx.has_quest_items(killer, SCORE_OF_ELEMENTS)
                        && ctx.has_quest_items(killer, SERPENT_CHARM)
                        && ctx.get_quest_items_count(killer, ENCHANTED_MONSTER_EYE_SHELL) < 10
                    {
                        ctx.give_items(killer, ENCHANTED_MONSTER_EYE_SHELL, 1);
                        play_collect_sound(
                            ctx,
                            killer,
                            ctx.get_quest_items_count(killer, ENCHANTED_MONSTER_EYE_SHELL),
                            10,
                        );
                    }
                }
                ENCHANTED_STOLEN_GOLEM => {
                    if ctx.has_quest_items(killer, SCORE_OF_ELEMENTS)
                        && ctx.has_quest_items(killer, SERPENT_CHARM)
                        && ctx.get_quest_items_count(killer, ENCHANTED_GOLEM_POWDER) < 10
                    {
                        ctx.give_items(killer, ENCHANTED_GOLEM_POWDER, 1);
                        play_collect_sound(
                            ctx,
                            killer,
                            ctx.get_quest_items_count(killer, ENCHANTED_GOLEM_POWDER),
                            10,
                        );
                    }
                }
                ENCHANTED_IRON_GOLEM => {
                    if ctx.has_quest_items(killer, SCORE_OF_ELEMENTS)
                        && ctx.has_quest_items(killer, SERPENT_CHARM)
                        && ctx.get_quest_items_count(killer, ENCHANTED_IRON_GOLEM_SCRAP) < 10
                    {
                        ctx.give_items(killer, ENCHANTED_IRON_GOLEM_SCRAP, 1);
                        play_collect_sound(
                            ctx,
                            killer,
                            ctx.get_quest_items_count(killer, ENCHANTED_IRON_GOLEM_SCRAP),
                            10,
                        );
                    }
                }
                SINGING_FLOWER_PHANTASM => {
                    if ctx.has_quest_items(killer, LILAC_CHARM)
                        && !ctx.has_quest_items(killer, GOLDEN_SEED_1ST)
                    {
                        ctx.give_items(killer, GOLDEN_SEED_1ST, 1);
                        ctx.broadcast_npc_say(
                            npc,
                            ChatType::NpcGeneral,
                            NpcStringId::IAmATreeOfNothingATreeThatKnowsWhereToReturn,
                            &[],
                        );
                        ctx.play_sound(killer, QuestSound::ItemsoundQuestMiddle);
                        if ctx.has_all_quest_items(killer, &[GOLDEN_SEED_2ND, GOLDEN_SEED_3RD]) {
                            qs.set_cond(ctx, 4, false);
                        }
                    }
                }
                SINGING_FLOWER_NIGTMATE => {
                    if ctx.has_quest_items(killer, LILAC_CHARM)
                        && !ctx.has_quest_items(killer, GOLDEN_SEED_2ND)
                    {
                        ctx.give_items(killer, GOLDEN_SEED_2ND, 1);
                        ctx.broadcast_npc_say(
                            npc,
                            ChatType::NpcGeneral,
                            NpcStringId::IAmACreatureThatShowsTheTruthOfThePlaceDeepInMyHeart,
                            &[],
                        );
                        ctx.play_sound(killer, QuestSound::ItemsoundQuestMiddle);
                        if ctx.has_all_quest_items(killer, &[GOLDEN_SEED_1ST, GOLDEN_SEED_3RD]) {
                            qs.set_cond(ctx, 4, false);
                        }
                    }
                }
                SINGING_FLOWER_DARKLING => {
                    if ctx.has_quest_items(killer, LILAC_CHARM)
                        && !ctx.has_quest_items(killer, GOLDEN_SEED_3RD)
                    {
                        ctx.give_items(killer, GOLDEN_SEED_3RD, 1);
                        ctx.broadcast_npc_say(
                            npc,
                            ChatType::NpcGeneral,
                            NpcStringId::IAmAMirrorOfDarknessAVirtualImageOfDarkness,
                            &[],
                        );
                        ctx.play_sound(killer, QuestSound::ItemsoundQuestMiddle);
                        if ctx.has_all_quest_items(killer, &[GOLDEN_SEED_1ST, GOLDEN_SEED_2ND]) {
                            qs.set_cond(ctx, 4, false);
                        }
                    }
                }
                GHOST_FIRE => {
                    if ctx.has_quest_items(killer, SCORE_OF_ELEMENTS)
                        && ctx.has_quest_items(killer, SALAMANDER_CHARM)
                        && ctx.get_quest_items_count(killer, FLAME_CRYSTAL) < 5
                        && ctx.get_random_boolean()
                    {
                        ctx.give_items(killer, FLAME_CRYSTAL, 1);
                        play_collect_sound(ctx, killer, ctx.get_quest_items_count(killer, FLAME_CRYSTAL), 5);
                    }
                }
                _ => {}
            }
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        let state = qs.state(ctx);
        if state == CREATED {
            if npc.id == BARD_RUKAL {
                if matches!(
                    ctx.player_class_id(player),
                    player_class::WIZARD | player_class::ELVEN_WIZARD | player_class::DARK_WIZARD
                ) {
                    if ctx.player_level(player) < MIN_LEVEL {
                        htmltext = "30629-02.html".to_owned();
                    } else {
                        htmltext = "30629-03.htm".to_owned();
                    }
                } else {
                    htmltext = "30629-01.html".to_owned();
                }
            }
        } else if state == STARTED {
            match npc.id {
                BARD_RUKAL => {
                    if ctx.has_quest_items(player, RUKALS_LETTER) {
                        htmltext = "30629-05.html".to_owned();
                    } else if ctx.has_quest_items(player, PARINAS_LETTER) {
                        htmltext = "30629-06.html".to_owned();
                    } else if ctx.has_quest_items(player, LILAC_CHARM) {
                        if ctx
                            .has_all_quest_items(player, &[GOLDEN_SEED_1ST, GOLDEN_SEED_2ND, GOLDEN_SEED_3RD])
                        {
                            htmltext = "30629-08.html".to_owned();
                        } else {
                            htmltext = "30629-07.html".to_owned();
                        }
                    } else if ctx.has_quest_items(player, SCORE_OF_ELEMENTS) {
                        if ctx.has_all_quest_items(
                            player,
                            &[TONE_OF_WATER, TONE_OF_FIRE, TONE_OF_WIND, TONE_OF_EARTH],
                        ) {
                            ctx.give_adena(player, 372_154, true);
                            ctx.give_items(player, MARK_OF_MAGUS, 1);
                            ctx.add_exp_and_sp(player, 2_058_244, 141_240);
                            qs.exit_quest(ctx, false, true);
                            ctx.world().send(
                                player,
                                &SocialAction {
                                    object_id: player.0,
                                    action_id: 3,
                                },
                            );
                            htmltext = "30629-12.html".to_owned();
                        } else {
                            htmltext = "30629-11.html".to_owned();
                        }
                    }
                }
                PARINA => {
                    if ctx.has_quest_items(player, RUKALS_LETTER) {
                        htmltext = "30391-01.html".to_owned();
                    } else if ctx.has_quest_items(player, PARINAS_LETTER) {
                        htmltext = "30391-03.html".to_owned();
                    } else if ctx.has_quest_items(player, LILAC_CHARM) {
                        htmltext = "30391-04.html".to_owned();
                    } else if ctx.has_quest_items(player, SCORE_OF_ELEMENTS) {
                        htmltext = "30391-05.html".to_owned();
                    }
                }
                EARTH_SNAKE => {
                    if ctx.has_quest_items(player, SCORE_OF_ELEMENTS) {
                        if !ctx.has_at_least_one_quest_item(player, &[TONE_OF_EARTH, SERPENT_CHARM]) {
                            htmltext = "30409-01.html".to_owned();
                        } else if ctx.has_quest_items(player, SERPENT_CHARM) {
                            if ctx.get_quest_items_count(player, ENCHANTED_MONSTER_EYE_SHELL) >= 10
                                && ctx.get_quest_items_count(player, ENCHANTED_GOLEM_POWDER) >= 10
                                && ctx.get_quest_items_count(player, ENCHANTED_IRON_GOLEM_SCRAP) >= 10
                            {
                                ctx.take_items(player, ENCHANTED_MONSTER_EYE_SHELL, -1);
                                ctx.take_items(player, ENCHANTED_GOLEM_POWDER, -1);
                                ctx.take_items(player, ENCHANTED_IRON_GOLEM_SCRAP, -1);
                                ctx.give_items(player, TONE_OF_EARTH, 1);
                                ctx.take_items(player, SERPENT_CHARM, 1);
                                if ctx
                                    .has_all_quest_items(player, &[TONE_OF_FIRE, TONE_OF_WATER, TONE_OF_WIND])
                                {
                                    qs.set_cond(ctx, 6, true);
                                }
                                htmltext = "30409-05.html".to_owned();
                            } else {
                                htmltext = "30409-04.html".to_owned();
                            }
                        } else if ctx.has_quest_items(player, TONE_OF_EARTH)
                            && !ctx.has_quest_items(player, SERPENT_CHARM)
                        {
                            htmltext = "30409-06.html".to_owned();
                        }
                    }
                }
                FLAME_SALAMANDER => {
                    if ctx.has_quest_items(player, SCORE_OF_ELEMENTS) {
                        if !ctx.has_at_least_one_quest_item(player, &[TONE_OF_FIRE, SALAMANDER_CHARM]) {
                            htmltext = "30411-01.html".to_owned();
                            ctx.give_items(player, SALAMANDER_CHARM, 1);
                        } else if ctx.has_quest_items(player, SALAMANDER_CHARM) {
                            if ctx.get_quest_items_count(player, FLAME_CRYSTAL) < 5 {
                                htmltext = "30411-02.html".to_owned();
                            } else {
                                ctx.take_items(player, FLAME_CRYSTAL, -1);
                                ctx.give_items(player, TONE_OF_FIRE, 1);
                                ctx.take_items(player, SALAMANDER_CHARM, 1);
                                if ctx.has_all_quest_items(
                                    player,
                                    &[TONE_OF_WATER, TONE_OF_WIND, TONE_OF_EARTH],
                                ) {
                                    qs.set_cond(ctx, 6, true);
                                }
                                htmltext = "30411-03.html".to_owned();
                            }
                        } else if ctx.has_quest_items(player, TONE_OF_FIRE)
                            && !ctx.has_quest_items(player, SALAMANDER_CHARM)
                        {
                            htmltext = "30411-04.html".to_owned();
                        }
                    }
                }
                WIND_SYLPH => {
                    if ctx.has_quest_items(player, SCORE_OF_ELEMENTS) {
                        if !ctx.has_at_least_one_quest_item(player, &[TONE_OF_WIND, SYLPH_CHARM]) {
                            htmltext = "30412-01.html".to_owned();
                        } else if ctx.has_quest_items(player, SYLPH_CHARM) {
                            if ctx.get_quest_items_count(player, HARPYS_FEATHER) >= 20
                                && ctx.get_quest_items_count(player, WYRMS_WINGBONE) >= 10
                                && ctx.get_quest_items_count(player, WINDSUS_MANE) >= 10
                            {
                                ctx.take_items(player, HARPYS_FEATHER, -1);
                                ctx.take_items(player, WYRMS_WINGBONE, -1);
                                ctx.take_items(player, WINDSUS_MANE, -1);
                                ctx.give_items(player, TONE_OF_WIND, 1);
                                ctx.take_items(player, SYLPH_CHARM, 1);
                                if ctx.has_all_quest_items(
                                    player,
                                    &[TONE_OF_WATER, TONE_OF_FIRE, TONE_OF_EARTH],
                                ) {
                                    qs.set_cond(ctx, 6, true);
                                }
                                htmltext = "30412-04.html".to_owned();
                            } else {
                                htmltext = "30412-03.html".to_owned();
                            }
                        } else if ctx.has_quest_items(player, TONE_OF_WIND)
                            && !ctx.has_quest_items(player, SYLPH_CHARM)
                        {
                            htmltext = "30412-05.html".to_owned();
                        }
                    }
                }
                WATER_UNDINE => {
                    if ctx.has_quest_items(player, SCORE_OF_ELEMENTS) {
                        if !ctx.has_at_least_one_quest_item(player, &[TONE_OF_WATER, UNDINE_CHARM]) {
                            htmltext = "30413-01.html".to_owned();
                            ctx.give_items(player, UNDINE_CHARM, 1);
                        } else if ctx.has_quest_items(player, UNDINE_CHARM) {
                            if ctx.get_quest_items_count(player, DAZZLING_DROP) < 20 {
                                htmltext = "30413-02.html".to_owned();
                            } else {
                                ctx.take_items(player, DAZZLING_DROP, -1);
                                ctx.give_items(player, TONE_OF_WATER, 1);
                                ctx.take_items(player, UNDINE_CHARM, 1);
                                if ctx
                                    .has_all_quest_items(player, &[TONE_OF_FIRE, TONE_OF_WIND, TONE_OF_EARTH])
                                {
                                    qs.set_cond(ctx, 6, true);
                                }
                                htmltext = "30413-03.html".to_owned();
                            }
                        } else if ctx.has_quest_items(player, TONE_OF_WATER)
                            && !ctx.has_quest_items(player, UNDINE_CHARM)
                        {
                            htmltext = "30413-04.html".to_owned();
                        }
                    }
                }
                ELDER_CASIAN => {
                    if ctx.has_quest_items(player, PARINAS_LETTER) {
                        htmltext = "30612-01.html".to_owned();
                    } else if ctx.has_quest_items(player, LILAC_CHARM) {
                        if ctx
                            .has_all_quest_items(player, &[GOLDEN_SEED_1ST, GOLDEN_SEED_2ND, GOLDEN_SEED_3RD])
                        {
                            htmltext = "30612-04.html".to_owned();
                        } else {
                            htmltext = "30612-03.html".to_owned();
                        }
                    } else if ctx.has_quest_items(player, SCORE_OF_ELEMENTS) {
                        htmltext = "30612-05.html".to_owned();
                    }
                }
                _ => {}
            }
        } else if state == COMPLETED && npc.id == BARD_RUKAL {
            htmltext = ctx.get_already_completed_msg(player);
        }
        Some(htmltext)
    }
}

/// Java: the collecting sound of a drop, the middle sound once the count reaches `limit`.
fn play_collect_sound(ctx: &mut ScriptCtx<'_>, killer: ObjectId, count: i64, limit: i64) {
    if count >= limit {
        ctx.play_sound(killer, QuestSound::ItemsoundQuestMiddle);
    } else {
        ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
    }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::ScriptRegistry;
    use l2_world::script::{player_class, race};

    use super::*;
    use crate::testing::{Pages, ScriptTest, names_to, pages_to, rows};

    const NAME: &str = "Q00228_TestOfMagus";

    const NPCS: &str = r#"
        <npc id="30629" type="Folk" level="70" name="Bard Rukal"/>
        <npc id="30391" type="Folk" level="70" name="Parina"/>
        <npc id="30409" type="Folk" level="70" name="Earth Snake"/>
        <npc id="30612" type="Folk" level="70" name="Elder Casian"/>
        <npc id="20145" type="Monster" level="40" name="Harpy"/>
        <npc id="27095" type="Monster" level="40" name="Singing Flower Phantasm"/>
        <npc id="27096" type="Monster" level="40" name="Singing Flower Nigtmate"/>
        <npc id="27097" type="Monster" level="40" name="Singing Flower Darkling"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="7562" type="EtcItem" name="Dimensional Diamond"><set name="is_stackable" val="true"/></item>
        <item id="2840" type="EtcItem" name="Mark of Magus"><set name="is_questitem" val="true"/></item>
        <item id="2841" type="EtcItem" name="Rukal's Letter"><set name="is_questitem" val="true"/></item>
        <item id="2842" type="EtcItem" name="Parina's Letter"><set name="is_questitem" val="true"/></item>
        <item id="2843" type="EtcItem" name="Lilac Charm"><set name="is_questitem" val="true"/></item>
        <item id="2844" type="EtcItem" name="Golden Seed 1st"><set name="is_questitem" val="true"/></item>
        <item id="2845" type="EtcItem" name="Golden Seed 2nd"><set name="is_questitem" val="true"/></item>
        <item id="2846" type="EtcItem" name="Golden Seed 3rd"><set name="is_questitem" val="true"/></item>
        <item id="2847" type="EtcItem" name="Score of Elements"><set name="is_questitem" val="true"/></item>
        <item id="2848" type="EtcItem" name="Dazzling Drop"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="2849" type="EtcItem" name="Flame Crystal"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="2850" type="EtcItem" name="Harpy's Feather"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="2851" type="EtcItem" name="Wyrm's Wingbone"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="2852" type="EtcItem" name="Windsus' Mane"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="2853" type="EtcItem" name="Enchanted Monster Eye Shell"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="2854" type="EtcItem" name="Enchanted Golem Powder"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="2855" type="EtcItem" name="Enchanted Iron Golem Scrap"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="2856" type="EtcItem" name="Tone of Water"><set name="is_questitem" val="true"/></item>
        <item id="2857" type="EtcItem" name="Tone of Fire"><set name="is_questitem" val="true"/></item>
        <item id="2858" type="EtcItem" name="Tone of Wind"><set name="is_questitem" val="true"/></item>
        <item id="2859" type="EtcItem" name="Tone of Earth"><set name="is_questitem" val="true"/></item>
        <item id="2860" type="EtcItem" name="Salamander Charm"><set name="is_questitem" val="true"/></item>
        <item id="2861" type="EtcItem" name="Sylph Charm"><set name="is_questitem" val="true"/></item>
        <item id="2862" type="EtcItem" name="Undine Charm"><set name="is_questitem" val="true"/></item>
        <item id="2863" type="EtcItem" name="Serpent Charm"><set name="is_questitem" val="true"/></item>"#;

    /// The pages of the quest with the links of the real pages.
    fn pages() -> Pages {
        Pages::of(&Q00228TestOfMagus)
            .page("30629-03.htm", &["ACCEPT"])
            .plain(&[
                "30629-01.html",
                "30629-02.html",
                "30629-04.htm",
                "30629-04a.htm",
                "30629-05.html",
                "30629-06.html",
                "30629-07.html",
                "30629-11.html",
                "30629-12.html",
            ])
            .page("30629-08.html", &["30629-09.html"])
            .page("30629-09.html", &["30629-10.html"])
            .plain(&["30629-10.html"])
            .page("30391-01.html", &["30391-02.html"])
            .plain(&["30391-02.html", "30391-03.html", "30391-04.html", "30391-05.html"])
            .page("30409-01.html", &["30409-02.html"])
            .page("30409-02.html", &["30409-03.html"])
            .plain(&["30409-03.html", "30409-04.html", "30409-05.html", "30409-06.html"])
            .plain(&["30411-01.html", "30411-02.html", "30411-03.html", "30411-04.html"])
            .page("30412-01.html", &["30412-02.html"])
            .plain(&["30412-02.html", "30412-03.html", "30412-04.html", "30412-05.html"])
            .plain(&["30413-01.html", "30413-02.html", "30413-03.html", "30413-04.html"])
            .page("30612-01.html", &["30612-02.html"])
            .plain(&["30612-02.html", "30612-03.html", "30612-04.html", "30612-05.html"])
            .npcs(&[
                BARD_RUKAL,
                PARINA,
                EARTH_SNAKE,
                FLAME_SALAMANDER,
                WIND_SYLPH,
                WATER_UNDINE,
                ELDER_CASIAN,
            ])
    }

    struct Npcs {
        rukal: Oid,
        parina: Oid,
        casian: Oid,
    }

    fn setup() -> (ScriptTest, Npcs) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00228TestOfMagus);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages());
        let npcs = Npcs {
            rukal: t.spawn(BARD_RUKAL, 20),
            parina: t.spawn(PARINA, 30),
            casian: t.spawn(ELDER_CASIAN, 40),
        };
        (t, npcs)
    }

    /// Gives the player items by hand (the steps the tests do not walk).
    fn give(t: &mut ScriptTest, items: &[i32], count: i64) {
        t.world.run_as_script(NAME, |ctx| {
            for &item in items {
                ctx.give_items(Oid(1), item, count);
            }
        });
    }

    #[test]
    fn only_the_wizards_of_level_39_are_taken() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 38, race::HUMAN, player_class::WIZARD, vec![]);
        assert!(t.quest_page(1, npcs.rukal).contains("30629-02.html"));
        t.enter_as(2, 40, race::HUMAN, player_class::FIGHTER, vec![]);
        assert!(t.quest_page(2, npcs.rukal).contains("30629-01.html"));
        t.enter_as(3, 39, race::ELF, player_class::ELVEN_WIZARD, vec![]);
        assert!(t.quest_page(3, npcs.rukal).contains("30629-03.htm"));
        t.enter_as(4, 39, race::DARK_ELF, player_class::DARK_WIZARD, vec![]);
        assert!(t.quest_page(4, npcs.rukal).contains("30629-03.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn a_wizard_accepts_and_gets_the_letter_and_the_diamonds() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 39, race::HUMAN, player_class::WIZARD, vec![]);
        t.quest_page(1, npcs.rukal);
        assert!(t.event(1, NAME, "ACCEPT").contains("30629-04a.htm"));
        assert_eq!(t.count(1, RUKALS_LETTER), 1);
        assert_eq!(t.count(1, DIMENSIONAL_DIAMOND), 122);
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started"),
            ]
        );
    }

    #[test]
    fn parina_and_elder_casian_hand_out_the_letter_and_the_lilac_charm() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 39, race::HUMAN, player_class::WIZARD, vec![]);
        t.quest_page(1, npcs.rukal);
        t.event(1, NAME, "ACCEPT");
        t.take_writes();

        assert!(t.quest_page(1, npcs.parina).contains("30391-01.html"));
        assert!(t.event(1, NAME, "30391-02.html").contains("30391-02.html"));
        assert_eq!((t.count(1, RUKALS_LETTER), t.count(1, PARINAS_LETTER)), (0, 1));
        assert!(t.take_writes().contains(&rows::update(NAME, "cond", "2")));

        assert!(t.quest_page(1, npcs.casian).contains("30612-01.html"));
        assert!(t.event(1, NAME, "30612-02.html").contains("30612-02.html"));
        assert_eq!((t.count(1, PARINAS_LETTER), t.count(1, LILAC_CHARM)), (0, 1));
        assert!(t.take_writes().contains(&rows::update(NAME, "cond", "3")));
    }

    #[test]
    fn the_singing_flowers_give_the_three_seeds_and_move_to_cond_4() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 39, race::HUMAN, player_class::WIZARD, vec![]);
        t.quest_page(1, npcs.rukal);
        t.event(1, NAME, "ACCEPT");
        give(&mut t, &[LILAC_CHARM], 1);
        t.take_writes();

        for (flower, seed) in [
            (SINGING_FLOWER_PHANTASM, GOLDEN_SEED_1ST),
            (SINGING_FLOWER_NIGTMATE, GOLDEN_SEED_2ND),
            (SINGING_FLOWER_DARKLING, GOLDEN_SEED_3RD),
        ] {
            let mob = t.spawn(flower, 60);
            t.kill(1, mob);
            t.world.delete_npc(mob);
            assert_eq!(t.count(1, seed), 1);
        }
        // Only the last seed moves the condition (the first two had no pair yet).
        assert!(t.take_writes().contains(&rows::update(NAME, "cond", "4")));
    }

    #[test]
    fn rukal_gives_the_mark_once_the_four_tones_are_in_hand() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 39, race::HUMAN, player_class::WIZARD, vec![]);
        t.quest_page(1, npcs.rukal);
        t.event(1, NAME, "ACCEPT");
        // The score and the four tones are in hand: every element is done (tested above).
        give(
            &mut t,
            &[
                SCORE_OF_ELEMENTS,
                TONE_OF_WATER,
                TONE_OF_FIRE,
                TONE_OF_WIND,
                TONE_OF_EARTH,
            ],
            1,
        );
        t.world.run_as_script(NAME, |ctx| {
            ctx.take_items(Oid(1), RUKALS_LETTER, -1);
        });
        t.take_writes();
        t.take();

        t.talk(1, npcs.rukal);
        t.bypass(1, "Script");
        let out = t.take();
        assert!(pages_to(&out, 1).last().unwrap().contains("30629-12.html"));
        assert!(names_to(&out, 1).contains(&"SOCIAL_ACTION".to_owned()));
        assert_eq!(t.count(1, MARK_OF_MAGUS), 1);
        assert_eq!(t.count(1, 57), 372_154);
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed"),
            ]
        );
        assert!(
            t.quest_page(1, npcs.rukal)
                .contains("This quest has already been completed.")
        );
    }
}
