//! Quest 219: Testimony of Fate (Magister Kaira in Dark Elf Village: the Dark Elf second class
//! change, through Metheus, Ixia, Roa, Norman, Thifiell and Arkenia).
// Java: dist/game/data/scripts/quests/Q00219_TestimonyOfFate/Q00219_TestimonyOfFate.java
//
// Pattern: the test chain of the "Testimony of ..." quests (see Q00222 for the shape), with a
// race and category gate at the start and a spawned NPC (Alder's Spirit) that stays 200 s.

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, STARTED, race};
use l2_world::subclass::CategoryType;
use l2_world::{Qs, QuestSound, Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const MAGISTER_ROA: i32 = 30114;
const WAREHOUSE_KEEPER_NORMAN: i32 = 30210;
const TETRARCH_THIFIELL: i32 = 30358;
const ARKENIA: i32 = 30419;
const MASTER_IXIA: i32 = 30463;
const MAGISTER_KAIRA: i32 = 30476;
const ALDERS_SPIRIT: i32 = 30613;
const BROTHER_METHEUS: i32 = 30614;
const BLOODY_PIXY: i32 = 31845;
const BLIGHT_TREANT: i32 = 31850;

// Items
const KAIRAS_LETTER: i32 = 3173;
const METHEUSS_FUNERAL_JAR: i32 = 3174;
const KASANDRAS_REMAINS: i32 = 3175;
const HERBALISM_TEXTBOOK: i32 = 3176;
const IXIAS_LIST: i32 = 3177;
const MEDUSAS_ICHOR: i32 = 3178;
const MARSH_SPIDER_FLUIDS: i32 = 3179;
const DEAD_SEEKER_DUNG: i32 = 3180;
const TYRANTS_BLOOD: i32 = 3181;
const NIGHTSHADE_ROOT: i32 = 3182;
const BELLADONNA: i32 = 3183;
const ALDERS_SKULL1: i32 = 3184;
const ALDERS_SKULL2: i32 = 3185;
const ALDERS_RECEIPT: i32 = 3186;
const REVELATIONS_MANUSCRIPT: i32 = 3187;
const KAIRAS_RECOMMENDATION: i32 = 3189;
const KAIRAS_INSTRUCTIONS: i32 = 3188;
const PALUS_CHARM: i32 = 3190;
const THIFIELLS_LETTER: i32 = 3191;
const ARKENIAS_NOTE: i32 = 3192;
const PIXY_GARNET: i32 = 3193;
const GRANDISS_SKULL: i32 = 3194;
const KARUL_BUGBEAR_SKULL: i32 = 3195;
const BREKA_OVERLORD_SKULL: i32 = 3196;
const LETO_OVERLORD_SKULL: i32 = 3197;
const RED_FAIRY_DUST: i32 = 3198;
const TIMIRIRAN_SEED: i32 = 3199;
const BLACK_WILLOW_LEAF: i32 = 3200;
const BLIGHT_TREANT_SAP: i32 = 3201;
const ARKENIAS_LETTER: i32 = 3202;

// Reward
const MARK_OF_FATE: i32 = 3172;
const DIMENSIONAL_DIAMOND: i32 = 7562;

// Monster
const HANGMAN_TREE: i32 = 20144;
const MARSH_STAKATO: i32 = 20157;
const MEDUSA: i32 = 20158;
const TYRANT: i32 = 20192;
const TYRANT_KINGPIN: i32 = 20193;
const DEAD_SEEKER: i32 = 20202;
const MARSH_STAKATO_WORKER: i32 = 20230;
const MARSH_STAKATO_SOLDIER: i32 = 20232;
const MARSH_SPIDER: i32 = 20233;
const MARSH_STAKATO_DRONE: i32 = 20234;
const BREKA_ORC_OVERLORD: i32 = 20270;
const GRANDIS: i32 = 20554;
const LETO_LIZARDMAN_OVERLORD: i32 = 20582;
const KARUL_BUGBEAR: i32 = 20600;

// Quest Monster
const BLACK_WILLOW_LURKER: i32 = 27079;

// Misc
const MIN_LEVEL: i32 = 37;

/// The five items Ixia's list asks for (10 of each).
const IXIA_ITEMS: [i32; 5] = [
    MEDUSAS_ICHOR,
    MARSH_SPIDER_FLUIDS,
    DEAD_SEEKER_DUNG,
    TYRANTS_BLOOD,
    NIGHTSHADE_ROOT,
];

/// Java `Q00219_TestimonyOfFate`.
pub struct Q00219TestimonyOfFate;

impl Script for Q00219TestimonyOfFate {
    fn name(&self) -> &'static str {
        "Q00219_TestimonyOfFate"
    }

    fn path(&self) -> &'static str {
        "quests/Q00219_TestimonyOfFate"
    }

    fn id(&self) -> i32 {
        219
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[MAGISTER_KAIRA]);
        s.add_talk_id(&[
            MAGISTER_KAIRA,
            MAGISTER_ROA,
            WAREHOUSE_KEEPER_NORMAN,
            TETRARCH_THIFIELL,
            ARKENIA,
            MASTER_IXIA,
            ALDERS_SPIRIT,
            BROTHER_METHEUS,
            BLOODY_PIXY,
            BLIGHT_TREANT,
        ]);
        s.add_kill_id(&[
            HANGMAN_TREE,
            MARSH_STAKATO,
            MEDUSA,
            TYRANT,
            TYRANT_KINGPIN,
            DEAD_SEEKER,
            MARSH_STAKATO_WORKER,
            MARSH_STAKATO_SOLDIER,
            MARSH_SPIDER,
            MARSH_STAKATO_DRONE,
            BREKA_ORC_OVERLORD,
            GRANDIS,
            LETO_LIZARDMAN_OVERLORD,
            KARUL_BUGBEAR,
            BLACK_WILLOW_LURKER,
        ]);
        s.register_quest_items(&[
            KAIRAS_LETTER,
            METHEUSS_FUNERAL_JAR,
            KASANDRAS_REMAINS,
            HERBALISM_TEXTBOOK,
            IXIAS_LIST,
            MEDUSAS_ICHOR,
            MARSH_SPIDER_FLUIDS,
            DEAD_SEEKER_DUNG,
            TYRANTS_BLOOD,
            NIGHTSHADE_ROOT,
            BELLADONNA,
            ALDERS_SKULL1,
            ALDERS_SKULL2,
            ALDERS_RECEIPT,
            REVELATIONS_MANUSCRIPT,
            KAIRAS_RECOMMENDATION,
            KAIRAS_INSTRUCTIONS,
            PALUS_CHARM,
            THIFIELLS_LETTER,
            ARKENIAS_NOTE,
            PIXY_GARNET,
            GRANDISS_SKULL,
            KARUL_BUGBEAR_SKULL,
            BREKA_OVERLORD_SKULL,
            LETO_OVERLORD_SKULL,
            RED_FAIRY_DUST,
            TIMIRIRAN_SEED,
            BLACK_WILLOW_LEAF,
            BLIGHT_TREANT_SAP,
            ARKENIAS_LETTER,
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
                    ctx.give_items(player, KAIRAS_LETTER, 1);
                    if ctx.player_variable_int(player, "2ND_CLASS_DIAMOND_REWARD", 0) == 0 {
                        ctx.give_items(player, DIMENSIONAL_DIAMOND, 98);
                        ctx.set_player_variable(player, "2ND_CLASS_DIAMOND_REWARD", "1");
                        htmltext = Some("30476-05a.htm".to_owned());
                    } else {
                        htmltext = Some("30476-05.htm".to_owned());
                    }
                }
            }
            "30476-04.htm" | "30476-13.html" | "30476-14.html" | "30114-02.html" | "30114-03.html"
            | "30463-02a.html" => {
                htmltext = Some(event.to_owned());
            }
            "30476-12.html" => {
                if ctx.has_quest_items(player, REVELATIONS_MANUSCRIPT) {
                    ctx.take_items(player, REVELATIONS_MANUSCRIPT, 1);
                    ctx.give_items(player, KAIRAS_RECOMMENDATION, 1);
                    qs.set_cond(ctx, 15, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "30114-04.html" => {
                if ctx.has_quest_items(player, ALDERS_SKULL2) {
                    ctx.take_items(player, ALDERS_SKULL2, 1);
                    ctx.give_items(player, ALDERS_RECEIPT, 1);
                    qs.set_cond(ctx, 12, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "30419-02.html" => {
                if ctx.has_quest_items(player, THIFIELLS_LETTER) {
                    ctx.take_items(player, THIFIELLS_LETTER, 1);
                    ctx.give_items(player, ARKENIAS_NOTE, 1);
                    qs.set_cond(ctx, 17, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "30419-05.html" => {
                if ctx.has_all_quest_items(player, &[ARKENIAS_NOTE, RED_FAIRY_DUST, BLIGHT_TREANT_SAP]) {
                    ctx.take_items(player, ARKENIAS_NOTE, 1);
                    ctx.take_items(player, RED_FAIRY_DUST, 1);
                    ctx.take_items(player, BLIGHT_TREANT_SAP, 1);
                    ctx.give_items(player, ARKENIAS_LETTER, 1);
                    qs.set_cond(ctx, 18, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "31845-02.html" => {
                ctx.give_items(player, PIXY_GARNET, 1);
                htmltext = Some(event.to_owned());
            }
            "31850-02.html" => {
                ctx.give_items(player, TIMIRIRAN_SEED, 1);
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
            HANGMAN_TREE => {
                if ctx.has_quest_items(killer, METHEUSS_FUNERAL_JAR)
                    && !ctx.has_quest_items(killer, KASANDRAS_REMAINS)
                {
                    ctx.take_items(killer, METHEUSS_FUNERAL_JAR, 1);
                    ctx.give_items(killer, KASANDRAS_REMAINS, 1);
                    qs.set_cond(ctx, 3, true);
                }
            }
            MARSH_STAKATO | MARSH_STAKATO_WORKER | MARSH_STAKATO_SOLDIER | MARSH_STAKATO_DRONE => {
                ixia_item(ctx, qs, killer, NIGHTSHADE_ROOT);
            }
            MEDUSA => ixia_item(ctx, qs, killer, MEDUSAS_ICHOR),
            TYRANT | TYRANT_KINGPIN => ixia_item(ctx, qs, killer, TYRANTS_BLOOD),
            DEAD_SEEKER => ixia_item(ctx, qs, killer, DEAD_SEEKER_DUNG),
            MARSH_SPIDER => ixia_item(ctx, qs, killer, MARSH_SPIDER_FLUIDS),
            BREKA_ORC_OVERLORD => skull(ctx, killer, BREKA_OVERLORD_SKULL),
            GRANDIS => skull(ctx, killer, GRANDISS_SKULL),
            LETO_LIZARDMAN_OVERLORD => skull(ctx, killer, LETO_OVERLORD_SKULL),
            KARUL_BUGBEAR => skull(ctx, killer, KARUL_BUGBEAR_SKULL),
            BLACK_WILLOW_LURKER => {
                if ctx.has_all_quest_items(killer, &[PALUS_CHARM, ARKENIAS_NOTE, TIMIRIRAN_SEED])
                    && !ctx.has_all_quest_items(killer, &[BLIGHT_TREANT_SAP, BLACK_WILLOW_LEAF])
                {
                    ctx.give_items(killer, BLACK_WILLOW_LEAF, 1);
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
        if state == CREATED {
            if npc.id == MAGISTER_KAIRA {
                if ctx.player_race(player) == race::DARK_ELF {
                    let level = ctx.player_level(player);
                    if level >= MIN_LEVEL && ctx.world().is_in_category(player, CategoryType::Delf2ndGroup) {
                        htmltext = "30476-03.htm".to_owned();
                    } else if level >= MIN_LEVEL {
                        htmltext = "30476-01a.html".to_owned();
                    } else {
                        htmltext = "30476-02.html".to_owned();
                    }
                } else {
                    htmltext = "30476-01.html".to_owned();
                }
            }
        } else if state == STARTED {
            match npc.id {
                MAGISTER_KAIRA => {
                    if ctx.has_quest_items(player, KAIRAS_LETTER) {
                        htmltext = "30476-06.html".to_owned();
                    } else if ctx
                        .has_at_least_one_quest_item(player, &[METHEUSS_FUNERAL_JAR, KASANDRAS_REMAINS])
                    {
                        htmltext = "30476-07.html".to_owned();
                    } else if ctx.has_at_least_one_quest_item(player, &[HERBALISM_TEXTBOOK, IXIAS_LIST]) {
                        qs.set_cond(ctx, 5, true);
                        htmltext = "30476-08.html".to_owned();
                    } else if ctx.has_quest_items(player, ALDERS_SKULL1) {
                        ctx.take_items(player, ALDERS_SKULL1, 1);
                        ctx.give_items(player, ALDERS_SKULL2, 1);
                        ctx.add_spawn(ALDERS_SPIRIT, 78977, 149036, -3597, 0, false, 200_000, 0);
                        qs.set_cond(ctx, 10, true);
                        htmltext = "30476-09.html".to_owned();
                    } else if ctx.has_at_least_one_quest_item(player, &[ALDERS_SKULL2, ALDERS_RECEIPT]) {
                        qs.set_cond(ctx, 11, true);
                        htmltext = "30476-10.html".to_owned();
                    } else if ctx.has_quest_items(player, REVELATIONS_MANUSCRIPT) {
                        htmltext = "30476-11.html".to_owned();
                    } else if ctx.has_quest_items(player, KAIRAS_INSTRUCTIONS) {
                        ctx.give_items(player, KAIRAS_RECOMMENDATION, 1);
                        ctx.take_items(player, KAIRAS_INSTRUCTIONS, 1);
                        qs.set_cond(ctx, 15, true);
                        htmltext = "30476-15.html".to_owned();
                    } else if ctx.has_quest_items(player, KAIRAS_RECOMMENDATION) {
                        htmltext = "30476-16.html".to_owned();
                    } else if ctx.has_quest_items(player, PALUS_CHARM) {
                        htmltext = "30476-17.html".to_owned();
                    }
                }
                BROTHER_METHEUS => {
                    if ctx.has_quest_items(player, KAIRAS_LETTER) {
                        ctx.take_items(player, KAIRAS_LETTER, 1);
                        ctx.give_items(player, METHEUSS_FUNERAL_JAR, 1);
                        qs.set_cond(ctx, 2, true);
                        htmltext = "30614-01.html".to_owned();
                    } else if ctx.has_quest_items(player, METHEUSS_FUNERAL_JAR)
                        && !ctx.has_quest_items(player, KASANDRAS_REMAINS)
                    {
                        htmltext = "30614-02.html".to_owned();
                    } else if ctx.has_quest_items(player, KASANDRAS_REMAINS)
                        && !ctx.has_quest_items(player, METHEUSS_FUNERAL_JAR)
                    {
                        ctx.take_items(player, KASANDRAS_REMAINS, 1);
                        ctx.give_items(player, HERBALISM_TEXTBOOK, 1);
                        qs.set_cond(ctx, 4, true);
                        htmltext = "30614-03.html".to_owned();
                    } else if ctx.has_at_least_one_quest_item(player, &[HERBALISM_TEXTBOOK, IXIAS_LIST]) {
                        qs.set_cond(ctx, 5, true);
                        htmltext = "30614-04.html".to_owned();
                    } else if ctx.has_quest_items(player, BELLADONNA) {
                        ctx.take_items(player, BELLADONNA, 1);
                        ctx.give_items(player, ALDERS_SKULL1, 1);
                        qs.set_cond(ctx, 9, true);
                        htmltext = "30614-05.html".to_owned();
                    } else if ctx.has_at_least_one_quest_item(player, &LATE_ITEMS) {
                        htmltext = "30614-06.html".to_owned();
                    }
                }
                MASTER_IXIA => {
                    if ctx.has_quest_items(player, HERBALISM_TEXTBOOK) {
                        ctx.take_items(player, HERBALISM_TEXTBOOK, 1);
                        ctx.give_items(player, IXIAS_LIST, 1);
                        qs.set_cond(ctx, 6, true);
                        htmltext = "30463-01.html".to_owned();
                    } else if ctx.has_quest_items(player, IXIAS_LIST) {
                        if IXIA_ITEMS
                            .iter()
                            .all(|&i| ctx.get_quest_items_count(player, i) >= 10)
                        {
                            ctx.take_items(player, IXIAS_LIST, 1);
                            ctx.take_items(player, MEDUSAS_ICHOR, -1);
                            ctx.take_items(player, MARSH_SPIDER_FLUIDS, -1);
                            ctx.take_items(player, DEAD_SEEKER_DUNG, -1);
                            ctx.take_items(player, TYRANTS_BLOOD, -1);
                            ctx.take_items(player, NIGHTSHADE_ROOT, -1);
                            ctx.give_items(player, BELLADONNA, 1);
                            qs.set_cond(ctx, 8, true);
                            htmltext = "30463-03.html".to_owned();
                        } else {
                            htmltext = "30463-02.html".to_owned();
                        }
                    } else if ctx.has_quest_items(player, BELLADONNA) {
                        htmltext = "30463-04.html".to_owned();
                    } else if ctx.has_at_least_one_quest_item(player, &LATE_ITEMS) {
                        htmltext = "30463-05.html".to_owned();
                    }
                }
                MAGISTER_ROA => {
                    if ctx.has_quest_items(player, ALDERS_SKULL2) {
                        htmltext = "30114-01.html".to_owned();
                    } else if ctx.has_quest_items(player, ALDERS_RECEIPT) {
                        htmltext = "30114-05.html".to_owned();
                    } else if ctx.has_at_least_one_quest_item(
                        player,
                        &[REVELATIONS_MANUSCRIPT, KAIRAS_INSTRUCTIONS, KAIRAS_RECOMMENDATION],
                    ) {
                        htmltext = "30114-06.html".to_owned();
                    }
                }
                WAREHOUSE_KEEPER_NORMAN => {
                    if ctx.has_quest_items(player, ALDERS_RECEIPT) {
                        ctx.take_items(player, ALDERS_RECEIPT, 1);
                        ctx.give_items(player, REVELATIONS_MANUSCRIPT, 1);
                        qs.set_cond(ctx, 13, true);
                        htmltext = "30210-01.html".to_owned();
                    } else if ctx.has_quest_items(player, REVELATIONS_MANUSCRIPT) {
                        htmltext = "30210-02.html".to_owned();
                    }
                }
                TETRARCH_THIFIELL => {
                    if ctx.has_quest_items(player, KAIRAS_RECOMMENDATION) {
                        ctx.take_items(player, KAIRAS_RECOMMENDATION, 1);
                        ctx.give_items(player, PALUS_CHARM, 1);
                        ctx.give_items(player, THIFIELLS_LETTER, 1);
                        qs.set_cond(ctx, 16, true);
                        htmltext = "30358-01.html".to_owned();
                    } else if ctx.has_quest_items(player, PALUS_CHARM) {
                        if ctx.has_quest_items(player, THIFIELLS_LETTER) {
                            htmltext = "30358-02.html".to_owned();
                        } else if ctx.has_quest_items(player, ARKENIAS_NOTE) {
                            htmltext = "30358-03.html".to_owned();
                        } else if ctx.has_quest_items(player, ARKENIAS_LETTER) {
                            ctx.give_adena(player, 247_708, true);
                            ctx.give_items(player, MARK_OF_FATE, 1);
                            ctx.add_exp_and_sp(player, 1_365_470, 91_124);
                            qs.exit_quest(ctx, false, true);
                            ctx.send_social_action(player, 3);
                            htmltext = "30358-04.html".to_owned();
                        }
                    }
                }
                ARKENIA => {
                    if ctx.has_quest_items(player, PALUS_CHARM) {
                        if ctx.has_quest_items(player, THIFIELLS_LETTER) {
                            htmltext = "30419-01.html".to_owned();
                        } else if ctx.has_quest_items(player, ARKENIAS_NOTE)
                            && !ctx.has_all_quest_items(player, &[RED_FAIRY_DUST, BLIGHT_TREANT_SAP])
                        {
                            htmltext = "30419-03.html".to_owned();
                        } else if ctx
                            .has_all_quest_items(player, &[ARKENIAS_NOTE, RED_FAIRY_DUST, BLIGHT_TREANT_SAP])
                        {
                            htmltext = "30419-04.html".to_owned();
                        } else if ctx.has_quest_items(player, ARKENIAS_LETTER) {
                            htmltext = "30419-06.html".to_owned();
                        }
                    }
                }
                ALDERS_SPIRIT => {
                    if ctx.has_at_least_one_quest_item(player, &[ALDERS_SKULL1, ALDERS_SKULL2]) {
                        htmltext = "30613-01.html".to_owned();
                    }
                }
                BLOODY_PIXY => {
                    if ctx.has_all_quest_items(player, &[PALUS_CHARM, ARKENIAS_NOTE]) {
                        if !ctx.has_at_least_one_quest_item(player, &[RED_FAIRY_DUST, PIXY_GARNET]) {
                            htmltext = "31845-01.html".to_owned();
                        } else if !ctx.has_quest_items(player, RED_FAIRY_DUST)
                            && ctx.has_quest_items(player, PIXY_GARNET)
                            && !ctx.has_at_least_one_quest_item(
                                player,
                                &[
                                    GRANDISS_SKULL,
                                    KARUL_BUGBEAR_SKULL,
                                    BREKA_OVERLORD_SKULL,
                                    LETO_OVERLORD_SKULL,
                                ],
                            )
                        {
                            htmltext = "31845-03.html".to_owned();
                        } else if !ctx.has_quest_items(player, RED_FAIRY_DUST)
                            && ctx.has_all_quest_items(
                                player,
                                &[
                                    PIXY_GARNET,
                                    GRANDISS_SKULL,
                                    KARUL_BUGBEAR_SKULL,
                                    BREKA_OVERLORD_SKULL,
                                    LETO_OVERLORD_SKULL,
                                ],
                            )
                        {
                            ctx.take_items(player, PIXY_GARNET, 1);
                            ctx.take_items(player, GRANDISS_SKULL, 1);
                            ctx.take_items(player, KARUL_BUGBEAR_SKULL, 1);
                            ctx.take_items(player, BREKA_OVERLORD_SKULL, 1);
                            ctx.take_items(player, LETO_OVERLORD_SKULL, 1);
                            ctx.give_items(player, RED_FAIRY_DUST, 1);
                            htmltext = "31845-04.html".to_owned();
                        } else if !ctx.has_quest_items(player, PIXY_GARNET)
                            && ctx.has_all_quest_items(player, &[PALUS_CHARM, ARKENIAS_NOTE, RED_FAIRY_DUST])
                        {
                            htmltext = "31845-05.html".to_owned();
                        }
                    }
                }
                BLIGHT_TREANT => {
                    if ctx.has_all_quest_items(player, &[PALUS_CHARM, ARKENIAS_NOTE]) {
                        if !ctx.has_at_least_one_quest_item(player, &[BLIGHT_TREANT_SAP, TIMIRIRAN_SEED]) {
                            htmltext = "31850-01.html".to_owned();
                        } else if ctx.has_quest_items(player, TIMIRIRAN_SEED)
                            && !ctx
                                .has_at_least_one_quest_item(player, &[BLIGHT_TREANT_SAP, BLACK_WILLOW_LEAF])
                        {
                            htmltext = "31850-03.html".to_owned();
                        } else if ctx.has_all_quest_items(player, &[TIMIRIRAN_SEED, BLACK_WILLOW_LEAF])
                            && !ctx.has_quest_items(player, BLIGHT_TREANT_SAP)
                        {
                            ctx.take_items(player, TIMIRIRAN_SEED, 1);
                            ctx.take_items(player, BLACK_WILLOW_LEAF, 1);
                            ctx.give_items(player, BLIGHT_TREANT_SAP, 1);
                            htmltext = "31850-04.html".to_owned();
                        } else if ctx.has_quest_items(player, BLIGHT_TREANT_SAP)
                            && !ctx.has_quest_items(player, TIMIRIRAN_SEED)
                        {
                            htmltext = "31850-05.html".to_owned();
                        }
                    }
                }
                _ => {}
            }
        } else if state == COMPLETED && npc.id == MAGISTER_KAIRA {
            htmltext = ctx.get_already_completed_msg(player);
        }
        Some(htmltext)
    }
}

/// The items of the later steps (Alder's skulls to Kaira's recommendation): the pages of Metheus
/// and Ixia that say "go on to the others" test for any of them.
const LATE_ITEMS: [i32; 6] = [
    ALDERS_SKULL1,
    ALDERS_SKULL2,
    ALDERS_RECEIPT,
    REVELATIONS_MANUSCRIPT,
    KAIRAS_INSTRUCTIONS,
    KAIRAS_RECOMMENDATION,
];

/// One kill of a monster of Ixia's list: the item up to 10 (the middle sound at the 10th) and
/// cond 7 when the other four are complete as well.
fn ixia_item(ctx: &mut ScriptCtx<'_>, qs: Qs, killer: ObjectId, item: i32) {
    if !(ctx.has_quest_items(killer, IXIAS_LIST) && ctx.get_quest_items_count(killer, item) < 10) {
        return;
    }
    if ctx.get_quest_items_count(killer, item) == 9 {
        ctx.give_items(killer, item, 1);
        ctx.play_sound(killer, QuestSound::ItemsoundQuestMiddle);
        if IXIA_ITEMS
            .iter()
            .filter(|&&other| other != item)
            .all(|&other| ctx.get_quest_items_count(killer, other) >= 10)
        {
            qs.set_cond(ctx, 7, false);
        }
    } else {
        ctx.give_items(killer, item, 1);
        ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
    }
}

/// One of the four overlord skulls for the pixy: with the charm, the note and the garnet.
/// NOTE(java-quirk): Java tests `!hasQuestItems(killer, RED_FAIRY_DUST, SKULL)` (not both) and
/// `!hasQuestItems(killer, SKULL)`; the second implies the first, so the skull is given once even
/// when the player already has the fairy dust.
fn skull(ctx: &mut ScriptCtx<'_>, killer: ObjectId, item: i32) {
    if ctx.has_all_quest_items(killer, &[PALUS_CHARM, ARKENIAS_NOTE, PIXY_GARNET])
        && !ctx.has_all_quest_items(killer, &[RED_FAIRY_DUST, item])
        && !ctx.has_quest_items(killer, item)
    {
        ctx.give_items(killer, item, 1);
        ctx.play_sound(killer, QuestSound::ItemsoundQuestMiddle);
    }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::ScriptRegistry;
    use l2_world::script::{player_class, race};

    use super::*;
    use crate::testing::{Pages, ScriptTest, rows};

    const NAME: &str = "Q00219_TestimonyOfFate";

    const NPCS: &str = r#"
        <npc id="30476" type="Folk" level="70" name="Magister Kaira"/>
        <npc id="30614" type="Folk" level="70" name="Brother Metheus"/>
        <npc id="30358" type="Folk" level="70" name="Tetrarch Thifiell"/>
        <npc id="20158" type="Monster" level="40" name="Medusa"/>
        <npc id="20144" type="Monster" level="40" name="Hangman Tree"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="7562" type="EtcItem" name="Dimensional Diamond"><set name="is_stackable" val="true"/></item>
        <item id="3172" type="EtcItem" name="Mark of Fate"><set name="is_questitem" val="true"/></item>
        <item id="3173" type="EtcItem" name="Kaira's Letter"><set name="is_questitem" val="true"/></item>
        <item id="3174" type="EtcItem" name="Metheus's Funeral Jar"><set name="is_questitem" val="true"/></item>
        <item id="3175" type="EtcItem" name="Kasandra's Remains"><set name="is_questitem" val="true"/></item>
        <item id="3177" type="EtcItem" name="Ixia's List"><set name="is_questitem" val="true"/></item>
        <item id="3178" type="EtcItem" name="Medusa's Ichor"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="3179" type="EtcItem" name="Marsh Spider Fluids"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="3180" type="EtcItem" name="Dead Seeker Dung"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="3181" type="EtcItem" name="Tyrant's Blood"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="3182" type="EtcItem" name="Nightshade Root"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="3189" type="EtcItem" name="Kaira's Recommendation"><set name="is_questitem" val="true"/></item>
        <item id="3190" type="EtcItem" name="Palus Charm"><set name="is_questitem" val="true"/></item>
        <item id="3191" type="EtcItem" name="Thifiell's Letter"><set name="is_questitem" val="true"/></item>
        <item id="3202" type="EtcItem" name="Arkenia's Letter"><set name="is_questitem" val="true"/></item>"#;

    /// The pages of the quest with the links of the real pages.
    fn pages() -> Pages {
        Pages::of(&Q00219TestimonyOfFate)
            .plain(&[
                "30476-01.html",
                "30476-01a.html",
                "30476-02.html",
                "30476-05.htm",
                "30476-05a.htm",
                "30476-06.html",
            ])
            .page("30476-03.htm", &["30476-04.htm"])
            .page("30476-04.htm", &["ACCEPT"])
            .plain(&["30614-01.html", "30614-02.html", "30358-04.html"])
            .npcs(&[MAGISTER_KAIRA, BROTHER_METHEUS, TETRARCH_THIFIELL])
    }

    struct Npcs {
        kaira: Oid,
        metheus: Oid,
        thifiell: Oid,
    }

    fn setup() -> (ScriptTest, Npcs) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00219TestimonyOfFate);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages());
        let npcs = Npcs {
            kaira: t.spawn(MAGISTER_KAIRA, 20),
            metheus: t.spawn(BROTHER_METHEUS, 30),
            thifiell: t.spawn(TETRARCH_THIFIELL, 40),
        };
        (t, npcs)
    }

    /// Started by hand: the category gate (`DELF_2ND_GROUP`) depends on the class tables.
    fn start_by_hand(t: &mut ScriptTest) {
        t.world.run_as_script(NAME, |ctx| {
            if let Some(qs) = ctx.get_quest_state(Oid(1), true) {
                qs.start_quest(ctx);
            }
            ctx.give_items(Oid(1), KAIRAS_LETTER, 1);
        });
    }

    #[test]
    fn the_start_is_gated_by_race_and_level() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 40, race::HUMAN, player_class::FIGHTER, vec![]);
        assert!(t.quest_page(1, npcs.kaira).contains("30476-01.html"));
        t.enter_as(2, 36, race::DARK_ELF, player_class::PALUS_KNIGHT, vec![]);
        assert!(t.quest_page(2, npcs.kaira).contains("30476-02.html"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_accept_gives_the_letter_and_98_diamonds() {
        let (mut t, _npcs) = setup();
        t.enter_as(1, 40, race::DARK_ELF, player_class::PALUS_KNIGHT, vec![]);
        // The page offering ACCEPT needs the DELF_2ND_GROUP category (not in the test world).
        t.world.run_as_script(NAME, |ctx| {
            ctx.get_quest_state(Oid(1), true);
            let out = Q00219TestimonyOfFate.on_event(ctx, "ACCEPT", None, Some(Oid(1)));
            assert_eq!(out.as_deref(), Some("30476-05a.htm"));
        });
        assert_eq!(t.count(1, KAIRAS_LETTER), 1);
        assert_eq!(t.count(1, DIMENSIONAL_DIAMOND), 98);
        assert!(t.take_writes().contains(&rows::insert(NAME, "cond", "1")));
    }

    #[test]
    fn metheus_takes_the_letter_and_the_hangman_gives_the_remains() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 40, race::DARK_ELF, player_class::PALUS_KNIGHT, vec![]);
        start_by_hand(&mut t);
        t.take_writes();

        assert!(t.quest_page(1, npcs.metheus).contains("30614-01.html"));
        assert_eq!(t.count(1, METHEUSS_FUNERAL_JAR), 1);
        assert!(t.take_writes().contains(&rows::update(NAME, "cond", "2")));
        let tree = t.spawn(HANGMAN_TREE, 50);
        t.kill(1, tree);
        t.world.delete_npc(tree);
        assert_eq!(t.count(1, KASANDRAS_REMAINS), 1);
        assert_eq!(t.count(1, METHEUSS_FUNERAL_JAR), 0);
        assert!(t.take_writes().contains(&rows::update(NAME, "cond", "3")));
    }

    #[test]
    fn the_tenth_ichor_is_the_last_one() {
        let (mut t, _npcs) = setup();
        t.enter_as(1, 40, race::DARK_ELF, player_class::PALUS_KNIGHT, vec![]);
        start_by_hand(&mut t);
        t.world.run_as_script(NAME, |ctx| {
            ctx.give_items(Oid(1), IXIAS_LIST, 1);
            for item in [
                MARSH_SPIDER_FLUIDS,
                DEAD_SEEKER_DUNG,
                TYRANTS_BLOOD,
                NIGHTSHADE_ROOT,
            ] {
                ctx.give_items(Oid(1), item, 10);
            }
        });
        t.take_writes();
        for _ in 0..11 {
            let medusa = t.spawn(MEDUSA, 50);
            t.kill(1, medusa);
            t.world.delete_npc(medusa);
        }
        assert_eq!(t.count(1, MEDUSAS_ICHOR), 10);
        // The tenth ichor completes the list: cond 7 (a quiet `setCond(7)`).
        assert!(t.take_writes().contains(&rows::update(NAME, "cond", "7")));
    }

    #[test]
    fn thifiell_pays_the_reward_for_arkenias_letter() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 40, race::DARK_ELF, player_class::PALUS_KNIGHT, vec![]);
        start_by_hand(&mut t);
        t.world.run_as_script(NAME, |ctx| {
            ctx.give_items(Oid(1), PALUS_CHARM, 1);
            ctx.give_items(Oid(1), ARKENIAS_LETTER, 1);
        });
        t.take_writes();
        assert!(t.quest_page(1, npcs.thifiell).contains("30358-04.html"));
        assert_eq!(t.count(1, MARK_OF_FATE), 1);
        assert_eq!(t.count(1, 57), 247_708);
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed"),
            ]
        );
        assert!(
            t.quest_page(1, npcs.kaira)
                .contains("This quest has already been completed.")
        );
    }
}
