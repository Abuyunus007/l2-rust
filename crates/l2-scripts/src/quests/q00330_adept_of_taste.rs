//! Quest 330: Adept of Taste (Jonas wants five ingredients from the NPCs of the area, cooks a
//! dish of them, Mirien reviews it and Jonas pays by the review).
// Java: dist/game/data/scripts/quests/Q00330_AdeptOfTaste/Q00330_AdeptOfTaste.java
//
// Pattern: a repeatable quest with intermediate items, kills that fill the intermediate items
// (`docs/design/scripts.md`, "Quest patterns": kill count with a drop chance).

use l2_util::ObjectId;
use l2_world::script::{QuestSound, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const ACCESSORY_MERCHANT_SONIA: i32 = 30062;
const PRIESTESS_GLYVKA: i32 = 30067;
const MAGISTER_ROLLANT: i32 = 30069;
const GUARD_JACOB: i32 = 30073;
const GROCER_PANO: i32 = 30078;
const MAGISTER_MIRIEN: i32 = 30461;
const JONAS: i32 = 30469;

// Items
const INGREDIENT_LIST: i32 = 1420;
const SONIAS_BOTANY_BOOK: i32 = 1421;
const RED_MANDRAGORA_ROOT: i32 = 1422;
const WHITE_MANDRAGORA_ROOT: i32 = 1423;
const RED_MANDRAGORA_SAP: i32 = 1424;
const WHITE_MANDRAGORA_SAP: i32 = 1425;
const JACOBS_INSECT_BOOK: i32 = 1426;
const NECTAR: i32 = 1427;
const ROYAL_JELLY: i32 = 1428;
const HONEY: i32 = 1429;
const GOLDEN_HONEY: i32 = 1430;
const PANOS_CONTRACT: i32 = 1431;
const HOBGOBLIN_AMULET: i32 = 1432;
const DIONIAN_POTATO: i32 = 1433;
const GLYVKAS_BOTANY_BOOK: i32 = 1434;
const GREEN_MARSH_MOSS: i32 = 1435;
const BROWN_MARSH_MOSS: i32 = 1436;
const GREEN_MOSS_BUNDLE: i32 = 1437;
const BROWN_MOSS_BUNDLE: i32 = 1438;
const ROLLANTS_CREATURE_BOOK: i32 = 1439;
const BODY_OF_MONSTER_EYE: i32 = 1440;
const MONSTER_EYE_MEAT: i32 = 1441;
const JONASS_1ST_STEAK_DISH: i32 = 1442;
const JONASS_2ND_STEAK_DISH: i32 = 1443;
const JONASS_3RD_STEAK_DISH: i32 = 1444;
const JONASS_4TH_STEAK_DISH: i32 = 1445;
const JONASS_5TH_STEAK_DISH: i32 = 1446;
const MIRIENS_REVIEW_1: i32 = 1447;
const MIRIENS_REVIEW_2: i32 = 1448;
const MIRIENS_REVIEW_3: i32 = 1449;
const MIRIENS_REVIEW_4: i32 = 1450;
const MIRIENS_REVIEW_5: i32 = 1451;

// Reward
const JONASS_SALAD_RECIPE: i32 = 1455;
const JONASS_SAUCE_RECIPE: i32 = 1456;
const JONASS_STEAK_RECIPE: i32 = 1457;

// Monster
const HOBGOBLIN: i32 = 20147;
const MANDRAGORA_SPROUT1: i32 = 20154;
const MANDRAGORA_SAPLING: i32 = 20155;
const MANDRAGORA_BLOSSOM: i32 = 20156;
const BLOODY_BEE: i32 = 20204;
const MANDRAGORA_SPROUT2: i32 = 20223;
const GRAY_ANT: i32 = 20226;
const GIANT_CRIMSON_ANT: i32 = 20228;
const STINGER_WASP: i32 = 20229;
const MONSTER_EYE_SEARCHER: i32 = 20265;
const MONSTER_EYE_GAZER: i32 = 20266;

// Misc
const MIN_LEVEL: i32 = 24;

/// The eight ingredients Jonas counts (`getQuestItemsCount(RED_MANDRAGORA_SAP) + ...`).
const INGREDIENTS: [i32; 8] = [
    RED_MANDRAGORA_SAP,
    WHITE_MANDRAGORA_SAP,
    HONEY,
    GOLDEN_HONEY,
    DIONIAN_POTATO,
    GREEN_MOSS_BUNDLE,
    BROWN_MOSS_BUNDLE,
    MONSTER_EYE_MEAT,
];
const DISHES: [i32; 5] = [
    JONASS_1ST_STEAK_DISH,
    JONASS_2ND_STEAK_DISH,
    JONASS_3RD_STEAK_DISH,
    JONASS_4TH_STEAK_DISH,
    JONASS_5TH_STEAK_DISH,
];
const REVIEWS: [i32; 5] = [
    MIRIENS_REVIEW_1,
    MIRIENS_REVIEW_2,
    MIRIENS_REVIEW_3,
    MIRIENS_REVIEW_4,
    MIRIENS_REVIEW_5,
];

/// The sum of the counts of `items`.
fn sum(ctx: &ScriptCtx<'_>, player: ObjectId, items: &[i32]) -> i64 {
    items.iter().map(|&i| ctx.get_quest_items_count(player, i)).sum()
}

/// The ingredient sum is below five (every NPC and kill branch tests it).
fn below_five(ctx: &ScriptCtx<'_>, player: ObjectId) -> bool {
    sum(ctx, player, &INGREDIENTS) < 5
}

/// The "good, better, best" part of Jonas' pick: how many of the white sap, the golden honey and
/// the brown bundle the player brought.
fn rare_count(ctx: &ScriptCtx<'_>, player: ObjectId) -> i64 {
    sum(ctx, player, &[WHITE_MANDRAGORA_SAP, GOLDEN_HONEY, BROWN_MOSS_BUNDLE])
}

/// `takeItems(.., -1)` of the list and the ingredients, then the dish.
fn cook(ctx: &mut ScriptCtx<'_>, player: ObjectId, dish: i32) {
    ctx.take_items(player, INGREDIENT_LIST, -1);
    for ingredient in INGREDIENTS {
        // The monster eye meat is the last of Java's eight `takeItems`; the order is the list's.
        ctx.take_items(player, ingredient, -1);
    }
    ctx.give_items(player, dish, 1);
}

/// `giveItems(killer, item, 1)` and the sound: "middle" with the last one of `limit`.
fn give_one(ctx: &mut ScriptCtx<'_>, killer: ObjectId, item: i32, limit: i64) {
    ctx.give_items(killer, item, 1);
    if ctx.get_quest_items_count(killer, item) == limit {
        ctx.play_sound(killer, QuestSound::ItemsoundQuestMiddle);
    } else {
        ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
    }
}

/// `if (count(item) < limit) give_one(..)`.
fn give_capped(ctx: &mut ScriptCtx<'_>, killer: ObjectId, item: i32, limit: i64) {
    if ctx.get_quest_items_count(killer, item) < limit {
        give_one(ctx, killer, item, limit);
    }
}

/// Java `Q00330_AdeptOfTaste`.
pub struct Q00330AdeptOfTaste;

impl Script for Q00330AdeptOfTaste {
    fn name(&self) -> &'static str {
        "Q00330_AdeptOfTaste"
    }

    fn path(&self) -> &'static str {
        "quests/Q00330_AdeptOfTaste"
    }

    fn id(&self) -> i32 {
        330
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[JONAS]);
        s.add_talk_id(&[
            JONAS,
            ACCESSORY_MERCHANT_SONIA,
            PRIESTESS_GLYVKA,
            MAGISTER_ROLLANT,
            GUARD_JACOB,
            GROCER_PANO,
            MAGISTER_MIRIEN,
        ]);
        s.add_kill_id(&[
            HOBGOBLIN,
            MANDRAGORA_SPROUT1,
            MANDRAGORA_SAPLING,
            MANDRAGORA_BLOSSOM,
            BLOODY_BEE,
            MANDRAGORA_SPROUT2,
            GRAY_ANT,
            GIANT_CRIMSON_ANT,
            STINGER_WASP,
            MONSTER_EYE_SEARCHER,
            MONSTER_EYE_GAZER,
        ]);
        s.register_quest_items(&[
            INGREDIENT_LIST,
            SONIAS_BOTANY_BOOK,
            RED_MANDRAGORA_ROOT,
            WHITE_MANDRAGORA_ROOT,
            RED_MANDRAGORA_SAP,
            WHITE_MANDRAGORA_SAP,
            JACOBS_INSECT_BOOK,
            NECTAR,
            ROYAL_JELLY,
            HONEY,
            GOLDEN_HONEY,
            PANOS_CONTRACT,
            HOBGOBLIN_AMULET,
            DIONIAN_POTATO,
            GLYVKAS_BOTANY_BOOK,
            GREEN_MARSH_MOSS,
            BROWN_MARSH_MOSS,
            GREEN_MOSS_BUNDLE,
            BROWN_MOSS_BUNDLE,
            ROLLANTS_CREATURE_BOOK,
            BODY_OF_MONSTER_EYE,
            MONSTER_EYE_MEAT,
            JONASS_1ST_STEAK_DISH,
            JONASS_2ND_STEAK_DISH,
            JONASS_3RD_STEAK_DISH,
            JONASS_4TH_STEAK_DISH,
            JONASS_5TH_STEAK_DISH,
            MIRIENS_REVIEW_1,
            MIRIENS_REVIEW_2,
            MIRIENS_REVIEW_3,
            MIRIENS_REVIEW_4,
            MIRIENS_REVIEW_5,
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
            "30469-03.htm" => {
                if qs.is_created(ctx) {
                    qs.start_quest(ctx);
                    ctx.give_items(player, INGREDIENT_LIST, 1);
                    htmltext = Some(event.to_owned());
                }
            }
            "30062-05.html" => {
                if ctx.has_quest_items(player, SONIAS_BOTANY_BOOK)
                    && ctx.get_quest_items_count(player, RED_MANDRAGORA_ROOT)
                        + ctx.get_quest_items_count(player, WHITE_MANDRAGORA_ROOT)
                        >= 40
                    && ctx.get_quest_items_count(player, WHITE_MANDRAGORA_ROOT) < 40
                {
                    ctx.take_items(player, SONIAS_BOTANY_BOOK, 1);
                    ctx.take_items(player, RED_MANDRAGORA_ROOT, -1);
                    ctx.take_items(player, WHITE_MANDRAGORA_ROOT, -1);
                    ctx.give_items(player, RED_MANDRAGORA_SAP, 1);
                    htmltext = Some(event.to_owned());
                }
            }
            "30067-05.html" => {
                if ctx.has_quest_items(player, GLYVKAS_BOTANY_BOOK)
                    && ctx.get_quest_items_count(player, GREEN_MARSH_MOSS)
                        + ctx.get_quest_items_count(player, BROWN_MARSH_MOSS)
                        >= 20
                    && ctx.get_quest_items_count(player, BROWN_MARSH_MOSS) < 20
                {
                    ctx.take_items(player, GLYVKAS_BOTANY_BOOK, 1);
                    ctx.take_items(player, GREEN_MARSH_MOSS, -1);
                    ctx.take_items(player, BROWN_MARSH_MOSS, -1);
                    ctx.give_items(player, GREEN_MOSS_BUNDLE, 1);
                    htmltext = Some(event.to_owned());
                }
            }
            "30073-05.html" => {
                if ctx.has_quest_items(player, JACOBS_INSECT_BOOK)
                    && ctx.get_quest_items_count(player, NECTAR) >= 20
                    && ctx.get_quest_items_count(player, ROYAL_JELLY) < 10
                {
                    ctx.take_items(player, JACOBS_INSECT_BOOK, 1);
                    ctx.take_items(player, NECTAR, -1);
                    ctx.take_items(player, ROYAL_JELLY, -1);
                    ctx.give_items(player, HONEY, 1);
                    htmltext = Some(event.to_owned());
                }
            }
            "30062-04.html" | "30067-04.html" | "30073-04.html" | "30469-04.html" | "30469-04t1.html"
            | "30469-04t2.html" | "30469-04t3.html" | "30469-04t4.html" | "30469-04t5.html" => {
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
            HOBGOBLIN => {
                if below_five(ctx, killer)
                    && ctx.has_all_quest_items(killer, &[INGREDIENT_LIST, PANOS_CONTRACT])
                    && ctx.get_quest_items_count(killer, HOBGOBLIN_AMULET) < 30
                {
                    give_one(ctx, killer, HOBGOBLIN_AMULET, 30);
                }
            }
            MANDRAGORA_SPROUT1 | MANDRAGORA_SPROUT2 | MANDRAGORA_SAPLING | MANDRAGORA_BLOSSOM => {
                if below_five(ctx, killer)
                    && ctx.has_all_quest_items(killer, &[INGREDIENT_LIST, SONIAS_BOTANY_BOOK])
                    && !ctx.has_at_least_one_quest_item(killer, &[RED_MANDRAGORA_SAP, WHITE_MANDRAGORA_SAP])
                {
                    // (red limit, white limit) of the roll, by monster.
                    let (red, white) = match npc.id {
                        MANDRAGORA_SAPLING => (77, 85),
                        MANDRAGORA_BLOSSOM => (87, 96),
                        _ => (70, 77),
                    };
                    let i0 = ctx.get_random(100);
                    if i0 < red {
                        give_capped(ctx, killer, RED_MANDRAGORA_ROOT, 40);
                    } else if i0 < white {
                        give_capped(ctx, killer, WHITE_MANDRAGORA_ROOT, 40);
                    }
                }
            }
            BLOODY_BEE => {
                if below_five(ctx, killer) && ctx.has_all_quest_items(killer, &[INGREDIENT_LIST, JACOBS_INSECT_BOOK])
                {
                    let i0 = ctx.get_random(100);
                    if i0 < 80 {
                        give_capped(ctx, killer, NECTAR, 20);
                    } else if i0 < 95 {
                        give_capped(ctx, killer, ROYAL_JELLY, 10);
                    }
                }
            }
            GRAY_ANT => {
                if below_five(ctx, killer)
                    && ctx.has_all_quest_items(killer, &[INGREDIENT_LIST, GLYVKAS_BOTANY_BOOK])
                {
                    let i0 = ctx.get_random(100);
                    if i0 < 87 {
                        give_capped(ctx, killer, GREEN_MARSH_MOSS, 20);
                    } else if i0 < 96 {
                        give_capped(ctx, killer, BROWN_MARSH_MOSS, 20);
                    }
                }
            }
            GIANT_CRIMSON_ANT => {
                if below_five(ctx, killer)
                    && ctx.has_all_quest_items(killer, &[INGREDIENT_LIST, GLYVKAS_BOTANY_BOOK])
                {
                    let i0 = ctx.get_random(100);
                    if i0 < 90 {
                        give_capped(ctx, killer, GREEN_MARSH_MOSS, 20);
                    } else {
                        give_capped(ctx, killer, BROWN_MARSH_MOSS, 20);
                    }
                }
            }
            STINGER_WASP => {
                if below_five(ctx, killer) && ctx.has_all_quest_items(killer, &[INGREDIENT_LIST, JACOBS_INSECT_BOOK])
                {
                    let i0 = ctx.get_random(100);
                    if i0 < 92 {
                        give_capped(ctx, killer, NECTAR, 20);
                    } else {
                        give_capped(ctx, killer, ROYAL_JELLY, 10);
                    }
                }
            }
            MONSTER_EYE_SEARCHER => {
                if below_five(ctx, killer)
                    && ctx.has_all_quest_items(killer, &[INGREDIENT_LIST, ROLLANTS_CREATURE_BOOK])
                    && ctx.get_quest_items_count(killer, BODY_OF_MONSTER_EYE) < 30
                {
                    let i0 = ctx.get_random(100);
                    if i0 < 77 {
                        if ctx.get_quest_items_count(killer, BODY_OF_MONSTER_EYE) == 29 {
                            ctx.give_items(killer, BODY_OF_MONSTER_EYE, 1);
                            ctx.play_sound(killer, QuestSound::ItemsoundQuestMiddle);
                        } else {
                            ctx.give_items(killer, BODY_OF_MONSTER_EYE, 2);
                            ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
                        }
                    } else if i0 < 97 {
                        // NOTE(java-quirk): Java tests the Brown Marsh Moss count here, not the body count.
                        let brown = ctx.get_quest_items_count(killer, BROWN_MARSH_MOSS);
                        if brown == 28 {
                            ctx.give_items(killer, BODY_OF_MONSTER_EYE, 2);
                            ctx.play_sound(killer, QuestSound::ItemsoundQuestMiddle);
                        } else if brown == 29 {
                            ctx.give_items(killer, BODY_OF_MONSTER_EYE, 1);
                            ctx.play_sound(killer, QuestSound::ItemsoundQuestMiddle);
                        } else {
                            ctx.give_items(killer, BODY_OF_MONSTER_EYE, 3);
                            ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
                        }
                    }
                }
            }
            MONSTER_EYE_GAZER => {
                if below_five(ctx, killer)
                    && ctx.has_all_quest_items(killer, &[INGREDIENT_LIST, ROLLANTS_CREATURE_BOOK])
                    && ctx.get_quest_items_count(killer, BODY_OF_MONSTER_EYE) < 30
                {
                    let i0 = ctx.get_random(10);
                    if i0 < 7 {
                        give_one(ctx, killer, BODY_OF_MONSTER_EYE, 30);
                    } else if ctx.get_quest_items_count(killer, BROWN_MARSH_MOSS) == 29 {
                        // NOTE(java-quirk): the Brown Marsh Moss count again.
                        ctx.give_items(killer, BODY_OF_MONSTER_EYE, 1);
                        ctx.play_sound(killer, QuestSound::ItemsoundQuestMiddle);
                    } else {
                        ctx.give_items(killer, BODY_OF_MONSTER_EYE, 2);
                        ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
                    }
                }
            }
            _ => {}
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = Some(ctx.get_no_quest_msg(player));
        let set = |s: &str| Some(s.to_owned());
        if qs.is_created(ctx) {
            if npc.id == JONAS {
                htmltext = if ctx.player_level(player) < MIN_LEVEL {
                    set("30469-01.htm")
                } else {
                    set("30469-02.htm")
                };
            }
        } else if qs.state(ctx) == STARTED {
            match npc.id {
                JONAS => {
                    if ctx.has_quest_items(player, INGREDIENT_LIST) {
                        if below_five(ctx, player) {
                            htmltext = set("30469-04.html");
                        } else {
                            // Java runs four `if`s one after the other; the first one that hits
                            // takes the ingredients, so the counts of the later ones are 0.
                            if rare_count(ctx, player) == 0 {
                                if ctx.get_random(10) < 5 {
                                    cook(ctx, player, JONASS_2ND_STEAK_DISH);
                                    htmltext = set("30469-05t2.html");
                                } else {
                                    cook(ctx, player, JONASS_1ST_STEAK_DISH);
                                    htmltext = set("30469-05t1.html");
                                }
                            }
                            if rare_count(ctx, player) == 1 {
                                if ctx.get_random(10) < 5 {
                                    cook(ctx, player, JONASS_3RD_STEAK_DISH);
                                    htmltext = set("30469-05t3.html");
                                } else {
                                    cook(ctx, player, JONASS_2ND_STEAK_DISH);
                                    htmltext = set("30469-05t2.html");
                                }
                            }
                            if rare_count(ctx, player) == 2 {
                                if ctx.get_random(10) < 5 {
                                    cook(ctx, player, JONASS_4TH_STEAK_DISH);
                                    htmltext = set("30469-05t4.html");
                                } else {
                                    cook(ctx, player, JONASS_3RD_STEAK_DISH);
                                    htmltext = set("30469-05t3.html");
                                }
                            }
                            if rare_count(ctx, player) == 3 {
                                if ctx.get_random(10) < 5 {
                                    cook(ctx, player, JONASS_5TH_STEAK_DISH);
                                    ctx.play_sound(player, QuestSound::ItemsoundQuestJackpot);
                                    htmltext = set("30469-05t5.html");
                                } else {
                                    cook(ctx, player, JONASS_4TH_STEAK_DISH);
                                    htmltext = set("30469-05t4.html");
                                }
                            }
                        }
                    } else if sum(ctx, player, &INGREDIENTS) == 0 {
                        if sum(ctx, player, &DISHES) == 1 {
                            if sum(ctx, player, &REVIEWS) == 0 {
                                htmltext = set("30469-06.html");
                            }
                        } else if sum(ctx, player, &REVIEWS) == 1 {
                            if ctx.has_quest_items(player, MIRIENS_REVIEW_1) {
                                ctx.take_items(player, MIRIENS_REVIEW_1, 1);
                                ctx.give_adena(player, 10000, true);
                                htmltext = set("30469-06t1.html");
                            }
                            if ctx.has_quest_items(player, MIRIENS_REVIEW_2) {
                                ctx.take_items(player, MIRIENS_REVIEW_2, 1);
                                ctx.give_adena(player, 14870, true);
                                htmltext = set("30469-06t2.html");
                            }
                            if ctx.has_quest_items(player, MIRIENS_REVIEW_3) {
                                ctx.take_items(player, MIRIENS_REVIEW_3, 1);
                                ctx.give_adena(player, 6490, true);
                                ctx.give_items(player, JONASS_SALAD_RECIPE, 1);
                                htmltext = set("30469-06t3.html");
                            }
                            if ctx.has_quest_items(player, MIRIENS_REVIEW_4) {
                                ctx.take_items(player, MIRIENS_REVIEW_4, 1);
                                ctx.give_adena(player, 12220, true);
                                ctx.give_items(player, JONASS_SAUCE_RECIPE, 1);
                                htmltext = set("30469-06t4.html");
                            }
                            if ctx.has_quest_items(player, MIRIENS_REVIEW_5) {
                                ctx.take_items(player, MIRIENS_REVIEW_5, 1);
                                ctx.give_adena(player, 16540, true);
                                ctx.give_items(player, JONASS_STEAK_RECIPE, 1);
                                htmltext = set("30469-06t5.html");
                            }
                            qs.exit_quest(ctx, true, true);
                        }
                    }
                }
                ACCESSORY_MERCHANT_SONIA => {
                    let list = ctx.has_quest_items(player, INGREDIENT_LIST);
                    let book = ctx.has_quest_items(player, SONIAS_BOTANY_BOOK);
                    let saps = ctx.has_at_least_one_quest_item(player, &[RED_MANDRAGORA_SAP, WHITE_MANDRAGORA_SAP]);
                    if list && below_five(ctx, player) && !book && !saps {
                        ctx.give_items(player, SONIAS_BOTANY_BOOK, 1);
                        htmltext = set("30062-01.html");
                    } else if list && book && below_five(ctx, player) && !saps {
                        if ctx.get_quest_items_count(player, RED_MANDRAGORA_ROOT)
                            + ctx.get_quest_items_count(player, WHITE_MANDRAGORA_ROOT)
                            < 40
                        {
                            htmltext = set("30062-02.html");
                        } else if ctx.get_quest_items_count(player, WHITE_MANDRAGORA_ROOT) < 40 {
                            htmltext = set("30062-03.html");
                        } else {
                            ctx.take_items(player, SONIAS_BOTANY_BOOK, 1);
                            ctx.take_items(player, RED_MANDRAGORA_ROOT, -1);
                            ctx.take_items(player, WHITE_MANDRAGORA_ROOT, -1);
                            ctx.give_items(player, WHITE_MANDRAGORA_SAP, 1);
                            htmltext = set("30062-06.html");
                        }
                    } else if list && !book && below_five(ctx, player) && saps {
                        htmltext = set("30062-07.html");
                    }
                }
                PRIESTESS_GLYVKA => {
                    let list = ctx.has_quest_items(player, INGREDIENT_LIST);
                    let book = ctx.has_quest_items(player, GLYVKAS_BOTANY_BOOK);
                    let bundles = ctx.has_at_least_one_quest_item(player, &[GREEN_MOSS_BUNDLE, BROWN_MOSS_BUNDLE]);
                    if list && below_five(ctx, player) && !book && !bundles {
                        ctx.give_items(player, GLYVKAS_BOTANY_BOOK, 1);
                        htmltext = set("30067-01.html");
                    } else if list && book && below_five(ctx, player) {
                        if ctx.get_quest_items_count(player, GREEN_MARSH_MOSS)
                            + ctx.get_quest_items_count(player, BROWN_MARSH_MOSS)
                            < 20
                        {
                            htmltext = set("30067-02.html");
                        } else if ctx.get_quest_items_count(player, BROWN_MARSH_MOSS) < 20 {
                            htmltext = set("30067-03.html");
                        } else {
                            ctx.take_items(player, GLYVKAS_BOTANY_BOOK, 1);
                            ctx.take_items(player, GREEN_MARSH_MOSS, -1);
                            ctx.take_items(player, BROWN_MARSH_MOSS, -1);
                            ctx.give_items(player, BROWN_MOSS_BUNDLE, 1);
                            htmltext = set("30067-06.html");
                        }
                    } else if list && !book && below_five(ctx, player) && bundles {
                        htmltext = set("30067-07.html");
                    }
                }
                MAGISTER_ROLLANT => {
                    let list = ctx.has_quest_items(player, INGREDIENT_LIST);
                    let book = ctx.has_quest_items(player, ROLLANTS_CREATURE_BOOK);
                    let meat = ctx.has_quest_items(player, MONSTER_EYE_MEAT);
                    if list && below_five(ctx, player) && !book && !meat {
                        ctx.give_items(player, ROLLANTS_CREATURE_BOOK, 1);
                        htmltext = set("30069-01.html");
                    } else if list && book && below_five(ctx, player) {
                        if ctx.get_quest_items_count(player, BODY_OF_MONSTER_EYE) < 30 {
                            htmltext = set("30069-02.html");
                        } else {
                            ctx.take_items(player, ROLLANTS_CREATURE_BOOK, 1);
                            ctx.take_items(player, BODY_OF_MONSTER_EYE, -1);
                            ctx.give_items(player, MONSTER_EYE_MEAT, 1);
                            htmltext = set("30069-03.html");
                        }
                    } else if list && meat && !book && below_five(ctx, player) {
                        htmltext = set("30069-04.html");
                    }
                }
                GUARD_JACOB => {
                    let list = ctx.has_quest_items(player, INGREDIENT_LIST);
                    let book = ctx.has_quest_items(player, JACOBS_INSECT_BOOK);
                    let honeys = ctx.has_at_least_one_quest_item(player, &[HONEY, GOLDEN_HONEY]);
                    if list && below_five(ctx, player) && !book && !honeys {
                        ctx.give_items(player, JACOBS_INSECT_BOOK, 1);
                        htmltext = set("30073-01.html");
                    } else if list && book && below_five(ctx, player) {
                        if ctx.get_quest_items_count(player, NECTAR) < 20 {
                            htmltext = set("30073-02.html");
                        } else if ctx.get_quest_items_count(player, ROYAL_JELLY) < 10 {
                            htmltext = set("30073-03.html");
                        } else {
                            ctx.take_items(player, JACOBS_INSECT_BOOK, 1);
                            ctx.take_items(player, NECTAR, -1);
                            ctx.take_items(player, ROYAL_JELLY, -1);
                            ctx.give_items(player, GOLDEN_HONEY, 1);
                            htmltext = set("30073-06.html");
                        }
                    } else if list && !book && below_five(ctx, player) && honeys {
                        htmltext = set("30073-07.html");
                    }
                }
                GROCER_PANO => {
                    let list = ctx.has_quest_items(player, INGREDIENT_LIST);
                    let contract = ctx.has_quest_items(player, PANOS_CONTRACT);
                    let potato = ctx.has_quest_items(player, DIONIAN_POTATO);
                    if list && below_five(ctx, player) && !contract && !potato {
                        ctx.give_items(player, PANOS_CONTRACT, 1);
                        htmltext = set("30078-01.html");
                    } else if list && contract && below_five(ctx, player) {
                        if ctx.get_quest_items_count(player, HOBGOBLIN_AMULET) < 30 {
                            htmltext = set("30078-02.html");
                        } else {
                            ctx.take_items(player, PANOS_CONTRACT, 1);
                            ctx.take_items(player, HOBGOBLIN_AMULET, -1);
                            ctx.give_items(player, DIONIAN_POTATO, 1);
                            htmltext = set("30078-03.html");
                        }
                    } else if list && potato && !contract && below_five(ctx, player) {
                        htmltext = set("30078-04.html");
                    }
                }
                MAGISTER_MIRIEN => {
                    if ctx.has_quest_items(player, INGREDIENT_LIST) {
                        htmltext = set("30461-01.html");
                    } else if sum(ctx, player, &INGREDIENTS) == 0 {
                        if sum(ctx, player, &DISHES) == 1 {
                            if sum(ctx, player, &REVIEWS) == 0 {
                                for (n, (dish, review)) in DISHES.into_iter().zip(REVIEWS).enumerate() {
                                    if ctx.has_quest_items(player, dish) {
                                        ctx.take_items(player, dish, 1);
                                        ctx.give_items(player, review, 1);
                                        htmltext = Some(format!("30461-02t{}.html", n + 1));
                                    }
                                }
                            }
                        } else if sum(ctx, player, &REVIEWS) == 1 {
                            htmltext = set("30461-04.html");
                        }
                    }
                }
                _ => {}
            }
        }
        htmltext
    }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::{Pages, ScriptTest, item, rows};

    const NAME: &str = "Q00330_AdeptOfTaste";

    const NPCS: &str = r#"
        <npc id="30062" type="Folk" level="20" name="Sonia"/>
        <npc id="30461" type="Folk" level="20" name="Mirien"/>
        <npc id="30469" type="Folk" level="20" name="Jonas"/>
        <npc id="20154" type="Monster" level="10" name="Mandragora Sprout"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"/>
        <item id="1420" type="EtcItem" name="Ingredient List"><set name="is_questitem" val="true"/></item>
        <item id="1421" type="EtcItem" name="Sonia's Botany Book"><set name="is_questitem" val="true"/></item>
        <item id="1422" type="EtcItem" name="Red Mandragora Root"><set name="is_questitem" val="true"/></item>
        <item id="1423" type="EtcItem" name="White Mandragora Root"><set name="is_questitem" val="true"/></item>
        <item id="1425" type="EtcItem" name="White Mandragora Sap"><set name="is_questitem" val="true"/></item>
        <item id="1442" type="EtcItem" name="Jonas's 1st Steak Dish"><set name="is_questitem" val="true"/></item>
        <item id="1447" type="EtcItem" name="Mirien's Review 1"><set name="is_questitem" val="true"/></item>
        <item id="1448" type="EtcItem" name="Mirien's Review 2"><set name="is_questitem" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid, Oid, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00330AdeptOfTaste);
        let pages = Pages::of(&Q00330AdeptOfTaste)
            .plain(&["30469-01.htm", "30469-03.htm", "30062-01.html", "30062-06.html", "30461-02t1.html"])
            .page("30469-02.htm", &["30469-03.htm"])
            .page("30469-06.html", &[])
            .plain(&["30469-06t1.html"])
            .npcs(&[30062, 30461, 30469]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let sonia = t.spawn(30062, 20);
        let mirien = t.spawn(30461, 40);
        let jonas = t.spawn(30469, 60);
        (t, sonia, mirien, jonas)
    }

    #[test]
    fn a_low_level_character_is_refused() {
        let (mut t, _, _, jonas) = setup();
        t.enter(1, 23, vec![]);
        assert!(t.quest_page(1, jonas).contains("30469-01.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_quest_starts_with_the_ingredient_list_and_sonia_gives_her_book() {
        let (mut t, sonia, _, jonas) = setup();
        t.enter(1, 24, vec![]);
        assert!(t.quest_page(1, jonas).contains("30469-02.htm"));
        assert!(t.event(1, NAME, "30469-03.htm").contains("30469-03.htm"));
        assert_eq!(t.count(1, INGREDIENT_LIST), 1);
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        assert!(t.quest_page(1, sonia).contains("30062-01.html"));
        assert_eq!(t.count(1, SONIAS_BOTANY_BOOK), 1);
    }

    #[test]
    fn mandragoras_drop_roots_for_the_book() {
        let (mut t, _, _, jonas) = setup();
        t.enter(1, 24, vec![item(10, INGREDIENT_LIST, 1), item(11, SONIAS_BOTANY_BOOK, 1)]);
        t.quest_page(1, jonas);
        t.event(1, NAME, "30469-03.htm");
        for _ in 0..30 {
            let m = t.spawn(20154, 20);
            t.kill(1, m);
        }
        assert!(t.count(1, RED_MANDRAGORA_ROOT) > 0);
    }

    #[test]
    fn the_dish_goes_to_mirien_and_the_review_is_paid_by_jonas() {
        let (mut t, _, mirien, jonas) = setup();
        t.enter(1, 24, vec![]);
        t.quest_page(1, jonas);
        t.event(1, NAME, "30469-03.htm");
        t.take_writes();
        // The list is used up: Jonas cooked the first dish (all ingredients are there, none rare).
        t.world.run_as_script(NAME, |ctx| {
            ctx.take_items(Oid(1), INGREDIENT_LIST, -1);
            ctx.give_items(Oid(1), JONASS_1ST_STEAK_DISH, 1);
        });
        assert!(t.quest_page(1, mirien).contains("30461-02t1.html"));
        assert_eq!((t.count(1, JONASS_1ST_STEAK_DISH), t.count(1, MIRIENS_REVIEW_1)), (0, 1));

        assert!(t.quest_page(1, jonas).contains("30469-06t1.html"));
        assert_eq!(t.count(1, MIRIENS_REVIEW_1), 0);
        assert_eq!(t.count(1, 57), 10000);
        assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
    }
}
