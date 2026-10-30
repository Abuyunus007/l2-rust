//! Quest 503: Pursuit of Clan Ambition (a level 4 clan's leader earns the Seal of Aspiration).
// Java: dist/game/data/scripts/quests/Q00503_PursuitOfClanAmbition/Q00503_PursuitOfClanAmbition.java
//
// Pattern: a long clan quest driven by the memo state (1000 .. 10000). The clan leader's quest
// state decides what the members' kills give; the members also hold a state (`lqs != null`) to
// talk to the NPCs.

use l2_protocol_game::NpcStringId;
use l2_protocol_game::server::ChatType;
use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, STARTED};
use l2_world::{Qs, Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const HEAD_BLACKSMITH_KUSTO: i32 = 30512;
const MARTIEN: i32 = 30645;
const WITCH_ATHREA: i32 = 30758;
const WITCH_KALIS: i32 = 30759;
const SIR_GUSTAV_ATHEBALDT: i32 = 30760;
const CORPSE_OF_FRITZ: i32 = 30761;
const CORPSE_OF_LUTZ: i32 = 30762;
const CORPSE_OF_KURTZ: i32 = 30763;
const BALTHAZAR: i32 = 30764;
const IMPERIAL_COFFER: i32 = 30765;
const WITCH_CLEO: i32 = 30766;
const SIR_ERIC_RODEMAI: i32 = 30868;

// Items
const MIST_DRAKES_EGG: i32 = 3839;
const BLITZ_WYRM_EGG: i32 = 3840;
const DRAKES_EGG: i32 = 3841;
const THUNDER_WYRM_EGG: i32 = 3842;
const BROOCH_OF_THE_MAGPIE: i32 = 3843;
const IMPERIAL_KEY: i32 = 3847;
const GUSTAVS_1ST_LETTER: i32 = 3866;
const GUSTAVS_2ND_LETTER: i32 = 3867;
const GUSTAVS_3RD_LETTER: i32 = 3868;
const SCEPTER_OF_JUDGMENT: i32 = 3869;
const BLACK_ANVIL_COIN: i32 = 3871;
const RECIPE_SPITEFUL_SOUL_ENERGY: i32 = 14854;
const SPITEFUL_SOUL_ENERGY: i32 = 14855;
const SPITEFUL_SOUL_VENGEANCE: i32 = 14856;

// Reward
const SEAL_OF_ASPIRATION: i32 = 3870;

// Monsters
const DRAKE: i32 = 20137;
const DRAKE2: i32 = 20285;
const THUNDER_WYRM: i32 = 20243;
const THUNDER_WYRM2: i32 = 20282;
const GRAVE_GUARD: i32 = 20668;
const SPITEFUL_SOUL_LEADER: i32 = 20974;

// Quest Monster
const GRAVE_KEYMASTER: i32 = 27179;
const IMPERIAL_GRAVEKEEPER: i32 = 27181;
const BLITZ_WYRM: i32 = 27178;

/// Java `Q00503_PursuitOfClanAmbition`.
pub struct Q00503PursuitOfClanAmbition;

/// Java `getLeaderQuestState(player, getName())`: the quest state of the clan leader, who has to
/// be online (no state is created).
fn leader_quest_state(ctx: &mut ScriptCtx<'_>, player: ObjectId) -> Option<Qs> {
    let clan_id = ctx.world().clan_id_of(player);
    if clan_id == 0 {
        return None;
    }
    let leader = ctx.world().clan_online_leader(clan_id)?;
    ctx.get_quest_state(leader, false)
}

/// The two Blitz Wyrms that attack the player at a corpse (Java's repeated `addSpawn` pair).
fn spawn_wyrms(ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) {
    for _ in 0..2 {
        if let Some((x, y, z)) = ctx.world().position_of(npc.object_id)
            && let Some(wyrm) = ctx.add_spawn(BLITZ_WYRM, x, y, z, 0, true, 180_000, 0)
        {
            ctx.add_attack_desire(wyrm, player, 999);
        }
    }
}

/// The reward of every ending: the seal and the SP.
fn reward(ctx: &mut ScriptCtx<'_>, player: ObjectId, qs: Qs) {
    ctx.give_items(player, SEAL_OF_ASPIRATION, 1);
    ctx.add_exp_and_sp(player, 0, 250000);
    qs.exit_quest(ctx, false, true);
}

impl Script for Q00503PursuitOfClanAmbition {
    fn name(&self) -> &'static str {
        "Q00503_PursuitOfClanAmbition"
    }

    fn path(&self) -> &'static str {
        "quests/Q00503_PursuitOfClanAmbition"
    }

    fn id(&self) -> i32 {
        503
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[SIR_GUSTAV_ATHEBALDT]);
        s.add_talk_id(&[
            SIR_GUSTAV_ATHEBALDT,
            HEAD_BLACKSMITH_KUSTO,
            MARTIEN,
            WITCH_ATHREA,
            WITCH_KALIS,
            CORPSE_OF_FRITZ,
            CORPSE_OF_LUTZ,
            CORPSE_OF_KURTZ,
            BALTHAZAR,
            IMPERIAL_COFFER,
            WITCH_CLEO,
            SIR_ERIC_RODEMAI,
        ]);
        s.add_kill_id(&[
            DRAKE,
            DRAKE2,
            THUNDER_WYRM,
            THUNDER_WYRM2,
            GRAVE_GUARD,
            SPITEFUL_SOUL_LEADER,
            GRAVE_KEYMASTER,
            BLITZ_WYRM,
            IMPERIAL_GRAVEKEEPER,
        ]);
        s.register_quest_items(&[
            MIST_DRAKES_EGG,
            BLITZ_WYRM_EGG,
            DRAKES_EGG,
            THUNDER_WYRM_EGG,
            BROOCH_OF_THE_MAGPIE,
            IMPERIAL_KEY,
            GUSTAVS_1ST_LETTER,
            GUSTAVS_2ND_LETTER,
            GUSTAVS_3RD_LETTER,
            SCEPTER_OF_JUDGMENT,
            BLACK_ANVIL_COIN,
            RECIPE_SPITEFUL_SOUL_ENERGY,
            SPITEFUL_SOUL_ENERGY,
            SPITEFUL_SOUL_VENGEANCE,
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
            "30760-08.html" => {
                if qs.is_created(ctx) {
                    ctx.give_items(player, GUSTAVS_1ST_LETTER, 1);
                    qs.start_quest(ctx);
                    qs.set_memo_state(ctx, 1000);
                    htmltext = Some(event.to_owned());
                }
            }
            "30760-12.html" => {
                ctx.give_items(player, GUSTAVS_2ND_LETTER, 1);
                qs.set_memo_state(ctx, 4000);
                qs.set_cond(ctx, 4, false);
                htmltext = Some(event.to_owned());
            }
            "30760-16.html" => {
                ctx.give_items(player, GUSTAVS_3RD_LETTER, 1);
                qs.set_memo_state(ctx, 7000);
                qs.set_cond(ctx, 7, false);
                htmltext = Some(event.to_owned());
            }
            "30760-20.html" | "30760-23.html" | "30766-08.html" => {
                if ctx.has_quest_items(player, SCEPTER_OF_JUDGMENT) {
                    reward(ctx, player, qs);
                    htmltext = Some(event.to_owned());
                }
            }
            "30760-22.html" => {
                qs.set_memo_state(ctx, 10000);
                qs.set_cond(ctx, 12, false);
                htmltext = Some(event.to_owned());
            }
            "30512-03.html" => {
                if ctx.has_quest_items(player, BROOCH_OF_THE_MAGPIE) {
                    ctx.take_items(player, BROOCH_OF_THE_MAGPIE, -1);
                    ctx.give_items(player, BLACK_ANVIL_COIN, 1);
                }

                htmltext = Some(event.to_owned());
            }
            "30645-03.html" => {
                ctx.take_items(player, GUSTAVS_1ST_LETTER, -1);
                qs.set_memo_state(ctx, 2000);
                qs.set_cond(ctx, 2, true);
                htmltext = Some(event.to_owned());
            }
            "30761-02.html" => {
                let npc = npc?;
                let memo = qs.get_memo_state(ctx);
                if [2000, 2011, 2010, 2001].contains(&memo) {
                    ctx.give_items(player, BLITZ_WYRM_EGG, 3);
                    qs.set_memo_state(ctx, memo + 100);
                    spawn_wyrms(ctx, npc, player);
                    ctx.start_quest_timer("DESPAWN", 10000, Some(npc), Some(player), false);
                    htmltext = Some(event.to_owned());
                } else if [2100, 2111, 2110, 2101].contains(&memo) {
                    spawn_wyrms(ctx, npc, player);
                    ctx.start_quest_timer("DESPAWN", 10000, Some(npc), Some(player), false);
                    htmltext = Some("30761-03.html".to_owned());
                }
            }
            "30762-02.html" => {
                let npc = npc?;
                let memo = qs.get_memo_state(ctx);
                // NOTE(java-quirk): 2100 is in both lists; the first (the reward) wins.
                if [2000, 2101, 2001, 2100].contains(&memo) {
                    ctx.give_items(player, BLITZ_WYRM_EGG, 3);
                    ctx.give_items(player, MIST_DRAKES_EGG, 4);
                    qs.set_memo_state(ctx, memo + 10);
                    spawn_wyrms(ctx, npc, player);
                    ctx.start_quest_timer("DESPAWN", 10000, Some(npc), Some(player), false);
                    htmltext = Some(event.to_owned());
                } else if [2100, 2111, 2011, 2110].contains(&memo) {
                    spawn_wyrms(ctx, npc, player);
                    ctx.start_quest_timer("DESPAWN", 10000, Some(npc), Some(player), false);
                    htmltext = Some("30762-03.html".to_owned());
                }
            }
            "30763-02.html" => {
                let npc = npc?;
                let memo = qs.get_memo_state(ctx);
                if [2000, 2110, 2010, 2100].contains(&memo) {
                    ctx.give_items(player, BROOCH_OF_THE_MAGPIE, 1);
                    ctx.give_items(player, MIST_DRAKES_EGG, 6);
                    qs.set_memo_state(ctx, memo + 1);
                    ctx.delete_npc(npc);
                    htmltext = Some(event.to_owned());
                }
            }
            "30764-03.html" => {
                ctx.take_items(player, GUSTAVS_2ND_LETTER, -1);
                qs.set_memo_state(ctx, 5000);
                qs.set_cond(ctx, 5, true);
                htmltext = Some(event.to_owned());
            }
            "30764-06.html" => {
                ctx.take_items(player, GUSTAVS_2ND_LETTER, -1);
                ctx.take_items(player, BLACK_ANVIL_COIN, -1);
                ctx.give_items(player, RECIPE_SPITEFUL_SOUL_ENERGY, 1);
                qs.set_memo_state(ctx, 5000);
                qs.set_cond(ctx, 5, true);
                htmltext = Some(event.to_owned());
            }
            "30765-04.html" => {
                ctx.take_items(player, IMPERIAL_KEY, -1);
                ctx.give_items(player, SCEPTER_OF_JUDGMENT, 1);
                qs.set_memo_state(ctx, 8700);
                htmltext = Some(event.to_owned());
            }
            "30766-04.html" => {
                let npc = npc?;
                qs.set_memo_state(ctx, 8100);
                qs.set_cond(ctx, 9, true);
                ctx.broadcast_npc_say(npc, ChatType::NpcGeneral, NpcStringId::BloodAndHonor, &[]);
                ctx.start_quest_timer("SPAWN_WITCH", 5000, Some(npc), Some(player), false);
                htmltext = Some(event.to_owned());
            }
            "30868-04.html" => {
                ctx.take_items(player, GUSTAVS_3RD_LETTER, -1);
                qs.set_memo_state(ctx, 8000);
                qs.set_cond(ctx, 8, true);
                htmltext = Some(event.to_owned());
            }
            "30868-10.html" => {
                qs.set_memo_state(ctx, 9000);
                qs.set_cond(ctx, 11, true);
                htmltext = Some(event.to_owned());
            }
            "30645-06.html" | "30760-05.html" | "30760-06.html" | "30760-07.html" | "30760-21.html"
            | "30764-05.html" | "30765-02.html" | "30765-05a.html" | "30766-03.html" | "30868-03.html"
            | "30868-06a.html" => {
                htmltext = Some(event.to_owned());
            }
            "SPAWN_WITCH" => {
                if let Some(athrea) = ctx.add_spawn(WITCH_ATHREA, 160688, 21296, -3714, 0, false, 180_000, 0)
                {
                    ctx.broadcast_npc_say(athrea, ChatType::NpcGeneral, NpcStringId::WarAndDeath, &[]);
                }
                if let Some(kalis) = ctx.add_spawn(WITCH_KALIS, 160690, 21176, -3712, 0, false, 180_000, 0) {
                    ctx.broadcast_npc_say(kalis, ChatType::NpcGeneral, NpcStringId::AmbitionAndPower, &[]);
                }
            }
            "DESPAWN" => {
                let npc = npc?;
                ctx.delete_npc(npc);
            }
            _ => {}
        }
        htmltext
    }

    // Java keeps the two identical branches of the Grave Guard (a 50 % roll from memo 8505 on, a
    // sure spawn at 8510).
    #[allow(clippy::if_same_then_else)]
    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        let Some(qs) = ctx.get_quest_state(killer, false) else {
            return;
        };
        if !qs.is_started(ctx) || !ctx.check_if_in_range(ctx.alt_party_range(), npc.object_id, killer, true) {
            return;
        }

        let clan_id = ctx.world().clan_id_of(killer);
        if clan_id == 0 {
            return;
        }
        let Some(leader) = ctx.world().clan_online_leader(clan_id) else {
            return;
        };
        if !ctx.check_if_in_range(ctx.alt_party_range(), npc.object_id, leader, true) {
            return;
        }

        let Some(leader_qs) = ctx.get_quest_state(leader, false) else {
            return;
        };

        match npc.id {
            DRAKE | DRAKE2 => {
                // NOTE(java-quirk): `>= 2000 || < 3000` is always true; the drops have no memo gate.
                let memo = leader_qs.get_memo_state(ctx);
                if memo >= 2000 || memo < 3000 {
                    ctx.give_item_randomly(leader, None, MIST_DRAKES_EGG, 1, 1, 10, 0.1, true);
                    ctx.give_item_randomly(leader, None, DRAKES_EGG, 1, 1, 10, 0.5, true);
                }
            }
            THUNDER_WYRM | THUNDER_WYRM2 => {
                // NOTE(java-quirk): the same always-true memo test.
                let memo = leader_qs.get_memo_state(ctx);
                if memo >= 2000 || memo < 3000 {
                    ctx.give_item_randomly(leader, None, THUNDER_WYRM_EGG, 1, 1, 10, 0.5, true);
                }
            }
            GRAVE_GUARD => {
                // NOTE(java-quirk): `< 8511 || >= 8500` is always true.
                let memo = leader_qs.get_memo_state(ctx);
                if memo < 8511 || memo >= 8500 {
                    leader_qs.set_memo_state(ctx, memo + 1);
                    let memo = leader_qs.get_memo_state(ctx);
                    if memo >= 8505 && ctx.get_random(100) < 50 {
                        leader_qs.set_memo_state(ctx, 8500);
                        ctx.add_spawn_at(GRAVE_KEYMASTER, npc.object_id, true, 0);
                    } else if memo >= 8510 {
                        leader_qs.set_memo_state(ctx, 8500);
                        ctx.add_spawn_at(GRAVE_KEYMASTER, npc.object_id, true, 0);
                    }
                }
            }
            SPITEFUL_SOUL_LEADER => {
                if leader_qs.get_memo_state(ctx) == 5000 {
                    let rand = ctx.get_random(100);
                    if rand < 10 {
                        ctx.give_item_randomly(leader, None, SPITEFUL_SOUL_ENERGY, 1, 1, 10, 1.0, false);
                    } else if rand < 60 {
                        ctx.give_items(leader, SPITEFUL_SOUL_VENGEANCE, 1);
                    }
                }
            }
            BLITZ_WYRM => {
                // NOTE(java-quirk): the same always-true memo test.
                let memo = leader_qs.get_memo_state(ctx);
                if memo >= 2000 || memo < 3000 {
                    ctx.give_item_randomly(leader, None, BLITZ_WYRM_EGG, 1, 1, 10, 1.0, true);
                }
            }
            GRAVE_KEYMASTER => {
                if leader_qs.get_memo_state(ctx) >= 8500 {
                    ctx.give_item_randomly(leader, None, IMPERIAL_KEY, 1, 1, 6, 1.0, true);
                }
            }
            IMPERIAL_GRAVEKEEPER => {
                // NOTE(java-quirk): `< 8511 || >= 8500` is always true.
                let memo = leader_qs.get_memo_state(ctx);
                if (memo < 8511 || memo >= 8500)
                    && let Some((x, y, z)) = ctx.world().position_of(npc.object_id)
                    && let Some(coffer) = ctx.add_spawn(IMPERIAL_COFFER, x, y, z, 0, true, 180_000, 0)
                {
                    ctx.broadcast_npc_say(
                        coffer,
                        ChatType::NpcGeneral,
                        NpcStringId::CurseOfTheGodsOnTheOneThatDefilesThePropertyOfTheEmpire,
                        &[],
                    );
                }
            }
            _ => {}
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let lqs = leader_quest_state(ctx, player);
        let mut htmltext = ctx.get_no_quest_msg(player);
        let state = qs.state(ctx);
        if state == CREATED || state == COMPLETED {
            if npc.id == SIR_GUSTAV_ATHEBALDT && lqs.is_some() {
                if ctx.is_clan_leader(player) {
                    if let Some(level) = ctx.clan_level_of(player) {
                        if level < 4 {
                            htmltext = "30760-01.html".to_owned();
                        } else if level >= 5 {
                            htmltext = "30760-02.html".to_owned();
                        } else if level == 4 && ctx.has_quest_items(player, SEAL_OF_ASPIRATION) {
                            htmltext = "30760-03.html".to_owned();
                        } else if level == 4 && !ctx.has_quest_items(player, SEAL_OF_ASPIRATION) {
                            htmltext = "30760-04.html".to_owned();
                        }
                    }
                } else {
                    htmltext = "30760-04t.html".to_owned();
                }
            }
        } else if state == STARTED {
            let memo = qs.get_memo_state(ctx);
            let leader = ctx.is_clan_leader(player);
            match npc.id {
                SIR_GUSTAV_ATHEBALDT => {
                    if lqs.is_some() {
                        if memo == 1000 {
                            htmltext = "30760-09.html".to_owned();
                        } else if memo == 2000 {
                            htmltext = "30760-10.html".to_owned();
                        } else if memo == 3000 {
                            htmltext = if !leader {
                                "30760-11t.html"
                            } else {
                                "30760-11.html"
                            }
                            .to_owned();
                        } else if memo == 4000 {
                            htmltext = "30760-13.html".to_owned();
                        } else if memo == 5000 {
                            htmltext = "30760-14.html".to_owned();
                        } else if memo == 6000 {
                            htmltext = if !leader {
                                "30760-15t.html"
                            } else {
                                "30760-15.html"
                            }
                            .to_owned();
                        } else if memo == 7000 {
                            htmltext = "30760-17.html".to_owned();
                        } else if (8000..8700).contains(&memo) {
                            htmltext = "30760-18.html".to_owned();
                        } else if (8700..10000).contains(&memo) && leader {
                            htmltext = "30760-19.html".to_owned();
                        } else if memo == 9000 && !leader {
                            htmltext = "30760-19t.html".to_owned();
                        } else if memo == 10000 {
                            htmltext = if !leader {
                                "30760-24t.html"
                            } else {
                                "30760-24.html"
                            }
                            .to_owned();
                        }
                    }
                }
                HEAD_BLACKSMITH_KUSTO => {
                    if lqs.is_some() && !leader {
                        htmltext = "30512-01a.html".to_owned();
                    } else if !ctx
                        .has_at_least_one_quest_item(player, &[BROOCH_OF_THE_MAGPIE, BLACK_ANVIL_COIN])
                    {
                        htmltext = "30512-01.html".to_owned();
                    } else if ctx.has_quest_items(player, BROOCH_OF_THE_MAGPIE) {
                        htmltext = "30512-02.html".to_owned();
                    } else if lqs.is_some()
                        && ctx.has_quest_items(player, BLACK_ANVIL_COIN)
                        && !ctx.has_quest_items(player, BROOCH_OF_THE_MAGPIE)
                    {
                        htmltext = "30512-04.html".to_owned();
                    }
                }
                MARTIEN => {
                    if lqs.is_some() {
                        if memo == 1000 {
                            htmltext = if !leader { "30645-01.html" } else { "30645-02.html" }.to_owned();
                        } else if (2000..3000).contains(&memo) {
                            if ctx.get_quest_items_count(player, MIST_DRAKES_EGG) < 10
                                || ctx.get_quest_items_count(player, BLITZ_WYRM_EGG) < 10
                                || ctx.get_quest_items_count(player, THUNDER_WYRM_EGG) < 10
                                || ctx.get_quest_items_count(player, DRAKES_EGG) < 10
                            {
                                htmltext = "30645-04.html".to_owned();
                            } else {
                                ctx.take_items(player, MIST_DRAKES_EGG, -1);
                                ctx.take_items(player, BLITZ_WYRM_EGG, -1);
                                ctx.take_items(player, DRAKES_EGG, -1);
                                ctx.take_items(player, THUNDER_WYRM_EGG, -1);
                                qs.set_memo_state(ctx, 3000);
                                qs.set_cond(ctx, 3, true);
                                htmltext = "30645-05.html".to_owned();
                            }
                        } else if memo == 3000 {
                            htmltext = "30645-07.html".to_owned();
                        } else if memo > 3000 {
                            htmltext = "30645-08.html".to_owned();
                        }
                    }
                }
                WITCH_ATHREA => {
                    if lqs.is_some() {
                        htmltext = "30758-01.html".to_owned();
                    }
                }
                WITCH_KALIS => {
                    if lqs.is_some() {
                        htmltext = "30759-01.html".to_owned();
                    }
                }
                CORPSE_OF_FRITZ => {
                    if (2000..3000).contains(&memo) {
                        htmltext = "30761-01.html".to_owned();
                    }
                }
                CORPSE_OF_LUTZ => {
                    if (2000..3000).contains(&memo) {
                        htmltext = "30762-01.html".to_owned();
                    }
                }
                CORPSE_OF_KURTZ => {
                    if (memo < 3000 && memo == 2000) || memo == 2110 || memo == 2010 || memo == 2100 {
                        htmltext = "30763-01.html".to_owned();
                    } else if memo == 2001 || memo == 2111 || memo == 2011 || memo == 2101 {
                        htmltext = "30763-03.html".to_owned();
                    }
                }
                BALTHAZAR => {
                    if lqs.is_some() {
                        if memo == 4000 {
                            if !leader {
                                htmltext = "30764-01.html".to_owned();
                            } else if !ctx.has_quest_items(player, BLACK_ANVIL_COIN) && leader {
                                htmltext = "30764-02.html".to_owned();
                            } else if ctx.has_quest_items(player, BLACK_ANVIL_COIN) {
                                htmltext = "30764-04.html".to_owned();
                            }
                        } else if memo == 5000 {
                            if ctx.get_quest_items_count(player, SPITEFUL_SOUL_ENERGY) < 10 {
                                htmltext = "30764-07a.html".to_owned();
                            } else {
                                ctx.take_items(player, SPITEFUL_SOUL_ENERGY, -1);
                                qs.set_memo_state(ctx, 6000);
                                qs.set_cond(ctx, 6, true);
                                htmltext = "30764-08a.html".to_owned();
                            }
                        } else if memo >= 6000 {
                            htmltext = "30764-09.html".to_owned();
                        }
                    }
                }
                IMPERIAL_COFFER => {
                    if lqs.is_some() {
                        if (8500..8700).contains(&memo) {
                            if ctx.get_quest_items_count(player, IMPERIAL_KEY) >= 6 {
                                htmltext = if !leader { "30765-01.html" } else { "30765-03.html" }.to_owned();
                            }
                        } else if memo >= 8700 {
                            htmltext = "30765-05.html".to_owned();
                        }
                    }
                }
                WITCH_CLEO => {
                    if lqs.is_some() {
                        if !leader {
                            htmltext = "30766-01.html".to_owned();
                        } else if memo == 8000 {
                            htmltext = "30766-02.html".to_owned();
                        } else if memo == 8100 {
                            htmltext = "30766-05.html".to_owned();
                        } else if memo > 8100 && memo < 10000 {
                            htmltext = "30766-06.html".to_owned();
                        } else if memo == 10000 && leader {
                            htmltext = "30766-07.html".to_owned();
                        }
                    }
                }
                SIR_ERIC_RODEMAI => {
                    if lqs.is_some() {
                        if memo == 7000 {
                            htmltext = if !leader { "30868-01.html" } else { "30868-02.html" }.to_owned();
                        } else if memo == 8000 {
                            htmltext = "30868-05.html".to_owned();
                        } else if memo == 8100 {
                            if leader {
                                qs.set_memo_state(ctx, 8500);
                                qs.set_cond(ctx, 10, true);
                                htmltext = "30868-06.html".to_owned();
                            } else {
                                htmltext = "30868-07.html".to_owned();
                            }
                        } else if (8500..8511).contains(&memo) {
                            htmltext = "30868-08.html".to_owned();
                        } else if memo == 8700 {
                            htmltext = "30868-09.html".to_owned();
                        } else if memo >= 9000 {
                            htmltext = "30868-11.html".to_owned();
                        }
                    }
                }
                _ => {}
            }
        }
        Some(htmltext)
    }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::{Pages, ScriptTest, rows};

    const NAME: &str = "Q00503_PursuitOfClanAmbition";

    const NPCS: &str = r#"
        <npc id="30760" type="Folk" level="70" name="Sir Gustav Athebaldt"/>
        <npc id="30645" type="Folk" level="70" name="Martien"/>
        <npc id="30761" type="Folk" level="70" name="Corpse of Fritz"/>
        <npc id="30762" type="Folk" level="70" name="Corpse of Lutz"/>
        <npc id="30763" type="Folk" level="70" name="Corpse of Kurtz"/>
        <npc id="27178" type="Monster" level="20" name="Blitz Wyrm"/>
        <npc id="20137" type="Monster" level="20" name="Drake"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="3839" type="EtcItem" name="Mist Drake's Egg"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="3840" type="EtcItem" name="Blitz Wyrm Egg"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="3841" type="EtcItem" name="Drake's Egg"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="3842" type="EtcItem" name="Thunder Wyrm Egg"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="3843" type="EtcItem" name="Brooch of the Magpie"><set name="is_questitem" val="true"/></item>
        <item id="3866" type="EtcItem" name="Gustav's 1st Letter"><set name="is_questitem" val="true"/></item>
        <item id="3870" type="EtcItem" name="Seal of Aspiration"/>"#;

    fn setup() -> (ScriptTest, Vec<Oid>) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00503PursuitOfClanAmbition);
        let pages = Pages::of(&Q00503PursuitOfClanAmbition)
            .page("30760-04.html", &["30760-05.html"])
            .page("30760-05.html", &["30760-06.html"])
            .page("30760-06.html", &["30760-07.html"])
            .page("30760-07.html", &["30760-08.html"])
            .page("30645-02.html", &["30645-03.html"])
            .page("30761-01.html", &["30761-02.html"])
            .page("30762-01.html", &["30762-02.html"])
            .page("30763-01.html", &["30763-02.html"])
            .plain(&[
                "30760-01.html",
                "30760-02.html",
                "30760-04t.html",
                "30760-08.html",
                "30760-09.html",
                "30760-10.html",
                "30645-03.html",
                "30645-04.html",
                "30645-05.html",
                "30761-02.html",
                "30761-03.html",
                "30762-02.html",
                "30762-03.html",
                "30763-02.html",
                "30763-03.html",
            ])
            .npcs(&[30760, 30645, 30761, 30762, 30763]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let npcs = [30760, 30645, 30761, 30762, 30763]
            .iter()
            .enumerate()
            .map(|(i, id)| t.spawn(*id, 20 + 5 * i as i32))
            .collect();
        (t, npcs)
    }

    fn leader(t: &mut ScriptTest, clan_level: i32) {
        t.enter(1, 40, vec![]);
        let clan = t.world.create_clan(Oid(1), "TestClan").expect("clan");
        t.world.change_clan_level(clan, clan_level);
    }

    fn memo(t: &mut ScriptTest) -> i32 {
        let mut m = 0;
        t.world.run_as_script(NAME, |ctx| {
            m = ctx.get_quest_state(Oid(1), false).unwrap().get_memo_state(ctx);
        });
        m
    }

    #[test]
    fn a_clanless_character_gets_no_page_from_gustav() {
        let (mut t, npcs) = setup();
        t.enter(1, 40, vec![]);
        let page = t.quest_page(1, npcs[0]);
        assert!(!page.contains("30760-0"), "{page}");
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_clan_level_decides_the_first_page() {
        let (mut t, npcs) = setup();
        leader(&mut t, 3);
        assert!(t.quest_page(1, npcs[0]).contains("30760-01.html"));
        t.world.change_clan_level(t.world.clan_id_of(Oid(1)), 4);
        assert!(t.quest_page(1, npcs[0]).contains("30760-04.html"));
        t.world.change_clan_level(t.world.clan_id_of(Oid(1)), 5);
        assert!(t.quest_page(1, npcs[0]).contains("30760-02.html"));
    }

    #[test]
    fn the_leader_of_a_level_4_clan_starts_and_collects_the_eggs() {
        let (mut t, npcs) = setup();
        leader(&mut t, 4);
        assert!(t.quest_page(1, npcs[0]).contains("30760-04.html"));
        t.event(1, NAME, "30760-05.html");
        t.event(1, NAME, "30760-06.html");
        t.event(1, NAME, "30760-07.html");
        assert!(t.event(1, NAME, "30760-08.html").contains("30760-08.html"));
        assert_eq!(t.count(1, 3866), 1);
        assert_eq!(memo(&mut t), 1000);
        let writes = t.take_writes();
        assert!(
            writes.contains(&rows::insert(NAME, "<state>", "Started")),
            "{writes:?}"
        );

        // Martien takes the letter and sends the leader to the corpses (memo 2000, cond 2).
        assert!(t.quest_page(1, npcs[1]).contains("30645-02.html"));
        t.event(1, NAME, "30645-03.html");
        assert_eq!((memo(&mut t), t.count(1, 3866)), (2000, 0));

        // Fritz: three Blitz Wyrm eggs, two wyrms and the memo + 100.
        assert!(t.quest_page(1, npcs[2]).contains("30761-01.html"));
        assert!(t.event(1, NAME, "30761-02.html").contains("30761-02.html"));
        assert_eq!((memo(&mut t), t.count(1, BLITZ_WYRM_EGG)), (2100, 3));
        assert_eq!(t.npcs_of(BLITZ_WYRM).len(), 2);
        // Once more at Fritz: only the wyrms and the other page.
        t.quest_page(1, npcs[2]);
        t.world.run_as_script(NAME, |ctx| {
            let npc = ScriptNpc {
                object_id: npcs[2],
                id: CORPSE_OF_FRITZ,
            };
            let page = Q00503PursuitOfClanAmbition.on_event(ctx, "30761-02.html", Some(npc), Some(Oid(1)));
            assert_eq!(page, Some("30761-03.html".to_owned()));
        });
        assert_eq!(memo(&mut t), 2100);
        assert_eq!(t.count(1, BLITZ_WYRM_EGG), 3);

        // Lutz at 2100: eggs and drake eggs, memo + 10.
        t.quest_page(1, npcs[3]);
        t.event(1, NAME, "30762-02.html");
        assert_eq!(memo(&mut t), 2110);
        assert_eq!((t.count(1, BLITZ_WYRM_EGG), t.count(1, MIST_DRAKES_EGG)), (6, 4));

        // Kurtz at 2110: the brooch and 6 more drake eggs, memo + 1; he is gone.
        t.quest_page(1, npcs[4]);
        t.event(1, NAME, "30763-02.html");
        assert_eq!(memo(&mut t), 2111);
        assert_eq!(
            (t.count(1, BROOCH_OF_THE_MAGPIE), t.count(1, MIST_DRAKES_EGG)),
            (1, 10)
        );
        assert!(!t.in_world(npcs[4]));
    }

    #[test]
    fn a_drake_kill_drops_to_the_clan_leader() {
        let (mut t, npcs) = setup();
        leader(&mut t, 4);
        t.quest_page(1, npcs[0]);
        t.event(1, NAME, "30760-05.html");
        t.event(1, NAME, "30760-06.html");
        t.event(1, NAME, "30760-07.html");
        t.event(1, NAME, "30760-08.html");
        for _ in 0..200 {
            let drake = t.spawn(20137, 60);
            t.kill(1, drake);
            if t.count(1, DRAKES_EGG) > 0 {
                return;
            }
        }
        panic!("no egg in 200 kills");
    }
}
