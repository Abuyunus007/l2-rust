//! Quest 234: Fate's Whisper (Maestro Leorin forges a B-grade weapon into an A-grade one: the
//! coffers of Cabrio, Kernon, Golkonda and Hallate, Cliff's varnish, Ferris' hammer, Kaspar's
//! fabrics and the pipette knife that is used on Baium).
// Java: dist/game/data/scripts/quests/Q00234_FatesWhisper/Q00234_FatesWhisper.java
//
// Pattern: the memo state drives the quest; the html pages send numeric events (`Script
// Q00234_FatesWhisper 21`), so `on_event` parses the event as a number; timed coffers
// (`startQuestTimer` from `onSpawn`, `decayMe`) and a quest item weapon checked in `onAttack`.

use l2_protocol_game::NpcStringId;
use l2_protocol_game::server::{ChatType, SocialAction};
use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED};
use l2_world::{QuestSound, Script, ScriptCtx, ScriptNpc, ScriptSetup, SkillKey};

// NPCs
const ZENKIN: i32 = 30178;
const CLIFF: i32 = 30182;
const MASTER_KASPAR: i32 = 30833;
const HEAD_BLACKSMITH_FERRIS: i32 = 30847;
const MAESTRO_LEORIN: i32 = 31002;
const COFFER_OF_THE_DEAD: i32 = 31027;
const CHEST_OF_KERNON: i32 = 31028;
const CHEST_OF_GOLKONDA: i32 = 31029;
const CHEST_OF_HALLATE: i32 = 31030;

// Quest Items
const Q_BLOODY_FABRIC_Q0234: i32 = 14361;
const Q_WHITE_FABRIC_Q0234: i32 = 14362;
const Q_STAR_OF_DESTINY: i32 = 5011;
const Q_PIPETTE_KNIFE: i32 = 4665;
const Q_REIRIAS_SOULORB: i32 = 4666;
const Q_INFERNIUM_SCEPTER_1: i32 = 4667;
const Q_INFERNIUM_SCEPTER_2: i32 = 4668;
const Q_INFERNIUM_SCEPTER_3: i32 = 4669;
const Q_MAESTRO_REORINS_HAMMER: i32 = 4670;
const Q_MAESTRO_REORINS_MOLD: i32 = 4671;
const Q_INFERNIUM_VARNISH: i32 = 4672;
const Q_RED_PIPETTE_KNIFE: i32 = 4673;

// Other Items
const CRYSTAL_B: i32 = 1460;

// Monsters
const PLATINUM_TRIBE_GRUNT: i32 = 20823;
const PLATINUM_TRIBE_ARCHER: i32 = 20826;
const PLATINUM_TRIBE_WARRIOR: i32 = 20827;
const PLATINUM_TRIBE_SHAMAN: i32 = 20828;
const PLATINUM_TRIBE_LORD: i32 = 20829;
const GUARDIAN_ANGEL: i32 = 20830;
const SEAL_ANGEL: i32 = 20831;
const SEAL_ANGEL_R: i32 = 20860;

const DOMB_DEATH_CABRIO: i32 = 25035;
const KERNON: i32 = 25054;
const GOLKONDA_LONGHORN: i32 = 25126;
const HALLATE_THE_DEATH_LORD: i32 = 25220;
const BAIUM: i32 = 29020;

// B-grade
const SWORD_OF_DAMASCUS: i32 = 79;
const SWORD_OF_DAMASCUS_FOCUS: i32 = 4717;
const SWORD_OF_DAMASCUS_CRT_DAMAGE: i32 = 4718;
const SWORD_OF_DAMASCUS_HASTE: i32 = 4719;
const HAZARD_BOW: i32 = 287;
const HAZARD_BOW_GUIDENCE: i32 = 4828;
const HAZARD_BOW_QUICKRECOVERY: i32 = 4829;
const HAZARD_BOW_CHEAPSHOT: i32 = 4830;
const LANCIA: i32 = 97;
const LANCIA_ANGER: i32 = 4858;
const LANCIA_CRT_STUN: i32 = 4859;
const LANCIA_LONGBLOW: i32 = 4860;
const ART_OF_BATTLE_AXE: i32 = 175;
const ART_OF_BATTLE_AXE_HEALTH: i32 = 4753;
const ART_OF_BATTLE_AXE_RSK_FOCUS: i32 = 4754;
const ART_OF_BATTLE_AXE_HASTE: i32 = 4755;
const STAFF_OF_EVIL_SPRIT: i32 = 210;
const STAFF_OF_EVIL_SPRIT_MAGICFOCUS: i32 = 4900;
const STAFF_OF_EVIL_SPRIT_MAGICBLESSTHEBODY: i32 = 4901;
const STAFF_OF_EVIL_SPRIT_MAGICPOISON: i32 = 4902;
const DEMONS_SWORD: i32 = 234;
const DEMONS_SWORD_CRT_BLEED: i32 = 4780;
const DEMONS_SWORD_CRT_POISON: i32 = 4781;
const DEMONS_SWORD_MIGHTMOTAL: i32 = 4782;
const BELLION_CESTUS: i32 = 268;
const BELLION_CESTUS_CRT_DRAIN: i32 = 4804;
const BELLION_CESTUS_CRT_POISON: i32 = 4805;
const BELLION_CESTUS_RSK_HASTE: i32 = 4806;
const DEADMANS_GLORY: i32 = 171;
const DEADMANS_GLORY_ANGER: i32 = 4750;
const DEADMANS_GLORY_HEALTH: i32 = 4751;
const DEADMANS_GLORY_HASTE: i32 = 4752;
const SAMURAI_LONGSWORD_SAMURAI_LONGSWORD: i32 = 2626;
const GUARDIANS_SWORD: i32 = 7883;
const GUARDIANS_SWORD_CRT_DRAIN: i32 = 8105;
const GUARDIANS_SWORD_HEALTH: i32 = 8106;
const GUARDIANS_SWORD_CRT_BLEED: i32 = 8107;
const TEARS_OF_WIZARD: i32 = 7889;
const TEARS_OF_WIZARD_ACUMEN: i32 = 8117;
const TEARS_OF_WIZARD_MAGICPOWER: i32 = 8118;
const TEARS_OF_WIZARD_UPDOWN: i32 = 8119;
const STAR_BUSTER: i32 = 7901;
const STAR_BUSTER_HEALTH: i32 = 8132;
const STAR_BUSTER_HASTE: i32 = 8133;
const STAR_BUSTER_RSK_FOCUS: i32 = 8134;
const BONE_OF_KAIM_VANUL: i32 = 7893;
const BONE_OF_KAIM_VANUL_MANAUP: i32 = 8144;
const BONE_OF_KAIM_VANUL_MAGICSILENCE: i32 = 8145;
const BONE_OF_KAIM_VANUL_UPDOWN: i32 = 8146;

// A-grade
const TALLUM_BLADE: i32 = 80;
const CARNIUM_BOW: i32 = 288;
const HALBARD: i32 = 98;
const ELEMENTAL_SWORD: i32 = 150;
const DASPARIONS_STAFF: i32 = 212;
const BLOODY_ORCHID: i32 = 235;
const BLOOD_TORNADO: i32 = 269;
const METEOR_SHOWER: i32 = 2504;
const KSHANBERK_KSHANBERK: i32 = 5233;
const INFERNO_MASTER: i32 = 7884;
const EYE_OF_SOUL: i32 = 7894;
const HAMMER_OF_DESTROYER: i32 = 7899;

/// Java `Q00234_FatesWhisper`.
pub struct Q00234FatesWhisper;

/// The four item ids of each memo state that `calculateReward` accepts (B-grade weapon and its
/// three special-ability variants); Java passes `0` for the missing ones of the samurai longsword.
fn reward_items(memo_state: i32) -> Option<[i32; 4]> {
    Some(match memo_state {
        11 => [
            SWORD_OF_DAMASCUS,
            SWORD_OF_DAMASCUS_FOCUS,
            SWORD_OF_DAMASCUS_CRT_DAMAGE,
            SWORD_OF_DAMASCUS_HASTE,
        ],
        12 => [
            HAZARD_BOW,
            HAZARD_BOW_GUIDENCE,
            HAZARD_BOW_QUICKRECOVERY,
            HAZARD_BOW_CHEAPSHOT,
        ],
        13 => [LANCIA, LANCIA_ANGER, LANCIA_CRT_STUN, LANCIA_LONGBLOW],
        14 => [
            ART_OF_BATTLE_AXE,
            ART_OF_BATTLE_AXE_HEALTH,
            ART_OF_BATTLE_AXE_RSK_FOCUS,
            ART_OF_BATTLE_AXE_HASTE,
        ],
        15 => [
            STAFF_OF_EVIL_SPRIT,
            STAFF_OF_EVIL_SPRIT_MAGICFOCUS,
            STAFF_OF_EVIL_SPRIT_MAGICBLESSTHEBODY,
            STAFF_OF_EVIL_SPRIT_MAGICPOISON,
        ],
        16 => [
            DEMONS_SWORD,
            DEMONS_SWORD_CRT_BLEED,
            DEMONS_SWORD_CRT_POISON,
            DEMONS_SWORD_MIGHTMOTAL,
        ],
        17 => [
            BELLION_CESTUS,
            BELLION_CESTUS_CRT_DRAIN,
            BELLION_CESTUS_CRT_POISON,
            BELLION_CESTUS_RSK_HASTE,
        ],
        18 => [
            DEADMANS_GLORY,
            DEADMANS_GLORY_ANGER,
            DEADMANS_GLORY_HEALTH,
            DEADMANS_GLORY_HASTE,
        ],
        19 => [SAMURAI_LONGSWORD_SAMURAI_LONGSWORD, 0, 0, 0],
        41 => [
            GUARDIANS_SWORD,
            GUARDIANS_SWORD_CRT_DRAIN,
            GUARDIANS_SWORD_HEALTH,
            GUARDIANS_SWORD_CRT_BLEED,
        ],
        42 => [
            TEARS_OF_WIZARD,
            TEARS_OF_WIZARD_ACUMEN,
            TEARS_OF_WIZARD_MAGICPOWER,
            TEARS_OF_WIZARD_UPDOWN,
        ],
        43 => [
            STAR_BUSTER,
            STAR_BUSTER_HEALTH,
            STAR_BUSTER_HASTE,
            STAR_BUSTER_RSK_FOCUS,
        ],
        44 => [
            BONE_OF_KAIM_VANUL,
            BONE_OF_KAIM_VANUL_MANAUP,
            BONE_OF_KAIM_VANUL_MAGICSILENCE,
            BONE_OF_KAIM_VANUL_UPDOWN,
        ],
        _ => return None,
    })
}

/// Java `hasAtLeastOneQuestItem(player, item1, item2, item3, item4)` (an item id 0 is no item).
fn has_one_of(ctx: &ScriptCtx<'_>, player: ObjectId, items: &[i32; 4]) -> bool {
    items.iter().any(|&i| i != 0 && ctx.has_quest_items(player, i))
}

/// Java `calculateReward`/`getReward`: the A-grade weapon and the Star of Destiny for the
/// B-grade weapon (or one of its variants) the player was forged.
fn calculate_reward(
    ctx: &mut ScriptCtx<'_>,
    qs: l2_world::script::Qs,
    player: ObjectId,
    reward: i32,
) -> bool {
    let Some(items) = reward_items(qs.get_memo_state(ctx)) else {
        return false;
    };
    if !has_one_of(ctx, player, &items) {
        return false;
    }
    ctx.give_items(player, reward, 1);
    ctx.give_items(player, Q_STAR_OF_DESTINY, 1);
    // The first item the player has is taken (Java's else-if chain).
    if let Some(&item) = items.iter().find(|&&i| i != 0 && ctx.has_quest_items(player, i)) {
        ctx.take_items(player, item, 1);
    }
    qs.exit_quest(ctx, false, true);
    // Java `player.broadcastSocialAction(3)`: the player sees it too.
    ctx.send_social_action(player, 3);
    ctx.world().broadcast_packet(
        player,
        &SocialAction {
            object_id: player.0,
            action_id: 3,
        },
    );
    true
}

/// Java `getRandomPlayerFromParty(player, npc, memoState)`.
fn random_player_from_party(
    ctx: &mut ScriptCtx<'_>,
    player: ObjectId,
    npc: ScriptNpc,
    memo_state: i32,
) -> Option<l2_world::script::Qs> {
    let mut candidates = Vec::new();
    if let Some(qs) = ctx.get_quest_state(player, false)
        && qs.is_started(ctx)
        && qs.get_memo_state(ctx) == memo_state
        && ctx.has_quest_items(player, Q_WHITE_FABRIC_Q0234)
    {
        candidates.push(qs);
    }

    if ctx.is_in_party(player) {
        for pm in ctx.world().party_members_of(player) {
            if let Some(qs2) = ctx.get_quest_state(pm, false)
                && qs2.is_started(ctx)
                && qs2.get_memo_state(ctx) == memo_state
                // NOTE(java-quirk): the fabric is checked on the killer, not on the member.
                && ctx.has_quest_items(player, Q_WHITE_FABRIC_Q0234)
                && ctx.check_if_in_range(ctx.alt_party_range(), npc.object_id, pm, true)
            {
                candidates.push(qs2);
            }
        }
    }

    ctx.get_random_entry(&candidates)
}

impl Script for Q00234FatesWhisper {
    fn name(&self) -> &'static str {
        "Q00234_FatesWhisper"
    }

    fn path(&self) -> &'static str {
        "quests/Q00234_FatesWhisper"
    }

    fn id(&self) -> i32 {
        234
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[MAESTRO_LEORIN]);
        s.add_talk_id(&[
            ZENKIN,
            CLIFF,
            MASTER_KASPAR,
            HEAD_BLACKSMITH_FERRIS,
            MAESTRO_LEORIN,
        ]);
        s.add_talk_id(&[
            COFFER_OF_THE_DEAD,
            CHEST_OF_KERNON,
            CHEST_OF_HALLATE,
            CHEST_OF_GOLKONDA,
        ]);
        s.add_kill_id(&[
            PLATINUM_TRIBE_GRUNT,
            PLATINUM_TRIBE_ARCHER,
            PLATINUM_TRIBE_WARRIOR,
            PLATINUM_TRIBE_SHAMAN,
            PLATINUM_TRIBE_LORD,
            GUARDIAN_ANGEL,
            SEAL_ANGEL,
            SEAL_ANGEL_R,
        ]);
        s.add_kill_id(&[
            DOMB_DEATH_CABRIO,
            KERNON,
            GOLKONDA_LONGHORN,
            HALLATE_THE_DEATH_LORD,
        ]);
        s.add_spawn_id(&[
            COFFER_OF_THE_DEAD,
            CHEST_OF_KERNON,
            CHEST_OF_HALLATE,
            CHEST_OF_GOLKONDA,
        ]);
        s.add_attack_id(&[BAIUM]);
        s.register_quest_items(&[
            Q_BLOODY_FABRIC_Q0234,
            Q_WHITE_FABRIC_Q0234,
            Q_PIPETTE_KNIFE,
            Q_REIRIAS_SOULORB,
            Q_INFERNIUM_SCEPTER_1,
            Q_INFERNIUM_SCEPTER_2,
            Q_INFERNIUM_SCEPTER_3,
            Q_MAESTRO_REORINS_HAMMER,
            Q_MAESTRO_REORINS_MOLD,
            Q_INFERNIUM_VARNISH,
            Q_RED_PIPETTE_KNIFE,
        ]);
    }

    fn on_spawn(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc) {
        let timer = match npc.id {
            COFFER_OF_THE_DEAD => "23401",
            CHEST_OF_KERNON => "23402",
            CHEST_OF_HALLATE => "23403",
            CHEST_OF_GOLKONDA => "23404",
            _ => return,
        };
        ctx.start_quest_timer(timer, 1000 * 120, Some(npc), None, false);
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let htmltext = ctx.get_no_quest_msg(player);
        let memo = qs.get_memo_state(ctx);
        let page = |s: &str| Some(s.to_owned());
        let p = player;
        match npc.id {
            ZENKIN => match memo {
                6 => return page("30178-01.html"),
                7 => return page("30178-03.html"),
                8 => return page("30178-04.html"),
                _ => {}
            },
            CLIFF => {
                if qs.is_memo_state(ctx, 4) && !ctx.has_quest_items(p, Q_INFERNIUM_VARNISH) {
                    return page("30182-01.html");
                }
                if qs.is_memo_state(ctx, 4) && ctx.has_quest_items(p, Q_INFERNIUM_VARNISH) {
                    return page("30182-05.html");
                }
                if memo >= 5 {
                    return page("30182-06.html");
                }
            }
            MASTER_KASPAR => {
                if qs.is_memo_state(ctx, 7) {
                    return page("30833-01.html");
                }

                let bloody_fabric_count = ctx.get_quest_items_count(p, Q_BLOODY_FABRIC_Q0234);
                let white_fabric_count = ctx.get_quest_items_count(p, Q_WHITE_FABRIC_Q0234);
                let white_bloody_fabric_count = bloody_fabric_count + white_fabric_count;
                if qs.is_memo_state(ctx, 8)
                    && !ctx.has_quest_items(p, Q_RED_PIPETTE_KNIFE)
                    && white_bloody_fabric_count <= 0
                {
                    return page("30833-03.html");
                }

                if qs.is_memo_state(ctx, 8)
                    && ctx.has_quest_items(p, Q_RED_PIPETTE_KNIFE)
                    && white_bloody_fabric_count <= 0
                {
                    ctx.give_items(p, Q_MAESTRO_REORINS_MOLD, 1);
                    ctx.take_items(p, Q_RED_PIPETTE_KNIFE, 1);
                    qs.set_memo_state(ctx, 9);
                    qs.set_cond(ctx, 10, true);
                    qs.show_question_mark(ctx, 234);
                    return page("30833-04.html");
                }

                if qs.is_memo_state(ctx, 8)
                    && !ctx.has_quest_items(p, Q_RED_PIPETTE_KNIFE)
                    && bloody_fabric_count < 30
                    && white_bloody_fabric_count >= 30
                {
                    return page("30833-03c.html");
                }

                if qs.is_memo_state(ctx, 8)
                    && !ctx.has_quest_items(p, Q_RED_PIPETTE_KNIFE)
                    && bloody_fabric_count >= 30
                    && white_bloody_fabric_count >= 30
                {
                    ctx.give_items(p, Q_MAESTRO_REORINS_MOLD, 1);
                    ctx.take_items(p, Q_BLOODY_FABRIC_Q0234, -1);
                    qs.set_memo_state(ctx, 9);
                    qs.set_cond(ctx, 10, true);
                    qs.show_question_mark(ctx, 234);
                    return page("30833-03d.html");
                }

                if qs.is_memo_state(ctx, 8)
                    && !ctx.has_quest_items(p, Q_RED_PIPETTE_KNIFE)
                    && white_bloody_fabric_count < 30
                    && white_bloody_fabric_count > 0
                {
                    ctx.give_items(p, Q_WHITE_FABRIC_Q0234, 30 - white_fabric_count);
                    ctx.take_items(p, Q_BLOODY_FABRIC_Q0234, -1);
                    return page("30833-03e.html");
                }

                if memo >= 9 {
                    return page("30833-05.html");
                }
            }
            HEAD_BLACKSMITH_FERRIS => {
                if qs.is_memo_state(ctx, 5) {
                    if ctx.has_quest_items(p, Q_MAESTRO_REORINS_HAMMER) {
                        return page("30847-02.html");
                    }

                    ctx.give_items(p, Q_MAESTRO_REORINS_HAMMER, 1);
                    return page("30847-01.html");
                }

                if memo >= 6 {
                    return page("30847-03.html");
                }
            }
            MAESTRO_LEORIN => return leorin_talk(ctx, qs, p, memo).or(Some(htmltext)),
            COFFER_OF_THE_DEAD => {
                if qs.is_memo_state(ctx, 1) && !ctx.has_quest_items(p, Q_REIRIAS_SOULORB) {
                    ctx.give_items(p, Q_REIRIAS_SOULORB, 1);
                    ctx.play_sound(p, QuestSound::ItemsoundQuestItemget);
                    return page("31027-01.html");
                }

                if memo > 1 || ctx.has_quest_items(p, Q_REIRIAS_SOULORB) {
                    return page("31027-02.html");
                }
            }
            CHEST_OF_KERNON => {
                if qs.is_memo_state(ctx, 2) && !ctx.has_quest_items(p, Q_INFERNIUM_SCEPTER_1) {
                    ctx.give_items(p, Q_INFERNIUM_SCEPTER_1, 1);
                    ctx.play_sound(p, QuestSound::ItemsoundQuestItemget);
                    return page("31028-01.html");
                }

                if memo != 2 || ctx.has_quest_items(p, Q_INFERNIUM_SCEPTER_1) {
                    return page("31028-02.html");
                }
            }
            CHEST_OF_GOLKONDA => {
                if qs.is_memo_state(ctx, 2) && !ctx.has_quest_items(p, Q_INFERNIUM_SCEPTER_2) {
                    ctx.give_items(p, Q_INFERNIUM_SCEPTER_2, 1);
                    ctx.play_sound(p, QuestSound::ItemsoundQuestItemget);
                    return page("31029-01.html");
                }

                if memo != 2 || ctx.has_quest_items(p, Q_INFERNIUM_SCEPTER_2) {
                    return page("31029-02.html");
                }
            }
            CHEST_OF_HALLATE => {
                if qs.is_memo_state(ctx, 2) && !ctx.has_quest_items(p, Q_INFERNIUM_SCEPTER_3) {
                    ctx.give_items(p, Q_INFERNIUM_SCEPTER_3, 1);
                    ctx.play_sound(p, QuestSound::ItemsoundQuestItemget);
                    return page("31030-01.html");
                }

                if memo != 2 || ctx.has_quest_items(p, Q_INFERNIUM_SCEPTER_3) {
                    return page("31030-02.html");
                }
            }
            _ => {}
        }

        Some(htmltext)
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        let Some(player) = player else {
            if matches!(event, "23401" | "23402" | "23403" | "23404") {
                // NOTE(java-quirk): `npc.decayMe()` removes the coffer without the death
                // handling; `delete_npc` is the closest world call.
                if let Some(npc) = npc {
                    ctx.delete_npc(npc);
                }
            }
            return None;
        };

        let qs = ctx.get_quest_state(player, false)?;

        if event == "QUEST_ACCEPTED" {
            qs.set_memo_state(ctx, 1);
            qs.start_quest(ctx);
            qs.show_question_mark(ctx, 234);
            ctx.play_sound(player, QuestSound::ItemsoundQuestAccept);
            return Some("31002-06.html".to_owned());
        }

        if event.contains(".htm") {
            return Some(event.to_owned());
        }

        // Java `Integer.parseInt(event)` throws for anything else.
        let npc = npc?;
        let event_id: i32 = event.parse().ok()?;
        let page = |s: &str| Some(s.to_owned());
        let p = player;

        match npc.id {
            ZENKIN => {
                if event_id == 1 {
                    qs.set_memo_state(ctx, 7);
                    qs.set_cond(ctx, 6, false);
                    qs.show_question_mark(ctx, 234);
                    ctx.play_sound(p, QuestSound::ItemsoundQuestMiddle);
                    return page("30178-02.html");
                }
            }
            CLIFF => match event_id {
                1 => return page("30182-02.html"),
                2 => return page("30182-03.html"),
                3 => {
                    if qs.get_memo_state(ctx) == 4 && !ctx.has_quest_items(p, Q_INFERNIUM_VARNISH) {
                        ctx.give_items(p, Q_INFERNIUM_VARNISH, 1);
                        return page("30182-04.html");
                    }
                }
                _ => {}
            },
            MASTER_KASPAR => match event_id {
                1 => {
                    if qs.is_memo_state(ctx, 7) {
                        return page("30833-02.html");
                    }
                }
                2 => {
                    if qs.is_memo_state(ctx, 7) {
                        ctx.give_items(p, Q_PIPETTE_KNIFE, 1);
                        qs.set_memo_state(ctx, 8);
                        qs.set_cond(ctx, 7, true);
                        qs.show_question_mark(ctx, 234);
                        return page("30833-03a.html");
                    }
                }
                3 => {
                    if qs.is_memo_state(ctx, 7) {
                        ctx.give_items(p, Q_WHITE_FABRIC_Q0234, 30);
                        qs.set_memo_state(ctx, 8);
                        qs.set_cond(ctx, 8, true);
                        qs.show_question_mark(ctx, 234);
                        return page("30833-03b.html");
                    }
                }
                _ => {}
            },
            MAESTRO_LEORIN => return leorin_event(ctx, qs, p, event_id),
            _ => {}
        }

        None
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        // The bosses leave their coffers at their place.
        let coffer = match npc.id {
            DOMB_DEATH_CABRIO => Some(COFFER_OF_THE_DEAD),
            KERNON => Some(CHEST_OF_KERNON),
            GOLKONDA_LONGHORN => Some(CHEST_OF_GOLKONDA),
            HALLATE_THE_DEATH_LORD => Some(CHEST_OF_HALLATE),
            _ => None,
        };
        if let Some(coffer) = coffer {
            if let Some((x, y, z)) = ctx.world().position_of(npc.object_id) {
                let heading = ctx.heading_of(npc.object_id);
                ctx.add_spawn(coffer, x, y, z, heading, false, 0, 0);
            }
            return;
        }

        let Some(qs) = random_player_from_party(ctx, killer, npc, 8) else {
            return;
        };
        match npc.id {
            PLATINUM_TRIBE_GRUNT
            | PLATINUM_TRIBE_ARCHER
            | PLATINUM_TRIBE_WARRIOR
            | PLATINUM_TRIBE_SHAMAN
            | PLATINUM_TRIBE_LORD
            | GUARDIAN_ANGEL
            | SEAL_ANGEL
            | SEAL_ANGEL_R => {
                let member = qs.player();
                ctx.take_items(member, Q_WHITE_FABRIC_Q0234, 1);
                ctx.give_items(member, Q_BLOODY_FABRIC_Q0234, 1);
                if ctx.get_quest_items_count(member, Q_BLOODY_FABRIC_Q0234) >= 30 {
                    qs.set_cond(ctx, 9, true);
                    qs.show_question_mark(ctx, 234);
                } else {
                    ctx.play_sound(member, QuestSound::ItemsoundQuestItemget);
                }
            }
            _ => {}
        }
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
        let qs = ctx.get_quest_state(attacker, false);
        if qs.is_some() && ctx.active_weapon_item_id(attacker) == Some(Q_PIPETTE_KNIFE) {
            ctx.take_items(attacker, Q_PIPETTE_KNIFE, 1);
            ctx.give_items(attacker, Q_RED_PIPETTE_KNIFE, 1);
            ctx.play_sound(attacker, QuestSound::ItemsoundQuestItemget);
            ctx.broadcast_npc_say(
                npc,
                ChatType::General,
                NpcStringId::WhoDaresToTryAndStealMyNobleBlood,
                &[],
            );
        }
    }
}

/// The talk of Maestro Leorin (Java's long `if` chain, then the weapon `switch`).
fn leorin_talk(ctx: &mut ScriptCtx<'_>, qs: l2_world::script::Qs, p: ObjectId, memo: i32) -> Option<String> {
    let page = |s: &str| Some(s.to_owned());
    let state = qs.state(ctx);
    let level = ctx.player_level(p);
    if state == CREATED && level >= 75 {
        return page("31002-01.htm");
    }
    if state == CREATED && level < 75 {
        return page("31002-01a.htm");
    }
    if state == COMPLETED {
        return Some(ctx.get_already_completed_msg(p));
    }

    if qs.is_memo_state(ctx, 1) && !ctx.has_quest_items(p, Q_REIRIAS_SOULORB) {
        return page("31002-09.html");
    }
    if qs.is_memo_state(ctx, 1) && ctx.has_quest_items(p, Q_REIRIAS_SOULORB) {
        return page("31002-10.html");
    }
    let scepters = [
        Q_INFERNIUM_SCEPTER_1,
        Q_INFERNIUM_SCEPTER_2,
        Q_INFERNIUM_SCEPTER_3,
    ];
    if qs.is_memo_state(ctx, 2) && !ctx.has_all_quest_items(p, &scepters) {
        return page("31002-12.html");
    }
    if qs.is_memo_state(ctx, 2) && ctx.has_all_quest_items(p, &scepters) {
        return page("31002-13.html");
    }
    if qs.is_memo_state(ctx, 4) && !ctx.has_quest_items(p, Q_INFERNIUM_VARNISH) {
        return page("31002-15.html");
    }
    if qs.is_memo_state(ctx, 4) && ctx.has_quest_items(p, Q_INFERNIUM_VARNISH) {
        return page("31002-16.html");
    }
    if qs.is_memo_state(ctx, 5) && !ctx.has_quest_items(p, Q_MAESTRO_REORINS_HAMMER) {
        return page("31002-18.html");
    }
    if qs.is_memo_state(ctx, 5) && ctx.has_quest_items(p, Q_MAESTRO_REORINS_HAMMER) {
        return page("31002-19.html");
    }
    if (6..9).contains(&memo) {
        return page("31002-21.html");
    }
    if qs.is_memo_state(ctx, 9) && ctx.has_quest_items(p, Q_MAESTRO_REORINS_MOLD) {
        return page("31002-22.html");
    }
    if qs.is_memo_state(ctx, 10) && ctx.get_quest_items_count(p, CRYSTAL_B) < 984 {
        return page("31002-24.html");
    }
    if qs.is_memo_state(ctx, 10) && ctx.get_quest_items_count(p, CRYSTAL_B) >= 984 {
        return page("31002-25.html");
    }

    // The forged weapon of each memo state: (page with the weapon, page without it).
    let (with, without) = match memo {
        11 => ("31002-35.html", "31002-35a.html"),
        12 => ("31002-36.html", "31002-36a.html"),
        13 => ("31002-37.html", "31002-37a.html"),
        14 => ("31002-38.html", "31002-38a.html"),
        15 => ("31002-39.html", "31002-39a.html"),
        16 => ("31002-40.html", "31002-40a.html"),
        17 => ("31002-41.html", "31002-41a.html"),
        18 => ("31002-42.html", "31002-42a.html"),
        19 => ("31002-43.html", "31002-43a.html"),
        41 => ("31002-43b.html", "31002-43c.html"),
        42 => ("31002-43d.html", "31002-43e.html"),
        43 => ("31002-43f.html", "31002-43g.html"),
        44 => ("31002-43h.html", "31002-43i.html"),
        _ => return None,
    };
    let items = reward_items(memo)?;
    if has_one_of(ctx, p, &items) {
        page(with)
    } else {
        page(without)
    }
}

/// The numeric events of Maestro Leorin's pages.
fn leorin_event(
    ctx: &mut ScriptCtx<'_>,
    qs: l2_world::script::Qs,
    p: ObjectId,
    event_id: i32,
) -> Option<String> {
    let page = |s: &str| Some(s.to_owned());
    match event_id {
        1 => return page("31002-02.htm"),
        2 => return page("31002-03.html"),
        3 => return page("31002-04.html"),
        4 => {
            if !qs.is_completed(ctx) && ctx.player_level(p) >= 75 {
                return page("31002-05.html");
            }
        }
        5 => {
            if qs.is_memo_state(ctx, 1) && ctx.has_quest_items(p, Q_REIRIAS_SOULORB) {
                ctx.take_items(p, Q_REIRIAS_SOULORB, 1);
                qs.set_memo_state(ctx, 2);
                qs.set_cond(ctx, 2, true);
                qs.show_question_mark(ctx, 234);
                return page("31002-11.html");
            }
        }
        6 => {
            if qs.is_memo_state(ctx, 2)
                && ctx.has_all_quest_items(
                    p,
                    &[
                        Q_INFERNIUM_SCEPTER_1,
                        Q_INFERNIUM_SCEPTER_2,
                        Q_INFERNIUM_SCEPTER_3,
                    ],
                )
            {
                ctx.take_items(p, Q_INFERNIUM_SCEPTER_1, -1);
                ctx.take_items(p, Q_INFERNIUM_SCEPTER_2, -1);
                ctx.take_items(p, Q_INFERNIUM_SCEPTER_3, -1);
                qs.set_memo_state(ctx, 4);
                qs.set_cond(ctx, 3, true);
                qs.show_question_mark(ctx, 234);
                return page("31002-14.html");
            }
        }
        7 => {
            if qs.is_memo_state(ctx, 4) && ctx.has_quest_items(p, Q_INFERNIUM_VARNISH) {
                ctx.take_items(p, Q_INFERNIUM_VARNISH, 1);
                qs.set_memo_state(ctx, 5);
                qs.set_cond(ctx, 4, true);
                qs.show_question_mark(ctx, 234);
                return page("31002-17.html");
            }
        }
        8 => {
            if qs.is_memo_state(ctx, 5) && ctx.has_quest_items(p, Q_MAESTRO_REORINS_HAMMER) {
                ctx.take_items(p, Q_MAESTRO_REORINS_HAMMER, 1);
                qs.set_memo_state(ctx, 6);
                qs.set_cond(ctx, 5, true);
                qs.show_question_mark(ctx, 234);
                return page("31002-20.html");
            }
        }
        9 => {
            if qs.is_memo_state(ctx, 9) && ctx.has_quest_items(p, Q_MAESTRO_REORINS_MOLD) {
                ctx.take_items(p, Q_MAESTRO_REORINS_MOLD, 1);
                qs.set_memo_state(ctx, 10);
                qs.set_cond(ctx, 11, true);
                qs.show_question_mark(ctx, 234);
                return page("31002-23.html");
            }
        }
        // The choice of the weapon to forge: 984 crystals B for the memo state and its page.
        10 | 11 | 12 | 13 | 14 | 15 | 16 | 17 | 18 | 41 | 42 | 43 | 44 => {
            if qs.is_memo_state(ctx, 10) {
                if ctx.get_quest_items_count(p, CRYSTAL_B) >= 984 {
                    let (memo_state, page_name) = match event_id {
                        10 => (11, "31002-26.html"),
                        11 => (19, "31002-26a.html"),
                        12 => (12, "31002-27.html"),
                        13 => (13, "31002-28.html"),
                        14 => (14, "31002-29.html"),
                        15 => (15, "31002-30.html"),
                        16 => (16, "31002-31.html"),
                        17 => (17, "31002-32.html"),
                        18 => (18, "31002-33.html"),
                        41 => (41, "31002-33a.html"),
                        42 => (42, "31002-33b.html"),
                        43 => (43, "31002-33c.html"),
                        _ => (44, "31002-33d.html"),
                    };
                    ctx.take_items(p, CRYSTAL_B, 984);
                    qs.set_memo_state(ctx, memo_state);
                    qs.set_cond(ctx, 12, true);
                    qs.show_question_mark(ctx, 234);
                    return page(page_name);
                }

                return page("31002-34.html");
            }
        }
        // The A-grade weapon chosen as the reward.
        21..=32 => {
            let reward = match event_id {
                21 => TALLUM_BLADE,
                22 => CARNIUM_BOW,
                23 => HALBARD,
                24 => ELEMENTAL_SWORD,
                25 => DASPARIONS_STAFF,
                26 => BLOODY_ORCHID,
                27 => BLOOD_TORNADO,
                28 => METEOR_SHOWER,
                29 => KSHANBERK_KSHANBERK,
                30 => INFERNO_MASTER,
                31 => EYE_OF_SOUL,
                _ => HAMMER_OF_DESTROYER,
            };
            if calculate_reward(ctx, qs, p, reward) {
                return page("31002-44.html");
            }
        }
        _ => {}
    }
    None
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::ScriptRegistry;
    use l2_world::script::player_class;

    use super::*;
    use crate::testing::{Pages, ScriptTest, item, rows};

    const NAME: &str = "Q00234_FatesWhisper";

    const NPCS: &str = r#"
        <npc id="31002" type="Folk" level="70" name="Maestro Leorin"/>
        <npc id="30833" type="Folk" level="70" name="Master Kaspar"/>
        <npc id="31027" type="Folk" level="70" name="Coffer of the Dead"/>
        <npc id="25035" type="RaidBoss" level="80" name="Domb Death Cabrio"/>
        <npc id="20823" type="Monster" level="70" name="Platinum Tribe Grunt"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="1460" type="EtcItem" name="Crystal: B Grade"><set name="is_stackable" val="true"/></item>
        <item id="79" type="Weapon" name="Sword of Damascus"/>
        <item id="80" type="Weapon" name="Tallum Blade"/>
        <item id="5011" type="EtcItem" name="Star of Destiny"/>
        <item id="4666" type="EtcItem" name="Reirias Soulorb"><set name="is_questitem" val="true"/></item>
        <item id="14361" type="EtcItem" name="Bloody Fabric"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="14362" type="EtcItem" name="White Fabric"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>"#;

    fn pages() -> Pages {
        Pages::of(&Q00234FatesWhisper)
            .page("31002-01.htm", &["1"])
            .page("31002-02.htm", &["2"])
            .page("31002-03.html", &["3"])
            .page("31002-04.html", &["4"])
            .page("31002-05.html", &["QUEST_ACCEPTED"])
            .plain(&["31002-01a.htm", "31002-06.html", "31002-10.html", "31002-11.html"])
            .page("31002-25.html", &["10", "11"])
            .plain(&["31002-26.html", "31002-26a.html", "31002-34.html"])
            .page("31002-35.html", &["21", "22"])
            .plain(&["31002-35a.html", "31002-44.html"])
            .plain(&["31027-01.html", "31027-02.html"])
            .npcs(&[MAESTRO_LEORIN, MASTER_KASPAR, COFFER_OF_THE_DEAD])
    }

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00234FatesWhisper);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages());
        let leorin = t.spawn(MAESTRO_LEORIN, 20);
        (t, leorin)
    }

    /// Starts the quest the way the pages do (`1`, `2`, `3`, `4`, `QUEST_ACCEPTED`).
    fn start(t: &mut ScriptTest, leorin: Oid) {
        t.quest_page(1, leorin);
        for event in ["1", "2", "3", "4"] {
            t.event(1, NAME, event);
        }
        t.event(1, NAME, "QUEST_ACCEPTED");
    }

    fn set_memo(t: &mut ScriptTest, memo: i32) {
        t.world.run_as_script(NAME, |ctx| {
            if let Some(qs) = ctx.get_quest_state(Oid(1), false) {
                qs.set_memo_state(ctx, memo);
            }
        });
    }

    #[test]
    fn the_quest_is_offered_from_level_75() {
        let (mut t, leorin) = setup();
        t.enter_as(1, 74, 0, player_class::FIGHTER, vec![]);
        assert!(t.quest_page(1, leorin).contains("31002-01a.htm"));
        t.enter_as(2, 75, 0, player_class::FIGHTER, vec![]);
        assert!(t.quest_page(2, leorin).contains("31002-01.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn accepting_starts_with_memo_1() {
        let (mut t, leorin) = setup();
        t.enter_as(1, 75, 0, player_class::FIGHTER, vec![]);
        t.quest_page(1, leorin);
        for event in ["1", "2", "3", "4"] {
            t.event(1, NAME, event);
        }
        t.take_writes();
        assert!(t.event(1, NAME, "QUEST_ACCEPTED").contains("31002-06.html"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "memoState", "1"),
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
    }

    #[test]
    fn the_cabrio_coffer_gives_the_soulorb_and_leaves_after_two_minutes() {
        let (mut t, leorin) = setup();
        t.enter_as(1, 75, 0, player_class::FIGHTER, vec![]);
        start(&mut t, leorin);
        let cabrio = t.spawn(DOMB_DEATH_CABRIO, 60);
        t.kill(1, cabrio);
        let coffers = t.npcs_of(COFFER_OF_THE_DEAD);
        assert_eq!(coffers.len(), 1);
        assert!(t.quest_page(1, coffers[0]).contains("31027-01.html"));
        assert_eq!(t.count(1, Q_REIRIAS_SOULORB), 1);
        assert!(t.quest_page(1, coffers[0]).contains("31027-02.html"));
        t.wait_ms(120_000);
        assert!(t.npcs_of(COFFER_OF_THE_DEAD).is_empty());
    }

    #[test]
    fn the_fabrics_turn_bloody_with_every_kill() {
        let (mut t, leorin) = setup();
        t.enter_as(1, 75, 0, player_class::FIGHTER, vec![]);
        start(&mut t, leorin);
        set_memo(&mut t, 8);
        t.world
            .run_as_script(NAME, |ctx| ctx.give_items(Oid(1), Q_WHITE_FABRIC_Q0234, 30));
        let grunt = t.spawn(PLATINUM_TRIBE_GRUNT, 80);
        t.kill(1, grunt);
        assert_eq!(
            (
                t.count(1, Q_WHITE_FABRIC_Q0234),
                t.count(1, Q_BLOODY_FABRIC_Q0234)
            ),
            (29, 1)
        );
    }

    #[test]
    fn the_crystals_forge_a_weapon_and_the_a_grade_weapon_ends_the_quest() {
        let (mut t, leorin) = setup();
        t.enter_as(1, 75, 0, player_class::FIGHTER, vec![item(10, CRYSTAL_B, 984)]);
        start(&mut t, leorin);
        set_memo(&mut t, 10);
        t.take_writes();
        assert!(t.quest_page(1, leorin).contains("31002-25.html"));
        assert!(t.event(1, NAME, "10").contains("31002-26.html"));
        assert_eq!(t.count(1, CRYSTAL_B), 0);
        assert_eq!(
            t.take_writes(),
            [
                rows::update(NAME, "memoState", "11"),
                rows::update(NAME, "cond", "12"),
                rows::insert(NAME, "__compltdStateFlags", "-2147481599")
            ]
        );

        // Without the B-grade weapon the reward is refused.
        t.quest_page(1, leorin);
        assert!(t.event(1, NAME, "21").is_empty());
        t.world
            .run_as_script(NAME, |ctx| ctx.give_items(Oid(1), SWORD_OF_DAMASCUS, 1));
        assert!(t.quest_page(1, leorin).contains("31002-35.html"));
        assert!(t.event(1, NAME, "21").contains("31002-44.html"));
        assert_eq!(
            (
                t.count(1, TALLUM_BLADE),
                t.count(1, Q_STAR_OF_DESTINY),
                t.count(1, SWORD_OF_DAMASCUS)
            ),
            (1, 1, 0)
        );
        assert_eq!(
            t.take_writes().last(),
            Some(&rows::update(NAME, "<state>", "Completed"))
        );
    }
}
