//! Quest 229: Test of Witchcraft (Shadow Orim in Dark Elf Village / Gludin: the Wizard, Knight
//! and Palus Knight second class change, through Alexandria, Iker, Kaira, Nestle, Vadin, Sir
//! Klaus Vasper, Leopold and Evert).
// Java: dist/game/data/scripts/quests/Q00229_TestOfWitchcraft/Q00229_TestOfWitchcraft.java
//
// Pattern: the test chain of the "Test of ..." quests (see Q00222) with a boss that is called by
// a quest NPC (`addSpawn(summoner, ..)`, counted by `getSummonedNpcCount`), `onAttack` talk on the
// script value and a kill that counts only with the Sword of Binding (`getKillingBlowWeapon`).

use l2_protocol_game::NpcStringId;
use l2_protocol_game::server::ChatType;
use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, STARTED, player_class};
use l2_world::{QuestSound, Script, ScriptCtx, ScriptNpc, ScriptSetup, SkillKey};

// NPCs
const GROCER_LARA: i32 = 30063;
const TRADER_ALEXANDRIA: i32 = 30098;
const MAGISTER_IKER: i32 = 30110;
const PRIEST_VADIN: i32 = 30188;
const TRADER_NESTLE: i32 = 30314;
const SIR_KLAUS_VASPER: i32 = 30417;
const LEOPOLD: i32 = 30435;
const MAGISTER_KAIRA: i32 = 30476;
const SHADOW_ORIM: i32 = 30630;
const WARDEN_RODERIK: i32 = 30631;
const WARDEN_ENDRIGO: i32 = 30632;
const FISHER_EVERT: i32 = 30633;

// Items
const SWORD_OF_BINDING: i32 = 3029;
const ORIMS_DIAGRAM: i32 = 3308;
const ALEXANDRIAS_BOOK: i32 = 3309;
const IKERS_LIST: i32 = 3310;
const DIRE_WYRM_FANG: i32 = 3311;
const LETO_LIZARDMAN_CHARM: i32 = 3312;
const ENCHANTED_STONE_GOLEM_HEARTSTONE: i32 = 3313;
const LARAS_MEMO: i32 = 3314;
const NESTLES_MEMO: i32 = 3315;
const LEOPOLDS_JOURNAL: i32 = 3316;
const AKLANTOTH_1ST_GEM: i32 = 3317;
const AKLANTOTH_2ND_GEM: i32 = 3318;
const AKLANTOTH_3RD_GEM: i32 = 3319;
const AKLANTOTH_4TH_GEM: i32 = 3320;
const AKLANTOTH_5TH_GEM: i32 = 3321;
const AKLANTOTH_6TH_GEM: i32 = 3322;
const BRIMSTONE_1ST: i32 = 3323;
const ORIMS_INSTRUCTIONS: i32 = 3324;
const ORIMS_1ST_LETTER: i32 = 3325;
const ORIMS_2ND_LETTER: i32 = 3326;
const SIR_VASPERS_LETTER: i32 = 3327;
const VADINS_CRUCIFIX: i32 = 3328;
const TAMLIN_ORC_AMULET: i32 = 3329;
const VADINS_SANCTIONS: i32 = 3330;
const IKERS_AMULET: i32 = 3331;
const SOULTRAP_CRYSTAL: i32 = 3332;
const PURGATORY_KEY: i32 = 3333;
const ZERUEL_BIND_CRYSTAL: i32 = 3334;
const BRIMSTONE_2ND: i32 = 3335;

// Reward
const MARK_OF_WITCHCRAFT: i32 = 3307;
const DIMENSIONAL_DIAMOND: i32 = 7562;

// Monster
const DIRE_WYRM: i32 = 20557;
const ENCHANTED_STONE_GOLEM: i32 = 20565;
const LETO_LIZARDMAN: i32 = 20577;
const LETO_LIZARDMAN_ARCHER: i32 = 20578;
const LETO_LIZARDMAN_SOLDIER: i32 = 20579;
const LETO_LIZARDMAN_WARRIOR: i32 = 20580;
const LETO_LIZARDMAN_SHAMAN: i32 = 20581;
const LETO_LIZARDMAN_OVERLORD: i32 = 20582;
const TAMLIN_ORC: i32 = 20601;
const TAMLIN_ORC_ARCHER: i32 = 20602;

// Quest Monster
const NAMELESS_REVENANT: i32 = 27099;
const SKELETAL_MERCENARY: i32 = 27100;
const DREVANUL_PRINCE_ZERUEL: i32 = 27101;

// Misc
const MIN_LEVEL: i32 = 39;

// Locations
const DREVANUL_PRINCE_ZERUEL_SPAWN: (i32, i32, i32) = (13395, 169807, -3708);

/// Java `Q00229_TestOfWitchcraft`.
pub struct Q00229TestOfWitchcraft;

impl Script for Q00229TestOfWitchcraft {
    fn name(&self) -> &'static str {
        "Q00229_TestOfWitchcraft"
    }

    fn path(&self) -> &'static str {
        "quests/Q00229_TestOfWitchcraft"
    }

    fn id(&self) -> i32 {
        229
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[SHADOW_ORIM]);
        s.add_talk_id(&[
            SHADOW_ORIM,
            GROCER_LARA,
            TRADER_ALEXANDRIA,
            MAGISTER_IKER,
            PRIEST_VADIN,
            TRADER_NESTLE,
            SIR_KLAUS_VASPER,
            LEOPOLD,
            MAGISTER_KAIRA,
            WARDEN_RODERIK,
            WARDEN_ENDRIGO,
            FISHER_EVERT,
        ]);
        s.add_kill_id(&[
            DIRE_WYRM,
            ENCHANTED_STONE_GOLEM,
            LETO_LIZARDMAN,
            LETO_LIZARDMAN_ARCHER,
            LETO_LIZARDMAN_SOLDIER,
            LETO_LIZARDMAN_WARRIOR,
            LETO_LIZARDMAN_SHAMAN,
            LETO_LIZARDMAN_OVERLORD,
            TAMLIN_ORC,
            TAMLIN_ORC_ARCHER,
            NAMELESS_REVENANT,
            SKELETAL_MERCENARY,
            DREVANUL_PRINCE_ZERUEL,
        ]);
        s.add_attack_id(&[NAMELESS_REVENANT, SKELETAL_MERCENARY, DREVANUL_PRINCE_ZERUEL]);
        s.register_quest_items(&[
            SWORD_OF_BINDING,
            ORIMS_DIAGRAM,
            ALEXANDRIAS_BOOK,
            IKERS_LIST,
            DIRE_WYRM_FANG,
            LETO_LIZARDMAN_CHARM,
            ENCHANTED_STONE_GOLEM_HEARTSTONE,
            LARAS_MEMO,
            NESTLES_MEMO,
            LEOPOLDS_JOURNAL,
            AKLANTOTH_1ST_GEM,
            AKLANTOTH_2ND_GEM,
            AKLANTOTH_3RD_GEM,
            AKLANTOTH_4TH_GEM,
            AKLANTOTH_5TH_GEM,
            AKLANTOTH_6TH_GEM,
            BRIMSTONE_1ST,
            ORIMS_INSTRUCTIONS,
            ORIMS_1ST_LETTER,
            ORIMS_2ND_LETTER,
            SIR_VASPERS_LETTER,
            VADINS_CRUCIFIX,
            TAMLIN_ORC_AMULET,
            VADINS_SANCTIONS,
            IKERS_AMULET,
            SOULTRAP_CRYSTAL,
            PURGATORY_KEY,
            ZERUEL_BIND_CRYSTAL,
            BRIMSTONE_2ND,
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
                    ctx.play_sound(player, QuestSound::ItemsoundQuestMiddle);
                    ctx.give_items(player, ORIMS_DIAGRAM, 1);
                    if ctx.player_variable_int(player, "2ND_CLASS_DIAMOND_REWARD", 0) == 0 {
                        if ctx.player_class_id(player) == player_class::WIZARD {
                            ctx.give_items(player, DIMENSIONAL_DIAMOND, 122);
                        } else {
                            ctx.give_items(player, DIMENSIONAL_DIAMOND, 104);
                        }
                        ctx.set_player_variable(player, "2ND_CLASS_DIAMOND_REWARD", "1");
                        htmltext = Some("30630-08a.htm".to_owned());
                    } else {
                        htmltext = Some("30630-08.htm".to_owned());
                    }
                }
            }
            "30630-04.htm" | "30630-06.htm" | "30630-07.htm" | "30630-12.htm" | "30630-13.htm"
            | "30630-20.htm" | "30630-21.htm" | "30098-02.htm" | "30110-02.htm" | "30417-02.htm" => {
                htmltext = Some(event.to_owned());
            }
            "30630-14.htm" => {
                if ctx.has_quest_items(player, ALEXANDRIAS_BOOK) {
                    ctx.take_items(player, ALEXANDRIAS_BOOK, 1);
                    ctx.take_items(player, AKLANTOTH_1ST_GEM, 1);
                    ctx.take_items(player, AKLANTOTH_2ND_GEM, 1);
                    ctx.take_items(player, AKLANTOTH_3RD_GEM, 1);
                    ctx.take_items(player, AKLANTOTH_4TH_GEM, 1);
                    ctx.take_items(player, AKLANTOTH_5TH_GEM, 1);
                    ctx.take_items(player, AKLANTOTH_6TH_GEM, 1);
                    ctx.give_items(player, BRIMSTONE_1ST, 1);
                    qs.set_cond(ctx, 4, true);
                    ctx.add_spawn_at(DREVANUL_PRINCE_ZERUEL, npc?.object_id, true, 0);
                    htmltext = Some(event.to_owned());
                }
            }
            "30630-16.htm" => {
                if ctx.has_quest_items(player, BRIMSTONE_1ST) {
                    ctx.take_items(player, BRIMSTONE_1ST, 1);
                    ctx.give_items(player, ORIMS_INSTRUCTIONS, 1);
                    ctx.give_items(player, ORIMS_1ST_LETTER, 1);
                    ctx.give_items(player, ORIMS_2ND_LETTER, 1);
                    qs.set_cond(ctx, 6, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "30630-22.htm" => {
                if ctx.has_quest_items(player, ZERUEL_BIND_CRYSTAL) {
                    ctx.give_adena(player, 372_154, true);
                    ctx.give_items(player, MARK_OF_WITCHCRAFT, 1);
                    ctx.add_exp_and_sp(player, 2_058_244, 141_240);
                    qs.exit_quest(ctx, false, true);
                    ctx.send_social_action(player, 3);
                    htmltext = Some(event.to_owned());
                }
            }
            "30063-02.htm" => {
                ctx.give_items(player, LARAS_MEMO, 1);
                htmltext = Some(event.to_owned());
            }
            "30098-03.htm" => {
                if ctx.has_quest_items(player, ORIMS_DIAGRAM) {
                    ctx.take_items(player, ORIMS_DIAGRAM, 1);
                    ctx.give_items(player, ALEXANDRIAS_BOOK, 1);
                    qs.set_cond(ctx, 2, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "30110-03.htm" => {
                ctx.give_items(player, IKERS_LIST, 1);
                htmltext = Some(event.to_owned());
            }
            "30110-08.htm" => {
                ctx.take_items(player, ORIMS_2ND_LETTER, 1);
                ctx.give_items(player, IKERS_AMULET, 1);
                ctx.give_items(player, SOULTRAP_CRYSTAL, 1);
                if ctx.has_quest_items(player, SWORD_OF_BINDING) {
                    qs.set_cond(ctx, 7, true);
                }
                htmltext = Some(event.to_owned());
            }
            "30314-02.htm" => {
                ctx.give_items(player, NESTLES_MEMO, 1);
                htmltext = Some(event.to_owned());
            }
            "30417-03.htm" => {
                if ctx.has_quest_items(player, ORIMS_1ST_LETTER) {
                    ctx.take_items(player, ORIMS_1ST_LETTER, 1);
                    ctx.give_items(player, SIR_VASPERS_LETTER, 1);
                    htmltext = Some(event.to_owned());
                }
            }
            "30435-02.htm" => {
                if ctx.has_quest_items(player, NESTLES_MEMO) {
                    ctx.take_items(player, NESTLES_MEMO, 1);
                    ctx.give_items(player, LEOPOLDS_JOURNAL, 1);
                    htmltext = Some(event.to_owned());
                }
            }
            "30476-02.htm" => {
                ctx.give_items(player, AKLANTOTH_2ND_GEM, 1);
                if ctx.has_all_quest_items(
                    player,
                    &[
                        AKLANTOTH_1ST_GEM,
                        AKLANTOTH_3RD_GEM,
                        AKLANTOTH_4TH_GEM,
                        AKLANTOTH_5TH_GEM,
                        AKLANTOTH_6TH_GEM,
                    ],
                ) {
                    qs.set_cond(ctx, 3, true);
                }
                htmltext = Some(event.to_owned());
            }
            "30633-02.htm" => {
                ctx.give_items(player, BRIMSTONE_2ND, 1);
                qs.set_cond(ctx, 9, true);
                let evert = npc?;
                if ctx.summoned_npc_count(evert) < 1 {
                    call_zeruel(ctx, evert);
                }
                htmltext = Some(event.to_owned());
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
        match npc.id {
            NAMELESS_REVENANT => {
                if ctx.is_script_value(npc, 0)
                    && ctx.has_all_quest_items(attacker, &[ALEXANDRIAS_BOOK, LARAS_MEMO])
                    && !ctx.has_quest_items(attacker, AKLANTOTH_3RD_GEM)
                {
                    ctx.set_script_value(npc, 1);
                    ctx.broadcast_npc_say(
                        npc,
                        ChatType::NpcGeneral,
                        NpcStringId::IAbsolutelyCannotGiveItToYouItIsMyPreciousJewel,
                        &[],
                    );
                }
            }
            SKELETAL_MERCENARY => {
                // NOTE(java-quirk): `!hasQuestItems(.., 4th, 5th, 6th)` is "not all three gems",
                // so the monster also talks for a player that has one or two of them.
                if ctx.is_script_value(npc, 0)
                    && ctx.has_quest_items(attacker, LEOPOLDS_JOURNAL)
                    && !ctx.has_all_quest_items(
                        attacker,
                        &[AKLANTOTH_4TH_GEM, AKLANTOTH_5TH_GEM, AKLANTOTH_6TH_GEM],
                    )
                {
                    ctx.set_script_value(npc, 1);
                    ctx.broadcast_npc_say(
                        npc,
                        ChatType::NpcGeneral,
                        NpcStringId::IAbsolutelyCannotGiveItToYouItIsMyPreciousJewel,
                        &[],
                    );
                }
            }
            DREVANUL_PRINCE_ZERUEL => {
                if ctx.has_quest_items(attacker, BRIMSTONE_1ST) {
                    ctx.broadcast_npc_say(npc, ChatType::NpcGeneral, NpcStringId::ILlTakeYourLivesLater, &[]);
                    ctx.delete_npc(npc);
                    qs.set_cond(ctx, 5, true);
                } else if ctx.has_all_quest_items(
                    attacker,
                    &[
                        ORIMS_INSTRUCTIONS,
                        BRIMSTONE_2ND,
                        SWORD_OF_BINDING,
                        SOULTRAP_CRYSTAL,
                    ],
                ) && ctx.is_script_value(npc, 0)
                    && check_weapon(ctx, attacker)
                {
                    ctx.set_script_value(npc, 1);
                    ctx.broadcast_npc_say(npc, ChatType::NpcGeneral, NpcStringId::ThatSwordIsReally, &[]);
                }
            }
            _ => {}
        }
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
            DIRE_WYRM => collect(ctx, killer, DIRE_WYRM_FANG),
            ENCHANTED_STONE_GOLEM => collect(ctx, killer, ENCHANTED_STONE_GOLEM_HEARTSTONE),
            LETO_LIZARDMAN
            | LETO_LIZARDMAN_ARCHER
            | LETO_LIZARDMAN_SOLDIER
            | LETO_LIZARDMAN_WARRIOR
            | LETO_LIZARDMAN_SHAMAN
            | LETO_LIZARDMAN_OVERLORD => collect(ctx, killer, LETO_LIZARDMAN_CHARM),
            TAMLIN_ORC | TAMLIN_ORC_ARCHER => {
                if ctx.has_quest_items(killer, VADINS_CRUCIFIX)
                    && ctx.get_random(100) < 50
                    && ctx.get_quest_items_count(killer, TAMLIN_ORC_AMULET) < 20
                {
                    ctx.give_items(killer, TAMLIN_ORC_AMULET, 1);
                    if ctx.get_quest_items_count(killer, TAMLIN_ORC_AMULET) >= 20 {
                        ctx.play_sound(killer, QuestSound::ItemsoundQuestMiddle);
                    } else {
                        ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
                    }
                }
            }
            NAMELESS_REVENANT => {
                if ctx.has_all_quest_items(killer, &[ALEXANDRIAS_BOOK, LARAS_MEMO])
                    && !ctx.has_quest_items(killer, AKLANTOTH_3RD_GEM)
                {
                    ctx.take_items(killer, LARAS_MEMO, 1);
                    ctx.give_items(killer, AKLANTOTH_3RD_GEM, 1);
                    ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
                    if ctx.has_all_quest_items(
                        killer,
                        &[
                            AKLANTOTH_1ST_GEM,
                            AKLANTOTH_2ND_GEM,
                            AKLANTOTH_4TH_GEM,
                            AKLANTOTH_5TH_GEM,
                            AKLANTOTH_6TH_GEM,
                        ],
                    ) {
                        qs.set_cond(ctx, 3, false);
                    }
                }
            }
            SKELETAL_MERCENARY => {
                if ctx.has_quest_items(killer, LEOPOLDS_JOURNAL)
                    && !ctx.has_all_quest_items(
                        killer,
                        &[AKLANTOTH_4TH_GEM, AKLANTOTH_5TH_GEM, AKLANTOTH_6TH_GEM],
                    )
                {
                    if !ctx.has_quest_items(killer, AKLANTOTH_4TH_GEM) {
                        ctx.give_items(killer, AKLANTOTH_4TH_GEM, 1);
                        ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
                    } else if !ctx.has_quest_items(killer, AKLANTOTH_5TH_GEM) {
                        ctx.give_items(killer, AKLANTOTH_5TH_GEM, 1);
                        ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
                    } else if !ctx.has_quest_items(killer, AKLANTOTH_6TH_GEM) {
                        ctx.take_items(killer, LEOPOLDS_JOURNAL, 1);
                        ctx.give_items(killer, AKLANTOTH_6TH_GEM, 1);
                        ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
                        if ctx.has_all_quest_items(
                            killer,
                            &[AKLANTOTH_1ST_GEM, AKLANTOTH_2ND_GEM, AKLANTOTH_3RD_GEM],
                        ) {
                            qs.set_cond(ctx, 3, false);
                        }
                    }
                }
            }
            DREVANUL_PRINCE_ZERUEL => {
                if ctx.has_all_quest_items(
                    killer,
                    &[
                        ORIMS_INSTRUCTIONS,
                        BRIMSTONE_2ND,
                        SWORD_OF_BINDING,
                        SOULTRAP_CRYSTAL,
                    ],
                ) && ctx.killing_blow_weapon(npc) == SWORD_OF_BINDING
                {
                    ctx.broadcast_npc_say(
                        npc,
                        ChatType::NpcGeneral,
                        NpcStringId::NoIHavenTCompletelyFinishedTheCommandForDestructionAndSlaughterYet,
                        &[],
                    );
                    ctx.take_items(killer, SOULTRAP_CRYSTAL, 1);
                    ctx.give_items(killer, PURGATORY_KEY, 1);
                    ctx.give_items(killer, ZERUEL_BIND_CRYSTAL, 1);
                    ctx.take_items(killer, BRIMSTONE_2ND, 1);
                    ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
                    qs.set_cond(ctx, 10, false);
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
            if npc.id == SHADOW_ORIM {
                let class = ctx.player_class_id(player);
                if class == player_class::WIZARD
                    || class == player_class::KNIGHT
                    || class == player_class::PALUS_KNIGHT
                {
                    if ctx.player_level(player) >= MIN_LEVEL {
                        if class == player_class::WIZARD {
                            htmltext = "30630-03.htm".to_owned();
                        } else {
                            htmltext = "30630-05.htm".to_owned();
                        }
                    } else {
                        htmltext = "30630-02.htm".to_owned();
                    }
                } else {
                    htmltext = "30630-01.htm".to_owned();
                }
            }
        } else if state == STARTED {
            match npc.id {
                SHADOW_ORIM => {
                    if ctx.has_quest_items(player, ORIMS_DIAGRAM) {
                        htmltext = "30630-09.htm".to_owned();
                    } else if ctx.has_quest_items(player, ALEXANDRIAS_BOOK) {
                        if ctx.has_all_quest_items(
                            player,
                            &[
                                AKLANTOTH_1ST_GEM,
                                AKLANTOTH_2ND_GEM,
                                AKLANTOTH_3RD_GEM,
                                AKLANTOTH_4TH_GEM,
                                AKLANTOTH_5TH_GEM,
                                AKLANTOTH_6TH_GEM,
                            ],
                        ) {
                            htmltext = "30630-11.htm".to_owned();
                        } else {
                            htmltext = "30630-10.htm".to_owned();
                        }
                    } else if ctx.has_quest_items(player, BRIMSTONE_1ST) {
                        htmltext = "30630-15.htm".to_owned();
                    } else if ctx.has_quest_items(player, ORIMS_INSTRUCTIONS)
                        && !ctx.has_at_least_one_quest_item(player, &[SWORD_OF_BINDING, SOULTRAP_CRYSTAL])
                    {
                        htmltext = "30630-17.htm".to_owned();
                    }

                    if ctx.has_all_quest_items(player, &[SWORD_OF_BINDING, SOULTRAP_CRYSTAL]) {
                        qs.set_cond(ctx, 8, true);
                        htmltext = "30630-18.htm".to_owned();
                    } else if ctx.has_all_quest_items(player, &[SWORD_OF_BINDING, ZERUEL_BIND_CRYSTAL]) {
                        htmltext = "30630-19.htm".to_owned();
                    }
                }
                GROCER_LARA => {
                    if ctx.has_quest_items(player, ALEXANDRIAS_BOOK) {
                        if !ctx.has_at_least_one_quest_item(player, &[LARAS_MEMO, AKLANTOTH_3RD_GEM]) {
                            htmltext = "30063-01.htm".to_owned();
                        } else if !ctx.has_quest_items(player, AKLANTOTH_3RD_GEM)
                            && ctx.has_quest_items(player, LARAS_MEMO)
                        {
                            htmltext = "30063-03.htm".to_owned();
                        } else if !ctx.has_quest_items(player, LARAS_MEMO)
                            && ctx.has_quest_items(player, AKLANTOTH_3RD_GEM)
                        {
                            htmltext = "30063-04.htm".to_owned();
                        }
                    } else if ctx.has_at_least_one_quest_item(player, &[BRIMSTONE_1ST, ORIMS_INSTRUCTIONS]) {
                        htmltext = "30063-05.htm".to_owned();
                    }
                }
                TRADER_ALEXANDRIA => {
                    if ctx.has_quest_items(player, ORIMS_DIAGRAM) {
                        htmltext = "30098-01.htm".to_owned();
                    } else if ctx.has_quest_items(player, ALEXANDRIAS_BOOK) {
                        htmltext = "30098-04.htm".to_owned();
                    } else if ctx.has_all_quest_items(player, &[ORIMS_INSTRUCTIONS, BRIMSTONE_1ST]) {
                        htmltext = "30098-05.htm".to_owned();
                    }
                }
                MAGISTER_IKER => {
                    if ctx.has_quest_items(player, ALEXANDRIAS_BOOK) {
                        if !ctx.has_at_least_one_quest_item(player, &[IKERS_LIST, AKLANTOTH_1ST_GEM]) {
                            htmltext = "30110-01.htm".to_owned();
                        } else if ctx.has_quest_items(player, IKERS_LIST) {
                            if ctx.get_quest_items_count(player, DIRE_WYRM_FANG) >= 20
                                && ctx.get_quest_items_count(player, LETO_LIZARDMAN_CHARM) >= 20
                                && ctx.get_quest_items_count(player, ENCHANTED_STONE_GOLEM_HEARTSTONE) >= 20
                            {
                                ctx.take_items(player, IKERS_LIST, 1);
                                ctx.take_items(player, DIRE_WYRM_FANG, -1);
                                ctx.take_items(player, LETO_LIZARDMAN_CHARM, -1);
                                ctx.take_items(player, ENCHANTED_STONE_GOLEM_HEARTSTONE, -1);
                                ctx.give_items(player, AKLANTOTH_1ST_GEM, 1);
                                if ctx.has_all_quest_items(
                                    player,
                                    &[
                                        AKLANTOTH_2ND_GEM,
                                        AKLANTOTH_3RD_GEM,
                                        AKLANTOTH_4TH_GEM,
                                        AKLANTOTH_5TH_GEM,
                                        AKLANTOTH_6TH_GEM,
                                    ],
                                ) {
                                    qs.set_cond(ctx, 3, true);
                                }
                                htmltext = "30110-05.htm".to_owned();
                            } else {
                                htmltext = "30110-04.htm".to_owned();
                            }
                        } else if !ctx.has_quest_items(player, IKERS_LIST)
                            && ctx.has_quest_items(player, AKLANTOTH_1ST_GEM)
                        {
                            htmltext = "30110-06.htm".to_owned();
                        }
                    } else if ctx.has_quest_items(player, ORIMS_INSTRUCTIONS) {
                        if !ctx.has_at_least_one_quest_item(player, &[SOULTRAP_CRYSTAL, ZERUEL_BIND_CRYSTAL])
                        {
                            htmltext = "30110-07.htm".to_owned();
                        } else if !ctx.has_quest_items(player, ZERUEL_BIND_CRYSTAL)
                            && ctx.has_quest_items(player, SOULTRAP_CRYSTAL)
                        {
                            htmltext = "30110-09.htm".to_owned();
                        } else if !ctx.has_quest_items(player, SOULTRAP_CRYSTAL)
                            && ctx.has_quest_items(player, ZERUEL_BIND_CRYSTAL)
                        {
                            htmltext = "30110-10.htm".to_owned();
                        }
                    }
                }
                PRIEST_VADIN => {
                    if ctx.has_all_quest_items(player, &[ORIMS_INSTRUCTIONS, SIR_VASPERS_LETTER]) {
                        ctx.take_items(player, SIR_VASPERS_LETTER, 1);
                        ctx.give_items(player, VADINS_CRUCIFIX, 1);
                        htmltext = "30188-01.htm".to_owned();
                    } else if ctx.has_quest_items(player, VADINS_CRUCIFIX) {
                        if ctx.get_quest_items_count(player, TAMLIN_ORC_AMULET) < 20 {
                            htmltext = "30188-02.htm".to_owned();
                        } else {
                            ctx.take_items(player, VADINS_CRUCIFIX, 1);
                            ctx.take_items(player, TAMLIN_ORC_AMULET, -1);
                            ctx.give_items(player, VADINS_SANCTIONS, 1);
                            htmltext = "30188-03.htm".to_owned();
                        }
                    } else if ctx.has_quest_items(player, ORIMS_INSTRUCTIONS) {
                        if ctx.has_quest_items(player, VADINS_SANCTIONS) {
                            htmltext = "30188-04.htm".to_owned();
                        } else if ctx.has_quest_items(player, SWORD_OF_BINDING) {
                            htmltext = "30188-05.htm".to_owned();
                        }
                    }
                }
                TRADER_NESTLE => {
                    if ctx.has_quest_items(player, ALEXANDRIAS_BOOK) {
                        if !ctx.has_at_least_one_quest_item(
                            player,
                            &[
                                LEOPOLDS_JOURNAL,
                                NESTLES_MEMO,
                                AKLANTOTH_4TH_GEM,
                                AKLANTOTH_5TH_GEM,
                                AKLANTOTH_6TH_GEM,
                            ],
                        ) {
                            htmltext = "30314-01.htm".to_owned();
                        } else if ctx.has_quest_items(player, NESTLES_MEMO)
                            && !ctx.has_quest_items(player, LEOPOLDS_JOURNAL)
                        {
                            htmltext = "30314-03.htm".to_owned();
                        } else if !ctx.has_quest_items(player, NESTLES_MEMO)
                            && ctx.has_at_least_one_quest_item(
                                player,
                                &[
                                    LEOPOLDS_JOURNAL,
                                    AKLANTOTH_4TH_GEM,
                                    AKLANTOTH_5TH_GEM,
                                    AKLANTOTH_6TH_GEM,
                                ],
                            )
                        {
                            htmltext = "30314-04.htm".to_owned();
                        }
                    }
                }
                SIR_KLAUS_VASPER => {
                    if ctx.has_quest_items(player, ORIMS_INSTRUCTIONS) {
                        if ctx.has_quest_items(player, ORIMS_1ST_LETTER) {
                            htmltext = "30417-01.htm".to_owned();
                        } else if ctx.has_quest_items(player, SIR_VASPERS_LETTER) {
                            htmltext = "30417-04.htm".to_owned();
                        } else if ctx.has_quest_items(player, VADINS_SANCTIONS) {
                            ctx.give_items(player, SWORD_OF_BINDING, 1);
                            ctx.take_items(player, VADINS_SANCTIONS, 1);
                            if ctx.has_quest_items(player, SOULTRAP_CRYSTAL) {
                                qs.set_cond(ctx, 7, true);
                            }
                            htmltext = "30417-05.htm".to_owned();
                        } else if ctx.has_quest_items(player, SWORD_OF_BINDING) {
                            htmltext = "30417-06.htm".to_owned();
                        }
                    }
                }
                LEOPOLD => {
                    if ctx.has_quest_items(player, ALEXANDRIAS_BOOK) {
                        if ctx.has_quest_items(player, NESTLES_MEMO)
                            && !ctx.has_quest_items(player, LEOPOLDS_JOURNAL)
                        {
                            htmltext = "30435-01.htm".to_owned();
                        } else if ctx.has_quest_items(player, LEOPOLDS_JOURNAL)
                            && !ctx.has_quest_items(player, NESTLES_MEMO)
                        {
                            htmltext = "30435-03.htm".to_owned();
                        } else if ctx.has_all_quest_items(
                            player,
                            &[AKLANTOTH_4TH_GEM, AKLANTOTH_5TH_GEM, AKLANTOTH_6TH_GEM],
                        ) {
                            htmltext = "30435-04.htm".to_owned();
                        }
                    } else if ctx.has_at_least_one_quest_item(player, &[BRIMSTONE_1ST, ORIMS_INSTRUCTIONS]) {
                        htmltext = "30435-05.htm".to_owned();
                    }
                }
                MAGISTER_KAIRA => {
                    if ctx.has_quest_items(player, ALEXANDRIAS_BOOK) {
                        if !ctx.has_quest_items(player, AKLANTOTH_2ND_GEM) {
                            htmltext = "30476-01.htm".to_owned();
                        } else {
                            htmltext = "30476-03.htm".to_owned();
                        }
                    } else if ctx.has_at_least_one_quest_item(player, &[BRIMSTONE_1ST, ORIMS_INSTRUCTIONS]) {
                        htmltext = "30476-04.htm".to_owned();
                    }
                }
                WARDEN_RODERIK => {
                    if ctx.has_quest_items(player, ALEXANDRIAS_BOOK)
                        && ctx.has_at_least_one_quest_item(player, &[LARAS_MEMO, AKLANTOTH_3RD_GEM])
                    {
                        htmltext = "30631-01.htm".to_owned();
                    }
                }
                WARDEN_ENDRIGO => {
                    if ctx.has_quest_items(player, ALEXANDRIAS_BOOK)
                        && ctx.has_at_least_one_quest_item(player, &[LARAS_MEMO, AKLANTOTH_3RD_GEM])
                    {
                        htmltext = "30632-01.htm".to_owned();
                    }
                }
                FISHER_EVERT => {
                    if ctx.has_quest_items(player, ORIMS_INSTRUCTIONS) {
                        if ctx.has_all_quest_items(player, &[SOULTRAP_CRYSTAL, SWORD_OF_BINDING])
                            && !ctx.has_quest_items(player, BRIMSTONE_2ND)
                        {
                            htmltext = "30633-01.htm".to_owned();
                        } else if ctx.has_all_quest_items(player, &[SOULTRAP_CRYSTAL, BRIMSTONE_2ND])
                            && !ctx.has_quest_items(player, ZERUEL_BIND_CRYSTAL)
                        {
                            if ctx.summoned_npc_count(npc) < 1 {
                                call_zeruel(ctx, npc);
                            }
                            htmltext = "30633-02.htm".to_owned();
                        } else if ctx.has_quest_items(player, ZERUEL_BIND_CRYSTAL)
                            && !ctx.has_at_least_one_quest_item(player, &[SOULTRAP_CRYSTAL, BRIMSTONE_2ND])
                        {
                            htmltext = "30633-03.htm".to_owned();
                        }
                    }
                }
                _ => {}
            }
        } else if state == COMPLETED && npc.id == SHADOW_ORIM {
            htmltext = ctx.get_already_completed_msg(player);
        }
        Some(htmltext)
    }
}

/// Java `addSpawn(npc, DREVANUL_PRINCE_ZERUEL, DREVANUL_PRINCE_ZERUEL_SPAWN, false, 0)`: Zeruel
/// at his place, counted as a summoned NPC of Evert.
fn call_zeruel(ctx: &mut ScriptCtx<'_>, evert: ScriptNpc) {
    let (x, y, z) = DREVANUL_PRINCE_ZERUEL_SPAWN;
    ctx.add_spawn_summoned(evert, DREVANUL_PRINCE_ZERUEL, x, y, z, 0, false, 0, 0);
}

/// One kill of a collected monster: the item up to 20, the middle sound at the 20th.
fn collect(ctx: &mut ScriptCtx<'_>, killer: ObjectId, item: i32) {
    if ctx.has_all_quest_items(killer, &[ALEXANDRIAS_BOOK, IKERS_LIST])
        && ctx.get_quest_items_count(killer, item) < 20
    {
        ctx.give_items(killer, item, 1);
        if ctx.get_quest_items_count(killer, item) >= 20 {
            ctx.play_sound(killer, QuestSound::ItemsoundQuestMiddle);
        } else {
            ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
        }
    }
}

/// Java `checkWeapon`: the Sword of Binding in the right hand.
fn check_weapon(ctx: &ScriptCtx<'_>, player: ObjectId) -> bool {
    ctx.active_weapon_item_id(player) == Some(SWORD_OF_BINDING)
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::ScriptRegistry;
    use l2_world::script::{player_class, race};

    use super::*;
    use crate::testing::{Pages, ScriptTest, rows};

    const NAME: &str = "Q00229_TestOfWitchcraft";

    const NPCS: &str = r#"
        <npc id="30630" type="Folk" level="70" name="Shadow Orim"/>
        <npc id="30098" type="Folk" level="70" name="Trader Alexandria"/>
        <npc id="30110" type="Folk" level="70" name="Magister Iker"/>
        <npc id="20557" type="Monster" level="40" name="Dire Wyrm"/>
        <npc id="27099" type="Monster" level="40" name="Nameless Revenant"/>
        <npc id="27101" type="Monster" level="40" name="Drevanul Prince Zeruel"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="7562" type="EtcItem" name="Dimensional Diamond"><set name="is_stackable" val="true"/></item>
        <item id="3029" type="EtcItem" name="Sword of Binding"><set name="is_questitem" val="true"/></item>
        <item id="3307" type="EtcItem" name="Mark of Witchcraft"><set name="is_questitem" val="true"/></item>
        <item id="3308" type="EtcItem" name="Orim's Diagram"><set name="is_questitem" val="true"/></item>
        <item id="3309" type="EtcItem" name="Alexandria's Book"><set name="is_questitem" val="true"/></item>
        <item id="3310" type="EtcItem" name="Iker's List"><set name="is_questitem" val="true"/></item>
        <item id="3311" type="EtcItem" name="Dire Wyrm Fang"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="3312" type="EtcItem" name="Leto Lizardman Charm"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="3313" type="EtcItem" name="Heartstone"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="3314" type="EtcItem" name="Lara's Memo"><set name="is_questitem" val="true"/></item>
        <item id="3317" type="EtcItem" name="Aklantoth 1st Gem"><set name="is_questitem" val="true"/></item>
        <item id="3318" type="EtcItem" name="Aklantoth 2nd Gem"><set name="is_questitem" val="true"/></item>
        <item id="3319" type="EtcItem" name="Aklantoth 3rd Gem"><set name="is_questitem" val="true"/></item>
        <item id="3320" type="EtcItem" name="Aklantoth 4th Gem"><set name="is_questitem" val="true"/></item>
        <item id="3321" type="EtcItem" name="Aklantoth 5th Gem"><set name="is_questitem" val="true"/></item>
        <item id="3322" type="EtcItem" name="Aklantoth 6th Gem"><set name="is_questitem" val="true"/></item>
        <item id="3323" type="EtcItem" name="Brimstone 1st"><set name="is_questitem" val="true"/></item>
        <item id="3324" type="EtcItem" name="Orim's Instructions"><set name="is_questitem" val="true"/></item>
        <item id="3325" type="EtcItem" name="Orim's 1st Letter"><set name="is_questitem" val="true"/></item>
        <item id="3326" type="EtcItem" name="Orim's 2nd Letter"><set name="is_questitem" val="true"/></item>
        <item id="3331" type="EtcItem" name="Iker's Amulet"><set name="is_questitem" val="true"/></item>
        <item id="3332" type="EtcItem" name="Soultrap Crystal"><set name="is_questitem" val="true"/></item>
        <item id="3333" type="EtcItem" name="Purgatory Key"><set name="is_questitem" val="true"/></item>
        <item id="3334" type="EtcItem" name="Zeruel Bind Crystal"><set name="is_questitem" val="true"/></item>
        <item id="3335" type="EtcItem" name="Brimstone 2nd"><set name="is_questitem" val="true"/></item>"#;

    /// The pages of the quest with the links of the real pages.
    fn pages() -> Pages {
        Pages::of(&Q00229TestOfWitchcraft)
            .plain(&[
                "30630-01.htm",
                "30630-02.htm",
                "30630-08.htm",
                "30630-08a.htm",
                "30630-09.htm",
                "30630-10.htm",
                "30630-22.htm",
            ])
            .page("30630-03.htm", &["30630-04.htm"])
            .page("30630-04.htm", &["30630-07.htm"])
            .page("30630-05.htm", &["30630-06.htm"])
            .page("30630-06.htm", &["30630-07.htm"])
            .page("30630-07.htm", &["ACCEPT"])
            .page("30630-11.htm", &["30630-12.htm", "30630-13.htm"])
            .page("30630-12.htm", &["30630-13.htm"])
            .page("30630-13.htm", &["30630-14.htm"])
            .plain(&["30630-14.htm"])
            .page("30630-19.htm", &["30630-20.htm"])
            .page("30630-20.htm", &["30630-21.htm"])
            .page("30630-21.htm", &["30630-22.htm"])
            .page("30098-01.htm", &["30098-02.htm"])
            .page("30098-02.htm", &["30098-03.htm"])
            .plain(&["30098-03.htm", "30098-04.htm"])
            .npcs(&[SHADOW_ORIM, TRADER_ALEXANDRIA])
    }

    struct Npcs {
        orim: Oid,
        alexandria: Oid,
    }

    fn setup() -> (ScriptTest, Npcs) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00229TestOfWitchcraft);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages());
        let npcs = Npcs {
            orim: t.spawn(SHADOW_ORIM, 20),
            alexandria: t.spawn(TRADER_ALEXANDRIA, 30),
        };
        (t, npcs)
    }

    /// Orim's pages up to the accept link, then the accept.
    fn start(t: &mut ScriptTest, orim: Oid) {
        t.quest_page(1, orim);
        // The wizard's page 03 offers 04, the others' page 05 offers 06; the other one is refused.
        t.event(1, NAME, "30630-04.htm");
        t.event(1, NAME, "30630-06.htm");
        t.event(1, NAME, "30630-07.htm");
        t.event(1, NAME, "ACCEPT");
    }

    #[test]
    fn the_start_is_gated_by_class_and_level() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 40, race::HUMAN, player_class::FIGHTER, vec![]);
        assert!(t.quest_page(1, npcs.orim).contains("30630-01.htm"));
        t.enter_as(2, 38, race::HUMAN, player_class::WIZARD, vec![]);
        assert!(t.quest_page(2, npcs.orim).contains("30630-02.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_wizard_gets_122_diamonds_and_the_diagram() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 39, race::HUMAN, player_class::WIZARD, vec![]);
        start(&mut t, npcs.orim);
        assert_eq!(t.count(1, ORIMS_DIAGRAM), 1);
        assert_eq!(t.count(1, DIMENSIONAL_DIAMOND), 122);
        assert!(t.take_writes().contains(&rows::insert(NAME, "cond", "1")));
    }

    #[test]
    fn alexandria_swaps_the_diagram_for_the_book() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 39, race::HUMAN, player_class::KNIGHT, vec![]);
        start(&mut t, npcs.orim);
        assert_eq!(t.count(1, DIMENSIONAL_DIAMOND), 104);
        t.take_writes();
        assert!(t.quest_page(1, npcs.alexandria).contains("30098-01.htm"));
        assert!(t.event(1, NAME, "30098-02.htm").contains("30098-02.htm"));
        assert!(t.event(1, NAME, "30098-03.htm").contains("30098-03.htm"));
        assert_eq!(t.count(1, ALEXANDRIAS_BOOK), 1);
        assert_eq!(t.count(1, ORIMS_DIAGRAM), 0);
        assert!(t.take_writes().contains(&rows::update(NAME, "cond", "2")));
    }

    #[test]
    fn the_wyrm_fangs_stop_at_twenty() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 39, race::HUMAN, player_class::WIZARD, vec![]);
        start(&mut t, npcs.orim);
        t.world.run_as_script(NAME, |ctx| {
            ctx.give_items(Oid(1), ALEXANDRIAS_BOOK, 1);
            ctx.give_items(Oid(1), IKERS_LIST, 1);
        });
        for _ in 0..21 {
            let wyrm = t.spawn(DIRE_WYRM, 50);
            t.kill(1, wyrm);
            t.world.delete_npc(wyrm);
        }
        assert_eq!(t.count(1, DIRE_WYRM_FANG), 20);
        assert_eq!(t.count(1, 57), 0);
    }

    #[test]
    fn the_reward_is_the_mark_with_the_bind_crystal() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 39, race::HUMAN, player_class::WIZARD, vec![]);
        start(&mut t, npcs.orim);
        t.world.run_as_script(NAME, |ctx| {
            ctx.give_items(Oid(1), SWORD_OF_BINDING, 1);
            ctx.give_items(Oid(1), ZERUEL_BIND_CRYSTAL, 1);
        });
        t.take_writes();
        t.take();
        assert!(t.quest_page(1, npcs.orim).contains("30630-19.htm"));
        t.event(1, NAME, "30630-20.htm");
        t.event(1, NAME, "30630-21.htm");
        t.take();
        t.event(1, NAME, "30630-22.htm");
        assert_eq!(t.count(1, MARK_OF_WITCHCRAFT), 1);
        assert_eq!(t.count(1, 57), 372_154);
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed"),
            ]
        );
        assert!(
            t.quest_page(1, npcs.orim)
                .contains("This quest has already been completed.")
        );
    }
}
