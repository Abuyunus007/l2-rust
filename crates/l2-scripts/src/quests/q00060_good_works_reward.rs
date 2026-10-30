//! Quest 60: Good Work's Reward (Blueprint Seller Daeger, Grocer Helveria, Black Marketeer of Mammon, Mark).
// Java: dist/game/data/scripts/quests/Q00060_GoodWorksReward/Q00060_GoodWorksReward.java
//
// Pattern: a memo-state chain with a class-change choice at the end (`docs/design/scripts.md`,
// "Quest patterns"). Java's thirty `REPLY_6`..`REPLY_36` branches differ only in the class they
// need and the three marks they hand out, so they are one table here (`MARK_REPLIES`), checked in
// the same order and with the same "give a mark only when the player has none" rule.
// The Java NPC variables (`SPAWNED` and `PLAYER_ID` of Mark, `npc0`/`player0` of the pursuer) live
// in `ctx.state`, keyed by object id.

use std::collections::{HashMap, HashSet};

use l2_protocol_game::NpcStringId;
use l2_protocol_game::server::ChatType;
use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, STARTED, player_class, race};
use l2_world::subclass::CategoryType;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const GROCER_HELVERIA: i32 = 30081;
const BLACK_MARKETEER_OF_MAMMON: i32 = 31092;
const BLUEPRINT_SELLER_DAEGER: i32 = 31435;
const MARK: i32 = 32487;

// Items
const BLOODY_CLOTH_FRAGMENT: i32 = 10867;
const HELVETIAS_ANTIDOTE: i32 = 10868;
const ADENA_ID: i32 = 57;

// Reward
const MARK_OF_CHALLENGER: i32 = 2627;
const MARK_OF_DUTY: i32 = 2633;
const MARK_OF_SEEKER: i32 = 2673;
const MARK_OF_SCHOLAR: i32 = 2674;
const MARK_OF_PILGRIM: i32 = 2721;
const MARK_OF_TRUST: i32 = 2734;
const MARK_OF_DUELIST: i32 = 2762;
const MARK_OF_SEARCHER: i32 = 2809;
const MARK_OF_HEALER: i32 = 2820;
const MARK_OF_REFORMER: i32 = 2821;
const MARK_OF_MAGUS: i32 = 2840;
const MARK_OF_MAESTRO: i32 = 2867;
const MARK_OF_WARSPIRIT: i32 = 2879;
const MARK_OF_GUILDSMAN: i32 = 3119;
const MARK_OF_LIFE: i32 = 3140;
const MARK_OF_FATE: i32 = 3172;
const MARK_OF_GLORY: i32 = 3203;
const MARK_OF_PROSPERITY: i32 = 3238;
const MARK_OF_CHAMPION: i32 = 3276;
const MARK_OF_SAGITTARIUS: i32 = 3293;
const MARK_OF_WITCHCRAFT: i32 = 3307;
const MARK_OF_SUMMONER: i32 = 3336;
const MARK_OF_LORD: i32 = 3390;

// Quest Monster
const PURSUER: i32 = 27340;

// Misc
const MIN_LEVEL: i32 = 39;
const ONE_MILLION: i64 = 1_000_000;
const TWO_MILLION: i64 = 2_000_000;
const THREE_MILLION: i64 = 3_000_000;

/// Java `Q00211..Q00216` (the first-trial group of `REPLY_1`).
const TRIALS_1: [&str; 6] = [
    "Q00211_TrialOfTheChallenger",
    "Q00212_TrialOfDuty",
    "Q00213_TrialOfTheSeeker",
    "Q00214_TrialOfTheScholar",
    "Q00215_TrialOfThePilgrim",
    "Q00216_TrialOfTheGuildsman",
];
/// Java `Q00217..Q00221`.
const TRIALS_2: [&str; 5] = [
    "Q00217_TestimonyOfTrust",
    "Q00218_TestimonyOfLife",
    "Q00219_TestimonyOfFate",
    "Q00220_TestimonyOfGlory",
    "Q00221_TestimonyOfProsperity",
];
/// Java `Q00222..Q00233`.
const TRIALS_3: [&str; 12] = [
    "Q00222_TestOfTheDuelist",
    "Q00223_TestOfTheChampion",
    "Q00224_TestOfSagittarius",
    "Q00225_TestOfTheSearcher",
    "Q00226_TestOfTheHealer",
    "Q00227_TestOfTheReformer",
    "Q00228_TestOfMagus",
    "Q00229_TestOfWitchcraft",
    "Q00230_TestOfTheSummoner",
    "Q00231_TestOfTheMaestro",
    "Q00232_TestOfTheLord",
    "Q00233_TestOfTheWarSpirit",
];

/// The classes of `REPLY_4`/`REPLY_5` in Java's order; the page of the n-th is `31092-(05+n)` with
/// an `a` for `REPLY_5`.
const SECOND_CLASSES: [i32; 18] = [
    player_class::WARRIOR,
    player_class::KNIGHT,
    player_class::ROGUE,
    player_class::WIZARD,
    player_class::CLERIC,
    player_class::ELVEN_KNIGHT,
    player_class::ELVEN_SCOUT,
    player_class::ELVEN_WIZARD,
    player_class::ORACLE,
    player_class::PALUS_KNIGHT,
    player_class::ASSASSIN,
    player_class::DARK_WIZARD,
    player_class::SHILLIEN_ORACLE,
    player_class::ORC_RAIDER,
    player_class::ORC_MONK,
    player_class::ORC_SHAMAN,
    player_class::SCAVENGER,
    player_class::ARTISAN,
];

/// `REPLY_6`..`REPLY_36`: (reply number, the class that needs it, the three marks).
const MARK_REPLIES: [(i32, i32, [i32; 3]); 31] = [
    (
        6,
        player_class::WARRIOR,
        [MARK_OF_CHALLENGER, MARK_OF_TRUST, MARK_OF_DUELIST],
    ),
    (
        7,
        player_class::WARRIOR,
        [MARK_OF_CHALLENGER, MARK_OF_TRUST, MARK_OF_CHAMPION],
    ),
    (
        8,
        player_class::KNIGHT,
        [MARK_OF_DUTY, MARK_OF_TRUST, MARK_OF_HEALER],
    ),
    (
        9,
        player_class::KNIGHT,
        [MARK_OF_DUTY, MARK_OF_TRUST, MARK_OF_WITCHCRAFT],
    ),
    (
        10,
        player_class::ROGUE,
        [MARK_OF_SEEKER, MARK_OF_TRUST, MARK_OF_SEARCHER],
    ),
    (
        11,
        player_class::ROGUE,
        [MARK_OF_SEEKER, MARK_OF_TRUST, MARK_OF_SAGITTARIUS],
    ),
    (
        12,
        player_class::WIZARD,
        [MARK_OF_SCHOLAR, MARK_OF_TRUST, MARK_OF_MAGUS],
    ),
    (
        13,
        player_class::WIZARD,
        [MARK_OF_SCHOLAR, MARK_OF_TRUST, MARK_OF_WITCHCRAFT],
    ),
    (
        14,
        player_class::WIZARD,
        [MARK_OF_SCHOLAR, MARK_OF_TRUST, MARK_OF_SUMMONER],
    ),
    (
        15,
        player_class::CLERIC,
        [MARK_OF_PILGRIM, MARK_OF_TRUST, MARK_OF_HEALER],
    ),
    (
        16,
        player_class::CLERIC,
        [MARK_OF_PILGRIM, MARK_OF_TRUST, MARK_OF_REFORMER],
    ),
    (
        17,
        player_class::ELVEN_KNIGHT,
        [MARK_OF_DUTY, MARK_OF_LIFE, MARK_OF_HEALER],
    ),
    (
        18,
        player_class::ELVEN_KNIGHT,
        [MARK_OF_CHALLENGER, MARK_OF_LIFE, MARK_OF_DUELIST],
    ),
    (
        19,
        player_class::ELVEN_SCOUT,
        [MARK_OF_SEEKER, MARK_OF_LIFE, MARK_OF_SEARCHER],
    ),
    (
        20,
        player_class::ELVEN_SCOUT,
        [MARK_OF_SEEKER, MARK_OF_LIFE, MARK_OF_SAGITTARIUS],
    ),
    (
        21,
        player_class::ELVEN_WIZARD,
        [MARK_OF_SCHOLAR, MARK_OF_LIFE, MARK_OF_MAGUS],
    ),
    (
        22,
        player_class::ELVEN_WIZARD,
        [MARK_OF_SCHOLAR, MARK_OF_LIFE, MARK_OF_SUMMONER],
    ),
    (
        23,
        player_class::ORACLE,
        [MARK_OF_PILGRIM, MARK_OF_LIFE, MARK_OF_HEALER],
    ),
    (
        24,
        player_class::PALUS_KNIGHT,
        [MARK_OF_DUTY, MARK_OF_FATE, MARK_OF_WITCHCRAFT],
    ),
    (
        25,
        player_class::PALUS_KNIGHT,
        [MARK_OF_CHALLENGER, MARK_OF_FATE, MARK_OF_DUELIST],
    ),
    (
        26,
        player_class::ASSASSIN,
        [MARK_OF_SEEKER, MARK_OF_FATE, MARK_OF_SEARCHER],
    ),
    (
        27,
        player_class::ASSASSIN,
        [MARK_OF_SEEKER, MARK_OF_FATE, MARK_OF_SAGITTARIUS],
    ),
    (
        28,
        player_class::DARK_WIZARD,
        [MARK_OF_SCHOLAR, MARK_OF_FATE, MARK_OF_MAGUS],
    ),
    (
        29,
        player_class::DARK_WIZARD,
        [MARK_OF_SCHOLAR, MARK_OF_FATE, MARK_OF_SUMMONER],
    ),
    (
        30,
        player_class::SHILLIEN_ORACLE,
        [MARK_OF_PILGRIM, MARK_OF_FATE, MARK_OF_REFORMER],
    ),
    (
        31,
        player_class::ORC_RAIDER,
        [MARK_OF_CHALLENGER, MARK_OF_GLORY, MARK_OF_CHAMPION],
    ),
    (
        32,
        player_class::ORC_MONK,
        [MARK_OF_CHALLENGER, MARK_OF_GLORY, MARK_OF_DUELIST],
    ),
    (
        33,
        player_class::ORC_SHAMAN,
        [MARK_OF_PILGRIM, MARK_OF_GLORY, MARK_OF_LORD],
    ),
    (
        34,
        player_class::ORC_SHAMAN,
        [MARK_OF_PILGRIM, MARK_OF_GLORY, MARK_OF_WARSPIRIT],
    ),
    (
        35,
        player_class::SCAVENGER,
        [MARK_OF_GUILDSMAN, MARK_OF_PROSPERITY, MARK_OF_SEARCHER],
    ),
    (
        36,
        player_class::ARTISAN,
        [MARK_OF_GUILDSMAN, MARK_OF_PROSPERITY, MARK_OF_MAESTRO],
    ),
];

/// The Java NPC variables: `SPAWNED`/`PLAYER_ID` of Mark, `npc0`/`player0` of a pursuer.
#[derive(Default)]
struct GoodWorksVars {
    /// Marks with `SPAWNED` set.
    spawned: HashSet<ObjectId>,
    /// `PLAYER_ID` of a mark.
    mark_player: HashMap<ObjectId, i32>,
    /// A pursuer: (its `npc0` Mark, its `PLAYER_ID` and `player0`).
    pursuer: HashMap<ObjectId, (ObjectId, ObjectId)>,
}

/// True when the player finished any quest of `names`.
fn any_completed(ctx: &ScriptCtx<'_>, player: ObjectId, names: &[&str]) -> bool {
    names.iter().any(|n| {
        ctx.player_quest_state(player, n)
            .is_some_and(|q| q.is_completed())
    })
}

/// Java `npc0.getVariables().set("SPAWNED", false)` for the Mark a pursuer came from.
fn release_mark(ctx: &mut ScriptCtx<'_>, pursuer: ObjectId) {
    let mark = ctx.state::<GoodWorksVars>().pursuer.get(&pursuer).map(|p| p.0);
    if let Some(mark) = mark {
        ctx.state::<GoodWorksVars>().spawned.remove(&mark);
    }
}

/// Java `Q00060_GoodWorksReward`.
pub struct Q00060GoodWorksReward;

impl Script for Q00060GoodWorksReward {
    fn name(&self) -> &'static str {
        "Q00060_GoodWorksReward"
    }

    fn path(&self) -> &'static str {
        "quests/Q00060_GoodWorksReward"
    }

    fn id(&self) -> i32 {
        60
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[BLUEPRINT_SELLER_DAEGER]);
        s.add_talk_id(&[
            BLUEPRINT_SELLER_DAEGER,
            GROCER_HELVERIA,
            BLACK_MARKETEER_OF_MAMMON,
            MARK,
        ]);
        s.add_kill_id(&[PURSUER]);
        s.add_spawn_id(&[PURSUER]);
        s.register_quest_items(&[BLOODY_CLOTH_FRAGMENT, HELVETIAS_ANTIDOTE]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        if event == "DESPAWN" {
            let npc = npc?;
            ctx.broadcast_npc_say(
                npc,
                ChatType::NpcGeneral,
                NpcStringId::YouHaveGoodLuckIShallReturn,
                &[],
            );
            release_mark(ctx, npc.object_id);
            ctx.state::<GoodWorksVars>().pursuer.remove(&npc.object_id);
            ctx.delete_npc(npc);
            // Java `super.onEvent(..)`: null.
            return None;
        }

        let player = player?;
        let qs = ctx.get_quest_state(player, false)?;

        let mut htmltext = None;
        match event {
            "31435-07.htm" => {
                if qs.is_created(ctx) {
                    qs.start_quest(ctx);
                    qs.set_memo_state(ctx, 1);
                    htmltext = Some(event.to_owned());
                }
            }
            "31435-02.htm" => htmltext = Some(event.to_owned()),
            "31435-10.html" => {
                if qs.is_memo_state(ctx, 3) {
                    qs.set_memo_state(ctx, 4);
                    qs.set_cond(ctx, 4, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "31435-14.html" => {
                if qs.is_memo_state(ctx, 8) {
                    qs.set_memo_state(ctx, 9);
                    qs.set_cond(ctx, 9, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "30081-02.html" => {
                if qs.is_memo_state(ctx, 4) {
                    htmltext = Some(event.to_owned());
                }
            }
            "30081-03.html" => {
                if qs.is_memo_state(ctx, 4) {
                    ctx.take_items(player, BLOODY_CLOTH_FRAGMENT, -1);
                    qs.set_memo_state(ctx, 5);
                    qs.set_cond(ctx, 5, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "30081-05.html" => {
                let memo_state = qs.get_memo_state(ctx);
                if (5..=6).contains(&memo_state) {
                    if ctx.get_quest_items_count(player, ADENA_ID) >= THREE_MILLION {
                        ctx.give_items(player, HELVETIAS_ANTIDOTE, 1);
                        ctx.take_items(player, ADENA_ID, THREE_MILLION);
                        qs.set_memo_state(ctx, 7);
                        qs.set_cond(ctx, 7, true);
                        htmltext = Some(event.to_owned());
                    } else {
                        qs.set_memo_state(ctx, 6);
                        qs.set_cond(ctx, 6, true);
                        htmltext = Some("30081-06.html".to_owned());
                    }
                }
            }
            "30081-07.html" => {
                if qs.is_memo_state(ctx, 5) {
                    qs.set_memo_state(ctx, 6);
                    qs.set_cond(ctx, 6, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "REPLY_1" => {
                if qs.is_memo_state(ctx, 10) {
                    // Java's nested ifs: the number of the three groups the player has finished
                    // a quest of, set only when it is not zero.
                    let groups = i32::from(any_completed(ctx, player, &TRIALS_1))
                        + i32::from(any_completed(ctx, player, &TRIALS_2))
                        + i32::from(any_completed(ctx, player, &TRIALS_3));
                    if groups > 0 {
                        qs.set_memo_state_ex(ctx, 1, groups);
                    }
                    htmltext = Some("31092-02.html".to_owned());
                }
            }
            "REPLY_2" => {
                if qs.is_memo_state(ctx, 10) {
                    if qs.get_memo_state_ex(ctx, 1) >= 3 {
                        htmltext = Some("31092-03b.html".to_owned());
                    } else if qs.get_memo_state_ex(ctx, 1) >= 1 {
                        htmltext = Some("31092-03.html".to_owned());
                    } else {
                        htmltext = Some("31092-03a.html".to_owned());
                    }
                }
            }
            "REPLY_3" => {
                if qs.is_memo_state(ctx, 10) {
                    let ex = qs.get_memo_state_ex(ctx, 1);
                    if ex >= 3 {
                        ctx.give_items(player, ADENA_ID, THREE_MILLION);
                        htmltext = Some("31092-04a.html".to_owned());
                    } else if ex == 2 {
                        ctx.give_items(player, ADENA_ID, TWO_MILLION);
                        htmltext = Some("31092-04b.html".to_owned());
                    } else if ex == 1 {
                        ctx.give_items(player, ADENA_ID, ONE_MILLION);
                        htmltext = Some("31092-04b.html".to_owned());
                    }
                    qs.exit_quest(ctx, false, true);
                }
            }
            "REPLY_4" => {
                if qs.is_memo_state(ctx, 10) {
                    let class = ctx.player_class_id(player);
                    if let Some(n) = SECOND_CLASSES.iter().position(|&c| c == class) {
                        htmltext = Some(format!("31092-{:02}.html", 5 + n));
                    }
                    qs.exit_quest(ctx, false, true);
                }
            }
            "REPLY_5" => {
                if ctx.world().is_in_category(player, CategoryType::SecondClassGroup) {
                    let class = ctx.player_class_id(player);
                    if let Some(n) = SECOND_CLASSES.iter().position(|&c| c == class) {
                        htmltext = Some(format!("31092-{:02}a.html", 5 + n));
                    }
                }
            }
            "32487-04.html" => {
                if qs.is_memo_state(ctx, 1) {
                    let npc = npc?;
                    if !ctx.state::<GoodWorksVars>().spawned.contains(&npc.object_id) {
                        ctx.state::<GoodWorksVars>().spawned.insert(npc.object_id);
                        ctx.state::<GoodWorksVars>()
                            .mark_player
                            .insert(npc.object_id, player.0);
                        if let Some((x, y, z)) = ctx.world().position_of(player)
                            && let Some(pursuer) = ctx.add_spawn(PURSUER, x + 50, y + 50, z, 0, false, 0, 0)
                        {
                            ctx.state::<GoodWorksVars>()
                                .pursuer
                                .insert(pursuer.object_id, (npc.object_id, player));
                            ctx.add_attack_desire(pursuer, player, 999);
                        }
                        htmltext = Some(event.to_owned());
                    } else {
                        htmltext = Some("32487-05.html".to_owned());
                    }
                }
            }
            "32487-10.html" => {
                if qs.is_memo_state(ctx, 7) {
                    ctx.take_items(player, HELVETIAS_ANTIDOTE, 1);
                    qs.set_memo_state(ctx, 8);
                    qs.set_cond(ctx, 8, true);
                    if let Some(npc) = npc {
                        ctx.state::<GoodWorksVars>().spawned.remove(&npc.object_id);
                    }
                    htmltext = Some(event.to_owned());
                }
            }
            _ => {
                if let Some(reply) = event.strip_prefix("REPLY_").and_then(|n| n.parse::<i32>().ok())
                    && let Some((_, class, marks)) = MARK_REPLIES.iter().find(|r| r.0 == reply)
                    && ctx.player_class_id(player) == *class
                {
                    for mark in marks {
                        if !ctx.has_quest_items(player, *mark) {
                            ctx.give_items(player, *mark, 1);
                        }
                    }
                    htmltext = Some("31092-25.html".to_owned());
                }
            }
        }
        htmltext
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        let qs = ctx.get_quest_state(killer, false);
        if let Some(qs) = qs
            && qs.is_started(ctx)
            && ctx.check_if_in_range(ctx.alt_party_range(), npc.object_id, killer, true)
            && qs.is_memo_state(ctx, 1)
        {
            let owner = ctx
                .state::<GoodWorksVars>()
                .pursuer
                .get(&npc.object_id)
                .map(|p| p.1.0);
            if owner == Some(killer.0) {
                qs.set_memo_state(ctx, 2);
                qs.set_cond(ctx, 2, true);
                ctx.broadcast_npc_say(
                    npc,
                    ChatType::NpcGeneral,
                    NpcStringId::YouAreStrongThisWasAMistake,
                    &[],
                );
            } else {
                ctx.broadcast_npc_say(
                    npc,
                    ChatType::NpcGeneral,
                    NpcStringId::WhoAreYouToJoinInTheBattleHowUpsetting,
                    &[],
                );
            }
        }
        // Always release Mark's lock when the pursuer dies, even if killed by someone without the quest.
        release_mark(ctx, npc.object_id);
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let memo_state = qs.get_memo_state(ctx);
        let mut htmltext = ctx.get_no_quest_msg(player);
        match qs.state(ctx) {
            CREATED => {
                if npc.id == BLUEPRINT_SELLER_DAEGER {
                    if ctx.player_race(player) != race::KAMAEL {
                        if ctx.world().is_in_category(player, CategoryType::SecondClassGroup) {
                            htmltext = if ctx.player_level(player) >= MIN_LEVEL {
                                "31435-01.htm"
                            } else {
                                "31435-03.htm"
                            }
                            .to_owned();
                        } else {
                            htmltext = "31435-04.htm".to_owned();
                        }
                    } else {
                        htmltext = "31435-06.htm".to_owned();
                    }
                }
            }
            STARTED => match npc.id {
                BLUEPRINT_SELLER_DAEGER => {
                    if memo_state <= 2 {
                        htmltext = "31435-08.html".to_owned();
                    } else if memo_state == 3 {
                        htmltext = "31435-09.html".to_owned();
                    } else if memo_state == 4 {
                        htmltext = "31435-11.html".to_owned();
                    } else if memo_state > 4 && memo_state < 8 {
                        htmltext = "31435-12.html".to_owned();
                    } else if memo_state == 8 {
                        htmltext = "31435-13.html".to_owned();
                    } else if memo_state == 9 {
                        qs.set_memo_state(ctx, 10);
                        qs.set_cond(ctx, 10, true);
                        htmltext = "31435-15.html".to_owned();
                    } else if memo_state == 10 {
                        htmltext = "31435-16.html".to_owned();
                    }
                }
                GROCER_HELVERIA => {
                    if memo_state == 4 {
                        htmltext = "30081-01.html".to_owned();
                    } else if memo_state == 5 {
                        htmltext = "30081-04.html".to_owned();
                    } else if memo_state == 6 {
                        htmltext = "30081-08.html".to_owned();
                    } else if memo_state == 7 {
                        if !ctx.has_quest_items(player, HELVETIAS_ANTIDOTE) {
                            ctx.give_items(player, HELVETIAS_ANTIDOTE, 1);
                            htmltext = "30081-09.html".to_owned();
                        } else {
                            htmltext = "30081-10.html".to_owned();
                        }
                    }
                }
                BLACK_MARKETEER_OF_MAMMON => {
                    if memo_state == 10 {
                        if ctx.world().is_in_category(player, CategoryType::SecondClassGroup) {
                            qs.set_memo_state_ex(ctx, 1, 0);
                            htmltext = "31092-01.html".to_owned();
                        } else {
                            ctx.give_items(player, ADENA_ID, THREE_MILLION);
                            qs.exit_quest(ctx, false, true);
                            htmltext = "31092-01a.html".to_owned();
                        }
                    }
                }
                MARK => {
                    if memo_state == 1 {
                        let (spawned, owner) = {
                            let vars = ctx.state::<GoodWorksVars>();
                            (
                                vars.spawned.contains(&npc.object_id),
                                vars.mark_player.get(&npc.object_id).copied().unwrap_or(0),
                            )
                        };
                        if !spawned {
                            htmltext = "32487-01.html".to_owned();
                        } else if owner == player.0 {
                            htmltext = "32487-03.html".to_owned();
                        } else {
                            htmltext = "32487-02.html".to_owned();
                        }
                    } else if memo_state == 2 {
                        ctx.give_items(player, BLOODY_CLOTH_FRAGMENT, 1);
                        qs.set_memo_state(ctx, 3);
                        qs.set_cond(ctx, 3, true);
                        htmltext = "32487-06.html".to_owned();
                    } else if (3..7).contains(&memo_state) {
                        htmltext = "32487-07.html".to_owned();
                    } else if memo_state == 7 {
                        htmltext = "32487-09.html".to_owned();
                    }
                }
                _ => {}
            },
            COMPLETED => {
                if npc.id == BLUEPRINT_SELLER_DAEGER {
                    htmltext = ctx.get_already_completed_msg(player);
                } else if npc.id == BLACK_MARKETEER_OF_MAMMON {
                    if ctx.world().is_in_category(player, CategoryType::SecondClassGroup) {
                        htmltext = "31092-23.html".to_owned();
                    } else {
                        htmltext = "31092-24.html".to_owned();
                    }
                }
            }
            _ => {}
        }
        Some(htmltext)
    }

    fn on_spawn(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc) {
        ctx.start_quest_timer("DESPAWN", 60_000, Some(npc), None, false);
        // The pursuer's `player0` was set right after `addSpawn`, before this queued hook ran.
        let player = ctx
            .state::<GoodWorksVars>()
            .pursuer
            .get(&npc.object_id)
            .map(|p| p.1);
        if let Some(player) = player
            && ctx.world().player(player).is_some()
        {
            let name = ctx.player_name(player);
            ctx.broadcast_npc_say(
                npc,
                ChatType::NpcGeneral,
                NpcStringId::S1IMustKillYouBlameYourOwnCuriosity,
                &[&name],
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::{Pages, ScriptTest, rows};

    const NAME: &str = "Q00060_GoodWorksReward";

    const NPCS: &str = r#"
        <npc id="30081" type="Folk" level="70" name="Helveria"/>
        <npc id="31092" type="Folk" level="70" name="Black Marketeer"/>
        <npc id="31435" type="Folk" level="70" name="Daeger"/>
        <npc id="32487" type="Folk" level="70" name="Mark"/>
        <npc id="27340" type="Monster" level="40" name="Pursuer"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="10867" type="EtcItem" name="Cloth"><set name="is_questitem" val="true"/></item>
        <item id="10868" type="EtcItem" name="Antidote"><set name="is_questitem" val="true"/></item>
        <item id="2627" type="EtcItem" name="Mark of Challenger"/>
        <item id="2734" type="EtcItem" name="Mark of Trust"/>
        <item id="2762" type="EtcItem" name="Mark of Duelist"/>"#;

    /// A stand-in for a quest this one reads.
    struct Other(&'static str, i32);

    impl Script for Other {
        fn name(&self) -> &'static str {
            self.0
        }
        fn path(&self) -> &'static str {
            "quests/Other"
        }
        fn id(&self) -> i32 {
            self.1
        }
        fn setup(&self, _setup: &mut ScriptSetup) {}
    }

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Other(TRIALS_1[0], 211));
        registry.add(Other(TRIALS_2[0], 217));
        registry.add(Q00060GoodWorksReward);
        let pages = Pages::of(&Q00060GoodWorksReward)
            .page("31435-01.htm", &["31435-07.htm", "31435-02.htm"])
            .plain(&[
                "31435-02.htm",
                "31435-03.htm",
                "31435-04.htm",
                "31435-06.htm",
                "31435-07.htm",
            ])
            .npcs(&[
                BLUEPRINT_SELLER_DAEGER,
                GROCER_HELVERIA,
                BLACK_MARKETEER_OF_MAMMON,
                MARK,
            ]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let daeger = t.spawn(BLUEPRINT_SELLER_DAEGER, 20);
        (t, daeger)
    }

    fn ev(t: &mut ScriptTest, event: &str, npc: Option<Oid>) -> Option<String> {
        let mut out = None;
        t.world.run_as_script(NAME, |ctx| {
            let npc = npc.and_then(|n| ctx.script_npc(n));
            out = Q00060GoodWorksReward.on_event(ctx, event, npc, Some(Oid(1)));
        });
        out
    }

    fn talk(t: &mut ScriptTest, npc: Oid) -> Option<String> {
        let mut out = None;
        t.world.run_as_script(NAME, |ctx| {
            if let Some(npc) = ctx.script_npc(npc) {
                out = Q00060GoodWorksReward.on_talk(ctx, npc, Oid(1));
            }
        });
        out
    }

    fn writes(t: &ScriptTest) -> Vec<l2_world::QuestWrite> {
        t.take_writes()
            .into_iter()
            .filter(|w| !format!("{w:?}").contains("__compltdStateFlags"))
            .collect()
    }

    fn memo(t: &mut ScriptTest, value: i32) {
        t.world.run_as_script(NAME, |ctx| {
            if let Some(qs) = ctx.get_quest_state(Oid(1), false) {
                qs.set_memo_state(ctx, value);
            }
        });
        t.take_writes();
    }

    fn finish(t: &mut ScriptTest, name: &'static str) {
        t.world.run_as_script(name, |ctx| {
            if let Some(qs) = ctx.quest_state_of(name, Oid(1), true) {
                qs.set_state(ctx, COMPLETED);
            }
        });
        t.take_writes();
    }

    #[test]
    fn it_needs_a_second_class_level_39_and_no_kamael() {
        // A first-class human is sent away, a warrior of level 38 too, a Kamael too.
        let (mut t, daeger) = setup();
        t.enter_as(1, 40, race::HUMAN, player_class::FIGHTER, vec![]);
        assert!(t.quest_page(1, daeger).contains("31435-04.htm"));

        let (mut t, daeger) = setup();
        t.enter_as(1, 38, race::HUMAN, player_class::WARRIOR, vec![]);
        assert!(t.quest_page(1, daeger).contains("31435-03.htm"));

        let (mut t, daeger) = setup();
        t.enter_as(1, 40, race::KAMAEL, player_class::WARRIOR, vec![]);
        assert!(t.quest_page(1, daeger).contains("31435-06.htm"));
        assert!(writes(&t).is_empty());
    }

    #[test]
    fn the_pursuer_the_cloth_and_the_chain_to_the_marks() {
        let (mut t, daeger) = setup();
        t.enter_as(1, 40, race::HUMAN, player_class::WARRIOR, vec![]);
        assert!(t.quest_page(1, daeger).contains("31435-01.htm"));
        assert!(t.event(1, NAME, "31435-07.htm").contains("31435-07.htm"));
        assert_eq!(
            writes(&t),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started"),
                rows::insert(NAME, "memoState", "1"),
            ]
        );

        // Mark: the first click spawns the pursuer, a second one is refused; killing it moves on.
        let mark = t.spawn(MARK, 40);
        assert_eq!(talk(&mut t, mark).as_deref(), Some("32487-01.html"));
        assert_eq!(
            ev(&mut t, "32487-04.html", Some(mark)).as_deref(),
            Some("32487-04.html")
        );
        assert_eq!(talk(&mut t, mark).as_deref(), Some("32487-03.html"));
        assert_eq!(
            ev(&mut t, "32487-04.html", Some(mark)).as_deref(),
            Some("32487-05.html")
        );
        let pursuer = t.npcs_of(PURSUER)[0];
        t.kill(1, pursuer);
        assert_eq!(talk(&mut t, mark).as_deref(), Some("32487-06.html"));
        assert_eq!(t.count(1, BLOODY_CLOTH_FRAGMENT), 1);
        writes(&t);

        // Daeger (memo 3 -> 4), Helveria takes the cloth (memo 5), no money: memo 6.
        assert_eq!(
            ev(&mut t, "31435-10.html", None).as_deref(),
            Some("31435-10.html")
        );
        let helveria = t.spawn(GROCER_HELVERIA, 60);
        assert_eq!(talk(&mut t, helveria).as_deref(), Some("30081-01.html"));
        assert_eq!(
            ev(&mut t, "30081-03.html", None).as_deref(),
            Some("30081-03.html")
        );
        assert_eq!(t.count(1, BLOODY_CLOTH_FRAGMENT), 0);
        assert_eq!(
            ev(&mut t, "30081-05.html", None).as_deref(),
            Some("30081-06.html")
        );
        // With 3 million: the antidote.
        t.world
            .run_as_script(NAME, |ctx| ctx.give_items(Oid(1), ADENA_ID, THREE_MILLION));
        assert_eq!(
            ev(&mut t, "30081-05.html", None).as_deref(),
            Some("30081-05.html")
        );
        assert_eq!((t.count(1, HELVETIAS_ANTIDOTE), t.count(1, ADENA_ID)), (1, 0));
        assert_eq!(
            ev(&mut t, "32487-10.html", Some(mark)).as_deref(),
            Some("32487-10.html")
        );
        assert_eq!(
            ev(&mut t, "31435-14.html", None).as_deref(),
            Some("31435-14.html")
        );
        writes(&t);

        // Daeger sends to the marketeer (memo 10); trials: Q211 + Q217 done -> two groups.
        assert_eq!(talk(&mut t, daeger).as_deref(), Some("31435-15.html"));
        finish(&mut t, TRIALS_1[0]);
        finish(&mut t, TRIALS_2[0]);
        let marketeer = t.spawn(BLACK_MARKETEER_OF_MAMMON, 80);
        assert_eq!(talk(&mut t, marketeer).as_deref(), Some("31092-01.html"));
        assert_eq!(ev(&mut t, "REPLY_1", None).as_deref(), Some("31092-02.html"));
        assert_eq!(ev(&mut t, "REPLY_2", None).as_deref(), Some("31092-03.html"));
        writes(&t);

        // A class-change reward (REPLY_6 for a warrior): three marks, the quest stays open.
        assert_eq!(ev(&mut t, "REPLY_6", None).as_deref(), Some("31092-25.html"));
        assert_eq!(
            (
                t.count(1, MARK_OF_CHALLENGER),
                t.count(1, MARK_OF_TRUST),
                t.count(1, MARK_OF_DUELIST)
            ),
            (1, 1, 1)
        );
        // Not for another class' reply.
        assert_eq!(ev(&mut t, "REPLY_8", None), None);
        assert_eq!(ev(&mut t, "REPLY_4", None).as_deref(), Some("31092-05.html"));
        writes(&t);
    }

    #[test]
    fn the_adena_reward_depends_on_the_groups() {
        let (mut t, daeger) = setup();
        t.enter_as(1, 40, race::HUMAN, player_class::WARRIOR, vec![]);
        t.quest_page(1, daeger);
        t.event(1, NAME, "31435-07.htm");
        memo(&mut t, 10);
        finish(&mut t, TRIALS_1[0]);
        finish(&mut t, TRIALS_2[0]);
        assert_eq!(ev(&mut t, "REPLY_1", None).as_deref(), Some("31092-02.html"));
        assert_eq!(ev(&mut t, "REPLY_3", None).as_deref(), Some("31092-04b.html"));
        assert_eq!(t.count(1, ADENA_ID), TWO_MILLION);
        let w = writes(&t);
        assert!(w.contains(&rows::delete(NAME, false)));
        assert!(w.contains(&rows::update(NAME, "<state>", "Completed")));
    }
}
