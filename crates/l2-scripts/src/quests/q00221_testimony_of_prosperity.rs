//! Quest 221: Testimony of Prosperity (Warehouse Keeper Parman in Dwarven Village: the Artisan
//! and Scavenger second class change, through the merchants and guild masters of Aden's
//! trading road, Emily, Lilith, Maestro Nikola and the Box of Titan).
// Java: dist/game/data/scripts/quests/Q00221_TestimonyOfProsperity/Q00221_TestimonyOfProsperity.java
//
// Pattern: the test chain of the "Testimony of ..." quests (see Q00222 for the shape): a long
// ladder of item tests per NPC, written with the three small helpers below (`has`, `all`, `any`
// for Java's `hasQuestItems(a)`, `hasQuestItems(a, b, ..)` and `hasAtLeastOneQuestItem(..)`).

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, STARTED, race};
use l2_world::subclass::CategoryType;
use l2_world::{Qs, QuestSound, Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const WAREHOUSE_KEEPER_WILFORD: i32 = 30005;
const WAREHOUSE_KEEPER_PARMAN: i32 = 30104;
const LILITH: i32 = 30368;
const GUARD_BRIGHT: i32 = 30466;
const TRADER_SHARI: i32 = 30517;
const TRADER_MION: i32 = 30519;
const IRON_GATES_LOCKIRIN: i32 = 30531;
const GOLDEN_WHEELS_SPIRON: i32 = 30532;
const SILVER_SCALES_BALANKI: i32 = 30533;
const BRONZE_KEYS_KEEF: i32 = 30534;
const GRAY_PILLAR_MEMBER_FILAUR: i32 = 30535;
const BLACK_ANVILS_ARIN: i32 = 30536;
const MARYSE_REDBONNET: i32 = 30553;
const MINER_BOLTER: i32 = 30554;
const CARRIER_TOROCCO: i32 = 30555;
const MASTER_TOMA: i32 = 30556;
const PIOTUR: i32 = 30597;
const EMILY: i32 = 30620;
const MAESTRO_NIKOLA: i32 = 30621;
const BOX_OF_TITAN: i32 = 30622;

// Items
const ADENA: i32 = 57;
const ANIMAL_SKIN: i32 = 1867;
const RECIPE_TITAN_KEY: i32 = 3023;
const KEY_OF_TITAN: i32 = 3030;
const RING_OF_TESTIMONY_1ST: i32 = 3239;
const RING_OF_TESTIMONY_2ND: i32 = 3240;
const OLD_ACCOUNT_BOOK: i32 = 3241;
const BLESSED_SEED: i32 = 3242;
const EMILYS_RECIPE: i32 = 3243;
const LILITHS_ELVEN_WAFER: i32 = 3244;
const MAPHR_TABLET_FRAGMENT: i32 = 3245;
const COLLECTION_LICENSE: i32 = 3246;
const LOCKIRINS_1ST_NOTICE: i32 = 3247;
const LOCKIRINS_2ND_NOTICE: i32 = 3248;
const LOCKIRINS_3RD_NOTICE: i32 = 3249;
const LOCKIRINS_4TH_NOTICE: i32 = 3250;
const LOCKIRINS_5TH_NOTICE: i32 = 3251;
const CONTRIBUTION_OF_SHARI: i32 = 3252;
const CONTRIBUTION_OF_MION: i32 = 3253;
const CONTRIBUTION_OF_MARYSE: i32 = 3254;
const MARYSES_REQUEST: i32 = 3255;
const CONTRIBUTION_OF_TOMA: i32 = 3256;
const RECEIPT_OF_BOLTER: i32 = 3257;
const RECEIPT_OF_CONTRIBUTION_1ST: i32 = 3258;
const RECEIPT_OF_CONTRIBUTION_2ND: i32 = 3259;
const RECEIPT_OF_CONTRIBUTION_3RD: i32 = 3260;
const RECEIPT_OF_CONTRIBUTION_4TH: i32 = 3261;
const RECEIPT_OF_CONTRIBUTION_5TH: i32 = 3262;
const PROCURATION_OF_TOROCCO: i32 = 3263;
const BRIGHTS_LIST: i32 = 3264;
const MANDRAGORA_PETAL: i32 = 3265;
const CRIMSON_MOSS: i32 = 3266;
const MANDRAGORA_BOUGUET: i32 = 3267;
const PARMANS_INSTRUCTIONS: i32 = 3268;
const PARMANS_LETTER: i32 = 3269;
const CLAY_DOUGH: i32 = 3270;
const PATTERN_OF_KEYHOLE: i32 = 3271;
const NIKOLAS_LIST: i32 = 3272;
const STAKATO_SHELL: i32 = 3273;
const TOAD_LORD_SAC: i32 = 3274;
const MARSH_SPIDER_THORN: i32 = 3275;
const CRYSTAL_BROOCH: i32 = 3428;

// Reward
const MARK_OF_PROSPERITY: i32 = 3238;
const DIMENSIONAL_DIAMOND: i32 = 7562;

// Monster
const MANDRAGORA_SPROUT1: i32 = 20154;
const MANDRAGORA_SAPLING: i32 = 20155;
const MANDRAGORA_BLOSSOM: i32 = 20156;
const MARSH_STAKATO: i32 = 20157;
const MANDRAGORA_SPROUT2: i32 = 20223;
const GIANT_CRIMSON_ANT: i32 = 20228;
const MARSH_STAKATO_WORKER: i32 = 20230;
const TOAD_LORD: i32 = 20231;
const MARSH_STAKATO_SOLDIER: i32 = 20232;
const MARSH_SPIDER: i32 = 20233;
const MARSH_STAKATO_DRONE: i32 = 20234;

// Misc
const MIN_LEVEL: i32 = 37;

/// Java `hasQuestItems(player, item)`.
fn has(ctx: &ScriptCtx<'_>, player: ObjectId, item: i32) -> bool {
    ctx.has_quest_items(player, item)
}

/// Java `hasQuestItems(player, a, b, ..)`: all of them.
fn all(ctx: &ScriptCtx<'_>, player: ObjectId, items: &[i32]) -> bool {
    ctx.has_all_quest_items(player, items)
}

/// Java `hasAtLeastOneQuestItem(player, a, b, ..)`.
fn any(ctx: &ScriptCtx<'_>, player: ObjectId, items: &[i32]) -> bool {
    ctx.has_at_least_one_quest_item(player, items)
}

/// Java `Q00221_TestimonyOfProsperity`.
pub struct Q00221TestimonyOfProsperity;

impl Script for Q00221TestimonyOfProsperity {
    fn name(&self) -> &'static str {
        "Q00221_TestimonyOfProsperity"
    }

    fn path(&self) -> &'static str {
        "quests/Q00221_TestimonyOfProsperity"
    }

    fn id(&self) -> i32 {
        221
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[WAREHOUSE_KEEPER_PARMAN]);
        s.add_talk_id(&[
            WAREHOUSE_KEEPER_PARMAN,
            WAREHOUSE_KEEPER_WILFORD,
            LILITH,
            GUARD_BRIGHT,
            TRADER_SHARI,
            TRADER_MION,
            IRON_GATES_LOCKIRIN,
            GOLDEN_WHEELS_SPIRON,
            SILVER_SCALES_BALANKI,
            BRONZE_KEYS_KEEF,
            GRAY_PILLAR_MEMBER_FILAUR,
            BLACK_ANVILS_ARIN,
            MARYSE_REDBONNET,
            MINER_BOLTER,
            CARRIER_TOROCCO,
            MASTER_TOMA,
            PIOTUR,
            EMILY,
            MAESTRO_NIKOLA,
            BOX_OF_TITAN,
        ]);
        s.add_kill_id(&[
            MANDRAGORA_SPROUT1,
            MANDRAGORA_SAPLING,
            MANDRAGORA_BLOSSOM,
            MARSH_STAKATO,
            MANDRAGORA_SPROUT2,
            GIANT_CRIMSON_ANT,
            MARSH_STAKATO_WORKER,
            TOAD_LORD,
            MARSH_STAKATO_SOLDIER,
            MARSH_SPIDER,
            MARSH_STAKATO_DRONE,
        ]);
        s.register_quest_items(&[
            RECIPE_TITAN_KEY,
            KEY_OF_TITAN,
            RING_OF_TESTIMONY_1ST,
            RING_OF_TESTIMONY_2ND,
            OLD_ACCOUNT_BOOK,
            BLESSED_SEED,
            EMILYS_RECIPE,
            LILITHS_ELVEN_WAFER,
            MAPHR_TABLET_FRAGMENT,
            COLLECTION_LICENSE,
            LOCKIRINS_1ST_NOTICE,
            LOCKIRINS_2ND_NOTICE,
            LOCKIRINS_3RD_NOTICE,
            LOCKIRINS_4TH_NOTICE,
            LOCKIRINS_5TH_NOTICE,
            CONTRIBUTION_OF_SHARI,
            CONTRIBUTION_OF_MION,
            CONTRIBUTION_OF_MARYSE,
            MARYSES_REQUEST,
            CONTRIBUTION_OF_TOMA,
            RECEIPT_OF_BOLTER,
            RECEIPT_OF_CONTRIBUTION_1ST,
            RECEIPT_OF_CONTRIBUTION_2ND,
            RECEIPT_OF_CONTRIBUTION_3RD,
            RECEIPT_OF_CONTRIBUTION_4TH,
            RECEIPT_OF_CONTRIBUTION_5TH,
            PROCURATION_OF_TOROCCO,
            BRIGHTS_LIST,
            MANDRAGORA_PETAL,
            CRIMSON_MOSS,
            MANDRAGORA_BOUGUET,
            PARMANS_INSTRUCTIONS,
            PARMANS_LETTER,
            CLAY_DOUGH,
            PATTERN_OF_KEYHOLE,
            NIKOLAS_LIST,
            STAKATO_SHELL,
            TOAD_LORD_SAC,
            MARSH_SPIDER_THORN,
            CRYSTAL_BROOCH,
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
                    if !has(ctx, player, RING_OF_TESTIMONY_1ST) {
                        ctx.give_items(player, RING_OF_TESTIMONY_1ST, 1);
                    }
                    ctx.play_sound(player, QuestSound::ItemsoundQuestMiddle);
                    if ctx.player_variable_int(player, "2ND_CLASS_DIAMOND_REWARD", 0) == 0 {
                        ctx.give_items(player, DIMENSIONAL_DIAMOND, 50);
                        ctx.set_player_variable(player, "2ND_CLASS_DIAMOND_REWARD", "1");
                        htmltext = Some("30104-04e.htm".to_owned());
                    } else {
                        htmltext = Some("30104-04.htm".to_owned());
                    }
                }
            }
            "30104-08.html" => {
                ctx.take_items(player, RING_OF_TESTIMONY_1ST, 1);
                ctx.give_items(player, RING_OF_TESTIMONY_2ND, 1);
                ctx.take_items(player, OLD_ACCOUNT_BOOK, 1);
                ctx.take_items(player, BLESSED_SEED, 1);
                ctx.take_items(player, EMILYS_RECIPE, 1);
                ctx.take_items(player, LILITHS_ELVEN_WAFER, 1);
                ctx.give_items(player, PARMANS_LETTER, 1);
                qs.set_cond(ctx, 4, true);
                htmltext = Some(event.to_owned());
            }
            "30104-04a.html" | "30104-04b.html" | "30104-04c.html" | "30104-04d.html" | "30104-05.html"
            | "30104-08a.html" | "30104-08b.html" | "30104-08c.html" | "30005-02.html" | "30005-03.html"
            | "30368-02.html" | "30466-02.html" | "30531-02.html" | "30620-02.html" | "30621-02.html"
            | "30621-03.html" => {
                htmltext = Some(event.to_owned());
            }
            "30005-04.html" => {
                ctx.give_items(player, CRYSTAL_BROOCH, 1);
                htmltext = Some(event.to_owned());
            }
            "30368-03.html" => {
                if has(ctx, player, CRYSTAL_BROOCH) {
                    ctx.give_items(player, LILITHS_ELVEN_WAFER, 1);
                    ctx.take_items(player, CRYSTAL_BROOCH, 1);
                    if all(ctx, player, &[OLD_ACCOUNT_BOOK, BLESSED_SEED, EMILYS_RECIPE]) {
                        qs.set_cond(ctx, 2, true);
                    }
                    htmltext = Some(event.to_owned());
                }
            }
            "30466-03.html" => {
                ctx.give_items(player, BRIGHTS_LIST, 1);
                htmltext = Some(event.to_owned());
            }
            "30531-03.html" => {
                ctx.give_items(player, COLLECTION_LICENSE, 1);
                ctx.give_items(player, LOCKIRINS_1ST_NOTICE, 1);
                ctx.give_items(player, LOCKIRINS_2ND_NOTICE, 1);
                ctx.give_items(player, LOCKIRINS_3RD_NOTICE, 1);
                ctx.give_items(player, LOCKIRINS_4TH_NOTICE, 1);
                ctx.give_items(player, LOCKIRINS_5TH_NOTICE, 1);
                htmltext = Some(event.to_owned());
            }
            "30534-03a.html" => {
                if ctx.get_quest_items_count(player, ADENA) < 5000 {
                    htmltext = Some(event.to_owned());
                } else if has(ctx, player, PROCURATION_OF_TOROCCO) {
                    ctx.take_items(player, ADENA, 5000);
                    ctx.give_items(player, RECEIPT_OF_CONTRIBUTION_3RD, 1);
                    ctx.take_items(player, PROCURATION_OF_TOROCCO, 1);
                    htmltext = Some("30534-03b.html".to_owned());
                }
            }
            "30555-02.html" => {
                ctx.give_items(player, PROCURATION_OF_TOROCCO, 1);
                htmltext = Some(event.to_owned());
            }
            "30597-02.html" => {
                ctx.give_items(player, BLESSED_SEED, 1);
                if all(
                    ctx,
                    player,
                    &[OLD_ACCOUNT_BOOK, EMILYS_RECIPE, LILITHS_ELVEN_WAFER],
                ) {
                    qs.set_cond(ctx, 2, true);
                }
                htmltext = Some(event.to_owned());
            }
            "30620-03.html" => {
                if has(ctx, player, MANDRAGORA_BOUGUET) {
                    ctx.give_items(player, EMILYS_RECIPE, 1);
                    ctx.take_items(player, MANDRAGORA_BOUGUET, 1);
                    if all(
                        ctx,
                        player,
                        &[OLD_ACCOUNT_BOOK, BLESSED_SEED, LILITHS_ELVEN_WAFER],
                    ) {
                        qs.set_cond(ctx, 2, true);
                    }
                    htmltext = Some(event.to_owned());
                }
            }
            "30621-04.html" => {
                ctx.give_items(player, CLAY_DOUGH, 1);
                qs.set_cond(ctx, 5, true);
                htmltext = Some(event.to_owned());
            }
            "30622-02.html" => {
                if has(ctx, player, CLAY_DOUGH) {
                    ctx.take_items(player, CLAY_DOUGH, 1);
                    ctx.give_items(player, PATTERN_OF_KEYHOLE, 1);
                    qs.set_cond(ctx, 6, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "30622-04.html" => {
                if has(ctx, player, KEY_OF_TITAN) {
                    ctx.take_items(player, KEY_OF_TITAN, 1);
                    ctx.give_items(player, MAPHR_TABLET_FRAGMENT, 1);
                    ctx.take_items(player, NIKOLAS_LIST, 1);
                    ctx.take_items(player, RECIPE_TITAN_KEY, 1);
                    ctx.take_items(player, STAKATO_SHELL, -1);
                    ctx.take_items(player, TOAD_LORD_SAC, -1);
                    ctx.take_items(player, MARSH_SPIDER_THORN, -1);
                    qs.set_cond(ctx, 9, true);
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
            MANDRAGORA_SPROUT1 | MANDRAGORA_SAPLING | MANDRAGORA_BLOSSOM | MANDRAGORA_SPROUT2 => {
                if all(ctx, killer, &[RING_OF_TESTIMONY_1ST, BRIGHTS_LIST])
                    && !has(ctx, killer, EMILYS_RECIPE)
                    && ctx.get_quest_items_count(killer, MANDRAGORA_PETAL) < 20
                {
                    ctx.give_items(killer, MANDRAGORA_PETAL, 1);
                    if ctx.get_quest_items_count(killer, MANDRAGORA_PETAL) == 20 {
                        ctx.play_sound(killer, QuestSound::ItemsoundQuestMiddle);
                    } else {
                        ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
                    }
                }
            }
            MARSH_STAKATO | MARSH_STAKATO_WORKER | MARSH_STAKATO_SOLDIER | MARSH_STAKATO_DRONE => {
                nikola_item(
                    ctx,
                    qs,
                    killer,
                    STAKATO_SHELL,
                    20,
                    (TOAD_LORD_SAC, MARSH_SPIDER_THORN),
                );
            }
            GIANT_CRIMSON_ANT => {
                if all(ctx, killer, &[RING_OF_TESTIMONY_1ST, BRIGHTS_LIST])
                    && !has(ctx, killer, EMILYS_RECIPE)
                    && ctx.get_quest_items_count(killer, CRIMSON_MOSS) < 10
                {
                    ctx.give_items(killer, CRIMSON_MOSS, 1);
                    if ctx.get_quest_items_count(killer, CRIMSON_MOSS) == 10 {
                        ctx.play_sound(killer, QuestSound::ItemsoundQuestMiddle);
                    } else {
                        ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
                    }
                }
            }
            TOAD_LORD => nikola_item(
                ctx,
                qs,
                killer,
                TOAD_LORD_SAC,
                10,
                (STAKATO_SHELL, MARSH_SPIDER_THORN),
            ),
            MARSH_SPIDER => {
                nikola_item(
                    ctx,
                    qs,
                    killer,
                    MARSH_SPIDER_THORN,
                    10,
                    (STAKATO_SHELL, TOAD_LORD_SAC),
                );
            }
            _ => {}
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        let state = qs.state(ctx);
        if state == CREATED {
            if npc.id == WAREHOUSE_KEEPER_PARMAN {
                let dwarf = ctx.player_race(player) == race::DWARF;
                let level_ok = ctx.player_level(player) >= MIN_LEVEL;
                if dwarf && level_ok && ctx.world().is_in_category(player, CategoryType::Dwarf2ndGroup) {
                    htmltext = "30104-03.htm".to_owned();
                } else if dwarf && level_ok {
                    htmltext = "30104-01a.html".to_owned();
                } else if dwarf {
                    htmltext = "30104-02.html".to_owned();
                } else {
                    htmltext = "30104-01.html".to_owned();
                }
            }
        } else if state == STARTED {
            if let Some(page) = started_page(ctx, qs, npc.id, player) {
                htmltext = page.to_owned();
            }
        } else if state == COMPLETED && npc.id == WAREHOUSE_KEEPER_PARMAN {
            htmltext = ctx.get_already_completed_msg(player);
        }
        Some(htmltext)
    }
}

/// The three monsters of Nikola's list (`item` up to `limit`; the middle sound at the last one,
/// cond 8 when the other two are complete as well: 10 of each, 20 shells).
fn nikola_item(ctx: &mut ScriptCtx<'_>, qs: Qs, killer: ObjectId, item: i32, limit: i64, others: (i32, i32)) {
    if all(ctx, killer, &[RING_OF_TESTIMONY_2ND, NIKOLAS_LIST])
        && !any(ctx, killer, &[CLAY_DOUGH, PATTERN_OF_KEYHOLE])
        && ctx.get_quest_items_count(killer, item) < limit
    {
        ctx.give_items(killer, item, 1);
        if ctx.get_quest_items_count(killer, item) == limit {
            ctx.play_sound(killer, QuestSound::ItemsoundQuestMiddle);
            let other_limit = |i: i32| if i == STAKATO_SHELL { 20 } else { 10 };
            if ctx.get_quest_items_count(killer, others.0) >= other_limit(others.0)
                && ctx.get_quest_items_count(killer, others.1) >= other_limit(others.1)
            {
                qs.set_cond(ctx, 8, false);
            }
        } else {
            ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
        }
    }
}

/// The `STARTED` branch of Java's `onTalk`: the page (or none, which keeps the no-quest message)
/// of an NPC for the items the player holds.
fn started_page(ctx: &mut ScriptCtx<'_>, qs: Qs, npc: i32, p: ObjectId) -> Option<&'static str> {
    let ring1 = has(ctx, p, RING_OF_TESTIMONY_1ST);
    let ring2 = has(ctx, p, RING_OF_TESTIMONY_2ND);
    let licensed = ring1 && has(ctx, p, COLLECTION_LICENSE);
    match npc {
        WAREHOUSE_KEEPER_PARMAN => {
            if ring1 {
                if all(
                    ctx,
                    p,
                    &[OLD_ACCOUNT_BOOK, BLESSED_SEED, EMILYS_RECIPE, LILITHS_ELVEN_WAFER],
                ) {
                    return Some("30104-06.html");
                }
                return Some("30104-05.html");
            } else if has(ctx, p, PARMANS_INSTRUCTIONS) {
                ctx.take_items(p, PARMANS_INSTRUCTIONS, 1);
                ctx.give_items(p, RING_OF_TESTIMONY_2ND, 1);
                ctx.give_items(p, PARMANS_LETTER, 1);
                qs.set_cond(ctx, 4, true);
                return Some("30104-10.html");
            } else if ring2 {
                if has(ctx, p, PARMANS_LETTER) {
                    return Some("30104-11.html");
                } else if any(ctx, p, &[CLAY_DOUGH, PATTERN_OF_KEYHOLE, NIKOLAS_LIST]) {
                    return Some("30104-12.html");
                } else if has(ctx, p, MAPHR_TABLET_FRAGMENT) {
                    ctx.give_adena(p, 217_682, true);
                    ctx.give_items(p, MARK_OF_PROSPERITY, 1);
                    ctx.add_exp_and_sp(p, 1_199_958, 80_080);
                    qs.exit_quest(ctx, false, true);
                    ctx.send_social_action(p, 3);
                    return Some("30104-13.html");
                }
            }
        }
        WAREHOUSE_KEEPER_WILFORD => {
            if ring1 {
                if !any(ctx, p, &[LILITHS_ELVEN_WAFER, CRYSTAL_BROOCH]) {
                    return Some("30005-01.html");
                } else if has(ctx, p, CRYSTAL_BROOCH) && !has(ctx, p, LILITHS_ELVEN_WAFER) {
                    return Some("30005-05.html");
                } else if has(ctx, p, LILITHS_ELVEN_WAFER) {
                    return Some("30005-06.html");
                }
            } else if ring2 {
                return Some("30005-07.html");
            }
        }
        LILITH => {
            if ring1 {
                if has(ctx, p, CRYSTAL_BROOCH) && !has(ctx, p, LILITHS_ELVEN_WAFER) {
                    return Some("30368-01.html");
                }
                return Some("30368-04.html");
            } else if ring2 {
                return Some("30368-05.html");
            }
        }
        GUARD_BRIGHT => {
            if ring1 {
                if !any(ctx, p, &[EMILYS_RECIPE, BRIGHTS_LIST, MANDRAGORA_BOUGUET]) {
                    return Some("30466-01.html");
                } else if has(ctx, p, BRIGHTS_LIST) && !has(ctx, p, EMILYS_RECIPE) {
                    if ctx.get_quest_items_count(p, MANDRAGORA_PETAL) < 20
                        || ctx.get_quest_items_count(p, CRIMSON_MOSS) < 10
                    {
                        return Some("30466-04.html");
                    }
                    ctx.take_items(p, BRIGHTS_LIST, 1);
                    ctx.take_items(p, MANDRAGORA_PETAL, -1);
                    ctx.take_items(p, CRIMSON_MOSS, -1);
                    ctx.give_items(p, MANDRAGORA_BOUGUET, 1);
                    return Some("30466-05.html");
                } else if has(ctx, p, MANDRAGORA_BOUGUET) && !any(ctx, p, &[EMILYS_RECIPE, BRIGHTS_LIST]) {
                    return Some("30466-06.html");
                } else if has(ctx, p, EMILYS_RECIPE) {
                    return Some("30466-07.html");
                }
            } else if ring2 {
                return Some("30466-08.html");
            }
        }
        TRADER_SHARI => {
            if licensed {
                if !any(
                    ctx,
                    p,
                    &[
                        RECEIPT_OF_CONTRIBUTION_1ST,
                        CONTRIBUTION_OF_SHARI,
                        LOCKIRINS_1ST_NOTICE,
                    ],
                ) {
                    ctx.give_items(p, CONTRIBUTION_OF_SHARI, 1);
                    return Some("30517-01.html");
                } else if has(ctx, p, CONTRIBUTION_OF_SHARI)
                    && !any(ctx, p, &[LOCKIRINS_1ST_NOTICE, RECEIPT_OF_CONTRIBUTION_1ST])
                {
                    return Some("30517-02.html");
                }
            }
        }
        TRADER_MION => {
            if licensed {
                if !any(
                    ctx,
                    p,
                    &[
                        RECEIPT_OF_CONTRIBUTION_2ND,
                        CONTRIBUTION_OF_MION,
                        LOCKIRINS_2ND_NOTICE,
                    ],
                ) {
                    ctx.give_items(p, CONTRIBUTION_OF_MION, 1);
                    return Some("30519-01.html");
                } else if has(ctx, p, CONTRIBUTION_OF_MION)
                    && !any(ctx, p, &[LOCKIRINS_2ND_NOTICE, RECEIPT_OF_CONTRIBUTION_2ND])
                {
                    return Some("30519-02.html");
                }
            }
        }
        IRON_GATES_LOCKIRIN => {
            if ring1 {
                if !any(ctx, p, &[COLLECTION_LICENSE, OLD_ACCOUNT_BOOK]) {
                    return Some("30531-01.html");
                } else if has(ctx, p, COLLECTION_LICENSE) {
                    if all(
                        ctx,
                        p,
                        &[
                            RECEIPT_OF_CONTRIBUTION_1ST,
                            RECEIPT_OF_CONTRIBUTION_2ND,
                            RECEIPT_OF_CONTRIBUTION_3RD,
                            RECEIPT_OF_CONTRIBUTION_4TH,
                            RECEIPT_OF_CONTRIBUTION_5TH,
                        ],
                    ) {
                        ctx.give_items(p, OLD_ACCOUNT_BOOK, 1);
                        ctx.take_items(p, COLLECTION_LICENSE, 1);
                        ctx.take_items(p, RECEIPT_OF_CONTRIBUTION_1ST, 1);
                        ctx.take_items(p, RECEIPT_OF_CONTRIBUTION_2ND, 1);
                        ctx.take_items(p, RECEIPT_OF_CONTRIBUTION_3RD, 1);
                        ctx.take_items(p, RECEIPT_OF_CONTRIBUTION_4TH, 1);
                        ctx.take_items(p, RECEIPT_OF_CONTRIBUTION_5TH, 1);
                        ctx.play_sound(p, QuestSound::ItemsoundQuestMiddle);
                        if all(ctx, p, &[BLESSED_SEED, EMILYS_RECIPE, LILITHS_ELVEN_WAFER]) {
                            qs.set_cond(ctx, 2, true);
                        }
                        return Some("30531-05.html");
                    }
                    return Some("30531-04.html");
                } else if has(ctx, p, OLD_ACCOUNT_BOOK) && !has(ctx, p, COLLECTION_LICENSE) {
                    return Some("30531-06.html");
                }
            } else if ring2 {
                return Some("30531-07.html");
            }
        }
        GOLDEN_WHEELS_SPIRON => {
            if licensed {
                if has(ctx, p, LOCKIRINS_1ST_NOTICE)
                    && !any(ctx, p, &[CONTRIBUTION_OF_SHARI, RECEIPT_OF_CONTRIBUTION_1ST])
                {
                    ctx.take_items(p, LOCKIRINS_1ST_NOTICE, 1);
                    return Some("30532-01.html");
                } else if !any(
                    ctx,
                    p,
                    &[
                        RECEIPT_OF_CONTRIBUTION_1ST,
                        CONTRIBUTION_OF_SHARI,
                        LOCKIRINS_1ST_NOTICE,
                    ],
                ) {
                    return Some("30532-02.html");
                } else if has(ctx, p, CONTRIBUTION_OF_SHARI)
                    && !any(ctx, p, &[RECEIPT_OF_CONTRIBUTION_1ST, LOCKIRINS_1ST_NOTICE])
                {
                    ctx.take_items(p, CONTRIBUTION_OF_SHARI, 1);
                    ctx.give_items(p, RECEIPT_OF_CONTRIBUTION_1ST, 1);
                    return Some("30532-03.html");
                } else if has(ctx, p, RECEIPT_OF_CONTRIBUTION_1ST)
                    && !any(ctx, p, &[CONTRIBUTION_OF_SHARI, LOCKIRINS_1ST_NOTICE])
                {
                    return Some("30532-04.html");
                }
            }
        }
        SILVER_SCALES_BALANKI => {
            if licensed {
                let contributions = ctx.get_quest_items_count(p, CONTRIBUTION_OF_MION)
                    + ctx.get_quest_items_count(p, CONTRIBUTION_OF_MARYSE);
                if has(ctx, p, LOCKIRINS_2ND_NOTICE)
                    && !has(ctx, p, RECEIPT_OF_CONTRIBUTION_2ND)
                    && contributions < 2
                {
                    ctx.take_items(p, LOCKIRINS_2ND_NOTICE, 1);
                    return Some("30533-01.html");
                } else if !any(ctx, p, &[LOCKIRINS_2ND_NOTICE, RECEIPT_OF_CONTRIBUTION_2ND])
                    && contributions < 2
                {
                    return Some("30533-02.html");
                } else if !any(ctx, p, &[LOCKIRINS_2ND_NOTICE, RECEIPT_OF_CONTRIBUTION_2ND])
                    && all(ctx, p, &[CONTRIBUTION_OF_MION, CONTRIBUTION_OF_MARYSE])
                {
                    ctx.take_items(p, CONTRIBUTION_OF_MION, 1);
                    ctx.take_items(p, CONTRIBUTION_OF_MARYSE, 1);
                    ctx.give_items(p, RECEIPT_OF_CONTRIBUTION_2ND, 1);
                    return Some("30533-03.html");
                } else if !has(ctx, p, LOCKIRINS_2ND_NOTICE)
                    && has(ctx, p, RECEIPT_OF_CONTRIBUTION_2ND)
                    && !all(ctx, p, &[CONTRIBUTION_OF_MION, CONTRIBUTION_OF_MARYSE])
                {
                    return Some("30533-04.html");
                }
            }
        }
        BRONZE_KEYS_KEEF => {
            if licensed {
                if has(ctx, p, LOCKIRINS_3RD_NOTICE)
                    && !any(ctx, p, &[RECEIPT_OF_CONTRIBUTION_3RD, PROCURATION_OF_TOROCCO])
                {
                    ctx.take_items(p, LOCKIRINS_3RD_NOTICE, 1);
                    return Some("30534-01.html");
                } else if !any(
                    ctx,
                    p,
                    &[
                        RECEIPT_OF_CONTRIBUTION_3RD,
                        PROCURATION_OF_TOROCCO,
                        LOCKIRINS_3RD_NOTICE,
                    ],
                ) {
                    return Some("30534-02.html");
                } else if has(ctx, p, PROCURATION_OF_TOROCCO)
                    && !any(ctx, p, &[LOCKIRINS_3RD_NOTICE, RECEIPT_OF_CONTRIBUTION_3RD])
                {
                    return Some("30534-03.html");
                } else if has(ctx, p, RECEIPT_OF_CONTRIBUTION_3RD)
                    && !any(ctx, p, &[PROCURATION_OF_TOROCCO, LOCKIRINS_3RD_NOTICE])
                {
                    return Some("30534-04.html");
                }
            }
        }
        GRAY_PILLAR_MEMBER_FILAUR => {
            if licensed {
                if has(ctx, p, LOCKIRINS_4TH_NOTICE)
                    && !any(ctx, p, &[RECEIPT_OF_CONTRIBUTION_4TH, RECEIPT_OF_BOLTER])
                {
                    ctx.take_items(p, LOCKIRINS_4TH_NOTICE, 1);
                    return Some("30535-01.html");
                } else if !any(
                    ctx,
                    p,
                    &[
                        RECEIPT_OF_CONTRIBUTION_4TH,
                        RECEIPT_OF_BOLTER,
                        LOCKIRINS_4TH_NOTICE,
                    ],
                ) {
                    return Some("30535-02.html");
                } else if has(ctx, p, RECEIPT_OF_BOLTER)
                    && !any(ctx, p, &[RECEIPT_OF_CONTRIBUTION_4TH, LOCKIRINS_4TH_NOTICE])
                {
                    ctx.take_items(p, RECEIPT_OF_BOLTER, 1);
                    ctx.give_items(p, RECEIPT_OF_CONTRIBUTION_4TH, 1);
                    return Some("30535-03.html");
                } else if has(ctx, p, RECEIPT_OF_CONTRIBUTION_4TH)
                    && !any(ctx, p, &[RECEIPT_OF_BOLTER, LOCKIRINS_4TH_NOTICE])
                {
                    return Some("30535-04.html");
                }
            }
        }
        BLACK_ANVILS_ARIN => {
            if licensed {
                if has(ctx, p, LOCKIRINS_5TH_NOTICE)
                    && !any(ctx, p, &[RECEIPT_OF_CONTRIBUTION_5TH, CONTRIBUTION_OF_TOMA])
                {
                    ctx.take_items(p, LOCKIRINS_5TH_NOTICE, 1);
                    return Some("30536-01.html");
                } else if !any(
                    ctx,
                    p,
                    &[
                        RECEIPT_OF_CONTRIBUTION_5TH,
                        CONTRIBUTION_OF_TOMA,
                        LOCKIRINS_5TH_NOTICE,
                    ],
                ) {
                    return Some("30536-02.html");
                } else if has(ctx, p, CONTRIBUTION_OF_TOMA)
                    && !any(ctx, p, &[RECEIPT_OF_CONTRIBUTION_5TH, LOCKIRINS_5TH_NOTICE])
                {
                    ctx.take_items(p, CONTRIBUTION_OF_TOMA, 1);
                    ctx.give_items(p, RECEIPT_OF_CONTRIBUTION_5TH, 1);
                    return Some("30536-03.html");
                } else if has(ctx, p, RECEIPT_OF_CONTRIBUTION_5TH)
                    && !any(ctx, p, &[CONTRIBUTION_OF_TOMA, LOCKIRINS_5TH_NOTICE])
                {
                    return Some("30536-04.html");
                }
            }
        }
        MARYSE_REDBONNET => {
            if licensed {
                if !any(
                    ctx,
                    p,
                    &[
                        RECEIPT_OF_CONTRIBUTION_2ND,
                        CONTRIBUTION_OF_MARYSE,
                        LOCKIRINS_2ND_NOTICE,
                        MARYSES_REQUEST,
                    ],
                ) {
                    ctx.give_items(p, MARYSES_REQUEST, 1);
                    return Some("30553-01.html");
                } else if has(ctx, p, MARYSES_REQUEST)
                    && !any(
                        ctx,
                        p,
                        &[
                            RECEIPT_OF_CONTRIBUTION_2ND,
                            CONTRIBUTION_OF_MARYSE,
                            LOCKIRINS_2ND_NOTICE,
                        ],
                    )
                {
                    if ctx.get_quest_items_count(p, ANIMAL_SKIN) < 10 {
                        return Some("30553-02.html");
                    }
                    ctx.take_items(p, ANIMAL_SKIN, 10);
                    ctx.give_items(p, CONTRIBUTION_OF_MARYSE, 1);
                    ctx.take_items(p, MARYSES_REQUEST, 1);
                    return Some("30553-03.html");
                } else if has(ctx, p, CONTRIBUTION_OF_MARYSE)
                    && !any(
                        ctx,
                        p,
                        &[RECEIPT_OF_CONTRIBUTION_2ND, LOCKIRINS_2ND_NOTICE, MARYSES_REQUEST],
                    )
                {
                    return Some("30553-04.html");
                }
            }
        }
        MINER_BOLTER => {
            if licensed {
                if !any(
                    ctx,
                    p,
                    &[
                        RECEIPT_OF_CONTRIBUTION_4TH,
                        RECEIPT_OF_BOLTER,
                        LOCKIRINS_4TH_NOTICE,
                    ],
                ) {
                    ctx.give_items(p, RECEIPT_OF_BOLTER, 1);
                    return Some("30554-01.html");
                } else if has(ctx, p, RECEIPT_OF_BOLTER)
                    && !any(ctx, p, &[RECEIPT_OF_CONTRIBUTION_4TH, LOCKIRINS_4TH_NOTICE])
                {
                    return Some("30554-02.html");
                }
            }
        }
        CARRIER_TOROCCO => {
            if licensed {
                if !any(
                    ctx,
                    p,
                    &[
                        RECEIPT_OF_CONTRIBUTION_3RD,
                        PROCURATION_OF_TOROCCO,
                        LOCKIRINS_3RD_NOTICE,
                    ],
                ) {
                    return Some("30555-01.html");
                } else if has(ctx, p, PROCURATION_OF_TOROCCO)
                    && !any(ctx, p, &[RECEIPT_OF_CONTRIBUTION_3RD, LOCKIRINS_3RD_NOTICE])
                {
                    return Some("30555-03.html");
                }
            }
        }
        MASTER_TOMA => {
            if licensed {
                if !any(
                    ctx,
                    p,
                    &[
                        RECEIPT_OF_CONTRIBUTION_5TH,
                        CONTRIBUTION_OF_TOMA,
                        LOCKIRINS_5TH_NOTICE,
                    ],
                ) {
                    ctx.give_items(p, CONTRIBUTION_OF_TOMA, 1);
                    return Some("30556-01.html");
                } else if has(ctx, p, CONTRIBUTION_OF_TOMA)
                    && !any(ctx, p, &[RECEIPT_OF_CONTRIBUTION_5TH, LOCKIRINS_5TH_NOTICE])
                {
                    return Some("30556-02.html");
                }
            }
        }
        PIOTUR => {
            if ring1 {
                if !has(ctx, p, BLESSED_SEED) {
                    return Some("30597-01.html");
                }
                return Some("30597-03.html");
            } else if ring2 {
                return Some("30597-04.html");
            }
        }
        EMILY => {
            if ring1 {
                if has(ctx, p, MANDRAGORA_BOUGUET) && !any(ctx, p, &[EMILYS_RECIPE, BRIGHTS_LIST]) {
                    return Some("30620-01.html");
                } else if has(ctx, p, EMILYS_RECIPE) {
                    return Some("30620-04.html");
                }
            } else if ring2 {
                return Some("30620-05.html");
            }
        }
        MAESTRO_NIKOLA => {
            if ring2 {
                if !any(
                    ctx,
                    p,
                    &[
                        CLAY_DOUGH,
                        PATTERN_OF_KEYHOLE,
                        NIKOLAS_LIST,
                        MAPHR_TABLET_FRAGMENT,
                    ],
                ) {
                    ctx.take_items(p, PARMANS_LETTER, 1);
                    return Some("30621-01.html");
                } else if has(ctx, p, CLAY_DOUGH)
                    && !any(ctx, p, &[PATTERN_OF_KEYHOLE, NIKOLAS_LIST, MAPHR_TABLET_FRAGMENT])
                {
                    return Some("30621-05.html");
                } else if has(ctx, p, PATTERN_OF_KEYHOLE)
                    && !any(ctx, p, &[CLAY_DOUGH, NIKOLAS_LIST, MAPHR_TABLET_FRAGMENT])
                {
                    ctx.give_items(p, RECIPE_TITAN_KEY, 1);
                    ctx.take_items(p, PATTERN_OF_KEYHOLE, 1);
                    ctx.give_items(p, NIKOLAS_LIST, 1);
                    qs.set_cond(ctx, 7, true);
                    return Some("30621-06.html");
                } else if has(ctx, p, NIKOLAS_LIST)
                    && !any(
                        ctx,
                        p,
                        &[
                            CLAY_DOUGH,
                            PATTERN_OF_KEYHOLE,
                            MAPHR_TABLET_FRAGMENT,
                            KEY_OF_TITAN,
                        ],
                    )
                {
                    return Some("30621-07.html");
                } else if all(ctx, p, &[NIKOLAS_LIST, KEY_OF_TITAN])
                    && !any(ctx, p, &[CLAY_DOUGH, PATTERN_OF_KEYHOLE, MAPHR_TABLET_FRAGMENT])
                {
                    return Some("30621-08.html");
                } else if has(ctx, p, MAPHR_TABLET_FRAGMENT)
                    && !any(ctx, p, &[CLAY_DOUGH, PATTERN_OF_KEYHOLE, NIKOLAS_LIST])
                {
                    return Some("30621-09.html");
                }
            }
        }
        BOX_OF_TITAN => {
            if ring2 {
                if has(ctx, p, CLAY_DOUGH) && !has(ctx, p, PATTERN_OF_KEYHOLE) {
                    return Some("30622-01.html");
                } else if has(ctx, p, KEY_OF_TITAN) && !has(ctx, p, MAPHR_TABLET_FRAGMENT) {
                    return Some("30622-03.html");
                } else if !any(ctx, p, &[KEY_OF_TITAN, CLAY_DOUGH]) {
                    return Some("30622-05.html");
                }
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
    use l2_world::script::{player_class, race};

    use super::*;
    use crate::testing::{Pages, ScriptTest, rows};

    const NAME: &str = "Q00221_TestimonyOfProsperity";

    const NPCS: &str = r#"
        <npc id="30104" type="Folk" level="70" name="Warehouse Keeper Parman"/>
        <npc id="30531" type="Folk" level="70" name="Iron Gates Lockirin"/>
        <npc id="30532" type="Folk" level="70" name="Golden Wheels Spiron"/>
        <npc id="30621" type="Folk" level="70" name="Maestro Nikola"/>
        <npc id="20231" type="Monster" level="40" name="Toad Lord"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="7562" type="EtcItem" name="Dimensional Diamond"><set name="is_stackable" val="true"/></item>
        <item id="3238" type="EtcItem" name="Mark of Prosperity"><set name="is_questitem" val="true"/></item>
        <item id="3239" type="EtcItem" name="Ring of Testimony 1st"><set name="is_questitem" val="true"/></item>
        <item id="3240" type="EtcItem" name="Ring of Testimony 2nd"><set name="is_questitem" val="true"/></item>
        <item id="3241" type="EtcItem" name="Old Account Book"><set name="is_questitem" val="true"/></item>
        <item id="3245" type="EtcItem" name="Maphr Tablet Fragment"><set name="is_questitem" val="true"/></item>
        <item id="3246" type="EtcItem" name="Collection License"><set name="is_questitem" val="true"/></item>
        <item id="3247" type="EtcItem" name="Lockirin 1st Notice"><set name="is_questitem" val="true"/></item>
        <item id="3248" type="EtcItem" name="Lockirin 2nd Notice"><set name="is_questitem" val="true"/></item>
        <item id="3249" type="EtcItem" name="Lockirin 3rd Notice"><set name="is_questitem" val="true"/></item>
        <item id="3250" type="EtcItem" name="Lockirin 4th Notice"><set name="is_questitem" val="true"/></item>
        <item id="3251" type="EtcItem" name="Lockirin 5th Notice"><set name="is_questitem" val="true"/></item>
        <item id="3252" type="EtcItem" name="Contribution of Shari"><set name="is_questitem" val="true"/></item>
        <item id="3258" type="EtcItem" name="Receipt 1st"><set name="is_questitem" val="true"/></item>
        <item id="3259" type="EtcItem" name="Receipt 2nd"><set name="is_questitem" val="true"/></item>
        <item id="3260" type="EtcItem" name="Receipt 3rd"><set name="is_questitem" val="true"/></item>
        <item id="3261" type="EtcItem" name="Receipt 4th"><set name="is_questitem" val="true"/></item>
        <item id="3262" type="EtcItem" name="Receipt 5th"><set name="is_questitem" val="true"/></item>
        <item id="3270" type="EtcItem" name="Clay Dough"><set name="is_questitem" val="true"/></item>
        <item id="3272" type="EtcItem" name="Nikola's List"><set name="is_questitem" val="true"/></item>
        <item id="3273" type="EtcItem" name="Stakato Shell"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="3274" type="EtcItem" name="Toad Lord Sac"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="3275" type="EtcItem" name="Marsh Spider Thorn"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="3269" type="EtcItem" name="Parman's Letter"><set name="is_questitem" val="true"/></item>"#;

    /// The pages of the quest with the links of the real pages.
    fn pages() -> Pages {
        Pages::of(&Q00221TestimonyOfProsperity)
            .plain(&[
                "30104-01.html",
                "30104-01a.html",
                "30104-02.html",
                "30104-04.htm",
                "30104-04e.htm",
                "30104-13.html",
                "30531-03.html",
            ])
            .page("30104-03.htm", &["ACCEPT"])
            .page("30531-01.html", &["30531-02.html"])
            .page("30531-02.html", &["30531-03.html"])
            .plain(&["30532-01.html", "30532-02.html", "30532-03.html", "30532-04.html"])
            .plain(&["30621-06.html", "30621-07.html"])
            .npcs(&[
                WAREHOUSE_KEEPER_PARMAN,
                IRON_GATES_LOCKIRIN,
                GOLDEN_WHEELS_SPIRON,
                MAESTRO_NIKOLA,
            ])
    }

    struct Npcs {
        parman: Oid,
        lockirin: Oid,
        spiron: Oid,
        nikola: Oid,
    }

    fn setup() -> (ScriptTest, Npcs) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00221TestimonyOfProsperity);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages());
        let npcs = Npcs {
            parman: t.spawn(WAREHOUSE_KEEPER_PARMAN, 20),
            lockirin: t.spawn(IRON_GATES_LOCKIRIN, 30),
            spiron: t.spawn(GOLDEN_WHEELS_SPIRON, 40),
            nikola: t.spawn(MAESTRO_NIKOLA, 50),
        };
        (t, npcs)
    }

    /// Started with the first ring through the script's own `ACCEPT` (the category gate of the
    /// start page depends on the class tables, which the test world has not).
    fn start_by_hand(t: &mut ScriptTest) {
        t.world.run_as_script(NAME, |ctx| {
            ctx.get_quest_state(Oid(1), true);
            let out = Q00221TestimonyOfProsperity.on_event(ctx, "ACCEPT", None, Some(Oid(1)));
            assert_eq!(out.as_deref(), Some("30104-04e.htm"));
        });
    }

    #[test]
    fn the_start_is_gated_by_race_and_level() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 40, race::HUMAN, player_class::FIGHTER, vec![]);
        assert!(t.quest_page(1, npcs.parman).contains("30104-01.html"));
        t.enter_as(2, 36, race::DWARF, player_class::DWARVEN_FIGHTER, vec![]);
        assert!(t.quest_page(2, npcs.parman).contains("30104-02.html"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_accept_gives_the_first_ring_and_50_diamonds() {
        let (mut t, _npcs) = setup();
        t.enter_as(1, 40, race::DWARF, player_class::ARTISAN, vec![]);
        start_by_hand(&mut t);
        assert_eq!(t.count(1, RING_OF_TESTIMONY_1ST), 1);
        assert_eq!(t.count(1, DIMENSIONAL_DIAMOND), 50);
        assert!(t.take_writes().contains(&rows::insert(NAME, "cond", "1")));
    }

    #[test]
    fn lockirin_hands_out_the_license_and_the_five_notices() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 40, race::DWARF, player_class::ARTISAN, vec![]);
        start_by_hand(&mut t);
        t.take_writes();
        assert!(t.quest_page(1, npcs.lockirin).contains("30531-01.html"));
        assert!(t.event(1, NAME, "30531-02.html").contains("30531-02.html"));
        assert!(t.event(1, NAME, "30531-03.html").contains("30531-03.html"));
        for item in [
            COLLECTION_LICENSE,
            LOCKIRINS_1ST_NOTICE,
            LOCKIRINS_2ND_NOTICE,
            LOCKIRINS_3RD_NOTICE,
            LOCKIRINS_4TH_NOTICE,
            LOCKIRINS_5TH_NOTICE,
        ] {
            assert_eq!(t.count(1, item), 1);
        }
        // Spiron takes the first notice.
        assert!(t.quest_page(1, npcs.spiron).contains("30532-01.html"));
        assert_eq!(t.count(1, LOCKIRINS_1ST_NOTICE), 0);
    }

    #[test]
    fn the_toad_lord_sacs_stop_at_ten_and_nikola_hands_over_the_key_list() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 40, race::DWARF, player_class::ARTISAN, vec![]);
        start_by_hand(&mut t);
        t.world.run_as_script(NAME, |ctx| {
            ctx.give_items(Oid(1), RING_OF_TESTIMONY_2ND, 1);
            ctx.give_items(Oid(1), NIKOLAS_LIST, 1);
            ctx.give_items(Oid(1), STAKATO_SHELL, 20);
            ctx.give_items(Oid(1), MARSH_SPIDER_THORN, 10);
        });
        t.take_writes();
        for _ in 0..11 {
            let toad = t.spawn(TOAD_LORD, 60);
            t.kill(1, toad);
            t.world.delete_npc(toad);
        }
        assert_eq!(t.count(1, TOAD_LORD_SAC), 10);
        // The tenth sac completes the list: a quiet `setCond(8)`.
        assert!(t.take_writes().contains(&rows::update(NAME, "cond", "8")));
        let _ = npcs.nikola;
    }

    #[test]
    fn parman_pays_the_reward_for_the_tablet_fragment() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 40, race::DWARF, player_class::ARTISAN, vec![]);
        start_by_hand(&mut t);
        t.world.run_as_script(NAME, |ctx| {
            ctx.take_items(Oid(1), RING_OF_TESTIMONY_1ST, 1);
            ctx.give_items(Oid(1), RING_OF_TESTIMONY_2ND, 1);
            ctx.give_items(Oid(1), MAPHR_TABLET_FRAGMENT, 1);
        });
        t.take_writes();
        assert!(t.quest_page(1, npcs.parman).contains("30104-13.html"));
        assert_eq!(t.count(1, MARK_OF_PROSPERITY), 1);
        assert_eq!(t.count(1, 57), 217_682);
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed"),
            ]
        );
        assert!(
            t.quest_page(1, npcs.parman)
                .contains("This quest has already been completed.")
        );
    }
}
