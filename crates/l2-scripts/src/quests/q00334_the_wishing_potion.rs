//! Quest 334: The Wishing Potion (Matild has the player collect eight ingredients for a wish
//! potion and then make a wish; the wish spawns fairies, succubi, demon kings or a chest).
// Java: dist/game/data/scripts/quests/Q00334_TheWishingPotion/Q00334_TheWishingPotion.java
//
// Pattern: events that are numbers (the page links), NPC timers that speak and delete NPCs,
// spawns that talk (`on_spawn`), a kill that picks a random party member
// (`docs/design/scripts.md`, "Quest patterns": timer, busy NPC, NPC turns into a monster).

use l2_protocol_game::NpcStringId;
use l2_protocol_game::server::ChatType;
use l2_util::ObjectId;
use l2_world::script::QuestSound;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const TORAI: i32 = 30557;
const ALCHEMIST_MATILD: i32 = 30738;
const FAIRY_RUPINA: i32 = 30742;
const WISDOM_CHEST: i32 = 30743;

// Monsters
const WHISPERING_WIND: i32 = 20078;
const ANT_SOLDIER: i32 = 20087;
const ANT_WARRIOR_CAPTAIN: i32 = 20088;
const SILENOS: i32 = 20168;
const TYRANT: i32 = 20192;
const TYRANT_KINGPIN: i32 = 20193;
const AMBER_BASILISK: i32 = 20199;
const MIST_HORROR_RIPPER: i32 = 20227;
const TURAK_BUGBEAR: i32 = 20248;
const TURAK_BUGBEAR_WARRIOR: i32 = 20249;
const GRIMA: i32 = 27135;
const GLASS_JAGUAR: i32 = 20250;
const SUCCUBUS_OF_SEDUCTION: i32 = 27136;
const GREAT_DEMON_KING: i32 = 27138;
const SECRET_KEEPER_TREE: i32 = 27139;
const DLORD_ALEXANDROSANCHES: i32 = 27153;
const ABYSSKING_BONAPARTERIUS: i32 = 27154;
const EVILOVERLORD_RAMSEBALIUS: i32 = 27155;

// Items
const Q_WISH_POTION: i32 = 3467;
const Q_ANCIENT_CROWN: i32 = 3468;
const Q_CERTIFICATE_OF_ROYALTY: i32 = 3469;
const Q_ALCHEMY_TEXT: i32 = 3678;
const Q_SECRET_BOOK_OF_POTION: i32 = 3679;
const Q_POTION_RECIPE_1: i32 = 3680;
const Q_POTION_RECIPE_2: i32 = 3681;
const Q_MATILDS_ORB: i32 = 3682;
const Q_FOBBIDEN_LOVE_SCROLL: i32 = 3683;

// Items required for create wish potion
const Q_AMBER_SCALE: i32 = 3684;
const Q_WIND_SOULSTONE: i32 = 3685;
const Q_GLASS_EYE: i32 = 3686;
const Q_HORROR_ECTOPLASM: i32 = 3687;
const Q_SILENOS_HORN: i32 = 3688;
const Q_ANT_SOLDIER_APHID: i32 = 3689;
const Q_TYRANTS_CHITIN: i32 = 3690;
const Q_BUGBEAR_BLOOD: i32 = 3691;

// Rewards
const NECKLACE_OF_GRACE: i32 = 931;
const DEMONS_TUNIC_FABRIC: i32 = 1979;
const DEMONS_HOSE_PATTERN: i32 = 1980;
const DEMONS_BOOTS_FABRIC: i32 = 2952;
const DEMONS_GLOVES_FABRIC: i32 = 2953;
const Q_MUSICNOTE_LOVE: i32 = 4408;
const Q_MUSICNOTE_BATTLE: i32 = 4409;
const Q_GOLD_CIRCLET: i32 = 12766;
const Q_SILVER_CIRCLET: i32 = 12767;

const DEMONS_TUNIC: i32 = 441;
const DEMONS_HOSE: i32 = 472;
const DEMONS_BOOTS: i32 = 2435;
const DEMONS_GLOVES: i32 = 2459;

// Misc
const FLAG: &str = "flag";
const I_QUEST0: &str = "i_quest0";
const EXCHANGE: &str = "Exchange";

const NAME: &str = "Q00334_TheWishingPotion";

/// `giveItems` of one of the four fabrics/patterns (`switch (getRandom(4))`).
fn give_random_fabric(ctx: &mut ScriptCtx<'_>, player: ObjectId) {
    match ctx.get_random(4) {
        0 => ctx.give_items(player, DEMONS_TUNIC_FABRIC, 1),
        1 => ctx.give_items(player, DEMONS_HOSE_PATTERN, 1),
        2 => ctx.give_items(player, DEMONS_BOOTS_FABRIC, 1),
        3 => ctx.give_items(player, DEMONS_GLOVES_FABRIC, 1),
        _ => {}
    }
}

/// The shared tail of the three demon kings' death: they shout, then either the next one comes
/// or the killer gets a fabric.
fn demon_chain(
    ctx: &mut ScriptCtx<'_>,
    npc: ScriptNpc,
    killer: ObjectId,
    shout: NpcStringId,
    next: i32,
) {
    ctx.broadcast_npc_say(npc, ChatType::Shout, shout, &[]);
    if ctx.get_random(2) == 0 {
        ctx.add_spawn_at(next, npc.object_id, true, 0);
    } else {
        give_random_fabric(ctx, killer);
    }
}

/// A started player of this quest from the killer's party: the killer counts twice and every
/// member in range once (the killer again, when in a party).
fn get_random_player_from_party(
    ctx: &mut ScriptCtx<'_>,
    player: ObjectId,
    npc: ScriptNpc,
) -> Option<ObjectId> {
    let started = |ctx: &ScriptCtx<'_>, p: ObjectId| {
        ctx.player_quest_state(p, NAME).is_some_and(|q| q.is_started())
    };
    let mut candidates: Vec<ObjectId> = Vec::new();
    if started(ctx, player) {
        candidates.push(player);
        candidates.push(player);
    }
    let members = ctx
        .world()
        .party_of(player)
        .map(|party| ctx.world().party_members(party));
    if let Some(members) = members {
        for pm in members {
            if started(ctx, pm) && ctx.check_if_in_range(ctx.alt_party_range(), npc.object_id, pm, true) {
                candidates.push(pm);
            }
        }
    }
    if candidates.is_empty() {
        return None;
    }
    let index = usize::try_from(ctx.get_random(i32::try_from(candidates.len()).unwrap_or(1))).unwrap_or(0);
    candidates.get(index).copied()
}

/// One ingredient kill: give `item` with 1 in 10 when the player has both recipes and not the
/// item, and finish with cond 4 when `complete` (the other seven) are in the bag.
fn ingredient(ctx: &mut ScriptCtx<'_>, qs: l2_world::script::Qs, killer: ObjectId, item: i32, complete: &[i32]) {
    if ctx.has_all_quest_items(killer, &[Q_POTION_RECIPE_1, Q_POTION_RECIPE_2])
        && !ctx.has_quest_items(killer, item)
        && ctx.get_random(10) == 0
    {
        ctx.give_items(killer, item, 1);
        ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
        if ctx.has_all_quest_items(killer, complete) {
            qs.set_cond(ctx, 4, true);
            qs.show_question_mark(ctx, 334);
        } else {
            ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
        }
    }
}

/// Java `Q00334_TheWishingPotion`.
pub struct Q00334TheWishingPotion;

impl Script for Q00334TheWishingPotion {
    fn name(&self) -> &'static str {
        NAME
    }

    fn path(&self) -> &'static str {
        "quests/Q00334_TheWishingPotion"
    }

    fn id(&self) -> i32 {
        334
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[ALCHEMIST_MATILD]);
        s.add_talk_id(&[ALCHEMIST_MATILD, TORAI, FAIRY_RUPINA, WISDOM_CHEST]);
        s.add_kill_id(&[
            WHISPERING_WIND,
            ANT_SOLDIER,
            ANT_WARRIOR_CAPTAIN,
            SILENOS,
            TYRANT,
            TYRANT_KINGPIN,
            AMBER_BASILISK,
            MIST_HORROR_RIPPER,
        ]);
        s.add_kill_id(&[
            TURAK_BUGBEAR,
            TURAK_BUGBEAR_WARRIOR,
            GRIMA,
            GLASS_JAGUAR,
            SUCCUBUS_OF_SEDUCTION,
            GREAT_DEMON_KING,
            SECRET_KEEPER_TREE,
        ]);
        s.add_kill_id(&[DLORD_ALEXANDROSANCHES, ABYSSKING_BONAPARTERIUS, EVILOVERLORD_RAMSEBALIUS]);
        s.add_spawn_id(&[
            GRIMA,
            SUCCUBUS_OF_SEDUCTION,
            GREAT_DEMON_KING,
            DLORD_ALEXANDROSANCHES,
            ABYSSKING_BONAPARTERIUS,
            EVILOVERLORD_RAMSEBALIUS,
            FAIRY_RUPINA,
        ]);
        s.register_quest_items(&[
            Q_SECRET_BOOK_OF_POTION,
            Q_AMBER_SCALE,
            Q_GLASS_EYE,
            Q_HORROR_ECTOPLASM,
            Q_SILENOS_HORN,
            Q_ANT_SOLDIER_APHID,
            Q_TYRANTS_CHITIN,
            Q_BUGBEAR_BLOOD,
        ]);
        s.register_quest_items(&[
            Q_WISH_POTION,
            Q_POTION_RECIPE_1,
            Q_POTION_RECIPE_2,
            Q_MATILDS_ORB,
            Q_ALCHEMY_TEXT,
        ]);
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let htmltext = ctx.get_no_quest_msg(player);
        match npc.id {
            TORAI => {
                if ctx.has_quest_items(player, Q_FOBBIDEN_LOVE_SCROLL) {
                    ctx.give_adena(player, 500000, true);
                    ctx.take_items(player, Q_FOBBIDEN_LOVE_SCROLL, 1);
                    ctx.play_sound(player, QuestSound::ItemsoundQuestMiddle);
                    return Some("30557-01.html".to_owned());
                }
            }
            ALCHEMIST_MATILD => {
                if qs.is_created(ctx) {
                    if ctx.player_level(player) < 30 {
                        return Some("30738-01.htm".to_owned());
                    }
                    return Some("30738-02.html".to_owned());
                }

                let has = |ctx: &ScriptCtx<'_>, item: i32| ctx.has_quest_items(player, item);
                if !has(ctx, Q_SECRET_BOOK_OF_POTION) && has(ctx, Q_ALCHEMY_TEXT) {
                    return Some("30738-05.html".to_owned());
                }

                if has(ctx, Q_SECRET_BOOK_OF_POTION) && has(ctx, Q_ALCHEMY_TEXT) {
                    return Some("30738-06.html".to_owned());
                }

                if ctx.has_all_quest_items(player, &[Q_POTION_RECIPE_1, Q_POTION_RECIPE_2])
                    && (!has(ctx, Q_AMBER_SCALE)
                        || (has(ctx, Q_WIND_SOULSTONE) && !has(ctx, Q_GLASS_EYE))
                        || (!has(ctx, Q_HORROR_ECTOPLASM)
                            || !has(ctx, Q_SILENOS_HORN)
                            || !has(ctx, Q_ANT_SOLDIER_APHID)
                            || !has(ctx, Q_TYRANTS_CHITIN)
                            || !has(ctx, Q_BUGBEAR_BLOOD)))
                {
                    return Some("30738-08.html".to_owned());
                }

                // NOTE(java-quirk): Java lists Q_WIND_SOULSTONE and Q_GLASS_EYE twice; `hasQuestItems`
                // does not count, so the repeats change nothing.
                if ctx.has_all_quest_items(
                    player,
                    &[
                        Q_POTION_RECIPE_1,
                        Q_POTION_RECIPE_2,
                        Q_AMBER_SCALE,
                        Q_WIND_SOULSTONE,
                        Q_GLASS_EYE,
                        Q_HORROR_ECTOPLASM,
                        Q_SILENOS_HORN,
                        Q_ANT_SOLDIER_APHID,
                        Q_TYRANTS_CHITIN,
                        Q_BUGBEAR_BLOOD,
                    ],
                ) {
                    return Some("30738-09.html".to_owned());
                }

                if has(ctx, Q_MATILDS_ORB)
                    && !has(ctx, Q_POTION_RECIPE_1)
                    && !has(ctx, Q_POTION_RECIPE_2)
                    && (!has(ctx, Q_AMBER_SCALE)
                        || (has(ctx, Q_WIND_SOULSTONE) && !has(ctx, Q_GLASS_EYE))
                        || !has(ctx, Q_HORROR_ECTOPLASM)
                        || !has(ctx, Q_SILENOS_HORN)
                        || !has(ctx, Q_ANT_SOLDIER_APHID)
                        || !has(ctx, Q_TYRANTS_CHITIN)
                        || !has(ctx, Q_BUGBEAR_BLOOD))
                {
                    return Some("30738-12.html".to_owned());
                }
            }
            FAIRY_RUPINA => {
                if qs.get_int(ctx, FLAG) == 1 {
                    let html;
                    // NOTE(java-quirk): `getRandom(4) < 4` is always true, the second branch never runs.
                    if ctx.get_random(4) < 4 {
                        ctx.give_items(player, NECKLACE_OF_GRACE, 1);
                        qs.set(ctx, FLAG, "0");
                        html = "30742-01.html";
                    } else {
                        give_random_fabric(ctx, player);
                        html = "30742-02.html";
                    }

                    qs.set(ctx, FLAG, "0");
                    ctx.delete_npc(npc);
                    return Some(html.to_owned());
                }
            }
            WISDOM_CHEST => {
                if qs.get_int(ctx, FLAG) == 4 {
                    let random = ctx.get_random(100);
                    let mut html = None;
                    if random < 10 {
                        ctx.give_items(player, Q_FOBBIDEN_LOVE_SCROLL, 1);
                        html = Some("30743-02.html");
                    } else if (10..50).contains(&random) {
                        give_random_fabric(ctx, player);
                        html = Some("30743-03.html");
                    } else if (50..100).contains(&random) {
                        // NOTE(java-quirk): this branch takes every roll from 50 to 99, so the
                        // demon's armour (85..95) and the circlets (95..) below are never reached.
                        match ctx.get_random(2) {
                            0 => ctx.give_items(player, Q_MUSICNOTE_LOVE, 1),
                            1 => ctx.give_items(player, Q_MUSICNOTE_BATTLE, 1),
                            _ => {}
                        }
                        html = Some("30743-04.html");
                    } else if (85..95).contains(&random) {
                        match ctx.get_random(4) {
                            0 => ctx.give_items(player, DEMONS_TUNIC, 1),
                            1 => ctx.give_items(player, DEMONS_HOSE, 1),
                            2 => ctx.give_items(player, DEMONS_BOOTS, 1),
                            3 => ctx.give_items(player, DEMONS_GLOVES, 1),
                            _ => {}
                        }
                        html = Some("30743-05.html");
                    } else if random >= 95 {
                        match ctx.get_random(2) {
                            0 => ctx.give_items(player, Q_GOLD_CIRCLET, 1),
                            _ => ctx.give_items(player, Q_SILVER_CIRCLET, 1),
                        }
                        html = Some("30743-06.htm");
                    }

                    qs.set(ctx, FLAG, "0");
                    ctx.delete_npc(npc);
                    return html.map(str::to_owned);
                }
            }
            _ => {}
        }
        Some(htmltext)
    }

    fn on_spawn(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc) {
        let (timer, delay, shout) = match npc.id {
            GRIMA => ("2336002", 1000 * 200, NpcStringId::OhOhOh),
            SUCCUBUS_OF_SEDUCTION => ("2336003", 1000 * 200, NpcStringId::DoYouWantUsToLoveYouOh),
            GREAT_DEMON_KING => ("2336007", 1000 * 600, NpcStringId::WhoKilledMyUnderlingDevil),
            DLORD_ALEXANDROSANCHES => ("2336004", 1000 * 200, NpcStringId::WhoIsCallingTheLordOfDarkness),
            ABYSSKING_BONAPARTERIUS => ("2336005", 1000 * 200, NpcStringId::IAmAGreatEmpireBonaparterius),
            EVILOVERLORD_RAMSEBALIUS => ("2336006", 1000 * 200, NpcStringId::LetYourHeadDownBeforeTheLord),
            FAIRY_RUPINA => (
                "2336001",
                120 * 1000,
                NpcStringId::IWillMakeYourLoveComeTrueLoveLoveLove,
            ),
            WISDOM_CHEST => (
                "2336007",
                120 * 1000,
                NpcStringId::IHaveWisdomInMeIAmTheBoxOfWisdom,
            ),
            _ => return,
        };
        ctx.start_quest_timer(timer, delay, Some(npc), None, false);
        ctx.broadcast_npc_say(npc, ChatType::Shout, shout, &[]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        // Java dereferences `npc` at once; an event without one would throw.
        let npc = npc?;
        match npc.id {
            FAIRY_RUPINA | GRIMA | SUCCUBUS_OF_SEDUCTION | GREAT_DEMON_KING => {
                ctx.delete_npc(npc);
            }
            DLORD_ALEXANDROSANCHES | ABYSSKING_BONAPARTERIUS | EVILOVERLORD_RAMSEBALIUS => {
                ctx.broadcast_npc_say(
                    npc,
                    ChatType::Shout,
                    NpcStringId::OhItSNotAnOpponentOfMineHaHaHa,
                    &[],
                );
                ctx.delete_npc(npc);
            }
            WISDOM_CHEST => {
                ctx.broadcast_npc_say(npc, ChatType::Shout, NpcStringId::DonTInterruptMyRestAgain, &[]);
                ctx.broadcast_npc_say(npc, ChatType::Shout, NpcStringId::YouReAGreatDevilNow, &[]);
                ctx.delete_npc(npc);
            }
            ALCHEMIST_MATILD => {
                let player = player?;
                let qs = ctx.get_quest_state(player, false)?;
                if event == "QUEST_ACCEPTED" {
                    ctx.play_sound(player, QuestSound::ItemsoundQuestAccept);
                    qs.start_quest(ctx);
                    qs.set_memo_state(ctx, 1);
                    qs.set_cond(ctx, 1, false);
                    qs.show_question_mark(ctx, 334);
                    if !ctx.has_quest_items(player, Q_ALCHEMY_TEXT) {
                        ctx.give_items(player, Q_ALCHEMY_TEXT, 1);
                    }

                    ctx.play_sound(player, QuestSound::ItemsoundQuestMiddle);
                    return Some("30738-04.htm".to_owned());
                }

                // NOTE(java-quirk): `Integer.parseInt(event)` throws on any other text.
                let number: i32 = event.parse().ok()?;
                match number {
                    1 => return Some("30738-03.htm".to_owned()),
                    2 => {
                        ctx.take_items(player, Q_SECRET_BOOK_OF_POTION, -1);
                        ctx.take_items(player, Q_ALCHEMY_TEXT, -1);
                        ctx.give_items(player, Q_POTION_RECIPE_1, 1);
                        ctx.give_items(player, Q_POTION_RECIPE_2, 1);
                        qs.set_memo_state(ctx, 2);
                        qs.set_cond(ctx, 3, true);
                        qs.show_question_mark(ctx, 334);
                        return Some("30738-07.html".to_owned());
                    }
                    3 => return Some("30738-10.html".to_owned()),
                    4 => {
                        if ctx.has_all_quest_items(
                            player,
                            &[
                                Q_AMBER_SCALE,
                                Q_GLASS_EYE,
                                Q_HORROR_ECTOPLASM,
                                Q_SILENOS_HORN,
                                Q_ANT_SOLDIER_APHID,
                                Q_TYRANTS_CHITIN,
                                Q_BUGBEAR_BLOOD,
                                Q_WIND_SOULSTONE,
                                Q_POTION_RECIPE_1,
                                Q_POTION_RECIPE_2,
                            ],
                        ) {
                            ctx.give_items(player, Q_WISH_POTION, 1);
                            if !ctx.has_quest_items(player, Q_MATILDS_ORB) {
                                ctx.give_items(player, Q_MATILDS_ORB, 1);
                            }

                            ctx.take_items(player, Q_AMBER_SCALE, 1);
                            ctx.take_items(player, Q_GLASS_EYE, 1);
                            ctx.take_items(player, Q_HORROR_ECTOPLASM, 1);
                            ctx.take_items(player, Q_SILENOS_HORN, 1);
                            ctx.take_items(player, Q_ANT_SOLDIER_APHID, 1);
                            ctx.take_items(player, Q_TYRANTS_CHITIN, 1);
                            ctx.take_items(player, Q_BUGBEAR_BLOOD, 1);
                            ctx.take_items(player, Q_WIND_SOULSTONE, 1);
                            ctx.take_items(player, Q_POTION_RECIPE_1, -1);
                            ctx.take_items(player, Q_POTION_RECIPE_2, -1);
                            qs.set_memo_state(ctx, 2);
                            ctx.play_sound(player, QuestSound::ItemsoundQuestMiddle);
                            qs.set_cond(ctx, 5, false);
                            qs.show_question_mark(ctx, 334);
                            return Some("30738-11.html".to_owned());
                        }
                    }
                    5 => {
                        if ctx.has_quest_items(player, Q_WISH_POTION) {
                            if qs.get_int(ctx, I_QUEST0) != 1 {
                                qs.set(ctx, I_QUEST0, "0");
                            }

                            return Some("30738-13.html".to_owned());
                        }

                        return Some("30738-14.html".to_owned());
                    }
                    6 => {
                        if ctx.has_quest_items(player, Q_WISH_POTION) {
                            return Some("30738-15a.html".to_owned());
                        }

                        ctx.give_items(player, Q_POTION_RECIPE_1, 1);
                        ctx.give_items(player, Q_POTION_RECIPE_2, 1);
                        return Some("30738-15.html".to_owned());
                    }
                    7..=10 => {
                        // (I_QUEST0 and FLAG value, the page) of the four wishes.
                        let (wish, page) = match number {
                            7 => (1, "30738-16.html"),
                            8 => (2, "30738-17.html"),
                            9 => (3, "30738-18.html"),
                            _ => (4, "30738-19.html"),
                        };
                        if ctx.has_quest_items(player, Q_WISH_POTION) {
                            if qs.get_int(ctx, EXCHANGE) == 0 {
                                ctx.take_items(player, Q_WISH_POTION, 1);
                                qs.set(ctx, I_QUEST0, &wish.to_string());
                                qs.set(ctx, FLAG, &wish.to_string());
                                ctx.start_quest_timer("2336008", 3 * 1000, Some(npc), Some(player), false);
                                return Some(page.to_owned());
                            }

                            return Some("30738-20.html".to_owned());
                        }

                        return Some("30738-14.html".to_owned());
                    }
                    2336008 => {
                        ctx.broadcast_npc_say(npc, ChatType::Shout, NpcStringId::OkEverybodyPrayFervently, &[]);
                        ctx.start_quest_timer("2336009", 4 * 1000, Some(npc), Some(player), false);
                    }
                    2336009 => {
                        ctx.broadcast_npc_say(
                            npc,
                            ChatType::Shout,
                            NpcStringId::BothHandsToHeavenEverybodyYellTogether,
                            &[],
                        );
                        ctx.start_quest_timer("2336010", 4 * 1000, Some(npc), Some(player), false);
                    }
                    2336010 => {
                        ctx.broadcast_npc_say(npc, ChatType::Shout, NpcStringId::OneTwoMayYourDreamsComeTrue, &[]);
                        let wish = qs.get_int(ctx, I_QUEST0);
                        let i0 = match wish {
                            1 => ctx.get_random(2),
                            2..=4 => ctx.get_random(3),
                            _ => 0,
                        };

                        match i0 {
                            0 => match wish {
                                1 => {
                                    ctx.add_spawn_at(FAIRY_RUPINA, npc.object_id, true, 0);
                                    qs.set(ctx, EXCHANGE, "0");
                                }
                                2 => {
                                    for _ in 0..3 {
                                        ctx.add_spawn_at(GRIMA, npc.object_id, true, 0);
                                    }
                                    qs.set(ctx, EXCHANGE, "0");
                                }
                                3 => {
                                    ctx.give_items(player, Q_CERTIFICATE_OF_ROYALTY, 1);
                                    qs.set(ctx, EXCHANGE, "0");
                                }
                                4 => {
                                    ctx.add_spawn_at(WISDOM_CHEST, npc.object_id, true, 0);
                                    qs.set(ctx, EXCHANGE, "0");
                                }
                                _ => {}
                            },
                            1 => match wish {
                                1 => {
                                    for _ in 0..4 {
                                        ctx.add_spawn_at(SUCCUBUS_OF_SEDUCTION, npc.object_id, true, 0);
                                    }
                                    qs.set(ctx, EXCHANGE, "0");
                                }
                                2 => {
                                    ctx.give_adena(player, 10000, true);
                                    qs.set(ctx, EXCHANGE, "0");
                                }
                                3 => {
                                    ctx.add_spawn_at(DLORD_ALEXANDROSANCHES, npc.object_id, true, 0);
                                    qs.set(ctx, EXCHANGE, "0");
                                }
                                4 => {
                                    ctx.add_spawn_at(WISDOM_CHEST, npc.object_id, true, 0);
                                    qs.set(ctx, EXCHANGE, "0");
                                }
                                _ => {}
                            },
                            2 => match wish {
                                2 => {
                                    ctx.give_adena(player, 10000, true);
                                    qs.set(ctx, EXCHANGE, "0");
                                }
                                3 => {
                                    ctx.give_items(player, Q_ANCIENT_CROWN, 1);
                                    qs.set(ctx, EXCHANGE, "0");
                                }
                                4 => {
                                    ctx.add_spawn_at(WISDOM_CHEST, npc.object_id, true, 0);
                                    qs.set(ctx, EXCHANGE, "0");
                                }
                                _ => {}
                            },
                            _ => {}
                        }
                    }
                    _ => {}
                }
            }
            _ => {}
        }

        // `super.onEvent(..)` is `null`.
        None
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        let Some(member) = get_random_player_from_party(ctx, killer, npc) else {
            return;
        };
        let Some(qs) = ctx.get_quest_state(member, false) else {
            return;
        };
        // NOTE(java-quirk): the drops go to `killer`, the member only picks the quest state whose
        // memo state and flag are tested.
        match npc.id {
            WHISPERING_WIND => ingredient(
                ctx,
                qs,
                killer,
                Q_WIND_SOULSTONE,
                &[
                    Q_AMBER_SCALE,
                    Q_GLASS_EYE,
                    Q_HORROR_ECTOPLASM,
                    Q_SILENOS_HORN,
                    Q_ANT_SOLDIER_APHID,
                    Q_TYRANTS_CHITIN,
                    Q_BUGBEAR_BLOOD,
                ],
            ),
            ANT_SOLDIER | ANT_WARRIOR_CAPTAIN => ingredient(
                ctx,
                qs,
                killer,
                Q_ANT_SOLDIER_APHID,
                &[
                    Q_AMBER_SCALE,
                    Q_WIND_SOULSTONE,
                    Q_GLASS_EYE,
                    Q_HORROR_ECTOPLASM,
                    Q_SILENOS_HORN,
                    Q_ANT_SOLDIER_APHID,
                    Q_TYRANTS_CHITIN,
                    Q_BUGBEAR_BLOOD,
                ],
            ),
            SILENOS => ingredient(
                ctx,
                qs,
                killer,
                Q_SILENOS_HORN,
                // NOTE(java-quirk): the Silenos Horn is not in this list, only the other seven.
                &[
                    Q_AMBER_SCALE,
                    Q_WIND_SOULSTONE,
                    Q_GLASS_EYE,
                    Q_HORROR_ECTOPLASM,
                    Q_ANT_SOLDIER_APHID,
                    Q_TYRANTS_CHITIN,
                    Q_BUGBEAR_BLOOD,
                ],
            ),
            TYRANT | TYRANT_KINGPIN => ingredient(
                ctx,
                qs,
                killer,
                Q_TYRANTS_CHITIN,
                &[
                    Q_AMBER_SCALE,
                    Q_WIND_SOULSTONE,
                    Q_GLASS_EYE,
                    Q_HORROR_ECTOPLASM,
                    Q_SILENOS_HORN,
                    Q_ANT_SOLDIER_APHID,
                    Q_TYRANTS_CHITIN,
                    Q_BUGBEAR_BLOOD,
                ],
            ),
            AMBER_BASILISK => ingredient(
                ctx,
                qs,
                killer,
                Q_AMBER_SCALE,
                &[
                    Q_WIND_SOULSTONE,
                    Q_GLASS_EYE,
                    Q_HORROR_ECTOPLASM,
                    Q_SILENOS_HORN,
                    Q_ANT_SOLDIER_APHID,
                    Q_TYRANTS_CHITIN,
                    Q_BUGBEAR_BLOOD,
                ],
            ),
            MIST_HORROR_RIPPER => ingredient(
                ctx,
                qs,
                killer,
                Q_HORROR_ECTOPLASM,
                &[
                    Q_AMBER_SCALE,
                    Q_WIND_SOULSTONE,
                    Q_GLASS_EYE,
                    Q_HORROR_ECTOPLASM,
                    Q_SILENOS_HORN,
                    Q_ANT_SOLDIER_APHID,
                    Q_TYRANTS_CHITIN,
                    Q_BUGBEAR_BLOOD,
                ],
            ),
            TURAK_BUGBEAR | TURAK_BUGBEAR_WARRIOR => ingredient(
                ctx,
                qs,
                killer,
                Q_BUGBEAR_BLOOD,
                &[
                    Q_AMBER_SCALE,
                    Q_WIND_SOULSTONE,
                    Q_GLASS_EYE,
                    Q_HORROR_ECTOPLASM,
                    Q_SILENOS_HORN,
                    Q_ANT_SOLDIER_APHID,
                    Q_TYRANTS_CHITIN,
                    Q_BUGBEAR_BLOOD,
                ],
            ),
            GLASS_JAGUAR => ingredient(
                ctx,
                qs,
                killer,
                Q_GLASS_EYE,
                &[
                    Q_AMBER_SCALE,
                    Q_WIND_SOULSTONE,
                    Q_GLASS_EYE,
                    Q_HORROR_ECTOPLASM,
                    Q_SILENOS_HORN,
                    Q_ANT_SOLDIER_APHID,
                    Q_TYRANTS_CHITIN,
                    Q_BUGBEAR_BLOOD,
                ],
            ),
            GRIMA => {
                if qs.is_memo_state(ctx, 2) && qs.get_int(ctx, FLAG) == 2 && ctx.get_random(1000) < 33 {
                    if ctx.get_random(1000) == 0 {
                        ctx.give_adena(killer, 100_000_000, true);
                    } else {
                        ctx.give_adena(killer, 900_000, true);
                    }

                    ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
                    qs.set(ctx, FLAG, "0");
                }
            }
            SUCCUBUS_OF_SEDUCTION => {
                if qs.is_memo_state(ctx, 2)
                    && !ctx.has_quest_items(killer, Q_FOBBIDEN_LOVE_SCROLL)
                    && qs.get_int(ctx, FLAG) == 1
                    && ctx.get_random(1000) < 28
                {
                    ctx.give_items(killer, Q_FOBBIDEN_LOVE_SCROLL, 1);
                    ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
                    qs.set(ctx, FLAG, "0");
                }
            }
            GREAT_DEMON_KING => {
                if qs.is_memo_state(ctx, 2) && qs.get_int(ctx, FLAG) == 3 {
                    ctx.give_adena(killer, 1_406_956, true);
                    ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
                    qs.set(ctx, FLAG, "0");
                }
            }
            SECRET_KEEPER_TREE => {
                if qs.is_memo_state(ctx, 1) && !ctx.has_quest_items(killer, Q_SECRET_BOOK_OF_POTION) {
                    ctx.give_items(killer, Q_SECRET_BOOK_OF_POTION, 1);
                    qs.set_cond(ctx, 2, true);
                    qs.show_question_mark(ctx, 334);
                    ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
                }
            }
            DLORD_ALEXANDROSANCHES => {
                if qs.is_memo_state(ctx, 2) && qs.get_int(ctx, FLAG) == 3 {
                    demon_chain(
                        ctx,
                        npc,
                        killer,
                        NpcStringId::BonaparteriusAbyssKingWillPunishYou,
                        ABYSSKING_BONAPARTERIUS,
                    );
                }
            }
            ABYSSKING_BONAPARTERIUS => {
                if qs.is_memo_state(ctx, 2) && qs.get_int(ctx, FLAG) == 3 {
                    demon_chain(
                        ctx,
                        npc,
                        killer,
                        NpcStringId::RevengeIsOverlordRamsebaliusOfTheEvilWorld,
                        EVILOVERLORD_RAMSEBALIUS,
                    );
                }
            }
            EVILOVERLORD_RAMSEBALIUS => {
                if qs.is_memo_state(ctx, 2) && qs.get_int(ctx, FLAG) == 3 {
                    demon_chain(ctx, npc, killer, NpcStringId::OhGreatDemonKing, GREAT_DEMON_KING);
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::{Pages, ScriptTest, item, rows};

    const NPCS: &str = r#"
        <npc id="30738" type="Folk" level="20" name="Matild"/>
        <npc id="30557" type="Folk" level="20" name="Torai"/>
        <npc id="27139" type="Monster" level="10" name="Secret Keeper Tree"/>
        <npc id="20250" type="Monster" level="10" name="Glass Jaguar"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="3467" type="EtcItem" name="Wish Potion"><set name="is_questitem" val="true"/></item>
        <item id="3678" type="EtcItem" name="Alchemy Text"><set name="is_questitem" val="true"/></item>
        <item id="3679" type="EtcItem" name="Secret Book of Potion"><set name="is_questitem" val="true"/></item>
        <item id="3680" type="EtcItem" name="Potion Recipe 1"><set name="is_questitem" val="true"/></item>
        <item id="3681" type="EtcItem" name="Potion Recipe 2"><set name="is_questitem" val="true"/></item>
        <item id="3683" type="EtcItem" name="Forbidden Love Scroll"/>
        <item id="3686" type="EtcItem" name="Glass Eye"><set name="is_questitem" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00334TheWishingPotion);
        let pages = Pages::of(&Q00334TheWishingPotion)
            .plain(&["30738-01.htm", "30738-04.htm", "30738-07.html", "30738-16.html", "30557-01.html"])
            .page("30738-02.html", &["1"])
            .page("30738-03.htm", &["QUEST_ACCEPTED"])
            .page("30738-06.html", &["2"])
            .page("30738-13.html", &["7"])
            .npcs(&[30738, 30557]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let matild = t.spawn(30738, 20);
        let torai = t.spawn(30557, 40);
        (t, matild, torai)
    }

    #[test]
    fn a_low_level_character_is_refused() {
        let (mut t, matild, _) = setup();
        t.enter(1, 29, vec![]);
        assert!(t.quest_page(1, matild).contains("30738-01.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn matild_starts_the_quest_with_the_alchemy_text() {
        let (mut t, matild, _) = setup();
        t.enter(1, 30, vec![]);
        assert!(t.quest_page(1, matild).contains("30738-02.html"));
        assert!(t.event(1, NAME, "1").contains("30738-03.htm"));
        assert!(t.take_writes().is_empty());
        assert!(t.event(1, NAME, "QUEST_ACCEPTED").contains("30738-04.htm"));
        assert_eq!(t.count(1, Q_ALCHEMY_TEXT), 1);
        assert_eq!(
            t.take_writes()[..2],
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
    }

    #[test]
    fn the_tree_gives_the_book_and_the_book_the_recipes() {
        let (mut t, matild, _) = setup();
        t.enter(1, 30, vec![]);
        t.quest_page(1, matild);
        t.event(1, NAME, "1");
        t.event(1, NAME, "QUEST_ACCEPTED");
        t.take_writes();
        let tree = t.spawn(27139, 20);
        t.kill(1, tree);
        assert_eq!(t.count(1, Q_SECRET_BOOK_OF_POTION), 1);
        assert!(t.quest_page(1, matild).contains("30738-06.html"));
        assert!(t.event(1, NAME, "2").contains("30738-07.html"));
        assert_eq!(
            (t.count(1, Q_POTION_RECIPE_1), t.count(1, Q_POTION_RECIPE_2)),
            (1, 1)
        );
        assert_eq!(t.count(1, Q_SECRET_BOOK_OF_POTION), 0);
    }

    #[test]
    fn torai_pays_for_the_love_scroll() {
        let (mut t, matild, torai) = setup();
        t.enter(1, 30, vec![item(10, Q_FOBBIDEN_LOVE_SCROLL, 1)]);
        t.quest_page(1, matild);
        t.take_writes();
        assert!(t.quest_page(1, torai).contains("30557-01.html"));
        assert_eq!(t.count(1, Q_FOBBIDEN_LOVE_SCROLL), 0);
        assert_eq!(t.count(1, 57), 500000);
    }
}
