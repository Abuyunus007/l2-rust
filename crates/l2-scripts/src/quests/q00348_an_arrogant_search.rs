//! Quest 348: An Arrogant Search (Magister Hanellin in the Town of Oren).
// Java: dist/game/data/scripts/quests/Q00348_AnArrogantSearch/Q00348_AnArrogantSearch.java
//
// Pattern: a long, memo-driven chain. `memoState` is the stage (1..19); `memoStateEx(0)` mirrors
// it for the kill hooks and `memoStateEx(1)` packs one digit per Holy Ark (letters delivered,
// guardians killed, keys found): units = the saint's item the player must bring, tens = Harne,
// hundreds = Claudia, thousands = Martien. NPCs of the quest talk to the same variables.

use l2_protocol_game::NpcStringId;
use l2_protocol_game::server::ChatType;
use l2_util::ObjectId;
use l2_world::script::{QuestSound, player_class};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup, SkillKey};

// NPCs
const HARNE: i32 = 30144;
const MARTIEN: i32 = 30645;
const SIR_GUSTAV_ATHEBALDT: i32 = 30760;
const HARDIN: i32 = 30832;
const HANELLIN: i32 = 30864;
const IASON_HEINE: i32 = 30969;
const HOLY_ARK_OF_SECRECY_1: i32 = 30977;
const HOLY_ARK_OF_SECRECY_2: i32 = 30978;
const HOLY_ARK_OF_SECRECY_3: i32 = 30979;
const ARK_GUARDIANS_CORPSE: i32 = 30980;
const CLAUDIA_ATHEBALDT: i32 = 31001;

// Items
const GREATER_HEALING_POTION: i32 = 1061;
const ANTIDOTE: i32 = 1831;
const TITANS_POWERSTONE: i32 = 4287;
const HANELLINS_1ST_LETTER: i32 = 4288;
const HANELLINS_2ND_LETTER: i32 = 4289;
const HANELLINS_3RD_LETTER: i32 = 4290;
const FIRST_KEY_OF_ARK: i32 = 4291;
const SECOND_KEY_OF_ARK: i32 = 4292;
const THIRD_KEY_OF_ARK: i32 = 4293;
const WHITE_FABRIC_1: i32 = 4294;
const BLOODED_FABRIC: i32 = 4295;
const BOOK_OF_SAINT: i32 = 4397;
const BLOOD_OF_SAINT: i32 = 4398;
const BOUGH_OF_SAINT: i32 = 4399;
const WHITE_FABRIC_2: i32 = 4400;
const SHELL_OF_MONSTERS: i32 = 14857;

// Misc
const MIN_LEVEL: i32 = 60;
const MIN_HP_PERCENTAGE: f64 = 0.3;

// Variables
const I_QUEST0: &str = "I_QUEST0";

// Rewards
const ANIMAL_BONE: i32 = 1872;
const ORIHARUKON_ORE: i32 = 1874;
const COKES: i32 = 1879;
const COARSE_BONE_POWDER: i32 = 1881;
const VARNISH_OF_PURITY: i32 = 1887;
const SYNTHETIC_COKES: i32 = 1888;
const ENRIA: i32 = 4042;
const GREAT_SWORD_BLADE: i32 = 4104;
const HEAVY_WAR_AXE_HEAD: i32 = 4105;
const SPRITES_STAFF_HEAD: i32 = 4106;
const KESHANBERK_BLADE: i32 = 4107;
const SWORD_OF_VALHALLA_BLADE: i32 = 4108;
const KRIS_EDGE: i32 = 4109;
const HELL_KNIFE_EDGE: i32 = 4110;
const ARTHRO_NAIL_BLADE: i32 = 4111;
const DARK_ELVEN_LONGBOW_SHAFT: i32 = 4112;
const GREAT_AXE_HEAD: i32 = 4113;
const SWORD_OF_DAMASCUS_BLADE: i32 = 4114;
const LANCE_BLADE: i32 = 4115;
const ART_OF_BATTLE_AXE_BLADE: i32 = 4117;
const EVIL_SPIRIT_HEAD: i32 = 4118;
const DEMONS_DAGGER_EDGE: i32 = 4119;
const BELLION_CESTUS_EDGE: i32 = 4120;
const BOW_OF_PERIL_SHAFT: i32 = 4121;

// Quest Monsters
const ARK_GUARDIAN_ELBEROTH: i32 = 27182;
const ARK_GUARDIAN_SHADOWFANG: i32 = 27183;
const ANGEL_KILLER: i32 = 27184;

// Monsters
const YINTZU: i32 = 20647;
const PALIOTE: i32 = 20648;
const PLATINUM_TRIBE_SHAMAN: i32 = 20828;
const PLATINUM_TRIBE_OVERLORD: i32 = 20829;
const GUARDIAN_ANGEL: i32 = 20830;
const SEAL_ANGEL_1: i32 = 20831;
const SEAL_ANGEL_2: i32 = 20860;

/// Java `Q00348_AnArrogantSearch`.
pub struct Q00348AnArrogantSearch;

/// Java `npc.isInsideRadius3D(player, ALT_PARTY_RANGE)`.
fn near(ctx: &ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> bool {
    match ctx.world().position_of(player) {
        Some((x, y, z)) => ctx.is_inside_radius_3d(npc, x, y, z, ctx.alt_party_range()),
        None => false,
    }
}

/// Java `getReward(player)`: the weapon part and materials by class and level.
fn get_reward(ctx: &mut ScriptCtx<'_>, player: ObjectId) {
    use player_class::*;
    let class_id = ctx.player_class_id(player);
    let level = ctx.player_level(player);
    match class_id {
        TREASURE_HUNTER | PLAINS_WALKER | ABYSS_WALKER | ADVENTURER | WIND_RIDER | GHOST_HUNTER
        | MALE_SOULBREAKER | FEMALE_SOULBREAKER | MALE_SOUL_HOUND | FEMALE_SOUL_HOUND | INSPECTOR
        | JUDICATOR => {
            if level < 69 {
                ctx.give_items(player, KRIS_EDGE, 1);
                ctx.give_items(player, SYNTHETIC_COKES, 2);
            } else {
                ctx.give_items(player, DEMONS_DAGGER_EDGE, 1);
                ctx.give_items(player, COKES, 2);
            }
        }
        TYRANT | GRAND_KHAVATARI => {
            if level < 69 {
                ctx.give_items(player, ARTHRO_NAIL_BLADE, 1);
                ctx.give_items(player, SYNTHETIC_COKES, 2);
                ctx.give_items(player, COKES, 1);
            } else {
                ctx.give_items(player, BELLION_CESTUS_EDGE, 1);
                ctx.give_items(player, ORIHARUKON_ORE, 2);
            }
        }
        PALADIN | DARK_AVENGER | PROPHET | TEMPLE_KNIGHT | SWORDSINGER | SHILLIEN_KNIGHT | BLADEDANCER
        | SHILLIEN_ELDER | PHOENIX_KNIGHT | HELL_KNIGHT | HIEROPHANT | EVA_TEMPLAR | SWORD_MUSE
        | SHILLIEN_TEMPLAR | SPECTRAL_DANCER | SHILLIEN_SAINT => {
            if level < 69 {
                ctx.give_items(player, KESHANBERK_BLADE, 1);
                ctx.give_items(player, SYNTHETIC_COKES, 2);
            } else {
                ctx.give_items(player, SWORD_OF_DAMASCUS_BLADE, 1);
                ctx.give_items(player, ORIHARUKON_ORE, 2);
            }
        }
        HAWKEYE | SILVER_RANGER | PHANTOM_RANGER | SAGITTARIUS | MOONLIGHT_SENTINEL | GHOST_SENTINEL
        | ARBALESTER | TRICKSTER => {
            if level < 69 {
                ctx.give_items(player, DARK_ELVEN_LONGBOW_SHAFT, 1);
                ctx.give_items(player, SYNTHETIC_COKES, 2);
            } else {
                ctx.give_items(player, BOW_OF_PERIL_SHAFT, 1);
                ctx.give_items(player, COARSE_BONE_POWDER, 9);
            }
        }
        GLADIATOR | BISHOP | ELDER | DUELIST | CARDINAL | EVA_SAINT => {
            if level < 69 {
                ctx.give_items(player, HEAVY_WAR_AXE_HEAD, 1);
                ctx.give_items(player, SYNTHETIC_COKES, 2);
                ctx.give_items(player, COKES, 1);
            } else {
                ctx.give_items(player, ART_OF_BATTLE_AXE_BLADE, 1);
                ctx.give_items(player, ORIHARUKON_ORE, 2);
            }
        }
        WARLORD | BOUNTY_HUNTER | WARSMITH | DREADNOUGHT | FORTUNE_SEEKER | MAESTRO => {
            if level < 63 {
                ctx.give_items(player, GREAT_AXE_HEAD, 1);
                ctx.give_items(player, ENRIA, 1);
                ctx.give_items(player, COKES, 1);
            } else {
                ctx.give_items(player, LANCE_BLADE, 1);
                ctx.give_items(player, ORIHARUKON_ORE, 2);
            }
        }
        SORCERER | SPELLSINGER | OVERLORD | ARCHMAGE | MYSTIC_MUSE | DOMINATOR => {
            if level < 63 {
                ctx.give_items(player, SPRITES_STAFF_HEAD, 1);
                ctx.give_items(player, ORIHARUKON_ORE, 4);
                ctx.give_items(player, COARSE_BONE_POWDER, 1);
            } else {
                ctx.give_items(player, EVIL_SPIRIT_HEAD, 1);
                ctx.give_items(player, ANIMAL_BONE, 5);
            }
        }
        NECROMANCER | SPELLHOWLER | SOULTAKER | STORM_SCREAMER => {
            ctx.give_items(player, HELL_KNIFE_EDGE, 1);
            ctx.give_items(player, SYNTHETIC_COKES, 2);
            ctx.give_items(player, ANIMAL_BONE, 2);
        }
        DESTROYER | TITAN | BERSERKER | DOOMBRINGER => {
            ctx.give_items(player, GREAT_SWORD_BLADE, 1);
            ctx.give_items(player, VARNISH_OF_PURITY, 2);
            ctx.give_items(player, SYNTHETIC_COKES, 2);
        }
        ELEMENTAL_SUMMONER | PHANTOM_SUMMONER | ELEMENTAL_MASTER | SPECTRAL_MASTER => {
            ctx.give_items(player, SWORD_OF_DAMASCUS_BLADE, 1);
            ctx.give_items(player, ENRIA, 1);
        }
        WARCRYER | DOOMCRYER => {
            ctx.give_items(player, SWORD_OF_VALHALLA_BLADE, 1);
            ctx.give_items(player, ORIHARUKON_ORE, 1);
            ctx.give_items(player, VARNISH_OF_PURITY, 1);
        }
        WARLOCK | ARCANA_LORD => {
            ctx.give_items(player, ART_OF_BATTLE_AXE_BLADE, 1);
            ctx.give_items(player, ENRIA, 1);
        }
        _ => ctx.give_adena(player, 49000, true),
    }
}

/// The shared tail of the platinum tribe hooks. `step` is the progress one hit or kill adds
/// (the same number is tested again after the add, like Java).
fn platinum_progress(
    ctx: &mut ScriptCtx<'_>,
    qs: l2_world::Qs,
    member: ObjectId,
    sound_to: ObjectId,
    step: i32,
) {
    if qs.get_memo_state_ex(ctx, 0) == 12 {
        qs.set_memo_state_ex(ctx, 1, qs.get_memo_state_ex(ctx, 1) + step);
        // NOTE(java-quirk): the limit is tested against the value plus the step again.
        if qs.get_memo_state_ex(ctx, 1) + step > 80000 {
            ctx.give_items(member, BLOODED_FABRIC, 1);
            ctx.take_items(member, WHITE_FABRIC_1, 1);
            qs.exit_quest(ctx, true, true);
        }
    }
    if qs.get_memo_state_ex(ctx, 0) == 13 {
        qs.set_memo_state_ex(ctx, 1, qs.get_memo_state_ex(ctx, 1) + step);
        if qs.get_memo_state_ex(ctx, 1) + step > 100000 {
            ctx.give_items(member, BLOODED_FABRIC, 1);
            ctx.take_items(member, WHITE_FABRIC_1, 1);
            qs.set_memo_state(ctx, 14); // Custom line
            qs.set_memo_state_ex(ctx, 0, 14);
            ctx.play_sound(sound_to, QuestSound::ItemsoundQuestMiddle);
        }
    }
}

impl Script for Q00348AnArrogantSearch {
    fn name(&self) -> &'static str {
        "Q00348_AnArrogantSearch"
    }

    fn path(&self) -> &'static str {
        "quests/Q00348_AnArrogantSearch"
    }

    fn id(&self) -> i32 {
        348
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_attack_id(&[
            ARK_GUARDIAN_ELBEROTH,
            ARK_GUARDIAN_SHADOWFANG,
            ANGEL_KILLER,
            PLATINUM_TRIBE_SHAMAN,
            PLATINUM_TRIBE_OVERLORD,
        ]);
        s.add_spawn_id(&[ARK_GUARDIAN_ELBEROTH, ARK_GUARDIAN_SHADOWFANG, ANGEL_KILLER]);
        s.add_start_npc(&[HANELLIN]);
        s.add_talk_id(&[
            HANELLIN,
            IASON_HEINE,
            HOLY_ARK_OF_SECRECY_1,
            HOLY_ARK_OF_SECRECY_2,
            HOLY_ARK_OF_SECRECY_3,
            ARK_GUARDIANS_CORPSE,
            CLAUDIA_ATHEBALDT,
            HARNE,
            MARTIEN,
            SIR_GUSTAV_ATHEBALDT,
            HARDIN,
        ]);
        s.add_kill_id(&[
            ARK_GUARDIAN_ELBEROTH,
            ARK_GUARDIAN_SHADOWFANG,
            YINTZU,
            PALIOTE,
            PLATINUM_TRIBE_SHAMAN,
            PLATINUM_TRIBE_OVERLORD,
            GUARDIAN_ANGEL,
            SEAL_ANGEL_1,
            SEAL_ANGEL_2,
        ]);
        s.register_quest_items(&[
            SHELL_OF_MONSTERS,
            TITANS_POWERSTONE,
            HANELLINS_1ST_LETTER,
            HANELLINS_2ND_LETTER,
            HANELLINS_3RD_LETTER,
            FIRST_KEY_OF_ARK,
            SECOND_KEY_OF_ARK,
            THIRD_KEY_OF_ARK,
            WHITE_FABRIC_1,
            BOOK_OF_SAINT,
            BLOOD_OF_SAINT,
            BOUGH_OF_SAINT,
            WHITE_FABRIC_2,
        ]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        if let Some(npc) = npc
            && matches!(
                npc.id,
                ARK_GUARDIAN_ELBEROTH | ARK_GUARDIAN_SHADOWFANG | ANGEL_KILLER
            )
            && event == "DESPAWN"
        {
            ctx.delete_npc(npc);
            return None;
        }

        let player = player?;
        let qs = ctx.get_quest_state(player, false)?;
        let mut htmltext = None;
        match event {
            "30864-02.htm" | "30864-03.htm" | "30864-10.html" | "30864-11.html" | "30864-12.html"
            | "30864-25.html" | "31001-02.html" | "30144-02.html" | "30645-02.html" => {
                htmltext = Some(event.to_owned());
            }
            "30864-04.htm" => {
                qs.set_memo_state(ctx, 1);
                qs.start_quest(ctx);
                qs.set_cond(ctx, 2, false);
                htmltext = Some(event.to_owned());
            }
            "30864-08.html" => {
                let memo_state = qs.get_memo_state(ctx);
                if (memo_state == 1
                    && ctx.has_at_least_one_quest_item(player, &[TITANS_POWERSTONE, SHELL_OF_MONSTERS]))
                    || memo_state == 2
                {
                    qs.set_memo_state_ex(ctx, 0, 4);
                    qs.set_memo_state_ex(ctx, 1, 0);
                    qs.set_memo_state(ctx, 4);
                    qs.set_cond(ctx, 4, false);
                    htmltext = Some(event.to_owned());
                }
            }
            "30864-09.html" => {
                if qs.is_memo_state(ctx, 4) && qs.get_memo_state_ex(ctx, 1) == 0 {
                    ctx.give_items(player, HANELLINS_1ST_LETTER, 1);
                    ctx.give_items(player, HANELLINS_2ND_LETTER, 1);
                    ctx.give_items(player, HANELLINS_3RD_LETTER, 1);
                    qs.set_memo_state(ctx, 5);
                    qs.set_cond(ctx, 5, false);
                    htmltext = Some(event.to_owned());
                }
            }
            "30864-26.html" => {
                if qs.is_memo_state(ctx, 10) && ctx.get_quest_items_count(player, WHITE_FABRIC_1) == 1 {
                    qs.set_memo_state(ctx, 11);
                    htmltext = Some(event.to_owned());
                }
            }
            "30864-27.html" => {
                if qs.is_memo_state(ctx, 11)
                    && ctx.get_quest_items_count(player, WHITE_FABRIC_1) == 1
                    && qs.get_memo_state_ex(ctx, 1) > 0
                {
                    match qs.get_memo_state_ex(ctx, 1) {
                        1 => ctx.give_adena(player, 43000, true),
                        2 => ctx.give_adena(player, 4000, true),
                        3 => ctx.give_adena(player, 13000, true),
                        _ => {}
                    }
                    qs.set_memo_state_ex(ctx, 0, 12);
                    qs.set_memo_state_ex(ctx, 1, 100);
                    qs.set_cond(ctx, 24, false);
                    htmltext = Some(event.to_owned());
                } else {
                    htmltext = Some("30864-28.html".to_owned());
                }
            }
            "30864-29.html" => {
                if qs.is_memo_state(ctx, 11)
                    && qs.get_memo_state_ex(ctx, 1) == 0
                    && ctx.get_quest_items_count(player, WHITE_FABRIC_1) == 1
                {
                    ctx.give_adena(player, 49000, true);
                    qs.set_memo_state(ctx, 12); // Custom line
                    qs.set_memo_state_ex(ctx, 0, 12);
                    qs.set_memo_state_ex(ctx, 1, 20000);
                    qs.set_cond(ctx, 24, false);
                    htmltext = Some(event.to_owned());
                }
            }
            "30864-30.html" => {
                if qs.is_memo_state(ctx, 11)
                    && qs.get_memo_state_ex(ctx, 1) == 0
                    && ctx.get_quest_items_count(player, WHITE_FABRIC_1) == 1
                {
                    qs.set_memo_state(ctx, 13); // Custom line
                    qs.set_memo_state_ex(ctx, 0, 13);
                    qs.set_memo_state_ex(ctx, 1, 20000);
                    qs.set_cond(ctx, 25, false);
                    htmltext = Some(event.to_owned());
                }
            }
            "30864-43.html" => {
                if qs.is_memo_state(ctx, 15) {
                    qs.set_memo_state(ctx, 16);
                    htmltext = Some(event.to_owned());
                }
            }
            "30864-44.html" => {
                if qs.is_memo_state(ctx, 15) || qs.is_memo_state(ctx, 16) {
                    if ctx.has_quest_items(player, BLOODED_FABRIC) {
                        ctx.give_items(player, WHITE_FABRIC_1, 9);
                    } else {
                        ctx.give_items(player, WHITE_FABRIC_1, 10);
                    }
                }
                qs.set_memo_state(ctx, 17); // Custom line
                qs.set_memo_state_ex(ctx, 0, 17);
                qs.set_memo_state_ex(ctx, 1, 0);
                qs.set_cond(ctx, 26, false);
                htmltext = Some(event.to_owned());
            }
            "30864-47.html" => {
                if qs.is_memo_state(ctx, 17)
                    && ctx.get_quest_items_count(player, BLOODED_FABRIC) >= 10
                    && !ctx.has_quest_items(player, WHITE_FABRIC_1)
                {
                    qs.set_memo_state(ctx, 18); // Custom line
                    qs.set_memo_state_ex(ctx, 0, 18);
                    qs.set_memo_state_ex(ctx, 1, 0);
                    qs.set_cond(ctx, 27, false);
                    htmltext = Some(event.to_owned());
                }
            }
            "30864-50.html" => {
                if qs.is_memo_state(ctx, 19) {
                    ctx.give_items(player, WHITE_FABRIC_1, 10);
                    qs.set_memo_state(ctx, 17); // Custom line
                    qs.set_memo_state_ex(ctx, 0, 17);
                    qs.set_memo_state_ex(ctx, 1, 0);
                    qs.set_cond(ctx, 29, false);
                    htmltext = Some(event.to_owned());
                }
            }
            "30864-51.html" => {
                qs.exit_quest(ctx, true, true);
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
        match npc.id {
            ARK_GUARDIAN_ELBEROTH => {
                if !ctx
                    .world()
                    .npc_variables(npc.object_id)
                    .is_some_and(|v| v.get_bool(I_QUEST0, false))
                {
                    ctx.world_mut()
                        .npc_variables_mut(npc.object_id)
                        .set_bool(I_QUEST0, true);
                    ctx.broadcast_npc_say(
                        npc,
                        ChatType::NpcGeneral,
                        NpcStringId::SorryAboutThisButIMustKillYouNow,
                        &[],
                    );
                }
            }
            ARK_GUARDIAN_SHADOWFANG => {
                if !ctx
                    .world()
                    .npc_variables(npc.object_id)
                    .is_some_and(|v| v.get_bool(I_QUEST0, false))
                {
                    ctx.world_mut()
                        .npc_variables_mut(npc.object_id)
                        .set_bool(I_QUEST0, true);
                    ctx.broadcast_npc_say(
                        npc,
                        ChatType::NpcGeneral,
                        NpcStringId::IShallDrenchThisMountainWithYourBlood,
                        &[],
                    );
                }
            }
            ANGEL_KILLER => {
                // NOTE(java-quirk): Java dereferences the quest state without a null check.
                let Some(qs) = ctx.get_quest_state(attacker, false) else {
                    return;
                };
                if qs.get_memo_state_ex(ctx, 0) < 8
                    && !ctx.has_quest_items(attacker, FIRST_KEY_OF_ARK)
                    && !ctx.has_quest_items(attacker, BLOOD_OF_SAINT)
                {
                    if ((qs.get_memo_state_ex(ctx, 1) % 100) / 10) == 1 {
                        if ctx.npc_current_hp(npc) < f64::from(ctx.npc_max_hp(npc)) * MIN_HP_PERCENTAGE {
                            qs.set_memo_state_ex(ctx, 1, qs.get_memo_state_ex(ctx, 1) + 10);
                            if qs.get_memo_state_ex(ctx, 1) % 10 == 0 {
                                ctx.clear_radar(attacker);
                                ctx.add_radar(attacker, -2908, 44128, -2712);
                            } else {
                                qs.set_cond(ctx, 19, true);
                            }
                            ctx.broadcast_npc_say(
                                npc,
                                ChatType::NpcGeneral,
                                NpcStringId::HaThatWasFunIfYouWishToFindTheKeySearchTheCorpse,
                                &[],
                            );
                            ctx.delete_npc(npc);
                        }
                    } else if ((qs.get_memo_state_ex(ctx, 1) % 100) / 10) == 2 {
                        ctx.broadcast_npc_say(
                            npc,
                            ChatType::NpcGeneral,
                            NpcStringId::WeDonTHaveAnyFurtherBusinessToDiscussHaveYouSearchedTheCorpseForTheKey,
                            &[],
                        );
                        ctx.delete_npc(npc);
                    }
                } else if ctx.has_at_least_one_quest_item(attacker, &[FIRST_KEY_OF_ARK, BLOOD_OF_SAINT]) {
                    ctx.broadcast_npc_say(
                        npc,
                        ChatType::NpcGeneral,
                        NpcStringId::WeDonTHaveAnyFurtherBusinessToDiscussHaveYouSearchedTheCorpseForTheKey,
                        &[],
                    );
                    ctx.delete_npc(npc);
                }
            }
            PLATINUM_TRIBE_SHAMAN | PLATINUM_TRIBE_OVERLORD => {
                let step = if npc.id == PLATINUM_TRIBE_SHAMAN { 60 } else { 70 };
                let qs = ctx.random_party_member_state(attacker, -1, 3, Some(npc));
                if let Some(qs) = qs
                    && near(ctx, npc, attacker)
                    && (qs.get_memo_state_ex(ctx, 0) == 12 || qs.get_memo_state_ex(ctx, 0) == 13)
                    && ctx.has_quest_items(qs.player(), WHITE_FABRIC_1)
                {
                    platinum_progress(ctx, qs, qs.player(), qs.player(), step);
                }
            }
            _ => {}
        }
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId, _is_summon: bool) {
        let qs = ctx.random_party_member_state(player, -1, 3, Some(npc));
        let Some(qs) = qs else {
            return;
        };
        if !near(ctx, npc, player) {
            return;
        }
        let member = qs.player();
        match npc.id {
            ARK_GUARDIAN_ELBEROTH => {
                if near(ctx, npc, player)
                    && qs.get_memo_state_ex(ctx, 0) < 8
                    && ((qs.get_memo_state_ex(ctx, 1) % 1000) / 100) == 1
                    && !ctx.has_quest_items(member, SECOND_KEY_OF_ARK)
                    && !ctx.has_quest_items(member, BOOK_OF_SAINT)
                {
                    qs.set_memo_state_ex(ctx, 1, qs.get_memo_state_ex(ctx, 1) + 100);
                    if qs.get_memo_state_ex(ctx, 1) % 10 != 0 {
                        qs.set_cond(ctx, 11, false);
                    }
                    ctx.give_items(member, SECOND_KEY_OF_ARK, 1);
                    ctx.play_sound(player, QuestSound::ItemsoundQuestItemget);
                    ctx.broadcast_npc_say(
                        npc,
                        ChatType::NpcGeneral,
                        NpcStringId::YouFoolsWillGetWhatSComingToYou,
                        &[],
                    );
                }
            }
            ARK_GUARDIAN_SHADOWFANG => {
                if near(ctx, npc, player)
                    && qs.get_memo_state_ex(ctx, 0) < 8
                    && ((qs.get_memo_state_ex(ctx, 1) % 10000) / 1000) == 1
                    && !ctx.has_quest_items(member, THIRD_KEY_OF_ARK)
                    && !ctx.has_quest_items(member, BOUGH_OF_SAINT)
                {
                    qs.set_memo_state_ex(ctx, 1, qs.get_memo_state_ex(ctx, 1) + 1000);
                    if qs.get_memo_state_ex(ctx, 1) % 10 != 0 {
                        qs.set_cond(ctx, 15, false);
                    }
                    ctx.give_items(member, THIRD_KEY_OF_ARK, 1);
                    ctx.play_sound(player, QuestSound::ItemsoundQuestItemget);
                    ctx.broadcast_npc_say(
                        npc,
                        ChatType::NpcGeneral,
                        NpcStringId::YouGuysWouldnTKnowTheSevenSealsAreArrrgh,
                        &[],
                    );
                }
            }
            YINTZU | PALIOTE => {
                if near(ctx, npc, player)
                    && qs.is_memo_state(ctx, 1)
                    && !ctx.has_quest_items(member, SHELL_OF_MONSTERS)
                {
                    ctx.give_items(member, SHELL_OF_MONSTERS, 1);
                    ctx.play_sound(player, QuestSound::ItemsoundQuestItemget);
                }
            }
            PLATINUM_TRIBE_SHAMAN | PLATINUM_TRIBE_OVERLORD => {
                let step = if npc.id == PLATINUM_TRIBE_SHAMAN { 600 } else { 700 };
                if (qs.get_memo_state_ex(ctx, 0) == 12 || qs.get_memo_state_ex(ctx, 0) == 13)
                    && ctx.has_quest_items(member, WHITE_FABRIC_1)
                {
                    platinum_progress(ctx, qs, member, player, step);
                }
            }
            GUARDIAN_ANGEL | SEAL_ANGEL_1 | SEAL_ANGEL_2 => {
                if qs.get_memo_state_ex(ctx, 0) == 17 && ctx.has_quest_items(member, WHITE_FABRIC_1) {
                    let i0 = qs.get_memo_state_ex(ctx, 1) + ctx.get_random(100) + 100;
                    qs.set_memo_state_ex(ctx, 1, i0);
                    // NOTE(java-quirk): the stored value is added to `i0` again.
                    if qs.get_memo_state_ex(ctx, 1) + i0 > 750 {
                        ctx.give_items(member, BLOODED_FABRIC, 1);
                        ctx.take_items(member, WHITE_FABRIC_1, 1);
                        ctx.play_sound(player, QuestSound::ItemsoundQuestMiddle);
                        qs.set_memo_state_ex(ctx, 1, 0);
                    }
                }
            }
            _ => {}
        }
    }

    fn on_spawn(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc) {
        match npc.id {
            ARK_GUARDIAN_ELBEROTH => {
                ctx.world_mut()
                    .npc_variables_mut(npc.object_id)
                    .set_bool(I_QUEST0, false);
                ctx.broadcast_npc_say(
                    npc,
                    ChatType::NpcGeneral,
                    NpcStringId::ThatDoesnTBelongToYouDonTTouchIt,
                    &[],
                );
                ctx.start_quest_timer("DESPAWN", 600_000, Some(npc), None, false);
            }
            ARK_GUARDIAN_SHADOWFANG => {
                ctx.world_mut()
                    .npc_variables_mut(npc.object_id)
                    .set_bool(I_QUEST0, false);
                ctx.broadcast_npc_say(
                    npc,
                    ChatType::NpcGeneral,
                    NpcStringId::GetOutOfMySightYouInfidels,
                    &[],
                );
                ctx.start_quest_timer("DESPAWN", 600_000, Some(npc), None, false);
            }
            ANGEL_KILLER => {
                ctx.broadcast_npc_say(
                    npc,
                    ChatType::NpcGeneral,
                    NpcStringId::IHaveTheKeyWhyDonTYouComeAndTakeIt,
                    &[],
                );
                ctx.start_quest_timer("DESPAWN", 600_000, Some(npc), None, false);
            }
            _ => {}
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        if qs.is_created(ctx) {
            htmltext = if ctx.player_level(player) >= MIN_LEVEL {
                "30864-01.htm"
            } else {
                "30864-05.html"
            }
            .to_owned();
        } else if qs.is_started(ctx) {
            let saints = [BOOK_OF_SAINT, BLOOD_OF_SAINT, BOUGH_OF_SAINT];
            match npc.id {
                HANELLIN => match qs.get_memo_state(ctx) {
                    1 | 2 => {
                        let memo_state = qs.get_memo_state(ctx);
                        if memo_state == 1
                            && !ctx.has_quest_items(player, TITANS_POWERSTONE)
                            && !ctx.has_quest_items(player, SHELL_OF_MONSTERS)
                        {
                            htmltext = "30864-06.html".to_owned();
                        } else if (memo_state == 1 && ctx.has_quest_items(player, TITANS_POWERSTONE))
                            || ctx.has_quest_items(player, SHELL_OF_MONSTERS)
                            || memo_state == 2
                        {
                            if ctx.has_quest_items(player, SHELL_OF_MONSTERS) {
                                ctx.take_items(player, SHELL_OF_MONSTERS, 1);
                            }
                            if ctx.has_quest_items(player, TITANS_POWERSTONE) {
                                ctx.take_items(player, TITANS_POWERSTONE, 1);
                            }
                            qs.set_memo_state(ctx, 2);
                            htmltext = "30864-07.html".to_owned();
                        }
                    }
                    4 => match qs.get_memo_state_ex(ctx, 1) {
                        0 => {
                            qs.set_memo_state(ctx, 5);
                            htmltext = "30864-09.html".to_owned();
                            ctx.give_items(player, HANELLINS_1ST_LETTER, 1);
                            ctx.give_items(player, HANELLINS_2ND_LETTER, 1);
                            ctx.give_items(player, HANELLINS_3RD_LETTER, 1);
                            qs.set_cond(ctx, 5, true);
                        }
                        1 => {
                            qs.set_memo_state(ctx, 5);
                            htmltext = "30864-13.html".to_owned();
                            ctx.give_items(player, HANELLINS_1ST_LETTER, 1);
                            qs.set_cond(ctx, 6, true);
                        }
                        2 => {
                            qs.set_memo_state(ctx, 5);
                            htmltext = "30864-14.html".to_owned();
                            ctx.give_items(player, HANELLINS_2ND_LETTER, 1);
                            qs.set_cond(ctx, 7, true);
                        }
                        3 => {
                            qs.set_memo_state(ctx, 5);
                            htmltext = "30864-15.html".to_owned();
                            ctx.give_items(player, HANELLINS_3RD_LETTER, 1);
                            qs.set_cond(ctx, 8, true);
                        }
                        _ => {}
                    },
                    5 => {
                        if qs.get_memo_state_ex(ctx, 1) % 10 == 0 {
                            htmltext = "30864-16.html".to_owned();
                        } else {
                            match qs.get_memo_state_ex(ctx, 1) {
                                1 => htmltext = "30864-17.html".to_owned(),
                                2 => htmltext = "30864-18.html".to_owned(),
                                3 => htmltext = "30864-19.html".to_owned(),
                                _ => {}
                            }
                        }
                        // Custom part
                        if ctx.has_all_quest_items(player, &saints) {
                            ctx.take_items_many(player, 1, &saints);
                            qs.set_memo_state(ctx, 9);
                            htmltext = "30864-21.html".to_owned();
                            qs.set_cond(ctx, 22, true);
                        }
                    }
                    6 | 7 => htmltext = "30864-20.html".to_owned(),
                    8 => {
                        if ctx.has_all_quest_items(player, &saints) {
                            ctx.take_items_many(player, 1, &saints);
                            qs.set_memo_state(ctx, 9);
                            htmltext = "30864-21.html".to_owned();
                            qs.set_cond(ctx, 22, true);
                        } else {
                            match qs.get_memo_state_ex(ctx, 1) {
                                0 => htmltext = "30864-22.html".to_owned(),
                                1 => {
                                    if ctx.has_quest_items(player, BLOOD_OF_SAINT)
                                        && !ctx.has_at_least_one_quest_item(
                                            player,
                                            &[BOOK_OF_SAINT, BOUGH_OF_SAINT],
                                        )
                                    {
                                        htmltext = "30864-33.html".to_owned();
                                    } else if !ctx
                                        .has_all_quest_items(player, &[BLOOD_OF_SAINT, WHITE_FABRIC_2])
                                    {
                                        htmltext = "30864-36.html".to_owned();
                                    }
                                }
                                2 => {
                                    if ctx.has_quest_items(player, BOOK_OF_SAINT)
                                        && !ctx.has_at_least_one_quest_item(
                                            player,
                                            &[BLOOD_OF_SAINT, BOUGH_OF_SAINT],
                                        )
                                    {
                                        htmltext = "30864-34.html".to_owned();
                                    } else if !ctx
                                        .has_all_quest_items(player, &[BOOK_OF_SAINT, WHITE_FABRIC_2])
                                    {
                                        htmltext = "30864-37.html".to_owned();
                                    }
                                }
                                3 => {
                                    if ctx.has_quest_items(player, BOUGH_OF_SAINT)
                                        && !ctx.has_at_least_one_quest_item(
                                            player,
                                            &[BLOOD_OF_SAINT, BOOK_OF_SAINT],
                                        )
                                    {
                                        htmltext = "30864-35.html".to_owned();
                                    } else if !ctx
                                        .has_all_quest_items(player, &[BOUGH_OF_SAINT, WHITE_FABRIC_2])
                                    {
                                        htmltext = "30864-38.html".to_owned();
                                    }
                                }
                                _ => {}
                            }
                        }

                        if ctx.get_quest_items_count(player, WHITE_FABRIC_2) > 1
                            && qs.get_memo_state_ex(ctx, 1) > 0
                        {
                            htmltext = "30864-40.html".to_owned();
                        }
                        if ctx.get_quest_items_count(player, WHITE_FABRIC_2) == 1
                            && qs.get_memo_state_ex(ctx, 1) > 0
                            && !ctx.has_all_quest_items(player, &saints)
                        {
                            ctx.give_items(player, WHITE_FABRIC_1, 1);
                            ctx.take_items(player, WHITE_FABRIC_2, 1);
                            qs.set_memo_state(ctx, 10);
                            htmltext = "30864-41.html".to_owned();
                        }
                    }
                    9 => {
                        let antidote_count = ctx.get_quest_items_count(player, ANTIDOTE);
                        if antidote_count < 5 || !ctx.has_quest_items(player, GREATER_HEALING_POTION) {
                            htmltext = "30864-23.html".to_owned();
                        } else if antidote_count >= 5 && ctx.has_quest_items(player, GREATER_HEALING_POTION) {
                            if qs.get_memo_state_ex(ctx, 1) == 0 {
                                htmltext = "30864-24.html".to_owned();
                                ctx.give_items(player, WHITE_FABRIC_1, 1);
                                qs.set_memo_state(ctx, 10);
                                ctx.take_items(player, ANTIDOTE, 5);
                                ctx.take_items(player, GREATER_HEALING_POTION, 1);
                            } else {
                                ctx.give_items(player, WHITE_FABRIC_2, 3);
                                ctx.take_items(player, ANTIDOTE, 5);
                                ctx.take_items(player, GREATER_HEALING_POTION, 1);
                                qs.set_memo_state(ctx, 10);
                                qs.set_cond(ctx, 23, true);
                                htmltext = "30864-39.html".to_owned();
                            }
                        }
                    }
                    10 => {
                        if ctx.get_quest_items_count(player, WHITE_FABRIC_1) == 1 {
                            htmltext = "30864-25.html".to_owned();
                        }
                        if ctx.get_quest_items_count(player, WHITE_FABRIC_2) > 1
                            && qs.get_memo_state_ex(ctx, 1) > 0
                        {
                            htmltext = "30864-40.html".to_owned();
                        }
                        if ctx.get_quest_items_count(player, WHITE_FABRIC_2) == 1
                            && qs.get_memo_state_ex(ctx, 1) > 0
                            && !ctx.has_all_quest_items(player, &saints)
                        {
                            ctx.give_items(player, WHITE_FABRIC_1, 1);
                            ctx.take_items(player, WHITE_FABRIC_2, 1);
                            qs.set_memo_state(ctx, 10);
                            htmltext = "30864-41.html".to_owned();
                        }
                    }
                    11 => {
                        if ctx.get_quest_items_count(player, WHITE_FABRIC_1) == 1 {
                            if qs.get_memo_state_ex(ctx, 1) > 0 {
                                match qs.get_memo_state_ex(ctx, 1) {
                                    1 => ctx.give_adena(player, 43000, true),
                                    2 => ctx.give_adena(player, 4000, true),
                                    3 => ctx.give_adena(player, 13000, true),
                                    _ => {}
                                }
                                qs.set_memo_state_ex(ctx, 0, 12);
                                qs.set_memo_state_ex(ctx, 1, 100);
                                qs.set_cond(ctx, 24, true);
                                htmltext = "30864-27.html".to_owned();
                            } else if qs.get_memo_state_ex(ctx, 1) == 0 {
                                htmltext = "30864-28.html".to_owned();
                            }
                        }
                    }
                    12 => {
                        if ctx.get_quest_items_count(player, WHITE_FABRIC_1) == 1 {
                            htmltext = "30864-31.html".to_owned();
                        }
                    }
                    13 => {
                        if ctx.get_quest_items_count(player, WHITE_FABRIC_1) == 1 {
                            htmltext = "30864-32.html".to_owned();
                        }
                    }
                    14 => {
                        get_reward(ctx, player);
                        qs.set_memo_state(ctx, 15);
                        htmltext = "30864-42.html".to_owned();
                    }
                    15 => htmltext = "30864-43.html".to_owned(),
                    16 => {
                        if ctx.has_quest_items(player, BLOODED_FABRIC) {
                            ctx.give_items(player, WHITE_FABRIC_1, 9);
                        } else {
                            ctx.give_items(player, WHITE_FABRIC_1, 10);
                        }
                        qs.set_memo_state(ctx, 17); // Custom line
                        qs.set_memo_state_ex(ctx, 0, 17);
                        qs.set_memo_state_ex(ctx, 1, 0);
                        qs.set_cond(ctx, 26, true);
                        htmltext = "30864-44.html".to_owned();
                    }
                    17 => {
                        if ctx.has_quest_items(player, WHITE_FABRIC_1) {
                            htmltext = "30864-45.html".to_owned();
                        } else {
                            let blooded_fabric_count = ctx.get_quest_items_count(player, BLOODED_FABRIC);
                            if blooded_fabric_count >= 10 {
                                htmltext = "30864-46.html".to_owned();
                            } else {
                                ctx.give_adena(player, (blooded_fabric_count * 1000) + 4000, true);
                                ctx.take_items(player, BLOODED_FABRIC, -1);
                                qs.exit_quest(ctx, true, true);
                                htmltext = "30864-48.html".to_owned();
                            }
                        }
                    }
                    18 => {
                        let memo_state_ex = qs.get_memo_state_ex(ctx, 1);
                        if (memo_state_ex % 10) < 7 {
                            let mut i1 = 0;
                            let mut i2 = 0;
                            let mut i0 = memo_state_ex % 10;
                            if i0 >= 4 {
                                i1 += 6;
                                i0 -= 4;
                                i2 += 1;
                            }
                            if i0 >= 2 {
                                i0 -= 2;
                                i1 += 1;
                                i2 += 1;
                            }
                            if i0 >= 1 {
                                i1 += 3;
                                i2 += 1;
                                i0 -= 1;
                            }
                            if i0 == 0 {
                                let blooded_fabric_count = ctx.get_quest_items_count(player, BLOODED_FABRIC);
                                if blooded_fabric_count + i1 >= 10 {
                                    htmltext = "30864-52.html".to_owned();
                                } else {
                                    htmltext = "30864-53.html".to_owned();
                                    if i2 == 2 {
                                        ctx.give_adena(player, 24000, true);
                                    } else if i2 == 1 {
                                        ctx.give_adena(player, 12000, true);
                                    }
                                    qs.exit_quest(ctx, true, true);
                                }
                            }
                        } else if memo_state_ex % 10 == 7 {
                            htmltext = "30864-54.html".to_owned();
                            qs.set_cond(ctx, 28, true);
                            get_reward(ctx, player);
                            qs.set_memo_state(ctx, 19);
                        }
                    }
                    19 => htmltext = "30864-49.html".to_owned(),
                    _ => {}
                },
                IASON_HEINE => {
                    if qs.get_memo_state_ex(ctx, 0) == 18 {
                        if (qs.get_memo_state_ex(ctx, 1) % 8) < 4 {
                            if ctx.get_quest_items_count(player, BLOODED_FABRIC) >= 6 {
                                ctx.take_items(player, BLOODED_FABRIC, 6);
                                qs.set_memo_state_ex(ctx, 1, qs.get_memo_state_ex(ctx, 1) + 4);
                                htmltext = "30969-01.html".to_owned();
                            } else {
                                htmltext = "30969-02.html".to_owned();
                            }
                        } else {
                            htmltext = "30969-03.html".to_owned();
                        }
                    }
                }
                HOLY_ARK_OF_SECRECY_1 => {
                    if ctx.has_quest_items(player, FIRST_KEY_OF_ARK) {
                        ctx.give_items(player, BLOOD_OF_SAINT, 1);
                        ctx.clear_radar(player);
                        if qs.get_memo_state_ex(ctx, 1) % 10 == 0 {
                            if ctx.has_all_quest_items(player, &[BOOK_OF_SAINT, BOUGH_OF_SAINT]) {
                                qs.set_cond(ctx, 21, true);
                            }
                        } else {
                            qs.set_cond(ctx, 20, true);
                        }
                        ctx.take_items(player, FIRST_KEY_OF_ARK, 1);
                        qs.set_memo_state_ex(ctx, 1, qs.get_memo_state_ex(ctx, 1) - 20);
                        if (((qs.get_memo_state_ex(ctx, 1) - 20) % 100) / 10) == 0 {
                            qs.set_memo_state_ex(ctx, 0, qs.get_memo_state_ex(ctx, 0) + 1);
                        }
                        if ((qs.get_memo_state_ex(ctx, 1) - 20) % 10) == 1 {
                            qs.set_memo_state_ex(ctx, 0, 8);
                        }
                        htmltext = "30977-01.html".to_owned();
                    } else if qs.get_memo_state(ctx) <= 8
                        && ((qs.get_memo_state_ex(ctx, 1) % 100) / 10) == 0
                        && ctx.has_quest_items(player, BLOOD_OF_SAINT)
                    {
                        htmltext = "30977-02.html".to_owned();
                    } else if qs.get_memo_state(ctx) < 8
                        && ((qs.get_memo_state_ex(ctx, 1) % 100) / 10) == 1
                        && !ctx.has_quest_items(player, BLOOD_OF_SAINT)
                    {
                        htmltext = "30977-03.html".to_owned();
                    }
                }
                HOLY_ARK_OF_SECRECY_2 => {
                    if ctx.has_quest_items(player, SECOND_KEY_OF_ARK) {
                        ctx.give_items(player, BOOK_OF_SAINT, 1);
                        ctx.take_items(player, SECOND_KEY_OF_ARK, 1);
                        ctx.clear_radar(player);
                        if qs.get_memo_state_ex(ctx, 1) % 10 == 0 {
                            if ctx.has_all_quest_items(player, &[BLOOD_OF_SAINT, BOUGH_OF_SAINT]) {
                                qs.set_cond(ctx, 21, true);
                            }
                        } else {
                            qs.set_cond(ctx, 12, true);
                        }
                        qs.set_memo_state_ex(ctx, 1, qs.get_memo_state_ex(ctx, 1) - 200);
                        if (((qs.get_memo_state_ex(ctx, 1) - 200) % 1000) / 100) == 0 {
                            qs.set_memo_state_ex(ctx, 0, qs.get_memo_state_ex(ctx, 0) + 1);
                        }
                        if ((qs.get_memo_state_ex(ctx, 1) - 200) % 10) == 2 {
                            qs.set_memo_state_ex(ctx, 0, 8);
                        }
                        htmltext = "30978-01.html".to_owned();
                    } else if qs.get_memo_state(ctx) < 8 && ((qs.get_memo_state_ex(ctx, 1) % 1000) / 100) == 1
                    {
                        htmltext = "30978-02.html".to_owned();
                        if qs.get_memo_state_ex(ctx, 1) % 10 != 0 {
                            qs.set_cond(ctx, 10, true);
                        }
                        // TODO (Adry_85): Missing Question Mark
                        ctx.play_sound(player, QuestSound::ItemsoundQuestMiddle);
                        spawn_at_player(ctx, ARK_GUARDIAN_ELBEROTH, player); // ark_guardian_elberoth
                    } else if qs.get_memo_state(ctx) <= 8
                        && ((qs.get_memo_state_ex(ctx, 1) % 1000) / 100) == 0
                        && ctx.has_quest_items(player, BOOK_OF_SAINT)
                    {
                        htmltext = "30978-03.html".to_owned();
                    }
                }
                HOLY_ARK_OF_SECRECY_3 => {
                    if ctx.has_quest_items(player, THIRD_KEY_OF_ARK) {
                        ctx.give_items(player, BOUGH_OF_SAINT, 1);
                        ctx.take_items(player, THIRD_KEY_OF_ARK, 1);
                        ctx.clear_radar(player);
                        if qs.get_memo_state_ex(ctx, 1) % 10 == 0 {
                            if ctx.has_all_quest_items(player, &[BLOOD_OF_SAINT, BOOK_OF_SAINT]) {
                                qs.set_cond(ctx, 21, true);
                            }
                        } else {
                            qs.set_cond(ctx, 16, true);
                        }
                        qs.set_memo_state_ex(ctx, 1, qs.get_memo_state_ex(ctx, 1) - 2000);
                        if (((qs.get_memo_state_ex(ctx, 1) - 2000) % 10000) / 1000) == 0 {
                            qs.set_memo_state_ex(ctx, 0, qs.get_memo_state_ex(ctx, 0) + 1);
                        }
                        if ((qs.get_memo_state_ex(ctx, 1) - 2000) % 10) == 3 {
                            qs.set_memo_state_ex(ctx, 0, 8);
                        }
                        htmltext = "30979-01.html".to_owned();
                    } else if qs.get_memo_state(ctx) < 8
                        && ((qs.get_memo_state_ex(ctx, 1) % 10000) / 1000) == 1
                    {
                        htmltext = "30979-02.html".to_owned();
                        if qs.get_memo_state_ex(ctx, 1) % 10 != 0 {
                            qs.set_cond(ctx, 14, true);
                        }
                        // TODO (Adry_85): Missing Question Mark
                        ctx.play_sound(player, QuestSound::ItemsoundQuestMiddle);
                        spawn_at_player(ctx, ARK_GUARDIAN_SHADOWFANG, player); // ark_guardian_shadowfang
                    } else if qs.get_memo_state(ctx) <= 8
                        && ((qs.get_memo_state_ex(ctx, 1) % 10000) / 1000) == 0
                        && ctx.has_quest_items(player, BOUGH_OF_SAINT)
                    {
                        htmltext = "30979-03.html".to_owned();
                    }
                }
                ARK_GUARDIANS_CORPSE => {
                    if qs.get_memo_state(ctx) < 8
                        && ((qs.get_memo_state_ex(ctx, 1) % 100) / 10) == 1
                        && !ctx.has_quest_items(player, FIRST_KEY_OF_ARK)
                        && !ctx.has_quest_items(player, BLOOD_OF_SAINT)
                    {
                        htmltext = "30980-02.html".to_owned();
                        ctx.clear_radar(player);
                        if qs.get_memo_state_ex(ctx, 1) % 10 != 0 {
                            qs.set_cond(ctx, 18, true);
                        }
                        // TODO (Adry_85): Missing Question Mark
                        ctx.play_sound(player, QuestSound::ItemsoundQuestMiddle);
                        spawn_at_player(ctx, ANGEL_KILLER, player); // angel_killer
                    } else if qs.get_memo_state(ctx) < 8
                        && ((qs.get_memo_state_ex(ctx, 1) % 100) / 10) == 2
                        && !ctx.has_quest_items(player, FIRST_KEY_OF_ARK)
                        && !ctx.has_quest_items(player, BLOOD_OF_SAINT)
                    {
                        ctx.give_items(player, FIRST_KEY_OF_ARK, 1);
                        ctx.add_radar(player, -418, 44174, -3568);
                        htmltext = "30980-03.html".to_owned();
                    } else if ctx.has_at_least_one_quest_item(player, &[FIRST_KEY_OF_ARK, BLOOD_OF_SAINT]) {
                        htmltext = "30980-01.html".to_owned();
                    }
                }
                CLAUDIA_ATHEBALDT => {
                    if ctx.has_quest_items(player, HANELLINS_2ND_LETTER) {
                        let i0 = qs.get_memo_state_ex(ctx, 1) + 100;
                        if i0 % 10 == 0 {
                            ctx.add_radar(player, 181472, 7158, -2725);
                        } else {
                            qs.set_cond(ctx, 9, true);
                        }
                        qs.set_memo_state_ex(ctx, 1, i0);
                        ctx.take_items(player, HANELLINS_2ND_LETTER, 1);
                        htmltext = "31001-01.html".to_owned();
                    } else if qs.get_memo_state(ctx) < 8
                        && ((qs.get_memo_state_ex(ctx, 1) % 1000) / 100) == 1
                        && !ctx.has_quest_items(player, SECOND_KEY_OF_ARK)
                    {
                        // retail typo
                        if qs.get_memo_state_ex(ctx, 1) % 10 == 0 {
                            ctx.add_radar(player, 181472, 7158, -2725);
                        }
                        htmltext = "31001-03.html".to_owned();
                    } else if ctx.has_quest_items(player, SECOND_KEY_OF_ARK) {
                        if qs.get_memo_state_ex(ctx, 1) % 10 == 0 {
                            ctx.add_radar(player, 181472, 7158, -2725);
                        }
                        htmltext = "31001-04.html".to_owned();
                    } else if ctx.has_quest_items(player, BOOK_OF_SAINT) {
                        htmltext = "31001-05.html".to_owned();
                    }
                }
                HARNE => {
                    if ctx.has_quest_items(player, HANELLINS_1ST_LETTER) {
                        let i0 = qs.get_memo_state_ex(ctx, 1) + 10;
                        if i0 % 10 == 0 {
                            ctx.add_radar(player, 2908, 44128, -2712);
                        } else {
                            qs.set_cond(ctx, 17, true);
                        }
                        qs.set_memo_state_ex(ctx, 1, i0);
                        ctx.take_items(player, HANELLINS_1ST_LETTER, 1);
                        htmltext = "30144-01.html".to_owned();
                    } else if qs.get_memo_state(ctx) < 8
                        && ((qs.get_memo_state_ex(ctx, 1) % 100) / 10) == 1
                        && !ctx.has_quest_items(player, FIRST_KEY_OF_ARK)
                    {
                        // retail typo
                        if qs.get_memo_state_ex(ctx, 1) % 10 == 0 {
                            ctx.add_radar(player, 2908, 44128, -2712);
                        }
                        htmltext = "30144-03.html".to_owned();
                    } else if ctx.has_quest_items(player, FIRST_KEY_OF_ARK) {
                        if qs.get_memo_state_ex(ctx, 1) % 10 == 0 {
                            ctx.add_radar(player, 2908, 44128, -2712);
                        }
                        htmltext = "30144-04.html".to_owned();
                    } else if ctx.has_quest_items(player, BLOOD_OF_SAINT) {
                        htmltext = "30144-05.html".to_owned();
                    }
                }
                MARTIEN => {
                    if ctx.has_quest_items(player, HANELLINS_3RD_LETTER) {
                        let i0 = qs.get_memo_state_ex(ctx, 1) + 1000;
                        if i0 % 10 == 0 {
                            ctx.add_radar(player, 50693, 158674, 376);
                        } else {
                            qs.set_cond(ctx, 13, true);
                        }
                        qs.set_memo_state_ex(ctx, 1, i0);
                        ctx.take_items(player, HANELLINS_3RD_LETTER, 1);
                        htmltext = "30645-01.html".to_owned();
                    } else if qs.get_memo_state(ctx) < 8
                        && ((qs.get_memo_state_ex(ctx, 1) % 10000) / 1000) == 1
                        && !ctx.has_quest_items(player, THIRD_KEY_OF_ARK)
                    {
                        // retail typo
                        if qs.get_memo_state_ex(ctx, 1) % 10 == 0 {
                            ctx.add_radar(player, 50693, 158674, 376);
                        }
                        htmltext = "30645-03.html".to_owned();
                    } else if ctx.has_quest_items(player, THIRD_KEY_OF_ARK) {
                        if qs.get_memo_state_ex(ctx, 1) % 10 == 0 {
                            ctx.add_radar(player, 50693, 158674, 376);
                        }
                        htmltext = "30645-04.html".to_owned();
                    } else if ctx.has_quest_items(player, BOUGH_OF_SAINT) {
                        htmltext = "30645-05.html".to_owned();
                    }
                }
                SIR_GUSTAV_ATHEBALDT => {
                    if qs.get_memo_state_ex(ctx, 0) == 18 {
                        if qs.get_memo_state_ex(ctx, 1) % 2 == 0 {
                            if ctx.get_quest_items_count(player, BLOODED_FABRIC) >= 3 {
                                ctx.take_items(player, BLOODED_FABRIC, 3);
                                qs.set_memo_state_ex(ctx, 1, qs.get_memo_state_ex(ctx, 1) + 1);
                                htmltext = "30760-01.html".to_owned();
                            } else {
                                htmltext = "30760-02.html".to_owned();
                            }
                        } else if qs.get_memo_state_ex(ctx, 1) % 2 == 1 {
                            htmltext = "30760-03.html".to_owned();
                        }
                    }
                }
                HARDIN => {
                    if qs.get_memo_state_ex(ctx, 0) == 18 {
                        if (qs.get_memo_state_ex(ctx, 1) % 4) < 2 {
                            if ctx.get_quest_items_count(player, BLOODED_FABRIC) >= 1 {
                                ctx.take_items(player, BLOODED_FABRIC, 1);
                                qs.set_memo_state_ex(ctx, 1, qs.get_memo_state_ex(ctx, 1) + 2);
                                htmltext = "30832-01.html".to_owned();
                            } else if ctx.get_quest_items_count(player, BLOODED_FABRIC) < 3 {
                                htmltext = "30832-02.html".to_owned();
                            }
                        } else {
                            htmltext = "30832-03.html".to_owned();
                        }
                    }
                }
                _ => {}
            }
        }
        Some(htmltext)
    }
}

/// Java `addSpawn(npcId, player.getX(), player.getY(), player.getZ(), 0, false, 0, false)`.
fn spawn_at_player(ctx: &mut ScriptCtx<'_>, npc_id: i32, player: ObjectId) {
    if let Some((x, y, z)) = ctx.world().position_of(player) {
        ctx.add_spawn(npc_id, x, y, z, 0, false, 0, 0);
    }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::script::race;
    use l2_world::{QuestLoad, ScriptRegistry};

    use super::*;
    use crate::testing::{Pages, ScriptTest, item, player_data, rows};

    const NAME: &str = "Q00348_AnArrogantSearch";

    const NPCS: &str = r#"
        <npc id="30144" type="Folk" level="50" name="Harne"/>
        <npc id="30864" type="Folk" level="50" name="Hanellin"/>
        <npc id="30978" type="Folk" level="50" name="Holy Ark of Secrecy 2"/>
        <npc id="20647" type="Monster" level="40" name="Yintzu"/>
        <npc id="27182" type="Monster" level="40" name="Ark Guardian Elberoth"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="4287" type="EtcItem" name="Titan's Powerstone"><set name="is_questitem" val="true"/></item>
        <item id="4288" type="EtcItem" name="Letter 1"><set name="is_questitem" val="true"/></item>
        <item id="4289" type="EtcItem" name="Letter 2"><set name="is_questitem" val="true"/></item>
        <item id="4290" type="EtcItem" name="Letter 3"><set name="is_questitem" val="true"/></item>
        <item id="4292" type="EtcItem" name="Second Key"><set name="is_questitem" val="true"/></item>
        <item id="4294" type="EtcItem" name="White Fabric"><set name="is_questitem" val="true"/></item>
        <item id="4295" type="EtcItem" name="Blooded Fabric"><set name="is_questitem" val="true"/></item>
        <item id="14857" type="EtcItem" name="Shell of Monsters"><set name="is_questitem" val="true"/></item>
        <item id="4109" type="Weapon" name="Kris Edge"/>
        <item id="4119" type="Weapon" name="Demon's Dagger Edge"/>
        <item id="1879" type="EtcItem" name="Cokes"><set name="is_stackable" val="true"/></item>
        <item id="1888" type="EtcItem" name="Synthetic Cokes"><set name="is_stackable" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid, Oid, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00348AnArrogantSearch);
        let pages = Pages::of(&Q00348AnArrogantSearch)
            .page("30864-01.htm", &["30864-02.htm"])
            .page("30864-02.htm", &["30864-03.htm"])
            .page("30864-03.htm", &["30864-04.htm"])
            .plain(&[
                "30864-04.htm",
                "30864-05.html",
                "30864-06.html",
                "30864-09.html",
                "30864-13.html",
            ])
            .page("30864-07.html", &["30864-08.html"])
            .page("30864-08.html", &["30864-09.html"])
            .plain(&["30864-42.html", "30864-16.html"])
            .page("30144-01.html", &["30144-02.html"])
            .plain(&["30144-02.html", "30144-03.html", "30144-04.html", "30144-05.html"])
            .plain(&["30978-01.html", "30978-02.html", "30978-03.html"])
            .npcs(&[HANELLIN, HARNE, HOLY_ARK_OF_SECRECY_2]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let hanellin = t.spawn(HANELLIN, 20);
        let harne = t.spawn(HARNE, 40);
        let ark = t.spawn(HOLY_ARK_OF_SECRECY_2, 60);
        (t, hanellin, harne, ark)
    }

    fn started(memo: i32, ex0: i32, ex1: i32, items: Vec<l2_world::ItemData>) -> l2_world::PlayerData {
        let mut data = player_data(1, 70, items);
        data.profile.quests = QuestLoad {
            states: vec![(NAME.to_owned(), Some("Started".to_owned()))],
            variables: vec![
                (NAME.to_owned(), "cond".to_owned(), Some("5".to_owned())),
                (NAME.to_owned(), "memoState".to_owned(), Some(memo.to_string())),
                (NAME.to_owned(), "memoStateEx0".to_owned(), Some(ex0.to_string())),
                (NAME.to_owned(), "memoStateEx1".to_owned(), Some(ex1.to_string())),
            ],
        };
        data
    }

    #[test]
    fn a_level_59_character_is_sent_away() {
        let (mut t, hanellin, ..) = setup();
        t.enter(1, 59, vec![]);
        assert!(t.quest_page(1, hanellin).contains("30864-05.html"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn start_and_the_three_letters() {
        let (mut t, hanellin, ..) = setup();
        t.enter(1, 60, vec![item(5, SHELL_OF_MONSTERS, 1)]);
        assert!(t.quest_page(1, hanellin).contains("30864-01.htm"));
        assert!(t.event(1, NAME, "30864-02.htm").contains("30864-02.htm"));
        assert!(t.event(1, NAME, "30864-03.htm").contains("30864-03.htm"));
        assert!(t.event(1, NAME, "30864-04.htm").contains("30864-04.htm"));
        let writes = t.take_writes();
        assert_eq!(writes[0], rows::insert(NAME, "memoState", "1"));
        assert!(writes.contains(&rows::insert(NAME, "<state>", "Started")));
        assert_eq!(writes.last(), Some(&rows::update(NAME, "cond", "2")));

        // The shell makes Hanellin take it (memo 2), then the letters follow (memo 4, then 5).
        assert!(t.quest_page(1, hanellin).contains("30864-07.html"));
        assert_eq!(t.count(1, SHELL_OF_MONSTERS), 0);
        assert!(t.event(1, NAME, "30864-08.html").contains("30864-08.html"));
        assert!(t.event(1, NAME, "30864-09.html").contains("30864-09.html"));
        assert_eq!((t.count(1, 4288), t.count(1, 4289), t.count(1, 4290)), (1, 1, 1));
        t.take_writes();
        // Memo state 5 with no key digits: "30864-16".
        assert!(t.quest_page(1, hanellin).contains("30864-16.html"));
    }

    #[test]
    fn harne_takes_the_first_letter_and_counts_it() {
        let (mut t, _, harne, _) = setup();
        t.enter_with(started(5, 4, 0, vec![item(5, HANELLINS_1ST_LETTER, 1)]));
        assert!(t.quest_page(1, harne).contains("30144-01.html"));
        assert_eq!(t.count(1, HANELLINS_1ST_LETTER), 0);
        // ex1 + 10 has units 0: a radar marker, no cond change.
        assert_eq!(t.take_writes(), [rows::update(NAME, "memoStateEx1", "10")]);
        assert!(t.quest_page(1, harne).contains("30144-03.html"));
    }

    #[test]
    fn yintzu_drops_the_shell_at_memo_1_only() {
        let (mut t, ..) = setup();
        t.enter_with(started(1, 0, 0, vec![]));
        let mob = t.spawn(YINTZU, 60);
        t.kill(1, mob);
        assert_eq!(t.count(1, SHELL_OF_MONSTERS), 1);
        let mob = t.spawn(YINTZU, 60);
        t.kill(1, mob);
        assert_eq!(t.count(1, SHELL_OF_MONSTERS), 1);
    }

    #[test]
    fn the_reward_depends_on_class_and_level() {
        let (mut t, hanellin, ..) = setup();
        let mut data = started(14, 14, 0, vec![]);
        data.race = race::HUMAN;
        data.class_id = player_class::TREASURE_HUNTER;
        data.base_class = player_class::TREASURE_HUNTER;
        data.level = 65;
        t.enter_with(data);
        assert!(t.quest_page(1, hanellin).contains("30864-42.html"));
        // Treasure hunter, below level 69: Kris edge and 2 synthetic cokes.
        assert_eq!(t.count(1, 4109), 1);
        assert_eq!(t.count(1, 1888), 2);
        assert_eq!(t.take_writes(), [rows::update(NAME, "memoState", "15")]);
    }

    #[test]
    fn the_second_ark_calls_its_guardian() {
        let (mut t, _, _, ark) = setup();
        // Claudia's letter was delivered (hundreds digit 1), units 0.
        t.enter_with(started(5, 4, 100, vec![]));
        assert!(t.quest_page(1, ark).contains("30978-02.html"));
        assert_eq!(t.npcs_of(ARK_GUARDIAN_ELBEROTH).len(), 1);
        // Nobody kills it: the spawn hook's DESPAWN timer (10 minutes) removes it.
        t.wait_ms(600_100);
        assert!(t.npcs_of(ARK_GUARDIAN_ELBEROTH).is_empty());
    }
}
