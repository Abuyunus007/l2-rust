//! Quest 214: Trial of the Scholar (Magister Mirien and the sigils of eleven NPCs; the wizard's mark).
// Java: dist/game/data/scripts/quests/Q00214_TrialOfTheScholar/Q00214_TrialOfTheScholar.java

use l2_util::ObjectId;
use l2_world::script::{QuestSound, player_class};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const HIGH_PRIEST_SYLVAIN: i32 = 30070;
const CAPTAIN_LUCAS: i32 = 30071;
const WAREHOUSE_KEEPER_VALKON: i32 = 30103;
const MAGISTER_DIETER: i32 = 30111;
const GRAND_MAGISTER_JUREK: i32 = 30115;
const TRADER_EDROC: i32 = 30230;
const WAREHOUSE_KEEPER_RAUT: i32 = 30316;
const BLACKSMITH_POITAN: i32 = 30458;
const MAGISTER_MIRIEN: i32 = 30461;
const MARIA: i32 = 30608;
const ASTROLOGER_CRETA: i32 = 30609;
const ELDER_CRONOS: i32 = 30610;
const DRUNKARD_TRIFF: i32 = 30611;
const ELDER_CASIAN: i32 = 30612;

// Items
const MIRIENS_1ST_SIGIL: i32 = 2675;
const MIRIENS_2ND_SIGIL: i32 = 2676;
const MIRIENS_3RD_SIGIL: i32 = 2677;
const MIRIENS_INSTRUCTION: i32 = 2678;
const MARIAS_1ST_LETTER: i32 = 2679;
const MARIAS_2ND_LETTER: i32 = 2680;
const LUCASS_LETTER: i32 = 2681;
const LUCILLAS_HANDBAG: i32 = 2682;
const CRETAS_1ST_LETTER: i32 = 2683;
const CRERAS_PAINTING1: i32 = 2684;
const CRERAS_PAINTING2: i32 = 2685;
const CRERAS_PAINTING3: i32 = 2686;
const BROWN_SCROLL_SCRAP: i32 = 2687;
const CRYSTAL_OF_PURITY1: i32 = 2688;
const HIGH_PRIESTS_SIGIL: i32 = 2689;
const GRAND_MAGISTER_SIGIL: i32 = 2690;
const CRONOS_SIGIL: i32 = 2691;
const SYLVAINS_LETTER: i32 = 2692;
const SYMBOL_OF_SYLVAIN: i32 = 2693;
const JUREKS_LIST: i32 = 2694;
const MONSTER_EYE_DESTROYER_SKIN: i32 = 2695;
const SHAMANS_NECKLACE: i32 = 2696;
const SHACKLES_SCALP: i32 = 2697;
const SYMBOL_OF_JUREK: i32 = 2698;
const CRONOS_LETTER: i32 = 2699;
const DIETERS_KEY: i32 = 2700;
const CRETAS_2ND_LETTER: i32 = 2701;
const DIETERS_LETTER: i32 = 2702;
const DIETERS_DIARY: i32 = 2703;
const RAUTS_LETTER_ENVELOPE: i32 = 2704;
const TRIFFS_RING: i32 = 2705;
const SCRIPTURE_CHAPTER_1: i32 = 2706;
const SCRIPTURE_CHAPTER_2: i32 = 2707;
const SCRIPTURE_CHAPTER_3: i32 = 2708;
const SCRIPTURE_CHAPTER_4: i32 = 2709;
const VALKONS_REQUEST: i32 = 2710;
const POITANS_NOTES: i32 = 2711;
const STRONG_LIGUOR: i32 = 2713;
const CRYSTAL_OF_PURITY2: i32 = 2714;
const CASIANS_LIST: i32 = 2715;
const GHOULS_SKIN: i32 = 2716;
const MEDUSAS_BLOOD: i32 = 2717;
const FETTERED_SOULS_ICHOR: i32 = 2718;
const ENCHANTED_GARGOYLES_NAIL: i32 = 2719;
const SYMBOL_OF_CRONOS: i32 = 2720;

// Reward
const MARK_OF_SCHOLAR: i32 = 2674;
const DIMENSIONAL_DIAMOND: i32 = 7562;

// Monsters
const MONSTER_EYE_DESTREOYER: i32 = 20068;
const MEDUSA: i32 = 20158;
const GHOUL: i32 = 20201;
const SHACKLE1: i32 = 20235;
const BREKA_ORC_SHAMAN: i32 = 20269;
const SHACKLE2: i32 = 20279;
const FETTERED_SOUL: i32 = 20552;
const GRANDIS: i32 = 20554;
const ENCHANTED_GARGOYLE: i32 = 20567;
const LETO_LIZARDMAN_WARRIOR: i32 = 20580;

// Misc
const MIN_LEVEL: i32 = 35;
const LEVEL: i32 = 36;
/// Java `player.getVariables()` key: the second-class diamonds are given once per character.
const DIAMOND_REWARD_VAR: &str = "2ND_CLASS_DIAMOND_REWARD";

/// Java `Q00214_TrialOfTheScholar`.
pub struct Q00214TrialOfTheScholar;

/// Java `hasQuestItems(player, a, b, ...)`: all of them.
fn all(ctx: &ScriptCtx<'_>, player: ObjectId, items: &[i32]) -> bool {
    ctx.has_all_quest_items(player, items)
}

/// Java `hasAtLeastOneQuestItem(player, a, b, ...)`.
fn any(ctx: &ScriptCtx<'_>, player: ObjectId, items: &[i32]) -> bool {
    ctx.has_at_least_one_quest_item(player, items)
}

/// Java `hasQuestItems(player, item)`.
fn has(ctx: &ScriptCtx<'_>, player: ObjectId, item: i32) -> bool {
    ctx.has_quest_items(player, item)
}

fn count(ctx: &ScriptCtx<'_>, player: ObjectId, item: i32) -> i64 {
    ctx.get_quest_items_count(player, item)
}

impl Script for Q00214TrialOfTheScholar {
    fn name(&self) -> &'static str {
        "Q00214_TrialOfTheScholar"
    }

    fn path(&self) -> &'static str {
        "quests/Q00214_TrialOfTheScholar"
    }

    fn id(&self) -> i32 {
        214
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[MAGISTER_MIRIEN]);
        s.add_talk_id(&[
            MAGISTER_MIRIEN,
            HIGH_PRIEST_SYLVAIN,
            CAPTAIN_LUCAS,
            WAREHOUSE_KEEPER_VALKON,
            MAGISTER_DIETER,
            GRAND_MAGISTER_JUREK,
            TRADER_EDROC,
            WAREHOUSE_KEEPER_RAUT,
            BLACKSMITH_POITAN,
            MARIA,
            ASTROLOGER_CRETA,
            ELDER_CRONOS,
            DRUNKARD_TRIFF,
            ELDER_CASIAN,
        ]);
        s.add_kill_id(&[
            MONSTER_EYE_DESTREOYER,
            MEDUSA,
            GHOUL,
            SHACKLE1,
            BREKA_ORC_SHAMAN,
            SHACKLE2,
            FETTERED_SOUL,
            GRANDIS,
            ENCHANTED_GARGOYLE,
            LETO_LIZARDMAN_WARRIOR,
        ]);
        // NOTE(java-quirk): Java lists CRERAS_PAINTING1 twice and CRERAS_PAINTING2 not at all, so
        // the second painting is no registered quest item (it stays on abort).
        s.register_quest_items(&[
            MIRIENS_1ST_SIGIL,
            MIRIENS_2ND_SIGIL,
            MIRIENS_3RD_SIGIL,
            MIRIENS_INSTRUCTION,
            MARIAS_1ST_LETTER,
            MARIAS_2ND_LETTER,
            LUCASS_LETTER,
            LUCILLAS_HANDBAG,
            CRETAS_1ST_LETTER,
            CRERAS_PAINTING1,
            CRERAS_PAINTING1,
            CRERAS_PAINTING3,
            BROWN_SCROLL_SCRAP,
            CRYSTAL_OF_PURITY1,
            HIGH_PRIESTS_SIGIL,
            GRAND_MAGISTER_SIGIL,
            CRONOS_SIGIL,
            SYLVAINS_LETTER,
            SYMBOL_OF_SYLVAIN,
            JUREKS_LIST,
            MONSTER_EYE_DESTROYER_SKIN,
            SHAMANS_NECKLACE,
            SHACKLES_SCALP,
            SYMBOL_OF_JUREK,
            CRONOS_LETTER,
            DIETERS_KEY,
            CRETAS_2ND_LETTER,
            DIETERS_LETTER,
            DIETERS_DIARY,
            RAUTS_LETTER_ENVELOPE,
            TRIFFS_RING,
            SCRIPTURE_CHAPTER_1,
            SCRIPTURE_CHAPTER_2,
            SCRIPTURE_CHAPTER_3,
            SCRIPTURE_CHAPTER_4,
            VALKONS_REQUEST,
            POITANS_NOTES,
            STRONG_LIGUOR,
            CRYSTAL_OF_PURITY2,
            CASIANS_LIST,
            GHOULS_SKIN,
            MEDUSAS_BLOOD,
            FETTERED_SOULS_ICHOR,
            ENCHANTED_GARGOYLES_NAIL,
            SYMBOL_OF_CRONOS,
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
                    if !has(ctx, player, MIRIENS_1ST_SIGIL) {
                        ctx.give_items(player, MIRIENS_1ST_SIGIL, 1);
                    }
                    ctx.play_sound(player, QuestSound::ItemsoundQuestMiddle);
                    if ctx.player_variable_int(player, DIAMOND_REWARD_VAR, 0) == 0 {
                        ctx.give_items(player, DIMENSIONAL_DIAMOND, 168);
                        ctx.set_player_variable(player, DIAMOND_REWARD_VAR, "1");
                        htmltext = Some("30461-04a.htm".to_owned());
                    } else {
                        htmltext = Some("30461-04.htm".to_owned());
                    }
                }
            }
            "30103-02.html" | "30103-03.html" | "30111-02.html" | "30111-03.html" | "30111-04.html"
            | "30111-08.html" | "30111-14.html" | "30115-02.html" | "30316-03.html" | "30461-09.html"
            | "30608-07.html" | "30609-02.html" | "30609-03.html" | "30609-04.html" | "30609-08.html"
            | "30609-13.html" | "30610-02.html" | "30610-03.html" | "30610-04.html" | "30610-05.html"
            | "30610-06.html" | "30610-07.html" | "30610-08.html" | "30610-09.html" | "30610-13.html"
            | "30611-02.html" | "30611-03.html" | "30611-06.html" | "30612-03.html" => {
                htmltext = Some(event.to_owned());
            }
            "30461-10.html" => {
                if all(ctx, player, &[MIRIENS_2ND_SIGIL, SYMBOL_OF_JUREK]) {
                    ctx.take_items(player, MIRIENS_2ND_SIGIL, 1);
                    ctx.give_items(player, MIRIENS_3RD_SIGIL, 1);
                    ctx.take_items(player, SYMBOL_OF_JUREK, 1);
                    qs.set_cond(ctx, 19, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "30070-02.html" => {
                ctx.give_items(player, HIGH_PRIESTS_SIGIL, 1);
                ctx.give_items(player, SYLVAINS_LETTER, 1);
                qs.set_cond(ctx, 2, true);
                htmltext = Some(event.to_owned());
            }
            "30071-04.html" => {
                if has(ctx, player, CRERAS_PAINTING2) {
                    ctx.take_items(player, CRERAS_PAINTING2, 1);
                    ctx.give_items(player, CRERAS_PAINTING3, 1);
                    qs.set_cond(ctx, 10, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "30103-04.html" => {
                ctx.give_items(player, VALKONS_REQUEST, 1);
                htmltext = Some(event.to_owned());
            }
            "30111-05.html" => {
                if has(ctx, player, CRONOS_LETTER) {
                    ctx.take_items(player, CRONOS_LETTER, 1);
                    ctx.give_items(player, DIETERS_KEY, 1);
                    qs.set_cond(ctx, 21, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "30111-09.html" => {
                if has(ctx, player, CRETAS_2ND_LETTER) {
                    ctx.take_items(player, CRETAS_2ND_LETTER, 1);
                    ctx.give_items(player, DIETERS_LETTER, 1);
                    ctx.give_items(player, DIETERS_DIARY, 1);
                    qs.set_cond(ctx, 23, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "30115-03.html" => {
                ctx.give_items(player, JUREKS_LIST, 1);
                ctx.give_items(player, GRAND_MAGISTER_SIGIL, 1);
                qs.set_cond(ctx, 16, true);
                htmltext = Some(event.to_owned());
            }
            "30230-02.html" => {
                if has(ctx, player, DIETERS_LETTER) {
                    ctx.take_items(player, DIETERS_LETTER, 1);
                    ctx.give_items(player, RAUTS_LETTER_ENVELOPE, 1);
                    qs.set_cond(ctx, 24, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "30316-02.html" => {
                if has(ctx, player, RAUTS_LETTER_ENVELOPE) {
                    ctx.take_items(player, RAUTS_LETTER_ENVELOPE, 1);
                    ctx.give_items(player, SCRIPTURE_CHAPTER_1, 1);
                    ctx.give_items(player, STRONG_LIGUOR, 1);
                    qs.set_cond(ctx, 25, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "30608-02.html" => {
                if has(ctx, player, SYLVAINS_LETTER) {
                    ctx.give_items(player, MARIAS_1ST_LETTER, 1);
                    ctx.take_items(player, SYLVAINS_LETTER, 1);
                    qs.set_cond(ctx, 3, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "30608-08.html" => {
                if has(ctx, player, CRETAS_1ST_LETTER) {
                    ctx.give_items(player, LUCILLAS_HANDBAG, 1);
                    ctx.take_items(player, CRETAS_1ST_LETTER, 1);
                    qs.set_cond(ctx, 7, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "30608-14.html" => {
                if has(ctx, player, CRERAS_PAINTING3) {
                    ctx.take_items(player, CRERAS_PAINTING3, 1);
                    ctx.take_items(player, BROWN_SCROLL_SCRAP, -1);
                    ctx.give_items(player, CRYSTAL_OF_PURITY1, 1);
                    qs.set_cond(ctx, 13, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "30609-05.html" => {
                if has(ctx, player, MARIAS_2ND_LETTER) {
                    ctx.take_items(player, MARIAS_2ND_LETTER, 1);
                    ctx.give_items(player, CRETAS_1ST_LETTER, 1);
                    qs.set_cond(ctx, 6, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "30609-09.html" => {
                if has(ctx, player, LUCILLAS_HANDBAG) {
                    ctx.take_items(player, LUCILLAS_HANDBAG, 1);
                    ctx.give_items(player, CRERAS_PAINTING1, 1);
                    qs.set_cond(ctx, 8, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "30609-14.html" => {
                if has(ctx, player, DIETERS_KEY) {
                    ctx.take_items(player, DIETERS_KEY, 1);
                    ctx.give_items(player, CRETAS_2ND_LETTER, 1);
                    qs.set_cond(ctx, 22, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "30610-10.html" => {
                ctx.give_items(player, CRONOS_SIGIL, 1);
                ctx.give_items(player, CRONOS_LETTER, 1);
                qs.set_cond(ctx, 20, true);
                htmltext = Some(event.to_owned());
            }
            "30610-14.html" => {
                if all(
                    ctx,
                    player,
                    &[SCRIPTURE_CHAPTER_1, SCRIPTURE_CHAPTER_2, SCRIPTURE_CHAPTER_3, SCRIPTURE_CHAPTER_4],
                ) {
                    ctx.take_items(player, CRONOS_SIGIL, 1);
                    ctx.take_items(player, DIETERS_DIARY, 1);
                    ctx.take_items(player, TRIFFS_RING, 1);
                    ctx.take_items(player, SCRIPTURE_CHAPTER_1, 1);
                    ctx.take_items(player, SCRIPTURE_CHAPTER_2, 1);
                    ctx.take_items(player, SCRIPTURE_CHAPTER_3, 1);
                    ctx.take_items(player, SCRIPTURE_CHAPTER_4, 1);
                    ctx.give_items(player, SYMBOL_OF_CRONOS, 1);
                    qs.set_cond(ctx, 31, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "30611-04.html" => {
                if has(ctx, player, STRONG_LIGUOR) {
                    ctx.give_items(player, TRIFFS_RING, 1);
                    ctx.take_items(player, STRONG_LIGUOR, 1);
                    qs.set_cond(ctx, 26, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "30612-04.html" => {
                ctx.give_items(player, CASIANS_LIST, 1);
                qs.set_cond(ctx, 28, true);
                htmltext = Some(event.to_owned());
            }
            "30612-07.html" => {
                ctx.give_items(player, SCRIPTURE_CHAPTER_4, 1);
                ctx.take_items(player, POITANS_NOTES, 1);
                ctx.take_items(player, CASIANS_LIST, 1);
                ctx.take_items(player, GHOULS_SKIN, -1);
                ctx.take_items(player, MEDUSAS_BLOOD, -1);
                ctx.take_items(player, FETTERED_SOULS_ICHOR, -1);
                ctx.take_items(player, ENCHANTED_GARGOYLES_NAIL, -1);
                qs.set_cond(ctx, 30, true);
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
        if !(qs.is_started(ctx) && ctx.check_if_in_range(ctx.alt_party_range(), npc.object_id, killer, true)) {
            return;
        }

        match npc.id {
            MONSTER_EYE_DESTREOYER => {
                if all(ctx, killer, &[MIRIENS_2ND_SIGIL, GRAND_MAGISTER_SIGIL, JUREKS_LIST])
                    && count(ctx, killer, MONSTER_EYE_DESTROYER_SKIN) < 5
                {
                    ctx.give_items(killer, MONSTER_EYE_DESTROYER_SKIN, 1);
                    if count(ctx, killer, MONSTER_EYE_DESTROYER_SKIN) == 5
                        && count(ctx, killer, SHAMANS_NECKLACE) >= 5
                        && count(ctx, killer, SHACKLES_SCALP) >= 2
                    {
                        qs.set_cond(ctx, 17, true);
                    } else {
                        ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
                    }
                }
            }
            MEDUSA => {
                if all(ctx, killer, &[TRIFFS_RING, POITANS_NOTES, CASIANS_LIST])
                    && count(ctx, killer, MEDUSAS_BLOOD) < 12
                {
                    ctx.give_items(killer, MEDUSAS_BLOOD, 1);
                    if count(ctx, killer, MEDUSAS_BLOOD) == 12 {
                        ctx.play_sound(killer, QuestSound::ItemsoundQuestMiddle);
                    } else {
                        ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
                    }
                }
            }
            GHOUL => {
                if all(ctx, killer, &[TRIFFS_RING, POITANS_NOTES, CASIANS_LIST])
                    && count(ctx, killer, GHOULS_SKIN) < 10
                {
                    ctx.give_items(killer, GHOULS_SKIN, 1);
                    if count(ctx, killer, GHOULS_SKIN) == 10 {
                        qs.set_cond(ctx, 29, true);
                    } else {
                        ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
                    }
                }
            }
            SHACKLE1 | SHACKLE2 => {
                if all(ctx, killer, &[MIRIENS_2ND_SIGIL, GRAND_MAGISTER_SIGIL, JUREKS_LIST])
                    && count(ctx, killer, SHACKLES_SCALP) < 2
                {
                    ctx.give_items(killer, SHACKLES_SCALP, 1);
                    if count(ctx, killer, MONSTER_EYE_DESTROYER_SKIN) >= 5
                        && count(ctx, killer, SHAMANS_NECKLACE) >= 5
                        && count(ctx, killer, SHACKLES_SCALP) == 2
                    {
                        qs.set_cond(ctx, 17, true);
                    } else {
                        ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
                    }
                }
            }
            BREKA_ORC_SHAMAN => {
                if all(ctx, killer, &[MIRIENS_2ND_SIGIL, GRAND_MAGISTER_SIGIL, JUREKS_LIST])
                    && count(ctx, killer, SHAMANS_NECKLACE) < 5
                {
                    ctx.give_items(killer, SHAMANS_NECKLACE, 1);
                    if count(ctx, killer, MONSTER_EYE_DESTROYER_SKIN) >= 5
                        && count(ctx, killer, SHAMANS_NECKLACE) == 5
                        && count(ctx, killer, SHACKLES_SCALP) >= 2
                    {
                        qs.set_cond(ctx, 17, true);
                    } else {
                        ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
                    }
                }
            }
            FETTERED_SOUL => {
                if all(ctx, killer, &[TRIFFS_RING, POITANS_NOTES, CASIANS_LIST])
                    && count(ctx, killer, FETTERED_SOULS_ICHOR) < 5
                {
                    ctx.give_items(killer, FETTERED_SOULS_ICHOR, 1);
                    if count(ctx, killer, FETTERED_SOULS_ICHOR) >= 5 {
                        ctx.play_sound(killer, QuestSound::ItemsoundQuestMiddle);
                    } else {
                        ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
                    }
                }
            }
            GRANDIS => {
                if all(ctx, killer, &[MIRIENS_3RD_SIGIL, CRONOS_SIGIL, TRIFFS_RING])
                    && !has(ctx, killer, SCRIPTURE_CHAPTER_3)
                {
                    ctx.give_items(killer, SCRIPTURE_CHAPTER_3, 1);
                    ctx.play_sound(killer, QuestSound::ItemsoundQuestMiddle);
                }
            }
            ENCHANTED_GARGOYLE => {
                if all(ctx, killer, &[TRIFFS_RING, POITANS_NOTES, CASIANS_LIST])
                    && count(ctx, killer, ENCHANTED_GARGOYLES_NAIL) < 5
                {
                    ctx.give_items(killer, ENCHANTED_GARGOYLES_NAIL, 1);
                    if count(ctx, killer, ENCHANTED_GARGOYLES_NAIL) >= 5 {
                        ctx.play_sound(killer, QuestSound::ItemsoundQuestMiddle);
                    } else {
                        ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
                    }
                }
            }
            LETO_LIZARDMAN_WARRIOR => {
                if all(ctx, killer, &[MIRIENS_1ST_SIGIL, HIGH_PRIESTS_SIGIL, CRERAS_PAINTING3])
                    && count(ctx, killer, BROWN_SCROLL_SCRAP) < 5
                {
                    ctx.give_items(killer, BROWN_SCROLL_SCRAP, 1);
                    if count(ctx, killer, BROWN_SCROLL_SCRAP) == 5 {
                        qs.set_cond(ctx, 12, true);
                    } else {
                        ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
                    }
                }
            }
            _ => {}
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        if qs.is_created(ctx) {
            if npc.id == MAGISTER_MIRIEN {
                let class = ctx.player_class_id(player);
                if class == player_class::WIZARD
                    || class == player_class::ELVEN_WIZARD
                    || class == player_class::DARK_WIZARD
                {
                    if ctx.player_level(player) < MIN_LEVEL {
                        htmltext = "30461-02.html".to_owned();
                    } else {
                        htmltext = "30461-03.htm".to_owned();
                    }
                } else {
                    htmltext = "30461-01.html".to_owned();
                }
            }
        } else if qs.is_started(ctx) {
            let chapters = [SCRIPTURE_CHAPTER_1, SCRIPTURE_CHAPTER_2, SCRIPTURE_CHAPTER_3, SCRIPTURE_CHAPTER_4];
            match npc.id {
                MAGISTER_MIRIEN => {
                    if has(ctx, player, MIRIENS_1ST_SIGIL) {
                        if !has(ctx, player, SYMBOL_OF_SYLVAIN) {
                            htmltext = "30461-05.html".to_owned();
                        } else {
                            ctx.take_items(player, MIRIENS_1ST_SIGIL, 1);
                            ctx.give_items(player, MIRIENS_2ND_SIGIL, 1);
                            ctx.take_items(player, SYMBOL_OF_SYLVAIN, 1);
                            qs.set_cond(ctx, 15, true);
                            htmltext = "30461-06.html".to_owned();
                        }
                    } else if has(ctx, player, MIRIENS_2ND_SIGIL) {
                        if !has(ctx, player, SYMBOL_OF_JUREK) {
                            htmltext = "30461-07.html".to_owned();
                        } else {
                            htmltext = "30461-08.html".to_owned();
                        }
                    } else if has(ctx, player, MIRIENS_INSTRUCTION) {
                        if ctx.player_level(player) < LEVEL {
                            htmltext = "30461-11.html".to_owned();
                        } else {
                            ctx.take_items(player, MIRIENS_INSTRUCTION, 1);
                            ctx.give_items(player, MIRIENS_3RD_SIGIL, 1);
                            qs.set_cond(ctx, 19, true);
                            htmltext = "30461-12.html".to_owned();
                        }
                    } else if has(ctx, player, MIRIENS_3RD_SIGIL) {
                        if !has(ctx, player, SYMBOL_OF_CRONOS) {
                            htmltext = "30461-13.html".to_owned();
                        } else {
                            ctx.give_adena(player, 319_628, true);
                            ctx.give_items(player, MARK_OF_SCHOLAR, 1);
                            ctx.add_exp_and_sp(player, 1_753_926, 113_754);
                            qs.exit_quest(ctx, false, true);
                            ctx.send_social_action(player, 3);
                            htmltext = "30461-14.html".to_owned();
                        }
                    }
                }
                HIGH_PRIEST_SYLVAIN => {
                    if has(ctx, player, MIRIENS_1ST_SIGIL)
                        && !any(ctx, player, &[HIGH_PRIESTS_SIGIL, SYMBOL_OF_SYLVAIN])
                    {
                        htmltext = "30070-01.html".to_owned();
                    } else if !has(ctx, player, CRYSTAL_OF_PURITY1)
                        && all(ctx, player, &[HIGH_PRIESTS_SIGIL, MIRIENS_1ST_SIGIL])
                    {
                        htmltext = "30070-03.html".to_owned();
                    } else if all(ctx, player, &[HIGH_PRIESTS_SIGIL, MIRIENS_1ST_SIGIL, CRYSTAL_OF_PURITY1]) {
                        ctx.take_items(player, CRYSTAL_OF_PURITY1, 1);
                        ctx.take_items(player, HIGH_PRIESTS_SIGIL, 1);
                        ctx.give_items(player, SYMBOL_OF_SYLVAIN, 1);
                        qs.set_cond(ctx, 14, true);
                        htmltext = "30070-04.html".to_owned();
                    } else if all(ctx, player, &[MIRIENS_1ST_SIGIL, SYMBOL_OF_SYLVAIN])
                        && !has(ctx, player, HIGH_PRIESTS_SIGIL)
                    {
                        htmltext = "30070-05.html".to_owned();
                    } else if any(ctx, player, &[MIRIENS_2ND_SIGIL, MIRIENS_3RD_SIGIL]) {
                        htmltext = "30070-06.html".to_owned();
                    }
                }
                CAPTAIN_LUCAS => {
                    if all(ctx, player, &[MIRIENS_1ST_SIGIL, HIGH_PRIESTS_SIGIL]) {
                        if has(ctx, player, MARIAS_1ST_LETTER) {
                            ctx.take_items(player, MARIAS_1ST_LETTER, 1);
                            ctx.give_items(player, LUCASS_LETTER, 1);
                            qs.set_cond(ctx, 4, true);
                            htmltext = "30071-01.html".to_owned();
                        } else if any(
                            ctx,
                            player,
                            &[MARIAS_2ND_LETTER, CRETAS_1ST_LETTER, LUCILLAS_HANDBAG, CRERAS_PAINTING1, LUCASS_LETTER],
                        ) {
                            htmltext = "30071-02.html".to_owned();
                        } else if has(ctx, player, CRERAS_PAINTING2) {
                            htmltext = "30071-03.html".to_owned();
                        } else if has(ctx, player, CRERAS_PAINTING3) {
                            if count(ctx, player, BROWN_SCROLL_SCRAP) < 5 {
                                htmltext = "30071-05.html".to_owned();
                            } else {
                                htmltext = "30071-06.html".to_owned();
                            }
                        }
                    } else if any(
                        ctx,
                        player,
                        &[SYMBOL_OF_SYLVAIN, MIRIENS_2ND_SIGIL, MIRIENS_3RD_SIGIL, CRYSTAL_OF_PURITY1],
                    ) {
                        htmltext = "30071-07.html".to_owned();
                    }
                }
                WAREHOUSE_KEEPER_VALKON => {
                    if has(ctx, player, TRIFFS_RING) {
                        if !any(ctx, player, &[VALKONS_REQUEST, CRYSTAL_OF_PURITY2, SCRIPTURE_CHAPTER_2]) {
                            htmltext = "30103-01.html".to_owned();
                        } else if has(ctx, player, VALKONS_REQUEST)
                            && !any(ctx, player, &[CRYSTAL_OF_PURITY2, SCRIPTURE_CHAPTER_2])
                        {
                            htmltext = "30103-05.html".to_owned();
                        } else if has(ctx, player, CRYSTAL_OF_PURITY2)
                            && !any(ctx, player, &[VALKONS_REQUEST, SCRIPTURE_CHAPTER_2])
                        {
                            ctx.give_items(player, SCRIPTURE_CHAPTER_2, 1);
                            ctx.take_items(player, CRYSTAL_OF_PURITY2, 1);
                            htmltext = "30103-06.html".to_owned();
                        } else if has(ctx, player, SCRIPTURE_CHAPTER_2)
                            && !any(ctx, player, &[VALKONS_REQUEST, CRYSTAL_OF_PURITY2])
                        {
                            htmltext = "30103-07.html".to_owned();
                        }
                    }
                }
                MAGISTER_DIETER => {
                    if all(ctx, player, &[MIRIENS_3RD_SIGIL, CRONOS_SIGIL]) {
                        if has(ctx, player, CRONOS_LETTER) {
                            htmltext = "30111-01.html".to_owned();
                        } else if has(ctx, player, DIETERS_KEY) {
                            htmltext = "30111-06.html".to_owned();
                        } else if has(ctx, player, CRETAS_2ND_LETTER) {
                            htmltext = "30111-07.html".to_owned();
                        } else if all(ctx, player, &[DIETERS_DIARY, DIETERS_LETTER]) {
                            htmltext = "30111-10.html".to_owned();
                        } else if all(ctx, player, &[DIETERS_DIARY, RAUTS_LETTER_ENVELOPE]) {
                            htmltext = "30111-11.html".to_owned();
                        } else if has(ctx, player, DIETERS_DIARY)
                            && !any(ctx, player, &[DIETERS_LETTER, RAUTS_LETTER_ENVELOPE])
                        {
                            if all(ctx, player, &chapters) {
                                htmltext = "30111-13.html".to_owned();
                            } else {
                                htmltext = "30111-12.html".to_owned();
                            }
                        }
                    } else if has(ctx, player, SYMBOL_OF_CRONOS) {
                        htmltext = "30111-15.html".to_owned();
                    }
                }
                GRAND_MAGISTER_JUREK => {
                    if has(ctx, player, MIRIENS_2ND_SIGIL) {
                        if !any(ctx, player, &[GRAND_MAGISTER_SIGIL, SYMBOL_OF_JUREK]) {
                            htmltext = "30115-01.html".to_owned();
                        } else if has(ctx, player, JUREKS_LIST) {
                            if count(ctx, player, MONSTER_EYE_DESTROYER_SKIN)
                                + count(ctx, player, SHAMANS_NECKLACE)
                                + count(ctx, player, SHACKLES_SCALP)
                                < 12
                            {
                                htmltext = "30115-04.html".to_owned();
                            } else {
                                ctx.take_items(player, GRAND_MAGISTER_SIGIL, 1);
                                ctx.take_items(player, JUREKS_LIST, 1);
                                ctx.take_items(player, MONSTER_EYE_DESTROYER_SKIN, -1);
                                ctx.take_items(player, SHAMANS_NECKLACE, -1);
                                ctx.take_items(player, SHACKLES_SCALP, -1);
                                ctx.give_items(player, SYMBOL_OF_JUREK, 1);
                                qs.set_cond(ctx, 18, true);
                                htmltext = "30115-05.html".to_owned();
                            }
                        } else if has(ctx, player, SYMBOL_OF_JUREK) && !has(ctx, player, GRAND_MAGISTER_SIGIL) {
                            htmltext = "30115-06.html".to_owned();
                        }
                    } else if any(ctx, player, &[MIRIENS_1ST_SIGIL, MIRIENS_3RD_SIGIL]) {
                        htmltext = "30115-07.html".to_owned();
                    }
                }
                TRADER_EDROC => {
                    if has(ctx, player, DIETERS_DIARY) {
                        if has(ctx, player, DIETERS_LETTER) {
                            htmltext = "30230-01.html".to_owned();
                        } else if has(ctx, player, RAUTS_LETTER_ENVELOPE) {
                            htmltext = "30230-03.html".to_owned();
                        } else if any(ctx, player, &[STRONG_LIGUOR, TRIFFS_RING]) {
                            htmltext = "30230-04.html".to_owned();
                        }
                    }
                }
                WAREHOUSE_KEEPER_RAUT => {
                    if has(ctx, player, DIETERS_DIARY) {
                        if has(ctx, player, RAUTS_LETTER_ENVELOPE) {
                            htmltext = "30316-01.html".to_owned();
                        } else if all(ctx, player, &[SCRIPTURE_CHAPTER_1, STRONG_LIGUOR]) {
                            htmltext = "30316-04.html".to_owned();
                        } else if all(ctx, player, &[SCRIPTURE_CHAPTER_1, TRIFFS_RING]) {
                            htmltext = "30316-05.html".to_owned();
                        }
                    }
                }
                BLACKSMITH_POITAN => {
                    if has(ctx, player, TRIFFS_RING) {
                        if !any(ctx, player, &[POITANS_NOTES, CASIANS_LIST, SCRIPTURE_CHAPTER_4]) {
                            ctx.give_items(player, POITANS_NOTES, 1);
                            htmltext = "30458-01.html".to_owned();
                        } else if has(ctx, player, POITANS_NOTES)
                            && !any(ctx, player, &[CASIANS_LIST, SCRIPTURE_CHAPTER_4])
                        {
                            htmltext = "30458-02.html".to_owned();
                        } else if all(ctx, player, &[POITANS_NOTES, CASIANS_LIST])
                            && !has(ctx, player, SCRIPTURE_CHAPTER_4)
                        {
                            htmltext = "30458-03.html".to_owned();
                        } else if has(ctx, player, SCRIPTURE_CHAPTER_4)
                            && !any(ctx, player, &[POITANS_NOTES, CASIANS_LIST])
                        {
                            htmltext = "30458-04.html".to_owned();
                        }
                    }
                }
                MARIA => {
                    if all(ctx, player, &[MIRIENS_1ST_SIGIL, HIGH_PRIESTS_SIGIL]) {
                        if has(ctx, player, SYLVAINS_LETTER) {
                            htmltext = "30608-01.html".to_owned();
                        } else if has(ctx, player, MARIAS_1ST_LETTER) {
                            htmltext = "30608-03.html".to_owned();
                        } else if has(ctx, player, LUCASS_LETTER) {
                            ctx.give_items(player, MARIAS_2ND_LETTER, 1);
                            ctx.take_items(player, LUCASS_LETTER, 1);
                            qs.set_cond(ctx, 5, true);
                            htmltext = "30608-04.html".to_owned();
                        } else if has(ctx, player, MARIAS_2ND_LETTER) {
                            htmltext = "30608-05.html".to_owned();
                        } else if has(ctx, player, CRETAS_1ST_LETTER) {
                            htmltext = "30608-06.html".to_owned();
                        } else if has(ctx, player, LUCILLAS_HANDBAG) {
                            htmltext = "30608-09.html".to_owned();
                        } else if has(ctx, player, CRERAS_PAINTING1) {
                            ctx.take_items(player, CRERAS_PAINTING1, 1);
                            ctx.give_items(player, CRERAS_PAINTING2, 1);
                            qs.set_cond(ctx, 9, true);
                            htmltext = "30608-10.html".to_owned();
                        } else if has(ctx, player, CRERAS_PAINTING2) {
                            htmltext = "30608-11.html".to_owned();
                        } else if has(ctx, player, CRERAS_PAINTING3) {
                            if count(ctx, player, BROWN_SCROLL_SCRAP) < 5 {
                                qs.set_cond(ctx, 11, true);
                                htmltext = "30608-12.html".to_owned();
                            } else {
                                htmltext = "30608-13.html".to_owned();
                            }
                        } else if has(ctx, player, CRYSTAL_OF_PURITY1) {
                            htmltext = "30608-15.html".to_owned();
                        }
                    } else if any(ctx, player, &[SYMBOL_OF_SYLVAIN, MIRIENS_2ND_SIGIL]) {
                        htmltext = "30608-16.html".to_owned();
                    } else if has(ctx, player, MIRIENS_3RD_SIGIL) {
                        if !has(ctx, player, VALKONS_REQUEST) {
                            htmltext = "30608-17.html".to_owned();
                        } else {
                            ctx.take_items(player, VALKONS_REQUEST, 1);
                            ctx.give_items(player, CRYSTAL_OF_PURITY2, 1);
                            htmltext = "30608-18.html".to_owned();
                        }
                    }
                }
                ASTROLOGER_CRETA => {
                    if all(ctx, player, &[MIRIENS_1ST_SIGIL, HIGH_PRIESTS_SIGIL]) {
                        if has(ctx, player, MARIAS_2ND_LETTER) {
                            htmltext = "30609-01.html".to_owned();
                        } else if has(ctx, player, CRETAS_1ST_LETTER) {
                            htmltext = "30609-06.html".to_owned();
                        } else if has(ctx, player, LUCILLAS_HANDBAG) {
                            htmltext = "30609-07.html".to_owned();
                        } else if any(ctx, player, &[CRERAS_PAINTING1, CRERAS_PAINTING2, CRERAS_PAINTING3]) {
                            htmltext = "30609-10.html".to_owned();
                        }
                    } else if any(ctx, player, &[CRYSTAL_OF_PURITY1, SYMBOL_OF_SYLVAIN, MIRIENS_2ND_SIGIL]) {
                        htmltext = "30609-11.html".to_owned();
                    } else if has(ctx, player, MIRIENS_3RD_SIGIL) {
                        if has(ctx, player, DIETERS_KEY) {
                            htmltext = "30609-12.html".to_owned();
                        } else {
                            htmltext = "30609-15.html".to_owned();
                        }
                    }
                }
                ELDER_CRONOS => {
                    if has(ctx, player, MIRIENS_3RD_SIGIL) {
                        if !any(ctx, player, &[CRONOS_SIGIL, SYMBOL_OF_CRONOS]) {
                            htmltext = "30610-01.html".to_owned();
                        } else if has(ctx, player, CRONOS_SIGIL) {
                            if all(ctx, player, &chapters) {
                                htmltext = "30610-12.html".to_owned();
                            } else {
                                htmltext = "30610-11.html".to_owned();
                            }
                        } else if has(ctx, player, SYMBOL_OF_CRONOS) && !has(ctx, player, CRONOS_SIGIL) {
                            htmltext = "30610-15.html".to_owned();
                        }
                    }
                }
                DRUNKARD_TRIFF => {
                    if all(ctx, player, &[DIETERS_DIARY, SCRIPTURE_CHAPTER_1, STRONG_LIGUOR]) {
                        htmltext = "30611-01.html".to_owned();
                    } else if any(ctx, player, &[TRIFFS_RING, SYMBOL_OF_CRONOS]) {
                        htmltext = "30611-05.html".to_owned();
                    }
                }
                ELDER_CASIAN => {
                    if all(ctx, player, &[TRIFFS_RING, POITANS_NOTES]) {
                        if !has(ctx, player, CASIANS_LIST) {
                            if all(ctx, player, &[SCRIPTURE_CHAPTER_1, SCRIPTURE_CHAPTER_2, SCRIPTURE_CHAPTER_3]) {
                                htmltext = "30612-02.html".to_owned();
                            } else {
                                htmltext = "30612-01.html".to_owned();
                            }
                        } else if count(ctx, player, GHOULS_SKIN)
                            + count(ctx, player, MEDUSAS_BLOOD)
                            + count(ctx, player, FETTERED_SOULS_ICHOR)
                            + count(ctx, player, ENCHANTED_GARGOYLES_NAIL)
                            < 32
                        {
                            htmltext = "30612-05.html".to_owned();
                        } else {
                            htmltext = "30612-06.html".to_owned();
                        }
                    } else if all(
                        ctx,
                        player,
                        &[
                            TRIFFS_RING,
                            SCRIPTURE_CHAPTER_1,
                            SCRIPTURE_CHAPTER_2,
                            SCRIPTURE_CHAPTER_3,
                            SCRIPTURE_CHAPTER_4,
                        ],
                    ) && !any(ctx, player, &[POITANS_NOTES, CASIANS_LIST])
                    {
                        htmltext = "30612-08.html".to_owned();
                    }
                }
                _ => {}
            }
        } else if qs.is_completed(ctx) && npc.id == MAGISTER_MIRIEN {
            htmltext = ctx.get_already_completed_msg(player);
        }
        Some(htmltext)
    }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::ScriptRegistry;
    use l2_world::script::{player_class, race};

    use super::*;
    use crate::testing::{Pages, ScriptTest, names_to, rows};

    const NAME: &str = "Q00214_TrialOfTheScholar";

    const NPCS: &str = r#"
        <npc id="30070" type="Folk" level="70" name="High Priest Sylvain"/>
        <npc id="30071" type="Folk" level="70" name="Captain Lucas"/>
        <npc id="30461" type="Folk" level="70" name="Magister Mirien"/>
        <npc id="30608" type="Folk" level="70" name="Maria"/>
        <npc id="20158" type="Monster" level="30" name="Medusa"/>
        <npc id="20580" type="Monster" level="30" name="Leto Lizardman Warrior"/>"#;

    fn items() -> String {
        let mut s = String::from(
            r#"<item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="7562" type="EtcItem" name="Dimensional Diamond"><set name="is_stackable" val="true"/></item>"#,
        );
        for id in 2674..=2720 {
            let stack = if matches!(id, 2687 | 2695 | 2696 | 2697 | 2716 | 2717 | 2718 | 2719) {
                r#"<set name="is_stackable" val="true"/>"#
            } else {
                ""
            };
            s.push_str(&format!(
                r#"<item id="{id}" type="EtcItem" name="Q214 {id}"><set name="is_questitem" val="true"/>{stack}</item>"#
            ));
        }
        s
    }

    /// Every page links to every event the tests click (the real links are many).
    const EVENTS: [&str; 6] = ["ACCEPT", "30070-02.html", "30608-02.html", "30071-04.html", "30608-14.html", "30461-10.html"];

    const FILES: [&str; 14] = [
        "30461-01.html",
        "30461-02.html",
        "30461-03.htm",
        "30461-04a.htm",
        "30461-05.html",
        "30461-14.html",
        "30070-01.html",
        "30608-01.html",
        "30608-04.html",
        "30071-01.html",
        "30608-12.html",
        "30608-13.html",
        "30071-06.html",
        "30070-04.html",
    ];

    struct Npcs {
        mirien: Oid,
        sylvain: Oid,
        maria: Oid,
        lucas: Oid,
    }

    fn setup() -> (ScriptTest, Npcs) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00214TrialOfTheScholar);
        let mut pages = Pages::of(&Q00214TrialOfTheScholar);
        for file in FILES {
            pages = pages.page(file, &EVENTS);
        }
        for event in EVENTS.iter().filter(|e| **e != "ACCEPT") {
            pages = pages.page(event, &EVENTS);
        }
        let pages = pages.npcs(&[MAGISTER_MIRIEN, HIGH_PRIEST_SYLVAIN, MARIA, CAPTAIN_LUCAS]);
        let mut t = ScriptTest::with_pages(registry, NPCS, &items(), pages);
        let npcs = Npcs {
            mirien: t.spawn(MAGISTER_MIRIEN, 20),
            sylvain: t.spawn(HIGH_PRIEST_SYLVAIN, 30),
            maria: t.spawn(MARIA, 40),
            lucas: t.spawn(CAPTAIN_LUCAS, 50),
        };
        (t, npcs)
    }

    fn give(t: &mut ScriptTest, items: &[i32]) {
        t.world.run_as_script(NAME, |ctx| {
            for &item in items {
                ctx.give_items(Oid(1), item, 1);
            }
        });
        t.take_writes();
    }

    #[test]
    fn only_a_wizard_of_level_35_is_offered_the_quest() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 35, race::HUMAN, player_class::FIGHTER, vec![]);
        assert!(t.quest_page(1, npcs.mirien).contains("30461-01.html"));
        t.enter_as(2, 34, race::HUMAN, player_class::WIZARD, vec![]);
        assert!(t.quest_page(2, npcs.mirien).contains("30461-02.html"));
        t.enter_as(3, 35, race::DARK_ELF, player_class::DARK_WIZARD, vec![]);
        assert!(t.quest_page(3, npcs.mirien).contains("30461-03.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_first_sigil_sylvain_maria_and_lucas() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 35, race::HUMAN, player_class::WIZARD, vec![]);
        assert!(t.quest_page(1, npcs.mirien).contains("30461-03.htm"));
        assert!(t.event(1, NAME, "ACCEPT").contains("30461-04a.htm"));
        assert_eq!(t.count(1, DIMENSIONAL_DIAMOND), 168);
        assert_eq!(t.count(1, MIRIENS_1ST_SIGIL), 1);
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );

        assert!(t.quest_page(1, npcs.sylvain).contains("30070-01.html"));
        assert!(t.event(1, NAME, "30070-02.html").contains("30070-02.html"));
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);
        assert!(t.quest_page(1, npcs.maria).contains("30608-01.html"));
        assert!(t.event(1, NAME, "30608-02.html").contains("30608-02.html"));
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "3")]);
        assert!(t.quest_page(1, npcs.lucas).contains("30071-01.html"));
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "4")]);
        assert!(t.quest_page(1, npcs.maria).contains("30608-04.html"));
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "5")]);
    }

    #[test]
    fn five_brown_scrolls_then_the_crystal_of_purity() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 35, race::HUMAN, player_class::WIZARD, vec![]);
        assert!(t.quest_page(1, npcs.mirien).contains("30461-03.htm"));
        assert!(t.event(1, NAME, "ACCEPT").contains("30461-04a.htm"));
        give(&mut t, &[HIGH_PRIESTS_SIGIL, CRERAS_PAINTING3]);
        for _ in 0..5 {
            let mob = t.spawn(LETO_LIZARDMAN_WARRIOR, 60);
            t.kill(1, mob);
            t.world.delete_npc(mob);
        }
        assert_eq!(t.count(1, BROWN_SCROLL_SCRAP), 5);
        assert!(t.take_writes().contains(&rows::update(NAME, "cond", "12")));
        assert!(t.quest_page(1, npcs.maria).contains("30608-13.html"));
        assert!(t.event(1, NAME, "30608-14.html").contains("30608-14.html"));
        assert_eq!(t.count(1, CRYSTAL_OF_PURITY1), 1);
        assert_eq!(t.count(1, BROWN_SCROLL_SCRAP), 0);
        assert!(t.take_writes().contains(&rows::update(NAME, "cond", "13")));
        assert!(t.quest_page(1, npcs.sylvain).contains("30070-04.html"));
        assert_eq!(t.count(1, SYMBOL_OF_SYLVAIN), 1);
        assert!(t.take_writes().contains(&rows::update(NAME, "cond", "14")));
    }

    #[test]
    fn the_symbol_of_cronos_is_paid_with_the_mark() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 35, race::HUMAN, player_class::WIZARD, vec![]);
        assert!(t.quest_page(1, npcs.mirien).contains("30461-03.htm"));
        assert!(t.event(1, NAME, "ACCEPT").contains("30461-04a.htm"));
        t.take_writes();
        // The first sigil is taken away: Mirien looks at it before the third one.
        t.world.run_as_script(NAME, |ctx| {
            ctx.take_items(Oid(1), MIRIENS_1ST_SIGIL, -1);
        });
        give(&mut t, &[MIRIENS_3RD_SIGIL, SYMBOL_OF_CRONOS]);
        t.take();
        t.talk(1, npcs.mirien);
        t.bypass(1, "Script");
        let out = t.take();
        assert!(names_to(&out, 1).contains(&"SOCIAL_ACTION".to_owned()));
        assert_eq!(t.count(1, MARK_OF_SCHOLAR), 1);
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed")
            ]
        );
        assert!(t.quest_page(1, npcs.mirien).contains("already been completed"));
    }
}
